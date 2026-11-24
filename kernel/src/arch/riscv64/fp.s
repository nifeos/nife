# Saving and restoring the FP register file, RISC-V. The assembly half of fp.rs.
#
# Structurally identical to arch/aarch64/fp.s; only the register set and the way the assembler is
# talked into accepting it differ. `f0`-`f31` are 64 bits each under the D extension, so this is
# 256 bytes against aarch64's 512, and `fcsr` carries both the rounding mode and the accrued
# exception flags in one CSR where aarch64 has two.
#
# # The `.option arch, +d`
#
# The kernel's target is `riscv64imac-unknown-none-elf`: integer, atomics, compressed, and
# deliberately no F and no D, so that the compiler cannot emit floating point into a kernel that
# had nowhere to save it. That is a statement about generated code, not about code written on
# purpose to move the register file, and `.option push` / `.option arch, +d` widens the assembler
# for exactly these two routines and `.option pop` puts it back.
#
# **The machine still has to have the extension.** `qemu-system-riscv64 -cpu rv64` and the JH7110's
# U74 cores both do. A hart that does not implement D leaves `sstatus.FS` hardwired to zero, which
# is the case `fp::is_enabled` reports back to `crate::fp::enable_for_current`; see fp.rs.

.section ".text", "ax"

# void fp_save(FpState *state)
#
# a0 = the state block. `f0`-`f31` at offset 16 (the `live` flag and `fcsr` are the first sixteen
# bytes; see `FpState` in fp.rs).
# CFI: see partsfoundry/record:notes/cfi-unwind.md. Ordinary leaf functions, `call`ed and `ret`urning; sp never moves,
# so the default frame (CFA = sp, return address in ra) is already correct.
.global fp_save
.type fp_save, @function
fp_save:
    .cfi_startproc
    .option push
    .option arch, +d
    fsd  f0,   16(a0)
    fsd  f1,   24(a0)
    fsd  f2,   32(a0)
    fsd  f3,   40(a0)
    fsd  f4,   48(a0)
    fsd  f5,   56(a0)
    fsd  f6,   64(a0)
    fsd  f7,   72(a0)
    fsd  f8,   80(a0)
    fsd  f9,   88(a0)
    fsd  f10,  96(a0)
    fsd  f11, 104(a0)
    fsd  f12, 112(a0)
    fsd  f13, 120(a0)
    fsd  f14, 128(a0)
    fsd  f15, 136(a0)
    fsd  f16, 144(a0)
    fsd  f17, 152(a0)
    fsd  f18, 160(a0)
    fsd  f19, 168(a0)
    fsd  f20, 176(a0)
    fsd  f21, 184(a0)
    fsd  f22, 192(a0)
    fsd  f23, 200(a0)
    fsd  f24, 208(a0)
    fsd  f25, 216(a0)
    fsd  f26, 224(a0)
    fsd  f27, 232(a0)
    fsd  f28, 240(a0)
    fsd  f29, 248(a0)
    fsd  f30, 256(a0)
    fsd  f31, 264(a0)
    csrr t0, fcsr
    sd   t0, 8(a0)
    .option pop
    ret
    .cfi_endproc
.size fp_save, . - fp_save

# void fp_restore(const FpState *state)
#
# The mirror. `fcsr` first, so that the loads below cannot be affected by a rounding mode belonging
# to the thread that just left; they are bit patterns rather than arithmetic, so it makes no
# difference today, and the ordering costs nothing and will still be right if that changes.
#
# Called with `sstatus.FS != Off`, which `fp.rs` guarantees: an `fld` under `FS == Off` raises the
# illegal-instruction trap this whole mechanism exists to serve, from inside the scheduler.
.global fp_restore
.type fp_restore, @function
fp_restore:
    .cfi_startproc
    .option push
    .option arch, +d
    ld   t0, 8(a0)
    csrw fcsr, t0
    fld  f0,   16(a0)
    fld  f1,   24(a0)
    fld  f2,   32(a0)
    fld  f3,   40(a0)
    fld  f4,   48(a0)
    fld  f5,   56(a0)
    fld  f6,   64(a0)
    fld  f7,   72(a0)
    fld  f8,   80(a0)
    fld  f9,   88(a0)
    fld  f10,  96(a0)
    fld  f11, 104(a0)
    fld  f12, 112(a0)
    fld  f13, 120(a0)
    fld  f14, 128(a0)
    fld  f15, 136(a0)
    fld  f16, 144(a0)
    fld  f17, 152(a0)
    fld  f18, 160(a0)
    fld  f19, 168(a0)
    fld  f20, 176(a0)
    fld  f21, 184(a0)
    fld  f22, 192(a0)
    fld  f23, 200(a0)
    fld  f24, 208(a0)
    fld  f25, 216(a0)
    fld  f26, 224(a0)
    fld  f27, 232(a0)
    fld  f28, 240(a0)
    fld  f29, 248(a0)
    fld  f30, 256(a0)
    fld  f31, 264(a0)
    .option pop
    ret
    .cfi_endproc
.size fp_restore, . - fp_restore
