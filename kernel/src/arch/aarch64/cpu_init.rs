//! **Bringing this CPU into a state where the kernel can safely run.**
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Bring the CPU into a state where the kernel can safely run.
///
/// Installs the exception vectors, so a fault produces a report. The console must already be up,
/// because the fault handler's job is to print.
pub fn init() {
    exceptions::init();
}
