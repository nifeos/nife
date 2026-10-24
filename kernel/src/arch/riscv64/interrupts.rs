//! S-mode interrupt masking on RISC-V, the analog of aarch64's `PSTATE.DAIF` handling.
//!
//! One bit, `sstatus.SIE`, gates every S-mode interrupt: set, the sources enabled in `sie` may
//! interrupt; clear, all are masked.

use super::instructions;

/// `sstatus.SIE`, bit 1. Small enough for the CSR immediate-form instructions.
const SIE: usize = 1 << 1;

/// Are S-mode interrupts currently enabled?
pub(crate) fn is_enabled() -> bool {
    instructions::read_sstatus() as usize & SIE != 0
}

/// Mask S-mode interrupts, returning whether they were enabled before (for [`restore`]). Atomic:
/// `csrrci` reads the old `sstatus` and clears `SIE` in one instruction, so no interrupt can land
/// between the read and the clear.
pub fn disable() -> bool {
    instructions::read_and_clear_sstatus_sie() as usize & SIE != 0
}

/// Restore the interrupt-enable state [`disable`] reported. The paired half of `disable`; nesting
/// composes because each `disable` returns the exact bit its `restore` puts back.
pub fn restore(was_enabled: bool) {
    if was_enabled {
        enable();
    } else {
        instructions::clear_sstatus_sie();
    }
}

/// Unmask S-mode interrupts.
pub fn enable() {
    instructions::set_sstatus_sie();
}
