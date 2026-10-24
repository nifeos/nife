//! **Milestone 134 (the register of measures), tier A, E1: IPC as more threads cycle through it**,
//! and the single-hart check E1 and E4 both run behind.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// The counter frequency QEMU `virt` states under TCG, with or without `-icount`; a real core
/// states its own (24 MHz on the Apple Silicon dev machine). Nothing else inside the guest tells
/// the accelerators apart.
#[cfg(target_arch = "aarch64")]
const TCG_VIRT_CNTFRQ_HZ: u64 = 62_500_000;

/// The riscv64 twin: QEMU `virt`'s `/cpus/timebase-frequency`, against radon's 4 MHz.
///
/// It says "not QEMU `virt`", not "has a 32 KB L1"; the runbook records which machine a number came
/// from. See notes/benchmarks/kernel-bench-harness.md.
#[cfg(target_arch = "riscv64")]
const TCG_VIRT_CNTFRQ_HZ: u64 = 10_000_000;

/// Shared precondition for E1 and E4: a real core, so there is a cache, and a single hart, so every
/// thread contends for one core's cache.
///
/// Returns `false`, printing why `name` skipped, when either does not hold. A board card needs the
/// `single_hart` feature for this. See notes/benchmarks/kernel-bench-harness.md.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
pub(super) fn real_single_hart_or_skip(name: &str) -> bool {
    let cpus = crate::smp::online_count();
    if cpus != 1 {
        println!(
            "bench: {name} skipped (needs a single hart to isolate one core's cache; this boot has {cpus})"
        );
        return false;
    }
    let freq = crate::arch::timer::frequency();
    if freq == TCG_VIRT_CNTFRQ_HZ {
        println!(
            "bench: {name} skipped (QEMU virt detected via counter frequency {freq} Hz; TCG \
             models no cache, see design/roadmap/0134-the-measurements-that-decide.md)"
        );
        return false;
    }
    true
}

/// The most pairs [`ipc_thread_scaling`] (E1) sweeps to: 96 threads, three times past the knee
/// predicted for 16 to 32.
///
/// Fixed so every run compares with the published ones; change it only if the knee moves. See
/// notes/benchmarks/kernel-bench-harness.md.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
pub(super) const SCALE_MAX_PAIRS: usize = 48;
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
pub(super) const SCALE_PAIRS: &[usize] = &[1, 2, 4, 8, 16, 32, SCALE_MAX_PAIRS];

/// E1: IPC round-trip latency against thread count, on one real hart.
///
/// Kernel stacks are `STACK_SLOT_SPAN` apart, so enough threads cycling through IPC stop fitting in
/// L1d. Runs [`tp_best`] at each of [`SCALE_PAIRS`]; a flat line says the process-kernel penalty is
/// invisible on this cache, and a knee gives Warton's effect its magnitude. See
/// notes/benchmarks/kernel-bench-harness.md.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
pub(super) fn ipc_thread_scaling() {
    if !real_single_hart_or_skip("ipc_thread_scaling") {
        return;
    }

    let mut req = [0u64; SCALE_MAX_PAIRS];
    let mut reply = [0u64; SCALE_MAX_PAIRS];
    for i in 0..SCALE_MAX_PAIRS {
        req[i] = sched::create_rendezvous();
        reply[i] = sched::create_rendezvous();
    }
    let done = sched::create_rendezvous();

    for &pairs in SCALE_PAIRS {
        let ticks = tp_best(&req[..pairs], &reply[..pairs], done, pairs);
        let threads = pairs * 2;
        println!(
            "bench: ipc_scale_{threads} {ticks} {}",
            pairs as u64 * TP_RTT
        );
    }
}
