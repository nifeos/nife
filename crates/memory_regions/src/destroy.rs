//! **What destroying a region does.**
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// What [`destroy`](destroy_outcome) decides for a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestroyOutcome {
    /// A live object pins the region, or it still has children: reclaiming would dangle or double-free.
    Refused,
    /// A **root** region (created from the frame allocator): free its whole run back to the allocator.
    FreeToGlobal,
    /// A **child** region: return its pages to the parent, never to the allocator. `unbump` is how
    /// many pages to give back to the parent's watermark: the child's page count when it sits at the
    /// top (last-in-first-out, the run is re-splittable), or `0` when it does not (a hole, until
    /// [`coalesced_watermark`](crate::coalesced_watermark) reclaims it once nothing above it is held).
    ReturnToParent {
        /// How many pages to give back to the parent's watermark.
        unbump: u64,
    },
}

/// Decide what destroying a region does. `pinned` and `children` gate it; `is_root` distinguishes a
/// created region from a split child; `is_lifo_top` says whether a child sits at the top of the
/// parent's watermark; `child_pages` is the child's size (used only for the last-in-first-out un-bump).
pub fn destroy_outcome(
    pinned: bool,
    children: u32,
    is_root: bool,
    is_lifo_top: bool,
    child_pages: u64,
) -> DestroyOutcome {
    if pinned || children > 0 {
        return DestroyOutcome::Refused;
    }
    if is_root {
        return DestroyOutcome::FreeToGlobal;
    }
    DestroyOutcome::ReturnToParent {
        unbump: if is_lifo_top { child_pages } else { 0 },
    }
}
