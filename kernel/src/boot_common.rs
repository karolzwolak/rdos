use crate::memory::paging::MemoryMapFrameAllocator;
use crate::util::ap_entry::{ap_core_entry_point, init_core};
use crate::{
    LIMINE_BASE_REVISION, graphics, interrupts, memory, memory::allocator, serial_println,
};
use crate::{MAX_CORES, serial_println_core};
use limine::{
    BaseRevision, RequestsEndMarker, RequestsStartMarker,
    mp::MpGotoFunction,
    paging::PagingMode,
    request::{
        EfiMemmapRequest, FramebufferRequest, HhdmRequest, MemmapRequest, MpRequest,
        PagingModeRequest, RsdpRequest,
    },
};

const LIMINE_MP_FLAG_NO_X2APIC: u64 = 0;

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::with_revision(LIMINE_BASE_REVISION);

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static RSDP_REUEST: RsdpRequest = RsdpRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static EFI_MEMMAP_REQUEST: EfiMemmapRequest = EfiMemmapRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static MEMMAP_REQUEST: MemmapRequest = MemmapRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static PAGING_MODE_REQUEST: PagingModeRequest = PagingModeRequest::new(
    PagingMode::X86_64_4LVL,
    PagingMode::X86_64_4LVL,
    PagingMode::X86_64_4LVL,
);

#[used]
#[unsafe(link_section = ".requests")]
static MP_REQUEST: MpRequest = MpRequest::new(LIMINE_MP_FLAG_NO_X2APIC);

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();


/// # Safety
///
/// Must be called exactly once during BSP boot, with Limine requests mapped and
/// while running on the bootloader-provided stack. Initializes GDT/IDT, paging, heap, 
/// bootstraps AP cores
pub unsafe fn bsp_init() {
    assert!(BASE_REVISION.is_supported());

    serial_println!("BigOS Booted!");

    let _efi_memory_map_response = EFI_MEMMAP_REQUEST
        .response()
        .expect("Failed to get UEFI memory map response");

    let memory_map_response = MEMMAP_REQUEST
        .response()
        .expect("Failed to get memory map response");

    let paging_mode_response = PAGING_MODE_REQUEST
        .response()
        .expect("Failed to get paging mode response");
    let _paging_mode = paging_mode_response.mode;

    let hhdm_response = HHDM_REQUEST.response().expect("Failed to get HHDM respone");
    let hhdm_offset = hhdm_response.offset;

    let rsdp_addr_respone = RSDP_REUEST
        .response()
        .expect("Failed to get RSDP address response");
    let rsdp_virt_addr: usize = rsdp_addr_respone.address as usize;
    let rsdp_phys_addr = rsdp_virt_addr - hhdm_offset as usize;

    serial_println!("HHDM offset: {:#x}", hhdm_offset);
    serial_println!("RSDP physical address: {:#x}", rsdp_phys_addr);
    serial_println!("RSDP virtual address: {:#x}", rsdp_virt_addr);

    let framebuffer_response = FRAMEBUFFER_REQUEST
        .response()
        .expect("Failed to get framebuffer response");
    let framebuffer = framebuffer_response.framebuffers().first().unwrap();
    graphics::framebuffer::init_framebuffer(framebuffer);

    unsafe {
        crate::gdt::init_core_gdt(0);
    }
    interrupts::init_idt();

    let mut mapper = unsafe { memory::paging::init_offset_page_table(hhdm_offset) };
    serial_println!("Offset page table initialized");

    serial_println!("Creating frame_allocator");
    let mut frame_allocator =
        unsafe { MemoryMapFrameAllocator::init(memory_map_response.entries()) };

    serial_println!("Initializing heap");
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Failed to initialize heap");
    serial_println!("Heap initialized");

    serial_println!("Installing guard pages for kernel stacks");
    {
        serial_println!(
            "GuardedKernelStack size {} guard off {} stack off {}",
            core::mem::size_of::<crate::gdt::GuardedKernelStack>(),
            core::mem::offset_of!(crate::gdt::GuardedKernelStack, guard),
            core::mem::offset_of!(crate::gdt::GuardedKernelStack, stack)
        );
        for i in 0..MAX_CORES {
            let guard = crate::gdt::rsp0_guard_page(i);
            let (bottom, top) = crate::gdt::rsp0_bounds(i);
            serial_println!(
                "RSP0 core {} guard {:#x} bottom {:#x} top {:#x}",
                i,
                guard.as_u64(),
                bottom.as_u64(),
                top.as_u64()
            );
        }
    }
    crate::gdt::install_guard_pages(&mut frame_allocator);
    serial_println!("Guard pages installed");

    unsafe {
        interrupts::init_acpi(
            rsdp_phys_addr,
            hhdm_offset,
            &mut mapper,
            &mut frame_allocator,
        )
    };

    let mp_response = MP_REQUEST.response().expect("Failed to get MP response");
    serial_println!("MP Response received");
    serial_println!("BSP LAPIC ID: {}", mp_response.bsp_lapic_id);

    let cpus = mp_response.cpus();
    let core_count = cpus.len();
    let bsp_lapic_id = mp_response.bsp_lapic_id;

    serial_println!("MP Info:");
    serial_println!("  Total cores: {}", core_count);
    serial_println!("  BSP LAPIC ID: {}", bsp_lapic_id);

    for (i, cpu) in cpus.iter().enumerate() {
        serial_println!(
            "  CPU {}: LAPIC ID={}, Processor ID={}",
            i,
            cpu.lapic_id,
            cpu.processor_id
        );
    }

    unsafe { init_core(0) };

    interrupts::disable_interrupts();

    let (kernel_page_table_frame, _) = x86_64::registers::control::Cr3::read();
    let kernel_page_table_phys = kernel_page_table_frame.start_address();
    let user_memory_manager =
        memory::usermem::UserMemoryManager::new(kernel_page_table_phys, hhdm_offset);
    memory::init_memory_globals(frame_allocator, user_memory_manager);
    serial_println!("Global memory managers initialized");

    #[allow(clippy::needless_range_loop)]
    for i in 1..core_count {
        let core_id = i as u8;
        serial_println_core!("Bootstrapping AP core {}", core_id);
        let ap_bootstrap_fn: MpGotoFunction = ap_core_entry_point;
        cpus[i].bootstrap(ap_bootstrap_fn, 0);
    }

    serial_println_core!("BSP past AP core bootstraps");
    serial_println_core!("Enabling interrupts");
    interrupts::enable_interrupts();
}
