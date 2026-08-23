use core::arch::asm;
use core::cell::UnsafeCell;
use x86_64::VirtAddr;
use x86_64::registers::segmentation::Segment;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

use crate::memory::paging::{MemoryMapFrameAllocator, PAGE_SIZE};
use crate::{MAX_CORES, serial_println};

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

pub const PER_CORE_STACK_SIZE: usize = 8 * 4096; // 32 KiB

#[repr(C, align(4096))]
pub struct GuardedKernelStack {
    pub guard: [u8; PAGE_SIZE],
    pub stack: [u8; PER_CORE_STACK_SIZE],
}

impl GuardedKernelStack {
    const fn new() -> Self {
        Self {
            guard: [0u8; PAGE_SIZE],
            stack: [0u8; PER_CORE_STACK_SIZE],
        }
    }
}

static mut RSP0_STACKS: [GuardedKernelStack; MAX_CORES as usize] =
    [const { GuardedKernelStack::new() }; MAX_CORES as usize];

static mut IST0_STACKS: [GuardedKernelStack; MAX_CORES as usize] =
    [const { GuardedKernelStack::new() }; MAX_CORES as usize];

pub fn rsp0_stack_top(core_id: u8) -> u64 {
    unsafe {
        let slot = &RSP0_STACKS[core_id as usize];
        slot.stack.as_ptr().add(PER_CORE_STACK_SIZE) as u64
    }
}

pub fn ist0_stack_top(core_id: u8) -> u64 {
    unsafe {
        let slot = &IST0_STACKS[core_id as usize];
        slot.stack.as_ptr().add(PER_CORE_STACK_SIZE) as u64
    }
}

pub fn rsp0_guard_page(core_id: u8) -> VirtAddr {
    unsafe {
        let slot = &RSP0_STACKS[core_id as usize];
        VirtAddr::new(slot.guard.as_ptr() as u64)
    }
}

pub fn ist0_guard_page(core_id: u8) -> VirtAddr {
    unsafe {
        let slot = &IST0_STACKS[core_id as usize];
        VirtAddr::new(slot.guard.as_ptr() as u64)
    }
}

pub fn rsp0_bounds(core_id: u8) -> (VirtAddr, VirtAddr) {
    unsafe {
        let slot = &RSP0_STACKS[core_id as usize];
        let bottom = VirtAddr::new(slot.stack.as_ptr() as u64);
        let top = VirtAddr::new(slot.stack.as_ptr().add(PER_CORE_STACK_SIZE) as u64);
        (bottom, top)
    }
}

pub fn ist0_bounds(core_id: u8) -> (VirtAddr, VirtAddr) {
    unsafe {
        let slot = &IST0_STACKS[core_id as usize];
        let bottom = VirtAddr::new(slot.stack.as_ptr() as u64);
        let top = VirtAddr::new(slot.stack.as_ptr().add(PER_CORE_STACK_SIZE) as u64);
        (bottom, top)
    }
}

pub fn install_guard_pages(frame_allocator: &mut MemoryMapFrameAllocator) {
    for core_id in 0..MAX_CORES as usize {
        unsafe {
            crate::util::stack_guard::ensure_guard_unmapped(
                rsp0_guard_page(core_id as u8),
                frame_allocator,
            );
            crate::util::stack_guard::ensure_guard_unmapped(
                ist0_guard_page(core_id as u8),
                frame_allocator,
            );
        }
    }
}

#[inline(always)]
pub fn assert_rsp_in_bounds(core_id: u8) {
    let rsp: u64;
    unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack)) };
    let (bottom, top) = rsp0_bounds(core_id);
    debug_assert!(
        rsp >= bottom.as_u64() && rsp <= top.as_u64(),
        "RSP {:#x} out of bounds for core {} RSP0 [{:#x}..{:#x}]",
        rsp,
        core_id,
        bottom.as_u64(),
        top.as_u64()
    );
}

static mut PER_CORE_GDT: [Gdt; MAX_CORES as usize] = {
    #[allow(clippy::declare_interior_mutable_const)]
    const EMPTY: Gdt = Gdt::empty();
    [EMPTY; MAX_CORES as usize]
};

struct Gdt {
    table: GlobalDescriptorTable,
    kernel_code_selector: SegmentSelector,
    kernel_data_selector: SegmentSelector,
    user_code_selector: SegmentSelector,
    user_data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

static mut PER_CORE_TSS: [UnsafeCell<TaskStateSegment>; MAX_CORES as usize] =
    [const { UnsafeCell::new(TaskStateSegment::new()) }; MAX_CORES as usize];

impl Gdt {
    const fn empty() -> Self {
        let mut table = GlobalDescriptorTable::new();
        let kernel_code_selector = table.append(Descriptor::kernel_code_segment());
        let kernel_data_selector = table.append(Descriptor::kernel_data_segment());
        let user_data_selector = table.append(Descriptor::user_data_segment());
        let user_code_selector = table.append(Descriptor::user_code_segment());
        let tss_selector = table.append(Descriptor::kernel_code_segment());

        Gdt {
            table,
            kernel_code_selector,
            kernel_data_selector,
            user_code_selector,
            user_data_selector,
            tss_selector,
        }
    }

    fn new(core_id: u8) -> Self {
        let idx = core_id as usize;
        let tss = unsafe { &mut *PER_CORE_TSS[idx].get() };

        let rsp0_top = rsp0_stack_top(core_id);
        tss.privilege_stack_table[0] = VirtAddr::new(rsp0_top);

        let ist0_top = ist0_stack_top(core_id);
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = VirtAddr::new(ist0_top);

        serial_println!(
            "Core {}: TSS RSP0={:#x}  IST0={:#x}",
            core_id,
            rsp0_top,
            ist0_top
        );

        let mut table = GlobalDescriptorTable::new();
        let kernel_code_selector = table.append(Descriptor::kernel_code_segment());
        let kernel_data_selector = table.append(Descriptor::kernel_data_segment());
        let user_data_selector = table.append(Descriptor::user_data_segment());
        let user_code_selector = table.append(Descriptor::user_code_segment());
        let tss_selector = table.append(Descriptor::tss_segment(tss));
        serial_println!("Initialized GDT for core {} with: kernel_code_selector={:#x}, kernel_data_selector={:#x}, 
        user_code_selector={:#x}, user_data_selector={:#x}, tss_selector={:#x}", core_id, kernel_code_selector.0, kernel_data_selector.0, 
        user_code_selector.0, user_data_selector.0, tss_selector.0);

        Gdt {
            table,
            kernel_code_selector,
            kernel_data_selector,
            user_data_selector,
            user_code_selector,
            tss_selector,
        }
    }
}

/// # Safety
///
/// `core_id` must be < `MAX_CORES` and the per-core stack for that core must be valid.
/// Loads the GDT and TSS for the current core.
pub unsafe fn init_core_gdt(core_id: u8) {
    let gdt = Gdt::new(core_id);

    let idx = core_id as usize;
    if idx < MAX_CORES as usize {
        unsafe {
            PER_CORE_GDT[idx] = gdt;
        }
    }

    let gdt_ref = unsafe { &PER_CORE_GDT[idx] };

    gdt_ref.table.load();

    // Reload segments
    unsafe {
        asm!(
            "push {sel}",
            "lea {tmp}, [2f + rip]",
            "push {tmp}",
            "retfq",
            "2:",
            sel = in(reg) gdt_ref.kernel_code_selector.0 as u64,
            tmp = in(reg) 0u64,
            options(nostack)
        );

        x86_64::registers::segmentation::DS::set_reg(gdt_ref.kernel_data_selector);
        x86_64::registers::segmentation::ES::set_reg(gdt_ref.kernel_data_selector);
        x86_64::registers::segmentation::SS::set_reg(gdt_ref.kernel_data_selector);

        x86_64::instructions::tables::load_tss(gdt_ref.tss_selector);
    }
}

pub fn get_kernel_code_selector(core_id: u8) -> SegmentSelector {
    unsafe { PER_CORE_GDT[core_id as usize].kernel_code_selector }
}

pub fn get_kernel_data_selector(core_id: u8) -> SegmentSelector {
    unsafe { PER_CORE_GDT[core_id as usize].kernel_data_selector }
}

pub fn get_user_code_selector(core_id: u8) -> SegmentSelector {
    unsafe { PER_CORE_GDT[core_id as usize].user_code_selector }
}

pub fn get_user_data_selector(core_id: u8) -> SegmentSelector {
    unsafe { PER_CORE_GDT[core_id as usize].user_data_selector }
}

pub fn get_tss_selector(core_id: u8) -> SegmentSelector {
    unsafe { PER_CORE_GDT[core_id as usize].tss_selector }
}
