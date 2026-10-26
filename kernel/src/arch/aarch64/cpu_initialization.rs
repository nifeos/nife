//! **Bringing this CPU into a state where the kernel can safely run.**
//!
//! Name: provisional. Spelled out under the `init` ruling of 2026-10-26 (UTC), in
//! design/naming/spelled-out-rulings.md; a module is a thing, so it takes the noun
//! `cpu_initialization` rather than the verb.

use super::*;

/// Bring the CPU into a state where the kernel can safely run.
///
/// Installs the exception vectors, so a fault produces a report. The console must already be up,
/// because the fault handler's job is to print.
///
/// Name: provisional. Spelled out under the `init` ruling of 2026-10-26 (UTC), in
/// design/naming/spelled-out-rulings.md.
pub fn initialize() {
    exceptions::initialize();
}
