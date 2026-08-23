#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    unsafe {
        let top = kernel::gdt::rsp0_stack_top(0);

        core::arch::asm!(
            "mov rsp, {top}",
            "call {bsp_init}",
            "call {main_fn}",
            top = in(reg) top,
            bsp_init = sym kernel::boot_common::bsp_init,
            main_fn = sym crate::main,
            options(noreturn)
        );
    }
}
