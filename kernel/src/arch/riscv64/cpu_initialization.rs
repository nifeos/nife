//! **Bringing this hart's architecture state up.**
//!
//! Name: provisional. Spelled out under the `init` ruling of 2026-10-26 (UTC), in
//! design/naming/spelled-out-rulings.md; a module is a thing, so it takes the noun
//! `cpu_initialization` rather than the verb.

use super::*;

/// Bring this hart's architecture state up. On RISC-V that is the trap vector (`stvec`): unlike
/// aarch64's `VBAR_EL1` this is the only per-hart install a secondary needs here, since the timer
/// and interrupt unmasking are separate steps in `secondary_main`. The boot hart reaches it as the
/// first step of the shared `bring_up_machine` (milestone 890 (riscv64 and `x86_64` join the shared
/// boot path)), and a secondary takes the same path.
pub fn initialize() {
    exceptions::initialize();
}
