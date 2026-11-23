// Saving and restoring the FP/SIMD register file, aarch64. The assembly half of fp.rs.
//
// # Why this is not in context.s
//
// `switch_to` saves the AAPCS64 callee-saved set, which is what a *function call* may destroy.
// This is a different quantity: the whole of `q0`-`q31` plus `FPCR` and `FPSR`, because the
// register file crosses a thread boundary rather than a call boundary. The kernel is built
// `softfloat` and executes no FP instruction of its own, so every bit in these registers belongs
// to whichever thread last ran, and all of it has to move.
//
// 512 bytes of register state, 32 store-pair instructions each way. That cost is why `fp.rs`
// arranges for these two routines to be called only for a thread that has taken the enable trap,
// and why they are out of line rather than folded into the switch.
//
// # The `.arch_extension` directives
//
// The kernel's target is `aarch64-unknown-none-softfloat`, whose feature string carries `-neon`,
// so the integrated assembler rejects `q` registers and `FPCR` by default. That is the whole point
// of the target: it stops the *compiler* emitting FP, which is what made a kernel with no save
// path safe. It is not meant to stop hand-written code that saves the file on purpose.
// `.arch_extension fp` and `simd` re-enable the two for the assembler, and the matching `no...`
// directives at the bottom put it back, so nothing after this file inherits the widened set.

.section ".text", "ax"

.arch_extension fp
.arch_extension simd

// void fp_save(FpState *state)
//
// x0 = the state block. `q0`-`q31` go at offset 16 (see `FpState` in fp.rs: the first sixteen
// bytes are the `live` flag and its padding, which keeps the vector area 16-byte aligned without
// the caller having to think about it). `FPCR` and `FPSR` follow at 528.
// CFI: see nifeos/record:notes/cfi-unwind.md. Both routines below are ordinary leaf functions reached by `bl`
// and returning by `ret`; sp never moves, so the default frame (CFA = sp, return address in lr)
// is already correct and no directive needs restating it.
.global fp_save
.type fp_save, @function
fp_save:
    .cfi_startproc
    add     x1, x0, #16
    stp     q0,  q1,  [x1, #0]
    stp     q2,  q3,  [x1, #32]
    stp     q4,  q5,  [x1, #64]
    stp     q6,  q7,  [x1, #96]
    stp     q8,  q9,  [x1, #128]
    stp     q10, q11, [x1, #160]
    stp     q12, q13, [x1, #192]
    stp     q14, q15, [x1, #224]
    stp     q16, q17, [x1, #256]
    stp     q18, q19, [x1, #288]
    stp     q20, q21, [x1, #320]
    stp     q22, q23, [x1, #352]
    stp     q24, q25, [x1, #384]
    stp     q26, q27, [x1, #416]
    stp     q28, q29, [x1, #448]
    stp     q30, q31, [x1, #480]
    // `str`, not `stp`: a store-pair's scaled immediate stops at 504 and these live at 528.
    mrs     x2, fpcr
    mrs     x3, fpsr
    str     x2, [x0, #528]
    str     x3, [x0, #536]
    ret
    .cfi_endproc
.size fp_save, . - fp_save

// void fp_restore(const FpState *state)
//
// The exact mirror. Called with FP already enabled for this core (`CPACR_EL1.FPEN` = 0b11), which
// `fp.rs` guarantees: a `ldp q0, q1` under a trapping `FPEN` would take the very trap this whole
// mechanism exists to serve, from inside the scheduler, on a path that cannot afford it.
.global fp_restore
.type fp_restore, @function
fp_restore:
    .cfi_startproc
    add     x1, x0, #16
    ldp     q0,  q1,  [x1, #0]
    ldp     q2,  q3,  [x1, #32]
    ldp     q4,  q5,  [x1, #64]
    ldp     q6,  q7,  [x1, #96]
    ldp     q8,  q9,  [x1, #128]
    ldp     q10, q11, [x1, #160]
    ldp     q12, q13, [x1, #192]
    ldp     q14, q15, [x1, #224]
    ldp     q16, q17, [x1, #256]
    ldp     q18, q19, [x1, #288]
    ldp     q20, q21, [x1, #320]
    ldp     q22, q23, [x1, #352]
    ldp     q24, q25, [x1, #384]
    ldp     q26, q27, [x1, #416]
    ldp     q28, q29, [x1, #448]
    ldp     q30, q31, [x1, #480]
    ldr     x2, [x0, #528]
    ldr     x3, [x0, #536]
    msr     fpcr, x2
    msr     fpsr, x3
    ret
    .cfi_endproc
.size fp_restore, . - fp_restore

.arch_extension nosimd
.arch_extension nofp
