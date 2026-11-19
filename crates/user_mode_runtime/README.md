# user_mode_runtime

[![check](https://github.com/nifeos/user_mode_runtime/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/user_mode_runtime/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/user_mode_runtime/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/user_mode_runtime/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/nifeos/user_mode_runtime#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/user_mode_runtime/blob/main/Cargo.toml)

The runtime a user-mode program of a capability microkernel links. `no_std`, on aarch64 (exception
level 0), riscv64 (U-mode) and `x86_64` (ring 3). It holds the one system call wrapper, the entry
macro, the panic handler macro and typed windows over memory the kernel handed the process.

## What it does

- `invoke` is the one system call wrapper. `send`, `receive`, `call`, `exit` and `require` are
  built on it, and each discharges `invoke`'s safety obligation once so a call site does not.
- `program_entry!` writes the process entry point, `_start`, with the stack alignment each
  architecture expects. `panic_handler!` writes a panic handler that traps.
- `mapped_window::MappedWindow` is one page the process was handed at a fixed virtual address. Every
  access is volatile and bounds-checked, `read::<T>` and `write::<T>` take the sealed
  `WindowInteger` types, and a misaligned address panics instead of causing undefined behavior.
- The opt-in modules `initial_ramdisk`, `virtio`, `entropy`, `socket_frame` and `child_stub` each
  hold one invariant several programs had copied.
- `link.ld` is the link script for the entry registers and link layout (interface 20, `VERSION`
  1). `build.rs` copies it into `OUT_DIR` and adds that directory to the linker search path, so a
  program links with `-C link-arg=-Tlink.ld` and no path.

A wrong argument panics or meets a refusal from the kernel. No safe function reads or writes memory
its type did not promise.

## EXAMPLES

```rust,no_run
use user_mode_runtime::mapped_window::{MappedWindow, PAGE_BYTES};

// SAFETY: the program's wiring maps one read/write page at this address before it runs.
const WINDOW: MappedWindow = unsafe { MappedWindow::new(0x0090_0000, PAGE_BYTES) };

WINDOW.write::<u16>(0x080, 7);
assert_eq!(WINDOW.read::<u16>(0x080), 7);
```

A program is built for one of the three targets and takes `link.ld` from this crate:

```rust,ignore
// build.rs
fn main() {
    println!("cargo::rustc-link-arg=-Tlink.ld");
}
```

```rust,ignore
#![no_std]
#![no_main]

user_mode_runtime::panic_handler!();
user_mode_runtime::program_entry!(start);

fn start(_argument0: u64, _argument1: u64, _argument2: u64) -> ! {
    user_mode_runtime::exit()
}
```

## What is proven

No model checker covers this crate, and none is claimed. Host tests cover the logic that runs
anywhere: the window's bounds and alignment checks and its copies, the frame accessors over a heap
page, the reply decoder, the three hand-assembled child stubs (each instruction decoded back to the
constant it carries) and the link script's base address. The system call arms run only in a
user-mode process under a kernel, so they are checked by building them for all three targets.

## Building and testing

```sh
cargo test                                      # host tests: bounds and alignment checks, frames, stubs
cargo build --lib --target x86_64-unknown-none
cargo build --lib --target aarch64-unknown-none-softfloat
cargo build --lib --target riscv64imac-unknown-none-elf
```


## BUGS

- Nothing here runs a system call on the host; the arms are checked by building them for all three
  targets and by the kernel's own boot tests.
- A program with its own panic handler does not invoke `panic_handler!`; the macro ignores the panic
  information because a program without a console cannot print it.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
