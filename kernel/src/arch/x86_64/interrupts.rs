//! **Interrupt masking, `x86_64`**: `RFLAGS.IF` (bit 9), set by `sti`, cleared by `cli`, and read
//! through `pushfq`, since RFLAGS has no `mov` form. The arch contract aarch64 states with
//! `PSTATE.DAIF` and RISC-V with `sstatus.SIE`.
//!
//! `sti` takes effect only after the next instruction retires, so `sti; hlt` in one `asm!` cannot
//! lose an interrupt between the two. Calling [`enable`] and then halting can: two calls leave room
//! for something between them, and the something is a hang.

use core::arch::asm;

/// `RFLAGS.IF`, bit 9: the interrupt-enable flag.
const IF: u64 = 1 << 9;

/// Read RFLAGS. There is no instruction that moves it to a register, so it goes via the stack;
/// `options(nomem)` is therefore wrong here even though nothing durable is written.
fn flags() -> u64 {
    let rflags: u64;
    // SAFETY: pushes and pops one word of this thread's own stack, leaving it as found.
    unsafe { asm!("pushfq", "pop {}", out(reg) rflags, options(preserves_flags)) };
    rflags
}

/// Are interrupts currently enabled?
pub(crate) fn is_enabled() -> bool {
    flags() & IF != 0
}

/// Mask interrupts, returning whether they were enabled before (for [`restore`]).
///
/// **Not atomic in the way the RISC-V twin is, and it does not need to be.** RISC-V clears its bit
/// and reports the old value in one `csrrci`; here the read and the clear are two instructions. An
/// interrupt landing between them is harmless: it is delivered while interrupts are still enabled,
/// which is what the caller's own prior state already permitted, and the value read is still the
/// state at the moment the caller asked. What would be a bug is the opposite order.
pub fn disable() -> bool {
    let was = is_enabled();
    // SAFETY: clears RFLAGS.IF. No memory effect.
    unsafe { asm!("cli", options(nomem, nostack)) };
    was
}

/// Restore the interrupt-enable state [`disable`] reported. Nesting composes because each `disable`
/// returns the exact bit its `restore` puts back.
pub fn restore(was_enabled: bool) {
    if was_enabled {
        enable();
    } else {
        // SAFETY: clears RFLAGS.IF. No memory effect.
        unsafe { asm!("cli", options(nomem, nostack)) };
    }
}

/// Unmask interrupts. Takes effect after the following instruction retires; see the module header.
pub fn enable() {
    // SAFETY: sets RFLAGS.IF. No memory effect.
    unsafe { asm!("sti", options(nomem, nostack)) };
}
