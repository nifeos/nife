//! **`allocation_cache`**: a bounded stash of freed blocks in front of a shared heap, so most
//! allocations and frees never take the heap's lock. The algorithm only: it never locks, maps or
//! reads an owner's identity, and the owner decides who touches it.
//!
//! A [`Class`] is a block size on a 16-byte grid ([`GRID_BYTES`]) up to [`MAXIMUM_CLASS_BYTES`],
//! under one heap layout, [`class_layout`]. Only [`class`] makes one, so every method that takes a
//! class takes a valid one. A [`Cache`] holds a stack of freed blocks per class. Blocks of one
//! class are fungible: the heap sees the same layout at both ends, so any cached block can answer
//! any request of its class.

//! # What the owner promises
//!
//! 1. **One owner at a time.** A [`Cache`] is neither `Send` nor `Sync`; its owner keeps it where
//!    one thread reaches it.
//! 2. **[`refill`](Cache::refill) and [`spill`](Cache::spill) run under whatever excludes the other
//!    users of the heap**, once per batch, so the heap's lock is taken once for several blocks.
//! 3. **Every block a [`spill`](Cache::spill) or [`give_back_all`](Cache::give_back_all) hands back
//!    goes to the heap under [`class_layout`]`(class)`**, and every block the heap hands to
//!    [`refill`](Cache::refill) was allocated under it. A block that only meets
//!    [`push`](Cache::push)'s contract (a slice of a stack array, as the example uses) must never
//!    be handed on to a heap.
//!
//! The heap also has to see [`heap_layout`] on the path that never touches a cache. A block may be
//! allocated before its owner has a cache and freed into one afterward, and the cache then treats
//! it as [`class_bytes`] long; a heap that rounds or tags its own way would have given a smaller
//! block to the shorter request.
//!
//! An all-zero [`Cache`] is empty, so one may live in memory a runtime zeroes.
//!
//! The names [`push`](Cache::push) and [`pop`](Cache::pop) are what glibc calls `tcache_put` and
//! `tcache_get`.

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
//! assert!(unsafe { cache.push(c, p) }.is_ok());
//! assert_eq!((cache.pop(c), cache.pop(c)), (Some(p), None)); // then the caller goes to the heap
//! ```

//! # BUGS
//!
//! - A cached block cannot coalesce until it is spilled: up to [`MAXIMUM_CACHED_BYTES`] (34,816)
//!   per cache. The owner empties the cache with [`give_back_all`](Cache::give_back_all) at its
//!   end. A [`Cache`] dropped while it holds blocks leaks them, because it cannot free; a debug
//!   build panics in that drop.
//! - [`BLOCKS_PER_CLASS`] (glibc's `tcache` count) and [`MAXIMUM_CLASS_BYTES`] are chosen, not
//!   measured per workload, and they do not move: they are constants, not parameters of [`Cache`].
//! - A block freed by an owner other than the one that allocated it lands in the freeing owner's
//!   cache, bounded by [`BLOCKS_PER_CLASS`] and spilled at the bound.
//!
//! Name: provisional, minted 2026-10-10 (UTC), for what the crate holds rather than its owner. No
//! architect has ruled on it; the maintainer kept it 2026-11-14 (UTC), provisionally.

#![no_std]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

use core::alloc::Layout;
use core::ptr::NonNull;

/// The size step between classes, and the most alignment a cached request may ask for.
///
/// Name: provisional, spelled with its unit 2026-11-16 (UTC) as the maintainer's provisional ruling
/// of 2026-11-14 on decision 2 of the outside review.
pub const GRID_BYTES: usize = 16;

/// The largest effective size a cache holds.
///
/// Name: provisional, as [`GRID_BYTES`].
pub const MAXIMUM_CLASS_BYTES: usize = 256;

/// How many size classes there are: one per grid step up to [`MAXIMUM_CLASS_BYTES`].
pub const CLASSES: usize = MAXIMUM_CLASS_BYTES / GRID_BYTES;

/// How many blocks of one class a cache holds before a free spills.
///
/// Name: provisional, as [`GRID_BYTES`].
pub const BLOCKS_PER_CLASS: usize = 16;

/// How many blocks a refill takes from the heap, and a spill gives back, under one acquisition of
/// its lock: half of [`BLOCKS_PER_CLASS`], so an owner that only allocates or only frees takes the
/// lock once per eight blocks, and one that alternates at the boundary does not take it on every
/// call.
///
/// Name: provisional, as [`GRID_BYTES`].
pub const BATCH_BLOCKS: usize = BLOCKS_PER_CLASS / 2;

/// The most memory one full cache keeps out of the heap.
///
/// Name: provisional, as [`GRID_BYTES`].
pub const MAXIMUM_CACHED_BYTES: usize = BLOCKS_PER_CLASS * GRID_BYTES * CLASSES * (CLASSES + 1) / 2;

// A class is a byte, so the class count must fit one.
const _: () = assert!(CLASSES <= 256);

/// **A size class**: a block size on the grid, from one [`GRID_BYTES`] step to
/// [`MAXIMUM_CLASS_BYTES`]. Only [`class`] makes one, so a method that takes a `Class` cannot be
/// handed an index past the last class.
///
/// Name: provisional, minted 2026-11-16 (UTC) as the maintainer's provisional ruling of 2026-11-14
/// on decision 3 of the outside review.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Class(u8);

impl Class {
    /// The class at `index`, which is under [`CLASSES`].
    const fn at(index: usize) -> Self {
        debug_assert!(index < CLASSES);
        // `CLASSES` fits a byte (asserted above), and `index` is under it.
        #[allow(clippy::cast_possible_truncation)]
        Self(index as u8)
    }

    const fn index(self) -> usize {
        self.0 as usize
    }
}

/// The class of a request of `size` bytes aligned to `align`, or `None` when it is not cached.
pub const fn class(size: usize, align: usize) -> Option<Class> {
    if align > GRID_BYTES {
        return None;
    }
    // Compare before rounding: `next_multiple_of` wraps near `usize::MAX`, which would pass the bound.
    if size > MAXIMUM_CLASS_BYTES {
        return None;
    }
    let effective = if size == 0 {
        GRID_BYTES
    } else {
        size.next_multiple_of(GRID_BYTES)
    };
    Some(Class::at(effective / GRID_BYTES - 1))
}

/// The effective size of a class's blocks.
pub const fn class_bytes(class: Class) -> usize {
    (class.index() + 1) * GRID_BYTES
}

/// The layout every block of `class` is allocated from, and freed to, the heap under.
///
/// Name: provisional, minted 2026-10-21 (UTC).
pub const fn class_layout(class: Class) -> Layout {
    // SAFETY: a nonzero multiple of `GRID_BYTES`, a power of two, aligned to `GRID_BYTES`.
    unsafe { Layout::from_size_align_unchecked(class_bytes(class), GRID_BYTES) }
}

/// **The layout to give the heap for `layout`**, at both ends of an allocation: its class's when it
/// has one, whether or not a cache is there to serve it, and otherwise `layout` itself, made at
/// least one byte, since a heap may refuse an empty layout.
///
/// A block can be allocated before its owner has a cache and freed into one afterward, and the
/// cache then treats it as [`class_bytes`] long. A heap that rounds sizes its own way, or tags
/// each block, would have given a 17-byte request fewer bytes than its class's 32, so the uncached
/// path has to ask for the class's layout too.
///
/// Name: provisional, minted 2026-10-21 (UTC).
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

const _: () = assert!(core::mem::size_of::<Free>() <= GRID_BYTES);
// `push` casts a `GRID_BYTES`-aligned block to a `Free`, so the grid must satisfy its alignment.
const _: () = assert!(core::mem::align_of::<Free>() <= GRID_BYTES);

/// **One owner's stash**: a stack of freed blocks per class, each at most [`BLOCKS_PER_CLASS`]
/// deep.
///
/// The owner's rules are in the crate documentation: one owner at a time, batches under the
/// heap's exclusion, and every block handed back goes to the heap under [`class_layout`]. An
/// all-zero `Cache` is empty, so it can live in memory a runtime zeroes.
///
/// `const` so it can sit inside a statically made structure. It holds raw pointers into memory it
/// does not own, so it is neither `Send` nor `Sync`: its owner keeps it where one thread reaches
/// it. The owner empties it with [`give_back_all`](Self::give_back_all) before it drops: a cache
/// dropped while it holds blocks leaks them, and a debug build panics in that drop.
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

/// A cache that still holds blocks when it drops leaks them, because it cannot free. A debug build
/// says so, which catches the owner that forgot [`give_back_all`](Cache::give_back_all); a release
/// build does nothing.
impl Drop for Cache {
    fn drop(&mut self) {
        debug_assert!(
            self.cached_bytes() == 0,
            "a Cache dropped while it holds blocks leaks them; call give_back_all first"
        );
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
    pub fn count(&self, class: Class) -> usize {
        usize::from(self.counts[class.index()])
    }

    /// The bytes the cache is keeping out of the heap.
    pub fn cached_bytes(&self) -> usize {
        (0..CLASSES)
            .map(|c| {
                let c = Class::at(c);
                self.count(c) * class_bytes(c)
            })
            .sum()
    }

    /// A cached block of `class`, or `None` when there is none and the caller must go to the heap.
    /// The glibc spelling is `tcache_get`.
    #[doc(alias = "tcache_get")]
    pub fn pop(&mut self, class: Class) -> Option<NonNull<u8>> {
        let head = NonNull::new(self.heads[class.index()])?;
        // SAFETY: `head` is a block this cache holds (`push`'s contract), whose first word `push`
        // wrote as a `Free`.
        self.heads[class.index()] = unsafe { (*head.as_ptr()).next };
        self.counts[class.index()] -= 1;
        Some(head.cast())
    }

    /// Keep `block` as a free block of `class`. Hands it back as `Err` when the class is full, and
    /// the caller should [`spill`](Self::spill) and try again, or free it to the heap. The glibc
    /// spelling is `tcache_put`.
    ///
    /// # Safety
    ///
    /// `block` is [`class_bytes`]`(class)` bytes, writable, aligned to [`GRID_BYTES`], and from now
    /// on used by nothing but this cache, until [`pop`](Self::pop) or a spill hands it back. A
    /// block the owner means to [`spill`](Self::spill) or [`give_back_all`](Self::give_back_all)
    /// into a heap also has to be one that heap accepts under [`class_layout`]`(class)`; the
    /// crate documentation says so.
    #[doc(alias = "tcache_put")]
    pub unsafe fn push(&mut self, class: Class, block: NonNull<u8>) -> Result<(), NonNull<u8>> {
        if self.count(class) >= BLOCKS_PER_CLASS {
            return Err(block);
        }
        // The caller promises `GRID_BYTES` alignment, which is at least a `Free`'s.
        #[allow(clippy::cast_ptr_alignment)]
        let node = block.as_ptr().cast::<Free>();
        // SAFETY: the caller's contract: the block is ours, writable and big and aligned enough.
        unsafe {
            node.write(Free {
                next: self.heads[class.index()],
            });
        };
        self.heads[class.index()] = node;
        self.counts[class.index()] += 1;
        Ok(())
    }

    /// Hand up to [`BATCH_BLOCKS`] blocks of `class` to `give`, which returns them to the heap. It
    /// hands back only blocks that [`push`](Self::push) or [`refill`](Self::refill) took. Call it
    /// holding the heap's lock, once, rather than locking per block.
    pub fn spill(&mut self, class: Class, mut give: impl FnMut(NonNull<u8>)) {
        for _ in 0..BATCH_BLOCKS {
            match self.pop(class) {
                Some(b) => give(b),
                None => break,
            }
        }
    }

    /// Fill `class` with up to [`BATCH_BLOCKS`] blocks from `get`, which allocates them from the
    /// heap and answers `None` when it has none. Call it holding the heap's lock. Returns how many
    /// it took.
    ///
    /// # Safety
    ///
    /// Every block `get` answers meets [`push`](Self::push)'s contract for `class`.
    pub unsafe fn refill(
        &mut self,
        class: Class,
        mut get: impl FnMut() -> Option<NonNull<u8>>,
    ) -> usize {
        let mut taken = 0;
        while taken < BATCH_BLOCKS && self.count(class) < BLOCKS_PER_CLASS {
            let Some(b) = get() else { break };
            // SAFETY: the caller's contract for what `get` answers; the class has room (checked).
            let kept = unsafe { self.push(class, b) };
            debug_assert!(kept.is_ok());
            taken += 1;
        }
        taken
    }

    /// Hand every block to `give` with its class, leaving the cache empty: an owner's end. It
    /// hands back only blocks that [`push`](Self::push) or [`refill`](Self::refill) took.
    ///
    /// Name: provisional, per the maintainer's provisional ruling of 2026-11-14 on decision 2 of
    /// the outside review: `drain` takes a closure here, where `std`'s `drain` returns an
    /// iterator.
    pub fn give_back_all(&mut self, mut give: impl FnMut(Class, NonNull<u8>)) {
        for index in 0..CLASSES {
            let c = Class::at(index);
            while let Some(b) = self.pop(c) {
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
        let mut arena: Vec<u128> = std::vec![0; MAXIMUM_CACHED_BYTES / GRID_BYTES];
        let base = arena.as_mut_ptr().cast::<u8>();
        let mut cache = Cache::new();
        let mut off = 0;
        for index in 0..CLASSES {
            let c = Class::at(index);
            for _ in 0..BLOCKS_PER_CLASS {
                // SAFETY: disjoint, grid-aligned slices of `arena`, which outlives `cache`.
                let b = unsafe { NonNull::new_unchecked(base.add(off)) };
                // SAFETY: as above; each slice is handed to the cache once.
                assert!(unsafe { cache.push(c, b) }.is_ok());
                off += class_bytes(c);
            }
            let spare = NonNull::new(base).unwrap();
            // SAFETY: refused before the block is touched, so aliasing it is harmless.
            assert_eq!(unsafe { cache.push(c, spare) }, Err(spare));
        }
        assert_eq!(off, MAXIMUM_CACHED_BYTES);
        assert_eq!(cache.cached_bytes(), MAXIMUM_CACHED_BYTES);
        let mut n = 0;
        cache.give_back_all(|_, _| n += 1);
        assert_eq!(n, CLASSES * BLOCKS_PER_CLASS);
        assert_eq!(cache.cached_bytes(), 0);
    }

    /// A request with no class reaches the heap exactly as asked, because the heap frees it under
    /// the same layout and only a class's blocks are fungible. The one change is an empty layout,
    /// over-aligned past the grid, which becomes one byte, since talc refuses an empty one.
    #[test]
    fn a_request_without_a_class_reaches_the_heap_as_asked_unless_empty() {
        for (size, align) in [
            (MAXIMUM_CLASS_BYTES + 1, 8),
            (4096, 4096),
            (1, 2 * GRID_BYTES),
        ] {
            let l = Layout::from_size_align(size, align).unwrap();
            assert_eq!(heap_layout(l), l);
        }
        let empty = Layout::from_size_align(0, 2 * GRID_BYTES).unwrap();
        assert_eq!(
            heap_layout(empty),
            Layout::from_size_align(1, 2 * GRID_BYTES).unwrap()
        );
    }

    /// The classes stop exactly at [`MAXIMUM_CLASS_BYTES`]: its class is the last one, so no class is sized
    /// past what [`class`] can ever answer.
    #[test]
    fn the_last_class_is_the_largest_size() {
        assert_eq!(class(MAXIMUM_CLASS_BYTES, 1), Some(Class::at(CLASSES - 1)));
        assert_eq!(class_bytes(Class::at(CLASSES - 1)), MAXIMUM_CLASS_BYTES);
    }

    /// An alignment equal to the grid is inside [`class`], and the first refusal is one past it:
    /// both ends of the comparison are the constants, so the boundary is asserted, not hoped for.
    #[test]
    fn an_alignment_equal_to_the_grid_is_a_class_and_one_past_is_not() {
        assert_eq!(class(1, GRID_BYTES), Some(Class::at(0)));
        assert_eq!(
            class(MAXIMUM_CLASS_BYTES, GRID_BYTES),
            Some(Class::at(CLASSES - 1))
        );
        assert_eq!(class(1, GRID_BYTES + 1), None);
    }

    /// A cache prints its counts, not its pointers.
    #[test]
    fn a_cache_prints_counts_and_not_pointers() {
        let mut block = [0u128; 1];
        let mut cache = Cache::new();
        // SAFETY: 16 writable, grid-aligned bytes nobody else uses while the cache holds them.
        assert!(unsafe { cache.push(Class::at(0), NonNull::from(&mut block).cast()) }.is_ok());
        let text = std::format!("{cache:?}");
        assert!(text.contains("cached_bytes: 16"), "{text}");
        assert!(!text.contains("0x"), "{text}");
        cache.give_back_all(|_, _| {});
    }

    /// A debug build notices an owner that drops a cache still holding a block, which would leak it.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "leaks them")]
    fn a_cache_dropped_while_it_holds_a_block_is_noticed() {
        let mut block = [0u128; 1];
        let mut cache = Cache::new();
        // SAFETY: 16 writable, grid-aligned bytes nobody else uses while the cache holds them.
        assert!(unsafe { cache.push(Class::at(0), NonNull::from(&mut block).cast()) }.is_ok());
    }

    /// A class is one byte wide, so a `Class` costs what the index it replaces cost.
    #[test]
    fn a_class_is_one_byte() {
        assert_eq!(core::mem::size_of::<Class>(), 1);
        assert_eq!(core::mem::size_of::<Option<Class>>(), 2);
    }

    /// A size no `Layout` reaches is refused, not wrapped: rounding `usize::MAX` up to the grid
    /// wraps to zero, which would pass the bound.
    #[test]
    fn a_size_near_the_top_of_usize_has_no_class() {
        assert_eq!(class(usize::MAX, 8), None);
        assert_eq!(class(usize::MAX - GRID_BYTES + 1, 8), None);
        assert_eq!(class(MAXIMUM_CLASS_BYTES + 1, 8), None);
        assert_eq!(class(MAXIMUM_CLASS_BYTES, 8), Some(Class::at(CLASSES - 1)));
        assert_eq!(class(0, 1), Some(Class::at(0)));
    }

    /// An all-zero cache is empty, which a runtime that zeroes the memory it holds a cache in relies on.
    #[test]
    fn an_all_zero_cache_is_empty() {
        // SAFETY: `Cache` holds null raw pointers and byte counters, valid when all-zero.
        let zeroed: Cache = unsafe { core::mem::zeroed() };
        assert_eq!(zeroed.cached_bytes(), 0);
        for index in 0..CLASSES {
            assert_eq!(zeroed.count(Class::at(index)), 0);
        }
        let mut zeroed = zeroed;
        assert_eq!(zeroed.pop(Class::at(0)), None);
    }

    /// A spill gives back half a full class and a refill into an empty one takes half a class, so
    /// an owner that only frees, or only allocates, takes the heap's lock once per eight blocks.
    #[test]
    fn a_spill_and_a_refill_each_move_half_a_class() {
        let mut arena: Vec<u128> = std::vec![0; BLOCKS_PER_CLASS];
        let base = arena.as_mut_ptr().cast::<u8>();
        let c0 = Class::at(0);
        let mut cache = Cache::new();
        for i in 0..BLOCKS_PER_CLASS {
            // SAFETY: disjoint 16-byte, grid-aligned slices of `arena`, which outlives `cache`.
            let b = unsafe { NonNull::new_unchecked(base.add(i * GRID_BYTES)) };
            // SAFETY: as above; each slice is handed to the cache once.
            assert!(unsafe { cache.push(c0, b) }.is_ok());
        }
        let mut given = 0;
        cache.spill(c0, |_| given += 1);
        assert_eq!(given, BLOCKS_PER_CLASS / 2);
        assert_eq!(cache.count(c0), BLOCKS_PER_CLASS / 2);

        let mut empty = Cache::new();
        let mut next = 0;
        // SAFETY: the same disjoint slices, now owned by `empty` alone (`cache` is not used again).
        let taken = unsafe {
            empty.refill(c0, || {
                next += 1;
                NonNull::new(base.wrapping_add((next - 1) * GRID_BYTES))
            })
        };
        assert_eq!(taken, BLOCKS_PER_CLASS / 2);
        assert_eq!(empty.count(c0), BLOCKS_PER_CLASS / 2);
        // The two caches now name the same blocks, which belong to `arena`, so neither frees them.
        core::mem::forget(cache);
        core::mem::forget(empty);
    }

    /// The test that matters: a cache in front of a real heap, `talc`, driven by a
    /// random mix of allocations and frees with refills and spills, never hands out a block twice,
    /// never hands one out shorter than its class, and gives every byte back, coalesced, at the end.
    ///
    /// One allocation in four goes to the heap directly under [`heap_layout`], as an owner without
    /// a cache does, and every free goes through the cache. That is the case a heap that rounds its
    /// own way breaks: a block allocated under its own layout, then cached as its class's size.
    /// The heap is claimed in two runs, as the runtime grows it, so `extend` is walked too.
    #[test]
    fn a_cache_in_front_of_the_heap_never_hands_out_a_block_twice() {
        use talc::base::Talc;
        use talc::base::binning::DefaultBinning;
        use talc::source::Manual;

        const ARENA: usize = 64 * 1024;
        let mut arena: Vec<u128> = std::vec![0; ARENA / GRID_BYTES];
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
                let size = (r >> 8) as usize % (MAXIMUM_CLASS_BYTES + 1);
                let l = Layout::from_size_align(size, 8).unwrap();
                let c = class(size, 8).unwrap();
                let p = if r % 4 == 1 {
                    // SAFETY: `heap_layout` is never empty.
                    match unsafe { heap.try_allocate(heap_layout(l)) } {
                        Some(p) => p,
                        None => continue, // the arena is full; fine
                    }
                } else {
                    match cache.pop(c) {
                        Some(p) => p,
                        None => {
                            // SAFETY: each block is the heap's, allocated under its class's layout.
                            unsafe {
                                cache.refill(c, || heap.try_allocate(class_layout(c)));
                            }
                            match cache.pop(c) {
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
                if let Err(p) = unsafe { cache.push(c, p) } {
                    // SAFETY: each spilled block came from the heap under `class_layout(c)`.
                    cache.spill(c, |b| unsafe {
                        heap.deallocate(b.as_ptr(), class_layout(c));
                    });
                    // SAFETY: as the first `put`; the spill made room.
                    assert!(unsafe { cache.push(c, p) }.is_ok());
                }
            }
        }
        for (p, l) in live.drain(..) {
            // SAFETY: live blocks from this heap, freed once, under the layout the heap gave them.
            unsafe { heap.deallocate(p.as_ptr(), heap_layout(l)) };
        }
        // SAFETY: as above, with the class layout every cached block was allocated under.
        cache.give_back_all(|c, b| unsafe { heap.deallocate(b.as_ptr(), class_layout(c)) });
        let counters = heap.counters();
        assert_eq!(counters.allocation_count, 0);
        assert_eq!(
            counters.fragment_count, 1,
            "everything given back coalesces"
        );
    }
}
