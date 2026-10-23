//! **How many CPU cycles one tick of this machine's clock is**, and what that ratio means on
//! each architecture.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// Print what one tick of this machine's reference clock costs in CPU cycles, as a probe.
///
/// Core cycles over reference ticks across one timed spin, with a `cycles_per_tick_means` line
/// naming both counters on this architecture. Prints the reason instead when the cycle counter is
/// unavailable or the spin migrated. See notes/benchmarks/kernel-bench-harness.md and
/// notes/pmu.md.
pub(super) fn cycles_per_tick() {
    // Checked at each end: on aarch64 each core's cycle counter is its own number.
    let cpu = crate::cpu::id();
    let Some(c0) = crate::arch::pmu::cycles() else {
        // Never a fallback to the reference clock, which on x86_64 would print an exact 1.00.
        println!(
            "bench-probe: cycles_per_tick unavailable ({:?})",
            crate::arch::pmu::outcome()
        );
        return;
    };

    let t0 = crate::arch::timer::now();
    while crate::arch::timer::now() - t0 < cycle_probe_window_ticks() {
        core::hint::spin_loop();
    }
    let t1 = crate::arch::timer::now();
    let c1 = crate::arch::pmu::cycles().expect("the counter did not vanish mid-window");
    if crate::cpu::id() != cpu {
        println!(
            "bench-probe: cycles_per_tick unavailable (the window moved cores; per-core counters)"
        );
        return;
    }

    let ticks = t1 - t0;
    let cycles = cycle_probe_delta(c0, c1);
    // Hundredths, since the ratio is under one on an emulator and in the hundreds on silicon.
    let hundredths = cycles.saturating_mul(100).checked_div(ticks).unwrap_or(0);
    println!(
        "bench-probe: cycles_per_tick {}.{:02} ({cycles} cycles over {ticks} ticks at cntfrq {})",
        hundredths / 100,
        hundredths % 100,
        crate::arch::timer::frequency(),
    );
    println!("bench-probe: cycles_per_tick_means {CYCLE_PROBE_MEANING}");
}

/// **What the ratio above is a ratio of**, in this machine's own vocabulary. Printed rather than
/// left to a note, because the number leaves the machine and the note does not travel with it.
#[cfg(target_arch = "riscv64")]
const CYCLE_PROBE_MEANING: &str = "core cycles (SBI PMU CPU_CYCLES) per tick of the `time` CSR, a fixed-rate timebase the device \
     tree states";

/// The aarch64 twin. It names the exception levels counted
/// (`arch::pmu::PMCCFILTR_COUNT_EL0_AND_EL1`, calef's ruling A1, 2026-10-07 UTC), since a system
/// that counted others measured a different quantity.
#[cfg(target_arch = "aarch64")]
const CYCLE_PROBE_MEANING: &str = "cycles (PMCCNTR_EL0, PMCCFILTR_EL0 0: EL0+EL1 counted, EL2 not) per tick of \
     CNTVCT_EL0, the generic timer at CNTFRQ_EL0";

/// The `x86_64` twin, and the difference a cross-architecture reader has to know: the denominator
/// here is the TSC, which is constant-rate, so the ratio moves with the core's frequency where
/// riscv64's does not.
#[cfg(target_arch = "x86_64")]
const CYCLE_PROBE_MEANING: &str = "unhalted core cycles (IA32_PERF_FIXED_CTR1) per TSC tick; the TSC is constant-rate and core \
     cycles are not, so this ratio moves with frequency scaling and turbo";

/// Ticks to spin for: 10 ms on QEMU and 25 ms on radon, a constant so radon runs stay comparable
/// with the one on record. See notes/benchmarks/kernel-bench-harness.md.
#[cfg(target_arch = "riscv64")]
fn cycle_probe_window_ticks() -> u64 {
    100_000
}

/// Ten milliseconds of generic timer, whatever `CNTFRQ_EL0` says: 62.5 MHz on QEMU and 19.2 MHz on
/// argon, so a constant would be a different window on each.
#[cfg(target_arch = "aarch64")]
fn cycle_probe_window_ticks() -> u64 {
    crate::arch::timer::frequency() / 100
}

/// Ten milliseconds of TSC, at the rate this kernel measured for this part (`arch::x86_64::timer`).
#[cfg(target_arch = "x86_64")]
fn cycle_probe_window_ticks() -> u64 {
    crate::arch::timer::frequency() / 100
}

/// The difference between two 64-bit counter reads, wrapping.
#[cfg(any(target_arch = "riscv64", target_arch = "aarch64"))]
fn cycle_probe_delta(first: u64, second: u64) -> u64 {
    second.wrapping_sub(first)
}

/// The `x86_64` twin, at the counter width `CPUID` reported (commonly 48 bits), so a wrap
/// mid-window does not print a difference near 2^64.
#[cfg(target_arch = "x86_64")]
fn cycle_probe_delta(first: u64, second: u64) -> u64 {
    crate::arch::pmu::elapsed_cycles(first, second)
}
