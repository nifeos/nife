/// The `next_cursor` that means the survey is complete. It is also the cursor to start one
/// with, which is not a collision: a caller passes it in once and stops when it comes back.
pub const DONE: u64 = 0;

/// On a run queue, waiting for a CPU.
pub const READY: u64 = 1;
/// On a CPU right now. On a multi-core machine this may include the surveyor's own thread, if
/// the surveyor is somehow its own supervisor's child.
pub const RUNNING: u64 = 2;
/// Blocked in an IPC rendezvous that has not happened.
pub const BLOCKED: u64 = 3;
/// A corpse (DECISIONS §26): it faulted or exited, its supervisor was told, and it persists
/// until [`rendezvous::REAP`](super::rendezvous::REAP) collects it. **This is the state `ps` exists to
/// make visible**, and
/// it is the one Unix cannot show you without a parent that happens to have called `wait`.
pub const DEAD: u64 = 4;

/// **Which per-thread record a [`SURVEY`](super::rendezvous::SURVEY) asks for.** The selector
/// calef ruled on 2026-09-21: a new per-thread fact is a new value here, never a new return
/// register.
///
/// The value rides in x1, the argument `SURVEY` never used, and lands in x2 of the answer. x0
/// (the cursor) and x1 (the tid) are the same for every record, so the walk a caller writes is
/// the same walk whichever record it wants.
///
/// Names provisional: calef names public items.
pub mod record {
    /// **What run state the thread is in**: one of this module's parent's state codes. The
    /// record `SURVEY` answered before selectors existed, and **0 so that it still does**.
    /// Every caller written against the old three-word contract passed 0 into the then-unused
    /// x1, so it selects this record by accident and keeps working on purpose.
    pub const STATE: u64 = 0;

    /// **Which core the thread was placed on when it started**: a cpu id, or [`NO_CPU`].
    ///
    /// **Placement, not where it runs now**: written once, at start, so it costs the IPC path
    /// nothing, and a thread stolen onto another core or woken onto its waker's still reports where
    /// it was placed. **The id is a name, not an index**: the online set need not be `0..n` (the
    /// VisionFive 2's is `{1, 2, 3}`), so key a per-core tally by the ids this returns and never
    /// iterate a range. The machine's online set is not available here; see
    /// notes/abi/method-contracts.md.
    pub const PLACEMENT: u64 = 1;

    /// The [`PLACEMENT`] answer for a thread with no placement to report. `u64::MAX` rather
    /// than a plausible small number, so a reader that ignores it is wrong loudly rather than
    /// quietly tallying core 255.
    ///
    /// Every thread a survey can report has started, so this should not be reachable: a
    /// supervision endpoint is recorded at `START` (DECISIONS §26 (the fault endpoint: thread death becomes a
    /// message a supervisor holds)), so an embryo is not in its
    /// domain yet, and a corpse keeps the placement it was started with. It is defined anyway
    /// rather than left to a guess, which is the same posture `sched::last_cpus` takes with its
    /// own `u8::MAX`.
    pub const NO_CPU: u64 = u64::MAX;

    /// **How long the thread has been scheduled on a CPU, in milliseconds**, as Linux's `utime`/
    /// `stime` and Zircon's `total_runtime` report it (§150 (how does a thread's CPU time reach userspace?)).
    ///
    /// - Milliseconds are the wire contract, so no reader learns this kernel's `TICK_HZ`; the
    ///   granularity is still the tick, 10 ms on all three architectures.
    /// - **Sampled at the timer tick**, `jiffies`-style: a thread that runs entirely between two
    ///   ticks is charged nothing, so two figures that should match may not.
    /// - A holder of [`rights::ENUMERATE`](crate::rights::ENUMERATE) learns a continuous signal of
    ///   another thread's workload inside its domain, which §150 accepted (notes/abi/method-contracts.md).
    ///
    /// Name: provisional. calef names public items.
    pub const CPU_TIME: u64 = 2;

    /// **Whether this kernel answers a record.** The one enumeration of the selector space, so
    /// adding a record is an edit here and an arm in the kernel's walk rather than a hunt.
    ///
    /// The caller refuses an unknown record with [`crate::Error::BadMethod`] *before* walking
    /// the domain, which is why this is a predicate rather than a `match` folded into the
    /// extraction: a bad selector must be refused even when the domain is empty and there is no
    /// thread to extract anything from.
    #[must_use]
    pub const fn is_known(record: u64) -> bool {
        matches!(record, STATE | PLACEMENT | CPU_TIME)
    }
}

#[cfg(test)]
mod tests;
