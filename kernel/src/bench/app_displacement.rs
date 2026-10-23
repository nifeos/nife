//! **Milestone 134 (the register of measures), tier A, E4: what an IPC-heavy kernel costs an
//! application's working set.**
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// Working-set sizes for [`app_displacement()`] (E4), in KiB: below, at and above a typical L1d.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
const APPDISP_WORKINGSET_KIB: &[usize] = &[4, 16, 32, 64, 128];
/// Bytes per working-set word; the workload reads and writes `u64`s.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
const APPDISP_WORD_BYTES: usize = 8;
/// The largest working set above, in words: the static scratch buffer's size.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
const APPDISP_MAX_WORDS: usize = 128 * 1024 / APPDISP_WORD_BYTES;
/// Roughly this many word-touches per measurement, regardless of working-set size, so every point
/// in the sweep takes about the same wall time: `passes = TARGET / words`.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
const APPDISP_TARGET_TOUCHES: u64 = 8_000_000;
/// Background IPC pairs during the "with traffic" batch: 16 threads, inside [`SCALE_PAIRS`]'s
/// sweep.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
const APPDISP_IPC_PAIRS: usize = 8;
/// Background IPC pairs for the higher-load condition: [`SCALE_MAX_PAIRS`], 96 threads, past E1's
/// knee. See notes/benchmarks/kernel-bench-harness.md.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
const APPDISP_IPC_PAIRS_HIGH: usize = SCALE_MAX_PAIRS;

/// Interior-mutable scratch the workload reads and writes, one thread at a time, serialized by the
/// caller rather than by a lock that would join the cost being measured.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
struct Racy<T>(core::cell::UnsafeCell<T>);
// SAFETY: access is serialized by `appdisp_batch`: the workload thread is the only one ever handed
// a reference into this buffer, and one batch's workload always finishes (and its reference drops)
// before the next batch's does (see `appdisp_batch`'s reap-to-baseline wait, which runs after
// `appdisp_workload` returns).
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
unsafe impl<T> Sync for Racy<T> {}

#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
static APPDISP_BUFFER: Racy<[u64; APPDISP_MAX_WORDS]> =
    Racy(core::cell::UnsafeCell::new([0; APPDISP_MAX_WORDS]));

#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
static APPDISP_STOP: AtomicBool = AtomicBool::new(false);

/// One background IPC pair: a server that echoes until the sentinel, and a client that
/// sends-then-receives until [`APPDISP_STOP`], then releases the server.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
fn appdisp_background_pair(rq: sched::RendezvousId, rp: sched::RendezvousId) {
    sched::spawn(move || {
        loop {
            let m = sched::ipc_receive(rq);
            if m[0] == u64::MAX {
                break;
            }
            sched::ipc_send(rp, [m[0], 0, 0]);
        }
    })
    .expect("bench: no appdisp background server");
    sched::spawn(move || {
        while !APPDISP_STOP.load(Ordering::Relaxed) {
            sched::ipc_send(rq, [1, 0, 0]);
            sched::ipc_receive(rp);
        }
        sched::ipc_send(rq, [u64::MAX, 0, 0]);
    })
    .expect("bench: no appdisp background client");
}

/// The application: read-modify-write `words` entries of [`APPDISP_BUFFER`], `passes` times.
///
/// Returns its own ticks. Yields about 64 times over the run in every condition, so the background
/// pairs run concurrently and the conditions differ only in what else is ready.
#[inline(never)]
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
fn appdisp_workload(words: usize, passes: u64) -> u64 {
    // SAFETY: the caller (`appdisp_batch`) guarantees this thread is the only one with a reference
    // into the buffer for the duration of this call; see `Racy`'s doc.
    let whole: &mut [u64; APPDISP_MAX_WORDS] = unsafe { &mut *APPDISP_BUFFER.0.get() };
    let buf = &mut whole[..words];
    let yield_every = (passes / 64).max(1);
    let mut sum: u64 = 0;
    let t0 = crate::arch::timer::now();
    for p in 0..passes {
        for (i, word) in buf.iter_mut().enumerate() {
            sum = sum.wrapping_add(*word).wrapping_add(i as u64);
            *word = sum;
        }
        if p % yield_every == 0 {
            sched::yield_now();
        }
    }
    let ticks = crate::arch::timer::now() - t0;
    core::hint::black_box(sum);
    ticks
}

/// One measurement: as many background pairs as `req`/`reply` have slots for (zero for solo),
/// running for as long as [`appdisp_workload`] takes, then released. Returns the workload's ticks.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
fn appdisp_batch(
    words: usize,
    passes: u64,
    req: &[sched::RendezvousId],
    reply: &[sched::RendezvousId],
) -> u64 {
    APPDISP_STOP.store(false, Ordering::SeqCst);
    let base = sched::thread_count();
    for i in 0..req.len() {
        appdisp_background_pair(req[i], reply[i]);
    }
    // The pairs reach their first blocking RECEIVE before the clock starts.
    for _ in 0..8 {
        sched::yield_now();
    }
    let ticks = appdisp_workload(words, passes);
    APPDISP_STOP.store(true, Ordering::SeqCst);
    while sched::thread_count() > base {
        sched::yield_now();
    }
    ticks
}

/// Batches per measurement point, the minimum kept as in [`tp_best`]; one more than
/// [`TP_REPEAT`], because a long batch catches more host preemptions.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
const APPDISP_REPEAT: usize = 5;

/// Minimum ticks over [`APPDISP_REPEAT`] batches: the least host-contended sample.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
fn appdisp_best(
    words: usize,
    passes: u64,
    req: &[sched::RendezvousId],
    reply: &[sched::RendezvousId],
) -> u64 {
    let mut best = u64::MAX;
    for _ in 0..APPDISP_REPEAT {
        best = best.min(appdisp_batch(words, passes, req, reply));
    }
    best
}

/// E4: what an IPC-heavy kernel costs an unrelated application's working set.
///
/// Times [`appdisp_workload`] over each of [`APPDISP_WORKINGSET_KIB`] alone, with
/// [`APPDISP_IPC_PAIRS`] background pairs on the same core, and with [`APPDISP_IPC_PAIRS_HIGH`];
/// the throughput lost between conditions is the number. Skips off a real single hart. See
/// notes/benchmarks/kernel-bench-harness.md and notes/register-of-measures/cache-experiments.md.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
pub(super) fn app_displacement() {
    if !real_single_hart_or_skip("app_displacement") {
        return;
    }

    let mut req = [0u64; APPDISP_IPC_PAIRS_HIGH];
    let mut reply = [0u64; APPDISP_IPC_PAIRS_HIGH];
    for i in 0..APPDISP_IPC_PAIRS_HIGH {
        req[i] = sched::create_rendezvous();
        reply[i] = sched::create_rendezvous();
    }

    for &kib in APPDISP_WORKINGSET_KIB {
        let words = kib * 1024 / APPDISP_WORD_BYTES;
        let passes = (APPDISP_TARGET_TOUCHES / words as u64).max(1);
        let solo = appdisp_best(words, passes, &[], &[]);
        let with_ipc = appdisp_best(
            words,
            passes,
            &req[..APPDISP_IPC_PAIRS],
            &reply[..APPDISP_IPC_PAIRS],
        );
        let with_ipc_high = appdisp_best(words, passes, &req, &reply);
        println!("bench: appdisp_{kib}k_solo {solo} {passes}");
        println!("bench: appdisp_{kib}k_ipc {with_ipc} {passes}");
        println!("bench: appdisp_{kib}k_ipc96 {with_ipc_high} {passes}");
        let lost_pct = |busy: u64| {
            if solo > 0 {
                ((busy as i64 - solo as i64) * 100) / solo as i64
            } else {
                0
            }
        };
        println!(
            "bench-probe: appdisp_{kib}k_throughput_lost_pct {}",
            lost_pct(with_ipc)
        );
        println!(
            "bench-probe: appdisp_{kib}k_highload_throughput_lost_pct {}",
            lost_pct(with_ipc_high)
        );
    }
}
