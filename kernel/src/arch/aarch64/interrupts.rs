//! Masking and unmasking interrupts: the `I` bit of `PSTATE.DAIF`, where a set bit means masked.
//!
//! `msr daifset, #2` and `msr daifclr, #2` touch only `I`, so there is no read-modify-write window.
//! FIQ and SError stay as the boot protocol left them. See notes/aarch64-port.md, "Masking
//! interrupts".

use super::instructions;

/// Bit 7 of DAIF, as read back by `mrs`.
const I_BIT: u64 = 1 << 7;

/// Are interrupts currently *unmasked*, i.e. can one fire right now?
pub(crate) fn is_enabled() -> bool {
    instructions::read_daif() & I_BIT == 0
}

/// Mask interrupts, and report whether they had been enabled.
///
/// The return value is the whole point. Callers must **restore** this state rather than
/// blindly enabling, or a lock taken inside an interrupt handler will re-enable interrupts
/// on release, inside a handler, which is a fine way to get a fault you cannot explain.
#[must_use = "the previous interrupt state must be restored, not discarded"]
pub fn disable() -> bool {
    let was_enabled = is_enabled();

    // An interrupt between the read and the mask runs and returns; the state read is still what to restore.
    instructions::mask_interrupts();

    was_enabled
}

/// Put interrupts back the way [`disable`] found them.
pub fn restore(was_enabled: bool) {
    if was_enabled {
        instructions::unmask_interrupts();
    }
}

/// Unconditionally unmask interrupts.
///
/// Only for the places that legitimately turn interrupts on for the first time (the boot, a
/// secondary coming online, a thread's trampoline) and for tests. Everywhere else wants [`disable`]
/// + [`restore`].
pub fn enable() {
    instructions::unmask_interrupts();
}
