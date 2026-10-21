//! **`user_mode_runtime`**: the tiny user-mode runtime shared by nife userspace programs, on all
//! three architectures. "User mode" is `EL0` on aarch64, `U-mode` on riscv64 and `ring 3` on
//! `x86_64`.
//!
//! One syscall wrapper ([`invoke`]) and what every program builds on it: [`send`], [`receive`],
//! [`call`] and [`exit`], plus the thin wrappers in `objects.rs` and `inter_process_communication.rs`, each discharging
//! [`invoke`]'s own safety obligation once instead of at every call site. [`require`] is the one
//! way a program says no: a failed check dies where the mistake was. Opt-in modules carry what
//! only some programs need: [`mapped_window`], [`initial_ramdisk`] and [`virtio`]. Each is the §94 (what may live in a
//! userspace library, and what must be per-binary) shape: one invariant that had been copied
//! verbatim into several programs.
//!
//! A `#[panic_handler]` cannot live here: exactly one may exist per linked binary, and `hello` wants
//! its own. The trap underneath it can, so [`trap`] is here and [`panic_handler!`] expands to the
//! handler in each binary. Device helpers stay in the drivers that own them.
//!
//! Nothing here runs on the host: every function traps to the kernel, and `script/test`'s host pass
//! excludes this crate and its dependents (derived and checked by `script/lint`). The examples are
//! `no_run`, type-checked only; the wire contracts layered over this crate carry runnable ones.
//!
//! # Three ABIs, one surface
//!
//! aarch64 uses `svc #0` with the number in `x8` and arguments in `x0..x5`; RISC-V uses `ecall`
//! with `a7` and `a0..a5`; `x86_64` uses `syscall` with `rax` and `rdi`, `rsi`, `rdx`, `r10`, `r8`,
//! `r9` ([§124 (ratify the x86_64 syscall ABI)](../../../design/decisions/0124-x86-64-syscall-abi.md)).
//! All return in the first argument register, the kernel reconciles them in `TrapFrame`, and every
//! signature and `abi` constant is identical. `x86_64` costs more in three places: `syscall`
//! clobbers `rcx` and `r11` at every site, `rdtsc` answers in two halves ([`now`]), and there is no
//! architected counter frequency, which is why [`cntfrq`] has a `BUGS` section.
//!
//! Name: ratified 2026-09-13 (calef), replacing `user_rt`: `rt` is an abbreviation
//! that needs a decoder, and in this tree `user` alone means a person, so `user_mode_` names the
//! privilege level. Refused `user_runtime` (ambiguous between a person and a privilege level) and
//! `el0_runtime` (correct on one architecture and wrong on the other two).

#![no_std]

pub mod child_stub;
pub mod entropy;
pub mod initial_ramdisk;
pub mod mapped_window;
pub mod socket_frame;
pub mod virtio;

mod clock;
mod devices;
mod inter_process_communication;
mod objects;
mod process;
mod syscall;

pub use abi::cycle_counter::CycleMeaning;
pub use clock::*;
pub use devices::*;
pub use inter_process_communication::*;
pub use objects::*;
pub use process::*;
pub use syscall::*;
