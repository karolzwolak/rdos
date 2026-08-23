use limine::mp::MpInfo;
use x86_64::instructions::hlt;

use crate::{
    gdt::init_core_gdt,
    interrupts::{self, init_timer_for_core},
    serial_println,
    util::{
        apic_util::{get_current_core_id, get_lapic_base_addr_phys, init_lapic_for_current_core},
        cpuinfo::init_cpu_info_for_core,
    },
};

/// CR0.MP (bit 1) = 1 - monitor coprocessor
/// CR0.EM (bit 2) = 0 - clear FPU emulation flag
/// CR4.OSFXSR (bit 9) = 1 - OS supports FXSAVE/FXRSTOR
/// CR4.OSXMMEXCPT (bit 10) = 1 - OS handles SSE exceptions
pub unsafe fn enable_sse_for_current_core() {
    unsafe {
        core::arch::asm!(
            "mov rax, cr0",
            "or  rax, 0x2", // set MP
            "and rax, ~0x4",// clear EM
            "mov cr0, rax",
            // set OSFXSR and OSXMMEXCPT
            "mov rax, cr4",
            "or  rax, 0x600",
            "mov cr4, rax",
            out("rax") _,
            options(nostack, nomem),
        );
    }
}

pub unsafe fn init_core(core_id: u8) {
    serial_println!("Core {}: Initializing", core_id);

    unsafe { enable_sse_for_current_core() };

    unsafe { init_lapic_for_current_core(core_id) };
    serial_println!("Core {}: LAPIC initialized", core_id);

    unsafe { init_cpu_info_for_core(core_id) };

    unsafe { init_timer_for_core(core_id) };

    serial_println!("Core {}: Initialized successfully", core_id);
}

pub unsafe extern "C" fn ap_core_entry_point(cpu: &MpInfo) -> ! {
    let proc_id = cpu.processor_id as u8;
    let lapic_id = cpu.lapic_id as u8;
    if proc_id == 0 {
        serial_println!("BSP core entered AP entry point, this should never happen");
        loop {
            hlt();
        }
    }
    serial_println!(
        "AP core entry point reached for APIC ID {} (CPU {})",
        lapic_id,
        proc_id
    );
    debug_assert!(get_current_core_id() == lapic_id);

    let lapic_base_addr = get_lapic_base_addr_phys();
    let (active_pml4_frame, _) = x86_64::registers::control::Cr3::read();
    serial_println!(
        "AP core {}: LAPIC ID {}, LAPIC base physical address: {:#x}, Active PML4 frame: {:#x}",
        proc_id,
        lapic_id,
        lapic_base_addr,
        active_pml4_frame.start_address().as_u64()
    );

    unsafe {
        init_core_gdt(proc_id);
    }
    serial_println!("Core {}: GDT loaded", proc_id);
    interrupts::init_idt();
    serial_println!("Core {}: IDT loaded", proc_id);

    unsafe { init_core(proc_id) };

    serial_println!("Core {}: Enabling interrupts...", proc_id);
    interrupts::enable_interrupts();
    serial_println!("Core {}: Interrupts enabled", proc_id);

    crate::AP_CORES_READY_COUNT.fetch_add(1, core::sync::atomic::Ordering::Release);

    loop {
        hlt()
    }
}
