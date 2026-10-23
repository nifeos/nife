//! The RISC-V S-mode timer, the analog of aarch64's generic virtual timer.
//!
//! The free-running counter is the `time` CSR (`rdtime`). Each tick is armed through the SBI TIME
//! extension, which also clears the pending interrupt. SBI `set_timer` is write-only, so this file
//! keeps each hart's deadline grid itself, in [`DEADLINE`].
//!
//! See notes/riscv-port/timer.md.

use core::sync::atomic::{AtomicU64, Ordering};

use super::instructions;
use crate::cpu::{self, MAX_CPUS};

/// The S-mode timer interrupt cause (`scause` = 5, the Supervisor timer interrupt). The RISC-V
/// analog of aarch64's per-CPU timer INTID.
pub(crate) const TIMER_INTID: u32 = 5;

/// Ticks per second, the preemption rate. Same 100 Hz as aarch64.
///
/// **1 Hz in the bench build** (milestone 939 (the icount bench runs without a preemption tick in
/// its windows)), so a timed window opened just after a tick has a full second of virtual time
/// before the next one. `kernel/src/bench/timing.rs`'s `TickWindow` opens every row's window that
/// way and fails the run if a tick lands inside one.
pub const TICK_HZ: u64 = if cfg!(feature = "bench") { 1 } else { 100 };

/// The `time` CSR frequency in Hz, read from `/cpus/timebase-frequency`.
///
/// Holds zero until [`init_frequency`] runs, and its readers ([`frequency`], [`interval`]) assert
/// it nonzero, so a boot that never read the machine panics rather than running at a guessed rate.
static TIMEBASE_HZ: AtomicU64 = AtomicU64::new(0);

/// Read the counter rate out of the device tree at `dtb_ptr` (a physical address).
///
/// Called once, on the boot hart, before [`init`] arms the first deadline there. Secondaries share
/// its reading. See notes/riscv-port/timer.md, BUGS.
///
/// # Panics
///
/// If the tree is unreadable, states no frequency, or states one no real part runs at.
pub(crate) fn init_frequency(dtb_ptr: usize) {
    // SAFETY: the pointer OpenSBI handed us in `a1`, named through the boot table's direct map.
    // `DeviceTreeBlob::from_ptr` re-checks the magic, and this runs on the same map `memory::init`
    // uses.
    let dt = unsafe {
        device_tree_blob::DeviceTreeBlob::from_ptr(
            super::mmu::phys_to_virt(dtb_ptr as u64) as *const u8
        )
    }
    .expect("device tree is unreadable");
    let list =
        machine_discovery::cpu_list::CpuList::from_device_tree(&dt).expect("cannot read /cpus");
    let hz = list
        .timebase_hz
        .expect("the device tree states no /cpus/timebase-frequency, and RISC-V has no CNTFRQ_EL0");
    assert!(
        counter_frequency_contract::is_plausible(hz),
        "the device tree states a timebase of {hz} Hz, which no real part runs at: a counter that \
         slow cannot drive a tick, and one that fast was never measured"
    );
    TIMEBASE_HZ.store(hz, Ordering::Relaxed);
}

/// The counter rate the machine stated. Panics if read before [`init_frequency`], as
/// `arch::isa::get` does.
fn timebase_hz() -> u64 {
    let hz = TIMEBASE_HZ.load(Ordering::Relaxed);
    assert!(
        hz > 0,
        "arch::timer used before arch::timer::init_frequency"
    );
    hz
}

/// `sie.STIE`, bit 5: the Supervisor Timer Interrupt Enable.
const STIE: u64 = 1 << 5;

/// Ticks since boot, per hart, maintained by [`tick`].
///
/// Per hart because masking `sstatus.SIE` masks one hart's timer (DECISIONS §11 (SMP: per-CPU run
/// queues)): only a per-hart count shows that holding a lock stopped this hart's ticks.
static TICKS: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

/// Deadlines already past when this hart re-armed, per hart. Nonzero means a whole tick period
/// elapsed inside the handler or a critical section.
static MISSED_TICKS: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

/// The absolute `time` value each hart's next tick is due at: the grid, anchored by [`init`]. SBI's
/// `set_timer` cannot be read back, so this is the only copy.
static DEADLINE: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

/// Arm this hart's S-mode timer interrupt for when `time` reaches `next`, through SBI TIME
/// `set_timer`, and clear any pending one.
fn sbi_set_timer(next: u64) {
    const SBI_TIME_EID: usize = 0x5449_4D45; // "TIME"
    const SBI_SET_TIMER_FID: usize = 0;
    // SAFETY: TIME set_timer with the absolute deadline. It programs this hart's timer interrupt
    // and touches no memory of ours. The result is ignored, as it always was.
    unsafe {
        super::sbi::call(
            SBI_TIME_EID,
            SBI_SET_TIMER_FID,
            [next as usize, 0, 0, 0, 0, 0],
        )
    };
}

/// The free-running counter (`rdtime`). Needs no timer set-up.
pub fn now() -> u64 {
    instructions::read_time()
}

/// The counter's frequency in Hz, as the machine stated it.
pub fn frequency() -> u64 {
    timebase_hz()
}

/// How many counter ticks make a second, or `None` if [`init_frequency`] has not run yet.
///
/// The non-panicking [`frequency`], for the timebase page (`kernel::user::timebase_page_phys`), the
/// same as `arch::x86_64::timer::frequency_checked`. It is `Some` before any process loads.
pub(crate) fn frequency_checked() -> Option<u64> {
    let hz = TIMEBASE_HZ.load(Ordering::Relaxed);
    (hz != 0).then_some(hz)
}

/// Counter ticks between two timer interrupts (the reload interval): one tick period.
pub fn interval() -> u64 {
    timebase_hz() / TICK_HZ
}

/// Start the periodic timer: arm the first deadline through SBI, and enable the S-mode timer
/// interrupt in `sie`. The caller enables interrupts globally (`sstatus.SIE`) when it is ready to
/// take them.
pub fn init() {
    // Anchor this hart's grid before arming it: `rearm` adds whole intervals to this value.
    let first = now() + interval();
    DEADLINE[cpu::id()].store(first, Ordering::Relaxed);
    sbi_set_timer(first);
    // Setting sie.STIE only unmasks the timer source; it takes effect once SIE is on.
    instructions::set_sie(STIE);

    // `scounteren` is per hart, so every hart writes it here. A whole-register write, not a bit
    // set: afterwards it is exactly `TM` whatever firmware left in it, so U-mode can read `time`
    // and not `cycle`, `instret` or the HPM counters. See notes/riscv-port/timer.md.
    //
    // SAFETY: setting scounteren.TM only *permits* a read U-mode could already attempt; it grants no
    // authority to affect anything. The same eyes-open exception to §10 that aarch64 records, with
    // the same reason: the cross-OS primitive suite needs userspace self-timing to be comparable to
    // lmbench. Every other bit written here is a zero, and a zero in this CSR only takes a U-mode
    // read permission away: CY (cycle) and IR (instret) are now closed by this instruction rather
    // than by assumption.
    const TM: u64 = 1 << 1;
    // The CSR governs U-mode counter reads only, so no S-mode access this kernel makes depends on
    // its value.
    instructions::write_scounteren(TM);
}

/// `scounteren.CY`, bit 0: lets U-mode read the `cycle` CSR. Built only with the grant that sets
/// it.
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
const CY: u64 = 1 << 0;

/// Whether a thread on this hart can be granted the cycle counter. Always `true` on this ISA,
/// because `scounteren` is mandatory in S-mode.
///
/// Returns `true` even where firmware left `mcounteren.CY` clear, in which case a granted `rdcycle`
/// still traps. Built only under `test` or `--features cycle_counter_grant`. See
/// notes/riscv-port/timer.md.
// Only tests call it, which must skip rather than fault where no counter can be granted.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
pub fn is_cycle_counter_grantable() -> bool {
    true
}

/// Open (`granted`) or close U-mode's view of the `cycle` CSR for the thread about to run
/// (DECISIONS 139 option 4).
///
/// Called on every context switch. It reads `scounteren` back and writes only when `CY` changes,
/// leaving `TM` as it found it. Built only under `test` or `--features cycle_counter_grant`. See
/// notes/riscv-port/timer.md.
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
pub(crate) fn set_cycle_counter_grant(granted: bool) {
    let current = instructions::read_scounteren();

    let want = if granted { current | CY } else { current & !CY };
    if want == current {
        return;
    }

    // `want` differs from the live value in `CY` alone: every other bit is written back exactly as
    // it was read, so `TM` survives whatever this does.
    instructions::write_scounteren(want);
}

/// Handle a timer interrupt: count the tick and arm the next deadline, which also clears the
/// pending interrupt. Called from the trap dispatcher on `scause` = timer.
///
/// `#[inline(never)]` keeps these bytes out of `riscv_trap_body`, which `script/fastpath-footprint`
/// counts as syscall entry. See notes/riscv-port/timer.md.
///
/// Context: interrupt context.
#[inline(never)]
pub(crate) fn tick() {
    TICKS[cpu::id()].fetch_add(1, Ordering::Relaxed);
    // Test builds feed the hang watchdog from the tick.
    #[cfg(any(test, feature = "system_tests"))]
    crate::testing::watchdog_tick();
    rearm();
}

/// Move this hart's deadline forward by exactly one interval, `deadline + interval`.
///
/// If that deadline has already passed, the tick is counted in [`MISSED_TICKS`] and the grid
/// re-anchors at `now + interval`, so the handler cannot fire back to back forever. See
/// notes/riscv-port/timer.md.
///
/// Context: interrupt context.
fn rearm() {
    let id = cpu::id();
    let now = now();
    // Named, not folded into the sum: the icount instrument measures arrival against it.
    let fired = DEADLINE[id].load(Ordering::Relaxed);
    let mut next = fired + interval();

    if next <= now {
        MISSED_TICKS[id].fetch_add(1, Ordering::Relaxed);
        // Trap context: record, never print (DECISIONS §9 (locking: IrqSafeMutex, plus a
        // discipline)).
        #[cfg(any(test, feature = "system_tests"))]
        miss_detail::record(now, next);
        next = now + interval();
    }

    DEADLINE[id].store(next, Ordering::Relaxed);
    sbi_set_timer(next);

    // After the SBI call: the firmware round trip is part of the span the instrument measures.
    #[cfg(feature = "icount")]
    crate::icount::tick_trace::record(fired, now, super::timer::now());
}

/// The most instructions `icount::run` allows between a deadline and the handler observing it.
///
/// Because `set_timer` cannot be read back, this bound is what shows firmware was armed with
/// [`DEADLINE`]'s word. See notes/riscv-port/timer.md and notes/instruction-clock.md.
#[cfg(feature = "icount")]
pub(crate) const ARRIVAL_BOUND: u64 = 1_500;

/// The most instructions `icount::run` allows from a deadline to the next one being armed: the
/// whole handler, SBI `ecall` included.
///
/// Larger than aarch64's because the firmware round trip is inside the span. See
/// notes/riscv-port/timer.md.
#[cfg(feature = "icount")]
pub(crate) const HANDLER_BOUND: u64 = 2_500;

/// Execute `2 * iters` instructions and return that count, for `icount::calibrate`.
///
/// The loop is `addi`/`bnez`, two instructions an iteration whether or not they are compressed. The
/// count covers the loop body only, so the caller compares with a tolerance.
#[cfg(feature = "icount")]
pub(crate) fn calibration_loop(iters: u64) -> u64 {
    // SAFETY: a self-contained counted loop in a caller-saved scratch register. It touches no
    // memory, makes no call, and leaves the register dead, which is what the operand spec and
    // `nomem`/`nostack` state.
    unsafe {
        core::arch::asm!(
            "2:",
            "addi {n}, {n}, -1",
            "bnez {n}, 2b",
            n = inout(reg) iters => _,
            options(nomem, nostack),
        );
    }
    2 * iters
}

/// Ticks since boot, **on this hart**.
pub fn ticks() -> u64 {
    ticks_on(cpu::id())
}

/// Ticks since boot on `hart`, so a test whose thread may migrate between two reads compares one
/// counter (DECISIONS §28.3 (SMP placement)). See notes/load-sensitive-assertions.md.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) fn ticks_on(hart: usize) -> u64 {
    TICKS[hart].load(Ordering::Relaxed)
}

/// Whether this hart's tick is raised and waiting (`sip.STIP`), masked or not.
///
/// True from the moment the deadline is signalled until the handler's `sbi_set_timer` clears it.
/// Tests that hold a tick across a masked window wait on this rather than on the clock, because
/// QEMU raises the timer late. See notes/load-sensitive-assertions/preemption-window-tick.md.
///
/// Name: ratified 2026-09-24 (calef, the Rust predicate-naming rule in design/naming.md). Refused
/// `tick_pending` (a bare participle reads as a getter, and Rust asks the question with `is_`).
/// The three architectures' copies are named alike on purpose.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn is_tick_pending() -> bool {
    let sip = instructions::read_sip();
    // `sip.STIP` is bit 5, the same bit position `sie.STIE` enables.
    sip & STIE != 0
}

/// Milliseconds since boot, from the free-running counter rather than the tick count.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) fn uptime_ms() -> u64 {
    now() / (timebase_hz() / 1000)
}

/// Busy-wait for `counter_ticks` of the free-running counter.
pub fn spin_for(counter_ticks: u64) {
    let start = now();
    while now().wrapping_sub(start) < counter_ticks {
        core::hint::spin_loop();
    }
}

/// This hart's missed ticks: deadlines that had already passed when [`rearm`] ran.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) fn missed_ticks() -> u64 {
    missed_ticks_on(cpu::id())
}

/// Missed ticks on `hart`, for the reason [`ticks_on`] takes one.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) fn missed_ticks_on(hart: usize) -> u64 {
    MISSED_TICKS[hart].load(Ordering::Relaxed)
}

/// This hart's next armed deadline, the grid cell [`rearm`] advances from.
///
/// Read by the drift test and by `icount::run`'s claim 4, both of which check that the deadline
/// moves by exactly one interval per delivered tick.
#[cfg_attr(
    all(not(any(test, feature = "system_tests")), not(feature = "icount")),
    allow(dead_code)
)]
pub(crate) fn deadline() -> u64 {
    DEADLINE[cpu::id()].load(Ordering::Relaxed)
}

/// How late each hart's last missed tick was, kept only in test builds.
///
/// Holds the last miss and a count, per hart. Late by under one interval points at a slow handler;
/// by a whole interval or more, at a descheduled emulator. See notes/riscv-port/timer.md.
#[cfg(any(test, feature = "system_tests"))]
pub mod miss_detail {
    use core::sync::atomic::{AtomicU64, Ordering};

    use crate::cpu::{self, MAX_CPUS};

    static NOW: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];
    static NEXT: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];
    static COUNT: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

    /// Record `now` and the deadline `next` it had already passed.
    ///
    /// Context: interrupt context, from [`super::rearm`].
    pub(crate) fn record(now: u64, next: u64) {
        let id = cpu::id();
        NOW[id].store(now, Ordering::Relaxed);
        NEXT[id].store(next, Ordering::Relaxed);
        COUNT[id].fetch_add(1, Ordering::Relaxed);
    }

    /// This hart's last miss as `(now, next, count)`. `now - next` is how late the re-arm was, in
    /// counter ticks; compare it against [`super::interval`] to tell a slow handler from a
    /// descheduled emulator.
    pub(crate) fn last() -> (u64, u64, u64) {
        let id = cpu::id();
        (
            NOW[id].load(Ordering::Relaxed),
            NEXT[id].load(Ordering::Relaxed),
            COUNT[id].load(Ordering::Relaxed),
        )
    }
}

#[cfg(test)]
mod tests {
    //! Tests for the SBI timer and for interrupt masking under a lock, the twins of aarch64's.
    //!
    //! Every delay is measured with `rdtime`, which keeps counting while interrupts are masked.

    /// The cycle-counter grant opens and closes `scounteren.CY` and leaves `TM` open.
    ///
    /// Each call is made twice, because the context switch repeats it and the no-change path must
    /// be right too. Leaves `CY` closed, as `init` did.
    #[test_case]
    fn the_cycle_counter_grant_moves_cy_and_leaves_tm_alone() {
        super::set_cycle_counter_grant(true);
        super::set_cycle_counter_grant(true);
        assert_eq!(
            read_scounteren() & 1,
            1,
            "the grant did not open the cycle counter"
        );
        assert_eq!(
            read_scounteren() & 2,
            2,
            "granting the cycle counter closed the time counter every thread is meant to have",
        );

        super::set_cycle_counter_grant(false);
        super::set_cycle_counter_grant(false);
        assert_eq!(
            read_scounteren() & 1,
            0,
            "the cycle counter was left open to U-mode"
        );
        assert_eq!(
            read_scounteren() & 2,
            2,
            "ungranting the cycle counter closed the time counter with it",
        );
    }

    /// `scounteren`, read back out of the hart rather than out of our record of it.
    fn read_scounteren() -> u64 {
        crate::arch::riscv64::instructions::read_scounteren()
    }
    /// The counter rate is the one the device tree states, re-read here apart from the boot path.
    ///
    /// It asserts against the blob rather than a number, because QEMU `virt`'s 10 MHz would match
    /// a compiled-in constant too.
    #[test_case]
    fn the_counter_rate_is_the_one_the_tree_states() {
        use core::sync::atomic::Ordering as O;

        let ptr = crate::DTB.load(O::Relaxed);
        // SAFETY: the pointer firmware handed us, already parsed twice on this boot.
        let dt = unsafe {
            device_tree_blob::DeviceTreeBlob::from_ptr(
                crate::arch::mmu::phys_to_virt(ptr as u64) as *const u8
            )
        }
        .expect("device tree is unreadable");
        let stated = machine_discovery::cpu_list::CpuList::from_device_tree(&dt)
            .expect("cannot read /cpus")
            .timebase_hz
            .expect("QEMU virt states /cpus/timebase-frequency");

        assert_eq!(
            crate::arch::timer::frequency(),
            stated,
            "the timer is running at a rate the machine did not state",
        );
        assert_eq!(
            crate::arch::timer::interval(),
            stated / super::TICK_HZ,
            "the tick interval is derived from the stated rate",
        );
    }

    /// The heartbeat is beating.
    #[test_case]
    fn the_timer_is_ticking() {
        use crate::arch::timer;

        // Per-hart counter, and this thread can migrate: name the hart (`ticks_on`).
        let hart = crate::cpu::id();
        let before = timer::ticks_on(hart);
        // Waited for, not spun for: QEMU can raise the timer tens of milliseconds late.
        let ticked = within_periods(RAISE_BOUND_PERIODS, || timer::ticks_on(hart) > before);

        assert!(
            ticked,
            "no timer interrupt in three tick periods: SBI set_timer, sie.STIE, or the scause=5 \
             arm of the trap dispatcher is not delivering"
        );
    }

    /// Ticks arrive at the configured rate, proven by the re-arm law rather than the wall clock.
    ///
    /// Over a window with no recorded miss, [`rearm`] must move the deadline by exactly one
    /// interval per delivered tick. Re-arming from `now()` fails on the first tick; a descheduled
    /// emulator cannot fail it, since a long deschedule is a counted miss and the window is
    /// retried. Skips when no miss-free window comes. See notes/riscv-port/timer.md.
    #[test_case]
    fn ticks_arrive_at_the_configured_rate() {
        use crate::arch::timer;

        // Re-read until an unchanged tick count and hart id bracket the other reads, so the four
        // values come from one hart between two ticks.
        let snapshot = || loop {
            let hart = crate::cpu::id();
            let t = timer::ticks();
            let m = timer::missed_ticks();
            let d = timer::deadline();
            let c = timer::now();
            if timer::ticks() == t && crate::cpu::id() == hart {
                break (hart, t, m, d, c);
            }
        };

        // A window holding a miss proves nothing, since a miss re-anchors the grid: retry it. Out
        // of attempts is a skip, not a failure, and the budget does not widen (DECISIONS §61 (a
        // lint is adopted on evidence)).
        const ATTEMPTS: u32 = 8;
        let mut attempts = 0;
        let measured = loop {
            let (h0, t0, m0, d0, c0) = snapshot();
            timer::spin_for(timer::frequency() / 4); // a quarter of a second, by the counter
            let (h1, t1, m1, d1, c1) = snapshot();

            if h0 == h1 && m0 == m1 && t1 - t0 >= 2 {
                break Some((t1 - t0, d1 - d0, (c1 - c0) / timer::interval()));
            }
            attempts += 1;
            if attempts == ATTEMPTS {
                break None;
            }
        };

        // An unmeasured run says so, with the numbers to triage it.
        let Some((elapsed_ticks, deadline_delta, expected)) = measured else {
            let (now, next, misses) = super::miss_detail::last();
            crate::println!(
                "    (UNMEASURED: no miss-free window in {} tries, so the re-arm law was not \
                 tested this run. {} misses recorded on this hart, the last re-armed {} counter \
                 ticks late against an interval of {}. A miss re-anchors the grid, so a window \
                 containing one proves nothing either way, and whether these misses are a \
                 contended host or a slow handler is the one question a wall clock cannot answer \
                 from inside the guest. `script/icount` answers it and asserts this same law with \
                 no host term in it. Milestone 62; notes/load-sensitive-assertions.md.)",
                ATTEMPTS,
                misses,
                now.saturating_sub(next),
                timer::interval(),
            );
            // A test that measured nothing is a skip, not a pass (milestone 214 (a test that prints
            // "skipping" and returns)).
            crate::testing::skip!(
                "no miss-free window in which to measure the re-arm law this run"
            );
        };

        assert_eq!(
            deadline_delta,
            elapsed_ticks * timer::interval(),
            "timer drift: {elapsed_ticks} ticks moved the deadline off the grid. Re-arming from \
             `now()` inside the handler instead of from the previous deadline does exactly this."
        );

        // Descheduling only drops ticks, so a surplus over elapsed periods (plus one for starting
        // mid-period) means the timer fires faster than the grid.
        assert!(
            elapsed_ticks <= expected + 1,
            "{elapsed_ticks} ticks in {expected} periods: the timer fires faster than configured"
        );
    }

    // The handler's speed with no lock held is asserted by `script/icount`, in instructions, not
    // here: from inside the guest a slow handler and a descheduled host look alike. See
    // notes/riscv-port/timer.md.

    /// Holding an `IrqSafeMutex` across more than one tick deadline loses a tick.
    ///
    /// If it did not, the mutex would not be masking interrupts. This is the cost DECISIONS §9's
    /// short critical sections avoid.
    #[test_case]
    fn a_long_critical_section_costs_a_tick() {
        use crate::arch::timer;
        use crate::sync::{IrqSafeMutex, rank};

        static M: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::PAGE_FRAMES, 0);

        // Read `before` inside the critical section, where this thread cannot migrate, so the
        // window measured is exactly the masked one.
        let (hart, before) = {
            let _guard = M.lock();
            let hart = crate::cpu::id();
            let before = timer::missed_ticks_on(hart);
            // Two and a half periods masked: a deadline passes unserviced, then one more.
            timer::spin_for(timer::interval() * 2 + timer::interval() / 2);
            (hart, before)
        };

        // Bounded wait, not one period: a descheduled host only makes the delivery later.
        assert!(
            within_periods(20, || timer::missed_ticks_on(hart) > before),
            "holding a lock across two tick periods did NOT lose a tick, which means \
             IrqSafeMutex is not masking interrupts and the deadlock in notes/locking.md is live"
        );
    }

    /// Uptime comes from the counter, not the tick count, so a missed tick cannot slow it.
    #[test_case]
    fn uptime_advances_monotonically() {
        use crate::arch::timer;

        let a = timer::uptime_ms();
        timer::spin_for(timer::frequency() / 50); // 20 ms
        let b = timer::uptime_ms();

        assert!(b > a, "uptime went backwards or stalled: {a} -> {b}");
        assert!(
            b - a >= 15,
            "uptime advanced {} ms in 20 ms of counter time",
            b - a
        );
    }

    /// Holding an `IrqSafeMutex` masks this hart's timer (`sstatus.SIE`), and releasing it delivers
    /// the held tick.
    ///
    /// Confirms ticks flow, spins three periods under a lock asserting none lands, waits for one to
    /// be raised, then releases and waits for delivery. This is the deadlock DECISIONS §9 and
    /// notes/locking.md exist to prevent.
    #[test_case]
    fn holding_a_lock_masks_the_timer() {
        use crate::arch::{interrupts, timer};
        use crate::sync::{IrqSafeMutex, rank};

        static M: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::PAGE_FRAMES, 0);

        assert!(
            interrupts::is_enabled(),
            "test setup: interrupts should be on"
        );

        // The timer is alive. Hart-scoped, for `ticks_on`'s reason.
        let alive_on = crate::cpu::id();
        let t0 = timer::ticks_on(alive_on);
        // Waited for, for `the_timer_is_ticking`'s reason.
        assert!(
            within_periods(RAISE_BOUND_PERIODS, || timer::ticks_on(alive_on) > t0),
            "the timer is not ticking at all"
        );

        // Both reads happen inside the critical section. A read before `M.lock()` would charge to
        // the lock a tick landing before the mask, which a resuming vCPU takes at once.
        let (hart, before) = {
            let _guard = M.lock();
            // Masked from here, so `cpu::id()` is fixed for the whole block.
            let hart = crate::cpu::id();
            let before = timer::ticks_on(hart);

            // Thirty milliseconds. Three ticks' worth. Not one of them may land.
            timer::spin_for(timer::interval() * 3);

            assert_eq!(
                timer::ticks_on(hart),
                before,
                "A TIMER INTERRUPT FIRED WHILE A LOCK WAS HELD. IrqSafeMutex is not masking \
                 sstatus.SIE, and the deadlock in notes/locking.md is live: a handler that touched \
                 this lock would spin forever waiting for code that cannot run."
            );

            // Wait until a tick is raised, so the release tests `restore` rather than the host.
            // The wait is inside the window, so the no-tick assertion is repeated after it.
            assert!(
                within_raise_bound(timer::is_tick_pending),
                "the timer was never raised in a second with the lock held, so the release below \
                 would test nothing: the timer is not being armed, or the emulator stopped \
                 delivering it"
            );
            assert_eq!(
                timer::ticks_on(hart),
                before,
                "A TIMER INTERRUPT FIRED WHILE A LOCK WAS HELD. IrqSafeMutex is not masking \
                 sstatus.SIE, and the deadlock in notes/locking.md is live: a handler that touched \
                 this lock would spin forever waiting for code that cannot run."
            );
            (hart, before)
        };

        // Dropping the guard is a preemption point, so read by hart index, with a bounded wait.
        assert!(
            within_periods(20, || timer::ticks_on(hart) > before),
            "interrupts did not resume after the lock was released: `restore` is broken"
        );
    }

    /// How long a wait on the timer being raised may take, in tick periods: one second. A leak
    /// trap, not a timing claim; the worst raise measured was 86 ms.
    ///
    /// Name: provisional, minted 2026-09-24 (`cda66d656`, waiting for the tick to be raised).
    const RAISE_BOUND_PERIODS: u32 = 100;

    /// Spin until `cond` holds, for at most [`RAISE_BOUND_PERIODS`], checking continuously. For
    /// waits with interrupts masked. Returns whether `cond` held.
    ///
    /// Name: provisional, minted 2026-09-24 (`cda66d656`, waiting for the tick to be raised).
    fn within_raise_bound(mut cond: impl FnMut() -> bool) -> bool {
        let bound = u64::from(RAISE_BOUND_PERIODS) * crate::arch::timer::interval();
        let start = crate::arch::timer::now();
        while !cond() {
            if crate::arch::timer::now().wrapping_sub(start) >= bound {
                return false;
            }
            core::hint::spin_loop();
        }
        true
    }

    /// Wait up to `periods` tick periods for `cond`, checking once a period; returns whether it
    /// held. Only for "has it happened yet", where a busy host makes a pass late, never wrong.
    fn within_periods(periods: u32, mut cond: impl FnMut() -> bool) -> bool {
        for _ in 0..periods {
            if cond() {
                return true;
            }
            crate::arch::timer::spin_for(crate::arch::timer::interval());
        }
        cond()
    }
}
