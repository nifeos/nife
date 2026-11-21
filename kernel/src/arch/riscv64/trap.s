# The S-mode trap vector and the U-mode return path.
#
# The RISC-V analog of aarch64's vectors.s. RISC-V has one trap entry (stvec) and does NOT switch
# stacks automatically: a trap from U-mode arrives still on the user stack, with the user's `tp`.
#
# So `sscratch` holds a pointer to this hart's per-hart trap stash (`struct TrapStash` in mod.rs), in
# BOTH U- and S-mode. The stash carries what the entry needs to recover before it has a stack or a
# valid `tp`:
#
#   0(stash)  kernel_sp  the current thread's kernel-stack top (where a U-mode trap lands)
#   8(stash)  percpu     this hart's PerCpu pointer (the kernel `tp`)
#   16(stash) scratch0   a scratch word
#   24(stash) scratch1   a scratch word
#
# Unlike the old single-hart design (which kept the kernel `tp` in one global and the kernel-stack top
# directly in `sscratch`), the stash is per-hart and reached through the per-hart `sscratch`, so every
# hart recovers ITS OWN `tp` and stack. That is what makes the trap path SMP-correct. Whether a trap
# came from U- or S-mode is read from `sstatus.SPP`, not from `sscratch` (which is never 0 now).
#
# The frame layout is `struct TrapFrame`: x[0..32] then sepc, scause, stval, sstatus (288 bytes).

# `.text.hot.*` puts trap_entry, and the trap_return it falls through to, in the pinned hot
# section: milestone 796 (pin the hot trap path's placement). See
# notes/benchmarks/kernel-footprint-and-caches.md, "The hot section".
.section ".text.hot.trap_entry", "ax"
.balign 4                       # stvec direct mode needs the vector 4-byte aligned
# CFI: see notes/cfi-unwind.md, "The hard case: a trap is not a call" (written for aarch64's
# vectors.s; the same reasoning applies here). `.cfi_signal_frame` marks this as an
# interrupt/exception frame, not an ordinary call. The GP-register offsets below are correct, plain
# arithmetic on this function's own stores and let a debugger recover x1..x31 from any PC in this
# function. **ra (x1) is left `.cfi_undefined`**, even though its real value is saved a few lines
# down at `1*8(sp)`: RISC-V's default return-address column is ra, and describing it there would
# tell an unwinder "the interrupted code's caller is at this ra value", which is false -- the true
# resume point is `sepc`, a data value the hardware does not restore into any register on its own.
# Unlike AArch64 (ELR_mode, DWARF register 33; see vectors.s), the RISC-V DWARF register mapping has
# no dedicated pseudo-register for sepc, so there is no spec-correct alternative column to offer
# either; this is a real gap in the mapping, not only a GDB limitation. See the BUGS section in the
# note.
.global trap_entry
.type trap_entry, @function
trap_entry:
    .cfi_startproc
    .cfi_signal_frame
    .cfi_undefined ra
    # sscratch = &TrapStash for this hart. Swap it into t0, parking the interrupted t0 in sscratch.
    csrrw   t0, sscratch, t0    # t0 = &stash ; sscratch = interrupted t0
    sd      t1, 16(t0)          # scratch0 = interrupted t1 (frees t1)
    sd      sp, 24(t0)          # scratch1 = interrupted sp (frees sp)

    # Which stack to build the frame on? SPP (sstatus bit 8): 1 = from S-mode (stay on the sp we were
    # on), 0 = from U-mode (switch to this thread's kernel-stack top).
    csrr    t1, sstatus
    andi    t1, t1, 0x100
    bnez    t1, 1f
    ld      sp, 0(t0)           # U-mode: land on kernel_sp
    j       2f
1:  ld      sp, 24(t0)          # S-mode: the interrupted sp (saved in scratch1)
2:  addi    sp, sp, -288
    # The frame is real from here (either path above lands here with sp already at the base minus
    # 288); before this point sp was in flux and nothing had been saved yet, so there was nothing
    # correct to describe.
    .cfi_def_cfa_offset 288

    # Save the general registers. Registers we have not clobbered (x1, x3, x4, x7..x31) are saved
    # straight from their live values; the clobbered ones (x2=sp, x5=t0, x6=t1) come from the stash
    # or sscratch, where the entry parked them.
    sd      x1,  1*8(sp)
    .cfi_offset x1, -280
    sd      x3,  3*8(sp)
    .cfi_offset x3, -264
    sd      x4,  4*8(sp)        # interrupted tp; the kernel tp is reloaded below
    .cfi_offset x4, -256
    csrr    t1, sscratch        # interrupted t0 (parked in sscratch by the first swap)
    sd      t1,  5*8(sp)
    .cfi_offset x5, -248
    ld      t1, 16(t0)          # interrupted t1 (scratch0)
    sd      t1,  6*8(sp)
    .cfi_offset x6, -240
    sd      x7,  7*8(sp)
    .cfi_offset x7, -232
    sd      x8,  8*8(sp)
    .cfi_offset x8, -224
    sd      x9,  9*8(sp)
    .cfi_offset x9, -216
    sd      x10, 10*8(sp)
    .cfi_offset x10, -208
    sd      x11, 11*8(sp)
    .cfi_offset x11, -200
    sd      x12, 12*8(sp)
    .cfi_offset x12, -192
    sd      x13, 13*8(sp)
    .cfi_offset x13, -184
    sd      x14, 14*8(sp)
    .cfi_offset x14, -176
    sd      x15, 15*8(sp)
    .cfi_offset x15, -168
    sd      x16, 16*8(sp)
    .cfi_offset x16, -160
    sd      x17, 17*8(sp)
    .cfi_offset x17, -152
    sd      x18, 18*8(sp)
    .cfi_offset x18, -144
    sd      x19, 19*8(sp)
    .cfi_offset x19, -136
    sd      x20, 20*8(sp)
    .cfi_offset x20, -128
    sd      x21, 21*8(sp)
    .cfi_offset x21, -120
    sd      x22, 22*8(sp)
    .cfi_offset x22, -112
    sd      x23, 23*8(sp)
    .cfi_offset x23, -104
    sd      x24, 24*8(sp)
    .cfi_offset x24, -96
    sd      x25, 25*8(sp)
    .cfi_offset x25, -88
    sd      x26, 26*8(sp)
    .cfi_offset x26, -80
    sd      x27, 27*8(sp)
    .cfi_offset x27, -72
    sd      x28, 28*8(sp)
    .cfi_offset x28, -64
    sd      x29, 29*8(sp)
    .cfi_offset x29, -56
    sd      x30, 30*8(sp)
    .cfi_offset x30, -48
    sd      x31, 31*8(sp)
    .cfi_offset x31, -40
    sd      zero, 0*8(sp)
    ld      t1, 24(t0)          # interrupted sp (scratch1)
    sd      t1,  2*8(sp)
    .cfi_offset x2, -272

    # sepc, scause, stval, sstatus: not GP registers, and sepc in particular is the interrupted
    # code's real resume point, which is exactly the value ra's CFI column would need in order to
    # keep unwinding past this frame. No CFI rule describes it: RISC-V's DWARF register mapping,
    # unlike AArch64's (which has ELR_mode, register 33, for this), has no pseudo-register for a
    # CSR, so there is no column to point it at. See the file header's CFI note and the note's
    # BUGS section.
    csrr    t1, sepc
    sd      t1, 32*8(sp)
    csrr    t1, scause
    sd      t1, 33*8(sp)
    csrr    t1, stval
    sd      t1, 34*8(sp)
    csrr    t1, sstatus
    sd      t1, 35*8(sp)

    # Recover the kernel per-CPU pointer (`tp`) for THIS hart from the stash. A trap from U-mode
    # arrived with the user's tp; from S-mode it is already correct, and this reloads the same value.
    ld      tp, 8(t0)           # percpu

    # Restore sscratch to &stash (t0) for the next trap; it currently holds the interrupted t0, which
    # is already saved in the frame.
    csrw    sscratch, t0

    mv      a0, sp
    call    riscv64_trap_dispatch
    # fall through to trap_return

# Restore a TrapFrame at sp and return from the trap. Shared by the trap path and by the first entry
# to U-mode (enter_user). On a return to U-mode (sstatus.SPP == 0), record this thread's kernel-stack
# top in the hart's stash, so the next U-mode trap lands there.
trap_return:
    # MASK INTERRUPTS FIRST. The aarch64 twin of this sequence had a real race here (see the long
    # comment on RESTORE_CONTEXT in arch/aarch64/vectors.s, milestone 22 phase B.2), and the same
    # window exists on RISC-V for the same reason: `sepc` and `sstatus` are the sret's only record of
    # where to go and at what privilege, and a trap taken between the `csrw sepc` below and the
    # `csrw sstatus` two instructions later overwrites both. The nested handler's own trap_return puts
    # the S-mode values back, so our `sret` would return to U-mode at a *kernel* address, which reads
    # as an impossible instruction fault.
    #
    # On a trap return the window is already closed (trap entry clears sstatus.SIE, and the frame's
    # saved sstatus carries SIE = 0 through the `csrw`). The exposed path is `user_return`, the first
    # entry to U-mode, which is jumped to from ordinary kernel code with interrupts enabled. Clearing
    # SIE costs nothing: the `csrw sstatus` and then the `sret` set the final state regardless.
    csrci   sstatus, 2          # clear SIE (bit 1)

    # AND THE GUARD IS NOT SELF-SUFFICIENT HERE, WHICH IS WHERE RISC-V DIVERGES FROM AARCH64. The
    # aarch64 comment says the mask covers any future path "by construction"; that claim is true
    # there and NOT true here, so do not read it across.
    #
    # aarch64 stages the return state in SPSR_EL1, a register separate from the live PSTATE, so
    # nothing between the `msr daifset` and the `eret` can put interrupts back on. RISC-V has ONE
    # register: `sstatus` holds both the staged fields (SPP, SPIE) and the LIVE enable bit (SIE). So
    # the `csrw sstatus, t0` thirteen lines below writes the frame's SIE straight into the live bit,
    # and if a frame ever carried SIE = 1 it would re-open interrupts for the ~32 instructions
    # between there and the `sret`, with `sepc` already staged. The `csrci` above would have bought
    # nothing.
    #
    # It is safe today because of an INVARIANT, not because of this instruction: every trap frame
    # carries SIE = 0. Two sources, both checked:
    #
    #   - a real trap: the hardware clears SIE on trap entry (moving it to SPIE), so the `csrr t1,
    #     sstatus` in trap_entry above always reads SIE = 0;
    #   - a fabricated frame: TrapFrame::for_user_entry sets SPIE and UXL only. There is a
    #     compile-time assertion next to it that keeps SIE out; see exceptions.rs.
    #
    # If you add a third way to build a frame, that assertion is the thing to copy. See
    # notes/arch-audit.md, finding 1.

    ld      t0, 35*8(sp)        # sstatus
    andi    t1, t0, 0x100       # SPP (bit 8): 1 = return to S-mode, 0 = return to U-mode
    bnez    t1, 3f
    csrr    t0, sscratch        # &stash for this hart
    addi    t1, sp, 288         # this thread's kernel-stack top
    sd      t1, 0(t0)           # stash.kernel_sp
3:
    ld      t0, 32*8(sp)
    csrw    sepc, t0
    ld      t0, 35*8(sp)
    csrw    sstatus, t0

    ld      x1,  1*8(sp)
    .cfi_restore x1
    ld      x3,  3*8(sp)
    .cfi_restore x3
    # tp (x4). A U-mode thread owns its tp, so a return to U-mode restores the saved value. An S-mode
    # (kernel) thread's tp is THIS hart's PerCpu pointer, and it must name the hart we RESUME on, not
    # the one the frame was built on. A preempted kernel thread can migrate to another hart under SMP
    # load balancing (DECISIONS §28); its frame's saved tp is then stale, and restoring it would make
    # the thread read a different hart's per-CPU state (current, idle, run queue, held_rank) and
    # corrupt it. The live tp is already this hart's (trap_entry reloaded it from the per-hart stash,
    # and switch_to preserves it across a migration), so for an S-mode return we KEEP it and skip the
    # frame's copy. t0 still holds the frame's sstatus (loaded just above for the CSR write); SPP
    # (bit 8) is 1 for a return to S-mode. This is the return-path mirror of trap_entry's
    # `ld tp, 8(t0)`. See notes/riscv-port.md.
    andi    t1, t0, 0x100
    bnez    t1, 4f
    ld      x4,  4*8(sp)
4:  .cfi_restore x4
    ld      x5,  5*8(sp)
    .cfi_restore x5
    ld      x6,  6*8(sp)
    .cfi_restore x6
    ld      x7,  7*8(sp)
    .cfi_restore x7
    ld      x8,  8*8(sp)
    .cfi_restore x8
    ld      x9,  9*8(sp)
    .cfi_restore x9
    ld      x10, 10*8(sp)
    .cfi_restore x10
    ld      x11, 11*8(sp)
    .cfi_restore x11
    ld      x12, 12*8(sp)
    .cfi_restore x12
    ld      x13, 13*8(sp)
    .cfi_restore x13
    ld      x14, 14*8(sp)
    .cfi_restore x14
    ld      x15, 15*8(sp)
    .cfi_restore x15
    ld      x16, 16*8(sp)
    .cfi_restore x16
    ld      x17, 17*8(sp)
    .cfi_restore x17
    ld      x18, 18*8(sp)
    .cfi_restore x18
    ld      x19, 19*8(sp)
    .cfi_restore x19
    ld      x20, 20*8(sp)
    .cfi_restore x20
    ld      x21, 21*8(sp)
    .cfi_restore x21
    ld      x22, 22*8(sp)
    .cfi_restore x22
    ld      x23, 23*8(sp)
    .cfi_restore x23
    ld      x24, 24*8(sp)
    .cfi_restore x24
    ld      x25, 25*8(sp)
    .cfi_restore x25
    ld      x26, 26*8(sp)
    .cfi_restore x26
    ld      x27, 27*8(sp)
    .cfi_restore x27
    ld      x28, 28*8(sp)
    .cfi_restore x28
    ld      x29, 29*8(sp)
    .cfi_restore x29
    ld      x30, 30*8(sp)
    .cfi_restore x30
    ld      x31, 31*8(sp)
    .cfi_restore x31
    ld      x2,  2*8(sp)        # the interrupted sp (user sp for a U-mode return)
    # This is the CFA-defining register changing to an unrelated value (the interrupted context's
    # own sp), the same move `switch_to` makes; nothing after this point needs the old formula, and
    # `sret` is the very next instruction.
    .cfi_restore x2

    sret
    .cfi_endproc
.size trap_entry, . - trap_entry

# Off the hot section: nothing below runs on a system_call.
.section ".text", "ax"

# RUN THE HANDLER ON THIS HART'S INTERRUPT STACK (milestone 124).
#
#   a0 = &mut TrapFrame      a1 = the stack to run on, or 0 to stay
#
# The twin of aarch64's `dispatch_on_interrupt_stack` in vectors.s, and the same contract: the frame
# stays where trap_entry built it (a preempted thread's frame must survive until that thread runs
# again, which a per-hart stack cannot promise), and everything above it moves. Rust decides whether
# to switch, in `interrupt_stack::top_for_trap`; this only moves `sp`.
#
# s0 holds the interrupted sp across the call because it is callee-saved, so the handler cannot
# clobber it and it needs no slot on either stack. Its own save costs the interrupted stack 16 bytes.
# `riscv64_trap_body` returns its bool in a0, which nothing here touches.
# CFI: an ordinary function, `call`ed and `ret`urning, so the default frame is real. The one wrinkle
# is the same one `switch_to` has (see arch/aarch64/context.s's CFI note): `mv sp, a1` moves onto an
# unrelated interrupt stack partway through, so the CFA formula is restated in terms of `s0` (which
# holds the pre-switch sp and does not move again until it is restored) rather than `sp`, exactly
# across the window where `sp` itself is not that value.
.global dispatch_on_interrupt_stack
.type dispatch_on_interrupt_stack, @function
dispatch_on_interrupt_stack:
    .cfi_startproc
    addi    sp, sp, -16
    .cfi_def_cfa_offset 16
    sd      ra, 8(sp)
    .cfi_offset ra, -8
    sd      s0, 0(sp)
    .cfi_offset s0, -16

    beqz    a1, 5f                  # 0: stay on this stack (from U-mode, pre-init, or nesting)
    mv      s0, sp
    .cfi_def_cfa_register s0
    mv      sp, a1
    call    riscv64_trap_body
    mv      sp, s0                  # back to the interrupted stack BEFORE anything can switch away
    .cfi_def_cfa_register sp
    j       6f
5:  call    riscv64_trap_body

6:  ld      ra, 8(sp)
    .cfi_restore ra
    ld      s0, 0(sp)
    .cfi_restore s0
    addi    sp, sp, 16
    .cfi_def_cfa_offset 0
    ret
    .cfi_endproc
.size dispatch_on_interrupt_stack, . - dispatch_on_interrupt_stack

# The first entry to U-mode: load `frame` (a0) as the trap frame and return into it. The frame was
# built by TrapFrame::for_user_entry with sstatus.SPP = 0 (U-mode) and SPIE set, sepc = the entry,
# x[2] = the user sp, a0..a2 = the child's arguments. Reached only through `enter_user` in
# exceptions.rs, which is #[inline(always)] so the frame (sitting on this same kernel stack) is not
# clobbered by a call-frame push before the `mv sp, a0`.
# CFI: reached by an ordinary `call`, so ra is real on entry, but `mv sp, a0` immediately points sp
# at a caller-fabricated TrapFrame rather than anything reachable from ra, and this never returns
# to that caller (it falls into trap_return's own `sret`). Treated conservatively as
# `.cfi_undefined ra` for the whole function; see the identical call on aarch64's
# `enter_userspace` in vectors.s for the one-instruction-window reasoning this is skipping.
.global user_return
.type user_return, @function
user_return:                    # a0 = *mut TrapFrame
    .cfi_startproc
    .cfi_undefined ra
    mv      sp, a0
    # `tail`, not `j`: trap_return lives in the pinned hot section at the front of `.text`, and a
    # `j` (JAL, +/-1 MiB) does not reach it from here in a debug test image, whose text is larger
    # (found by CI on 2026-10-07: `relocation R_RISCV_JAL out of range`). `tail` is auipc + jr
    # through t1, which trap_return writes before it reads. The linker relaxes it back to a `j`
    # wherever that reaches.
    tail    trap_return
    .cfi_endproc
.size user_return, . - user_return
