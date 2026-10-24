//! Counts the memory reorderings a guest can see under QEMU's multi-threaded TCG.
//!
//! Four cores run two litmus shapes, two pairs at a time (cores 0 and 1, cores 2 and 3), each
//! pair on its own cache lines: MP ([`message_passing`]) and SB ([`store_buffering`]). Each
//! test prints one `RESULT` line ([`run`]). `LITMUS_SCALE` (build time, default 1) multiplies
//! every iteration count. See notes/benchmarks/host-reordering-under-tcg.md.
//!
//! Name: provisional, minted 2026-10-24 (UTC) by maintainer/measure-arm64-host-reordering.

#![no_std]
#![no_main]

#[cfg(target_arch = "aarch64")]
#[path = "aarch64.rs"]
mod arch;
#[cfg(target_arch = "riscv64")]
#[path = "riscv64.rs"]
mod arch;

mod console;
mod cores;
mod message_passing;
mod run;
mod store_buffering;
