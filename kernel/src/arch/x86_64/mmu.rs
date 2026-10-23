//! **The MMU, `x86_64`.** `crates/paging` owns the level walk and `paging::x86_64::Ia32e` the
//! entry format; this module names `CR3`, the direct map, and where this machine's devices are.
//!
//! Two kernel bases, unlike aarch64 and RISC-V: `code-model: kernel` pins the image at
//! [`KERNEL_VA_BASE`] (PML4[511]), so the direct map of physical memory is a separate PML4 entry
//! at [`DIRECT_MAP_BASE`] (PML4[273]), as on Linux. `boot.s` installs the direct map's entry
//! before the first Rust instruction, so [`phys_to_virt`] means one thing for the machine's whole
//! life, and [`init`] drops the boot identity map. See notes/x86-port/kernel-page-tables.md,
//! which holds the BUGS.

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering};

use paging::x86_64::Ia32e;
use paging::{Flags, Half, MapError, Mapper, PAGE_SIZE, PageSize, PageTable};

// The `x86_64` modules the children name as `super::isa` and the like. Imported here so that a
// path written when this code lived in `mmu.rs` still means what it meant: the children's `super`
// is this module, and these are what it would have reached.
use super::{ap_boot, current_sp, discovery, instructions, irq, isa, machine, segments};
use crate::memory;

/// This architecture's page-table format. Portable code names it as `arch::mmu::Format`.
pub type Format = Ia32e;

// **Every type and function of `mmu` lives in a child named for it**, and this file is the index
// (§282 (when a file is split, the parent becomes an index)). `Format` stays: the arch contract's
// one alias, a single line. Each child takes `use super::*` and is re-exported whole, so every
// `arch::mmu::` path a caller names still resolves, and `map_tests` keeps its path. Cut by lane
// `maintainer/last-three-over-2000` (2026-10-12 UTC), which brought this file under §266 (a Rust
// source file stays under 2,000 lines).
mod layout;
pub use layout::*;
mod kernel_map;
pub use kernel_map::*;
mod shootdown;
pub use shootdown::*;
mod claims;
use claims::*;
mod memory_mapped_io_window;
pub use memory_mapped_io_window::*;
mod fine_map;
use fine_map::*;
mod image_bounds;
pub use image_bounds::*;
mod user_space;
pub use user_space::*;
mod device_windows;
pub use device_windows::*;

#[cfg(test)]
mod map_tests;
