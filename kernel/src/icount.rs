//! The instruction-count instrument: a boot that asserts timer claims in instructions.
//!
//! Under `-icount shift=0,sleep=off` QEMU's virtual clock advances one nanosecond per retired
//! instruction, so a busy host cannot move these numbers. [`run`] asserts four claims: the timer
//! fired at the armed deadline, the handler stays under a bound, no tick is missed, and the deadline
//! advances one interval per tick. Its own boot, `-smp 1`, behind the `icount` feature, which
//! implies `bench`. See notes/instruction-clock.md.
//!
//! *Provisional names throughout: the `icount` feature, this module, `tick_trace`,
//! `script/icount`.*

use crate::println;

/// Timer ticks to sample: enough for a maximum to mean something, and 0.64 s of virtual time, the
/// boot's dominant cost.
const SAMPLE_TICKS: u64 = 64;

/// Iterations of the calibration loop, two instructions each on both ISAs: a 2,000,000-instruction
/// window, usually inside one 10 ms tick period.
const CALIBRATION_ITERS: u64 = 1_000_000;

/// Per-tick timing, recorded from the timer handler itself.
///
/// Relaxed stores and read-modify-writes only, which is DECISIONS §9's rule for handlers. Compiled
/// in only under `icount`, so no other handler carries it. One set of counters: the boot runs
/// `-smp 1`, and [`record`] drops any hart but the boot one.
pub mod tick_trace {
    use core::sync::atomic::{AtomicU64, Ordering};

    static COUNT: AtomicU64 = AtomicU64::new(0);
    static ARRIVAL_SUM: AtomicU64 = AtomicU64::new(0);
    static ARRIVAL_MAX: AtomicU64 = AtomicU64::new(0);
    static ARRIVAL_MIN: AtomicU64 = AtomicU64::new(u64::MAX);
    static HANDLER_SUM: AtomicU64 = AtomicU64::new(0);
    static HANDLER_MAX: AtomicU64 = AtomicU64::new(0);
    static EARLY: AtomicU64 = AtomicU64::new(0);

    /// Record one tick's numbers, all in counter ticks.
    ///
    /// `fired` is the deadline this interrupt was armed against (`CNTV_CVAL_EL0` on aarch64, the
    /// `DEADLINE` word handed to SBI on riscv64), `arrival` the counter as the handler first read
    /// it, and `done` the counter after the next deadline is armed. Called from `rearm`.
    ///
    /// Context: the timer interrupt handler; takes no lock.
    pub(crate) fn record(fired: u64, arrival: u64, done: u64) {
        if crate::cpu::id() != 0 {
            return;
        }
        // An early arrival is a finding about the machine, counted apart rather than averaged.
        let Some(arrival_latency) = arrival.checked_sub(fired) else {
            EARLY.fetch_add(1, Ordering::Relaxed);
            return;
        };
        let handler = done.wrapping_sub(fired);

        COUNT.fetch_add(1, Ordering::Relaxed);
        ARRIVAL_SUM.fetch_add(arrival_latency, Ordering::Relaxed);
        ARRIVAL_MAX.fetch_max(arrival_latency, Ordering::Relaxed);
        ARRIVAL_MIN.fetch_min(arrival_latency, Ordering::Relaxed);
        HANDLER_SUM.fetch_add(handler, Ordering::Relaxed);
        HANDLER_MAX.fetch_max(handler, Ordering::Relaxed);
    }

    /// Ticks recorded since the last [`reset`].
    pub(crate) fn count() -> u64 {
        COUNT.load(Ordering::Relaxed)
    }

    /// Interrupts that arrived before their armed deadline. Nonzero means the emulator, not the
    /// kernel.
    pub(crate) fn early() -> u64 {
        EARLY.load(Ordering::Relaxed)
    }

    /// `(count, arrival_min, arrival_mean, arrival_max, handler_mean, handler_max)`, in counter
    /// ticks. Means are integer divisions; the caller converts to instructions.
    pub(crate) fn snapshot() -> (u64, u64, u64, u64, u64, u64) {
        let n = COUNT.load(Ordering::Relaxed).max(1);
        (
            COUNT.load(Ordering::Relaxed),
            ARRIVAL_MIN.load(Ordering::Relaxed),
            ARRIVAL_SUM.load(Ordering::Relaxed) / n,
            ARRIVAL_MAX.load(Ordering::Relaxed),
            HANDLER_SUM.load(Ordering::Relaxed) / n,
            HANDLER_MAX.load(Ordering::Relaxed),
        )
    }

    /// Drop everything recorded so far, so bring-up ticks are not in the sample.
    pub fn reset() {
        COUNT.store(0, Ordering::Relaxed);
        ARRIVAL_SUM.store(0, Ordering::Relaxed);
        ARRIVAL_MAX.store(0, Ordering::Relaxed);
        ARRIVAL_MIN.store(u64::MAX, Ordering::Relaxed);
        HANDLER_SUM.store(0, Ordering::Relaxed);
        HANDLER_MAX.store(0, Ordering::Relaxed);
        EARLY.store(0, Ordering::Relaxed);
    }
}

/// Instructions per counter tick under `-icount shift=0`: `1e9 / frequency()`, 16 on aarch64 and
/// 100 on riscv64.
///
/// Panics unless the frequency divides 1 GHz, since otherwise every number would be rounded.
fn instructions_per_tick() -> u64 {
    let freq = crate::arch::timer::frequency();
    assert!(freq > 0, "icount: the counter frequency is zero");
    assert!(
        1_000_000_000u64.is_multiple_of(freq),
        "icount: the counter frequency ({freq} Hz) does not divide 1 GHz, so a counter tick is not \
         a whole number of instructions and every number this boot prints would be rounded"
    );
    1_000_000_000 / freq
}

/// Panic unless this boot is running on the instrument.
///
/// Booted without `-icount shift=0` every number would be wall clock in instruction units. A loop
/// of a known instruction count must advance virtual time by that many nanoseconds, within 1%,
/// which one tick admits and plain TCG or HVF does not (notes/instruction-clock.md).
fn calibrate(per_tick: u64) {
    let before = crate::arch::timer::now();
    let expected = crate::arch::timer::calibration_loop(CALIBRATION_ITERS);
    let observed = (crate::arch::timer::now() - before) * per_tick;

    println!("icount: calibration {observed} {expected}");
    let slack = expected / 100;
    assert!(
        observed >= expected - slack && observed <= expected + slack,
        "icount: this boot is not running under -icount shift=0. A loop of exactly {expected} \
         instructions advanced virtual time by {observed} ns, and on the instrument those two are \
         the same number. Run it through `script/icount`, which passes the flag."
    );
}

/// Run the four claims and park. Never returns and never semihosts: `xtask` owns the QEMU child.
pub fn run() -> ! {
    let per_tick = instructions_per_tick();
    println!();
    println!("icount: cntfrq {}", crate::arch::timer::frequency());
    println!("icount: instructions_per_counter_tick {per_tick}");
    println!(
        "icount: tick_interval {} {}",
        crate::arch::timer::interval(),
        crate::arch::timer::interval() * per_tick
    );

    calibrate(per_tick);

    tick_trace::reset();
    let missed_before = crate::arch::timer::missed_ticks();

    // A consistent (ticks, deadline) pair for claim 4: re-read until no tick landed between the
    // two reads. On `-smp 1` the handler never leaves them half-updated.
    let snapshot = || loop {
        let t = tick_trace::count();
        let d = crate::arch::timer::deadline();
        if tick_trace::count() == t {
            break (t, d);
        }
    };

    let (ticks_before, deadline_before) = snapshot();
    while tick_trace::count() < ticks_before + SAMPLE_TICKS {
        core::hint::spin_loop();
    }
    let (ticks_after, deadline_after) = snapshot();
    let missed = crate::arch::timer::missed_ticks() - missed_before;
    let ticks_measured = ticks_after - ticks_before;
    let deadline_delta = deadline_after - deadline_before;

    let (count, arrival_min, arrival_mean, arrival_max, handler_mean, handler_max) =
        tick_trace::snapshot();
    let early = tick_trace::early();

    println!("icount: ticks {count}");
    println!(
        "icount: arrival_instructions min {} mean {} max {}",
        arrival_min * per_tick,
        arrival_mean * per_tick,
        arrival_max * per_tick
    );
    println!(
        "icount: handler_instructions mean {} max {}",
        handler_mean * per_tick,
        handler_max * per_tick
    );
    println!("icount: missed_ticks {missed}");
    println!("icount: early_arrivals {early}");
    println!(
        "icount: deadline_delta {} expected {}",
        deadline_delta,
        ticks_measured * crate::arch::timer::interval()
    );

    // Claim 1: the timer fired at the deadline the kernel armed. `ARRIVAL_BOUND` is the arch's,
    // because the trap path is.
    assert_eq!(
        early, 0,
        "icount: {early} interrupts arrived BEFORE the deadline they were armed against, which is \
         the emulator rather than this kernel"
    );
    assert!(
        arrival_max * per_tick <= crate::arch::timer::ARRIVAL_BOUND,
        "icount: an interrupt arrived {} instructions after the deadline it was armed against, over \
         a bound of {}. On this instrument that is the kernel's own path length, not the host: \
         either the trap path grew, or the timer was armed with something other than the deadline \
         the kernel recorded.",
        arrival_max * per_tick,
        crate::arch::timer::ARRIVAL_BOUND
    );

    // Claim 2: deadline to re-armed, the span `MISSED_TICKS` only approximates.
    assert!(
        handler_max * per_tick <= crate::arch::timer::HANDLER_BOUND,
        "icount: the timer handler took {} instructions from deadline to re-arm, over a bound of \
         {}. Nothing about the host can move this number.",
        handler_max * per_tick,
        crate::arch::timer::HANDLER_BOUND
    );

    // Claim 3: virtual time has no deschedules, so a miss can only be the handler.
    assert_eq!(
        missed, 0,
        "icount: {missed} deadlines had already passed when the handler re-armed. Under -icount \
         the host cannot cause that, so it is the handler taking longer than a whole tick period."
    );

    // Claim 4: one interval per delivered tick. Claims 1 to 3 cannot see a grid re-anchored from
    // `now`. The law holds only over a miss-free window, which claim 3 just asserted.
    assert_eq!(
        deadline_delta,
        ticks_measured * crate::arch::timer::interval(),
        "icount: {ticks_measured} ticks moved the armed deadline off the grid. Over a window with \
         no missed tick the deadline advances by exactly one interval per tick; re-arming relative \
         to `now` inside the handler instead of from the deadline that fired does exactly this, and \
         nothing else in this boot can see it."
    );

    println!("icount: done");
    crate::arch::halt(crate::arch::HaltReason::measurement_boot())
}
