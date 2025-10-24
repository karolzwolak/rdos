RDOS (Really dumb OS)

[bootimage]: https://crates.io/crates/bootimage

# Build dependencies

You have to install the following dependencies yourself:

- qemu
- llvm tools
- [`bootimage`][bootimage] tool: `cargo install bootimage`

If you are on Debian system, install the required packages with:

```sh
sudo apt-get install -y qemu-system llvm-14-tools
```

# Running

1. Using [`bootimage runner`][bootimage]:

- build and run the kernel: `cargo run`

2. Running qemu explicitly:

- build the kernel: `cargo build`
- run qemu: `qemu-system-x86_64 -drive format=raw,file=target/x86_64-rdos/debug/bootimage-rdos.bin`
