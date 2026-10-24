//! E3's footprint-perturbation experiment: pad the IPC fastpath's call graph with a reachable but
//! never-taken nop sled, so its measured footprint grows while the executed work stays the same.
//!
//! Gated on the `fastpath_pad` feature. `crate::sched::ipc_send` calls [`maybe_pad`], whose
//! [`core::hint::black_box`] guard is a real branch LLVM cannot delete and never takes.
//! [`PAD_UNITS`] sizes the sled; [`SHIFT_BYTES`] appends zeros, which trap if executed, so a pad
//! and a shift differ only in whether the bytes count as footprint. Names are provisional. See
//! notes/footprint-perturbation/layout-control.md.

/// The sled's length, in units of `UNIT_NOPS` (each arch's `fastpath_pad.rs`).
pub(crate) const PAD_UNITS: usize = parse(env!("NIFE_FASTPATH_PAD"));

/// Unreferenced zero bytes after the sled; non-zero only in a layout variant.
pub(crate) const SHIFT_BYTES: usize = parse(env!("NIFE_FASTPATH_SHIFT"));

/// The two numbers as data, for the bench boot to print, so a card says which E3 image it is.
/// Immediates would differ in length (`li a0, 0` against `li a0, 3280`) and move the code after.
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
// In the pinned hot section, since every padded `ipc_send` runs it; the sled stays outside.
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
