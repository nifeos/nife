//! **Jobs whose region and exit this shell holds** (milestone 933 (every job is built the way a
//! supervised job is), ruled 2026-10-27 UTC): the shell's one exit endpoint, the deaths it has
//! heard but not yet matched to a job, and the collect that tears a job down and tells the
//! progenitor its file-service window is free.
//!
//! A held job is built by the progenitor out of a region this shell splits off its own budget, and
//! is supervised on [`exit_endpoint`], badged with a label the progenitor mints. A job that answers
//! in the foreground answers on the same endpoint through a copy with the same label, so its output
//! and its death arrive in order on one endpoint, and a job that faults before it answers ends the
//! read instead of hanging it. The kernel stamps the label on a death message's sixth word and on
//! nothing else, so a message with a label is a death and any other is the job's own.
//!
//! # BUGS
//!
//! - A foreground job that answers and then does not exit holds the prompt: the shell reads for its
//!   death after its output, and `^C` cannot reach a blocked receive. The cooperative tier (a `std`
//!   program's `SIGINT`) is a question of its own, and milestone 933 leaves it unbuilt; `kill -9
//!   %N` and `^C` under `fg` reach a job that went to the background.
//! - At most [`STASHED`] deaths wait for their job to be asked about, and the table is eight
//!   background jobs and the foreground one; a ninth is not started.
//!
//! Name: provisional (milestone 933's lane, 2026-10-27 UTC).

use core::sync::atomic::Ordering::Relaxed;
use core::sync::atomic::{AtomicBool, AtomicU64};

use super::*;

/// This shell's exit endpoint once it has minted one, else `0`: slot 0 is the terminal in every
/// wiring, so no endpoint is ever there.
static EXIT: AtomicU64 = AtomicU64::new(0);

/// **The endpoint every held job's death arrives on**, retyped out of [`BUDGET`] on first use and
/// kept for the session, as [`screen_rendezvous`] is. `None` when the budget cannot back it.
pub(super) fn exit_endpoint() -> Option<u64> {
    let held = EXIT.load(Relaxed);
    if held != 0 {
        return Some(held);
    }
    let ep = retype_rendezvous(BUDGET)?;
    EXIT.store(ep, Relaxed);
    Some(ep)
}

/// How many deaths wait, heard from the endpoint while this shell was asking after another job.
const STASHED: usize = 16;

/// The label of each stashed death, `0` for an empty entry.
static STASH_LABEL: [AtomicU64; STASHED] = [const { AtomicU64::new(0) }; STASHED];

/// Whether each stashed death was a fault rather than a clean exit.
static STASH_FAULT: [AtomicBool; STASHED] = [const { AtomicBool::new(false) }; STASHED];

/// Keep a death heard while asking after another job. Dropped when the table is full, which the
/// eight-job table above it cannot reach.
fn stash(label: u64, fault: bool) {
    for (l, f) in STASH_LABEL.iter().zip(&STASH_FAULT) {
        if l.load(Relaxed) == 0 {
            f.store(fault, Relaxed);
            l.store(label, Relaxed);
            return;
        }
    }
}

/// Whether `label` has a stashed death, and whether it was a fault.
fn stashed_death(label: u64) -> Option<bool> {
    STASH_LABEL
        .iter()
        .zip(&STASH_FAULT)
        .find(|(l, _)| l.load(Relaxed) == label)
        .map(|(_, f)| f.load(Relaxed))
}

/// Forget `label`'s stashed death, if it has one.
fn unstash(label: u64) {
    for l in &STASH_LABEL {
        if l.load(Relaxed) == label {
            l.store(0, Relaxed);
        }
    }
}

/// How many deaths are stashed.
fn stashed() -> usize {
    STASH_LABEL.iter().filter(|l| l.load(Relaxed) != 0).count()
}

/// What one receive on the exit endpoint heard.
enum Heard {
    /// A job's own message: its three words.
    Message(u64, u64, u64),
    /// A death: the job's label, and whether it was a fault.
    Died(u64, bool),
}

/// Receive once on `exit`, and say which of the two it was. A death carries the label in its sixth
/// word, which the kernel writes on a death alone, so a label is never a job's forgery.
fn hear(exit: u64) -> Heard {
    let (w0, w1, w2, _, _, label) = receive_fault(exit);
    if label != 0 {
        Heard::Died(label, w0 == abi::fault::EVENT_FAULT)
    } else {
        Heard::Message(w0, w1, w2)
    }
}

/// **The next message job `label` sent, or how it died instead**: `Err(true)` for a fault and
/// `Err(false)` for an exit. Other jobs' deaths heard on the way are kept for their own asking.
pub(super) fn read_job(label: u64) -> Result<(u64, u64, u64), bool> {
    if let Some(fault) = stashed_death(label) {
        return Err(fault);
    }
    // No endpoint means no held job was ever started, so this is only a refusal to hang.
    let Some(exit) = exit_endpoint() else {
        return Err(true);
    };
    loop {
        match hear(exit) {
            Heard::Message(a, b, c) => return Ok((a, b, c)),
            Heard::Died(l, fault) if l == label => return Err(fault),
            Heard::Died(l, fault) => stash(l, fault),
        }
    }
}

/// **Block until job `label` has died**, and say whether it faulted. Output it sent after the
/// caller stopped reading is dropped.
pub(super) fn await_death(label: u64) -> bool {
    loop {
        if let Err(fault) = read_job(label) {
            return fault;
        }
    }
}

/// How many threads of this shell's held jobs are dead on the exit endpoint, which is the number of
/// deaths that arrived there, parked or already heard: a corpse stays in the domain until its
/// region is destroyed (`rendezvous::SURVEY`, notes/process-view.md).
fn dead_threads(exit: u64) -> usize {
    let mut cursor = abi::survey::DONE;
    let mut dead = 0;
    // Bounded by the thread table's size in practice; the bound only keeps a misbehaving walk finite.
    for _ in 0..4096 {
        let (next, _tid, state) = user_mode_runtime::survey(exit, cursor);
        if next <= 0 {
            break;
        }
        if state == abi::survey::DEAD {
            dead += 1;
        }
        cursor = next as u64;
    }
    dead
}

/// **Whether job `label` has died, without waiting**: `Some(true)` for a fault and `Some(false)`
/// for an exit. The ABI has no non-blocking receive, so this asks the survey how many threads are
/// dead and hears from the endpoint only when that is more than the deaths already heard: then a
/// death is parked there and the receive returns at once.
pub(super) fn has_died(label: u64) -> Option<bool> {
    let exit = exit_endpoint()?;
    for _ in 0..=STASHED {
        if let Some(fault) = stashed_death(label) {
            return Some(fault);
        }
        if dead_threads(exit) <= stashed() {
            return None;
        }
        if let Heard::Died(l, fault) = hear(exit) {
            stash(l, fault);
        }
    }
    None
}

/// **Tear down a job this shell held, and tell the progenitor it is gone.** `DESTROY` of the region
/// takes the job, its corpse and anything built beside it (a directory grant's caretaker); the
/// progenitor then frees the job's window, resolver grants and installer reply on the release
/// message, which is sent only when the region came back, since a job that is still there could
/// still use them. Returns whether the region came back.
pub(super) fn collect(region: u64, label: u64) -> bool {
    let back = reclaim(region);
    unstash(label);
    cap_delete(region);
    if back {
        let (w0, w1, w2) = spawnproto::release(label);
        send(SPAWN, w0, w1, w2);
    }
    back
}
