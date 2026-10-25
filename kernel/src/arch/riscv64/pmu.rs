//! Ask firmware for a hardware cycle counter through the SBI PMU extension, and read it.
//!
//! The counters are configured from M-mode only, so S-mode asks SBI PMU (EID `0x504D55`) for one
//! counter matching CPU cycles, started for the life of the boot on the boot hart, then reads its
//! CSR directly. This is cycles, not the fixed-rate `time` tick (notes/pmu.md). Nothing here has
//! run on silicon.
//!
//! See notes/riscv-cycle-counters.md.

//! # BUGS
//!
//! - Nothing here has run on silicon; under QEMU-TCG a green test proves the plumbing only.
//! - The boot hart only: SBI PMU counters are per hart, and [`init`] runs on the boot hart.
//! - A firmware counter is refused, since reading one costs an `ecall` per read.
//! - `counter_stop` is never called on the happy path; only [`stop`]'s test calls it.
//!
//! Details in notes/riscv-cycle-counters.md, BUGS.

use core::arch::asm;
use core::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};

use machine_discovery::riscv64::{
    CounterInfo, EID_PMU, EVENT_HW_CPU_CYCLES, PMU_CFG_AUTO_START, PMU_CFG_CLEAR_VALUE, SBI_PMU,
};

/// `sbi_pmu_num_counters`.
const FID_NUM_COUNTERS: usize = 0;
/// `sbi_pmu_counter_get_info`.
const FID_COUNTER_GET_INFO: usize = 1;
/// `sbi_pmu_counter_config_matching`.
const FID_COUNTER_CONFIG_MATCHING: usize = 2;
/// `sbi_pmu_counter_stop`.
const FID_COUNTER_STOP: usize = 4;

/// Why there is or is not a cycle counter, decided once by [`init`] and printed on the boot line,
/// because each cause of "no counter" needs a different fix.
///
/// The discriminant is the stored byte, and `Unknown` is zero, what an unreached [`init`] leaves.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(crate) enum CycleCounter {
    /// [`init`] has not run.
    Unknown = 0,
    /// Firmware does not implement the SBI PMU extension. Nothing to ask.
    NoPmuExtension,
    /// It does, but no counter on this hart can count `SBI_PMU_HW_CPU_CYCLES`
    /// (`SBI_ERR_NOT_SUPPORTED` from the match), or it has no counters at all.
    NoMatchingCounter,
    /// Firmware offered a counter it maintains itself. Refused: reading one costs an `ecall` per
    /// read, and an `ecall` inside a cycle measurement measures the `ecall`.
    FirmwareCounter,
    /// The CSR firmware named is outside the unprivileged counter block, so this kernel cannot name
    /// it in an instruction.
    UnreadableCsr,
    /// The counter was read twice across a spin and had not moved, as on QEMU's `rva23s64`.
    Stuck,
    /// A hardware counter, readable, and advancing.
    Running,
}

/// [`CycleCounter`] as a byte, written by [`init`] on the boot hart before anything reads it.
/// Its Release store of `Running` publishes the other statics.
static OUTCOME: AtomicU8 = AtomicU8::new(CycleCounter::Unknown as u8);

/// The CSR that reads the counter firmware gave us, or [`NO_CSR`]. Written as soon as firmware
/// names it, so the boot line can name a refused counter; [`OUTCOME`] is the gate.
static CYCLE_CSR: AtomicU32 = AtomicU32::new(NO_CSR);

/// "No counter", distinct from every legal CSR number (which are 12 bits).
const NO_CSR: u32 = u32::MAX;

/// The logical counter index firmware chose, for the boot line and [`stop`], written early for
/// [`CYCLE_CSR`]'s reason.
static CYCLE_COUNTER_IDX: AtomicU64 = AtomicU64::new(0);

/// How wide the counter is, in bits, for the boot line: a narrow counter wraps.
static CYCLE_BITS: AtomicU32 = AtomicU32::new(0);

/// Find a hardware counter that counts CPU cycles, start it, check it moves, and record how to
/// read it, or why not, in [`outcome`].
///
/// Called once from `kernel_main`, after [`super::isa::init`] probed the SBI extensions. Without
/// SBI PMU, [`cycles`] answers `None`. A refused counter is stopped. See
/// notes/riscv-cycle-counters.md.
pub fn init() {
    OUTCOME.store(CycleCounter::Unknown as u8, Ordering::Relaxed);
    CYCLE_CSR.store(NO_CSR, Ordering::Relaxed);

    if !super::isa::get().sbi.extensions.contains(SBI_PMU) {
        OUTCOME.store(CycleCounter::NoPmuExtension as u8, Ordering::Release);
        return;
    }

    let (err, count) = sbi_call(FID_NUM_COUNTERS, [0; 5]);
    if err != 0 || count == 0 {
        OUTCOME.store(CycleCounter::NoMatchingCounter as u8, Ordering::Release);
        return;
    }

    // One base and an XLEN-wide mask covers 64 counters; any beyond go unasked, never misread.
    let mask = if count >= 64 {
        u64::MAX as usize
    } else {
        (1usize << count) - 1
    };

    // Not `SKIP_MATCH`: firmware must match the event. `CLEAR_VALUE` drops an inherited count.
    let (err, idx) = sbi_call(
        FID_COUNTER_CONFIG_MATCHING,
        [
            0,
            mask,
            PMU_CFG_CLEAR_VALUE | PMU_CFG_AUTO_START,
            EVENT_HW_CPU_CYCLES,
            0,
        ],
    );
    if err != 0 {
        // `SBI_ERR_NOT_SUPPORTED` (-2): no counter here counts cycles, a legitimate machine.
        OUTCOME.store(CycleCounter::NoMatchingCounter as u8, Ordering::Release);
        return;
    }
    CYCLE_COUNTER_IDX.store(idx as u64, Ordering::Relaxed);

    let (err, raw) = sbi_call(FID_COUNTER_GET_INFO, [idx, 0, 0, 0, 0]);
    if err != 0 {
        stop_counter(idx);
        OUTCOME.store(CycleCounter::NoMatchingCounter as u8, Ordering::Release);
        return;
    }
    let info = CounterInfo::from_raw(raw as u64);

    // A firmware counter costs an `ecall` per read, so it is refused.
    let (Some(csr), Some(bits)) = (info.csr(), info.bits()) else {
        stop_counter(idx);
        OUTCOME.store(CycleCounter::FirmwareCounter as u8, Ordering::Release);
        return;
    };
    CYCLE_CSR.store(csr as u32, Ordering::Relaxed);
    CYCLE_BITS.store(bits, Ordering::Relaxed);

    // The read must be one instruction, so an unnameable CSR is refused too.
    let Some(first) = read_csr(csr) else {
        stop_counter(idx);
        OUTCOME.store(CycleCounter::UnreadableCsr as u8, Ordering::Release);
        return;
    };

    // A counter can be reported working and read zero. 100 `time` ticks moves any real one by
    // thousands.
    const CHECK_TICKS: u64 = 100;
    let t0 = super::timer::now();
    while super::timer::now() - t0 < CHECK_TICKS {
        core::hint::spin_loop();
    }
    if read_csr(csr) == Some(first) {
        stop_counter(idx);
        OUTCOME.store(CycleCounter::Stuck as u8, Ordering::Release);
        return;
    }

    // Last, and with Release: a reader that sees `Running` must see the CSR, index and width
    // written before it. Every read is `Acquire` on this one location for the same reason.
    OUTCOME.store(CycleCounter::Running as u8, Ordering::Release);
}

/// Why there is or is not a cycle counter on this hart. See [`CycleCounter`].
pub(crate) fn outcome() -> CycleCounter {
    match OUTCOME.load(Ordering::Acquire) {
        x if x == CycleCounter::NoPmuExtension as u8 => CycleCounter::NoPmuExtension,
        x if x == CycleCounter::NoMatchingCounter as u8 => CycleCounter::NoMatchingCounter,
        x if x == CycleCounter::FirmwareCounter as u8 => CycleCounter::FirmwareCounter,
        x if x == CycleCounter::UnreadableCsr as u8 => CycleCounter::UnreadableCsr,
        x if x == CycleCounter::Stuck as u8 => CycleCounter::Stuck,
        x if x == CycleCounter::Running as u8 => CycleCounter::Running,
        _ => CycleCounter::Unknown,
    }
}

/// Stop counter `idx` and discard the answer, on [`init`]'s give-up paths.
fn stop_counter(idx: usize) {
    let _ = sbi_call(FID_COUNTER_STOP, [idx, 1, 0, 0, 0]);
}

/// The cycle count on this hart (meaningful only on the boot hart), or `None` when no hardware
/// cycle counter was found.
///
/// One CSR read, no `ecall`. Meaningful only as a difference: the cycles may vary in frequency and
/// stop when the clock halts, so a count is not a duration.
// The bench probe and tests are the only callers; every build still configures and prints it.
#[cfg_attr(
    not(any(test, feature = "system_tests", feature = "bench")),
    allow(dead_code)
)]
pub(crate) fn cycles() -> Option<u64> {
    if outcome() != CycleCounter::Running {
        return None;
    }
    read_csr(CYCLE_CSR.load(Ordering::Relaxed) as u16)
}

/// Whether the counter [`init`] started is the `cycle` CSR (0xc00) itself, the one a granted
/// U-mode `rdcycle` reads. A `Running` outcome on that CSR means this hart read it from S-mode, so
/// firmware left `mcounteren.CY` set and a granted U-mode read will not trap.
///
/// The bench boot's `ipc_rtt_el0_cycles` probe asks before it grants, because a granted read that
/// traps kills the client and leaves the bench boot waiting forever for its report.
///
/// Name: provisional (the lane for milestone 101 (the L4 calibration), 2026-10-24 UTC).
#[cfg(all(feature = "bench", feature = "cycle_counter_grant"))]
pub(crate) fn counts_through_the_cycle_csr() -> bool {
    outcome() == CycleCounter::Running && CYCLE_CSR.load(Ordering::Relaxed) == 0xc00
}

/// How wide the counter is, in bits, or `None` when there is none.
pub(crate) fn cycle_counter_width() -> Option<u32> {
    if outcome() != CycleCounter::Running {
        return None;
    }
    Some(CYCLE_BITS.load(Ordering::Relaxed))
}

/// Stop the counter [`init`] started. Only its test calls it.
///
/// Returns the SBI error: 0 stopped, -8 (`SBI_ERR_ALREADY_STOPPED`) already stopped, -3 no counter.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) fn stop() -> isize {
    if outcome() != CycleCounter::Running {
        return -3;
    }
    let idx = CYCLE_COUNTER_IDX.load(Ordering::Relaxed) as usize;
    let (err, _) = sbi_call(FID_COUNTER_STOP, [idx, 1, 0, 0, 0]);
    err
}

/// Read counter CSR `csr` by number, or `None` outside `cycle`, `instret` and `hpmcounter3..31`.
///
/// The CSR is an instruction immediate, so this must dispatch. `time` (`0xc01`) is left out on
/// purpose: it is the fixed tick, never a cycle counter.
fn read_csr(csr: u16) -> Option<u64> {
    macro_rules! read {
        ($name:literal) => {{
            let value: u64;
            // SAFETY: reading an unprivileged counter CSR touches no memory and changes no state,
            // which the options state. S-mode reads of these are gated by `mcounteren` in firmware;
            // this path is only reached after SBI PMU handed us the CSR number for a counter it
            // started, which is firmware saying it is readable.
            unsafe {
                asm!(
                    concat!("csrr {}, ", $name),
                    out(reg) value,
                    options(nomem, nostack, preserves_flags),
                );
            }
            value
        }};
    }

    /// Expand one arm per `hpmcounterN`, so the 29 regular cases are written once.
    macro_rules! hpm {
        ($csr:expr, $($num:literal => $name:literal,)*) => {
            match $csr {
                $($num => Some(read!($name)),)*
                _ => None,
            }
        };
    }

    hpm! { csr,
        0xc00 => "cycle",
        0xc02 => "instret",
        0xc03 => "hpmcounter3",
        0xc04 => "hpmcounter4",
        0xc05 => "hpmcounter5",
        0xc06 => "hpmcounter6",
        0xc07 => "hpmcounter7",
        0xc08 => "hpmcounter8",
        0xc09 => "hpmcounter9",
        0xc0a => "hpmcounter10",
        0xc0b => "hpmcounter11",
        0xc0c => "hpmcounter12",
        0xc0d => "hpmcounter13",
        0xc0e => "hpmcounter14",
        0xc0f => "hpmcounter15",
        0xc10 => "hpmcounter16",
        0xc11 => "hpmcounter17",
        0xc12 => "hpmcounter18",
        0xc13 => "hpmcounter19",
        0xc14 => "hpmcounter20",
        0xc15 => "hpmcounter21",
        0xc16 => "hpmcounter22",
        0xc17 => "hpmcounter23",
        0xc18 => "hpmcounter24",
        0xc19 => "hpmcounter25",
        0xc1a => "hpmcounter26",
        0xc1b => "hpmcounter27",
        0xc1c => "hpmcounter28",
        0xc1d => "hpmcounter29",
        0xc1e => "hpmcounter30",
        0xc1f => "hpmcounter31",
    }
}

/// One SBI PMU call to function `fid` with `args`, returning `(error, value)`. Every PMU call can
/// fail, so unlike `isa::sbi_call` this returns the error.
fn sbi_call(fid: usize, args: [usize; 5]) -> (isize, usize) {
    let [a0, a1, a2, a3, a4] = args;
    // SAFETY: the PMU extension configures, starts and stops counters; it changes what the hardware
    // counts and touches no memory of ours.
    let ret = unsafe { super::sbi::call(EID_PMU, fid, [a0, a1, a2, a3, a4, 0]) };
    (ret.error, ret.value)
}

/// The boot line, printed beside the ISA summary.
pub(crate) fn print_summary() {
    match outcome() {
        CycleCounter::Running => crate::println!(
            "  cycles      : SBI PMU counter {}, CSR {:#05x}, {} bits",
            CYCLE_COUNTER_IDX.load(Ordering::Relaxed),
            CYCLE_CSR.load(Ordering::Relaxed),
            cycle_counter_width().unwrap_or(0),
        ),
        CycleCounter::NoPmuExtension => {
            crate::println!("  cycles      : no SBI PMU extension; ticks only");
        }
        CycleCounter::NoMatchingCounter => {
            crate::println!("  cycles      : SBI PMU present, no counter can count CPU cycles");
        }
        CycleCounter::FirmwareCounter => crate::println!(
            "  cycles      : SBI PMU offered a firmware counter; refused (an ecall per read)"
        ),
        CycleCounter::UnreadableCsr => {
            crate::println!("  cycles      : SBI PMU named a CSR outside the counter block");
        }
        CycleCounter::Stuck => crate::println!(
            "  cycles      : SBI PMU counter {} (CSR {:#05x}) did not advance; refused",
            CYCLE_COUNTER_IDX.load(Ordering::Relaxed),
            CYCLE_CSR.load(Ordering::Relaxed),
        ),
        CycleCounter::Unknown => crate::println!("  cycles      : arch::pmu::init has not run"),
    }
}

#[cfg(test)]
mod tests {
    //! The SBI PMU plumbing, which needs firmware; the decoders are host-tested in
    //! `crates/machine_discovery/tests/riscv64_isa_strings.rs`.
    //!
    //! None of these is a claim about cycles: QEMU-TCG's `cycle` counts instructions.

    use super::*;

    /// The outcome, the counter and the width agree, and the outcome is decided before the suite.
    #[test_case]
    fn the_outcome_and_the_counter_agree() {
        let outcome = outcome();
        assert_ne!(
            outcome,
            CycleCounter::Unknown,
            "kernel_main calls arch::pmu::init before the suite"
        );

        let running = outcome == CycleCounter::Running;
        assert_eq!(cycles().is_some(), running);
        assert_eq!(cycle_counter_width().is_some(), running);

        if outcome == CycleCounter::NoPmuExtension {
            assert!(
                !super::super::isa::get().sbi.extensions.contains(SBI_PMU),
                "the outcome says no PMU extension and the probe says there is one"
            );
        }

        if let Some(bits) = cycle_counter_width() {
            assert!(
                (1..=64).contains(&bits),
                "width {bits} is outside what a 6-bit field plus one can encode"
            );
        }
    }

    /// QEMU's OpenSBI implements SBI PMU, and the outcome is one of the two QEMU gives: `Running`
    /// on a plausible CSR, or `Stuck` on `rva23s64`.
    ///
    /// The counter is firmware's choice, so no CSR number is pinned. See
    /// notes/riscv-cycle-counters.md.
    #[test_case]
    fn qemu_reaches_one_of_the_two_answers_it_has_ever_given() {
        assert!(
            super::super::isa::get().sbi.extensions.contains(SBI_PMU),
            "OpenSBI has implemented SBI PMU since 0.8"
        );

        match outcome() {
            CycleCounter::Running => {
                let csr = CYCLE_CSR.load(Ordering::Relaxed);
                assert!(
                    csr == 0xc00 || (0xc03..=0xc1f).contains(&csr),
                    "CSR {csr:#05x} is neither `cycle` nor an `hpmcounter`; `time` (0xc01) in \
                     particular would be the fixed tick wearing a cycle counter's name"
                );
                assert_eq!(cycle_counter_width(), Some(64));
            }
            // `rva23s64`: `hpmcounter3`, modelled by TCG as a constant zero.
            CycleCounter::Stuck => {}
            other => panic!(
                "SBI PMU is present and the outcome is {other:?}; on QEMU that is a regression"
            ),
        }
    }

    /// The counter still moves forward, which catches one firmware stops after [`init`] checked.
    ///
    /// The window is a `time` spin, as in the bench probe. Under TCG the delta counts instructions.
    #[test_case]
    fn the_counter_advances() {
        let Some(first) = cycles() else {
            return;
        };

        let start = super::super::timer::now();
        while super::super::timer::now() - start < 1000 {
            core::hint::spin_loop();
        }

        let second = cycles().expect("the counter did not vanish mid-test");
        assert!(
            second.wrapping_sub(first) > 0,
            "counter read {first} twice across a timed spin; it is not counting"
        );
    }

    /// Firmware accepts the stop with 0 or -8, and re-running [`init`] leaves a decided outcome.
    ///
    /// It does not assert the counter comes back: on `rva23s64` it goes `Running` to `Stuck`
    /// (notes/riscv-cycle-counters.md, BUGS).
    #[test_case]
    fn stopping_the_counter_is_accepted() {
        if outcome() != CycleCounter::Running {
            return;
        }
        let err = stop();
        assert!(
            err == 0 || err == -8,
            "sbi_pmu_counter_stop returned {err}, which is neither success nor already-stopped"
        );

        init();
        assert_ne!(
            outcome(),
            CycleCounter::Unknown,
            "init left the module undecided, which it cannot do: every path stores an outcome"
        );
    }
}
