//! The multi-tasking workload benchmark's supervisor, and the thing that times it.
//!
//! With `--features job_mix`, [`run`] replaces the boot tour's halt: it builds a pool of
//! [`job_mix::MAX_TASKS`] tasks (`fixtures/src/job_mix_task.rs`) once, releases the first N per
//! subrun over [`job_mix::TASK_SWEEP`], prints each point, then [`job_mix::DONE`], and halts. The
//! number feeds §96 (process kernel or event kernel, and how to decide it). See notes/job-mix.md
//! and notes/job-mix/supervisor.md.

//! # BUGS
//! - A single boot's jobs-per-minute is not a result: placement spans the rate fifteenfold.
//! - Each point is the median of [`job_mix::REPEATS`] repeats, with the spread beside it.
//! - The untimed breakdown exchange warms the report path between repeats.
//! - It cannot compare against an event kernel, which does not exist.
//! - The supervisor is one more thread, blocked in `RECEIVE` through every timed window.
//! - The clock includes draining the N task reports.
//! - Any message on the report endpoint counts as a report, so a stray `CALL` wedges the sweep.

#[cfg(feature = "lock_wait")]
use job_mix::LOCK;
use job_mix::{
    CENSUS, DONE, ECHO_SERVERS, FAILED, JOB_KINDS, KIND, MAX_TASKS, POINT, REPEATS, STARTED,
    SUBRUN, TASK_SWEEP,
};

use crate::cap::{Rights, memory_region_lending_capability, rendezvous_cap};
use crate::user::{self, Spawn};
use crate::{arch, println, sched, smp};

/// Run the sweep and never come back. The caller is the boot thread at the end of the tour, and
/// this replaces its `arch::halt()`.
pub fn run() -> ! {
    let cpus = smp::online_count();

    // Through the trust root, because this boot has no progenitor to measure it.
    let Some(image) = crate::trust::require_program("job_mix_task") else {
        println!("{FAILED}no 'job_mix_task' program in the initial ramdisk; nothing to run");
        arch::halt(arch::HaltReason::measurement_boot());
    };

    let pool = build_pool();

    // Where each thread was placed at spawn, servers first, for the census.
    let mut placed = [u8::MAX; ECHO_SERVERS + MAX_TASKS];

    // Servers first, so the first subrun does not time a server's spawn.
    spawn_echo_servers(image, &pool.echo, &mut placed);
    spawn_tasks(image, &pool, &mut placed);

    let hz = arch::timer::frequency();
    print_preamble(cpus, hz, &placed);

    for &tasks in &TASK_SWEEP {
        measure_point(pool.report, &pool.go, tasks, hz);
    }

    println!("{DONE}");
    arch::halt(arch::HaltReason::measurement_boot());
}

/// The supervisor's endpoints and budgets, built once with the pool rather than per subrun.
struct Pool {
    report: sched::RendezvousId,
    go: [sched::RendezvousId; MAX_TASKS],
    budget: [u64; MAX_TASKS],
    child_done: [sched::RendezvousId; MAX_TASKS],
    echo: [sched::RendezvousId; ECHO_SERVERS],
}

/// Create the report endpoint, each task's go endpoint, budget and child-done endpoint, and the
/// echo servers' endpoints. Halts the boot if a budget cannot be created.
fn build_pool() -> Pool {
    let report = sched::create_rendezvous();
    let mut go = [0u64; MAX_TASKS];
    for slot in &mut go {
        *slot = sched::create_rendezvous();
    }
    let mut budget = [0u64; MAX_TASKS];
    for (i, slot) in budget.iter_mut().enumerate() {
        let Some(region) = crate::memory_region::create(job_mix::TASK_BUDGET_PAGES) else {
            println!(
                "{FAILED}could not create task {i}'s {}-page budget",
                job_mix::TASK_BUDGET_PAGES
            );
            arch::halt(arch::HaltReason::measurement_boot());
        };
        *slot = region;
    }
    let mut child_done = [0u64; MAX_TASKS];
    for slot in &mut child_done {
        *slot = sched::create_rendezvous();
    }
    let mut echo = [0u64; ECHO_SERVERS];
    for slot in &mut echo {
        *slot = sched::create_rendezvous();
    }
    Pool {
        report,
        go,
        budget,
        child_done,
        echo,
    }
}

/// Spawn the echo servers and record the core each was placed on.
fn spawn_echo_servers(
    image: &'static [u8],
    echo: &[sched::RendezvousId; ECHO_SERVERS],
    placed: &mut [u8; ECHO_SERVERS + MAX_TASKS],
) {
    for (i, &ep) in echo.iter().enumerate() {
        let grant = rendezvous_cap(ep, Rights::READ);
        let started = sched::spawn_reporting_placement(move || {
            user::run(
                image,
                Spawn {
                    arg0: job_mix::ROLE_ECHO,
                    arg1: i as u64,
                    arg2: 0,
                    grants: &[grant],
                    maps: &[],
                },
            )
        });
        let Some((_tid, cpu)) = started else {
            println!("{FAILED}could not spawn echo server {i} of {ECHO_SERVERS}");
            arch::halt(arch::HaltReason::measurement_boot());
        };
        placed[i] = u8::try_from(cpu).unwrap_or(u8::MAX);
    }
}

/// Spawn every task with its five grants and record the core each was placed on.
///
/// The grants are in `crates/job_mix`'s slot order, each the least its use needs. See
/// notes/job-mix/supervisor.md.
fn spawn_tasks(image: &'static [u8], pool: &Pool, placed: &mut [u8; ECHO_SERVERS + MAX_TASKS]) {
    let &Pool {
        report,
        ref go,
        ref budget,
        ref child_done,
        ref echo,
    } = pool;
    for i in 0..MAX_TASKS {
        let grants = [
            rendezvous_cap(report, Rights::WRITE),
            rendezvous_cap(go[i], Rights::READ),
            rendezvous_cap(echo[i % ECHO_SERVERS], Rights::WRITE),
            memory_region_lending_capability(budget[i]),
            rendezvous_cap(
                child_done[i],
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
        ];
        let started = sched::spawn_reporting_placement(move || {
            user::run(
                image,
                Spawn {
                    arg0: job_mix::ROLE_MIXER,
                    arg1: i as u64,
                    // Distinct per task, or two tasks walk the mix in lockstep.
                    arg2: (i as u64)
                        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        .wrapping_add(1),
                    grants: &grants,
                    maps: &[],
                },
            )
        });
        let Some((_tid, cpu)) = started else {
            println!("{FAILED}could not spawn task {i} of {MAX_TASKS}");
            arch::halt(arch::HaltReason::measurement_boot());
        };
        placed[ECHO_SERVERS + i] = u8::try_from(cpu).unwrap_or(u8::MAX);
    }
}

/// The start marker, what the run measures and does not, and the spawn census.
fn print_preamble(cpus: usize, hz: u64, placed: &[u8; ECHO_SERVERS + MAX_TASKS]) {
    println!();
    println!(
        "{STARTED} {MAX_TASKS} tasks and {ECHO_SERVERS} servers on {cpus} online core(s), \
         cntfrq={hz} Hz"
    );
    println!(
        "job-mix: one job is one of {} kinds ({} per round, {} rounds per task per subrun); see \
         crates/job_mix for the mix and what it deliberately leaves out",
        job_mix::JOB_KINDS,
        job_mix::MIX_LEN,
        job_mix::ROUNDS_PER_TASK
    );
    println!(
        "job-mix: this measures THIS kernel under multi-tasking load; it compares nothing against an \
         event kernel and does not decide design/decisions/0096-process-kernel-or-event-kernel.md"
    );
    println!(
        "job-mix: each point is the median of {REPEATS} repeats, with the fastest and slowest \
         beside it (since 2026-09-19); a transcript before that date printed the best of 3 as \
         ticks= and jpm=, which is a different statistic from a different mix and is not \
         comparable with jpm_median="
    );
    print_census(placed);
    println!(
        "{CENSUS} placement decides throughput on real silicon by up to fifteenfold \
         (notes/soak.md, milestone 240), so a jobs-per-minute figure from ONE boot is a draw and \
         not a result; notes/job-mix.md's procedure asks for repeated boots"
    );
}

/// One point of the sweep: `REPEATS` subruns of `tasks` tasks, then the spread and the per-kind
/// breakdown.
fn measure_point(
    report: sched::RendezvousId,
    go: &[sched::RendezvousId; MAX_TASKS],
    tasks: usize,
    hz: u64,
) {
    let mut samples = [0u64; REPEATS];
    let mut kind_ticks = [0u64; JOB_KINDS];
    let mut region_ticks = [0u64; JOB_KINDS];
    #[cfg(feature = "lock_wait")]
    crate::lock_wait::reset();
    for (repeat, sample) in samples.iter_mut().enumerate() {
        let ticks = subrun(report, &go[..tasks]);
        println!("{SUBRUN}tasks={tasks} repeat={repeat} ticks={ticks}");
        *sample = ticks;
        breakdown(report, &go[..tasks], &mut kind_ticks, &mut region_ticks);
    }
    let Some(spread) = job_mix::spread(&mut samples) else {
        unreachable!("REPEATS is a nonzero constant");
    };
    let jobs = tasks as u64 * job_mix::JOBS_PER_TASK;
    println!(
        "{POINT}{tasks} jobs={jobs} repeats={REPEATS} ticks_min={} ticks_median={} \
         ticks_max={} jpm_median={}",
        spread.min,
        spread.median,
        spread.max,
        job_mix::jobs_per_minute(jobs, spread.median, hz)
    );
    // Totals over every task and repeat at this point, so the per-job division can be re-derived.
    for kind in 0..JOB_KINDS {
        let kind_jobs = job_mix::jobs_of_kind(kind as u8) * tasks as u64 * REPEATS as u64;
        println!(
            "{KIND} tasks={tasks} kind={} jobs={kind_jobs} ticks={} per_job={} \
             region_ticks={}",
            job_mix::KIND_NAMES[kind],
            kind_ticks[kind],
            kind_ticks[kind] / kind_jobs.max(1),
            region_ticks[kind]
        );
    }
    #[cfg(feature = "lock_wait")]
    print_lock_wait(tasks);
}

/// One subrun: release `go.len()` tasks, wait for all of them, and return the wall-clock ticks.
///
/// The clock starts before the first release and stops after the last report, which is AIM7's own
/// definition of a subrun (it ends when every one of its tasks has completed its jobs) and which
/// this module's `BUGS` prices.
///
/// A task that could not finish reports [`job_mix::REPORT_FAILED`], and this halts on it with the
/// task, the job kind and the refusal, rather than printing a tick count for work that was not
/// done. The clock has already been read by then, and no number is printed for the point.
fn subrun(report: sched::RendezvousId, go: &[sched::RendezvousId]) -> u64 {
    #[cfg(feature = "lock_wait")]
    crate::lock_wait::arm();
    let t0 = arch::timer::now();
    for &ep in go {
        sched::ipc_send(ep, [job_mix::GO_RUN, 0, 0]);
    }
    let mut failure = None;
    for _ in go {
        let [jobs, err, who, ..] = sched::ipc_receive(report);
        if jobs == job_mix::REPORT_FAILED && failure.is_none() {
            failure = Some((err, who));
        }
    }
    let ticks = arch::timer::now() - t0;
    #[cfg(feature = "lock_wait")]
    crate::lock_wait::disarm();
    if let Some((err, who)) = failure {
        let kind = (who >> 32) as usize;
        println!(
            "{FAILED}task {} could not finish a {} job: the kernel refused with {}",
            who & 0xffff_ffff,
            job_mix::KIND_NAMES.get(kind).copied().unwrap_or("unknown"),
            err as i64
        );
        arch::halt(arch::HaltReason::measurement_boot());
    }
    ticks
}

/// Ask each task released in the last subrun for its per-kind breakdown and add it to the running
/// totals. **After the clock stopped**, so it costs the timed window nothing; it is `JOB_KINDS`
/// rendezvous per task, which on the largest point is a few hundred, untimed.
fn breakdown(
    report: sched::RendezvousId,
    go: &[sched::RendezvousId],
    kind_ticks: &mut [u64; JOB_KINDS],
    region_ticks: &mut [u64; JOB_KINDS],
) {
    for &ep in go {
        sched::ipc_send(ep, [job_mix::GO_BREAKDOWN, 0, 0]);
        for _ in 0..JOB_KINDS {
            let [kind, ticks, region, ..] = sched::ipc_receive(report);
            if let Some(k) = kind_ticks.get_mut(kind as usize) {
                *k += ticks;
                region_ticks[kind as usize] += region;
            }
        }
    }
}

/// The `job-mix-lock:` lines for one point: every lock rank some thread found held during a timed
/// window, then the `current_cap` site. See `crate::lock_wait` for what each number can and cannot
/// see, and `job_mix::LOCK` for the line's shape.
#[cfg(feature = "lock_wait")]
fn print_lock_wait(tasks: usize) {
    for rank in 0..crate::lock_wait::ranks() {
        let (contended, wait_ticks) = crate::lock_wait::rank_totals(rank);
        if contended != 0 {
            println!(
                "{LOCK} tasks={tasks} rank={rank} name={} contended={contended} \
                 wait_ticks={wait_ticks}",
                crate::lock_wait::rank_name(rank).unwrap_or("-")
            );
        }
    }
    let (reaps, stack_free_ticks, remove_ticks) = crate::lock_wait::reap_totals();
    println!(
        "{LOCK} tasks={tasks} site=reap reaps={reaps} stack_free_ticks={stack_free_ticks} \
         remove_ticks={remove_ticks}"
    );
    let (calls, contended, wait_ticks) = crate::lock_wait::current_cap_totals();
    println!(
        "{LOCK} tasks={tasks} site=current_cap calls={calls} contended={contended} \
         wait_ticks={wait_ticks}"
    );
}

/// Print which of the pool is on which core, one line per online core, empty ones included.
///
/// `placed[i]` is where thread `i` was put at spawn, servers first and then tasks, and `u8::MAX`
/// means nothing knows yet. Tokens are `S<n>` for a server and `T<n>` for a task. It draws no
/// conclusion from the placement. See notes/soak/placement-census.md.
fn print_census(placed: &[u8]) {
    println!("{CENSUS} where the kernel placed each thread at spawn: S=echo server, T=task");
    let mut accounted = 0usize;
    for cpu in smp::online_cpus() {
        let here = u8::try_from(cpu).unwrap_or(u8::MAX);
        let n = placed.iter().filter(|&&c| c == here).count();
        accounted += n;
        crate::print!("{CENSUS} core={cpu} threads={n}");
        for (i, &c) in placed.iter().enumerate() {
            if c == here {
                if i < ECHO_SERVERS {
                    crate::print!(" S{i}");
                } else {
                    crate::print!(" T{}", i - ECHO_SERVERS);
                }
            }
        }
        println!();
    }
    // A thread not yet run, or on a core outside the online set (a placement bug).
    let missing = placed.len() - accounted;
    if missing != 0 {
        let unknown = placed.iter().filter(|&&c| c == u8::MAX).count();
        println!(
            "{CENSUS} unplaced={missing} of which not-yet-run={unknown} (online={})",
            smp::online_count()
        );
    }
}
