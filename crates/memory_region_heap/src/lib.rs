//! The memory-region-backed heap, a [`GlobalAlloc`](core::alloc::GlobalAlloc) paid from the
//! process's own memory region on a capability microkernel. A memory region is the budget of pages
//! a process was handed; the kernel's own word for its capability is "untyped". Pages are mapped
//! out of it one at a time and given to the allocator, so the region is what pays for the heap, the
//! heap lives at one virtual range the program picks, and nothing else supplies memory.
//!
//! A program declares it:
//! ```ignore
//! #[global_allocator]
//! static HEAP: memory_region_heap::MemoryRegionHeap = memory_region_heap::MemoryRegionHeap::new();
//! // first thing in _start, with a page-aligned base and a cap of at least one page:
//! HEAP.initialize(SLOT, memory_region_heap::DEFAULT_BASE_VIRTUAL_ADDRESS, 4 << 20).unwrap();
//! ```
//! **How it grows.** The heap maps nothing until the first allocation. A grow maps as many pages
//! again as the heap holds, at least 8 (32 KiB) and never past the cap, so a burst of small
//! allocations costs a logarithmic number of system call batches. [`MemoryRegionHeap::commit_all`] maps
//! the whole cap at once.
//!
//! **When it refuses.** An allocation returns null at the cap, when the region has no page left
//! ([`abi::Error::OutOfMemory`]), or after the region refused for any other reason. The first two
//! leave the heap serving what it holds. The third stops growth for good, because a retry would meet
//! the same refusal and might spend a page each time. [`MemoryRegionHeap::last_refusal`] reports it.
//!
//! **The smallest useful cap is one page** (4,096 bytes), which [`MemoryRegionHeap::initialize`]
//! enforces. `talc`'s first heap takes 1,575 bytes of bookkeeping on a 64-bit target (a test pins
//! it), so one page serves about 2,500 bytes of allocations.
//!
//! **The heap never shrinks.** `talc`'s `truncate` is never called and the memory-region interface
//! has no call that returns one page, so a heap holds its peak until the region's owner destroys it.
//!
//! **Which `talc`.** The manifest asks crates.io for `talc` 5.1.1. A consuming workspace's
//! `[patch.crates-io]` may build a vendored copy that carries upstream pull request 57 (two fit
//! checks that compared addresses where they meant sizes). Without that patch the package lacks the
//! fix, which matters only for a free gap within one chunk of the top of the address space, and a
//! 64-bit user process never maps one.
//!
//! [`MemoryRegionHeap`] takes any [`PageSource`], and the system call ([`MemoryRegionPages`]) is the
//! default, so the growth policy runs on a host.
//!
//! Name: ratified 2026-11-14 (UTC) (block 1100 decision A), for the type it holds.

#![no_std]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

mod growth;
mod heap;
#[cfg(kani)]
mod verification;

pub use heap::{MemoryRegionHeap, MemoryRegionPages, PageSource, Refusal, Short, WiringRefusal};

/// The suggested heap base: 1 GiB, the start of the address-space map's heap band, which sits above
/// any program image under 896 KiB.
/// A convention, not a law; a program with its own layout passes its own base to [`initialize`],
/// and one that does should still keep it inside `address_space_layout::HEAP`.
///
/// [`initialize`]: MemoryRegionHeap::initialize
///
/// Name: ratified 2026-11-14 (UTC) (block 1100 decision A). An address is `_VIRTUAL_ADDRESS` in
/// every location.
pub const DEFAULT_BASE_VIRTUAL_ADDRESS: u64 = address_space_layout::HEAP.start;
