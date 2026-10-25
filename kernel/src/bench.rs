//! Microbenchmarks over the paths a microkernel lives on.
//!
//! Compiled in only by `--features bench` (`script/bench`): the boot diverges into [`run`] before
//! the milestone tour, runs each benchmark in a fixed order, prints one
//! `bench: <name> <counter_ticks> <iters>` line each after the counter frequency, prints
//! `bench: done` and halts. `xtask bench` watches for that marker and ends QEMU; nothing semihosts.
//! See notes/benchmarks.md for the two instruments and the numbers, and
//! notes/benchmarks/kernel-bench-harness.md for how each row is built.
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::cap::{Rights, address_space_cap, page_frame_cap, rendezvous_cap};
use crate::{println, sched};

// The index of children (§282 (when a file is split, the parent becomes an index)). Each takes
// `use super::*` and is imported back whole, so siblings reach each other. E1 and E4 are built on
// aarch64 and riscv64 only.
mod timing;
use timing::*;
mod sequence;
pub use sequence::*;
mod switch;
use switch::*;
mod ipc;
use ipc::*;
mod spawn_map;
use spawn_map::*;
mod cycle_probe;
use cycle_probe::*;
mod el0;
use el0::*;
mod compute;
use compute::*;
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
mod thread_scaling;
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
use thread_scaling::*;
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
mod app_displacement;
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
use app_displacement::*;
mod filesystem;
use filesystem::*;
mod smp_throughput;
use smp_throughput::*;
mod keyboard;
use keyboard::*;
