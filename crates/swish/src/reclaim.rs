//! **Giving a region back, retrying the way a refusal that passes calls for**: [`is_reclaimed_within`].
//!
//! The kernel refuses a `DESTROY` of a region while a thread built in it is still live, and arms
//! that thread's kill. The refusal passes within a timeslice or two. A caller that asks once and
//! deletes its slot loses the region's pages for the boot, so every place the shell gives a
//! region back to its budget asks through here.
//!
//! Name: provisional, minted 2026-10-31 (UTC) by lane/mem-budget-retry.

/// Ask `destroy` until it answers zero or `patience_ns` of `now` has passed, calling `wait`
/// between asks. Returns whether it answered zero. Always asks at least once.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane/mem-budget-retry.
///
/// The bound is in time and not in attempts, because a wait that returns at once when nothing else
/// can run would spend every attempt inside the one timeslice the kill still needs.
pub fn is_reclaimed_within(
    patience_ns: u64,
    mut destroy: impl FnMut() -> i64,
    mut now: impl FnMut() -> u64,
    mut wait: impl FnMut(),
) -> bool {
    let start = now();
    loop {
        if destroy() == 0 {
            return true;
        }
        if now().wrapping_sub(start) > patience_ns {
            return false;
        }
        wait();
    }
}

#[cfg(test)]
mod tests;
