//! The memory-region accounting as pure logic (DECISIONS §16 (object revocation)): the arithmetic
//! the kernel's region allocator (`kernel/src/memory_region.rs`) calls, so Kani proves the code the
//! kernel runs. [`split_new_watermark`] carves a child off a parent's budget. [`destroy_outcome`]
//! decides whether a region refuses, frees its run to the allocator (a root) or returns it to its
//! parent (a child, never to the allocator), which is what rules out a double free. Everything is
//! in page units, so the crate is `FRAME_SIZE`- and address-agnostic; the kernel does the byte
//! arithmetic and the I/O. See notes/memory-regions.md, "The `memory_regions` crate".
//!
//! The model, in one screen. A **region** is a run of pages with a **watermark**: carves (a
//! retype into an object, a split into a child) only move the watermark up, so spent pages are
//! never handed out twice. A **pin** is set while an object lives in a page of the region, and a
//! pinned region cannot be reclaimed. A **claim** ([`DestroyClaim`]) is the only right to reclaim
//! a region, and it is single-winner because it removes the region's generational slot. A child's
//! pages go back to its parent's budget through the **return**, and never to the allocator: only
//! a root's run is freed to the allocator. **Ceilings** bound how many objects of each kind, and
//! how many child regions, live in a region's whole subtree at once. [`RegionTable`] holds all of
//! it and has the worked example.
//!
//! //! Name: ratified 2026-08-23 (calef, a kernel-dependency crate naming review). Renamed from
//! `regions`, which collides with `crates/compositor`'s damaged-screen "region". DECISIONS §113
//! (eleven kernel object and identifier names move) found that when naming `MemoryRegion`, the
//! kernel object whose accounting this crate is.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

mod table;

pub use table::{
    CeilingReached, Ceilings, Counted, DestroyClaim, NO_PARENT, ObjectKind, PageUse, RegionTable,
};

mod destroy;
mod watermark;

pub use destroy::{DestroyOutcome, destroy_outcome};
pub use watermark::{coalesced_watermark, retype_new_watermark, retype_pages, split_new_watermark};

#[cfg(kani)]
mod proofs;

#[cfg(test)]
mod tests;
