//! **A thread's floating-point and SIMD registers, aarch64**, for the portable [`crate::fp`]. With
//! no dirty bit, [`FpState::live`] records whether a thread has ever used the unit. See
//! notes/aarch64-port.md, "Floating point and SIMD".
//!
//! # BUGS
//!
//! - **`live` never clears**: a thread that touches FP once pays the save on every switch after.
//! - **SVE and SME are not saved.** [`init`] closes them, and `crate::fp` treats their trap as a
//!   fault; the emulator's default CPU cannot show it.

use super::instructions;

/// **How many vector registers this architecture saves**: `q0`-`q31`.
///
/// Read by the tests in [`crate::fp`], which name the register that failed rather than reporting
/// that one did, and which therefore cannot hard-code a count that is 32 here, 32 on RISC-V and 16
/// on `x86_64`.
#[cfg(any(test, feature = "system_tests"))]
pub const REGISTERS: usize = 32;

/// **The whole of a thread's FP/SIMD register file**, plus the flag that says whether any of it is
/// worth moving.
///
/// `#[repr(C, align(16))]` and the padding are a contract with `fp.s`, which reaches the vector
/// area at a fixed offset of 16 and needs it 16-byte aligned for `stp q`. Reorder a field here and
/// the assembly writes the flag with `q0`.
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct FpState {
    /// Nonzero once this thread has executed an FP or SIMD instruction. **The save and the restore
    /// are both conditioned on it**, which is what makes a kernel full of soft-float threads pay
    /// two loads and two branches per switch instead of a kilobyte of memory traffic.
    live: u64,
    /// Padding, so the vector area below lands at offset 16 and 16-byte aligned. Never read.
    _pad: u64,
    /// `q0`-`q31`. 512 bytes, and the reason this file exists.
    q: [u128; 32],
    /// `FPCR`: rounding mode, exception masks, flush-to-zero. Architectural state, so it moves.
    fpcr: u64,
    /// `FPSR`: the cumulative exception flags a program reads to find out what its arithmetic did.
    fpsr: u64,
}

const _: () = assert!(size_of::<FpState>() == 544);
const _: () = assert!(core::mem::offset_of!(FpState, q) == 16);
const _: () = assert!(core::mem::offset_of!(FpState, fpcr) == 528);

impl FpState {
    /// **The state a thread starts with, and the state the registers are scrubbed to.**
    ///
    /// Zeros throughout, which for `FPCR` means round-to-nearest with every exception trap disabled
    /// and flush-to-zero off: the architectural reset configuration, and what a program compiled by
    /// anything sane expects to find. It doubles as the scrub value, so
    /// [`crate::fp::hand_over`] has exactly one thing to install when it takes the registers away
    /// from a thread rather than a second code path that zeroes them.
    pub(crate) const INITIAL: Self = FpState {
        live: 0,
        _pad: 0,
        q: [0; 32],
        fpcr: 0,
        fpsr: 0,
    };

    /// A fresh thread's state: no registers worth saving, and FP trapped until it asks.
    pub const fn new() -> Self {
        Self::INITIAL
    }

    /// Has this thread ever executed an FP instruction?
    pub(crate) fn is_live(&self) -> bool {
        self.live != 0
    }

    /// Record that it has. Called once, from the trap the disabled `FPEN` produced.
    pub(crate) fn set_live(&mut self) {
        self.live = 1;
    }
}

impl Default for FpState {
    fn default() -> Self {
        Self::new()
    }
}

/// `CPACR_EL1.FPEN`, bits 21:20. `0b11` here means "do not trap"; `0b00` means "trap at both EL0
/// and EL1".
const FPEN: u64 = 0b11 << 20;

/// `CPACR_EL1.ZEN` (bits 17:16) and `CPACR_EL1.SMEN` (bits 25:24): the SVE and SME enables, which
/// [`init`] closes with `FPEN` because this module saves neither. RES0 on a part without the
/// extension, so clearing them is a no-op there.
const ZEN_AND_SMEN: u64 = (0b11 << 16) | (0b11 << 24);

/// **Put this core into the state the rest of this module assumes**: FP and SIMD trapped, for EL1
/// as well as EL0.
///
/// Called per core from `sched::init` and `sched::adopt_secondary_idle`: `CPACR_EL1` is banked and
/// resets to an UNKNOWN value. QEMU resets it to the value this writes, so no test sees a part that
/// reset it open; see notes/aarch64-port.md, "Floating point and SIMD".
pub fn init() {
    instructions::write_cpacr_synchronized(instructions::read_cpacr() & !(FPEN | ZEN_AND_SMEN));
}

/// Let the current core execute FP and SIMD instructions.
///
/// The `isb` is not decoration. `CPACR_EL1` is a context-synchronizing control register: without
/// it, an FP instruction already in the pipeline may be resolved against the old value, and the
/// caller here is about to execute thirty-two `ldp q` in [`restore`].
pub fn enable() {
    instructions::write_cpacr_synchronized(instructions::read_cpacr() | FPEN);
}

/// Is the FP unit open on this core right now?
///
/// Read by `crate::fp::enable_for_current`, which asks whether the enable it just wrote took (it
/// does not on a RISC-V hart with no FP unit, and the question is asked uniformly), and by the test
/// helpers in [`crate::fp`], which borrow the register file and have to put the core back the way
/// they found it. **Nothing on the switch path asks**: there the answer is `live`, which is a fact
/// about a thread rather than about a core, and reading `CPACR_EL1` to learn it would be a
/// system-register read per switch for something already in memory.
pub(crate) fn is_enabled() -> bool {
    instructions::read_cpacr() & FPEN != 0
}

/// Trap FP and SIMD again, at both exception levels.
///
/// **The caller owes the scrub, not this function.** Disabling the unit does not clear the
/// registers, and a thread that cannot read them today may be able to read them tomorrow through a
/// speculation window nobody has thought of yet; that is the whole of CVE-2018-3665. The invariant
/// [`crate::fp`] keeps is that the file holds the running thread's data or the initial state, and
/// it installs the latter before calling this.
pub(crate) fn disable() {
    instructions::write_cpacr_synchronized(instructions::read_cpacr() & !FPEN);
}

/// Copy the live register file into `state`.
///
/// # Safety
/// `state` must be a valid, writable, 16-byte-aligned `FpState`, and FP must be enabled on this
/// core. Both are the caller's, and the only caller is [`crate::fp::hand_over`].
pub(crate) unsafe fn save(state: *mut FpState) {
    // SAFETY: forwarded. `fp_save` writes 544 bytes at `state` and touches nothing else.
    unsafe { fp_save(state) }
}

/// Load `state` into the live register file.
///
/// # Safety
/// `state` must be a valid, readable, 16-byte-aligned `FpState`, and FP must be enabled on this
/// core. Both are the caller's.
pub(crate) unsafe fn restore(state: *const FpState) {
    // SAFETY: forwarded. `fp_restore` reads 544 bytes at `state` and writes only registers.
    unsafe { fp_restore(state) }
}

unsafe extern "C" {
    /// `q0`-`q31`, `FPCR`, `FPSR` out to memory. See fp.s.
    fn fp_save(state: *mut FpState);
    /// The mirror. See fp.s.
    fn fp_restore(state: *const FpState);
}

/// **One harmless FP instruction, to take the first-use trap on purpose.**
///
/// `fmov d0, xzr` writes zero into a register [`crate::fp::load_pattern`] is about to overwrite
/// anyway, so it has no effect beyond the trap it provokes. Tests only: the shipping kernel is
/// `softfloat` and asks for the unit nowhere.
#[cfg(any(test, feature = "system_tests"))]
pub fn touch() {
    // SAFETY: writes one vector register. Under a trapping `CPACR_EL1.FPEN` this takes the enable
    // trap and is re-executed after it, which is the entire point of calling it.
    unsafe {
        core::arch::asm!(
            ".arch_extension fp",
            "fmov d0, xzr",
            ".arch_extension nofp",
            options(nomem, nostack, preserves_flags),
        );
    }
}

#[cfg(any(test, feature = "system_tests"))]
impl FpState {
    /// Write [`crate::fp::register_pattern`] across all 32 lanes, each one's complement in the high
    /// half so the full 128 bits are exercised rather than only the `d` half a scalar `double`
    /// would occupy.
    pub fn set_pattern(&mut self, seed: u64) {
        for (index, lane) in self.q.iter_mut().enumerate() {
            let low = crate::fp::register_pattern(seed, index);
            *lane = (u128::from(!low) << 64) | u128::from(low);
        }
    }

    /// Is [`Self::set_pattern`]'s pattern still there, every lane of it?
    pub fn has_pattern(&self, seed: u64) -> bool {
        self.q.iter().enumerate().all(|(index, &lane)| {
            let low = crate::fp::register_pattern(seed, index);
            lane == (u128::from(!low) << 64) | u128::from(low)
        })
    }

    /// The low 64 bits of one lane, so a test can name the register that went wrong rather than
    /// reporting that something did.
    pub fn lane_low(&self, index: usize) -> u64 {
        self.q[index] as u64
    }

    /// Do the registers hold [`Self::INITIAL`]'s values, i.e. has the file been scrubbed?
    pub fn is_scrubbed(&self) -> bool {
        self.q.iter().all(|&lane| lane == 0) && self.fpcr == 0 && self.fpsr == 0
    }
}

/// Proofs of the `CPACR_EL1` edits, through stubs of the [`instructions`] functions.
///
/// The stub models `CPACR_EL1` as a plain 64-bit register with no RES0 fields and no pipeline, so
/// these proofs say only that the code never writes a field it did not mean to, not which fields
/// exist. See notes/kernel-proofs/stubbing-an-instruction.md.
#[cfg(kani)]
mod proofs {
    use core::sync::atomic::AtomicU64;
    use core::sync::atomic::Ordering::Relaxed;

    use super::*;

    /// The modelled `CPACR_EL1`.
    static CPACR: AtomicU64 = AtomicU64::new(0);

    #[allow(dead_code)] // named only by `#[kani::stub]`, which rustc cannot see
    fn read_cpacr_model() -> u64 {
        CPACR.load(Relaxed)
    }

    #[allow(dead_code)] // named only by `#[kani::stub]`
    fn write_cpacr_model(cpacr: u64) {
        CPACR.store(cpacr, Relaxed);
    }

    /// **Every `CPACR_EL1` edit changes exactly the field it names, and [`is_enabled`] reads back
    /// what the last one did.**
    ///
    /// [`init`] closes FP, SVE and SME; [`enable`] and [`disable`] touch `FPEN` alone.
    ///
    /// Falsification: attested 2026-09-25. On patagonia (aarch64 host), three times and one edit at
    /// a time: `init`'s mask without `ZEN_AND_SMEN`, `enable` writing `FPEN >> 1` (half the field),
    /// and `disable` clearing `FPEN | ZEN_AND_SMEN`. Each turns this red. `attested` rather than
    /// `replayable` for the reason `script/falsifications` gives for every architecture-specific
    /// harness.
    #[kani::proof]
    #[kani::stub(super::super::instructions::read_cpacr, read_cpacr_model)]
    #[kani::stub(
        super::super::instructions::write_cpacr_synchronized,
        write_cpacr_model
    )]
    fn every_cpacr_edit_changes_exactly_the_field_it_names() {
        let before: u64 = kani::any();
        CPACR.store(before, Relaxed);

        let which: u8 = kani::any();
        kani::assume(which < 3);
        let (touched, want) = match which {
            0 => {
                init();
                (FPEN | ZEN_AND_SMEN, 0)
            }
            1 => {
                enable();
                (FPEN, FPEN)
            }
            _ => {
                disable();
                (FPEN, 0)
            }
        };

        let after = CPACR.load(Relaxed);
        assert!(
            after & touched == want,
            "the named field did not reach its value"
        );
        assert!(
            after & !touched == before & !touched,
            "an edit moved a field it does not name"
        );
        assert!(
            is_enabled() == (which == 1),
            "is_enabled disagrees with the last edit"
        );
    }
}
