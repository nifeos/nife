//! **A region's watermark**: carving pages off a parent, retyping a run of them, and reclaiming
//! the holes a returned child leaves.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// The result of carving `want` pages off a parent whose budget is `parent_pages` and whose spent
/// watermark is `parent_watermark`. `Some(new_watermark)` on success (the caller carves the run
/// `[parent_watermark, new_watermark)` and sets the parent's watermark to `new_watermark`); `None`
/// if the carve is empty, would overflow, or exceeds the parent's remaining budget.
///
/// # Examples
///
/// ```
/// use memory_regions::split_new_watermark;
///
/// // A 100-page parent that has spent 40: carving 10 more moves the watermark to 50.
/// assert_eq!(split_new_watermark(100, 40, 10), Some(50));
/// // Exactly exhausting the budget is fine; one page beyond it is not.
/// assert_eq!(split_new_watermark(100, 90, 10), Some(100));
/// assert_eq!(split_new_watermark(100, 90, 11), None);
/// // An empty carve would mint a zero-page region, and a wrap would forge budget.
/// assert_eq!(split_new_watermark(100, 40, 0), None);
/// assert_eq!(split_new_watermark(100, 40, u64::MAX), None);
/// ```
pub fn split_new_watermark(parent_pages: u64, parent_watermark: u64, want: u64) -> Option<u64> {
    if want == 0 {
        return None; // an empty carve makes no child; refuse rather than mint a zero-page region
    }
    let new = parent_watermark.checked_add(want)?; // no overflow: a wrap would forge budget
    if new > parent_pages {
        return None; // beyond the parent's remaining budget
    }
    Some(new)
}

/// **How many pages a `RETYPE` asks for, from the word the caller passed.** `0` means one, which is
/// what every caller passed before the count existed (the first argument was ignored), so the
/// ruling that added a count (calef, 2026-09-26) changed nothing for them. Anything else is the
/// count itself.
pub const fn retype_pages(requested: u64) -> u64 {
    if requested == 0 { 1 } else { requested }
}

/// **The watermark after retyping a run of `requested` pages**, or `None` when it does not fit.
///
/// The same arithmetic a split is, over [`retype_pages`]: a run is carved off the region's own
/// budget exactly as a child is carved off a parent's, and it has to be fresh, contiguous, and
/// within budget for the same reason. What differs is only that `0` means one here rather than
/// being refused, because a retype of zero pages has always meant "the one page".
pub fn retype_new_watermark(pages: u64, watermark: u64, requested: u64) -> Option<u64> {
    split_new_watermark(pages, watermark, retype_pages(requested))
}

/// **The parent's watermark after a child's return, holes reclaimed** (2026-10-03, UTC; name
/// provisional). `watermark` is the parent's after the last-in-first-out un-bump, `retyped_to` how far its own
/// retypes reached, `live_to` where its highest live child ends (`0` for none), and `quiescent`
/// whether every child the parent counts is live, so none is claimed and still being revoked.
///
/// Quiescent, the watermark drops to the higher of the two floors, because every page above both
/// was a child's and that child is gone. Otherwise it stays: an in-flight sibling's pages sit
/// somewhere under the watermark, nothing here knows where, and carving them again before their
/// revoke finishes is the use-after-free. Never above `watermark`, so a floor that somehow
/// exceeded it cannot hand out budget the parent never had.
pub fn coalesced_watermark(watermark: u64, retyped_to: u64, live_to: u64, quiescent: bool) -> u64 {
    if !quiescent {
        return watermark;
    }
    retyped_to.max(live_to).min(watermark)
}
