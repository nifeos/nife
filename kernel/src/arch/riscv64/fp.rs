//! A thread's floating-point registers on RISC-V: the register file and the `sstatus.FS` writes
//! that decide who may touch it. The policy is [`crate::fp`]'s.
//!
//! This kernel uses `FS` as a plain enable, Off against everything else, though it has four states.
//! `sstatus` travels in the trap frame, so the first-use handler sets the frame's copy too. See
//! notes/riscv-port/traps.md.
//!
//! # BUGS
//!
//! F without D is not handled; a hart without D is untested (notes/riscv-port/traps.md, BUGS).

use super::instructions;

/// **How many FP registers this architecture saves**: `f0`-`f31`. See the aarch64 twin.
#[cfg(any(test, feature = "system_tests"))]
pub(crate) const REGISTERS: usize = 32;

/// **A thread's FP register file**, plus the flag that says whether any of it is worth moving.
///
/// `#[repr(C, align(16))]` is a contract with `fp.s`, which reaches `fcsr` at offset 8 and `f0` at
/// offset 16.
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub(crate) struct FpState {
    /// Nonzero once this thread has executed an FP instruction. See the aarch64 twin: the save and
    /// the restore are both conditioned on it.
    live: u64,
    /// `fcsr`: the rounding mode and the accrued exception flags, in one CSR. aarch64 splits the
    /// same information across `FPCR` and `FPSR`.
    fcsr: u64,
    /// `f0`-`f31`, 64 bits each under D. 256 bytes, and the reason this file exists.
    f: [u64; 32],
}

const _: () = assert!(size_of::<FpState>() == 272);
const _: () = assert!(core::mem::offset_of!(FpState, fcsr) == 8);
const _: () = assert!(core::mem::offset_of!(FpState, f) == 16);

impl FpState {
    /// The state a thread starts with and the registers are scrubbed to: zeros, and `fcsr`
    /// round-to-nearest-even with no accrued exceptions.
    pub(crate) const INITIAL: Self = FpState {
        live: 0,
        fcsr: 0,
        f: [0; 32],
    };

    /// A fresh thread's state.
    pub const fn new() -> Self {
        Self::INITIAL
    }

    /// Has this thread ever executed an FP instruction?
    pub(crate) fn is_live(&self) -> bool {
        self.live != 0
    }

    /// Record that it has. Called once, from the first-use trap.
    pub(crate) fn set_live(&mut self) {
        self.live = 1;
    }
}

impl Default for FpState {
    fn default() -> Self {
        Self::new()
    }
}

/// `sstatus.FS`, bits 14:13. Zero is Off; anything else lets FP instructions execute.
pub(super) const SSTATUS_FS: u64 = 0b11 << 13;
/// `FS = Initial`: the unit is open and the registers hold their initial values.
const SSTATUS_FS_INITIAL: u64 = 0b01 << 13;
/// `FS = Dirty`: what the hardware sets the instant an FP register is written, and therefore what
/// the first-use handler writes into the trap frame once it has loaded a register file.
pub(super) const SSTATUS_FS_DIRTY: u64 = 0b11 << 13;
/// `sstatus.VS`, bits 10:9, the vector unit's enable. Nothing saves vector state, so [`init`] turns
/// it Off; a hart without V hardwires it to zero.
const SSTATUS_VS: u64 = 0b11 << 9;
/// `XTheadVector`'s `VS`, bits 24:23 (Linux's `SR_VS_THEAD`), where the C906 and C910 keep vector
/// 0.7.1's enable. Cleared only on a T-Head hart; elsewhere these bits are reserved.
const SSTATUS_XTHEADVECTOR_VS: u64 = 0b11 << 23;

/// Put this hart into the state this module assumes: FP and both vector enables off.
///
/// Called per hart from `sched::init` and `sched::adopt_secondary_idle`, not `arch::init`, which
/// the RISC-V boot does not pass through; firmware may leave `FS` open. The call site says why.
pub fn init() {
    disable();
    instructions::clear_sstatus(SSTATUS_VS);
    if super::isa::is_thead() {
        instructions::clear_sstatus(SSTATUS_XTHEADVECTOR_VS);
    }
}

/// Let this hart execute FP instructions, by moving `FS` out of Off.
///
/// Setting bit 13 takes Off to Initial and leaves Clean or Dirty alone. A CSR write is ordered
/// before the instructions after it, so no fence is needed.
pub fn enable() {
    // A no-op without an FP unit; [`is_enabled`] tells the caller.
    instructions::set_sstatus(SSTATUS_FS_INITIAL);
}

/// Whether the FP unit is open on this hart now. False just after [`enable`] means the hart has no
/// FP unit, so the first-use trap becomes a fault.
pub(crate) fn is_enabled() -> bool {
    instructions::read_sstatus() & SSTATUS_FS != 0
}

/// Take the unit away again: `FS = Off`.
///
/// The caller owes the scrub, as on every architecture ([`crate::fp`]).
pub(crate) fn disable() {
    instructions::clear_sstatus(SSTATUS_FS);
}

/// Copy the live register file into `state`.
///
/// # Safety
/// `state` must be a valid, writable, 16-byte-aligned `FpState`, and `sstatus.FS` must not be Off.
/// The only caller is [`crate::fp::hand_over`].
pub(crate) unsafe fn save(state: *mut FpState) {
    // SAFETY: forwarded. `fp_save` writes 272 bytes at `state` and touches nothing else.
    unsafe { fp_save(state) }
}

/// Load `state` into the live register file.
///
/// # Safety
/// As [`save`], readable rather than writable.
pub(crate) unsafe fn restore(state: *const FpState) {
    // SAFETY: forwarded. `fp_restore` reads 272 bytes at `state` and writes only registers.
    unsafe { fp_restore(state) }
}

unsafe extern "C" {
    /// `f0`-`f31` and `fcsr` out to memory. See fp.s.
    fn fp_save(state: *mut FpState);
    /// The mirror. See fp.s.
    fn fp_restore(state: *const FpState);
}

/// One harmless FP instruction, `fmv.d.x f0, zero`, to take the first-use trap on purpose. Tests
/// only.
#[cfg(any(test, feature = "system_tests"))]
pub(crate) fn touch() {
    // SAFETY: writes one FP register. Under `FS == Off` this raises the illegal-instruction trap
    // that `exceptions.rs` turns into an enable, and is re-executed after it.
    unsafe {
        core::arch::asm!(
            ".option push",
            ".option arch, +d",
            "fmv.d.x f0, zero",
            ".option pop",
            options(nomem, nostack, preserves_flags),
        );
    }
}

#[cfg(any(test, feature = "system_tests"))]
impl FpState {
    /// Write [`crate::fp::register_pattern`] across all 32 registers.
    pub(crate) fn set_pattern(&mut self, seed: u64) {
        for (index, register) in self.f.iter_mut().enumerate() {
            *register = crate::fp::register_pattern(seed, index);
        }
    }

    /// Is that pattern still there, in every register?
    pub(crate) fn has_pattern(&self, seed: u64) -> bool {
        self.f
            .iter()
            .enumerate()
            .all(|(index, &value)| value == crate::fp::register_pattern(seed, index))
    }

    /// One register, so a test can name the one that went wrong.
    pub(crate) fn lane_low(&self, index: usize) -> u64 {
        self.f[index]
    }

    /// Do the registers hold [`Self::INITIAL`]'s values, i.e. has the file been scrubbed?
    pub(crate) fn is_scrubbed(&self) -> bool {
        self.f.iter().all(|&value| value == 0) && self.fcsr == 0
    }
}
