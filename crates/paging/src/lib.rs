//! Page tables, portable across formats. Each hardware format ([`Aarch64`], [`Sv39`] and x86's)
//! supplies the encode and decode operations the walk needs through [`PageFormat`], and the walk
//! (descend the levels, allocate tables, write a leaf) is written once in [`Mapper`]: §17 (the
//! second architecture: RISC-V, and the page-table format trait). See notes/paging.md.
//!
//! **A block is a leaf the walk meets early.** [`PageSize`] names the 2 MiB and 1 GiB leaves, and
//! [`Mapper::map_span`] picks the largest that fits. `map` refuses to descend through a block,
//! `unmap` refuses to take a 4 KiB bite out of one ([`MapError::InsideBlock`]), and `translate`
//! stops at one and adds the offset. Nothing splits a block.
//!
//! Name: ratified 2026-08-01 (calef, the naming tenet): a standard term that is already right.

#![cfg_attr(not(test), no_std)]

use core::marker::PhantomData;

pub mod aarch64;
pub mod domain;
pub mod sv39;
// The third format (milestone 161). Named `x86_64` for the architecture, like `aarch64` above,
// rather than for the mode; the type inside is `Ia32e`, which is Intel's own name for it.
pub mod x86_64;

pub use aarch64::Aarch64;
pub use domain::{DmaRegion, build_identity_domain};
pub use sv39::{Sv39, Sv39Mae};
pub use x86_64::{AmdVi, Ia32e, Vtd};

// --- Portable access flags ---
//
// `Flags` is a format-neutral set of capabilities, what access a mapping grants, and each
// `PageFormat` translates it to and from its own descriptor bits (`leaf_entry`/`leaf_flags`).

#[cfg(test)]
mod flag_tests;

#[cfg(test)]
mod geometry_tests;

#[cfg(test)]
mod eighth_pass_attacks;

/// Machine-checked proofs of the leaf-size choice [`Mapper::map_span`] makes for every block it
/// writes. **This is the property that makes a block mapping safe**: a block that ran past the span
/// it was asked for would map bytes nobody granted, which on the direct map means a second,
/// writable alias of the kernel image or a cacheable view of a device window. Loopless arithmetic,
/// so it is proved for every address and length rather than tested through a built table (the
/// "prefer refactoring the logic to shrinking the proof" move notes/verification.md records for
/// `domain::grant_pages`). The formats' own block encodings are proved in their modules.
#[cfg(kani)]
mod verification;

mod flags;
mod mapper;
mod page_format;
mod page_size;
mod page_table;
mod tlb_flush;

pub use flags::Flags;
use flags::*;
pub use mapper::{MapError, Mapper, Walk, WalkEnd, WalkStep};
pub use page_format::{Half, PageFormat, is_user_page_va};
pub use page_size::{PAGE_SIZE, PageSize};
pub use page_table::{ENTRIES, PageTable};
pub use tlb_flush::TlbFlush;
