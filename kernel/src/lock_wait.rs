//! How often the job mix's threads found a kernel lock held, and how long they waited.
//!
//! Built only with `--features lock_wait`. Counts contended acquisitions and their wait per rank,
//! the reaper's stack-free and removal times, and `sched::current_cap`'s calls and waits, per core
//! and only while [`ARMED`]. An uncontended acquisition costs what it always did. What it cannot
//! see is in notes/job-mix/lock-wait-instrument.md.
//!
//! Module, feature and the `job-mix-lock:` line are provisional names, a lane's coinage, per the
//! naming tenet.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::cpu::MAX_CPUS;

/// Lock ranks are below this (`sync::rank` tops out at 61). A rank at or above it is not counted.
const RANKS: usize = 64;

/// True for the duration of a timed subrun. Read on every counted path, written twice a subrun by
/// the supervisor.
static ARMED: AtomicBool = AtomicBool::new(false);

/// One core's counters, on lines of their own so the instrument adds no cross-core traffic.
/// Written only by the owning core; read and reset by the supervisor between subruns.
#[repr(align(64))]
struct Core {
    /// Set by `current_cap` around its one lock acquisition.
    in_current_cap: AtomicBool,
    cap_calls: AtomicU64,
    cap_contended: AtomicU64,
    cap_wait_ticks: AtomicU64,
    reaps: AtomicU64,
    stack_free_ticks: AtomicU64,
    remove_ticks: AtomicU64,
    contended: [AtomicU64; RANKS],
    wait_ticks: [AtomicU64; RANKS],
}

impl Core {
    const fn new() -> Self {
        Self {
            in_current_cap: AtomicBool::new(false),
            cap_calls: AtomicU64::new(0),
            cap_contended: AtomicU64::new(0),
            cap_wait_ticks: AtomicU64::new(0),
            reaps: AtomicU64::new(0),
            stack_free_ticks: AtomicU64::new(0),
            remove_ticks: AtomicU64::new(0),
            contended: [const { AtomicU64::new(0) }; RANKS],
            wait_ticks: [const { AtomicU64::new(0) }; RANKS],
        }
    }
}

static CORES: [Core; MAX_CPUS] = [const { Core::new() }; MAX_CPUS];

fn here() -> &'static Core {
    &CORES[crate::cpu::id() & (MAX_CPUS - 1)]
}

fn bump(counter: &AtomicU64, by: u64) {
    counter.fetch_add(by, Ordering::Relaxed);
}

/// Count one contended acquisition of a lock of `rank` that spun for `ticks`.
///
/// Context: `IrqSafeMutex::lock`, after the contended lock is taken; interrupts masked, that lock
/// held.
pub(crate) fn contended(rank: u32, ticks: u64) {
    if !ARMED.load(Ordering::Relaxed) {
        return;
    }
    let core = here();
    if let (Some(c), Some(w)) = (
        core.contended.get(rank as usize),
        core.wait_ticks.get(rank as usize),
    ) {
        bump(c, 1);
        bump(w, ticks);
    }
    if core.in_current_cap.load(Ordering::Relaxed) {
        bump(&core.cap_contended, 1);
        bump(&core.cap_wait_ticks, ticks);
    }
}

/// Count a `sched::current_cap` call and mark its one acquisition, so a contended wait on this core
/// until [`leave_current_cap`] is charged to it.
pub(crate) fn enter_current_cap() {
    if !ARMED.load(Ordering::Relaxed) {
        return;
    }
    let core = here();
    bump(&core.cap_calls, 1);
    core.in_current_cap.store(true, Ordering::Relaxed);
}

/// End `current_cap`'s mark: it holds its lock now.
///
/// Context: holding the running thread's capability-table lock, when it has a table.
pub(crate) fn leave_current_cap() {
    here().in_current_cap.store(false, Ordering::Relaxed);
}

/// Count one reap whose kernel-stack free took `ticks`.
///
/// Context: `sched::reap_switched_out`, with no lock held.
pub(crate) fn stack_freed(ticks: u64) {
    if !ARMED.load(Ordering::Relaxed) {
        return;
    }
    let core = here();
    bump(&core.reaps, 1);
    bump(&core.stack_free_ticks, ticks);
}

/// Count `ticks` of the reaper's final critical section, removing the thread.
///
/// Context: `sched::reap_switched_out`, holding `IPC_TABLES`.
pub(crate) fn reaped(ticks: u64) {
    if !ARMED.load(Ordering::Relaxed) {
        return;
    }
    bump(&here().remove_ticks, ticks);
}

/// Start counting (the supervisor, just before it releases a subrun's tasks).
pub(crate) fn arm() {
    ARMED.store(true, Ordering::Relaxed);
}

/// Stop counting (the supervisor, once the subrun's last task has reported).
pub(crate) fn disarm() {
    ARMED.store(false, Ordering::Relaxed);
}

/// Zero every core's counters. Between sweep points, with counting disarmed.
pub fn reset() {
    for core in &CORES {
        for c in core.contended.iter().chain(core.wait_ticks.iter()) {
            c.store(0, Ordering::Relaxed);
        }
        core.cap_calls.store(0, Ordering::Relaxed);
        core.cap_contended.store(0, Ordering::Relaxed);
        core.cap_wait_ticks.store(0, Ordering::Relaxed);
        core.reaps.store(0, Ordering::Relaxed);
        core.stack_free_ticks.store(0, Ordering::Relaxed);
        core.remove_ticks.store(0, Ordering::Relaxed);
    }
}

/// Summed over every core: `(reaps, stack_free_ticks, remove_ticks)` for the reaper.
pub(crate) fn reap_totals() -> (u64, u64, u64) {
    CORES.iter().fold((0, 0, 0), |(n, f, r), core| {
        (
            n + core.reaps.load(Ordering::Relaxed),
            f + core.stack_free_ticks.load(Ordering::Relaxed),
            r + core.remove_ticks.load(Ordering::Relaxed),
        )
    })
}

/// Summed over every core: `(contended, wait_ticks)` for one rank.
pub(crate) fn rank_totals(rank: usize) -> (u64, u64) {
    CORES.iter().fold((0, 0), |(c, w), core| {
        (
            c + core
                .contended
                .get(rank)
                .map_or(0, |a| a.load(Ordering::Relaxed)),
            w + core
                .wait_ticks
                .get(rank)
                .map_or(0, |a| a.load(Ordering::Relaxed)),
        )
    })
}

/// Summed over every core: `(calls, contended, wait_ticks)` for `current_cap`.
pub(crate) fn current_cap_totals() -> (u64, u64, u64) {
    CORES.iter().fold((0, 0, 0), |(n, c, w), core| {
        (
            n + core.cap_calls.load(Ordering::Relaxed),
            c + core.cap_contended.load(Ordering::Relaxed),
            w + core.cap_wait_ticks.load(Ordering::Relaxed),
        )
    })
}

/// The name `rank` is printed as, or `None` to print it by number. Rank 59 has none: three locks
/// share it, and a name would claim an attribution the counter cannot make.
pub(crate) fn rank_name(rank: usize) -> Option<&'static str> {
    use crate::sync::rank as r;
    match u32::try_from(rank).ok()? {
        r::IPC_TABLES => Some("ipc_tables"),
        r::CAPABILITY_TABLE => Some("capability_table"),
        r::MEMORY_REGION => Some("memory_region"),
        r::ADDRESS_SPACES => Some("address_spaces"),
        _ => None,
    }
}

/// How many ranks there are to walk when printing.
pub(crate) const fn ranks() -> usize {
    RANKS
}
