# Saving and restoring the FP/SSE register file, x86_64. The assembly half of fp.rs.
#
# Intel syntax, like the rest of this architecture's assembly.
#
# Two instructions, where aarch64 needs sixteen store-pairs and RISC-V thirty-two stores: `fxsave`
# moves the whole legacy x87 stack, `MXCSR`, and `xmm0`-`xmm15` in one go. That is not this
# architecture being tidy. It is a 512-byte block whose layout was frozen in 1999 and which the CPU
# writes with a microcoded sequence, and the `xsave` family exists because it was not extensible;
# see the note in fp.rs on why this kernel uses the old one anyway.
#
# **`CR0.TS` must be clear when either of these runs.** `fxsave` and `fxrstor` are themselves
# affected by it, so a save issued while the unit is shut takes the very `#NM` the save exists to
# serve. `crate::fp::hand_over` is the only caller and guarantees it.

.section .text
.code64

# void fp_save(FpState *state)
#
# rdi = the state block; the 512-byte area is at offset 16, which is 16-byte aligned because
# `FpState` is (see fp.rs). `fxsave64` rather than `fxsave`: the 64-bit form records the full
# 64-bit instruction and data pointers instead of the 32-bit ones, which is the only difference and
# is the truthful one on this architecture.
# CFI: see nifeos/record:notes/cfi-unwind.md. Ordinary leaf functions, `call`ed and `ret`urning; rsp never
# moves, so the default frame (CFA = rsp + 8, return address at CFA - 8) is already correct.
.global fp_save
.type fp_save, @function
fp_save:
    .cfi_startproc
    fxsave64 [rdi + 16]
    ret
    .cfi_endproc
.size fp_save, . - fp_save

# void fp_restore(const FpState *state)
#
# The mirror. `fxrstor64` raises #GP if the `MXCSR` field has a bit set that this CPU's
# `MXCSR_MASK` does not allow, which is why `FpState::INITIAL` spells `MXCSR` as `0x1f80` (all
# exceptions masked, round to nearest) rather than leaving the block all zeros the way the other
# two architectures can.
.global fp_restore
.type fp_restore, @function
fp_restore:
    .cfi_startproc
    fxrstor64 [rdi + 16]
    ret
    .cfi_endproc
.size fp_restore, . - fp_restore
