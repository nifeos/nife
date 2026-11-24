# The x86_64 context switch and the first-run trampolines (the asm half of context.rs).
#
# Intel syntax, like the rest of this architecture's assembly.
#
# WHAT A CONTEXT IS HERE. The System V AMD64 ABI's callee-saved set is rbx, rbp, r12, r13, r14 and
# r15: six registers, against RISC-V's thirteen and aarch64's twelve. Everything else is the
# compiler's to spill, so a switch that saves those six and the return address has saved everything
# that can outlive a call. The saved stack pointer is the context pointer itself, so it is not a
# field, exactly as on the other two.

.section .text
.code64

# void switch_to(Context **prev_context, Context *next_context)
#
# rdi = where to store OUR context pointer, rsi = the context to resume.
#
# The last instruction returns to a DIFFERENT thread: `ret` pops the return address that thread
# pushed when it was switched away from (or, for a thread that has never run, the trampoline address
# context.rs put there).
# CFI (see partsfoundry/record:notes/cfi-unwind.md, and arch/aarch64/context.s's CFI note for the full argument, which
# applies unchanged here). One continuous frame description, no special handling at the `mov rsp,
# rsi` stack swap: the CFA formula is in terms of the CURRENT rsp, and `next_context` (rsi) is by
# construction the exact rsp another thread's own earlier call to this function (or
# `Context::for_*_thread`'s synthetic frame, context.rs) left after these same six pushes. The
# return address needs no special rule either: it is always at CFA-8 by the ordinary x86-64
# convention, which stays true across the swap for the same reason the six GP-register offsets do.
# In the pinned hot section, because every IPC that blocks runs it: milestone 796 (pin the
# hot trap path's placement).
# See partsfoundry/record:notes/benchmarks/kernel-footprint-and-caches.md, "The hot section".
.section ".text.hot.switch_to", "ax"
.global switch_to
.type switch_to, @function
switch_to:
    .cfi_startproc
    .cfi_def_cfa_offset 8
    push rbp
    .cfi_def_cfa_offset 16
    .cfi_offset rbp, -16
    push rbx
    .cfi_def_cfa_offset 24
    .cfi_offset rbx, -24
    push r12
    .cfi_def_cfa_offset 32
    .cfi_offset r12, -32
    push r13
    .cfi_def_cfa_offset 40
    .cfi_offset r13, -40
    push r14
    .cfi_def_cfa_offset 48
    .cfi_offset r14, -48
    push r15
    .cfi_def_cfa_offset 56
    .cfi_offset r15, -56
    mov [rdi], rsp                  # our context pointer is our stack pointer
    mov rsp, rsi                    # adopt theirs. No CFI directive belongs here; see the note above.
    pop r15
    .cfi_restore r15
    .cfi_def_cfa_offset 48
    pop r14
    .cfi_restore r14
    .cfi_def_cfa_offset 40
    pop r13
    .cfi_restore r13
    .cfi_def_cfa_offset 32
    pop r12
    .cfi_restore r12
    .cfi_def_cfa_offset 24
    pop rbx
    .cfi_restore rbx
    .cfi_def_cfa_offset 16
    pop rbp
    .cfi_restore rbp
    .cfi_def_cfa_offset 8
    ret
    .cfi_endproc
.size switch_to, . - switch_to
.section ".text", "ax"

# The first-run landing pad for a KERNEL thread.
#
# `Context::for_kernel_thread` put the closure pointer in rbx and the monomorphized call shim in
# rbp, because those are two of the six registers the switch restores. The closure's concrete type
# was erased, so the address says *where* and the shim says *how*.
#
# rsp is 16-byte aligned here by construction (see context.rs's alignment note), which is what the
# `call` below requires.
# CFI: never reached by `call`, this is the fake return address `Context::for_kernel_thread`
# writes into a synthetic switch frame, so `switch_to`'s `ret` lands here the first time this
# thread runs; the return address `ret` used is this function's own, not a real caller.
# `.cfi_undefined` on the return column (rip) says so; the `xor rbp, rbp` below is the same fact
# again, in the frame-pointer convention this ISA also has ("the bottom of the backtrace").
.global thread_trampoline
.type thread_trampoline, @function
thread_trampoline:
    .cfi_startproc
    .cfi_undefined rip
    .cfi_def_cfa_offset 8
    mov rdi, rbx                    # closure_at
    mov rsi, rbp                    # call_shim
    xor rbp, rbp                    # the bottom of the backtrace
    call thread_entry
    # thread_entry is `-> !`. If it ever returns, stop rather than run on.
1:  hlt
    jmp 1b
    .cfi_endproc
.size thread_trampoline, . - thread_trampoline

# The first-run landing pad for a USER thread. rbx = entry, rbp = user stack pointer,
# r13..r15 = the child's first three arguments.
#
# THE REGISTERS HERE WERE WRONG UNTIL MILESTONE 161 AND NOTHING COULD HAVE CAUGHT IT. This read
# r12, r13 and r14 while `Context::for_user_thread` wrote r13, r14 and r15, so a child would have
# received (0, arg0, arg1) and arg2 would have vanished. Both files were internally consistent and
# neither was executed: this port had no way to enter ring 3 until item 3 of its roadmap, so the
# only witness would have been a user program reading its own arguments. It is corrected against
# context.rs, whose per-field doc comments are the more specific of the two records.
#
# It is STILL not executed: this trampoline is the scheduler's entry path, and the scheduler has not
# been brought up on this architecture (roadmap item 4). The ring-3 self test enters through
# `enter_user` directly and does not pass here.
#
# RESERVE THE TRAP FRAME BEFORE THE FIRST RUST FRAME EXISTS (milestone 71's fix, carried across for
# parity; the other two ISAs each have this line and the reasoning in full). We arrive with
# rsp = the kernel stack top, and `user::enter_frame` puts this thread's TrapFrame at top - 176,
# where every trap from ring 3 will rebuild it (`TSS.RSP0` = top). Without this the entry path's own
# frames start at the same top and overlap the region `frame.write` is about to fill. 176 is
# size_of::<TrapFrame>(), asserted in exceptions.rs, and a multiple of 16 so rsp stays aligned for
# the `call` below.
# CFI: same fake-frame reasoning as thread_trampoline above.
.global user_entry_trampoline
.type user_entry_trampoline, @function
user_entry_trampoline:
    .cfi_startproc
    .cfi_undefined rip
    .cfi_def_cfa_offset 8
    sub rsp, 176                    # reserve [top-176, top) for this thread's TrapFrame
    .cfi_def_cfa_offset 184
    mov rdi, rbx
    mov rsi, rbp
    mov rdx, r13
    mov rcx, r14
    mov r8, r15
    xor rbp, rbp
    call user_thread_entry
1:  hlt
    jmp 1b
    .cfi_endproc
.size user_entry_trampoline, . - user_entry_trampoline
