//! **Multi-hart aggregate throughput** (DECISIONS §28 (SMP placement)): independent pipelines
//! across every core, measured by wall clock.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// Independent ping-pong pipelines. 16 on a 4-hart boot is four per core, enough that placement and
/// stealing fill every core and the aggregate is not starved by too few streams.
const TP_PIPES: usize = 16;
/// Round trips per pipeline in a batch. Large enough that spawn and first-rendezvous costs (paid
/// before the clock starts, behind the GO barrier) are noise against the timed steady state.
pub(super) const TP_RTT: u64 = 2000;
/// Batches per measurement; the minimum ticks is kept (least host contention under HVF).
pub(super) const TP_REPEAT: usize = 4;

static TP_GO: AtomicBool = AtomicBool::new(false);

/// Run `pipes` independent ping-pong pipelines over the endpoint pairs and return the wall-clock
/// ticks to complete them all.
///
/// Threads are spawned first and released together by `TP_GO` after the clock starts, and the
/// caller blocks on `done`, one signal per pipeline. See notes/benchmarks/kernel-bench-harness.md.
pub(super) fn tp_batch(
    req: &[sched::RendezvousId],
    reply: &[sched::RendezvousId],
    done: sched::RendezvousId,
    pipes: usize,
) -> u64 {
    TP_GO.store(false, Ordering::SeqCst);
    let base = sched::thread_count();

    for i in 0..pipes {
        let (rq, rp) = (req[i], reply[i]);
        // The server half starts when its client first sends.
        sched::spawn(move || {
            for _ in 0..TP_RTT {
                let _ = sched::ipc_receive(rq);
                sched::ipc_send(rp, [1, 0, 0]);
            }
        })
        .expect("bench: throughput server spawn failed");

        // Yields, not spins, at the barrier, so a waiting client does not burn its core.
        sched::spawn(move || {
            while !TP_GO.load(Ordering::Acquire) {
                sched::yield_now();
            }
            for _ in 0..TP_RTT {
                sched::ipc_send(rq, [1, 0, 0]);
                let _ = sched::ipc_receive(rp);
            }
            sched::ipc_send(done, [1, 0, 0]);
        })
        .expect("bench: throughput client spawn failed");
    }

    // Blocking, not yield-spinning, keeps the solo pair co-located.
    let t0 = crate::arch::timer::now();
    TP_GO.store(true, Ordering::Release);
    for _ in 0..pipes {
        let _ = sched::ipc_receive(done);
    }
    let ticks = crate::arch::timer::now() - t0;

    // Drained to the baseline, so a batch is never charged the previous one's teardown.
    while sched::thread_count() > base {
        sched::yield_now();
    }
    ticks
}

/// Minimum ticks over `TP_REPEAT` batches of `pipes` pipelines: the least host-contended sample.
pub(super) fn tp_best(
    req: &[sched::RendezvousId],
    reply: &[sched::RendezvousId],
    done: sched::RendezvousId,
    pipes: usize,
) -> u64 {
    let mut best = u64::MAX;
    for _ in 0..TP_REPEAT {
        best = best.min(tp_batch(req, reply, done, pipes));
    }
    best
}

/// Inner iterations per compute worker. Sized so a single worker runs a few hundred microseconds
/// under HVF, long enough that the 41 ns counter grain and host jitter are noise against the batch.
const TC_WORK: u64 = 300_000;

static TC_SINK: AtomicU64 = AtomicU64::new(0);

/// A non-elidable integer grind with no syscalls, so it isolates placement from cross-core wakes.
/// Returns a value the caller sinks, so the optimizer cannot delete the loop.
#[inline(never)]
fn busy(iters: u64) -> u64 {
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    for i in 0..iters {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407 ^ i);
        x ^= x >> 29;
    }
    x
}

/// Run `workers` CPU-bound workers behind the GO barrier and return the wall-clock ticks to finish
/// them all, blocking on `done`. Placement should finish them in about `ceil(N/cores)`
/// worker-times.
fn tc_batch(done: sched::RendezvousId, workers: usize) -> u64 {
    TP_GO.store(false, Ordering::SeqCst);
    let base = sched::thread_count();
    for _ in 0..workers {
        sched::spawn(move || {
            while !TP_GO.load(Ordering::Acquire) {
                sched::yield_now();
            }
            let v = busy(TC_WORK);
            TC_SINK.fetch_add(v, Ordering::Relaxed);
            sched::ipc_send(done, [1, 0, 0]);
        })
        .expect("bench: compute worker spawn failed");
    }
    let t0 = crate::arch::timer::now();
    TP_GO.store(true, Ordering::Release);
    for _ in 0..workers {
        let _ = sched::ipc_receive(done);
    }
    let ticks = crate::arch::timer::now() - t0;
    while sched::thread_count() > base {
        sched::yield_now();
    }
    ticks
}

/// Minimum ticks over `TP_REPEAT` compute batches: the least host-contended sample.
fn tc_best(done: sched::RendezvousId, workers: usize) -> u64 {
    let mut best = u64::MAX;
    for _ in 0..TP_REPEAT {
        best = best.min(tc_batch(done, workers));
    }
    best
}

/// Aggregate multi-hart throughput, the picture of §28 (SMP placement) filling the machine.
///
/// Prints `smp_compute_*` (independent CPU-bound workers) and `smp_pipe_*` (ping-pong pairs), each
/// solo and with every pipeline; the scaling factor is solo ns/iter over all ns/iter. `--real
/// --smp` only, and never gates. Under HVF compute scales and pipelines do not, a property of the
/// host. See notes/benchmarks/kernel-bench-harness.md.
pub(super) fn smp_throughput() {
    let cpus = crate::smp::online_count();
    if cpus <= 1 {
        return; // single hart (the icount instrument): there is no placement win to show.
    }

    // Created once and reused, so repeated batches do not drain the endpoint table.
    let mut req = [0u64; TP_PIPES];
    let mut reply = [0u64; TP_PIPES];
    for i in 0..TP_PIPES {
        req[i] = sched::create_rendezvous();
        reply[i] = sched::create_rendezvous();
    }
    let done = sched::create_rendezvous();

    let _ = tc_batch(done, 1);
    let compute_solo = tc_best(done, 1);
    let compute_all = tc_best(done, TP_PIPES);

    let _ = tp_batch(&req, &reply, done, 1);
    let pipe_solo = tp_best(&req, &reply, done, 1);
    let pipe_all = tp_best(&req, &reply, done, TP_PIPES);

    // Each `*_all` is TP_PIPES times its `*_solo`'s work; `smp_cpus` records the ceiling.
    println!("bench: smp_compute_solo {compute_solo} {TC_WORK}");
    println!(
        "bench: smp_compute_all {compute_all} {}",
        TP_PIPES as u64 * TC_WORK
    );
    println!("bench: smp_pipe_solo {pipe_solo} {TP_RTT}");
    println!(
        "bench: smp_pipe_all {pipe_all} {}",
        TP_PIPES as u64 * TP_RTT
    );
    println!("bench: smp_cpus {cpus} {cpus}");
}
