//! The riscv64 half of E3's padding (`kernel/src/fastpath_pad.rs`): a `nop` sled sized to roughly
//! double `ipc_fastpath`, for the static footprint measurement only.
//!
//! `.option norvc` keeps each `nop` 4 bytes, so 1,272 of them are 5,088 bytes, as on aarch64.
//! Re-measure with `script/fastpath-footprint --features fastpath_pad --arch riscv64` after
//! touching this or the fastpath. E3's latency half needs `--real`, which riscv64 lacks
//! (design/roadmap/0134-the-measurements-that-decide.md, BUGS).
use core::arch::global_asm;

/// One sled unit: `fastpath_pad` alone links exactly this many `nop`s, as it did before the
/// ladder (milestone 134's layout control) made the count a multiple. See
/// `crate::fastpath_pad::PAD_UNITS` and, for the zero block after the sled,
/// `crate::fastpath_pad::SHIFT_BYTES`.
const UNIT_NOPS: usize = 1272;

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
    .option push
    .option norvc
    .rept {nops}
    nop
    .endr
    ret
    .option pop
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
