#![no_std]
#![no_main]

mod vga;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

static HELLO: &str = "Hello, world!";

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut writer = vga::Writer::new(vga::Color::White, vga::Color::Black);
    writer.write_string(HELLO);

    loop {}
}
