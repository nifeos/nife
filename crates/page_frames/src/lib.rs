//! A physical frame allocator: hands out 4 KiB pages of physical memory from a bitmap, one bit per
//! frame. It is the bottom of the memory hierarchy; page tables, the kernel heap, direct memory
//! access (DMA) buffers and user pages all ask it, and nothing is underneath it to ask. Pure logic with no `unsafe`, so its
//! tests run on the host (DECISIONS §7 (testing: QEMU harness and host-testable crates)).
//! notes/physical-memory.md has why a bitmap and not a free list (contiguous runs for DMA, and host
//! testability), and the bootstrap order.
//!
//! Name: ratified 2026-08-23 (calef, a kernel-dependency crate naming review). Renamed from
//! `frames`, which collided with `crates/compositor`'s own use of "frame" for a rendered screen
//! update, the collision DECISIONS §113 (eleven kernel object and identifier names move to the plain, standard term) had already found for the kernel's capability object, which
//! is named `PageFrame` too. This crate is the physical-page allocator that backs those page
//! frames.

#![no_std]
#![forbid(unsafe_code)]

// The casts between `u64` and `usize` below (a frame count, a bitmap index) are exact only where
// `usize` holds 64 bits; a narrower target would truncate them silently.
const _: () = assert!(usize::BITS >= 64, "page_frames needs a 64-bit usize");

/// 4 KiB. The unit the memory management unit (MMU) translates in, so it is the unit we
/// allocate in.
pub const FRAME_SIZE: u64 = 4096;

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
/// use page_frames::{PageFrame, FRAME_SIZE};
///
/// // `containing` rounds down to the frame an arbitrary address falls in.
/// assert_eq!(PageFrame::containing(0x4000_0FFF).addr(), 0x4000_0000);
/// assert_eq!(PageFrame::containing(0x4000_1000).addr(), 0x4000_1000);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageFrame(u64);

impl PageFrame {
    /// Name the frame at `addr`.
    ///
    /// Naming a frame is not the same as owning it. This type carries one invariant
    /// (the address is frame-aligned) and no claim about who the frame belongs to.
    /// Ownership lives entirely in the allocator's bitmap, which is where it can
    /// actually be checked.
    ///
    /// # Panics
    /// If `addr` is not frame-aligned.
    #[must_use]
    pub const fn from_addr(addr: u64) -> Self {
        assert!(
            addr.is_multiple_of(FRAME_SIZE),
            "address is not frame-aligned"
        );
        PageFrame(addr)
    }

    /// The frame at `physical_address`, or `None` when it is not frame-aligned: the fallible form
    /// of [`from_addr`](Self::from_addr), for a caller that holds an address it has not checked.
    ///
    /// Name: provisional, minted 2026-11-14 (UTC) by the outside-review fixes lane.
    #[must_use]
    pub const fn try_aligned(physical_address: u64) -> Option<Self> {
        if physical_address.is_multiple_of(FRAME_SIZE) {
            Some(PageFrame(physical_address))
        } else {
            None
        }
    }

    /// The frame containing `addr`, rounding down.
    #[must_use]
    pub const fn containing(addr: u64) -> Self {
        PageFrame(addr - addr % FRAME_SIZE)
    }

    /// The physical address of this frame's first byte.
    #[must_use]
    pub const fn addr(self) -> u64 {
        self.0
    }
}

/// A snapshot of an allocator's occupancy, from [`PageFrameAllocator::stats`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// Frames the allocator tracks in total.
    pub total: usize,
    /// Frames currently marked used.
    pub used: usize,
}

impl Stats {
    /// Frames currently available: `total - used`.
    #[must_use]
    pub fn free(&self) -> usize {
        self.total - self.used
    }
}

/// One bit per frame. `1` means used.
///
/// **Everything starts used**: a frame is handed out only after someone has said its range is real
/// RAM, so an unknown address (a memory-mapped input and output (MMIO) hole) is never allocated. notes/physical-memory.md, "Guilty
/// until proven innocent".
///
/// # Examples
///
/// The wiring an early boot does: name the span, hand it a bitmap, then declare what is real RAM
/// and what inside it is already spoken for.
///
/// ```
/// use page_frames::{PageFrameAllocator, PageFrame, FRAME_SIZE};
///
/// const BASE: u64 = 0x4000_0000;
/// const TOTAL: usize = 64;                      // 64 frames of address space
/// let mut bitmap = [0u8; PageFrameAllocator::bitmap_bytes(TOTAL)];
/// let mut fa = PageFrameAllocator::new(BASE, TOTAL, &mut bitmap);
///
/// // Nothing is free until someone says so: an allocator starts owning nothing.
/// assert_eq!(fa.stats().free(), 0);
///
/// fa.mark_free(BASE, TOTAL as u64 * FRAME_SIZE);        // all of it is real RAM
/// fa.mark_used(BASE, 4 * FRAME_SIZE);                   // ... but the first four hold the kernel
/// assert_eq!(fa.stats().free(), TOTAL - 4);
///
/// let f = fa.alloc().expect("60 frames are free");
/// assert!(f.addr() >= BASE + 4 * FRAME_SIZE, "never hands back the kernel's own frames");
/// assert_eq!(fa.stats().free(), TOTAL - 5);
///
/// fa.free(f);
/// assert_eq!(fa.stats().free(), TOTAL - 4);
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

    /// How many **whole** frames fit in `span` bytes, truncating a partial frame at the end. For
    /// the frames a span must reach, partial ones included, use
    /// [`page_frames_covering`](Self::page_frames_covering).
    #[must_use]
    pub const fn page_frames_in(span: u64) -> usize {
        (span / FRAME_SIZE) as usize
    }

    /// How many frames `span` bytes touch, rounding a partial frame at the end up: the count a
    /// bitmap must track to reach the end of the span.
    ///
    /// Name: provisional, minted 2026-11-14 (UTC) by the outside-review fixes lane.
    #[must_use]
    pub const fn page_frames_covering(span: u64) -> usize {
        span.div_ceil(FRAME_SIZE) as usize
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
            base.is_multiple_of(FRAME_SIZE),
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
            total: self.total,
            used: self.used,
        }
    }

    /// Mark every frame **wholly inside** `[start, start+size)` as free.
    ///
    /// The start rounds **up** and the end **down**, the opposite of
    /// [`mark_used`](Self::mark_used): a frame only partly inside the region also holds bytes the
    /// caller never said were RAM, so it stays used. A region that holds no whole frame frees
    /// nothing. Under-freeing wastes at most 4 KiB at each end of a region. Frames outside what
    /// the allocator manages are ignored. notes/physical-memory.md, "The rounding asymmetry, which
    /// is the whole ballgame".
    pub fn mark_free(&mut self, start: u64, size: u64) {
        for i in self.whole_frames_in(start, size) {
            if self.get(i) {
                self.set(i, false);
                self.used -= 1;
            }
        }
        self.hint = 0;
    }

    /// Mark every frame **touched by** `[start, start+size)` as used.
    ///
    /// The start rounds **down** and the end **up**, the opposite of
    /// [`mark_free`](Self::mark_free): a region ending mid-frame (a kernel image ending at
    /// `0x4009_7010`) still owns that frame, and rounding down would hand out the tail of the
    /// kernel. Over-reserving wastes at most 4 KiB at each end of a region. Frames outside what the
    /// allocator manages are ignored. notes/physical-memory.md, "The rounding asymmetry, which is
    /// the whole ballgame".
    pub fn mark_used(&mut self, start: u64, size: u64) {
        for i in self.page_frame_range(start, size) {
            if !self.get(i) {
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
            .find(|&i| !self.get(i))?;

        self.set(i, true);
        self.used += 1;
        self.hint = i + 1;
        Some(PageFrame(self.base + i as u64 * FRAME_SIZE))
    }

    /// Allocate `count` **physically contiguous** frames, and return the first.
    ///
    /// This is the reason we are a bitmap and not a free list. A device doing DMA reads
    /// physical addresses directly, with no MMU in the way to paper over a scattered
    /// buffer, so a virtio ring at milestone 8 needs its frames genuinely adjacent.
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
            if self.get(i) {
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
                return Some(PageFrame(self.base + run_start as u64 * FRAME_SIZE));
            }
        }

        None
    }

    /// **The longest run of free frames, in frames.** The number
    /// [`alloc_contiguous`](Self::alloc_contiguous) can still satisfy, which is not the same
    /// question as [`Stats::free`] and is the one that actually fails a boot: milestone 107 found
    /// 137 frames free and no run of 128, and the failure surfaced three tests away as
    /// "no swish program in the initial ramdisk".
    ///
    /// Same O(total) scan as `alloc_contiguous` and for the same reason: it is an instrument, called
    /// between tests, never in a hot path.
    #[must_use]
    pub fn largest_free_run(&self) -> usize {
        let mut best = 0;
        let mut run = 0;
        for i in 0..self.total {
            if self.get(i) {
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
        let i = self.index_of(frame).expect("freeing a frame we don't own");
        assert!(self.get(i), "double free of frame {:#x}", frame.addr());

        self.set(i, false);
        self.used -= 1;
        self.hint = self.hint.min(i);
    }

    /// Is this frame currently allocated? Test-support, and useful in assertions.
    #[must_use]
    pub fn is_used(&self, frame: PageFrame) -> Option<bool> {
        Some(self.get(self.index_of(frame)?))
    }

    fn index_of(&self, frame: PageFrame) -> Option<usize> {
        let addr = frame.addr();
        if addr < self.base {
            return None;
        }
        let i = ((addr - self.base) / FRAME_SIZE) as usize;
        (i < self.total).then_some(i)
    }

    /// Every frame index wholly inside `[start, start+size)`, clamped to what we manage.
    ///
    /// Start rounds **up**, end rounds **down**, and a range holding no whole frame is empty. See
    /// [`mark_free`](Self::mark_free). `base` is frame-aligned, so rounding an offset from it
    /// rounds the address.
    fn whole_frames_in(&self, start: u64, size: u64) -> core::ops::Range<usize> {
        let end = start.saturating_add(size);
        if end <= self.base {
            return 0..0;
        }

        let first = start.max(self.base) - self.base;
        let last = end - self.base;

        let lo = first.div_ceil(FRAME_SIZE);
        let hi = (last / FRAME_SIZE).min(self.total as u64);

        if lo >= hi {
            0..0
        } else {
            lo as usize..hi as usize
        }
    }

    /// Every frame index touched by `[start, start+size)`, clamped to what we manage.
    ///
    /// Start rounds **down**, end rounds **up**. See [`mark_used`](Self::mark_used).
    fn page_frame_range(&self, start: u64, size: u64) -> core::ops::Range<usize> {
        if size == 0 || start.saturating_add(size) <= self.base {
            return 0..0;
        }

        let first = start.max(self.base) - self.base;
        let last = (start.saturating_add(size)).saturating_sub(self.base);

        let lo = (first / FRAME_SIZE) as usize;
        let hi = (last.div_ceil(FRAME_SIZE) as usize).min(self.total);

        if lo >= hi { 0..0 } else { lo..hi }
    }

    fn get(&self, i: usize) -> bool {
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

/// Machine-checked proofs of the frame allocator (DECISIONS §14, milestone 18).
///
/// The allocator is the bottom of the memory hierarchy, so its correctness is the floor everything
/// above it stands on. The flagship property is the one address-space isolation ultimately rests on:
/// **`alloc` never hands the same physical frame out twice.** Because these harnesses live inside the
/// crate, they build a small allocator over a *symbolic* bitmap directly, rather than through `new`
/// (which fills the bitmap all-used). See notes/verification.md.
#[cfg(kani)]
mod verification {
    use super::*;

    /// **`containing` rounds down to an aligned frame that holds the address.** For every address,
    /// the frame it names is aligned, sits at or below the address, and the address is within the
    /// 4 KiB it covers.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.containing_rounds_down_within_a_frame.patch`
    #[kani::proof]
    fn containing_rounds_down_within_a_frame() {
        let addr: u64 = kani::any();
        let f = PageFrame::containing(addr);
        assert!(f.addr().is_multiple_of(FRAME_SIZE));
        assert!(f.addr() <= addr);
        assert!(addr - f.addr() < FRAME_SIZE);
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

    /// **`PageFrame` address and bitmap index are inverses.** A frame at `base + i * FRAME_SIZE`, for any
    /// in-range `i`, maps back to index `i`. This is what makes the allocator's naming unambiguous:
    /// no two indices share an address and no address is misfiled.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.index_of_inverts_frame_addressing.patch`
    #[kani::proof]
    fn index_of_inverts_frame_addressing() {
        let base: u64 = kani::any();
        let total: usize = kani::any();
        kani::assume(base.is_multiple_of(FRAME_SIZE));
        kani::assume(total <= 1 << 20);
        kani::assume(base.checked_add(total as u64 * FRAME_SIZE).is_some());
        let i: usize = kani::any();
        kani::assume(i < total);

        let mut bm = [0u8; 1]; // index_of reads only base and total, never the bitmap
        let fa = PageFrameAllocator {
            bitmap: &mut bm,
            base,
            total,
            used: 0,
            hint: 0,
        };
        let frame = PageFrame(base + i as u64 * FRAME_SIZE);
        assert_eq!(fa.index_of(frame), Some(i));
    }

    /// **Two allocations are never the same frame.** Over any bitmap state, if two back-to-back
    /// `alloc`s both succeed they return distinct physical frames. This is the property the whole
    /// isolation story rests on: the allocator cannot hand one physical page to two owners.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.two_allocations_are_distinct.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn two_allocations_are_distinct() {
        let mut bm: [u8; 1] = kani::any(); // eight frames, arbitrary used/free pattern
        let mut fa = PageFrameAllocator {
            bitmap: &mut bm,
            base: 0,
            total: 8,
            used: 0,
            hint: 0,
        };
        let a = fa.alloc();
        let b = fa.alloc();
        if let (Some(fa), Some(fb)) = (a, b) {
            assert_ne!(fa.addr(), fb.addr());
        }
    }

    /// **`mark_free` frees only frames wholly inside the range.** From an all-used bitmap, over any
    /// aligned base and any `start` and `size`, every frame `mark_free` leaves free lies entirely
    /// within `[start, start + size)`. A frame only partly inside a usable region also holds bytes
    /// nobody said were RAM, and handing it out would put a page table or a heap block over them.
    /// The end is compared in `u128`, so a range that wraps the address space is held to its true
    /// end and not a saturated one.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.mark_free_frees_only_whole_frames_inside_the_range.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn mark_free_frees_only_whole_frames_inside_the_range() {
        let base: u64 = kani::any();
        kani::assume(base.is_multiple_of(FRAME_SIZE));
        kani::assume(base.checked_add(8 * FRAME_SIZE).is_some());
        let start: u64 = kani::any();
        let size: u64 = kani::any();

        let mut bitmap = [0xffu8; 1]; // eight frames, all used, as `new` leaves them
        let mut fa = PageFrameAllocator {
            bitmap: &mut bitmap,
            base,
            total: 8,
            used: 8,
            hint: 0,
        };
        fa.mark_free(start, size);

        let i: usize = kani::any();
        kani::assume(i < 8);
        if !fa.get(i) {
            let frame_start = u128::from(base) + i as u128 * u128::from(FRAME_SIZE);
            let frame_end = frame_start + u128::from(FRAME_SIZE);
            assert!(frame_start >= u128::from(start));
            assert!(frame_end <= u128::from(start) + u128::from(size));
        }
    }

    /// **An allocated frame is aligned and in range.** Whatever the bitmap holds, a frame `alloc`
    /// returns is frame-aligned and lies within `[base, base + total * FRAME_SIZE)`.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.an_allocated_frame_is_aligned_and_in_range.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn an_allocated_frame_is_aligned_and_in_range() {
        let mut bm: [u8; 1] = kani::any();
        let base = 0x4000_0000u64; // an arbitrary aligned base
        let mut fa = PageFrameAllocator {
            bitmap: &mut bm,
            base,
            total: 8,
            used: 0,
            hint: 0,
        };
        if let Some(f) = fa.alloc() {
            assert!(f.addr().is_multiple_of(FRAME_SIZE));
            assert!(f.addr() >= base);
            assert!(f.addr() < base + 8 * FRAME_SIZE);
        }
    }

    /// **`mark_used` claims exactly the frames the range touches.** From an all-free bitmap, over
    /// any aligned base and any `start` and `size`, a frame is used afterwards when and only when
    /// it overlaps `[start, start + size)`: a region that ends mid-frame still owns that frame, and
    /// no frame it does not touch is claimed. The end is compared in `u128`, so a range that wraps
    /// the address space is held to its true end.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.mark_used_claims_exactly_the_frames_the_range_touches.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn mark_used_claims_exactly_the_frames_the_range_touches() {
        let base: u64 = kani::any();
        kani::assume(base.is_multiple_of(FRAME_SIZE));
        kani::assume(base.checked_add(8 * FRAME_SIZE).is_some());
        let start: u64 = kani::any();
        let size: u64 = kani::any();

        let mut bitmap = [0u8; 1]; // eight frames, all free
        let mut fa = PageFrameAllocator {
            bitmap: &mut bitmap,
            base,
            total: 8,
            used: 0,
            hint: 0,
        };
        fa.mark_used(start, size);

        let i: usize = kani::any();
        kani::assume(i < 8);
        let frame_start = u128::from(base) + i as u128 * u128::from(FRAME_SIZE);
        let frame_end = frame_start + u128::from(FRAME_SIZE);
        let end = u128::from(start) + u128::from(size);
        let touched = size > 0 && frame_start < end && frame_end > u128::from(start);
        assert_eq!(fa.get(i), touched);
    }

    /// **The `used` counter is the number of set bits, after any one operation.** From any bitmap
    /// whose counter agrees, `mark_free`, `mark_used`, `alloc`, `alloc_contiguous` or `free` of a
    /// used frame leaves the counter equal to the popcount, which is what `Stats::free` reports.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.the_used_count_is_the_number_of_set_bits.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn the_used_count_is_the_number_of_set_bits() {
        let mut bm: [u8; 1] = kani::any();
        let used = bm[0].count_ones() as usize;
        let mut fa = PageFrameAllocator {
            bitmap: &mut bm,
            base: 0,
            total: 8,
            used,
            hint: kani::any(),
        };
        kani::assume(fa.hint <= 8);
        match kani::any::<u8>() % 5 {
            0 => fa.mark_free(kani::any(), kani::any()),
            1 => fa.mark_used(kani::any(), kani::any()),
            2 => {
                let _ = fa.alloc();
            }
            3 => {
                let _ = fa.alloc_contiguous(kani::any());
            }
            _ => {
                let i: usize = kani::any();
                kani::assume(i < 8 && fa.get(i));
                fa.free(PageFrame(i as u64 * FRAME_SIZE));
            }
        }
        let set_bits: usize = fa.bitmap[0].count_ones() as usize;
        assert_eq!(fa.used, set_bits);
    }

    /// **A successful `alloc_contiguous` returns a run that was all free and is now all used.**
    /// For every bitmap and every count, the `count` frames from the one returned were each free
    /// before the call and are each used after it.
    /// Falsification: replayable `crates/page_frames/falsifications/verification.a_contiguous_run_was_free_and_is_now_used.patch`
    #[kani::proof]
    #[kani::unwind(9)]
    fn a_contiguous_run_was_free_and_is_now_used() {
        let mut bm: [u8; 1] = kani::any();
        let before = bm[0];
        let mut fa = PageFrameAllocator {
            bitmap: &mut bm,
            base: 0,
            total: 8,
            used: 0,
            hint: 0,
        };
        let count: usize = kani::any();
        if let Some(first) = fa.alloc_contiguous(count) {
            let first = (first.addr() / FRAME_SIZE) as usize;
            assert!(count >= 1 && first + count <= 8);
            for j in first..first + count {
                assert!(before & (1 << j) == 0, "a frame of the run was not free");
                assert!(fa.get(j), "a frame of the run was left free");
            }
        }
    }
}
