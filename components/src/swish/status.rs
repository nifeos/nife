//! **What a command did**: the status of the one now running and of the one before it, which is
//! what `$?` reports and what `&&` reads.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// **What the command now running has done so far** (milestone 67), as a [`Status`] code.
///
/// A global for the reason [`DIAG_EP`] is one: the printers that set it are free functions reached
/// from every command path. `xargs` reads it to stop a sweep. Single-threaded EL0, so `Relaxed` is
/// the whole ordering story.
pub(super) static CURRENT: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// **What the command before this one did**, which is what `$?` reports and what `&&` reads.
///
/// Two cells rather than one, because a segment has to be able to read the previous segment's
/// answer *while* accumulating its own: `false || echo $?` prints the status of the thing that
/// failed, which is exactly the case one cell could not serve.
pub(super) static LAST: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Record what just happened, **first one wins**: the first thing that went wrong is what stopped
/// the line, and a later printer describing a consequence should not overwrite the cause.
pub(super) fn record(s: Status) {
    if CURRENT.load(core::sync::atomic::Ordering::Relaxed) == Status::Ran.code() {
        CURRENT.store(s.code(), core::sync::atomic::Ordering::Relaxed);
    }
}

/// **This shell declined to run it**, decided from what this shell holds with nothing spawned.
pub(super) fn refused() {
    record(Status::Refused);
}

/// **Something was attempted and did not work**: a filesystem errno, a spawn the progenitor could not back,
/// a job torn down. See [`Status`] for why this is a different number from [`refused`].
pub(super) fn failed() {
    record(Status::Failed);
}

/// What the command now running has done so far.
pub(super) fn current() -> Status {
    Status::from_code(CURRENT.load(core::sync::atomic::Ordering::Relaxed))
}

/// What the command before this one did.
pub(super) fn last() -> Status {
    Status::from_code(LAST.load(core::sync::atomic::Ordering::Relaxed))
}
