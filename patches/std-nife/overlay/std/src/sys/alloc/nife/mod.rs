//! The System allocator for nife: the untyped-backed heap, inside std.
//!
//! The algorithm is `talc`'s core, generated into `talc/` next door by `cargo xtask std-src` from
//! the version `Cargo.lock` pins (milestone 784 (reuse what the tree wrote), item d), so std runs
//! the same allocator `user_mode_runtime` takes from crates.io. This file is the same policy layer as
//! `user_mode_runtime::heap::MemoryRegionHeap`, re-stated for std because std cannot depend on the
//! out-of-tree crate: a spinlock, lazy wiring to the untyped in slot 0, and grow-on-demand by
//! `memory_region::MAP` at the top of the committed range.
//!
//! Lazy, because std has no moment where a program says "init the heap": the first allocation
//! (often std's own rt setup) simply finds committed == 0 and grows. A program whose slot 0 is
//! not an untyped gets a failed allocation, which std turns into the OOM abort: honest, if
//! blunt.
//!
//! **Each thread has a cache in front of the lock** (milestone 561 (a per-CPU allocator is what
//! the current-CPU page was for)): `crates/allocation_cache`, generated into `allocation_cache.rs`
//! next door, kept in the thread's own block (`sys/thread_local/key/nife.rs`). A small request is
//! served from it without the lock; a miss refills, and a full class spills, eight blocks under one
//! acquisition. A thread's end closes its cache and gives every block back
//! ([`spill_thread_cache`]). Before the main thread's block exists, and in a program whose thread
//! pointer could not be set, every request takes the lock, as before. Why per thread and not per
//! CPU, and what was measured, is `notes/std/allocation.md`.

use crate::alloc::Layout;
use crate::ptr::NonNull;
use crate::sys::pal::nife::{abi, rt};
use crate::sys::thread_local::key::nife as tls;

// Generated whole from the crate; its counters and bound are the host tests', hence the allow.
#[allow(dead_code)]
pub(crate) mod allocation_cache;
// Upstream code, written to talc's edition and lints rather than std's: std denies an unsafe
// operation outside an `unsafe` block, and talc's `unsafe fn` bodies are written without them.
#[allow(unsafe_op_in_unsafe_fn, dead_code)]
mod talc;

use allocation_cache::{Cache, class, class_layout, heap_layout};

mod heap;
mod thread_cache;
mod entry;

use heap::*;
pub(crate) use thread_cache::spill_thread_cache;
use thread_cache::*;
pub use entry::{alloc, alloc_zeroed, dealloc, realloc};
use entry::*;

