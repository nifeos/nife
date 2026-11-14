//! **`allocation_cache`**: a bounded stash of freed blocks in front of a shared heap, so most
//! allocations never take the heap's lock. The algorithm only: it never locks, maps or reads a CPU
//! id, and its owner decides who touches it.
//!
//! A class is a block size on a 16-byte grid ([`GRID`]) up to [`LARGEST`], under one heap layout,
//! [`class_layout`]. A [`Cache`] holds a stack of freed blocks per class. Blocks of one class are
//! fungible: the heap sees the same layout at both ends, so any cached block can answer any
//! request of its class.
//!
//! # What the owner promises
//!
//! 1. **One owner at a time.** A [`Cache`] is neither `Send` nor `Sync`; its owner keeps it where
//!    one thread reaches it.
//! 2. **[`refill`](Cache::refill) and [`spill`](Cache::spill) run under whatever excludes the other
//!    users of the heap**, once per batch, so the heap's lock is taken once for several blocks.
//! 3. **Every block a [`spill`](Cache::spill) or [`drain`](Cache::drain) hands back goes to the
//!    heap under [`class_layout`]`(class)`**, and every block the heap hands to
//!    [`refill`](Cache::refill) was allocated under it. A block that only meets
//!    [`put`](Cache::put)'s contract (a slice of a stack array, as the example uses) must never be
//!    handed on to a heap.
//!
//! The heap also has to see [`heap_layout`] on the path that never touches a cache. A block may be
//! allocated before its owner has a cache and freed into one afterward, and the cache then treats
//! it as [`class_bytes`] long; a heap that rounds or tags its own way would have given a smaller
//! block to the shorter request.
//!
//! An all-zero [`Cache`] is empty, so one may live in memory a runtime zeroes.
//!
//! # Examples
//!
//! ```
//! use allocation_cache::{Cache, class};
//! use core::ptr::NonNull;
//!
//! let mut cache = Cache::new();
//! let c = class(48, 8).unwrap(); // a `[u8; 48]`
//! assert_eq!((class(16, 64), class(257, 8)), (None, None)); // over-aligned, too big
//! let mut block = [0u128; 3]; // 48 grid-aligned bytes, as the heap would hand out
//! let p = NonNull::from(&mut block).cast::<u8>();
//! // SAFETY: `p` is 48 writable, grid-aligned bytes nobody else uses while the cache holds them.
//! assert!(unsafe { cache.put(c, p) }.is_ok());
//! assert_eq!((cache.take(c), cache.take(c)), (Some(p), None)); // then the caller goes to the heap
//! ```
//!
//! # BUGS
//!
//! - A cached block cannot coalesce until it is spilled: up to [`MOST_CACHED_BYTES`] (34,816) per
//!   cache. The owner spills at its end with [`drain`](Cache::drain); a [`Cache`] dropped while it
//!   holds blocks leaks them, because it cannot free.
//! - [`DEPTH`] (glibc's `tcache` count) and [`LARGEST`] are chosen, not measured per workload, and
//!   they do not move: they are constants, not parameters of [`Cache`].
//! - A block freed by an owner other than the one that allocated it lands in the freeing owner's
//!   cache, bounded by [`DEPTH`] and spilled at the bound.
//! - [`class`] and [`class_bytes`] take bare `usize`. An index that did not come from [`class`]
//!   panics in [`Cache`]'s methods, or overflows in [`class_bytes`].
//!
//! Name: provisional, minted 2026-10-10 (UTC) by the lane for milestone 561 (a per-CPU
//! allocator is what the current-CPU page was for), for what it holds rather than its owner.
//! calef has not ruled on it.

#![no_std]

use core::alloc::Layout;
use core::ptr::NonNull;

/// The size step between classes, and the most alignment a cached request may ask for.
pub const GRID: usize = 16;

/// The largest effective size a cache holds.
pub const LARGEST: usize = 256;

/// How many size classes there are: one per grid step up to [`LARGEST`].
pub const CLASSES: usize = LARGEST / GRID;

/// How many blocks of one class a cache holds before a free spills.
pub const DEPTH: usize = 16;

/// How many blocks a refill takes from the heap, and a spill gives back, under one acquisition of
/// its lock: half the depth, so a thread that only allocates or only frees takes the lock once per
/// eight blocks, and one that alternates at the boundary does not take it on every call.
pub const BATCH: usize = DEPTH / 2;

/// The most memory one full cache keeps out of the heap.
pub const MOST_CACHED_BYTES: usize = DEPTH * GRID * CLASSES * (CLASSES + 1) / 2;

/// The class of a request of `size` bytes aligned to `align`, or `None` when it is not cached.
pub const fn class(size: usize, align: usize) -> Option<usize> {
    if align > GRID {
        return None;
    }
    // Compare before rounding: `next_multiple_of` wraps near `usize::MAX`, which would pass the bound.
    if size > LARGEST {
        return None;
    }
    let effective = if size == 0 {
        GRID
    } else {
        size.next_multiple_of(GRID)
    };
    Some(effective / GRID - 1)
}

/// The effective size of a class's blocks.
pub const fn class_bytes(class: usize) -> usize {
    (class + 1) * GRID
}

/// The layout every block of `class` is allocated from, and freed to, the heap under.
///
/// Name: provisional, minted 2026-10-21 (UTC) by lane/784-talc, the name std's thread cache gave it.
pub const fn class_layout(class: usize) -> Layout {
    // SAFETY: a nonzero multiple of `GRID`, a power of two, aligned to `GRID`.
    unsafe { Layout::from_size_align_unchecked(class_bytes(class), GRID) }
}

/// **The layout to give the heap for `layout`**, at both ends of an allocation: its class's when it
/// has one, whether or not a cache is there to serve it, and otherwise `layout` itself, made at
/// least one byte, since a heap may refuse an empty layout.
///
/// A block can be allocated before its thread has a cache and freed into one afterward, and the
/// cache then treats it as [`class_bytes`] long. A heap that rounds sizes its own way (`talc` adds a
/// tag and a 32-byte chunk) would have given a 17-byte request fewer bytes than its class's 32, so
/// the uncached path has to ask for the class's layout too.
///
/// Name: provisional, minted 2026-10-21 (UTC) by lane/784-talc.
pub const fn heap_layout(layout: Layout) -> Layout {
    match class(layout.size(), layout.align()) {
        Some(c) => class_layout(c),
        // SAFETY: `layout`'s own alignment, and a size of at least one byte that cannot overflow
        // it (`layout.size()` was valid and one is the least valid size).
        None => unsafe {
            Layout::from_size_align_unchecked(
                if layout.size() == 0 { 1 } else { layout.size() },
                layout.align(),
            )
        },
    }
}

/// A cached block's first word, linking it to the next block of its class.
struct Free {
    next: *mut Free,
}

const _: () = assert!(core::mem::size_of::<Free>() <= GRID);
// `put` casts a `GRID`-aligned block to a `Free`, so the grid must satisfy its alignment.
const _: () = assert!(core::mem::align_of::<Free>() <= GRID);

/// **One owner's stash**: a stack of freed blocks per class, each at most [`DEPTH`] deep.
///
/// The owner's rules are in the crate documentation: one owner at a time, batches under the
/// heap's exclusion, and every block handed back goes to the heap under [`class_layout`]. An
/// all-zero `Cache` is empty, so it can live in memory a runtime zeroes.
///
/// `const` so it can sit inside a statically made structure (`std`'s per-thread block). It holds raw
/// pointers into memory it does not own, so it is neither `Send` nor `Sync`: its owner keeps it
/// where one thread reaches it, and `std` keeps it in that thread's own block.
pub struct Cache {
    heads: [*mut Free; CLASSES],
    counts: [u8; CLASSES],
}

/// Prints the count per class and never a pointer, so a cache sits in a `#[derive(Debug)]` struct.
impl core::fmt::Debug for Cache {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Cache")
            .field("counts", &self.counts)
            .field("cached_bytes", &self.cached_bytes())
            .finish()
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    /// An empty cache.
    pub const fn new() -> Self {
        Self {
            heads: [core::ptr::null_mut(); CLASSES],
            counts: [0; CLASSES],
        }
    }

    /// How many blocks of `class` the cache holds.
    pub fn count(&self, class: usize) -> usize {
        usize::from(self.counts[class])
    }

    /// The bytes the cache is keeping out of the heap.
    pub fn cached_bytes(&self) -> usize {
        (0..CLASSES).map(|c| self.count(c) * class_bytes(c)).sum()
    }

    /// A cached block of `class`, or `None` when there is none and the caller must go to the heap.
    pub fn take(&mut self, class: usize) -> Option<NonNull<u8>> {
        let head = NonNull::new(self.heads[class])?;
        // SAFETY: `head` is a block this cache holds (`put`'s contract), whose first word `put`
        // wrote as a `Free`.
        self.heads[class] = unsafe { (*head.as_ptr()).next };
        self.counts[class] -= 1;
        Some(head.cast())
    }

    /// Keep `block` as a free block of `class`. Hands it back as `Err` when the class is full, and
    /// the caller should [`spill`](Self::spill) and try again, or free it to the heap.
    ///
    /// # Safety
    ///
    /// `block` is [`class_bytes`]`(class)` bytes, writable, aligned to [`GRID`], and from now on
    /// used by nothing but this cache, until [`take`](Self::take) or a spill hands it back. A block
    /// the owner means to [`spill`](Self::spill) or [`drain`](Self::drain) into a heap also has to
    /// be one that heap accepts under [`class_layout`]`(class)`; the crate documentation says so.
    pub unsafe fn put(&mut self, class: usize, block: NonNull<u8>) -> Result<(), NonNull<u8>> {
        if self.count(class) >= DEPTH {
            return Err(block);
        }
        // The caller promises `GRID` alignment, which is at least a `Free`'s.
        #[allow(clippy::cast_ptr_alignment)]
        let node = block.as_ptr().cast::<Free>();
        // SAFETY: the caller's contract: the block is ours, writable and big and aligned enough.
        unsafe {
            node.write(Free {
                next: self.heads[class],
            });
        };
        self.heads[class] = node;
        self.counts[class] += 1;
        Ok(())
    }

    /// Hand up to [`BATCH`] blocks of `class` to `give`, which returns them to the heap. It hands
    /// back only blocks that [`put`](Self::put) or [`refill`](Self::refill) took. Call it
    /// holding the heap's lock, once, rather than locking per block.
    pub fn spill(&mut self, class: usize, mut give: impl FnMut(NonNull<u8>)) {
        for _ in 0..BATCH {
            match self.take(class) {
                Some(b) => give(b),
                None => break,
            }
        }
    }

    /// Fill `class` with up to [`BATCH`] blocks from `get`, which allocates them from the heap and
    /// answers `None` when it has none. Call it holding the heap's lock. Returns how many it took.
    ///
    /// # Safety
    ///
    /// Every block `get` answers meets [`put`](Self::put)'s contract for `class`.
    pub unsafe fn refill(
        &mut self,
        class: usize,
        mut get: impl FnMut() -> Option<NonNull<u8>>,
    ) -> usize {
        let mut taken = 0;
        while taken < BATCH && self.count(class) < DEPTH {
            let Some(b) = get() else { break };
            // SAFETY: the caller's contract for what `get` answers; the class has room (checked).
            let kept = unsafe { self.put(class, b) };
            debug_assert!(kept.is_ok());
            taken += 1;
        }
        taken
    }

    /// Hand every block to `give` with its class, leaving the cache empty: an owner's end. It
    /// hands back only blocks that [`put`](Self::put) or [`refill`](Self::refill) took.
    pub fn drain(&mut self, mut give: impl FnMut(usize, NonNull<u8>)) {
        for c in 0..CLASSES {
            while let Some(b) = self.take(c) {
                give(c, b);
            }
        }
    }
}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests {
    extern crate std;

    use core::alloc::Layout;
    use std::collections::HashSet;
    use std::vec::Vec;

    use super::*;

    #[test]
    fn the_bound_is_what_a_full_cache_holds() {
        let mut arena: Vec<u128> = std::vec![0; MOST_CACHED_BYTES / GRID];
        let base = arena.as_mut_ptr().cast::<u8>();
        let mut cache = Cache::new();
        let mut off = 0;
        for c in 0..CLASSES {
            for _ in 0..DEPTH {
                // SAFETY: disjoint, grid-aligned slices of `arena`, which outlives `cache`.
                let b = unsafe { NonNull::new_unchecked(base.add(off)) };
                // SAFETY: as above; each slice is handed to the cache once.
                assert!(unsafe { cache.put(c, b) }.is_ok());
                off += class_bytes(c);
            }
            let spare = NonNull::new(base).unwrap();
            // SAFETY: refused before the block is touched, so aliasing it is harmless.
            assert_eq!(unsafe { cache.put(c, spare) }, Err(spare));
        }
        assert_eq!(off, MOST_CACHED_BYTES);
        assert_eq!(cache.cached_bytes(), MOST_CACHED_BYTES);
        let mut n = 0;
        cache.drain(|_, _| n += 1);
        assert_eq!(n, CLASSES * DEPTH);
        assert_eq!(cache.cached_bytes(), 0);
    }

    /// A request with no class reaches the heap exactly as asked, because the heap frees it under
    /// the same layout and only a class's blocks are fungible. The one change is an empty layout,
    /// over-aligned past the grid, which becomes one byte, since talc refuses an empty one.
    #[test]
    fn a_request_without_a_class_reaches_the_heap_as_asked_unless_empty() {
        for (size, align) in [(LARGEST + 1, 8), (4096, 4096), (1, 2 * GRID)] {
            let l = Layout::from_size_align(size, align).unwrap();
            assert_eq!(heap_layout(l), l);
        }
        let empty = Layout::from_size_align(0, 2 * GRID).unwrap();
        assert_eq!(
            heap_layout(empty),
            Layout::from_size_align(1, 2 * GRID).unwrap()
        );
    }

    /// The classes stop exactly at [`LARGEST`]: its class is the last one, so no class is sized
    /// past what [`class`] can ever answer.
    #[test]
    fn the_last_class_is_the_largest_size() {
        assert_eq!(class(LARGEST, 1), Some(CLASSES - 1));
        assert_eq!(class_bytes(CLASSES - 1), LARGEST);
    }

    /// An alignment equal to the grid is inside [`class`], and the first refusal is one past it:
    /// both ends of the comparison are the constants, so the boundary is asserted, not hoped for.
    #[test]
    fn an_alignment_equal_to_the_grid_is_a_class_and_one_past_is_not() {
        assert_eq!(class(1, GRID), Some(0));
        assert_eq!(class(LARGEST, GRID), Some(CLASSES - 1));
        assert_eq!(class(1, GRID + 1), None);
    }

    /// A cache prints its counts, not its pointers.
    #[test]
    fn a_cache_prints_counts_and_not_pointers() {
        let mut block = [0u128; 1];
        let mut cache = Cache::new();
        // SAFETY: 16 writable, grid-aligned bytes nobody else uses while the cache holds them.
        assert!(unsafe { cache.put(0, NonNull::from(&mut block).cast()) }.is_ok());
        let text = std::format!("{cache:?}");
        assert!(text.contains("cached_bytes: 16"), "{text}");
        assert!(!text.contains("0x"), "{text}");
    }

    /// A size no `Layout` reaches is refused, not wrapped: rounding `usize::MAX` up to the grid
    /// wraps to zero, which would pass the bound.
    #[test]
    fn a_size_near_the_top_of_usize_has_no_class() {
        assert_eq!(class(usize::MAX, 8), None);
        assert_eq!(class(usize::MAX - GRID + 1, 8), None);
        assert_eq!(class(LARGEST + 1, 8), None);
        assert_eq!(class(LARGEST, 8), Some(CLASSES - 1));
        assert_eq!(class(0, 1), Some(0));
    }

    /// An all-zero cache is empty, which the std overlay relies on when it zeroes a thread block.
    #[test]
    fn an_all_zero_cache_is_empty() {
        // SAFETY: `Cache` holds null raw pointers and byte counters, valid when all-zero.
        let zeroed: Cache = unsafe { core::mem::zeroed() };
        assert_eq!(zeroed.cached_bytes(), 0);
        for c in 0..CLASSES {
            assert_eq!(zeroed.count(c), 0);
        }
        let mut zeroed = zeroed;
        assert_eq!(zeroed.take(0), None);
    }

    /// A spill gives back half a full class and a refill into an empty one takes half a class, so
    /// a thread that only frees, or only allocates, takes the heap's lock once per eight blocks.
    #[test]
    fn a_spill_and_a_refill_each_move_half_a_class() {
        let mut arena: Vec<u128> = std::vec![0; DEPTH];
        let base = arena.as_mut_ptr().cast::<u8>();
        let mut cache = Cache::new();
        for i in 0..DEPTH {
            // SAFETY: disjoint 16-byte, grid-aligned slices of `arena`, which outlives `cache`.
            let b = unsafe { NonNull::new_unchecked(base.add(i * GRID)) };
            // SAFETY: as above; each slice is handed to the cache once.
            assert!(unsafe { cache.put(0, b) }.is_ok());
        }
        let mut given = 0;
        cache.spill(0, |_| given += 1);
        assert_eq!(given, DEPTH / 2);
        assert_eq!(cache.count(0), DEPTH / 2);

        let mut empty = Cache::new();
        let mut next = 0;
        // SAFETY: the same disjoint slices, now owned by `empty` alone (`cache` is not used again).
        let taken = unsafe {
            empty.refill(0, || {
                next += 1;
                NonNull::new(base.wrapping_add((next - 1) * GRID))
            })
        };
        assert_eq!(taken, DEPTH / 2);
        assert_eq!(empty.count(0), DEPTH / 2);
    }

    /// The test that matters: a cache in front of the heap std really uses, `talc`, driven by a
    /// random mix of allocations and frees with refills and spills, never hands out a block twice,
    /// never hands one out shorter than its class, and gives every byte back, coalesced, at the end.
    ///
    /// One allocation in four goes to the heap directly under [`heap_layout`], as a thread without
    /// a cache does, and every free goes through the cache. That is the case a heap that rounds its
    /// own way breaks: a block allocated under its own layout, then cached as its class's size.
    /// The heap is claimed in two runs, as the runtime grows it, so `extend` is walked too.
    #[test]
    fn a_cache_in_front_of_the_heap_never_hands_out_a_block_twice() {
        use talc::base::Talc;
        use talc::base::binning::DefaultBinning;
        use talc::source::Manual;

        const ARENA: usize = 64 * 1024;
        let mut arena: Vec<u128> = std::vec![0; ARENA / GRID];
        let base = arena.as_mut_ptr().cast::<u8>();
        let mut heap = Talc::<Manual, DefaultBinning>::new(Manual);
        // SAFETY: `arena` is ours and outlives `heap`, and the second run continues the first.
        unsafe {
            let end = heap.claim(base, ARENA / 2).unwrap();
            heap.extend(end, base.wrapping_add(ARENA));
        }
        let mut cache = Cache::new();
        let mut live: Vec<(NonNull<u8>, Layout)> = Vec::new();
        let mut seen = HashSet::new();
        let mut x: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        // Miri interprets about a thousand times slower; a tenth of the walk still crosses every
        // refill and spill boundary many times over.
        let steps = if cfg!(miri) { 2_000 } else { 20_000 };
        for _ in 0..steps {
            let r = next();
            if r % 3 != 0 || live.is_empty() {
                let size = (r >> 8) as usize % (LARGEST + 1);
                let l = Layout::from_size_align(size, 8).unwrap();
                let c = class(size, 8).unwrap();
                let p = if r % 4 == 1 {
                    // SAFETY: `heap_layout` is never empty.
                    match unsafe { heap.try_allocate(heap_layout(l)) } {
                        Some(p) => p,
                        None => continue, // the arena is full; fine
                    }
                } else {
                    match cache.take(c) {
                        Some(p) => p,
                        None => {
                            // SAFETY: each block is the heap's, allocated under its class's layout.
                            unsafe {
                                cache.refill(c, || heap.try_allocate(class_layout(c)));
                            }
                            match cache.take(c) {
                                Some(p) => p,
                                None => continue,
                            }
                        }
                    }
                };
                assert!(seen.insert(p), "a block was handed out while live");
                // SAFETY: a live block of `class_bytes(c)` bytes, which is what the cache will
                // hand it out as once it is freed; writing all of it would overwrite the heap's
                // tag above a block that was short, which the heap's debug checks then catch.
                unsafe { core::ptr::write_bytes(p.as_ptr(), 0xa5, class_bytes(c)) };
                live.push((p, l));
            } else {
                let (p, l) = live.swap_remove((r >> 8) as usize % live.len());
                seen.remove(&p);
                let c = class(l.size(), l.align()).unwrap();
                // SAFETY: `p` is live, ours, of class `c`.
                if let Err(p) = unsafe { cache.put(c, p) } {
                    // SAFETY: each spilled block came from the heap under `class_layout(c)`.
                    cache.spill(c, |b| unsafe {
                        heap.deallocate(b.as_ptr(), class_layout(c));
                    });
                    // SAFETY: as the first `put`; the spill made room.
                    assert!(unsafe { cache.put(c, p) }.is_ok());
                }
            }
        }
        for (p, l) in live.drain(..) {
            // SAFETY: live blocks from this heap, freed once, under the layout the heap gave them.
            unsafe { heap.deallocate(p.as_ptr(), heap_layout(l)) };
        }
        // SAFETY: as above, with the class layout every cached block was allocated under.
        cache.drain(|c, b| unsafe { heap.deallocate(b.as_ptr(), class_layout(c)) });
        let counters = heap.counters();
        assert_eq!(counters.allocation_count, 0);
        assert_eq!(
            counters.fragment_count, 1,
            "everything given back coalesces"
        );
    }
}
