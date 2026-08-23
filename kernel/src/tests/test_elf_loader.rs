#![no_std]
#![no_main]

extern crate alloc;
extern crate kernel;

use core::panic::PanicInfo;
use kernel::{
    process::elf_loader::{ElfLoadError, ElfLoadInfo},
    testing::{test_case, test_panic_handler},
};

// During static analysis (clippy) the user binary may not be built yet.
// Clippy passes `--cfg clippy` to rustc, so we fall back to empty bytes which
// is enough to satisfy the type system; the real binary is required at runtime.
#[cfg(not(clippy))]
const FIRST_ELF: &[u8] = include_bytes!("../../../target/user/programs/first/first");
#[cfg(clippy)]
const FIRST_ELF: &[u8] = &[];

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

#[unsafe(no_mangle)]
extern "C" fn kmain() -> ! {
    unsafe { kernel::boot_common::bsp_init() };
    kernel::testing::run_all_tests()
}

#[test_case]
fn parse_valid_elf() {
    let info = ElfLoadInfo::from_elf_data(FIRST_ELF).expect("valid ELF should parse");
    assert!(info.entry_point > 0, "entry point should be non-zero");
    assert!(
        !info.segments.is_empty(),
        "should have at least one segment"
    );
    assert!(
        info.max_vaddr > info.min_vaddr,
        "vaddr range should be valid"
    );
}

#[test_case]
fn reject_invalid_magic() {
    let bad = [0u8; 64];
    assert!(matches!(
        ElfLoadInfo::from_elf_data(&bad),
        Err(ElfLoadError::ParseError(_))
    ));
}

#[test_case]
fn reject_empty() {
    assert!(matches!(
        ElfLoadInfo::from_elf_data(&[]),
        Err(ElfLoadError::ParseError(_))
    ));
}
