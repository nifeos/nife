//! The timer, `x86_64`: the TSC, measured with the APIC timer against the 8254 PIT in polled
//! 10 ms windows (smallest wins), and the APIC timer for the tick. See notes/tsc-under-tcg.md.
//!
//! # BUGS
//! - The TSC is assumed invariant; `isa::init` refuses a bare-metal part that says otherwise.
//! - Min-of-N filters a fought host rather than bounding it; the boot line prints the worst window.
//! - Under `-icount shift=0,sleep=off` (`script/bench --x86`'s default) the TSC is not wall time.
//! - `now()` is this core's TSC; nothing measures or corrects skew between cores (QEMU has none).
//! - Calibration costs about 35 ms of boot on a quiet host, up to 160 ms at the cap.
//! - `init` panics if the local APIC is not up: the order (APIC, then timer) is real.

use core::sync::atomic::{AtomicU64, Ordering};

use super::irq;
use super::port::{in8, out8};

/// Ticks per second the scheduler asks for. The same 100 Hz both other architectures use.
///
/// **1 Hz in the bench build** (milestone 939 (the icount bench runs without a preemption tick in
/// its windows)), so a timed window opened just after a tick has a full second of virtual time
/// before the next one. `kernel/src/bench/timing.rs`'s `TickWindow` opens every row's window that
/// way and fails the run if a tick lands inside one.
pub const TICK_HZ: u64 = if cfg!(feature = "bench") { 1 } else { 100 };

/// **The 8254 PIT's input frequency**, 1193182 Hz, and the one rate on a PC that is a fixed number
/// rather than something to measure. It is 315/22 MHz divided by 12, which is to say it descends
/// from the NTSC colour-burst crystal that made 1981's parts cheap.
const PIT_HZ: u64 = 1_193_182;

/// How long one calibration window is. Ten milliseconds is a compromise: long enough that the
/// polling loop's own overhead is noise, short enough that the PIT's 16-bit counter holds it (its
/// maximum is about 54.9 ms) and that boot does not visibly pause.
const CALIBRATION_MS: u64 = 10;

/// The most windows that will ever be timed; [`CALIBRATION_AGREEMENT_PARTS_PER`] usually stops
/// sooner. A window can only be late, so each is an upper bound on the rate and the minimum is the
/// tightest. Sixteen is measured: on a host saturated to load 30, a cap of five left one boot in
/// eighteen wrong by over 1%, and sixteen left none of 200. The table is in
/// notes/benchmarks/counter-frequency-and-calibration.md.
const CALIBRATION_WINDOW_CAP: usize = 16;

/// When to stop taking windows: two within one part in this of the smallest end the calibration.
/// A descheduled window is inflated by an arbitrary amount, so two that agree to a thousandth were
/// both left alone. Over 490 boots this matched the full cap's accuracy at a mean of 3.5 windows
/// on a quiet host and 5.2 at load 30; see milestone 571's block in `design/roadmap/`.
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

/// Every window's implied TSC rate, in the order timed; only the first [`CALIBRATION_TAKEN`] mean
/// anything. Their spread says whether the host fought this boot's calibration, and the boot line
/// prints the worst beside the chosen one. [`calibration`] is the accessor.
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

    // Poll channel 2's output, which mode 0 raises at terminal count. Unbounded on purpose: a bound
    // would yield a wrong frequency silently, where a machine with no PIT hangs visibly.
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

/// Establish the TSC's rate and measure the local APIC timer against the PIT. The arch contract's
/// boot-info pointer is ignored: the numbers come from the machine.
///
/// [`crate::arch::x86_64::isa::tsc_crystal_hz`] (`CPUID` leaf 0x15) is asked first for the TSC,
/// and has why QEMU always takes the calibrated path. The windows run regardless, because the APIC
/// timer's rate has no `CPUID` source.
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

    // The minimum, not an average: every window's error is non-negative, so an average would carry
    // each descheduling into the answer. The TSC's and the APIC timer's minima are taken
    // independently, so neither is dragged up by the other's outlier.
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

    // A measured rate can be wrong with nothing broken, so it is held to the band the other
    // architectures' rates are (`counter_frequency_contract::is_plausible`).
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
        // The gap between the worst window and the chosen one is this boot's evidence of how hard
        // the host descheduled the vCPU; a quiet host prints two numbers a megahertz apart.
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

/// Start PIT channel 0 pulsing its interrupt line at about `hz`, and return the rate programmed.
///
/// The divisor is an integer, so 100 Hz is really 100.0035 Hz, and the achieved rate is returned.
/// This does not route the interrupt: the 8259s are masked, and the IO APIC delivers nothing until
/// `irq::enable` arms the entry the MADT's override names.
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
    let count = u32::try_from(apic_hz / TICK_HZ)
        .expect("one tick period does not fit the APIC timer's 32-bit initial count");
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

/// The `x86_64` half of the cycle-counter grant, which does nothing (milestone 229, §139 part 3).
/// Called on every switch like its aarch64 and riscv64 twins, so `sched` has one portable call.
///
/// A stated exception to §19 (architectural parity is a tenet): `CR4.TSD` could close `rdtsc`, but
/// `crates/user_mode_runtime`'s `now()` is `rdtsc` with no coarse clock to fall back on, so every
/// thread runs with the counter open, granted or not. See notes/x86-port/user-mode-runtime.md.
///
/// Built only under `test` or `--features cycle_counter_grant` (milestone 237); see
/// `kernel/Cargo.toml`'s feature block.
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
    // In a test build, feed the hang watchdogs, as the other two architectures' ticks do.
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
/// This ignores its argument and reports the global count; the other two architectures keep a
/// per-CPU array. It blinds `sched::dump_threads`'s per-core `ticks` column, which tells a core
/// wedged with interrupts masked from an idle one: here every core prints the same number.
#[allow(dead_code)]
pub(crate) fn ticks_on(cpu: usize) -> u64 {
    let _ = cpu;
    ticks()
}
