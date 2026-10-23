//! **Page sizes**: the 4 KiB base page and the block sizes a leaf may map.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// 4 KiB, the smallest leaf every format here maps, and the unit every table is.
pub const PAGE_SIZE: u64 = 4096;

/// **How much one leaf maps**: a page at the bottom level, or a block one or two levels up.
///
/// The sizes are the same on all three CPU formats because they all use 4 KiB granules and 9-bit
/// indices: each level up multiplies the span by 512. The names are the byte counts rather than
/// "huge"/"giant"/"super", because those words mean different sizes in different kernels and the
/// number does not. Name provisional (milestone 161); the `x86_64` crate spells the same three
/// sizes the same way, which is the prior art.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PageSize {
    /// A 4 KiB page, at the bottom level.
    Size4KiB,
    /// A 2 MiB block, one level up.
    Size2MiB,
    /// A 1 GiB block, two levels up.
    Size1GiB,
}

impl PageSize {
    /// Every size, smallest first.
    pub const ALL: [PageSize; 3] = [PageSize::Size4KiB, PageSize::Size2MiB, PageSize::Size1GiB];

    /// How many bytes one leaf of this size maps.
    pub const fn bytes(self) -> u64 {
        PAGE_SIZE << (9 * self.levels_above_bottom())
    }

    /// How many levels above the bottom a leaf of this size sits: 0, 1 or 2.
    pub const fn levels_above_bottom(self) -> usize {
        match self {
            PageSize::Size4KiB => 0,
            PageSize::Size2MiB => 1,
            PageSize::Size1GiB => 2,
        }
    }

    /// **The leaf size for the next step of mapping `remaining` bytes from `va` to `pa`**: the
    /// largest no bigger than `largest` with both addresses aligned and the leaf inside the rest.
    ///
    /// The decision that makes a block mapping safe, kept out of
    /// [`Mapper::map_span`](crate::Mapper::map_span) so Kani proves it for every input: a block
    /// that overran the span would map bytes nobody asked for, such as a writable alias of the
    /// kernel image (`a_chosen_leaf_is_aligned_and_inside_the_span`,
    /// `the_chosen_leaf_is_the_largest_that_fits`).
    ///
    /// Answers [`PageSize::Size4KiB`] when nothing larger fits, including when `remaining` is less
    /// than a page; the caller is the one that knows a span must be page-aligned and whole.
    pub const fn largest_fitting(va: u64, pa: u64, remaining: u64, largest: PageSize) -> PageSize {
        // Written out rather than looped over `ALL`, so the proof below is over straight-line
        // arithmetic. Every size is a power of two, so alignment is a mask test, not a division.
        const fn fits(va: u64, pa: u64, remaining: u64, size: PageSize) -> bool {
            let bytes = size.bytes();
            (va | pa) & (bytes - 1) == 0 && remaining >= bytes
        }
        let allowed = largest.levels_above_bottom();
        if allowed >= 2 && fits(va, pa, remaining, PageSize::Size1GiB) {
            PageSize::Size1GiB
        } else if allowed >= 1 && fits(va, pa, remaining, PageSize::Size2MiB) {
            PageSize::Size2MiB
        } else {
            PageSize::Size4KiB
        }
    }
}
