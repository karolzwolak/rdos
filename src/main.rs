#![no_std]
#![no_main]

mod vga;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[unsafe(no_mangle)]
#[allow(clippy::empty_loop)]
pub extern "C" fn _start() -> ! {
    println!("Hello, World!");

    loop {}
}
