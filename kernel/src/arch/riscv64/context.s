# The context switch and the two first-run trampolines, RISC-V. The asm half of context.rs.
#
# This mirrors arch/aarch64/context.s exactly in structure; only the register set differs. A
# voluntary switch is an ordinary function call, so the RISC-V calling convention already lets the
# caller assume the temporaries (t0-t6, a0-a7) are clobbered. We save only what the convention
# says a callee preserves: ra (the return address, i.e. WHERE TO RETURN) and s0-s11. sp is the
# saved state itself, so it is not stored in the frame; it is what `switch_to` swaps.
#
# The trick is identical to aarch64: `ret` returns into `ra`, and by that point `ra` was loaded
# from the OTHER thread's stack, so the return lands in a different thread. See partsfoundry/record:notes/threads.md.
#
# UNPROVEN until RISC-V boots (the console step and beyond exercise it). It assembles and links now,
# which is all the compile milestone claims.

# In the pinned hot section, because every IPC that blocks runs it: milestone 796 (pin the
# hot trap path's placement).
# See partsfoundry/record:notes/benchmarks/kernel-footprint-and-caches.md, "The hot section".
.section ".text.hot.switch_to", "ax"

# switch_to(prev_context: *mut *mut Context, next_context: *mut Context)
#   a0 = where to STORE our stack pointer (&mut prev.context)
#   a1 = the stack pointer to RESTORE     (next.context)
# The RISC-V calling convention puts the first two arguments in a0 and a1.
# CFI (see partsfoundry/record:notes/cfi-unwind.md and, for the full argument, the identical technique in
# arch/aarch64/context.s). One continuous frame description, no special handling at the stack
# swap: `.cfi_def_cfa_offset 112` is a formula in terms of the CURRENT sp, and `next_context` (a1)
# is by construction the exact sp another thread's own earlier call to this function (or
# `Context::for_*_thread`'s synthetic frame) left behind after this same `addi sp, sp, -112`. So
# the formula, and the thirteen `.cfi_offset` rules below, stay correct on both sides of `mv sp,
# a1`.
.global switch_to
.type switch_to, @function
switch_to:
    .cfi_startproc
    # Push the callee-saved registers. 112 bytes: 13 registers plus 8 bytes of padding, a multiple
    # of 16 so sp stays aligned. The layout matches `struct Context` field for field.
    addi sp, sp, -112
    .cfi_def_cfa_offset 112
    sd   ra,   0(sp)
    .cfi_offset ra, -112
    sd   s0,   8(sp)
    .cfi_offset s0, -104
    sd   s1,  16(sp)
    .cfi_offset s1, -96
    sd   s2,  24(sp)
    .cfi_offset s2, -88
    sd   s3,  32(sp)
    .cfi_offset s3, -80
    sd   s4,  40(sp)
    .cfi_offset s4, -72
    sd   s5,  48(sp)
    .cfi_offset s5, -64
    sd   s6,  56(sp)
    .cfi_offset s6, -56
    sd   s7,  64(sp)
    .cfi_offset s7, -48
    sd   s8,  72(sp)
    .cfi_offset s8, -40
    sd   s9,  80(sp)
    .cfi_offset s9, -32
    sd   s10, 88(sp)
    .cfi_offset s10, -24
    sd   s11, 96(sp)
    .cfi_offset s11, -16
    # offset 104 is the padding field; nothing is written there.

    # Remember where we put them: THIS is the entire saved state of a thread, a single sp.
    sd   sp, 0(a0)

    # And now we run on somebody else's stack. No CFI directive belongs here; see the note above.
    mv   sp, a1

    # Pop THEIR callee-saved registers, pushed by their own call to switch_to, whenever that was
    # (or faked by Context::for_*_thread for a thread that has never run).
    ld   ra,   0(sp)
    .cfi_restore ra
    ld   s0,   8(sp)
    .cfi_restore s0
    ld   s1,  16(sp)
    .cfi_restore s1
    ld   s2,  24(sp)
    .cfi_restore s2
    ld   s3,  32(sp)
    .cfi_restore s3
    ld   s4,  40(sp)
    .cfi_restore s4
    ld   s5,  48(sp)
    .cfi_restore s5
    ld   s6,  56(sp)
    .cfi_restore s6
    ld   s7,  64(sp)
    .cfi_restore s7
    ld   s8,  72(sp)
    .cfi_restore s8
    ld   s9,  80(sp)
    .cfi_restore s9
    ld   s10, 88(sp)
    .cfi_restore s10
    ld   s11, 96(sp)
    .cfi_restore s11
    addi sp, sp, 112
    .cfi_def_cfa_offset 0

    # ra now holds the OTHER thread's return address. This does not go back to our caller.
    ret
    .cfi_endproc
.size switch_to, . - switch_to

.section ".text", "ax"

# Where a brand-new KERNEL thread begins. Context::for_kernel_thread faked a frame whose ra points
# here, with s0 = the closure pointer and s1 = the monomorphized caller, both restored by switch_to
# on the way in. We hand them to the portable `thread_entry`, which never returns.
.global thread_trampoline
.type thread_trampoline, @function
thread_trampoline:
    .cfi_startproc
    # Never reached by `call`: the fake return address Context::for_kernel_thread writes into a
    # synthetic switch frame. ra here is self-referential (still this function's own address,
    # restored by switch_to's `ret`), not a real caller.
    .cfi_undefined ra
    # We arrive with interrupts masked (as on aarch64); the portable `thread_entry` unmasks after
    # `finish_switch`, never here. See arch/aarch64/context.s for the hang an early unmask caused.
    mv   a0, s0            # the closure, on this thread's own stack
    mv   a1, s1            # extern "C" fn(*mut ()): the closure's monomorphized caller
    call thread_entry      # extern "C" fn(*mut (), fn(*mut ())) -> !  -- never returns
1:  wfi
    j    1b
    .cfi_endproc
.size thread_trampoline, . - thread_trampoline

# Where a brand-new USER thread begins: the U-mode mirror of thread_trampoline.
# Context::for_user_thread faked a frame whose ra points here, with s0 = the U-mode entry, s1 = the
# user sp, and s2..s4 = the child's initial a0..a2. The portable `user_thread_entry` reaps our
# predecessor and drops to U-mode.
#
# RESERVE THE TRAP FRAME BEFORE THE FIRST RUST FRAME EXISTS (milestone 71). We arrive with
# sp = the kernel stack top, and `enter_frame` will put this thread's TrapFrame at top - 288 and
# leave it there for the life of the thread: every U-mode trap lands on `stash.kernel_sp` (= top)
# and rebuilds the frame at exactly that address. So the entry path must never own those bytes.
# Dropping sp by a frame's worth here is what makes that true by construction, on a path that is
# otherwise shallow enough to overlap. The alternative, computing the frame's address from the live
# sp so it lands below the entry path, is what milestone 71 removed: it put the frame 16 bytes off
# from where trap.s builds an S-mode frame, so any interrupt in the window rewrote it. The 288 is
# size_of::<TrapFrame>(), asserted in exceptions.rs, and is a multiple of 16 so sp stays aligned.
.global user_entry_trampoline
.type user_entry_trampoline, @function
user_entry_trampoline:
    .cfi_startproc
    # Same fake-frame reasoning as thread_trampoline above.
    .cfi_undefined ra
    addi sp, sp, -288      # reserve [top-288, top) for this thread's TrapFrame
    .cfi_def_cfa_offset 288
    mv   a0, s0            # the U-mode entry address
    mv   a1, s1            # the user stack pointer
    mv   a2, s2            # the child's initial a0
    mv   a3, s3            # ...a1
    mv   a4, s4            # ...a2
    call user_thread_entry # extern "C" fn(u64, u64, u64, u64, u64) -> !  -- never returns
1:  wfi
    j    1b
    .cfi_endproc
.size user_entry_trampoline, . - user_entry_trampoline
