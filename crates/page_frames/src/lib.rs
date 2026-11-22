//! A physical frame allocator: hands out 4 KiB pages of physical memory from a bitmap, one bit per
//! frame. It is the bottom of the memory hierarchy; page tables, the kernel heap, direct memory
//! access (DMA) buffers and user pages all ask it, and nothing is underneath it to ask. Pure logic with no `unsafe`, so its
//! tests run on the host. It is a bitmap and not a free list because a device doing DMA needs
//! contiguous runs and because a bitmap tests on the host without an operating system.
//!
//! Name: ratified 2026-08-23 (calef, a kernel-dependency crate naming review). Renamed from
//! `frames`, which collided with a compositor's own use of "frame" for a rendered screen
//! update. The page frame this crate hands out is also what a kernel names its capability for a
//! page of memory. This crate is the physical-page allocator that backs those page
//! frames.

#![no_std]
#![forbid(unsafe_code)]

// The casts between `u64` and `usize` below (a frame count, a bitmap index) are exact only where
// `usize` holds 64 bits; a narrower target would truncate them silently.
const _: () = assert!(usize::BITS >= 64, "page_frames needs a 64-bit usize");

/// 4 KiB. The unit the memory management unit (MMU) translates in, so it is the unit we
/// allocate in.
///
/// Name: ratified 2026-11-20 (UTC).
pub const FRAME_BYTES: u64 = 4096;

/// A physical address, guaranteed frame-aligned.
///
/// A newtype rather than a bare `u64`, so the compiler keeps physical and virtual addresses
/// apart.
///
/// # Examples
///
/// A frame is an address that is known to be aligned, which is the point of the newtype:
///
/// ```
/// use page_frames::{PageFrame, FRAME_BYTES};
///
/// // `containing` rounds down to the frame an arbitrary address falls in.
/// assert_eq!(PageFrame::containing(0x4000_0FFF).physical_address(), 0x4000_0000);
/// assert_eq!(PageFrame::containing(0x4000_1000).physical_address(), 0x4000_1000);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageFrame(u64);

impl PageFrame {
    /// Name the frame at `physical_address`.
    ///
    /// Naming a frame is not the same as owning it. This type carries one invariant
    /// (the address is frame-aligned) and no claim about who the frame belongs to.
    /// Ownership lives entirely in the allocator's bitmap, which is where it can
    /// actually be checked.
    ///
    /// # Panics
    /// If `physical_address` is not frame-aligned.
    ///
    /// Name: ratified 2026-11-20 (UTC).
    #[must_use]
    pub const fn from_physical_address(physical_address: u64) -> Self {
        assert!(
            physical_address.is_multiple_of(FRAME_BYTES),
            "address is not frame-aligned"
        );
        PageFrame(physical_address)
    }

    /// The frame at `physical_address`, or `None` when it is not frame-aligned: the fallible form
    /// of [`from_physical_address`](Self::from_physical_address), for a caller that holds an address it has not checked.
    ///
    /// Name: ratified 2026-11-20 (UTC).
    #[must_use]
    pub const fn try_aligned(physical_address: u64) -> Option<Self> {
        if physical_address.is_multiple_of(FRAME_BYTES) {
            Some(PageFrame(physical_address))
        } else {
            None
        }
    }

    /// The frame containing `physical_address`, rounding down.
    #[must_use]
    pub const fn containing(physical_address: u64) -> Self {
        PageFrame(physical_address - physical_address % FRAME_BYTES)
    }

    /// The physical address of this frame's first byte.
    ///
    /// Name: ratified 2026-11-20 (UTC).
    #[must_use]
    pub const fn physical_address(self) -> u64 {
        self.0
    }
}

/// A snapshot of an allocator's occupancy, from [`PageFrameAllocator::stats`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// Frames the allocator tracks in total.
    pub frames: usize,
    /// Frames currently marked used.
    pub used_frames: usize,
}

impl Stats {
    /// Frames currently available: `frames - used_frames`.
    ///
    /// Name: ratified 2026-11-20 (UTC).
    #[must_use]
    pub fn free_frames(&self) -> usize {
        self.frames - self.used_frames
    }
}

/// One bit per frame. `1` means used.
///
/// **Everything starts used**: a frame is handed out only after someone has said its range is real
/// RAM, so an unknown address (a memory-mapped input and output (MMIO) hole) is never allocated.
/// A frame claimed free by mistake is memory some other owner holds, so the allocator trusts
/// nothing it was not told.
///
/// # Examples
///
/// The wiring an early boot does: name the span, hand it a bitmap, then declare what is real RAM
/// and what inside it is already spoken for.
///
/// ```
/// use page_frames::{PageFrameAllocator, PageFrame, FRAME_BYTES};
///
/// const BASE: u64 = 0x4000_0000;
/// const TOTAL: usize = 64;                      // 64 frames of address space
/// let mut bitmap = [0u8; PageFrameAllocator::bitmap_bytes(TOTAL)];
/// let mut allocator = PageFrameAllocator::new(BASE, TOTAL, &mut bitmap);
///
/// // Nothing is free until someone says so: an allocator starts owning nothing.
/// assert_eq!(allocator.stats().free_frames(), 0);
///
/// allocator.mark_free(BASE, TOTAL as u64 * FRAME_BYTES);        // all of it is real RAM
/// allocator.mark_used(BASE, 4 * FRAME_BYTES);                   // ... but the first four hold the kernel
/// assert_eq!(allocator.stats().free_frames(), TOTAL - 4);
///
/// let f = allocator.alloc().expect("60 frames are free");
/// assert!(f.physical_address() >= BASE + 4 * FRAME_BYTES, "never hands back the kernel's own frames");
/// assert_eq!(allocator.stats().free_frames(), TOTAL - 5);
///
/// allocator.free(f);
/// assert_eq!(allocator.stats().free_frames(), TOTAL - 4);
/// ```
pub struct PageFrameAllocator<'a> {
    bitmap: &'a mut [u8],
    /// Physical address of frame 0.
    base: u64,
    /// How many frames we track. May be less than `bitmap.len() * 8`.
    total: usize,
    used: usize,
    /// Where the next linear scan starts. Purely an optimization; correctness does not
    /// depend on it.
    hint: usize,
}

impl<'a> PageFrameAllocator<'a> {
    /// Bytes of bitmap needed to track `frames` frames.
    #[must_use]
    pub const fn bitmap_bytes(frames: usize) -> usize {
        frames.div_ceil(8)
    }

    /// How many **whole** frames fit in `span_bytes`, truncating a partial frame at the end. For
    /// the frames a span must reach, partial ones included, use
    /// [`page_frames_covering`](Self::page_frames_covering).
    #[must_use]
    pub const fn page_frames_in(span_bytes: u64) -> usize {
        (span_bytes / FRAME_BYTES) as usize
    }

    /// How many frames `span_bytes` touch, rounding a partial frame at the end up: the count a
    /// bitmap must track to reach the end of the span.
    ///
    /// Name: ratified 2026-11-20 (UTC).
    #[must_use]
    pub const fn page_frames_covering(span_bytes: u64) -> usize {
        span_bytes.div_ceil(FRAME_BYTES) as usize
    }

    /// Create an allocator covering `total` frames starting at `base`, with **every
    /// frame marked used**.
    ///
    /// Call [`mark_free`](Self::mark_free) for each region of real RAM, then
    /// [`mark_used`](Self::mark_used) for everything inside it that is already spoken
    /// for: the kernel image, the device tree, and the bitmap itself.
    ///
    /// # Panics
    /// If `bitmap` is too small, or `base` is not frame-aligned.
    pub fn new(base: u64, total: usize, bitmap: &'a mut [u8]) -> Self {
        assert!(
            base.is_multiple_of(FRAME_BYTES),
            "base must be frame-aligned"
        );
        assert!(
            bitmap.len() >= Self::bitmap_bytes(total),
            "bitmap too small: {} bytes for {} frames",
            bitmap.len(),
            total
        );

        bitmap.fill(0xff);

        Self {
            bitmap,
            base,
            total,
            used: total,
            hint: 0,
        }
    }

    /// The allocator's current occupancy.
    #[must_use]
    pub fn stats(&self) -> Stats {
        Stats {
            frames: self.total,
            used_frames: self.used,
        }
    }

    /// Mark every frame **wholly inside** `[start, start+bytes)` as free.
    ///
    /// The start rounds **up** and the end **down**, the opposite of
    /// [`mark_used`](Self::mark_used): a frame only partly inside the region also holds bytes the
    /// caller never said were RAM, so it stays used. A region that holds no whole frame frees
    /// nothing. Under-freeing wastes at most 4 KiB at each end of a region. Frames outside what
    /// the allocator manages are ignored. Usable memory rounds in and reserved memory rounds out,
    /// so a mistake at the edge of a region wastes a frame and never hands out a held one.
    pub fn mark_free(&mut self, start: u64, bytes: u64) {
        for i in self.whole_frames_in(start, bytes) {
            if self.is_index_used(i) {
                self.set(i, false);
                self.used -= 1;
            }
        }
        self.hint = 0;
    }

    /// Mark every frame **touched by** `[start, start+bytes)` as used.
    ///
    /// The start rounds **down** and the end **up**, the opposite of
    /// [`mark_free`](Self::mark_free): a region ending mid-frame (a kernel image ending at
    /// `0x4009_7010`) still owns that frame, and rounding down would hand out the tail of the
    /// kernel. Over-reserving wastes at most 4 KiB at each end of a region. Frames outside what the
    /// allocator manages are ignored. Usable memory rounds in and reserved memory rounds out,
    /// so a mistake at the edge of a region wastes a frame and never hands out a held one.
    pub fn mark_used(&mut self, start: u64, bytes: u64) {
        for i in self.page_frame_range(start, bytes) {
            if !self.is_index_used(i) {
                self.set(i, true);
                self.used += 1;
            }
        }
    }

    /// Allocate one frame.
    #[must_use = "a dropped frame stays marked used and is leaked"]
    pub fn alloc(&mut self) -> Option<PageFrame> {
        let i = (self.hint..self.total)
            .chain(0..self.hint)
            .find(|&i| !self.is_index_used(i))?;

        self.set(i, true);
        self.used += 1;
        self.hint = i + 1;
        Some(PageFrame(self.base + i as u64 * FRAME_BYTES))
    }

    /// Allocate `count` **physically contiguous** frames, and return the first.
    ///
    /// This is the reason we are a bitmap and not a free list. A device doing DMA reads
    /// physical addresses directly, with no MMU in the way to paper over a scattered
    /// buffer, so a virtio ring needs its frames genuinely adjacent.
    ///
    /// The scan is O(total) and dumb. It is called rarely and never in a hot path. The scan hint
    /// that [`alloc`](Self::alloc) advances stays where it was: the hint is only a place to start
    /// looking, and the next `alloc` skips over the run's now-used frames and still answers a free
    /// frame.
    ///
    /// # BUGS
    /// - The run has no alignment beyond one frame. A device that needs more cannot ask; an
    ///   `alloc_contiguous_aligned(count, align_frames)` lands when a caller appears.
    #[must_use = "a dropped run stays marked used and is leaked"]
    pub fn alloc_contiguous(&mut self, count: usize) -> Option<PageFrame> {
        if count == 0 || count > self.total {
            return None;
        }

        let mut run_start = 0;
        let mut run = 0;

        for i in 0..self.total {
            if self.is_index_used(i) {
                run = 0;
                run_start = i + 1;
                continue;
            }

            run += 1;
            if run == count {
                for j in run_start..run_start + count {
                    self.set(j, true);
                }
                self.used += count;
                return Some(PageFrame(self.base + run_start as u64 * FRAME_BYTES));
            }
        }

        None
    }

    /// **The longest run of free frames, in frames.** The number
    /// [`alloc_contiguous`](Self::alloc_contiguous) can still satisfy, which is not the same
    /// question as [`Stats::free_frames`] and is the one that actually fails a boot: a boot can hold
    /// 137 frames free and no run of 128, and the failure then surfaces far from its cause, as a
    /// program that cannot be loaded.
    ///
    /// Same O(total) scan as `alloc_contiguous` and for the same reason: it is an instrument, called
    /// between tests, never in a hot path.
    #[must_use]
    pub fn largest_free_run(&self) -> usize {
        let mut best = 0;
        let mut run = 0;
        for i in 0..self.total {
            if self.is_index_used(i) {
                run = 0;
            } else {
                run += 1;
                if run > best {
                    best = run;
                }
            }
        }
        best
    }

    /// Return a frame to the pool.
    ///
    /// # Panics
    /// On a double free, and on a frame we never owned. Both are kernel bugs, and a
    /// kernel that keeps running after one is a kernel that corrupts memory somewhere
    /// far away and blames innocent code. Fail loudly and immediately.
    pub fn free(&mut self, frame: PageFrame) {
        let i = self.index_of(frame).expect("freeing a frame we do not own");
        assert!(
            self.is_index_used(i),
            "double free of frame {:#x}",
            frame.physical_address()
        );

        self.set(i, false);
        self.used -= 1;
        self.hint = self.hint.min(i);
    }

    /// Return `count` frames from `first`, the run an [`alloc_contiguous`](Self::alloc_contiguous)
    /// of `count` handed out. A `count` of zero frees nothing.
    ///
    /// Every frame of the run is checked before any is freed, so a bad call panics with the
    /// bitmap unchanged.
    ///
    /// # Panics
    /// When the run reaches past the frames the allocator owns, and when any frame of it is
    /// already free. Both are the same kernel bugs [`free`](Self::free) refuses.
    ///
    /// Name: provisional, minted 2026-11-22 (UTC) by lane 1103-finish-review. The partner of
    /// `alloc_contiguous`, as `free` is of `alloc`.
    pub fn free_contiguous(&mut self, first: PageFrame, count: usize) {
        if count == 0 {
            return;
        }
        let start = self
            .index_of(first)
            .expect("freeing a run we do not own");
        let end = start
            .checked_add(count)
            .filter(|&end| end <= self.total)
            .expect("freeing a run that reaches past the frames we own");
        if let Some(i) = (start..end).find(|&i| !self.is_index_used(i)) {
            panic!(
                "double free of frame {:#x} in a run",
                self.base + i as u64 * FRAME_BYTES
            );
        }

        for i in start..end {
            self.set(i, false);
        }
        self.used -= count;
        self.hint = self.hint.min(start);
    }

    /// Is this frame currently allocated? Test-support, and useful in assertions.
    #[must_use]
    pub fn is_used(&self, frame: PageFrame) -> Option<bool> {
        Some(self.is_index_used(self.index_of(frame)?))
    }

    fn index_of(&self, frame: PageFrame) -> Option<usize> {
        let physical_address = frame.physical_address();
        if physical_address < self.base {
            return None;
        }
        let i = ((physical_address - self.base) / FRAME_BYTES) as usize;
        (i < self.total).then_some(i)
    }

    /// Every frame index wholly inside `[start, start+bytes)`, clamped to what we manage.
    ///
    /// Start rounds **up**, end rounds **down**, and a range holding no whole frame is empty. See
    /// [`mark_free`](Self::mark_free). `base` is frame-aligned, so rounding an offset from it
    /// rounds the address.
    fn whole_frames_in(&self, start: u64, bytes: u64) -> core::ops::Range<usize> {
        let end = start.saturating_add(bytes);
        if end <= self.base {
            return 0..0;
        }

        let first = start.max(self.base) - self.base;
        let last = end - self.base;

        let lo = first.div_ceil(FRAME_BYTES);
        let hi = (last / FRAME_BYTES).min(self.total as u64);

        if lo >= hi {
            0..0
        } else {
            lo as usize..hi as usize
        }
    }

    /// Every frame index touched by `[start, start+bytes)`, clamped to what we manage.
    ///
    /// Start rounds **down**, end rounds **up**. See [`mark_used`](Self::mark_used).
    fn page_frame_range(&self, start: u64, bytes: u64) -> core::ops::Range<usize> {
        if bytes == 0 || start.saturating_add(bytes) <= self.base {
            return 0..0;
        }

        let first = start.max(self.base) - self.base;
        let last = (start.saturating_add(bytes)).saturating_sub(self.base);

        let lo = (first / FRAME_BYTES) as usize;
        let hi = (last.div_ceil(FRAME_BYTES) as usize).min(self.total);

        if lo >= hi { 0..0 } else { lo..hi }
    }

    fn is_index_used(&self, i: usize) -> bool {
        self.bitmap[i / 8] & (1 << (i % 8)) != 0
    }

    fn set(&mut self, i: usize, used: bool) {
        let byte = &mut self.bitmap[i / 8];
        let mask = 1 << (i % 8);
        if used {
            *byte |= mask;
        } else {
            *byte &= !mask;
        }
    }
}

/// Machine-checked proofs of the frame allocator (a bounded model checker proves each property for every input within its bounds).
///
/// The allocator is the bottom of the memory hierarchy, so its correctness is the floor everything
/// above it stands on. The flagship property is the one address-space isolation ultimately rests on:
/// **`alloc` never hands the same physical frame out twice.** Because these harnesses live inside the
/// crate, they build a small allocator over a *symbolic* bitmap directly, rather than through `new`
/// (which fills the bitmap all-used).
#[cfg(kani)]
mod verification {
    use super::*;

    /// **`containing` rounds down to an aligned frame that holds the address.** For every address,
    /// the frame it names is aligned, sits at or below the address, and the address is within the
    /// 4 KiB it covers.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.containing_rounds_down_within_a_frame.patch`
    #[kani::proof]
    fn containing_rounds_down_within_a_frame() {
        let physical_address: u64 = kani::any();
        let f = PageFrame::containing(physical_address);
        assert!(f.physical_address().is_multiple_of(FRAME_BYTES));
        assert!(f.physical_address() <= physical_address);
        assert!(physical_address - f.physical_address() < FRAME_BYTES);
    }

    /// **The bitmap is always big enough.** `bitmap_bytes(n)` gives enough bytes to hold one bit per
    /// frame, for every frame count. An undersized bitmap would index out of bounds in `get`/`set`.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.bitmap_bytes_covers_every_frame.patch`
    #[kani::proof]
    fn bitmap_bytes_covers_every_frame() {
        let frames: usize = kani::any();
        kani::assume(frames <= 1 << 40); // a real RAM frame count; keeps the `* 8` below in range
        assert!(PageFrameAllocator::bitmap_bytes(frames) * 8 >= frames);
    }

    /// **`PageFrame` address and bitmap index are inverses.** A frame at `base + i * FRAME_BYTES`, for any
    /// in-range `i`, maps back to index `i`. This is what makes the allocator's naming unambiguous:
    /// no two indices share an address and no address is misfiled.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.index_of_inverts_frame_addressing.patch`
    #[kani::proof]
    fn index_of_inverts_frame_addressing() {
        let base: u64 = kani::any();
        let total: usize = kani::any();
        kani::assume(base.is_multiple_of(FRAME_BYTES));
        kani::assume(total <= 1 << 20);
        kani::assume(base.checked_add(total as u64 * FRAME_BYTES).is_some());
        let i: usize = kani::any();
        kani::assume(i < total);

        let mut bitmap = [0u8; 1]; // index_of reads only base and total, never the bitmap
        let allocator = PageFrameAllocator {
            bitmap: &mut bitmap,
            base,
            total,
            used: 0,
            hint: 0,
        };
        let frame = PageFrame(base + i as u64 * FRAME_BYTES);
        assert_eq!(allocator.index_of(frame), Some(i));
    }

    /// **Two allocations are never the same frame.** Over any bitmap state, if two back-to-back
    /// `alloc`s both succeed they return distinct physical frames. This is the property the whole
    /// isolation story rests on: the allocator cannot hand one physical page to two owners.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.two_allocations_are_distinct.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn two_allocations_are_distinct() {
        let mut bitmap: [u8; 1] = kani::any(); // eight frames, arbitrary used/free pattern
        let mut allocator = PageFrameAllocator {
            bitmap: &mut bitmap,
            base: 0,
            total: 8,
            used: 0,
            hint: 0,
        };
        let a = allocator.alloc();
        let b = allocator.alloc();
        if let (Some(first), Some(second)) = (a, b) {
            assert_ne!(first.physical_address(), second.physical_address());
        }
    }

    /// **`mark_free` frees only frames wholly inside the range.** From an all-used bitmap, over any
    /// aligned base and any `start` and `bytes`, every frame `mark_free` leaves free lies entirely
    /// within `[start, start + bytes)`. A frame only partly inside a usable region also holds bytes
    /// nobody said were RAM, and handing it out would put a page table or a heap block over them.
    /// The end is compared in `u128`, so a range that wraps the address space is held to its true
    /// end and not a saturated one.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.mark_free_frees_only_whole_frames_inside_the_range.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn mark_free_frees_only_whole_frames_inside_the_range() {
        let base: u64 = kani::any();
        kani::assume(base.is_multiple_of(FRAME_BYTES));
        kani::assume(base.checked_add(8 * FRAME_BYTES).is_some());
        let start: u64 = kani::any();
        let bytes: u64 = kani::any();

        let mut bitmap = [0xffu8; 1]; // eight frames, all used, as `new` leaves them
        let mut allocator = PageFrameAllocator {
            bitmap: &mut bitmap,
            base,
            total: 8,
            used: 8,
            hint: 0,
        };
        allocator.mark_free(start, bytes);

        let i: usize = kani::any();
        kani::assume(i < 8);
        if !allocator.is_index_used(i) {
            let frame_start = u128::from(base) + i as u128 * u128::from(FRAME_BYTES);
            let frame_end = frame_start + u128::from(FRAME_BYTES);
            assert!(frame_start >= u128::from(start));
            assert!(frame_end <= u128::from(start) + u128::from(bytes));
        }
    }

    /// **An allocated frame is aligned and in range.** Whatever the bitmap holds, a frame `alloc`
    /// returns is frame-aligned and lies within `[base, base + total * FRAME_BYTES)`.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.an_allocated_frame_is_aligned_and_in_range.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn an_allocated_frame_is_aligned_and_in_range() {
        let mut bitmap: [u8; 1] = kani::any();
        let base = 0x4000_0000u64; // an arbitrary aligned base
        let mut allocator = PageFrameAllocator {
            bitmap: &mut bitmap,
            base,
            total: 8,
            used: 0,
            hint: 0,
        };
        if let Some(f) = allocator.alloc() {
            assert!(f.physical_address().is_multiple_of(FRAME_BYTES));
            assert!(f.physical_address() >= base);
            assert!(f.physical_address() < base + 8 * FRAME_BYTES);
        }
    }

    /// **`mark_used` claims exactly the frames the range touches.** From an all-free bitmap, over
    /// any aligned base and any `start` and `bytes`, a frame is used afterwards when and only when
    /// it overlaps `[start, start + bytes)`: a region that ends mid-frame still owns that frame, and
    /// no frame it does not touch is claimed. The end is compared in `u128`, so a range that wraps
    /// the address space is held to its true end.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.mark_used_claims_exactly_the_frames_the_range_touches.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn mark_used_claims_exactly_the_frames_the_range_touches() {
        let base: u64 = kani::any();
        kani::assume(base.is_multiple_of(FRAME_BYTES));
        kani::assume(base.checked_add(8 * FRAME_BYTES).is_some());
        let start: u64 = kani::any();
        let bytes: u64 = kani::any();

        let mut bitmap = [0u8; 1]; // eight frames, all free
        let mut allocator = PageFrameAllocator {
            bitmap: &mut bitmap,
            base,
            total: 8,
            used: 0,
            hint: 0,
        };
        allocator.mark_used(start, bytes);

        let i: usize = kani::any();
        kani::assume(i < 8);
        let frame_start = u128::from(base) + i as u128 * u128::from(FRAME_BYTES);
        let frame_end = frame_start + u128::from(FRAME_BYTES);
        let end = u128::from(start) + u128::from(bytes);
        let touched = bytes > 0 && frame_start < end && frame_end > u128::from(start);
        assert_eq!(allocator.is_index_used(i), touched);
    }

    /// **The `used` counter is the number of set bits, after any one operation.** From any bitmap
    /// whose counter agrees, `mark_free`, `mark_used`, `alloc`, `alloc_contiguous` or `free` of a
    /// used frame leaves the counter equal to the popcount, which is what `Stats::free_frames` reports.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.the_used_count_is_the_number_of_set_bits.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn the_used_count_is_the_number_of_set_bits() {
        let mut bitmap: [u8; 1] = kani::any();
        let used = bitmap[0].count_ones() as usize;
        let mut allocator = PageFrameAllocator {
            bitmap: &mut bitmap,
            base: 0,
            total: 8,
            used,
            hint: kani::any(),
        };
        kani::assume(allocator.hint <= 8);
        match kani::any::<u8>() % 5 {
            0 => allocator.mark_free(kani::any(), kani::any()),
            1 => allocator.mark_used(kani::any(), kani::any()),
            2 => {
                let _ = allocator.alloc();
            }
            3 => {
                let _ = allocator.alloc_contiguous(kani::any());
            }
            _ => {
                let i: usize = kani::any();
                kani::assume(i < 8 && allocator.is_index_used(i));
                allocator.free(PageFrame(i as u64 * FRAME_BYTES));
            }
        }
        let set_bits: usize = allocator.bitmap[0].count_ones() as usize;
        assert_eq!(allocator.used, set_bits);
    }

    /// **A successful `alloc_contiguous` returns a run that was all free and is now all used.**
    /// For every bitmap and every count, the `count` frames from the one returned were each free
    /// before the call and are each used after it.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.a_contiguous_run_was_free_and_is_now_used.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn a_contiguous_run_was_free_and_is_now_used() {
        let mut bitmap: [u8; 1] = kani::any();
        let before = bitmap[0];
        let mut allocator = PageFrameAllocator {
            bitmap: &mut bitmap,
            base: 0,
            total: 8,
            used: 0,
            hint: 0,
        };
        let count: usize = kani::any();
        if let Some(first) = allocator.alloc_contiguous(count) {
            let first = (first.physical_address() / FRAME_BYTES) as usize;
            assert!(count >= 1 && first + count <= 8);
            for j in first..first + count {
                assert!(before & (1 << j) == 0, "a frame of the run was not free");
                assert!(
                    allocator.is_index_used(j),
                    "a frame of the run was left free"
                );
            }
        }
    }
}
