//! The table's Kani-only constructors and observers, for the subtree-ceiling harnesses in
//! `proofs.rs`. A child of `table` so they reach its private fields, and a file of its own so
//! the host mutation census leaves them out: no build but Kani's compiles them, and their checker
//! is `script/verify`. See `.cargo/mutants.toml`.
//!
//! Name: provisional, minted 2026-11-11 (UTC) by lane/region-subtree-ceilings; `verification` is
//! the tree's name for a Kani module.

use generational_table::EntryName;

use super::{COUNTED_KINDS, Ceilings, Region, RegionTable, Spent, entry};

/// What the subtree-ceiling harnesses in `proofs.rs` compare before and after a refusal.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct RegionSnapshot {
    pub(crate) watermark: u64,
    pub(crate) pinned: bool,
    pub(crate) children: u32,
    pub(crate) live: Ceilings,
    pub(crate) ceilings: Ceilings,
}

impl<const N: usize> RegionTable<N> {
    /// Every field a charge, a split or a carve could move, for a live name.
    pub(crate) fn snapshot(&self, name: u64) -> Option<RegionSnapshot> {
        self.table.get(entry(name)).map(|r| RegionSnapshot {
            watermark: r.watermark,
            pinned: r.pinned,
            children: r.children,
            live: r.live,
            ceilings: r.ceilings,
        })
    }

    /// Insert a region in whatever state the subtree-ceiling harnesses in `proofs.rs` choose, so
    /// one step can start from any state their invariant allows instead of one a short run of
    /// steps reaches. The region holds no spending record and is pinned as `pinned` says.
    pub(crate) fn insert_in_any_state(
        &mut self,
        parent: u64,
        (base_page, pages, watermark, retyped_to): (u64, u64, u64, u64),
        pinned: bool,
        children: u32,
        ceilings: Ceilings,
        live: Ceilings,
    ) -> Option<u64> {
        self.table
            .insert_with(|_| Region {
                base_page,
                pages,
                watermark,
                retyped_to,
                pinned,
                parent,
                children,
                spent: Spent::default(),
                ceilings,
                live,
            })
            .map(EntryName::to_bits)
    }
}

/// The per-kind counts behind [`Ceilings`], for the harnesses in `proofs.rs` that build a state.
impl Ceilings {
    pub(crate) const fn from_counts(counts: [u32; COUNTED_KINDS]) -> Ceilings {
        Ceilings(counts)
    }
}
