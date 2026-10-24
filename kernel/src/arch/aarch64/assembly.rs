//! **The assembly this architecture links**: the Image header, the boot entry, the exception
//! vectors, the context switch and the vector-register save area.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

// The arm64 Image header. `_start` lands at byte 0 of the image, which is where QEMU
// begins executing. It does nothing but branch to `_boot`.
global_asm!(include_str!("image_header.s"));

// The real entry point.
global_asm!(include_str!("boot.s"));

// The exception vector table. VBAR_EL1 will point here once `init` runs.
global_asm!(include_str!("vectors.s"));

// The context switch, and where a new thread begins.
global_asm!(include_str!("context.s"));

// Saving and restoring `q0`-`q31`. Separate from context.s because it moves a register file, not a
// calling convention's callee-saved set; see fp.rs.
global_asm!(include_str!("fp.s"));
