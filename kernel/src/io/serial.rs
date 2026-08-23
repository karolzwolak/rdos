use lazy_static::lazy_static;
use spin::Mutex;
use uart_16550::{Config, Uart16550Tty, backend::PioBackend};

lazy_static! {
    pub static ref SERIAL1: Mutex<Uart16550Tty<PioBackend>> = {
        let serial_port = unsafe { Uart16550Tty::new_port(0x3F8, Config::default()).unwrap() };
        Mutex::new(serial_port)
    };
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    // disable interrupts to avoid SERIAL1 deadlocks
    interrupts::without_interrupts(|| {
        SERIAL1
            .lock()
            .write_fmt(args)
            .expect("Printing to serial failed");
    });
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::io::serial::_print(format_args!($($arg)*))
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}

/// Prints to the host through the serial interface with core ID
#[macro_export]
macro_rules! serial_println_core {
    () => (
        $crate::serial_print!(
            "[Core {}] \n",
            $crate::util::apic_util::get_current_core_id(),
        )
    );
    ($fmt:expr) => (
        $crate::serial_print!(
            concat!("[Core {}] ", $fmt, "\n"),
            $crate::util::apic_util::get_current_core_id(),
        )
    );
    ($fmt:expr, $($arg:tt)*) => (
        $crate::serial_print!(
            concat!("[Core {}] ", $fmt, "\n"),
            $crate::util::apic_util::get_current_core_id(),
            $($arg)*
        )
    );
}
