//! The ARM Generic Timer: the preemption source, and the counter `Instant` is made of.
//!
//! Part of the CPU, reached through system registers, one per core. The handler re-arms from the
//! absolute deadline (`CNTV_CVAL_EL0`), so the ticks sit on a grid anchored at boot. See
//! notes/aarch64-port.md, "The Generic Timer", and notes/interrupts.md.

// `ticks`, `missed_ticks`, `uptime_ms` and `interval` have no caller outside this file's tests on
// aarch64, so each is `cfg_attr(not(test), allow(dead_code))` rather than the whole module.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use aarch64_cpu::registers::{CNTFRQ_EL0, CNTKCTL_EL1, CNTV_CTL_EL0, CNTV_CVAL_EL0, CNTVCT_EL0};
use tock_registers::interfaces::{Readable, Writeable};

use crate::cpu::{self, MAX_CPUS};

/// The EL1 **virtual** timer, as a GIC interrupt ID.
///
/// A PPI, per core: the device tree's `interrupts = <1 11 ...>` is PPI 11, and PPIs start at INTID
/// 16. The virtual timer, not the physical one, because a hypervisor owns the physical timer and
/// a guest that touches it traps. See notes/virtualization.md.
pub(crate) const TIMER_INTID: u32 = 27;

/// 100 Hz. Ten milliseconds per tick.
///
/// **1 Hz in the bench build**, so a timed window opened just after a tick has a second of virtual
/// time before the next; `kernel/src/bench/timing.rs`'s `TickWindow` fails a run if a tick lands
/// inside a window.
pub const TICK_HZ: u64 = if cfg!(feature = "bench") { 1 } else { 100 };

/// Ticks delivered, per core: each core has its own timer, and a lock masks only the holding core's
/// interrupts. See §11 (SMP: per-CPU run queues).
static TICKS: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

/// Counter ticks between interrupts, computed from `CNTFRQ_EL0`: the rate is the board's.
static INTERVAL: AtomicU64 = AtomicU64::new(0);

/// **Does this core have `PMUSERENR_EL0`**, the register the cycle-counter grant writes? Recorded
/// per core by `init` from `ID_AA64DFR0_EL1.PMUVer`, so the context switch reads no ID register.
static PMU_PRESENT: [AtomicBool; MAX_CPUS] = [const { AtomicBool::new(false) }; MAX_CPUS];

/// **What this core last wrote to `PMUSERENR_EL0`**, as a bool: open (`CR` set) or closed (zero).
/// `init` writes zero, so `false` is the truth at boot on every core.
///
/// A cache, not a read-back, because without `FEAT_PMUv3` reading the register is UNDEFINED.
/// Nothing but this file writes `PMUSERENR_EL0`, so the cache cannot go stale.
static COUNTER_OPEN: [AtomicBool; MAX_CPUS] = [const { AtomicBool::new(false) }; MAX_CPUS];

/// Start the heartbeat.
///
/// The GIC must already be up: this enables [`TIMER_INTID`] at it.
pub fn init() {
    let freq = CNTFRQ_EL0.get();
    assert!(
        counter_frequency_contract::is_plausible(freq),
        "CNTFRQ_EL0 reports {freq} Hz, which no real part does: firmware never wrote it, or wrote \
         nonsense"
    );

    // EL0 may read `CNTVCT_EL0` and `CNTFRQ_EL0`, a deliberate exception to §10 (notes/abi.md);
    // the physical counter and the timer registers stay trapped. `aarch64_cpu` names no fields of
    // CNTKCTL_EL1, so EL0VCTEN is bit 1 by hand.
    const EL0VCTEN: u64 = 1 << 1;
    CNTKCTL_EL1.set(CNTKCTL_EL1.get() | EL0VCTEN);

    close_cycle_counter_to_el0();
    super::pmu::init_this_cpu();

    let interval = freq / TICK_HZ;
    INTERVAL.store(interval, Ordering::Relaxed);

    super::irq::enable(TIMER_INTID); // PPI: enabled on this core, no target

    start(interval);
}

/// Write `PMUSERENR_EL0 = 0`, so EL0 cannot read the **cycle** counter.
///
/// Per core, beside the `CNTKCTL_EL1` write, because the register is banked and resets to an
/// UNKNOWN value. See notes/pmu.md, "Who may read the cycle counter from EL0".
fn close_cycle_counter_to_el0() {
    // Without FEAT_PMUv3 the register does not exist and an access is UNDEFINED.
    if !super::pmu::is_pmuv3_present() {
        return;
    }
    PMU_PRESENT[cpu::id()].store(true, Ordering::Relaxed);

    // SAFETY: writing zero to `pmuserenr_el0` only *removes* EL0 permissions; every field of that register
    // is an EL0 enable, so zero cannot make any access newly legal, and it does not affect EL1's own
    // PMU access (which `MDCR_EL2.TPM` and `PMCR_EL0` govern, not this register). Writing it is a
    // legal EL1 operation whenever the register is present, which the PMUVer check above
    // establishes. It touches no memory and clobbers no flags, which the options state.
    unsafe { super::instructions::write_pmuserenr(0) };
    COUNTER_OPEN[cpu::id()].store(false, Ordering::Relaxed);
}

/// **Can a thread on this core be granted the cycle counter at all?** False on a part with no
/// `FEAT_PMUv3`, where `PMCCNTR_EL0` and the register that gates it both simply do not exist.
///
/// The grant is still accepted on such a part; this lets a test skip instead of faulting. Whether
/// the counter runs is `arch::pmu::outcome`, a separate question: a counter can be grantable and
/// stuck. Built only under `test` or `--features cycle_counter_grant` (`kernel/Cargo.toml`).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
pub fn is_cycle_counter_grantable() -> bool {
    PMU_PRESENT[cpu::id()].load(Ordering::Relaxed)
}

/// **Open or close EL0's view of `PMCCNTR_EL0` for the thread about to run** (DECISIONS 139
/// option 4). Writes `PMUSERENR_EL0.CR` and nothing else, and only when the value changes. No
/// `isb`: the way back to EL0 is an `eret`. See notes/pmu.md, "Who may read the cycle counter".
///
/// # BUGS
///
/// **Under HVF, closing does not close**: the ungranted half of the EL0 grant test fails on an
/// Apple core, likely answered by the hypervisor without consulting this register. Under TCG it
/// passes; silicon is untested.
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
pub(crate) fn set_cycle_counter_grant(granted: bool) {
    let cpu = cpu::id();
    if COUNTER_OPEN[cpu].load(Ordering::Relaxed) == granted {
        return;
    }
    if !PMU_PRESENT[cpu].load(Ordering::Relaxed) {
        // No `FEAT_PMUv3`: nothing to open, and the cache stays `false`.
        return;
    }

    // `CR`, bit 2 of `PMUSERENR_EL0`: "EL0 using AArch64: EL0 access to `PMCCNTR_EL0` is enabled."
    const CR: u64 = 1 << 2;
    let value = if granted { CR } else { 0 };

    // SAFETY: `PMUSERENR_EL0` is present, which the `PMU_PRESENT` check above establishes from this
    // core's own `ID_AA64DFR0_EL1.PMUVer`, so the `msr` is a legal EL1 operation and not UNDEFINED.
    // Every field of the register is an EL0 *enable*, so no value written here can affect what EL1
    // may do, nor make any EL1 access newly legal; EL1's own PMU access is governed by `MDCR_EL2`
    // and `PMCR_EL0`. It touches no memory and clobbers no flags, which the options state.
    unsafe { super::instructions::write_pmuserenr(value) };
    COUNTER_OPEN[cpu].store(granted, Ordering::Relaxed);
}

/// Set the first deadline and enable.
///
/// `IMASK` clear raises the line; with it set the timer still fires and sets `ISTATUS`.
fn start(interval: u64) {
    CNTV_CVAL_EL0.set(CNTVCT_EL0.get() + interval);
    CNTV_CTL_EL0.write(CNTV_CTL_EL0::ENABLE::SET + CNTV_CTL_EL0::IMASK::CLEAR);
}

/// Move the deadline forward by exactly one interval.
///
/// **`previous + interval`, not `now + interval`**, so the deadlines stay on the boot grid. A
/// deadline already past counts a missed tick and re-anchors to now, rather than firing in a loop.
fn rearm(interval: u64) {
    let now = CNTVCT_EL0.get();
    // Read before the write below: the icount instrument compares the arrival against it.
    let fired = CNTV_CVAL_EL0.get();
    let mut next = fired + interval;

    if next <= now {
        MISSED_TICKS[cpu::id()].fetch_add(1, Ordering::Relaxed);
        // Interrupt context: record, never print (§9 (locking: IrqSafeMutex, plus a discipline)).
        #[cfg(any(test, feature = "system_tests"))]
        miss_detail::record(now, next);
        next = now + interval;
    }

    CNTV_CVAL_EL0.set(next);

    // After the deadline is armed, so the span measured is the whole handler. See
    // kernel/src/icount.rs.
    #[cfg(feature = "icount")]
    crate::icount::tick_trace::record(fired, now, CNTVCT_EL0.get());
}

/// **Instructions between a deadline and the handler observing it** (milestone 78's instrument),
/// the ceiling `icount::run` asserts.
///
/// Delivery, the vector, the register save, dispatch, the acknowledge and the counter bump, in
/// the debug build. A ceiling with room for codegen movement, not a baseline; the run prints what
/// it measured beside it.
#[cfg(feature = "icount")]
pub(crate) const ARRIVAL_BOUND: u64 = 2_000;

/// **Instructions from a deadline to the next one being armed** (milestone 78's instrument): the
/// whole handler, which is [`ARRIVAL_BOUND`]'s span plus the tick bookkeeping and the `CNTV_CVAL_EL0`
/// write.
///
/// A miss is this exceeding one tick period, 10,000,000 instructions at 100 Hz, so the bound is
/// about 4,000 times tighter, and no host can move it. See notes/instruction-clock.md.
#[cfg(feature = "icount")]
pub(crate) const HANDLER_BOUND: u64 = 2_500;

/// Execute `2 * iters` instructions and return that count, so `icount::calibrate` can check the
/// virtual clock against a known instruction count and refuse to measure anything if this boot is
/// not on the instrument.
///
/// Assembly, because the count must be known: `subs`/`b.ne` is two instructions per iteration.
/// The return counts the loop body only, so the caller's tolerance is a percentage rather than an
/// equality.
#[cfg(feature = "icount")]
pub(crate) fn calibration_loop(iters: u64) -> u64 {
    // SAFETY: a self-contained counted loop in a caller-saved scratch register. It touches no
    // memory, makes no call, and leaves the register dead, which is what the operand spec and
    // `nomem`/`nostack` state.
    unsafe {
        core::arch::asm!(
            "2:",
            "subs {n}, {n}, #1",
            "b.ne 2b",
            n = inout(reg) iters => _,
            options(nomem, nostack),
        );
    }
    2 * iters
}

/// Deadlines already past when the handler re-armed, per core. Zero unless the handler took longer
/// than a tick period or the emulator was descheduled.
static MISSED_TICKS: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

/// This core's missed ticks.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // this file's tests are the callers
pub fn missed_ticks() -> u64 {
    missed_ticks_on(cpu::id())
}

/// A named core's missed ticks, for the reason [`ticks_on`] gives.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn missed_ticks_on(cpu: usize) -> u64 {
    MISSED_TICKS[cpu].load(Ordering::Relaxed)
}

/// This core's next armed deadline: `CNTV_CVAL_EL0`, the grid cell [`rearm`] advances from.
///
/// Banked per core. Read by the drift test and `icount::run`'s claim 4, which assert the re-arm
/// law: one interval per delivered tick.
#[cfg_attr(
    all(not(any(test, feature = "system_tests")), not(feature = "icount")),
    allow(dead_code)
)]
pub fn deadline() -> u64 {
    CNTV_CVAL_EL0.get()
}

/// Why a miss happened, kept only in test builds.
///
/// The last miss's counter and deadline, plus a count, so a report says how late: a few hundred
/// cycles is a slow handler, a whole period a descheduled emulator.
#[cfg(any(test, feature = "system_tests"))]
pub mod miss_detail {
    use core::sync::atomic::{AtomicU64, Ordering};

    use crate::cpu::{self, MAX_CPUS};

    static NOW: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];
    static NEXT: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];
    static COUNT: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

    /// Record the counter and the deadline it had already passed. Called from `rearm`, in
    /// interrupt context, so this is three relaxed stores and nothing else.
    pub(crate) fn record(now: u64, next: u64) {
        let id = cpu::id();
        NOW[id].store(now, Ordering::Relaxed);
        NEXT[id].store(next, Ordering::Relaxed);
        COUNT[id].fetch_add(1, Ordering::Relaxed);
    }

    /// This core's last miss as `(now, next, count)`. `now - next` is how late the re-arm was, in
    /// counter ticks; compare it against `timer::interval()` to tell a slow handler from a
    /// descheduled emulator.
    pub fn last() -> (u64, u64, u64) {
        let id = cpu::id();
        (
            NOW[id].load(Ordering::Relaxed),
            NEXT[id].load(Ordering::Relaxed),
            COUNT[id].load(Ordering::Relaxed),
        )
    }
}

/// The tick handler: count, re-arm, return. Re-arming is what lowers the line.
///
/// Context: interrupt context, IRQs masked; records and defers (DECISIONS §9).
pub(crate) fn tick() {
    TICKS[cpu::id()].fetch_add(1, Ordering::Relaxed);
    // Test builds: fail fast on a hung test rather than block the run.
    #[cfg(any(test, feature = "system_tests"))]
    crate::testing::watchdog_tick();
    rearm(INTERVAL.load(Ordering::Relaxed));
}

/// This core's ticks since it started.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // this file's tests are the callers
pub fn ticks() -> u64 {
    ticks_on(cpu::id())
}

/// Ticks since boot on a **named** core, which is what a test needs when the thread doing the
/// measuring can move between the two reads.
///
/// A kernel thread can be stolen by an idle core at any preemption point (§28 (SMP placement)),
/// so two reads of [`ticks`] may be of two counters. See notes/load-sensitive-assertions.md.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) fn ticks_on(cpu: usize) -> u64 {
    TICKS[cpu].load(Ordering::Relaxed)
}

/// The raw counter: monotonic, and counting while interrupts are masked.
pub fn now() -> u64 {
    CNTVCT_EL0.get()
}

pub fn frequency() -> u64 {
    CNTFRQ_EL0.get()
}

/// **Refuse the boot if `CNTFRQ_EL0` and the device tree's timer rate disagree.**
///
/// `CNTFRQ_EL0` is written by firmware, and `arm,armv8-timer`'s optional `clock-frequency` exists
/// for boards where firmware gets it wrong. This refuses rather than preferring either (calef,
/// 2026-09-21 UTC). QEMU's `virt` states no such property, so only the absent path has run. See
/// notes/aarch64-port.md, "The counter's rate is checked against the device tree".
///
/// # Panics
///
/// If the tree's rate and the register disagree, naming both, or if the tree is unreadable.
pub(crate) fn check_frequency_against_device_tree(dtb_ptr: usize) {
    // SAFETY: the pointer firmware handed us, whose magic `memory::init` has already checked, named
    // through the boot map's direct region. Same read `isa::init` makes of the same pointer.
    let dt = unsafe {
        device_tree_blob::DeviceTreeBlob::from_ptr(
            super::mmu::phys_to_virt(dtb_ptr as u64) as *const u8
        )
    }
    .expect("device tree is unreadable");

    let Ok(Some(bytes)) = dt.node_prop_compatible(b"arm,armv8-timer", b"clock-frequency") else {
        return;
    };
    // The binding spells one cell; anything else is not understood, and is not a mismatch.
    let Ok(cell) = <[u8; 4]>::try_from(bytes) else {
        return;
    };
    let stated = u64::from(u32::from_be_bytes(cell));

    let register = CNTFRQ_EL0.get();
    assert_eq!(
        stated, register,
        "the device tree says this timer runs at {stated} Hz and CNTFRQ_EL0 says {register} Hz; \
         one of them is wrong and nothing here can tell which"
    );
}

/// Milliseconds since boot, from the counter rather than from the tick count.
///
/// Not `ticks() * 10`, which undercounts every missed tick.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // this file's tests are the callers
pub fn uptime_ms() -> u64 {
    let freq = CNTFRQ_EL0.get();
    if freq == 0 {
        return 0;
    }
    now() * 1000 / freq
}

/// **Is this core's tick raised and waiting** (`CNTV_CTL_EL0.ISTATUS`)? True once the deadline has
/// passed and the timer has signalled it, until the handler moves `CNTV_CVAL_EL0` forward, whether
/// or not interrupts are masked.
///
/// The emulator can raise the timer tens of milliseconds after the deadline, so tests that hold a
/// tick across a mask wait for this bit. See notes/load-sensitive-assertions.md.
///
/// Name: ratified 2026-09-24 (calef, the Rust predicate-naming rule in design/naming.md). Refused
/// `tick_pending` (a bare participle reads as a getter, and Rust asks the question with `is_`).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn is_tick_pending() -> bool {
    CNTV_CTL_EL0.is_set(CNTV_CTL_EL0::ISTATUS)
}

/// Busy-wait `counter_ticks` on the counter, so it works with interrupts masked. A `bench` boot
/// has no caller.
#[cfg_attr(feature = "bench", allow(dead_code))]
pub fn spin_for(counter_ticks: u64) {
    let start = now();
    while now().wrapping_sub(start) < counter_ticks {
        core::hint::spin_loop();
    }
}

/// Counter ticks in one timer period.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // this file's tests are the callers
pub fn interval() -> u64 {
    INTERVAL.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    //! Tests for the timer, and for the thing the whole locking discipline was written to
    //! prevent.

    /// **The cycle-counter grant opens and closes `PMUSERENR_EL0.CR`, and opens nothing else.**
    ///
    /// Reads the register out of the core, not the cache. Each call is made twice because the
    /// switch makes it on every switch and the unchanged path must be right too. Leaves zero.
    #[test_case]
    fn the_cycle_counter_grant_moves_cr_and_nothing_else() {
        if !super::is_cycle_counter_grantable() {
            crate::testing::skip!("this core has no PMUv3, so there is no register to grant");
        }
        super::set_cycle_counter_grant(true);
        super::set_cycle_counter_grant(true);
        assert_eq!(
            read_pmuserenr(),
            1 << 2,
            "the grant should set CR and only CR",
        );

        super::set_cycle_counter_grant(false);
        super::set_cycle_counter_grant(false);
        assert_eq!(
            read_pmuserenr(),
            0,
            "the cycle counter was left open to EL0"
        );
    }

    /// `PMUSERENR_EL0`, read back out of the core. Only call this where
    /// [`is_cycle_counter_grantable`](super::is_cycle_counter_grantable) is true: without
    /// `FEAT_PMUv3` the read is as UNDEFINED as the write.
    fn read_pmuserenr() -> u64 {
        let value: u64;
        // SAFETY: the caller has checked `is_cycle_counter_grantable`, so the register exists and
        // an `mrs` from it is a legal EL1 operation. It touches no memory and clobbers no flags,
        // which the options state.
        unsafe {
            core::arch::asm!("mrs {}, pmuserenr_el0", out(reg) value, options(nomem, nostack, preserves_flags));
        }
        value
    }
    /// The heartbeat is beating.
    #[test_case]
    fn the_timer_is_ticking() {
        use crate::arch::timer;

        // Name the core, for `ticks_on`'s reason.
        let cpu = crate::cpu::id();
        let before = timer::ticks_on(cpu);
        // Waited for, not spun for: the emulator raises the timer up to 86 ms late.
        let ticked = within_periods(RAISE_BOUND_PERIODS, || timer::ticks_on(cpu) > before);

        assert!(
            ticked,
            "no timer interrupt in three tick periods: the GIC or the timer is not delivering"
        );
    }

    /// Ticks arrive at the configured rate, proven by the grid rather than by the wall clock.
    ///
    /// Over a window with no miss, [`rearm`] moves `CNTV_CVAL_EL0` by exactly one interval per
    /// delivered tick; re-arming with `TVAL` fails it on the first tick. Kept in lockstep with the
    /// riscv64 twin. See notes/aarch64-port.md, "What the timer tests can and cannot claim".
    #[test_case]
    fn ticks_arrive_at_the_configured_rate() {
        use crate::arch::timer;

        // Re-read until the tick count and the core id bracket the other reads unchanged: a tick
        // between them would skew the snapshot, and the statics and CVAL are per core.
        let snapshot = || loop {
            let cpu = crate::cpu::id();
            let t = timer::ticks();
            let m = timer::missed_ticks();
            let d = timer::deadline();
            let c = timer::now();
            if timer::ticks() == t && crate::cpu::id() == cpu {
                break (cpu, t, m, d, c);
            }
        };

        // A miss re-anchors the grid, so a window holding one proves nothing: retry it. An
        // exhausted budget skips rather than fails, and the budget is not widened.
        const ATTEMPTS: u32 = 8;
        let mut attempts = 0;
        let measured = loop {
            let (k0, t0, m0, d0, c0) = snapshot();
            timer::spin_for(timer::frequency() / 4); // a quarter of a second, by the counter
            let (k1, t1, m1, d1, c1) = snapshot();

            if k0 == k1 && m0 == m1 && t1 - t0 >= 2 {
                break Some((t1 - t0, d1 - d0, (c1 - c0) / timer::interval()));
            }
            attempts += 1;
            if attempts == ATTEMPTS {
                break None;
            }
        };

        let Some((elapsed_ticks, deadline_delta, expected)) = measured else {
            let (now, next, misses) = super::miss_detail::last();
            crate::println!(
                "    (UNMEASURED: no miss-free window in {} tries, so the re-arm law was not \
                 tested this run. {} misses recorded on this core, the last re-armed {} counter \
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
            // A test that measures nothing is a skip, not a pass (milestone 214 (a test that
            // prints "skipping" and returns is counted as passed)).
            crate::testing::skip!(
                "no miss-free window in which to measure the re-arm law this run"
            );
        };

        assert_eq!(
            deadline_delta,
            elapsed_ticks * timer::interval(),
            "timer drift: {elapsed_ticks} ticks moved CNTV_CVAL_EL0 off the grid. Re-arming with \
             a RELATIVE countdown (TVAL) instead of an absolute deadline (CVAL) does exactly this."
        );

        // Descheduling only drops ticks, so a surplus means the timer fires faster than the grid.
        assert!(
            elapsed_ticks <= expected + 1,
            "{elapsed_ticks} ticks in {expected} periods: the timer fires faster than configured"
        );
    }

    // No wall-clock test bounds the handler's latency: a slow handler and a descheduled host land
    // in the same band. `script/icount` bounds it in instructions. See
    // notes/load-sensitive-assertions/timer-assertion-disposition.md.

    /// **The cost of masking, made visible.**
    ///
    /// A lock held across more than one tick period loses a tick, which is why DECISIONS §9 keeps
    /// critical sections short. If it stopped costing one, `IrqSafeMutex` would have stopped
    /// masking. See notes/interrupts.md.
    #[test_case]
    fn a_long_critical_section_costs_a_tick() {
        use crate::arch::timer;
        use crate::sync::{IrqSafeMutex, rank};

        static M: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::PAGE_FRAMES, 0);

        // Read inside the critical section, where this thread cannot be preempted or migrated.
        let (cpu, before) = {
            let _guard = M.lock();
            let cpu = crate::cpu::id();
            let before = timer::missed_ticks_on(cpu);
            timer::spin_for(timer::interval() * 2 + timer::interval() / 2);
            (cpu, before)
        };

        // Bounded, not one period: a descheduled host only makes the delivery later.
        assert!(
            within_periods(20, || timer::missed_ticks_on(cpu) > before),
            "holding a lock across two tick periods did NOT lose a tick, which means \
             IrqSafeMutex is not masking interrupts and the deadlock is live"
        );
    }

    /// Uptime comes from the *counter*, not the tick count.
    ///
    /// A tick count would undercount every missed tick.
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

    /// **No tick lands while a lock is held**, the deadlock DECISIONS §9 and notes/locking.md
    /// exist to prevent. Confirms ticks flow, holds a lock across three periods, asserts none
    /// landed and one is pending, then releases and watches them resume. See notes/interrupts.md,
    /// "The test the whole locking discipline was written for".
    #[test_case]
    fn holding_a_lock_masks_the_timer() {
        use crate::arch::{interrupts, timer};
        use crate::sync::{IrqSafeMutex, rank};

        static M: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::PAGE_FRAMES, 0);

        assert!(
            interrupts::is_enabled(),
            "test setup: interrupts should be on"
        );

        // The timer is alive. Core-scoped, for `ticks_on`'s reason.
        let alive_on = crate::cpu::id();
        let t0 = timer::ticks_on(alive_on);
        // Waited for, for `the_timer_is_ticking`'s reason.
        assert!(
            within_periods(RAISE_BOUND_PERIODS, || timer::ticks_on(alive_on) > t0),
            "the timer is not ticking at all"
        );

        // Both reads inside the critical section: a tick between a read and the mask would be
        // charged to the lock, and a descheduled vCPU takes its late tick at the first instruction.
        let (cpu, before) = {
            let _guard = M.lock();
            let cpu = crate::cpu::id();
            let before = timer::ticks_on(cpu);

            timer::spin_for(timer::interval() * 3);

            assert_eq!(
                timer::ticks_on(cpu),
                before,
                "A TIMER INTERRUPT FIRED WHILE A LOCK WAS HELD. IrqSafeMutex is not masking, \
                 and the deadlock in notes/locking.md is live: a handler that touched this lock \
                 would spin forever waiting for code that cannot run."
            );

            // A raised tick makes the release below a test of `restore`, not of the host. The
            // first assertion is repeated because the wait is part of the window.
            assert!(
                within_raise_bound(timer::is_tick_pending),
                "the timer was never raised in a second with the lock held, so the release below \
                 would test nothing: the timer is not being armed, or the emulator stopped \
                 delivering it"
            );
            assert_eq!(
                timer::ticks_on(cpu),
                before,
                "A TIMER INTERRUPT FIRED WHILE A LOCK WAS HELD. IrqSafeMutex is not masking, \
                 and the deadlock in notes/locking.md is live: a handler that touched this lock \
                 would spin forever waiting for code that cannot run."
            );
            (cpu, before)
        };

        // By core index: dropping the guard is a preemption point.
        assert!(
            within_periods(20, || timer::ticks_on(cpu) > before),
            "interrupts did not resume after the lock was released: `restore` is broken"
        );
    }

    /// How long a wait on the timer being raised may take, in tick periods: one second, against a
    /// worst case measured at under nine periods (86 ms). A leak trap, not a timing claim.
    ///
    /// Name: provisional, minted 2026-09-24 (`cda66d656`, waiting for the tick to be raised).
    const RAISE_BOUND_PERIODS: u32 = 100;

    /// Spin until `cond`, bounded by [`RAISE_BOUND_PERIODS`] of the free-running counter, checking
    /// continuously rather than once a period. For waits made **with interrupts masked**, where
    /// nothing can change `cond` except the hardware and a period's granularity would only add
    /// latency.
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

    /// Wait for `cond`, bounded in tick periods of the free-running counter, checking once a
    /// period. Used only on the "has it happened yet" side of a property, where a busy host makes
    /// a late pass rather than a wrong answer. See notes/load-sensitive-assertions.md.
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
