//! **The timer, `x86_64`.** A calibrated TSC for reading time, and the local APIC timer for the tick.
//!
//! aarch64's `CNTFRQ_EL0` and RISC-V's device tree state their counter's rate; x86 does not on the
//! parts this runs on (`CPUID` leaf 0x15 is often absent, and nothing reports the APIC timer's bus
//! clock). So both rates are measured, from one shared wait, against the one fixed-rate device on a
//! PC: the 8254 PIT at 1193182 Hz, over ten-millisecond windows timed by polling.
//!
//! **Several windows, and the smallest wins.** Polling sees the PIT's terminal count late, never
//! early, so every window is an upper bound on the true rate and one descheduled window inflated it
//! by up to 1153%. The minimum is the tightest bound; an average would be biased.
//! [`CALIBRATION_WINDOW_CAP`] has the measured distributions, and
//! [`CALIBRATION_AGREEMENT_PARTS_PER`] why the boot usually stops after three or four windows. The
//! APIC timer's rate is the minimum over the same windows, taken independently.
//!
//! # BUGS
//!
//! - The TSC is assumed invariant; `arch::x86_64::isa::init` refuses a part whose
//!   `CPUID.80000007H:EDX[8]` says otherwise, untested on silicon. Under QEMU on an aarch64 host
//!   the TSC is the host's nanosecond clock (notes/tsc-under-tcg.md), which does not carry to x86.
//! - Min-of-N is a filter, not a bound: a host that never leaves this vCPU alone for ten
//!   milliseconds still inflates the rate. The boot line prints the worst window beside the chosen
//!   one, so a fought calibration shows a gap. A part reporting leaf 0x15 skips all of this
//!   (`arch::x86_64::isa::tsc_crystal_hz`).
//! - Under `-icount shift=0,sleep=off`, `script/bench --x86`'s default, the TSC is not wall time:
//!   its rate follows the instruction stream.
//! - `now()` is the calling core's TSC, and nothing measures or corrects skew between cores,
//!   unlike aarch64's and RISC-V's system counters. A `kernel::user::wait_for` deadline that spans
//!   a migration is only as good as that agreement. QEMU's skew is zero, so no test sees it.
//! - Calibration costs boot time no other architecture pays: about 35 ms on a quiet host, up to
//!   160 ms at the cap.
//! - `init` panics if the local APIC is not up, because the order (APIC, then timer) is real.

use core::sync::atomic::{AtomicU64, Ordering};

use super::irq;
use super::port::{in8, out8};

/// Ticks per second, the preemption rate: `abi::timer::TICK_HZ`, which every architecture takes
/// so that user space can know a timed wait's resolution.
pub const TICK_HZ: u64 = abi::timer::TICK_HZ;

/// **The 8254 PIT's input frequency**, 1193182 Hz, and the one rate on a PC that is a fixed number
/// rather than something to measure. It is 315/22 MHz divided by 12, which is to say it descends
/// from the NTSC colour-burst crystal that made 1981's parts cheap.
const PIT_HZ: u64 = 1_193_182;

/// How long one calibration window is. Ten milliseconds is a compromise: long enough that the
/// polling loop's own overhead is noise, short enough that the PIT's 16-bit counter holds it (its
/// maximum is about 54.9 ms) and that boot does not visibly pause.
const CALIBRATION_MS: u64 = 10;

/// **The most windows that will ever be timed.** The answer is the smallest of the ones taken, and
/// [`CALIBRATION_AGREEMENT_PARTS_PER`] usually stops it long before this.
///
/// One window was the original design and it is wrong by an unbounded amount: see this module's
/// `BUGS`, and notes/tsc-under-tcg.md for the sweep that measured 1001 MHz to 4330 MHz against a
/// counter known to tick at 1000.000 MHz. The error is **one-sided by construction**, because the
/// poll can only notice the PIT's terminal count late and the whole TSC delta is then attributed
/// to exactly [`CALIBRATION_MS`]. That is what makes the *minimum* the right estimator and an
/// average the wrong one: every sample is an upper bound on the truth, so the smallest is the
/// tightest bound available and more samples can only tighten it.
///
/// **This is a cap rather than a count, and it is where the accuracy actually comes from.** The
/// stopping rule below decides when to stop early; this decides how bad a host the calibration can
/// still survive, because on a host that keeps descheduling the vCPU every window is inflated and
/// the only remedy is more of them.
///
/// **Sixteen is measured, not assumed** (the `calib` lane, 2026-09-21). calef's ruling named the
/// shape, "fix the calibration with min-of-five windows", and left the count to be chosen
/// honestly. Two hundred boots on a host deliberately saturated to load 30 on eight cores, each
/// timing sixteen windows and reporting every one, give the error of the min over the first k:
///
/// | Cap | Median error | 99th percentile | Worst of 200 | Boots worse than 1% |
/// |---|---|---|---|---|
/// | 1 (what shipped) | +0.36% | +884% | **+1153%** | 56 / 200 |
/// | 3 | +0.00% | +120% | +131% | 24 / 200 |
/// | 5 | +0.00% | +20.6% | +51.4% | 11 / 200 |
/// | 9 | +0.00% | +0.49% | +7.1% | 2 / 200 |
/// | 16 | +0.00% | +0.01% | +0.47% | **0 / 200** |
///
/// **Five would have been the wrong number to ship**, and this is what the ruling asked to be
/// checked rather than assumed: at this load a cap of five still leaves one boot in eighteen wrong
/// by more than a per cent and a worst case of +51%. The tail only closes at sixteen. What makes
/// that affordable rather than a 160 ms boot tax is the stopping rule below: the *mean* number of
/// windows actually timed is 5.20 here and 3.5 on a quiet host, so the ruling's five is what this
/// costs, and sixteen is only what it is willing to spend when the host is fighting.
const CALIBRATION_WINDOW_CAP: usize = 16;

/// **When to stop taking windows**, as the reciprocal of the tolerance: two windows within one
/// part in this of each other end the calibration.
///
/// The reasoning is the one-sidedness again, used the other way round. A window is inflated by the
/// host descheduling the vCPU inside it, which is an event of essentially arbitrary size; for two
/// windows to land within a thousandth of each other they must both have been left alone, because
/// two independent deschedulings agreeing to three decimal places is not a thing that happens. So
/// *agreement is the evidence of cleanliness*, and once there is evidence the remaining windows
/// buy nothing but boot time.
///
/// Measured over 490 boots at three host loads, this reaches **exactly** the accuracy of taking
/// the full cap every time, boot for boot, while averaging **3.5 windows on a quiet host and 5.2
/// on one saturated to load 30**. A tighter tolerance (one part in 2000) changed the mean by 0.07
/// windows and the error distribution not at all, which says the choice is not delicate: the clean
/// windows agree to within the PIT's own quantisation and the dirty ones are nowhere near.
const CALIBRATION_AGREEMENT_PARTS_PER: u64 = 1000;

/// PIT channel 2's data port. Channel 2 is the one to use, and the reason is that it is the only
/// channel whose **gate is under software control** (port 0x61 bit 0) and whose **output can be
/// polled** (bit 5). Channels 0 and 1 are wired to the interrupt controller and to DRAM refresh, so
/// timing against either means taking interrupts, which is what this runs before.
const PIT_CHANNEL2: u16 = 0x42;
/// The PIT's mode/command register.
const PIT_COMMAND: u16 = 0x43;
/// The "system control port B" that gates channel 2 and reports its output. Bit 0 is the gate, bit 1
/// is the speaker (which stays off), and bit 5 reads channel 2's output.
const PIT_GATE_PORT: u16 = 0x61;

/// Channel 2, access mode lobyte/hibyte, operating mode 0 (interrupt on terminal count), binary.
/// Mode 0 is the one that counts down once and raises its output line, which is exactly a
/// one-shot stopwatch.
const PIT_CHANNEL2_ONESHOT: u8 = 0b1011_0010;

/// PIT **channel 0**'s data port. Channel 0 is the one whose output is wired to the interrupt
/// controller, which is why calibration cannot use it (it would mean taking interrupts) and why
/// proving the IO APIC works can use nothing else.
const PIT_CHANNEL0: u16 = 0x40;

/// Channel 0, access mode lobyte/hibyte, operating mode 2 (rate generator), binary. Mode 2 pulses
/// the output line once per reload and then reloads itself, which is a periodic interrupt source
/// rather than the one-shot mode 0 the calibration uses.
const PIT_CHANNEL0_RATE: u8 = 0b0011_0100;

/// **The legacy ISA IRQ number channel 0's output carries.** Zero, on every PC.
///
/// It is emphatically **not** the IO APIC input the line arrives on: the MADT's interrupt source
/// overrides say the PIT is wired to global system interrupt 2, because pin 0 carries the 8259
/// cascade. See `arch/x86_64/irq.rs`.
pub(crate) const PIT_IRQ: u32 = 0;

/// The TSC's frequency in hertz, established by [`init_frequency`]. Zero until then.
static TSC_HZ: AtomicU64 = AtomicU64::new(0);

/// Whether [`TSC_HZ`] came from `CPUID` leaf `0x15` (`true`) or from PIT calibration (`false`).
/// Set by [`init_frequency`] alongside `TSC_HZ` itself; meaningless before that (reads `false`,
/// the calibration path's own value, which is also what every QEMU boot actually takes: see
/// `arch::x86_64::isa::tsc_crystal_hz`'s own docs for why leaf 0x15 is empirically unavailable
/// under `-cpu max`).
static TSC_HZ_FROM_CPUID: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// The local APIC timer's frequency in hertz, at the divider `irq` programs. Zero until measured.
static APIC_TIMER_HZ: AtomicU64 = AtomicU64::new(0);

/// **Every window's implied TSC rate, kept rather than discarded**, in the order they were timed.
/// Only the first [`CALIBRATION_TAKEN`] entries mean anything; the rest are zero.
///
/// The spread across these is what says whether this boot's calibration was clean or whether the
/// host was fighting it, and it is free once the windows have been taken. Keeping it is rung three
/// of AGENTS.md's ladder: a bad calibration used to be silent, and a boot line that prints the
/// worst window next to the chosen one makes it something a reader meets. [`calibration`] is the
/// accessor.
static CALIBRATION_SAMPLES: [AtomicU64; CALIBRATION_WINDOW_CAP] =
    [const { AtomicU64::new(0) }; CALIBRATION_WINDOW_CAP];

/// How many of [`CALIBRATION_SAMPLES`] were actually timed before the stopping rule fired. Zero
/// until [`init_frequency`] runs, which is also how [`calibration`] reports "not measured yet".
static CALIBRATION_TAKEN: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// Scheduler ticks taken since the timer was armed.
static TICKS: AtomicU64 = AtomicU64::new(0);

/// Read the time-stamp counter.
///
/// `rdtsc` returns a 64-bit value in `edx:eax`, which is why this is two 32-bit outputs shifted
/// together rather than one register. **Not serialising**: the CPU may execute it out of order with
/// respect to surrounding instructions, so a measurement of a short interval wants `lfence` or
/// `rdtscp` around it. The calibration below measures ten milliseconds, where a few tens of cycles
/// of reordering is not measurable, so it does not pay for the fence.
fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    // SAFETY: reads a counter. No memory effect, no flag effect.
    unsafe {
        core::arch::asm!(
            "rdtsc",
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags),
        );
    }
    ((high as u64) << 32) | low as u64
}

/// Wait `CALIBRATION_MS` by polling PIT channel 2, and return what the TSC and the APIC timer did
/// across it.
///
/// Returns `(tsc_delta, apic_delta)`. The APIC timer **counts down**, so its delta is
/// `start - end`; a caller that got the subtraction the other way round would compute an enormous
/// frequency and a timer that never fires, which is why it is done here rather than at the call
/// site.
fn measure_against_the_pit() -> (u64, u32) {
    let count = (PIT_HZ * CALIBRATION_MS / 1000) as u16;

    // Start the APIC timer as a free-running masked counter from its maximum, so it cannot wrap
    // during a window this short: at any plausible bus clock, 0xffffffff ticks divided by 16 is
    // minutes, not milliseconds.
    irq::start_timer_for_calibration(u32::MAX);
    let apic_start = irq::timer_current_count();

    // SAFETY: the PIT and its gate port are fixed ISA devices present on every x86 machine. The
    // speaker bit is explicitly cleared, so nothing audible happens; the gate is toggled low then
    // high, which is what arms channel 2's countdown.
    let tsc_start = unsafe {
        let gate = in8(PIT_GATE_PORT) & 0xfc; // clear the gate and the speaker
        out8(PIT_GATE_PORT, gate); // gate low: the channel is held
        out8(PIT_COMMAND, PIT_CHANNEL2_ONESHOT);
        out8(PIT_CHANNEL2, count as u8);
        out8(PIT_CHANNEL2, (count >> 8) as u8);
        let t = rdtsc();
        out8(PIT_GATE_PORT, gate | 1); // gate high: it starts counting
        t
    };

    // Poll channel 2's output. Mode 0 holds it low while counting and raises it at terminal count.
    // Bounded by nothing, deliberately: this is before any interrupt exists, the PIT is not
    // optional on a PC, and a bound here would silently produce a wrong frequency rather than a
    // visible hang. If it never returns, the machine has no PIT and that is worth finding out
    // loudly.
    // SAFETY: reading the gate port, which has no read side effects.
    while unsafe { in8(PIT_GATE_PORT) } & 0x20 == 0 {
        core::hint::spin_loop();
    }

    let tsc_end = rdtsc();
    let apic_end = irq::timer_current_count();

    // SAFETY: put the gate back down so channel 2 is not left running.
    unsafe {
        let gate = in8(PIT_GATE_PORT) & 0xfc;
        out8(PIT_GATE_PORT, gate);
    }

    (
        tsc_end.wrapping_sub(tsc_start),
        apic_start.wrapping_sub(apic_end),
    )
}

/// **Have enough windows been timed to stop?** True once two of `samples` agree to within one
/// part in [`CALIBRATION_AGREEMENT_PARTS_PER`] of the smallest.
///
/// Split out from [`init_frequency`] because it is the only part of the calibration that is a
/// decision rather than a device access, and so the only part a host test can reach: everything
/// around it is `in8` and `out8` against the PIT. See `super::timer_calibration_tests`.
///
/// The comparison is against the running **minimum** rather than the previous sample, because the
/// pair that agrees need not be adjacent: a boot whose second window is descheduled and whose
/// third matches the first is finished at three.
pub(super) fn calibration_has_converged(samples: &[u64]) -> bool {
    let Some(&best) = samples.iter().min() else {
        return false;
    };
    let tolerance = best / CALIBRATION_AGREEMENT_PARTS_PER;
    samples.iter().filter(|&&s| s <= best + tolerance).count() >= 2
}

/// **Establish the TSC's rate and measure the local APIC timer against the PIT.**
///
/// Takes the portable arch contract's boot-info-pointer argument, which x86 ignores; the numbers
/// come from the machine rather than from a table, which is the shape of the difference this
/// module's header is about.
///
/// **The TSC rate itself is asked for before it is measured** (milestone 161's `cntfrq`
/// follow-up): `isa::tsc_crystal_hz` reads `CPUID` leaf `0x15` first, and only the PIT-measured
/// delta is used if that comes back `None`. The local APIC timer's rate has no `CPUID`
/// equivalent at all, so the PIT windows always run regardless of which source wins the TSC
/// number; the same windows price both. See [`crate::arch::x86_64::isa::tsc_crystal_hz`] for why
/// this project's own QEMU invocation always takes the calibrated path.
///
/// # Panics
/// If the local APIC is not up. The order is APIC then timer, and a timer calibrated against a
/// counter that is not running would produce a plausible TSC frequency and a nonsense tick period.
pub(crate) fn init_frequency(boot_info_pointer: usize) {
    let _ = boot_info_pointer;
    assert!(
        irq::is_local_apic_ready(),
        "the timer calibrates the local APIC's counter, so the APIC must be up first",
    );

    let per_second = 1000 / CALIBRATION_MS;

    // **The smallest window, not the only one.** Each window's error is non-negative (the poll
    // notices the terminal count late, never early), so each sample is an upper bound on the true
    // rate and the minimum is the tightest bound taken. An average would be a *biased* estimator
    // here for exactly the same reason: it would carry every descheduling into the answer instead
    // of discarding them. See `CALIBRATION_WINDOW_CAP` for the measured distributions.
    //
    // The TSC's minimum and the APIC timer's minimum are taken independently rather than both
    // being read off whichever window was shortest. In practice that is the same window, because
    // both deltas are proportional to the window's real duration; taking them separately costs
    // nothing and means neither number can be dragged up by the other's outlier.
    let mut samples = [0u64; CALIBRATION_WINDOW_CAP];
    let mut apic_delta = u32::MAX;
    let mut taken = 0;
    while taken < CALIBRATION_WINDOW_CAP {
        let (tsc, apic) = measure_against_the_pit();
        samples[taken] = tsc;
        apic_delta = apic_delta.min(apic);
        taken += 1;
        if calibration_has_converged(&samples[..taken]) {
            break;
        }
    }
    let tsc_delta = samples[..taken].iter().copied().min().unwrap_or(0);

    // Published in hertz, which is the unit every reader of these wants and the unit the loop
    // above deliberately does not work in: converging on raw deltas keeps the tolerance exact
    // rather than comparing numbers that have each been multiplied and rounded.
    for (slot, delta) in CALIBRATION_SAMPLES.iter().zip(samples) {
        slot.store(delta * per_second, Ordering::Relaxed);
    }
    CALIBRATION_TAKEN.store(taken, Ordering::Relaxed);

    APIC_TIMER_HZ.store(u64::from(apic_delta) * per_second, Ordering::Relaxed);

    let (hz, from_cpuid) = match super::isa::tsc_crystal_hz() {
        Some(hz) => (hz, true),
        None => (tsc_delta * per_second, false),
    };

    // **The weakest of the three checks became the same check as the other two** (2026-09-21). This
    // number had no validation at all: `frequency` asserted only that it was nonzero, which a
    // calibration that came back garbage passes. Calibration is a *measurement*, against a PIT, on a
    // vCPU the host may deschedule mid-window, so this is the one of the three rates that can be
    // wrong without anything being broken. A rate outside the band is not a machine this kernel can
    // time anything on; see `counter_frequency_contract::is_plausible` for where the bounds come
    // from.
    assert!(
        counter_frequency_contract::is_plausible(hz),
        "the TSC rate came back as {hz} Hz ({}), which no real part runs at",
        if from_cpuid {
            "CPUID leaf 0x15"
        } else {
            "PIT calibration"
        }
    );

    TSC_HZ.store(hz, Ordering::Relaxed);
    TSC_HZ_FROM_CPUID.store(from_cpuid, Ordering::Relaxed);

    let calibration = calibration();
    crate::println!(
        // The worst window is printed beside the chosen one on purpose. The chosen rate is the
        // *smallest* of several timed windows, because a window's error is one-sided, so the gap
        // between them is this boot's own evidence of how much the host was descheduling the vCPU
        // mid-calibration. A quiet host prints two numbers a megahertz apart; the boot that
        // measured 4330 MHz would have printed the gap that said so. See CALIBRATION_WINDOWS.
        "  clocks      : tsc {} MHz ({}, best of {} windows, worst {} MHz), apic timer {} MHz (measured against the PIT)",
        frequency() / 1_000_000,
        frequency_source(),
        calibration.windows().len(),
        calibration.worst() / 1_000_000,
        apic_timer_frequency() / 1_000_000,
    );

    // The TSC-vs-RTC measurement instrument, off unless the `tsc_probe` feature is on. Here
    // because the rate it checks was measured a line ago and nothing else in the boot matters to
    // it; it halts when it is done, the same shape `bench` and `icount` take. See
    // notes/tsc-under-tcg.md.
    #[cfg(feature = "tsc_probe")]
    {
        super::tsc_probe::probe();
        crate::arch::halt(crate::arch::HaltReason::measurement_boot());
    }
}

/// **Start PIT channel 0 pulsing its interrupt line at about `hz`**, and report the rate actually
/// programmed.
///
/// The rate is "about" because the divisor is an integer: the PIT counts down from it at
/// [`PIT_HZ`], so only the frequencies that divide 1193182 exactly are exact. 100 Hz is 11932.4,
/// which rounds to a real rate of 100.0035 Hz. Reporting the achieved rate rather than the asked-for
/// one is what keeps a boot print from claiming a number the hardware is not producing.
///
/// **This does not route the interrupt anywhere.** The line goes to both interrupt controllers; the
/// 8259s are masked, and the IO APIC delivers nothing until `irq::enable` arms the redirection entry
/// the MADT's override names. This is the device half, and it is here because `timer.rs` is where
/// the PIT lives.
///
/// Name: provisional (milestone 161 (the kernel port)).
pub fn start_pit_ticking(hz: u64) -> u64 {
    let divisor = (PIT_HZ / hz).clamp(1, u16::MAX as u64) as u16;

    // SAFETY: the PIT is a fixed ISA device present on every x86 machine, and these three writes
    // are its documented programming sequence: the mode/command byte first (which latches the
    // access mode), then the divisor's low byte and high byte in that order, because the access
    // mode just selected says there are two of them.
    unsafe {
        out8(PIT_COMMAND, PIT_CHANNEL0_RATE);
        out8(PIT_CHANNEL0, divisor as u8);
        out8(PIT_CHANNEL0, (divisor >> 8) as u8);
    }

    PIT_HZ / divisor as u64
}

/// The counter's current value: the TSC.
pub fn now() -> u64 {
    rdtsc()
}

/// How many counter ticks make a second, or `None` if [`init_frequency`] has not run yet.
///
/// Unlike [`frequency`], does not panic: this is for a caller (the timebase page,
/// `kernel::user::x86_timebase_page_phys`) that must handle "not measured yet" as a normal,
/// representable state rather than a bug to crash on. In practice this is never `None` by the
/// time any process is loaded: `init_frequency` runs early in the boot, well before the
/// first call to `kernel::user::load`.
pub(crate) fn frequency_checked() -> Option<u64> {
    let hz = TSC_HZ.load(Ordering::Relaxed);
    (hz != 0).then_some(hz)
}

/// How many counter ticks make a second.
///
/// # Panics
/// If [`init_frequency`] has not run. Returning zero would make every duration computed from it
/// either zero or a division by zero, arbitrarily far from the missing call.
pub fn frequency() -> u64 {
    frequency_checked().expect("the TSC frequency has not been measured yet")
}

/// Whether [`frequency`]'s number came from `CPUID` leaf `0x15` or from PIT calibration, for the
/// boot print and this milestone's own evidence. `"uncalibrated"` before [`init_frequency`] has
/// run, which [`frequency`] itself would panic on; this never panics.
pub(crate) fn frequency_source() -> &'static str {
    if TSC_HZ.load(Ordering::Relaxed) == 0 {
        "uncalibrated"
    } else if TSC_HZ_FROM_CPUID.load(Ordering::Relaxed) {
        "cpuid leaf 0x15"
    } else {
        "PIT calibration"
    }
}

/// **What the PIT calibration actually saw**, which is more than the one number it returns.
///
/// This is a copy rather than a borrow of the statics behind it, so that a caller printing several
/// of these fields cannot see two different boots' worth (nothing writes them after
/// [`init_frequency`], so that is defensive rather than load-bearing, and it costs one stack
/// array).
///
/// Name: provisional (the `calib` lane, 2026-09-21).
pub struct Calibration {
    samples: [u64; CALIBRATION_WINDOW_CAP],
    taken: usize,
}

impl Calibration {
    /// **What each window timed implied the TSC's rate was**, in hertz, in the order they were
    /// timed. Empty before [`init_frequency`] has run.
    ///
    /// Every one of these is an *upper bound* on the truth, which is the whole argument: see
    /// [`CALIBRATION_WINDOW_CAP`]. So the spread across them is not an error bar. It says how hard
    /// the host was fighting this calibration, and the truth is at or below the smallest of them.
    pub(crate) fn windows(&self) -> &[u64] {
        &self.samples[..self.taken]
    }

    /// **The worst window's implied rate**, in hertz, for the boot print to show beside the chosen
    /// one. Zero before [`init_frequency`] has run.
    pub(crate) fn worst(&self) -> u64 {
        self.windows().iter().copied().max().unwrap_or(0)
    }
}

/// **What this boot's PIT calibration saw**, all of it. See [`Calibration`].
///
/// [`frequency`] returns the smallest window when the rate came from the PIT. It can also report
/// `cpuid leaf 0x15` (see [`frequency_source`]), in which case these windows were still timed, for
/// the local APIC timer's rate which has no `CPUID` equivalent, and simply not used for the TSC.
pub(crate) fn calibration() -> Calibration {
    Calibration {
        samples: core::array::from_fn(|i| CALIBRATION_SAMPLES[i].load(Ordering::Relaxed)),
        taken: CALIBRATION_TAKEN.load(Ordering::Relaxed),
    }
}

/// Counter ticks between scheduler ticks.
pub fn interval() -> u64 {
    frequency() / TICK_HZ
}

/// The local APIC timer's measured frequency, for the boot print. Zero before calibration.
pub(crate) fn apic_timer_frequency() -> u64 {
    APIC_TIMER_HZ.load(Ordering::Relaxed)
}

/// **Arm the periodic tick.** The local APIC timer, at [`TICK_HZ`], on the vector `irq` chose.
///
/// # Panics
/// If [`init_frequency`] has not run, for the same reason [`frequency`] does.
pub fn init() {
    let apic_hz = APIC_TIMER_HZ.load(Ordering::Relaxed);
    assert!(
        apic_hz != 0,
        "the local APIC timer's rate has not been measured yet (call init_frequency first)",
    );
    let count = (apic_hz / TICK_HZ) as u32;
    assert!(
        count != 0,
        "the measured APIC timer rate is below the tick rate, which cannot be right",
    );
    irq::arm_periodic_timer(count);
}

/// **Can a thread on this core be granted the cycle counter at all?** Yes, and it already has it:
/// `rdtsc` is ambient in ring 3 on this architecture and DECISIONS 139 part 3 decided to keep it
/// that way. Answering `true` is the honest answer to the question the caller is asking, which is
/// "will a granted thread be able to read a cycle-rate counter", and not a claim that anything is
/// gated.
///
/// **Built only under `test` or `--features cycle_counter_grant`** (milestone 237): the grant is
/// a measurement build the way `soak_test` is. `kernel/Cargo.toml`'s feature block carries the
/// reasoning and the measured cost. Milestone 228's closed default at `init` is NOT gated.
// Asked only by tests today (`sched`'s grant round trip and `user`'s EL0 one), which are the
// callers that have to skip rather than fault on a part with no counter to grant. Marked rather
// than deleted: milestone 74's cycle-counter work is the caller that will want it in anger.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
pub fn is_cycle_counter_grantable() -> bool {
    true
}

/// **The `x86_64` half of the cycle-counter grant, which does nothing** (milestone 229, DECISIONS
/// 139 part 3). Called from the context switch on every switch, exactly as its aarch64 and riscv64
/// twins are, so that `sched` has one portable call rather than three `cfg`s at the switch site.
/// It compiles to nothing.
///
/// **This is a stated exception to DECISIONS §19 (architectural parity is a tenet), not a gap.**
/// `CR4.TSD` (bit 2) would close `rdtsc` to ring 3 and is writable per switch like the other two
/// registers, so option 4 is mechanically available here. What is not available is a fallback:
/// `crates/user_mode_runtime`'s `now()` on this architecture **is** `rdtsc`, with no coarse monotonic source
/// to fall back to the way aarch64 has `CNTVCT_EL0` and riscv64 has `rdtime`, so closing it for
/// ungranted threads would take out `Instant`, `thread::sleep`, the random seed, smoltcp's
/// timestamps and the benchmark harness at once. DECISIONS 139 measured the two alternatives
/// (trap-and-emulate at 1,667 ns, 4.1x the syscall it is meant to beat; and there is no second
/// user-readable clock, which is why Linux's own vDSO cannot work without a userspace TSC read)
/// and closed both. `notes/x86-port/user-mode-runtime.md` carries the position where a reader meets it.
///
/// So on this architecture every thread runs with the counter open, granted or not, and a program
/// written against the grant works here for a reason it should not rely on.
///
/// **Built only under `test` or `--features cycle_counter_grant`** (milestone 237): the grant is
/// a measurement build the way `soak_test` is. `kernel/Cargo.toml`'s feature block carries the
/// reasoning and the measured cost. Milestone 228's closed default at `init` is NOT gated.
#[inline(always)]
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
pub(crate) fn set_cycle_counter_grant(_granted: bool) {}

/// **Take one tick.** Called from the trap handler when [`irq::TIMER_VECTOR`] arrives.
///
/// It does not write the EOI: the trap handler does that for every interrupt vector, in one place,
/// because a missed EOI is a hang rather than an error and one place is easier to be sure about
/// than one per handler.
pub(crate) fn tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
    // In a test build, feed the hang watchdogs (`testing::watchdog_tick`), as the other two
    // architectures' ticks do. Missing here from milestone 161 until 2026-10-11 (UTC), so no x86_64
    // test could fail by its budget or by a stall, and one hung for 9.5 hours (#1912).
    #[cfg(any(test, feature = "system_tests"))]
    crate::testing::watchdog_tick();
}

/// Scheduler ticks taken so far.
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// **Is this CPU's tick raised and waiting?** The local APIC timer's bit in the IRR.
///
/// The twin of the riscv64 `is_tick_pending`, whose comment has the measurement: the emulator
/// raises the timer from its own main loop, milliseconds and occasionally tens of milliseconds
/// late, so a test that assumes a tick is pending after a fixed masked spin can be measuring the
/// host. See notes/load-sensitive-assertions.md.
///
/// Name: ratified 2026-09-24 (calef, the Rust predicate-naming rule in design/naming.md). Refused
/// `tick_pending` (a bare participle reads as a getter, and Rust asks the question with `is_`).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn is_tick_pending() -> bool {
    irq::is_timer_pending()
}

/// Spin for `counter_ticks` of the TSC.
pub fn spin_for(counter_ticks: u64) {
    let start = now();
    while now().wrapping_sub(start) < counter_ticks {
        core::hint::spin_loop();
    }
}

/// Scheduler ticks taken on `cpu`.
///
/// # BUGS
/// **This ignores its argument and reports the global count**, which was harmless when there was
/// one CPU and is wrong now that there are several. The other two architectures keep a per-CPU
/// array so a before/after pair names one CPU even if the caller migrated. It also blinds
/// `sched::dump_threads`, whose per-core `ticks` column is how a dump tells a core wedged with
/// interrupts masked from one that is merely idle: here every core prints the same number, and the
/// other core's ticks hide the wedged one (corrected 2026-10-13 UTC by lane
/// `maintainer/supervision-label-hang`, whose hang was exactly that shape).
#[allow(dead_code)]
pub(crate) fn ticks_on(cpu: usize) -> u64 {
    let _ = cpu;
    ticks()
}
