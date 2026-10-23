//! The aarch64 half of E3's padding (see `kernel/src/fastpath_pad.rs`): one function of `nop`s,
//! sized to roughly double `bench/fastpath-aarch64.txt`'s recorded `ipc_fastpath` (5,792 bytes).
//! 1,448 `nop`s at 4 bytes each is 5,792 bytes plus one `ret`. Re-measure with
//! `script/fastpath-footprint --features fastpath_pad --arch aarch64` after touching this file or
//! the fastpath, since the target is the current baseline, not a constant.
//!
//! `global_asm!` rather than a Rust loop: the point is static footprint, and a `.rept` block is
//! bytes on disk, which `script/fastpath-footprint` sums. A loop over `nop`s is a handful of
//! instructions whatever the count.
use core::arch::global_asm;

/// One sled unit: `fastpath_pad` alone links exactly this many `nop`s, and milestone 134's layout
/// ladder links a multiple of it. See
/// `crate::fastpath_pad::PAD_UNITS` and, for the zero block after the sled,
/// `crate::fastpath_pad::SHIFT_BYTES`.
const UNIT_NOPS: usize = 1448;

unsafe extern "C" {
    /// The nop sled defined below. Never actually called (see `crate::fastpath_pad::maybe_pad`);
    /// declared so the reachable-but-dead call exists in the compiled binary for
    /// `script/fastpath-footprint`'s static closure walk to find.
    pub fn fastpath_pad_body();
}

global_asm!(
    "
    .pushsection .text.fastpath_pad_body,\"ax\",%progbits
    .global fastpath_pad_body
    .type fastpath_pad_body, %function
fastpath_pad_body:
    .rept {nops}
    nop
    .endr
    ret
    .if {shift}
    .global fastpath_layout_shift
    .type fastpath_layout_shift, %object
fastpath_layout_shift:
    .zero {shift}
    .size fastpath_layout_shift, {shift}
    .endif
    .popsection
    ",
    nops = const UNIT_NOPS * crate::fastpath_pad::PAD_UNITS,
    shift = const crate::fastpath_pad::SHIFT_BYTES,
);
