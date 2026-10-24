//! **Start the cycle counter, check it counts, and read it** (milestone 74, the aarch64 half).
//!
//! `PMCCNTR_EL0` is stopped until something writes `PMCR_EL0.E` and `PMCNTENSET_EL0.C`. As in
//! `arch::riscv64::pmu` and `arch::x86_64::pmu`, the counter is checked to move before it is
//! believed, and why it does not is recorded as a value the boot line prints. [`init_this_cpu`]
//! runs on every core and assigns each register rather than read-modify-writing it. See
//! notes/pmu.md, "How this kernel starts it, and what it prints".

//! # BUGS
//!
//! - **Nothing here has run on silicon.**
//! - **The did-it-move check cannot tell a real counter from QEMU's**, which under `-icount` is the
//!   instruction count (a round ratio is the tell).
//! - **A trap here is a hang or a fault before the banner**: with `MDCR_EL3.TPM` set every PMU
//!   access traps to EL3, first the `PMUSERENR_EL0` write in `timer::init`.
//! - **Each core's counter is its own**: a caller of [`cycles`] that may migrate checks the core.
//! - **The counter is never stopped.**

use core::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use aarch64_cpu::registers::{CNTVCT_EL0, ID_AA64DFR0_EL1};
use tock_registers::interfaces::Readable;

use crate::cpu::{self, MAX_CPUS};

/// **The value written to `PMCCFILTR_EL0` on every core: zero, so EL0 and EL1 are counted and EL2
/// is not.** calef's ruling A1, 2026-10-07 (UTC), recorded in milestone 353's block.
///
/// Zero counts EL0 and EL1, Secure and Non-secure, and not EL2. See notes/pmu.md, "Why
/// `PMCCFILTR_EL0` is zero".
///
/// Name: provisional, minted by milestone 353's lane on 2026-10-07 (UTC), replacing
/// `PMCCFILTR_PROVISIONAL` once the value stopped being provisional. Names the levels counted,
/// which is the fact a reader needs. `PMCCFILTR_A1` was considered and refused (an option letter
/// means nothing outside the block that lettered it).
pub const PMCCFILTR_COUNT_EL0_AND_EL1: u64 = 0;

/// `PMCR_EL0.E`: enable the counters.
const PMCR_E: u64 = 1 << 0;
/// `PMCR_EL0.C`: write 1 to zero the cycle counter.
const PMCR_C: u64 = 1 << 2;
/// `PMCR_EL0.LC`: record cycle-counter overflow at 64 bits rather than 32.
const PMCR_LC: u64 = 1 << 6;
/// `PMCR_EL0.D`: count every 64th cycle. Asserted clear by the tests.
#[cfg(test)]
const PMCR_D: u64 = 1 << 3;
/// `PMCNTENSET_EL0.C`, and `PMCNTENCLR_EL0.C`: the cycle counter's enable bit.
const PMCNTEN_C: u64 = 1 << 31;

/// How long [`init_this_cpu`]'s check spins, in generic-timer ticks. 100 ticks is 1.6 us at QEMU's
/// 62.5 MHz and 5.2 us at the TX1's 19.2 MHz, long enough that a real cycle counter has moved by
/// thousands and short enough that four cores doing it are not felt in a boot. The riscv64 half
/// uses the same number for the same reason.
const CHECK_TICKS: u64 = 100;

/// **Why there is or is not a cycle counter on a core**, decided once per core by
/// [`init_this_cpu`].
///
/// The same vocabulary as `arch::riscv64::pmu::CycleCounter` and `arch::x86_64::pmu::CycleCounter`,
/// with the members this mechanism can actually produce. There is no firmware to refuse a counter
/// and no CSR to be out of range, so the list is short: the PMU is absent, or it was started and did
/// not move, or it runs.
///
/// Ordered so the discriminant is the stored value and `Unknown` is zero, which is what a core that
/// never ran [`init_this_cpu`] leaves behind.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum CycleCounter {
    /// [`init_this_cpu`] has not run on this core.
    Unknown = 0,
    /// `ID_AA64DFR0_EL1.PMUVer` is 0 (no PMU) or 0xf (an IMPLEMENTATION DEFINED one that is not
    /// `PMUv3`). `PMCCNTR_EL0` and every register that controls it are UNDEFINED, so nothing was
    /// written and nothing will be read.
    NoPmuV3,
    /// **Enabled, read twice across a timed spin, and unmoved.** A cycle counter cannot do that on a
    /// core that is executing, so it is refused rather than reported as zero. On a board this is
    /// the outcome a secure world that prohibits Non-secure counting produces (`MDCR_EL3.SPME` and
    /// friends), and the boot line says so.
    Stuck,
    /// Enabled and advancing.
    Running,
}

/// [`CycleCounter`] per core, as a byte. Written once by that core's [`init_this_cpu`] before the
/// core is counted online, read-only afterwards. Atomics rather than a lock: nothing here nests with
/// another lock, so it earns no rank in `sync::rank`.
static OUTCOME: [AtomicU8; MAX_CPUS] =
    [const { AtomicU8::new(CycleCounter::Unknown as u8) }; MAX_CPUS];

/// Each core's `PMCR_EL0` as read back after the enable, for the boot line: `N` (bits 15:11) is how
/// many event counters EL1 can see, which is `MDCR_EL2.HPMN` when an EL2 is implemented and so is
/// the visible trace of `boot.s` having set it.
static PMCR_READBACK: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

/// What the check on each core measured, cycles and generic-timer ticks over the same window, kept
/// for the boot line. **The ticks are what elapsed, not [`CHECK_TICKS`]**: the loop exits at the
/// first read past the threshold, and under TCG that first read can come milliseconds late (the
/// block is being translated), so printing the constant would divide by the wrong number. On the
/// `Stuck` outcome the window the counter failed to move across is the diagnostic.
static CHECK_CYCLES: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];
static CHECK_ELAPSED: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

/// **What `PMCCFILTR_EL0` held before this kernel overwrote it**, per core, for the boot line.
///
/// Evidence for milestone 25 (cross-OS comparison), never state this kernel uses: seL4's published
/// TX1 figures counted whatever firmware left here. See notes/pmu.md, "Why `PMCCFILTR_EL0` is zero".
static FIRMWARE_FILTER: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

/// **Does this core implement `PMUv3`**, and therefore every register this module and
/// `timer::close_cycle_counter_to_el0` write?
///
/// `ID_AA64DFR0_EL1.PMUVer`: 0 is no PMU, 0xf is an IMPLEMENTATION DEFINED PMU that does not follow
/// `PMUv3` and so carries none of these registers either. Per core, because `PMUVer` is a per-PE ID
/// register and this kernel does not assume a homogeneous machine anywhere else.
pub(super) fn is_pmuv3_present() -> bool {
    let pmuver = ID_AA64DFR0_EL1.read(ID_AA64DFR0_EL1::PMUVer);
    pmuver != 0 && pmuver != 0xf
}

/// **Start this core's cycle counter, check it moves, and record the answer.**
///
/// Called from `timer::init`, which every core runs (core 0 from `kernel_main`, each secondary from
/// `smp::secondary_main`), after `PMUSERENR_EL0` has been closed. Init-time only: nothing here is on
/// the context-switch path, which milestone 237 measured and keeps clean.
pub(crate) fn init_this_cpu() {
    let cpu = cpu::id();
    if !is_pmuv3_present() {
        OUTCOME[cpu].store(CycleCounter::NoPmuV3 as u8, Ordering::Release);
        return;
    }

    // What firmware left, before it is overwritten; see `FIRMWARE_FILTER` for why it is kept.
    let inherited: u64;
    // SAFETY: as the block below: PMUv3 is present, so the register exists; a read has no effect.
    unsafe {
        core::arch::asm!("mrs {}, pmccfiltr_el0", out(reg) inherited, options(nomem, nostack, preserves_flags));
    }
    FIRMWARE_FILTER[cpu].store(inherited, Ordering::Relaxed);

    // SAFETY: `is_pmuv3_present` read this core's own `ID_AA64DFR0_EL1.PMUVer` and found PMUv3, so
    // all three registers exist and an EL1 access is not UNDEFINED. `MDCR_EL2.TPM` is clear
    // (boot.s, when this kernel was entered at EL2) so the accesses do not trap to EL2. The values
    // change what the PMU counts and nothing else: no memory, no translation, no exception routing.
    // The `isb` makes the enable take effect before the check below reads the counter.
    unsafe {
        core::arch::asm!(
            "msr pmccfiltr_el0, {filter}",
            "msr pmcr_el0, {pmcr}",
            "msr pmcntenset_el0, {cnten}",
            "isb",
            filter = in(reg) PMCCFILTR_COUNT_EL0_AND_EL1,
            pmcr = in(reg) PMCR_E | PMCR_C | PMCR_LC,
            cnten = in(reg) PMCNTEN_C,
            options(nomem, nostack, preserves_flags)
        );
    }
    PMCR_READBACK[cpu].store(read_pmcr(), Ordering::Relaxed);

    // Does it count? A short window on the generic timer, the one clock this kernel has that does
    // not depend on the answer.
    let (first, t0) = (read_counter(), CNTVCT_EL0.get());
    let mut t1 = t0;
    while t1.wrapping_sub(t0) < CHECK_TICKS {
        core::hint::spin_loop();
        t1 = CNTVCT_EL0.get();
    }
    let moved = read_counter().wrapping_sub(first);
    CHECK_CYCLES[cpu].store(moved, Ordering::Relaxed);
    CHECK_ELAPSED[cpu].store(t1.wrapping_sub(t0), Ordering::Relaxed);

    let outcome = if moved == 0 {
        CycleCounter::Stuck
    } else {
        CycleCounter::Running
    };
    // Last, and Release: a reader that sees the outcome sees the records written before it.
    OUTCOME[cpu].store(outcome as u8, Ordering::Release);
}

/// **Why there is or is not a cycle counter on this core.** See [`CycleCounter`].
pub fn outcome() -> CycleCounter {
    outcome_on(cpu::id())
}

/// [`outcome`] for a named core, for the boot line and for a test whose thread may run anywhere.
pub fn outcome_on(cpu: usize) -> CycleCounter {
    match OUTCOME[cpu].load(Ordering::Acquire) {
        x if x == CycleCounter::NoPmuV3 as u8 => CycleCounter::NoPmuV3,
        x if x == CycleCounter::Stuck as u8 => CycleCounter::Stuck,
        x if x == CycleCounter::Running as u8 => CycleCounter::Running,
        _ => CycleCounter::Unknown,
    }
}

/// **This core's cycle count**, or `None` when this core has no counter worth believing.
///
/// One `mrs`, no trap and no call, so a caller may put this on either side of the thing it is
/// measuring. **Only meaningful as a difference of two reads on the same core**: see this module's
/// BUGS. It counts EL0 and EL1, per [`PMCCFILTR_COUNT_EL0_AND_EL1`].
// The consumers are the bench probe and the tests, so a production boot has no caller.
#[cfg_attr(
    not(any(test, feature = "system_tests", feature = "bench")),
    allow(dead_code)
)]
pub fn cycles() -> Option<u64> {
    if outcome() != CycleCounter::Running {
        return None;
    }
    Some(read_counter())
}

/// One `mrs` of `PMCCNTR_EL0`. Only called once [`init_this_cpu`] found `PMUv3` on this core.
fn read_counter() -> u64 {
    let value: u64;
    // SAFETY: reached only on a core whose PMUVer reported PMUv3 (the outcome is not `NoPmuV3`), so
    // the register exists. A read has no side effect and touches no memory.
    unsafe {
        core::arch::asm!("mrs {}, pmccntr_el0", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// One `mrs` of `PMCR_EL0`, under the same precondition as [`read_counter`].
fn read_pmcr() -> u64 {
    let value: u64;
    // SAFETY: as `read_counter`: PMUv3 is present on this core, and the read has no side effect.
    unsafe {
        core::arch::asm!("mrs {}, pmcr_el0", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// The boot line. Printed once, from core 0, after every core has run [`init_this_cpu`].
///
/// It names core 0's answer in full and how many of the cores that reached init agree, because the
/// failure worth seeing on a board is one core disagreeing (a big.LITTLE part, or a secondary whose
/// firmware state differs), and a line about core 0 alone would hide it.
pub(crate) fn print_summary() {
    let boot = cpu::id();
    let mut initialised = 0;
    let mut running = 0;
    let mut first_other: Option<(usize, CycleCounter)> = None;
    for cpu in 0..MAX_CPUS {
        let o = outcome_on(cpu);
        if o == CycleCounter::Unknown {
            continue;
        }
        initialised += 1;
        if o == CycleCounter::Running {
            running += 1;
        }
        if first_other.is_none() && o != outcome_on(boot) {
            first_other = Some((cpu, o));
        }
    }

    let pmcr = PMCR_READBACK[boot].load(Ordering::Relaxed);
    let event_counters = (pmcr >> 11) & 0x1f;
    let checked = CHECK_CYCLES[boot].load(Ordering::Relaxed);
    let elapsed = CHECK_ELAPSED[boot].load(Ordering::Relaxed);
    let inherited = FIRMWARE_FILTER[boot].load(Ordering::Relaxed);
    match outcome_on(boot) {
        CycleCounter::Running => crate::println!(
            "  cycles      : PMCCNTR_EL0 running on {running} of {initialised} cores ({checked} over \
             {elapsed} ticks at boot), {event_counters} event counters visible, PMCCFILTR_EL0 \
             {PMCCFILTR_COUNT_EL0_AND_EL1:#x} (EL0+EL1 counted, EL2 not)"
        ),
        CycleCounter::Stuck => crate::println!(
            "  cycles      : PMCCNTR_EL0 enabled but did not advance over {elapsed} ticks; refused \
             ({running} of {initialised} cores running)"
        ),
        CycleCounter::NoPmuV3 => crate::println!(
            "  cycles      : no PMUv3 (ID_AA64DFR0_EL1.PMUVer {:#x}); ticks only",
            ID_AA64DFR0_EL1.read(ID_AA64DFR0_EL1::PMUVer)
        ),
        CycleCounter::Unknown => {
            crate::println!(
                "  cycles      : arch::pmu::init_this_cpu has not run on the boot core"
            );
        }
    }
    if outcome_on(boot) != CycleCounter::NoPmuV3 {
        // The one number that says what seL4's published TX1 figures counted; see `FIRMWARE_FILTER`.
        // Read for milestone 25's comparison only: it never changes what this kernel writes.
        crate::println!(
            "  cycles      : firmware left PMCCFILTR_EL0 {inherited:#x} on this core before it was overwritten"
        );
    }
    if let Some((cpu, o)) = first_other {
        crate::println!("  cycles      : core {cpu} disagrees with the boot core: {o:?}");
    }
}

#[cfg(test)]
mod tests {
    //! What only a boot can say. **None of these is a claim about cycles**: under QEMU-TCG the
    //! counter runs off the emulator's virtual clock, so every assertion is about the plumbing, that
    //! the registers hold what was written, that the outcome is decided on every core that ran init,
    //! and that a counter called `Running` moves.

    use super::*;

    /// **Every core that came online decided an answer, and the answer agrees with the ID register.**
    ///
    /// `Unknown` on the boot core would mean `timer::init` never reached this module. A `NoPmuV3`
    /// on a core that reports `PMUv3`, or the reverse, would mean the two gates (this module's and
    /// `timer`'s) disagree about which registers exist, which on a board is an undefined-instruction
    /// fault rather than a wrong number.
    #[test_case]
    fn every_cpu_decided_and_the_boot_cpu_agrees_with_pmuver() {
        let here = outcome();
        assert_ne!(
            here,
            CycleCounter::Unknown,
            "timer::init did not run init_this_cpu here"
        );
        assert_eq!(
            here == CycleCounter::NoPmuV3,
            !is_pmuv3_present(),
            "the outcome ({here:?}) and ID_AA64DFR0_EL1.PMUVer disagree about whether there is a PMU"
        );
        assert_eq!(cycles().is_some(), here == CycleCounter::Running);

        for cpu in crate::smp::online_cpus() {
            assert_ne!(
                outcome_on(cpu),
                CycleCounter::Unknown,
                "core {cpu} is online and never ran init_this_cpu: its counter is whatever reset left"
            );
        }
    }

    /// **The three registers hold what [`init_this_cpu`] wrote.**
    ///
    /// Read back from the core rather than trusted, because the failure this catches is the one
    /// nothing else would: an enable that the hardware (or an emulator) silently did not take, or a
    /// `D` bit that survived and makes every count 64 times too small.
    #[test_case]
    fn the_counter_is_enabled_undivided_and_filtered_as_written() {
        if !is_pmuv3_present() {
            crate::testing::skip!("this core has no PMUv3, so there are no registers to read back");
        }
        let pmcr = read_pmcr();
        assert_ne!(
            pmcr & PMCR_E,
            0,
            "PMCR_EL0.E is clear: the counter is stopped"
        );
        assert_eq!(
            pmcr & PMCR_D,
            0,
            "PMCR_EL0.D is set: the counter divides by 64"
        );

        let (cnten, filter): (u64, u64);
        // SAFETY: PMUv3 is present on this core (checked above), so both registers exist; reads have
        // no side effect.
        unsafe {
            core::arch::asm!(
                "mrs {}, pmcntenset_el0",
                "mrs {}, pmccfiltr_el0",
                out(reg) cnten,
                out(reg) filter,
                options(nomem, nostack, preserves_flags)
            );
        }
        assert_ne!(
            cnten & PMCNTEN_C,
            0,
            "PMCNTENSET_EL0.C is clear: the cycle counter is not enabled"
        );
        assert_eq!(
            filter, PMCCFILTR_COUNT_EL0_AND_EL1,
            "PMCCFILTR_EL0 does not hold the value init wrote"
        );
    }

    /// **QEMU reaches one of the answers it can give, and `cycles` agrees with the gate.**
    ///
    /// A set rather than one value, for the reason the riscv64 twin learned: what an emulator answers
    /// is a property of its configuration. `cortex-a72` under TCG models `PMUv3` and drives the counter
    /// from its virtual clock, so `Running` is expected there; `Stuck` is what a model that left the
    /// counter constant would produce, and is handled. What must never happen is a number handed
    /// back under any outcome but `Running`.
    #[test_case]
    fn qemu_reaches_a_decided_answer() {
        let o = outcome();
        assert!(
            matches!(
                o,
                CycleCounter::Running | CycleCounter::Stuck | CycleCounter::NoPmuV3
            ),
            "outcome {o:?} on a booted core"
        );
        assert_eq!(
            cycles().is_some(),
            o == CycleCounter::Running,
            "{o:?} handed back a count"
        );
    }

    /// **A `Running` counter still moves.** [`init_this_cpu`] checked once at boot; this re-checks
    /// after the whole boot has run, which is what would catch something later stopping it (a stray
    /// `PMCR_EL0` write, or a `PMCNTENCLR` from code that thinks it owns the PMU).
    ///
    /// Not a measurement: under TCG the delta is virtual time. Interrupts are masked across the two
    /// reads so both are on this core.
    #[test_case]
    fn a_running_counter_advances() {
        let irqs = crate::arch::interrupts::disable();
        let Some(first) = cycles() else {
            crate::arch::interrupts::restore(irqs);
            return;
        };
        let t0 = CNTVCT_EL0.get();
        while CNTVCT_EL0.get().wrapping_sub(t0) < 1000 {
            core::hint::spin_loop();
        }
        let second = cycles().expect("the counter did not vanish mid-test");
        crate::arch::interrupts::restore(irqs);
        assert!(
            second.wrapping_sub(first) > 0,
            "PMCCNTR_EL0 read {first} twice across 1000 ticks; something stopped it after boot"
        );
    }
}
