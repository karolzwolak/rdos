RDOS (Really dumb OS)

[bootimage]: https://crates.io/crates/bootimage

# Dependencies

- nightly rust toolchain
- qemu
- [`bootimage`][bootimage] tool

# Running

1. Using [`bootimage runner`][bootimage]:

- build and run the kernel: `cargo run`

2. Running qemu explicitly:

- build the kernel: `cargo build`
- run qemu: `qemu-system-x86_64 -drive format=raw,file=target/x86_64-rdos/debug/bootimage-rdos.bin`
