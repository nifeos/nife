//! **Giving a region back, retrying the way a refusal that passes calls for**: [`is_reclaimed_within`].
//!
//! The kernel refuses a `DESTROY` of a region while a thread built in it is still live, and arms
//! that thread's kill. The refusal passes within a timeslice or two. A caller that asks once and
//! deletes its slot loses the region's pages for the boot, so every place the shell gives a
//! region back to its budget asks through here.
//!
//! Name: provisional, minted 2026-10-31 (UTC) by lane/mem-budget-retry.

/// How a budget's return ended: see [`reclaim_budget_within`].
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane/933-a-job-may-not-leave-children-in-its-budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetReturn {
    /// The `DESTROY` answered zero: the pages are back.
    Reclaimed,
    /// A refusal while a child region of the budget is live. It never passes, so the retry stopped
    /// at once. The budget's pages stay spent until reboot.
    LeftChildren,
    /// Refused for the whole patience bound with no child region live: a resident that stayed.
    Refused,
}

/// Write the one sentence the shell prints for [`BudgetReturn::LeftChildren`]: the job, and the
/// pages that stay spent until reboot.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane/933-a-job-may-not-leave-children-in-its-budget.
pub fn write_budget_left(job: &[u8], pages: u64, out: &mut dyn FnMut(&[u8])) {
    out(b"  ");
    out(job);
    out(b" left a child region in its --mem budget; its ");
    crate::write_num(pages, out);
    out(b" pages stay spent until reboot.\n");
}

/// Ask `destroy` until it answers zero, as [`is_reclaimed_within`] does, but stop at the first
/// refusal for which `children_pages` (the region's `USAGE` `CHILDREN` record) is above zero.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane/933-a-job-may-not-leave-children-in-its-budget.
///
/// The kernel checks for children before it looks at a resident (`reclaim_region`), so a refusal
/// with children live is always `HasChildren` and the kill is never armed: nothing will change by
/// waiting. A refusal reaches here as one `NotPermitted` whatever its cause, which is why the
/// shell asks `USAGE`. A `children_pages` answer below zero (the record could not be read) counts
/// as no children, so the retry runs as before.
pub fn reclaim_budget_within(
    patience_ns: u64,
    mut destroy: impl FnMut() -> i64,
    mut children_pages: impl FnMut() -> i64,
    mut now: impl FnMut() -> u64,
    mut wait: impl FnMut(),
) -> BudgetReturn {
    let start = now();
    loop {
        if destroy() == 0 {
            return BudgetReturn::Reclaimed;
        }
        if children_pages() > 0 {
            return BudgetReturn::LeftChildren;
        }
        if now().wrapping_sub(start) > patience_ns {
            return BudgetReturn::Refused;
        }
        wait();
    }
}

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
