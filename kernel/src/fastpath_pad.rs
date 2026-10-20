//! E3's footprint-perturbation experiment (milestone 134,
//! design/roadmap/0134-the-measurements-that-decide.md): pad the IPC fastpath's call graph with a
//! large, reachable-but-never-taken function, so the fastpath's *measured* footprint
//! (`script/fastpath-footprint`) grows while the *executed* work on a benchmarked round trip stays
//! what it was. Liedtke's claim is that footprint costs cache; padding tests that without a cache
//! counter. See notes/footprint-perturbation.md.
//!
//! Gated on the `fastpath_pad` Cargo feature: in an ordinary build this module, the call into it
//! and the per-arch nop sleds do not exist. With the feature on, `crate::sched::ipc_send` calls
//! [`maybe_pad`] unconditionally, and whether that reaches the padding is decided at runtime by
//! [`core::hint::black_box`]: the padding is *resident* (in `.text`, walked by the footprint tool)
//! and *dead* (never executed). The guard is a real compare-and-branch on every `ipc_send`, a
//! confound of about a nanosecond, named in the roadmap doc; an always-false the compiler could see
//! is exactly what LLVM may delete.
//!

//! # The layout control (milestone 370 (a layout control))
//!
//! A sled that is never executed can still move other code: which L1i sets each hot line lands in,
//! where 64-byte line boundaries fall, and whether a branch target is 8-byte aligned (which the
//! U74's BTB needs for a zero-bubble prediction). So one padded kernel against one un-padded kernel
//! reports footprint and layout summed. Two numbers, read at build time by kernel/build.rs:
//!
//! - `NIFE_FASTPATH_PAD=<units>`, [`PAD_UNITS`]: the sled's length in units (5,092 bytes on
//!   riscv64, 5,796 on aarch64). Unset is 1. 0 is the dose-response's zero: the guard and a `ret`.
//! - `NIFE_FASTPATH_SHIFT=<bytes>`, [`SHIFT_BYTES`]: zero bytes appended after the sled's `ret`,
//!   under their own unreferenced symbol (`fastpath_layout_shift`). Zeros because both ISAs decode
//!   zero as an illegal instruction, so a block ever executed would trap.
//!
//! **The linker script places this module's section right after `.text.hot`** (kernel/link-riscv64.ld,
//! kernel/link-aarch64.ld), so a sled of N bytes and a block of N bytes both move all kernel text
//! after the pinned hot block by N and nothing else. Neither moves the hot block (milestone 796),
//! which is why E3 as specified cannot run on this tree
//! (notes/footprint-perturbation/step-0-under-the-pin.md). A pad and a shift of the same
//! displacement produce identical binaries apart from the sled's own bytes, so the two ladders
//! differ only in whether the bytes are inside the counted footprint. An ordinary build has no such
//! section, so its link order is unchanged. `script/fastpath-footprint --layout` proves every image
//! executes the same instructions. Names are provisional.

/// The sled's length, in units of `UNIT_NOPS` (each arch's `fastpath_pad.rs`).
pub(crate) const PAD_UNITS: usize = parse(env!("NIFE_FASTPATH_PAD"));

/// Unreferenced zero bytes after the sled; non-zero only in a layout variant.
pub(crate) const SHIFT_BYTES: usize = parse(env!("NIFE_FASTPATH_SHIFT"));

/// The two numbers as data, for the bench boot to print (`bench-probe: fastpath_pad ...`), so a
/// card can say which E3 image it is. Data rather than immediates on purpose: on riscv64 `li a0, 0`
/// is two bytes and `li a0, 3280` is eight, so printing constants would change the length of the
/// printing function and move everything linked after it, which is the variable being controlled.
#[cfg(feature = "bench")]
pub(crate) static BUILD: [usize; 2] = [PAD_UNITS, SHIFT_BYTES];

/// Decimal, checked by kernel/build.rs before it gets here; a const fn because `str::parse` is not.
const fn parse(s: &str) -> usize {
    let b = s.as_bytes();
    let mut i = 0;
    let mut n = 0;
    while i < b.len() {
        n = n * 10 + (b[i] - b'0') as usize;
        i += 1;
    }
    n
}

/// Call from a root the fastpath-footprint closure walks (today: [`crate::sched::ipc_send`]).
/// Branches into the arch's nop sled only if [`core::hint::black_box`] fails to prove its input
/// false, which it never does, so this costs one untaken branch and nothing else at runtime.
#[inline(never)]
// In the pinned hot section, because every `ipc_send` in a padded build runs it: milestone 796
// (pin the hot trap path's placement).
// The sled it guards stays outside: never executed, it is what moves the rest of the text.
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.fastpath_pad.maybe_pad")
)]
pub(crate) fn maybe_pad() {
    if core::hint::black_box(false) {
        // SAFETY: the arch nop sled is a leaf function (no memory access, no stack frame beyond
        // its own `ret`) reached with the platform's ordinary call ABI; it touches nothing this
        // caller owns. It is also never actually called (see the module doc), so its body has no
        // runtime effect to be unsound about; this `unsafe` covers the FFI call shape only.
        unsafe { crate::arch::fastpath_pad_body() };
    }
}
