//! **A supervised job, interrupted** (DECISIONS §24): the heeder that stops cleanly on one
//! `^C`, and the ignorer the grace timeout escalates to a `DESTROY`.
//!
//! Cut from the table in `script.rs` verbatim by milestone 900 (the swish-check script
//! splits by topic before it crosses the ceiling).
//!
//! Name: provisional (milestone 900's lane, 2026-10-15 UTC).

use super::*;

/// The interrupt rows of [`SWISH_CHECK_SCRIPT`], in table order.
pub(super) const LINES: &[Line] = &[
    // **A supervised job, interrupted**, under DECISIONS §24 (interrupting the foreground
    // process), and these two are the only lines this gate presses `^C` for (see
    // [`SWISH_CHECK_INTERRUPTED`]). Neither was typed here
    // until 2026-09-25, and in that time the first supervised job of every boot failed with `could
    // not map the job frame`, because the shell mapped its job frame over its own terminal page.
    // The heeder is the cooperative tier: one `^C`, and the job reports it stopped. It goes first
    // because it is the first supervised job of the boot, which is the one that failed.
    line(
        0,
        "interrupt_heeder",
        &[
            "^C interrupts it.",
            "the job caught the interrupt and stopped cleanly after",
        ],
    ),
    // The forcible tier: the ignorer never looks at the flag, so the grace timeout escalates one
    // `^C` to a `DESTROY` of the region the shell built it from, which force-kills its live thread
    // (DECISIONS §16 (object revocation), as amended). A shell whose teardown was refused says so
    // instead.
    line(
        0,
        "interrupt_ignorer",
        &["^C again: tearing the job down.", "its memory reclaimed"],
    ),
    // **Job control's phase one** (milestone 48 (job control)): two supervised jobs in the background, listed,
    // one torn down with `kill -9`, a `wait` the gate interrupts (the job keeps running), the
    // survivor brought to the foreground and interrupted there, then the cooperative `kill` and
    // a `wait` on the one job it named. `wait` and `fg %1` are in [`SWISH_CHECK_INTERRUPTED`];
    // `wait %1` is spelled differently so the gate presses nothing under it. Every job here is
    // built from the shell's own untyped, so none counts against the pool.
    line(
        0,
        "interrupt_heeder &",
        &["[1] interrupt_heeder running in the background"],
    ),
    line(
        0,
        "interrupt_ignorer &",
        &["[2] interrupt_ignorer running in the background"],
    ),
    line(
        0,
        "jobs",
        &[
            "[1] interrupt_heeder running",
            "[2] interrupt_ignorer running",
        ],
    ),
    line(
        0,
        "kill -9 %2",
        &["[2] interrupt_ignorer: its process was torn down and its memory reclaimed"],
    ),
    line(0, "wait", &["stopped waiting; the job keeps running"]),
    line(
        0,
        "fg %1",
        &[
            "running interrupt_heeder in the foreground",
            "the job caught the interrupt and stopped cleanly after",
        ],
    ),
    line(
        0,
        "interrupt_heeder &",
        &["[1] interrupt_heeder running in the background"],
    ),
    line(0, "kill %1", &["asked job 1 to stop"]),
    line(
        0,
        "wait %1",
        &[
            "[1] interrupt_heeder:",
            "the job caught the interrupt and stopped cleanly after",
        ],
    ),
    line(0, "jobs", &["no jobs"]),
    // **Job control on an ordinary program** (milestone 933 (every job is built the way a
    // supervised job is)): `job_sleeper` is not interruptible and keeps no frame, so the shell
    // holds only the region it built it from and the endpoint its death arrives on. A job blocked
    // for ever and a job that exits on its own, listed; the cooperative `kill` refused with a
    // sentence; `kill -9` tearing down the blocked one; `wait` seeing the other exit while it waits;
    // a blocked job brought to the foreground and torn down by `^C`; one in the foreground that
    // ends on its own; and one that speaks while `wait` has the prompt.
    // Every job here is built from the shell's own budget, so none counts against the pool.
    line(
        0,
        "job_sleeper 0 --mem 4 &",
        &["[1] job_sleeper running in the background"],
    ),
    line(
        0,
        "job_sleeper 2500 --mem 4 &",
        &["[2] job_sleeper running in the background"],
    ),
    line(
        0,
        "jobs",
        &["[1] job_sleeper running", "[2] job_sleeper running"],
    ),
    line(0, "kill %1", &["that job has no cooperative stop"]),
    line(
        0,
        "kill -9 %1",
        &["[1] job_sleeper: its process was torn down and its memory reclaimed"],
    ),
    line(0, "wait %2", &["[2] job_sleeper:", "the job finished."]),
    line(0, "jobs", &["no jobs"]),
    line(
        0,
        "job_sleeper 0 --mem 4 &",
        &["[1] job_sleeper running in the background"],
    ),
    line(
        0,
        "fg %1",
        &[
            "running job_sleeper in the foreground",
            "^C: tearing the job down.",
            "its memory reclaimed",
        ],
    ),
    line(0, "job_sleeper 40 --mem 4", &["the sleeper woke"]),
    // A background job's output goes to the terminal's sink whenever it is written, over whatever
    // the prompt is doing, so the gate lets this one speak only while a `wait` has the prompt.
    line(
        0,
        "job_sleeper 1500 --mem 4 &",
        &["[1] job_sleeper running in the background"],
    ),
    line(0, "wait %1", &["[1] job_sleeper:", "the job finished."]),
    line(0, "jobs", &["no jobs"]),
    // **Ctrl-Z** (milestone 48's phase two): a job brought to the foreground with a bare `fg`,
    // which is in [`SWISH_CHECK_SUSPENDED`], is suspended by the `^Z` the gate presses, listed
    // stopped, resumed in the background with `bg`, and finished in the foreground with `^C`.
    line(
        0,
        "interrupt_heeder &",
        &["[1] interrupt_heeder running in the background"],
    ),
    line(
        0,
        "fg",
        &[
            "running interrupt_heeder in the foreground",
            "[1] interrupt_heeder stopped",
        ],
    ),
    line(0, "jobs", &["[1] interrupt_heeder stopped"]),
    line(0, "bg", &["[1] interrupt_heeder running in the background"]),
    line(0, "jobs", &["[1] interrupt_heeder running"]),
    line(
        0,
        "fg %1",
        &[
            "running interrupt_heeder in the foreground",
            "the job caught the interrupt and stopped cleanly after",
        ],
    ),
    // **The shell's budget holds what the progenitor's pool held** (milestone 933): six native
    // jobs in the background, each a region of 48 pages and 4 of `--mem`, and a `std` job of 384
    // pages in the foreground beside them. That is the pool's old mix, six native regions and one
    // `std` region (`grant_plan::JOB_POOL_PAGES`). The six are torn down after, so the budget they
    // took comes back for the hundred cycles below. Six jobs hold eighteen slots of the shell's
    // capability table, which keeps the boot's recorded peak where it was.
    line(
        0,
        "job_sleeper 0 --mem 4 &",
        &["[1] job_sleeper running in the background"],
    ),
    line(
        0,
        "job_sleeper 0 --mem 4 &",
        &["[2] job_sleeper running in the background"],
    ),
    line(
        0,
        "job_sleeper 0 --mem 4 &",
        &["[3] job_sleeper running in the background"],
    ),
    line(
        0,
        "job_sleeper 0 --mem 4 &",
        &["[4] job_sleeper running in the background"],
    ),
    line(
        0,
        "job_sleeper 0 --mem 4 &",
        &["[5] job_sleeper running in the background"],
    ),
    line(
        0,
        "job_sleeper 0 --mem 4 &",
        &["[6] job_sleeper running in the background"],
    ),
    line(
        0,
        "std_exerciser beside six",
        &["hello from std on nife", "exiting through process::exit"],
    ),
    line(
        0,
        "kill -9 %1",
        &["[1] job_sleeper: its process was torn down"],
    ),
    line(
        0,
        "kill -9 %2",
        &["[2] job_sleeper: its process was torn down"],
    ),
    line(
        0,
        "kill -9 %3",
        &["[3] job_sleeper: its process was torn down"],
    ),
    line(
        0,
        "kill -9 %4",
        &["[4] job_sleeper: its process was torn down"],
    ),
    line(
        0,
        "kill -9 %5",
        &["[5] job_sleeper: its process was torn down"],
    ),
    line(
        0,
        "kill -9 %6",
        &["[6] job_sleeper: its process was torn down"],
    ),
];
