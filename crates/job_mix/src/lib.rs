//! **The job mix a multi-tasking benchmark runs, defined once** (milestone 168 (a multi-tasking
//! workload benchmark)): one workload definition the kernel-side supervisor and the EL0 task both
//! read (AGENTS.md rule 7). It feeds §96 (process kernel or event kernel, and how to decide it),
//! whose live input is performance on a multitasking workload, where micro-benchmarks cannot see
//! the difference. notes/job-mix.md has the AIM7 background and why Warton's 20% is not a target:
//! this instrument asks whether the mechanism behind it, kernel cache and TLB footprint, is live
//! here, as a knee in jobs per minute against task count.
//!
//! It keeps AIM7's heterogeneous jobs, random order per task, task-count sweep and throughput, and
//! its categories rather than its 53 jobs, most of which name Unix services this system lacks.
//!
//! | job | AIM7 category | what it costs here |
//! |---|---|---|
//! | [`COMPUTE`] | compute-bound arithmetic loops | no syscall at all; the control arm |
//! | [`TOUCH`] | user virtual-memory operations | a working set walked in user mode, no syscall |
//! | [`NULL_SYSCALL`] | (the trap itself) | EL0 to EL1 and back, the cheapest kernel entry |
//! | [`YIELD`] | (scheduling pressure) | a full context switch through the ready queue |
//! | [`ROUND_TRIP`] | pipe I/O | `CALL` to a shared server and its `REPLY`: two rendezvous |
//! | [`MAP`] | user virtual-memory operations (mapping) | split a region, build a space, 32 `MAP_INTO`s, `DESTROY` |
//! | [`SPAWN`] | process creation | build two children from EL0, `RECEIVE` each one's exit, reclaim |
//!
//! [`ROUND_TRIP`] answers §96, since a process kernel's cost is the stack a blocking thread leaves
//! behind; [`COMPUTE`] and [`TOUCH`] displace the cache between kernel entries, which a ping-pong
//! micro-benchmark leaves out. [`MAP`] and [`SPAWN`] build from each task's own budget
//! ([`SLOT_BUDGET`]), with no new syscall, method or object type.
//!
//! # BUGS
//!
//! - No number from this is comparable with Warton's 20% (notes/job-mix.md): his ran two tasks on
//!   a hosted Linux and is a ratio between two kernel models, and this tree has one.
//! - Disk-file operations are absent: radon cannot read a disk, and one shared file channel would
//!   be raced by 32 tasks (`design/roadmap/0493-a-disk-file-job-mix-needs-a-disk-radon-can-drive.md`).
//! - [`MAP`] and [`SPAWN`] take the one memory-region lock; `region_ticks` on each `job-mix-kind:`
//!   line lets a reader subtract it.
//! - Per-kind ticks are wall time and include preemption, so they do not sum to the subrun.
//! - [`MIX`] is chosen, not derived: quote a result with the mix that produced it.
//! - The work constants are sized for real machines; TCG magnitudes are fiction.
//! - [`order`] is a shuffle of one multiset per task, not AIM7's independent draws.
//!
//! Name: ratified 2026-09-13 (calef): AIM7's workfile is a *mix* of *jobs*.

#![no_std]
#![deny(missing_docs)]

// **The console markers, which are a contract and not a wording** (milestone 324 part 2).
//
// Everything below this comment and above `COMPUTE` is text that leaves the machine and is read
// back by something that is not the machine: `crates/serial_console` on a bench or in CI, and
// `cargo xtask job-mix` under QEMU. `crates/boot_ladder` holds the boot tour's markers for exactly
// this reason and its header carries the argument; these are the sweep's, and they lived as four
// private `const`s in `kernel/src/job_mix.rs` plus four string literals in `xtask/src/main.rs`
// until this milestone. Three copies of a contract agreeing by a reader having checked is milestone
// 268's finding 3, and the fix is the one that milestone found: there is one of them.
//
// **What is shared is the head, and the fields inside the line are not.** That is the gap
// `crates/serial_console`'s `SweepPoint` carries a `BUGS` entry for, and 2026-09-19 is the day it
// cost something: the tail of [`POINT`] gained four field names and lost two, every marker here
// still matched, and the recogniser silently read nothing.
//
// They are **stable heads**, on `boot_ladder`'s rule and for its reason: the head is what a matcher
// keys on and never changes, the tail carries the numbers and is free to improve. A contract on
// the whole line would make every improvement to the diagnosis a breaking change.
//
// Names provisional (milestone 324): they are printed and matched, so they are a contract, and
// calef names public items. Spelled as bare nouns to match `boot_ladder`'s `BANNER`, `MACHINE`,
// `TOUR`; the kernel's own `START_MARKER` and `DONE_MARKER` spellings were retired into these
// rather than moved, because `job_mix::START_MARKER` says *marker* twice.

/// **The sweep has begun**: `job-mix: started <n> tasks and <m> servers on <c> online core(s), ...`.
///
/// Printed by `kernel/src/job_mix.rs` once the whole pool has spawned, so reaching it means every
/// task and every echo server exists. A reader that finds this and then nothing has a sweep that
/// wedged rather than a kernel that refused, and telling those two apart is what milestone 324's
/// part 2 is for.
pub const STARTED: &str = "job-mix: started";

/// **The sweep ran to its end**: `job-mix: done`, on a line of its own.
///
/// The kernel parks in `wfi` afterwards rather than exiting, so this line is the only thing that
/// says a sweep finished. Silence after it is the correct end state; silence before it is not.
pub const DONE: &str = "job-mix: done";

/// **The kernel would not start the sweep**: `job-mix: FAILED: <why>`.
///
/// Three cases print it (no `job_mix_task` in the archive, and either kind of spawn failing), all
/// of them before [`STARTED`], and all of them followed by a halt. The tail names which.
pub const FAILED: &str = "job-mix: FAILED: ";

/// **One point of the sweep completed**: `job-mix: tasks=<n> jobs=<j> repeats=<k> ticks_min=<a>
/// ticks_median=<b> ticks_max=<c> jpm_median=<r>`.
///
/// One per entry in [`TASK_SWEEP`], printed after that entry's [`REPEATS`] subruns, carrying their
/// [`Spread`] and the jobs-per-minute figure computed from the median. Counting these is how a
/// reader knows how far along a sweep is.
///
/// **The tail changed on 2026-09-19 and this is the record of it.** Until then it was
/// `ticks=<t> jpm=<r>`, the best of three; [`REPEATS`] explains why the statistic is now the median
/// of 21 and why the two ends are printed beside it. The head did not move, so a transcript from
/// either side of that date is still recognisably a sweep, and the two statistics are not
/// comparable. A reader with an older log has `ticks=` and `jpm=` and should say which it is
/// quoting.
pub const POINT: &str = "job-mix: tasks=";

/// **One measured subrun finished**: `job-mix-repeat: tasks=<n> repeat=<r> ticks=<t>`.
///
/// Finer than [`POINT`] by a factor of [`REPEATS`], and it is the finest progress a sweep emits.
/// That matters to a watcher: a sweep has **no wall-clock heartbeat** the way
/// `kernel/src/soak.rs` does, so the longest silence a healthy sweep can produce is one subrun at
/// the top of [`TASK_SWEEP`], and anything deciding that a sweep is wedged has to allow for it.
pub const SUBRUN: &str = "job-mix-repeat: ";

/// **A placement-census line**: `job-mix-census: ...`, printed at sweep start.
///
/// Its own word rather than `job-mix:` on purpose, the same split `kernel/src/soak.rs` makes: a
/// census is neither the start of a run nor a result, and a watcher keying on the result prefix
/// should not have to be proven harmless against it. The `-census` suffix is outside both
/// [`STARTED`] and [`POINT`], which is what makes that true rather than hoped.
pub const CENSUS: &str = "job-mix-census:";

/// **One job kind's share of one sweep point**: `job-mix-kind: tasks=<n> kind=<name> jobs=<j>
/// ticks=<t> per_job=<p> region_ticks=<r>`.
///
/// [`JOB_KINDS`] of these after each [`POINT`], summed over every task and every repeat at that
/// point. Its own word rather than `job-mix:` for [`CENSUS`]'s reason: a watcher counting [`POINT`]
/// lines should not have to be proven harmless against it, and the `-kind` suffix is what makes
/// that true rather than hoped. Name provisional (2026-09-19), the same standing as the six above.
pub const KIND: &str = "job-mix-kind:";

/// **How long one sweep point's threads waited for kernel locks**, summed over every core and every
/// repeat at that point, timed windows only. Two shapes, after the [`KIND`] lines:
///
/// - `job-mix-lock: tasks=<n> rank=<r> name=<word or -> contended=<c> wait_ticks=<w>`, one per lock
///   rank that was ever found held;
/// - `job-mix-lock: tasks=<n> site=reap reaps=<r> stack_free_ticks=<f> remove_ticks=<m>`, the
///   reaper: threads reaped, time freeing their kernel stacks (no lock held), and time its last
///   critical section held `IPC_TABLES`;
/// - `job-mix-lock: tasks=<n> site=current_cap calls=<c> contended=<k> wait_ticks=<w>`, the one
///   lock the cheapest syscall takes: every call, and the ones that found it held.
///
/// Printed only by a kernel built with `--features lock_wait` (2026-10-04, for fatal risk 4's
/// `null_syscall` question; notes/job-mix/null-syscall-under-load.md). Its own word for [`CENSUS`]'s reason. Name
/// provisional (2026-10-04), the same standing as the seven above.
pub const LOCK: &str = "job-mix-lock:";

/// A compute-bound arithmetic loop, [`COMPUTE_ITERS`] iterations. No syscall.
pub const COMPUTE: u8 = 0;
/// A walk over [`TOUCH_WORDS`] words of the task's own memory, read and written. No syscall.
pub const TOUCH: u8 = 1;
/// [`NULL_SYSCALL_CALLS`] trips through the cheapest syscall this ABI has.
pub const NULL_SYSCALL: u8 = 2;
/// [`YIELD_CALLS`] voluntary yields, each a trip through the ready queue.
pub const YIELD: u8 = 3;
/// [`ROUND_TRIP_CALLS`] `CALL`/`REPLY` round trips against a shared server.
pub const ROUND_TRIP: u8 = 4;
/// **User page mapping** (added 2026-09-19): [`MAP_CALLS`] `MAP_INTO`s into an address space the
/// task builds for the job from its own budget, then one `DESTROY` of the whole thing. AIM7's
/// virtual-memory category, and the first job in the mix whose kernel path takes a lock every other
/// task's same job also takes (the memory-region table, `kernel/src/memory_region.rs`'s `REGIONS`).
pub const MAP: u8 = 5;
/// **Process creation** (added 2026-09-19): [`SPAWN_CALLS`] children built from EL0 through the
/// granular verbs, run to exit, reaped and reclaimed. AIM7's process-creation category, and the job
/// that blocks deepest: the parent waits in `RECEIVE` on a thread that did not exist a moment before.
pub const SPAWN: u8 = 6;

/// How many job kinds there are. A counted claim: the table in this crate's header has one row per
/// kind, [`KIND_NAMES`] one name per kind, and [`MIX`] must contain each of them at least once.
pub const JOB_KINDS: usize = 7;

/// The word each kind is printed as on a `job-mix-kind:` line, indexed by kind. Provisional (this
/// lane's coinage, 2026-09-19): they are console strings a reader greps for, so they follow the rule
/// the markers do and are spelled the way a reader meets them, which is lower case.
pub const KIND_NAMES: [&str; JOB_KINDS] = [
    "compute",
    "touch",
    "null_syscall",
    "yield",
    "round_trip",
    "map",
    "spawn",
];

/// **The workfile, in AIM7's sense**: the multiset of jobs one task runs per round, and therefore
/// the proportions. Sixteen entries so a round is long enough to time and short enough that a task
/// visits every kind several times inside one subrun.
///
/// The weighting is stated rather than derived (see this crate's `BUGS`): [`ROUND_TRIP`] gets a
/// quarter of the mix because IPC is the primitive this kernel exists to be fast at, and the other
/// six split the rest evenly, two each.
///
/// **Changed 2026-09-19, and a transcript says which mix it ran.** Until then the mix had five kinds
/// (three each of the first four, four round trips); [`MAP`] and [`SPAWN`] took one slot from each
/// of those four. The `job-mix: started` line prints [`JOB_KINDS`], so a five-kind transcript and a
/// seven-kind one cannot be mistaken for each other, and they are not comparable point for point.
pub const MIX: [u8; 16] = [
    COMPUTE,
    TOUCH,
    NULL_SYSCALL,
    YIELD,
    ROUND_TRIP,
    MAP,
    SPAWN,
    ROUND_TRIP,
    COMPUTE,
    TOUCH,
    NULL_SYSCALL,
    YIELD,
    ROUND_TRIP,
    MAP,
    SPAWN,
    ROUND_TRIP,
];

/// Jobs in one round, which is [`MIX`]'s length.
pub const MIX_LEN: usize = MIX.len();

/// Rounds of [`MIX`] one task runs per subrun. `JOBS_PER_TASK` is this times [`MIX_LEN`].
///
/// Sized so that a single task's subrun runs for a few tens of milliseconds on real silicon, which
/// is two things at once: long against the release skew the supervisor cannot avoid (it hands out
/// the go-ahead one rendezvous at a time, so the last task starts N round trips after the first),
/// and long against a timer whose grain is tens of nanoseconds.
pub const ROUNDS_PER_TASK: u64 = 8;

/// Jobs one task completes in one subrun. The numerator of the AIM7 metric.
pub const JOBS_PER_TASK: u64 = ROUNDS_PER_TASK * MIX_LEN as u64;

/// Iterations of the arithmetic grind in one [`COMPUTE`] job.
pub const COMPUTE_ITERS: u64 = 20_000;

/// `u64` words touched in one [`TOUCH`] job. 4,096 words is 32 KiB, which is the smallest L1d this
/// project targets (the `SiFive` U74's), so one job's working set is exactly the size that evicts
/// it. Chosen against that cache rather than against the dev Mac's, for the reason milestone 134's
/// E1 records: a large-cache machine hides this whole effect.
pub const TOUCH_WORDS: usize = 4_096;

/// Syscalls in one [`NULL_SYSCALL`] job.
pub const NULL_SYSCALL_CALLS: u64 = 64;

/// Yields in one [`YIELD`] job.
pub const YIELD_CALLS: u64 = 64;

/// `CALL`/`REPLY` round trips in one [`ROUND_TRIP`] job.
pub const ROUND_TRIP_CALLS: u64 = 32;

/// `MAP_INTO`s in one [`MAP`] job, each of one frame at a fresh page-aligned address in the job's
/// own target space. 32 is inside one leaf page table on every architecture, so the job pays for
/// its tables once and then the map path proper; sized so one job costs the same order as a
/// [`ROUND_TRIP`] job on radon (`map_el0` there is about 1.5 us a map, `ipc_rtt_el0` about 6 us).
pub const MAP_CALLS: u64 = 32;

/// Pages split off a task's budget for one [`MAP`] job: the address-space object, the one frame
/// being aliased, the page tables down to one leaf table, and the mapping-record pages revocation
/// needs to find every mapping again. **Sized to fit with a margin, by running it, not derived**:
/// the kernel refuses a mapping whose record it cannot afford (`OutOfMemory`), the task reports that
/// as `job-mix: FAILED`, and `script/job-mix` on all three architectures is what says this is enough.
pub const MAP_REGION_PAGES: u64 = 16;

/// Children one [`SPAWN`] job builds, runs and reclaims, one at a time. Two, because a spawn is the
/// heaviest thing in the mix (`spawn_el0` on radon is about 65 us a child) and two already make it
/// the most expensive job per call; one child per job would leave the parent's own `RECEIVE` on a
/// brand-new thread as the whole of it.
pub const SPAWN_CALLS: u64 = 2;

/// Pages split off a task's budget for one child: its address space and page tables, its stack
/// page, its thread object and the revocation records. `os_primitives_benchmarker`'s `CHILD_PAGES`,
/// the same child, measured there.
pub const CHILD_PAGES: u64 = 10;

/// **The untyped budget each task is granted, in pages**, and the reason it is one region per task
/// rather than one shared: two tasks splitting one region would interleave their `SPLIT`s, and a
/// region reclaims only in the order it was split (DECISIONS §16), so the first `DESTROY` out of
/// order would be refused and the job would be measuring a protocol error.
///
/// One page for the child code frame the task keeps for the whole run, eight for the page tables
/// that frame's own mapping may need in the task's address space, and the larger of the two jobs'
/// transient regions, since a task runs one job at a time and each gives back everything it split.
pub const TASK_BUDGET_PAGES: u64 = 1
    + 8
    + if MAP_REGION_PAGES > CHILD_PAGES {
        MAP_REGION_PAGES
    } else {
        CHILD_PAGES
    };

/// The largest task pool the supervisor builds, and therefore the length of its go-endpoint array.
///
/// 32, against `sched::MAX_THREADS`'s 256 and against the 64 user threads the soak already builds
/// on this machine. The sweep tops out here rather than at milestone 134's E1 ceiling of 96 because
/// every task in this pool is a **process**, with an address space and a loaded image behind it,
/// where E1's were kernel threads; the memory, not the thread table, is what binds.
pub const MAX_TASKS: usize = 32;

/// The subruns, in AIM7's sense: how many tasks are released for each measurement.
///
/// AIM7 increments by one and runs until throughput collapses. This doubles, because a run on a
/// board is an evening of somebody's time and the interesting feature (a knee, or its absence) is
/// visible on a log axis. 1 is the control: a single task with the whole machine.
pub const TASK_SWEEP: [usize; 6] = [1, 2, 4, 8, 16, MAX_TASKS];

/// **Measured repeats per subrun, and the statistic is their median, not their best** (changed
/// 2026-09-19; it was three repeats and the best of them until then).
///
/// **Why the median.** The best of N is `kernel/src/bench.rs`'s rule and it is right there: on a
/// busy dev Mac the minimum is the least host-contended sample and everything above it is somebody
/// else's load. Here the contention *is* the subject. On radon the tasks' own contention for
/// [`ECHO_SERVERS`] spreads a subrun's time over a wide distribution (37% within one boot at
/// `tasks=4`), and a minimum drawn from a wide distribution is the statistic that moves most with
/// the sample count: the more repeats, the luckier the best one gets. A median converges instead.
///
/// **Why 21, from the five radon boots of 2026-09-16 rather than from taste.** Resampling the fifteen
/// `tasks=4` repeats those boots produced, five simulated boots of N repeats each give a
/// boot-to-boot spread of the reported figure of about 22% for the best of 3 (29.4% was observed),
/// 17% for the median of 3, 5.1% for the median of 15 and 4.2% for the median of 21, where
/// `tasks=16` and `tasks=32` already sat at 2.7 to 6.7% with three. 21 brings the worst point into
/// the band the stable points were in, and it is odd so the median is a sample rather than an
/// average of two. The method and the figures are in milestone 168's appendix,
/// `design/roadmap/0168-multitasking-benchmark/holes-closed-2026-09-19.md`.
///
/// **One count for every point, and the table that would have saved time was refused.** Varying the
/// count by sweep point was proposed because board time was thought to be the cost, and it is not:
/// the whole sweep's timed windows at 21 repeats come to about eleven seconds on radon at the old mix's rates,
/// against a boot that takes minutes. A table would buy seconds and cost a reader a second thing to
/// hold. The cost that does grow is the TCG rehearsal's, which is minutes, and is paid once per
/// architecture by a gate, not by anybody at a bench.
pub const REPEATS: usize = 21;

// A task's budget holds what it keeps plus the larger of the two transient regions, so no job is
// ever refused a split the budget was sized to allow. A relation between constants in one file, so
// the compiler checks it (AGENTS.md's ladder, rung one).
//
// **Tightened, because the old form left the whole sizing argument unchecked.** Found by
// milestone 326 (nobody has been assigned to turn a mutation score upward).
// It asserted only that the budget exceeds each transient region on its own, which 19, 24 and 144
// all satisfy, so five mutants of the expression above compiled and passed: one of them picked the
// SMALLER of the two regions, leaving a task running `MAP` nine pages short of what the comment
// promises it. The two bounds below are what the paragraph on `TASK_BUDGET_PAGES` actually claims:
// at least the kept pages plus the larger region, and less than both regions together, because a
// task runs one job at a time and each gives its region back. Written as compile-time assertions
// rather than as a test for the reason the comment above already gives.
const _: () = assert!(TASK_BUDGET_PAGES >= 1 + 8 + MAP_REGION_PAGES);
const _: () = assert!(TASK_BUDGET_PAGES >= 1 + 8 + CHILD_PAGES);
const _: () = assert!(TASK_BUDGET_PAGES < 1 + 8 + MAP_REGION_PAGES + CHILD_PAGES);

/// The median, and the two ends, of one sweep point's repeats: what a `job-mix:` point line carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spread {
    /// The fastest repeat, in ticks. It is printed so the spread stays visible; it is not the result.
    pub min: u64,
    /// The middle repeat, in ticks. **This is the result**, and it is what `jpm_median` is made from.
    pub median: u64,
    /// The slowest repeat, in ticks.
    pub max: u64,
}

// An odd count, so the median is one of the samples rather than a mean of the middle two. A mean
// would be a tick count no subrun ever took, and on a bimodal point (`tasks=2` on radon has two
// modes about 14% apart) it would be a figure between the modes that describes neither.
const _: () = assert!(REPEATS % 2 == 1);

/// Sort `samples` in place and return its [`Spread`], or `None` for an empty slice.
///
/// In place and allocation-free because the caller is a kernel thread with a fixed array. For an
/// even length the median is the lower of the middle two, a sample rather than a mean, for the
/// reason the assertion above [`REPEATS`] gives; the sweep never passes one, but a test might.
#[must_use]
pub fn spread(samples: &mut [u64]) -> Option<Spread> {
    if samples.is_empty() {
        return None;
    }
    samples.sort_unstable();
    Some(Spread {
        min: samples[0],
        median: samples[(samples.len() - 1) / 2],
        max: samples[samples.len() - 1],
    })
}

/// Shared servers the [`ROUND_TRIP`] job calls. More than one so that the sweep's larger subruns are
/// not measuring a single server's serialization; fewer than the task count so that the endpoint is
/// genuinely contended, which is what a multi-tasking workload does to a microkernel.
pub const ECHO_SERVERS: usize = 2;

/// `arg0` for a task that runs the mix.
pub const ROLE_MIXER: u64 = 0;
/// `arg0` for a shared server the mixers call.
pub const ROLE_ECHO: u64 = 1;

/// Mixer slot 0: the endpoint it `SEND`s its result on. Slot 0 is "the endpoint I report on"
/// throughout this tree's benchmark programs, and this keeps that true.
pub const SLOT_REPORT: u64 = 0;
/// Mixer slot 1: the endpoint it `RECEIVE`s its go-ahead on, one per task.
pub const SLOT_GO: u64 = 1;
/// Mixer slot 2: the endpoint it `CALL`s for a [`ROUND_TRIP`] job.
pub const SLOT_ECHO: u64 = 2;
/// Echo-server slot 0: the endpoint it serves. It holds nothing else, and cannot report, spawn or
/// call: a compromised echo server can only answer wrongly, which is the least authority that does
/// the job.
pub const SLOT_SERVE: u64 = 0;
/// Mixer slot 3: the task's own untyped budget, [`TASK_BUDGET_PAGES`] long, which the [`MAP`] and
/// [`SPAWN`] jobs split their transient objects from and give back whole. A region and not a
/// narrower object because both jobs have to create objects, and creating is what a region is for.
pub const SLOT_BUDGET: u64 = 3;
/// Mixer slot 4: the endpoint the [`SPAWN`] job's children report on, one per task so no task can
/// receive another's child. The task holds it `READ | WRITE | GRANT` and inserts a `WRITE` view
/// into each child, the shape `os_primitives_benchmarker`'s spawn loop already has.
pub const SLOT_CHILD_DONE: u64 = 4;

/// Go word 0 on a task's go endpoint: run one subrun.
pub const GO_RUN: u64 = 0;
/// Go word 1: answer with the last subrun's per-kind breakdown, [`JOB_KINDS`] messages of
/// `[kind, ticks, region_ticks]`. Sent after the supervisor has stopped the clock, so the breakdown
/// costs the timed window nothing.
pub const GO_BREAKDOWN: u64 = 1;

/// The first word of a subrun report from a task that could not finish it: a job's syscall was
/// refused. The second word is the refusal (a negative `abi::Error` as `u64`) and the third is the
/// task's index with the failing kind in bits 32 and up, so the supervisor can say which job on
/// which task failed rather than stalling on a task that stopped.
pub const REPORT_FAILED: u64 = u64::MAX;

// **More tasks than servers, checked by the compiler rather than by a test.** The [`ROUND_TRIP`]
// job's endpoint has to be contended at the top of the sweep or the instrument is measuring an idle
// machine, and an instrument with no server at all cannot run that job at all. This is a relation
// between two constants in one file, which is the case where AGENTS.md's ladder says to make the
// wrong state unrepresentable rather than to write a check that runs later.
const _: () = assert!(ECHO_SERVERS >= 1 && ECHO_SERVERS < MAX_TASKS);

/// The order one task runs [`MIX`] in: a permutation of [`MIX`] chosen by `seed`.
///
/// Fisher-Yates driven by a 64-bit LCG, which is enough randomness for "these tasks are not in
/// lockstep" and is deterministic given the seed, so a run is reproducible. It is **not** a source
/// of randomness for anything else and must not be used as one.
///
/// # BUGS
///
/// - **Two seeds in, one order out: `order(2k)` and `order(2k + 1)` are the same permutation**,
///   for every `k`. The `seed | 1` below exists to keep the LCG off its degenerate state, and it
///   does that by throwing away bit zero, which halves the seed space. Measured rather than
///   reasoned, by milestone 326 triaging a mutation survivor that turned that `|` into a `^`.
///
///   **Nothing in the tree hits it**, which is why this is recorded instead of fixed: the kernel
///   hands every task "a distinct, odd, well-spread seed" (`kernel/src/job_mix.rs`), and an odd
///   seed passes through `| 1` unchanged, so no two tasks in a sweep have ever shared an order.
///   A caller that seeded tasks by consecutive index would put every adjacent pair in lockstep,
///   which is the one property this function exists to prevent.
///
///   Fixing it would change every permutation this function has ever produced, and a benchmark's
///   published numbers are a fact that has left the machine (AGENTS.md's *move fast on what can
///   be undone*). So the seed contract is written down here and the arithmetic is left alone.
#[must_use]
pub fn order(seed: u64) -> [u8; MIX_LEN] {
    let mut out = MIX;
    let mut x = seed | 1;
    let mut i = MIX_LEN;
    while i > 1 {
        i -= 1;
        x = x
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        // The high bits of an LCG are the good ones; the low bits cycle short.
        let j = ((x >> 33) as usize) % (i + 1);
        out.swap(i, j);
    }
    out
}

/// **The AIM7 metric**: jobs completed per minute, from a job count, a tick delta and the counter's
/// frequency in hertz.
///
/// Saturating rather than wrapping, and zero on a zero-length or zero-frequency measurement, so a
/// nonsense input produces a number a reader will disbelieve rather than one they will quote. The
/// intermediate is `u128` because `jobs * 60 * hz` overflows `u64` at unremarkable inputs: 4,096
/// jobs on radon's 4 MHz `rdtime` is already 9.8e11, and a 1 GHz counter would be there in one
/// subrun.
#[must_use]
pub fn jobs_per_minute(jobs: u64, ticks: u64, hz: u64) -> u64 {
    if ticks == 0 || hz == 0 {
        return 0;
    }
    let n = u128::from(jobs) * 60 * u128::from(hz) / u128::from(ticks);
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// How many jobs of `kind` one task runs in one subrun: its count in [`MIX`] times
/// [`ROUNDS_PER_TASK`]. The denominator a `job-mix-kind:` line's per-job figure is made with.
///
/// A `for` over the mix rather than a hand-rolled index, and the difference is not style. The
/// index version's own increment is a mutable it has to remember to advance, so the way to break
/// it is to make it never advance, and the whole suite's answer to that is to hang rather than to
/// fail: milestone 326 measured exactly one timeout in this crate and it was this loop. A `for`
/// has no increment to lose (AGENTS.md's ladder, rung one).
#[must_use]
pub fn jobs_of_kind(kind: u8) -> u64 {
    let mut n = 0u64;
    for &k in &MIX {
        if k == kind {
            n += 1;
        }
    }
    n * ROUNDS_PER_TASK
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every job kind named in the header's table appears in the mix. A kind that is defined and
    /// never run is a row of documentation describing a workload that does not exist.
    #[test]
    fn the_mix_runs_every_job_kind() {
        for kind in 0..JOB_KINDS as u8 {
            assert!(MIX.contains(&kind), "job kind {kind} is never run");
        }
    }

    /// The mix is exactly [`JOB_KINDS`] kinds and nothing else, so a kind added to the constants
    /// without a row in the table cannot ride along unnoticed.
    #[test]
    fn the_mix_contains_no_kind_that_is_not_defined() {
        for job in MIX {
            assert!(job < JOB_KINDS as u8, "mix holds undefined job kind {job}");
        }
    }

    /// **The property that makes two tasks comparable**: an order is a permutation of the mix, so
    /// every task does exactly the same work and only the sequence differs. If this were merely a
    /// random draw, a subrun's slowest task might simply have drawn more expensive jobs, and the
    /// throughput number would be measuring the dice.
    #[test]
    fn an_order_is_a_permutation_of_the_mix() {
        for seed in [0u64, 1, 7, 0x9E37_79B9_7F4A_7C15, u64::MAX] {
            let got = order(seed);
            let mut a = MIX;
            let mut b = got;
            a.sort_unstable();
            b.sort_unstable();
            assert_eq!(a, b, "seed {seed} did not produce a permutation");
        }
    }

    /// Different seeds produce different orders, which is the whole reason the seed exists: tasks
    /// released together must not walk the mix in lockstep. Not every pair need differ, but the
    /// spread over a hundred seeds must not collapse to one sequence.
    #[test]
    fn distinct_seeds_do_not_all_walk_the_mix_in_lockstep() {
        let first = order(1);
        let mut differing = 0;
        for seed in 2..102u64 {
            if order(seed) != first {
                differing += 1;
            }
        }
        assert!(
            differing > 90,
            "only {differing} of 100 seeds differed from the first; the shuffle is degenerate"
        );
    }

    /// **No position in the order is pinned to one job kind**, which is the property
    /// "different seeds give different orders" does not reach: a shuffle can vary a great deal
    /// overall while one slot never moves, and a slot that never moves is a slot every task runs
    /// the same job in, which is lockstep in the one place it matters least visibly.
    ///
    /// It is the draw this checks rather than the output. `order` takes the *high* bits of the
    /// LCG because the low ones cycle short, and taking the low ones instead makes the draw a
    /// multiple of a large power of two, which is zero modulo every power-of-two swap index: the
    /// last position then receives the mix's first job on every seed. Milestone 326 found that
    /// mutant alive with every other test here passing.
    #[test]
    fn no_position_in_the_order_is_pinned_to_one_job() {
        let mut seen = [[false; JOB_KINDS]; MIX_LEN];
        for seed in 1..=512u64 {
            for (position, &kind) in order(seed).iter().enumerate() {
                seen[position][kind as usize] = true;
            }
        }
        for (position, kinds) in seen.iter().enumerate() {
            for (kind, &ever) in kinds.iter().enumerate() {
                assert!(
                    ever,
                    "{} never appeared at position {position} across 512 seeds",
                    KIND_NAMES[kind]
                );
            }
        }
    }

    /// The same seed twice is the same order. A run has to be reproducible for a second run on the
    /// same board to be a comparison rather than a new experiment.
    #[test]
    fn an_order_is_deterministic_in_its_seed() {
        assert_eq!(order(12_345), order(12_345));
    }

    /// **The order a published run used does not move.** Seed 1 is the one the kernel hands its
    /// first task, so this is the sequence every recorded sweep's task 0 walked. Changing the
    /// arithmetic would change it, and the BUGS on [`order`] says why that is refused.
    #[test]
    fn the_first_tasks_order_is_the_one_every_recorded_run_used() {
        assert_eq!(
            order(1),
            [
                MAP,
                YIELD,
                ROUND_TRIP,
                TOUCH,
                NULL_SYSCALL,
                SPAWN,
                ROUND_TRIP,
                TOUCH,
                NULL_SYSCALL,
                COMPUTE,
                ROUND_TRIP,
                ROUND_TRIP,
                MAP,
                COMPUTE,
                YIELD,
                SPAWN,
            ]
        );
    }

    /// The metric, against a case worked by hand: 600 jobs in one second of a 4 MHz counter is
    /// 36,000 jobs per minute.
    #[test]
    fn the_metric_is_jobs_per_minute() {
        assert_eq!(jobs_per_minute(600, 4_000_000, 4_000_000), 36_000);
    }

    /// A zero-length or unmeasurable window reports zero rather than dividing by zero or reporting
    /// an enormous rate a reader might believe.
    #[test]
    fn an_unmeasurable_window_reports_zero() {
        assert_eq!(jobs_per_minute(600, 0, 4_000_000), 0);
        assert_eq!(jobs_per_minute(600, 4_000_000, 0), 0);
    }

    /// **The overflow this function exists to hold.** `jobs * 60 * hz` at a plausible silicon
    /// frequency passes `u64::MAX` while the answer is small, so a `u64` intermediate would have
    /// reported a wrapped rate as though it were a measurement.
    #[test]
    fn a_plausible_silicon_measurement_does_not_overflow() {
        // 32 tasks * 128 jobs at 1 GHz over one second: the numerator is 2.5e11 * 1e9.
        let jobs = MAX_TASKS as u64 * JOBS_PER_TASK;
        let hz = 1_000_000_000;
        assert_eq!(jobs_per_minute(jobs, hz, hz), jobs * 60);
    }

    /// The sweep is increasing and ends at the pool size, so the supervisor never releases more
    /// tasks than it built and a partial run's printed points are still a prefix of the curve.
    #[test]
    fn the_sweep_climbs_to_the_pool_size() {
        assert_eq!(*TASK_SWEEP.last().expect("a sweep has points"), MAX_TASKS);
        for w in TASK_SWEEP.windows(2) {
            assert!(w[0] < w[1], "the sweep is not increasing at {w:?}");
        }
        assert!(TASK_SWEEP[0] >= 1);
    }

    /// The median is the middle sample and the ends are the ends, whatever order they arrive in.
    #[test]
    fn a_spread_is_the_middle_sample_and_the_ends() {
        let mut v = [181_408, 132_148, 149_654];
        assert_eq!(
            spread(&mut v),
            Some(Spread {
                min: 132_148,
                median: 149_654,
                max: 181_408
            })
        );
    }

    /// **The case the median exists for.** A bimodal point (radon's `tasks=2`, five fast repeats
    /// and ten slow ones across five boots) reports a slow-mode sample, where the best-of rule
    /// reported whichever fast repeat a boot happened to draw.
    #[test]
    fn a_bimodal_point_reports_its_majority_mode() {
        let mut v = [
            97_672, 97_778, 98_076, 98_109, 98_713, 111_005, 111_113, 111_256, 111_268, 111_400,
            111_427, 111_496, 111_513, 111_568, 112_124,
        ];
        let s = spread(&mut v).expect("fifteen samples");
        assert_eq!(s.median, 111_256);
        assert_eq!(s.min, 97_672);
    }

    /// An even count takes the lower middle, a sample and never a mean of two.
    #[test]
    fn an_even_count_takes_a_sample_not_a_mean() {
        let mut v = [4, 1, 3, 2];
        assert_eq!(spread(&mut v).expect("four").median, 2);
        assert_eq!(spread(&mut []), None);
    }

    /// Every kind has a printable name and runs at least once a round, so every `job-mix-kind:`
    /// line has a nonzero denominator.
    #[test]
    fn every_kind_is_named_and_counted() {
        let mut total = 0;
        for kind in 0..JOB_KINDS as u8 {
            assert!(!KIND_NAMES[kind as usize].is_empty());
            assert!(jobs_of_kind(kind) >= ROUNDS_PER_TASK);
            total += jobs_of_kind(kind);
        }
        assert_eq!(total, JOBS_PER_TASK);
    }
}
