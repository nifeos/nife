//! Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
//! question 2), with `user_mode_runtime::interrupt_wait` and `interrupt_acknowledge`. His words:
//! "Ratify those three." Refused `irq` (round 7's module name; a newcomer says interrupt, per §154
//! (the acronym test is whether the phrase is spoken)).

/// `invoke(capability, WAIT, _, _, _)` -> 1. **Blocks until the interrupt fires.** The kernel masks
/// the interrupt when it fires and hands it to us as a message; nothing device-specific
/// happens in the kernel.
pub const WAIT: u64 = 0;

/// `invoke(capability, ACK, _, _, _)` -> 0. Re-enable the interrupt at the GIC, once we have quieted
/// the device. Until we call this, the interrupt stays masked and cannot storm.
pub const ACK: u64 = 1;
