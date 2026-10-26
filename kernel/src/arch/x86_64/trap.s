# The x86_64 trap entry: 256 stubs, one common save/restore path, one Rust handler.
#
# Intel syntax, like boot.s, because that is `global_asm!`'s default on x86.
#
# WHY 256 STUBS AND NOT ONE VECTOR TABLE. aarch64 has sixteen exception vectors, each 128 bytes,
# reached by an offset the hardware computes from the exception class; RISC-V has one, and the cause
# is in `scause`. x86 has neither: the CPU jumps to the address in IDT entry N and tells the handler
# **nothing** about which N that was. The vector number is only recoverable if each entry points at
# a different piece of code that knows its own number. So the number is pushed by the stub, and the
# stubs are generated rather than written out.
#
# THE ERROR CODE IS THE SECOND ASYMMETRY, and it is worse than it looks. Ten of the 32 architectural
# exceptions push an extra word before the return frame; the other 246 vectors do not. If the common
# path did not know which, `iretq` would return to whatever the error code happened to be. The stubs
# push a dummy zero for the vectors that have none, so exactly one frame layout reaches Rust and
# exactly one `add rsp, 16` undoes it.
#
# THE `swapgs` PAIR, AND WHY IT IS GUARDED RATHER THAN UNCONDITIONAL (milestone 161, roadmap
# item 3). A trap from ring 3 arrives with the USER's GS base, and this kernel keeps its per-CPU
# pointer in the kernel's, so the first thing that takes a lock would read whatever the user left in
# `IA32_GS_BASE`. `swapgs` exchanges `IA32_GS_BASE` with `IA32_KERNEL_GS_BASE`, which is what makes
# the kernel's pointer unforgeable: the value the kernel needs is in a register the user cannot
# write, exactly as RISC-V's `sscratch` is.
#
# It is an EXCHANGE, not a load, so it must run exactly once per privilege change in each direction.
# A trap from ring 0 that swapped would install the user's base while running kernel code; a nested
# trap that swapped again would put the kernel's back and then hand it to the user on the way out.
# So both sites test the saved CS's low two bits, which ARE the interrupted CPL, and swap only for a
# 3. Note what is deliberately not tested: `SS`, which a `syscall` return leaves unreliable, and the
# frame's RPL alone, which is the same two bits read a longer way round.
#
# `swapgs` writes an MSR pair and does NOT load a segment register, so it does not trip the hazard
# `segments.rs` documents at length (loading a segment register in long mode zeroes that segment's
# base MSR). Verified against the SDM's description of the instruction rather than assumed, because
# that hazard has already cost this port an afternoon once.
#
# THE WINDOW ON THE WAY OUT is the one place this is delicate. Between the exit `swapgs` and the
# `iretq` the CPU is in ring 0 holding the USER's GS base, so an interrupt taken there would see
# CPL 0, decline to swap, and dereference the user's value. `RFLAGS.IF` is clear at every such site
# (an interrupt gate cleared it on entry; `IA32_FMASK` clears it for `syscall`; the two entry points
# below `cli` before they touch anything), which closes it for everything except an NMI or a machine
# check. The same is true on the way in: between `syscall` and its `swapgs`, CS says ring 0 and the
# GS base is still the user's.
#
# SO THE NMI, THE DOUBLE FAULT AND THE MACHINE CHECK DO NOT DECIDE FROM CS AT ALL. Their stubs jump
# to `isr_paranoid` instead of `isr_common`. They arrive on IST stacks of their own (ist.rs), and the
# word just above each such stack's frame holds the core's own per-CPU pointer, written at bring-up.
# `isr_paranoid` reads `IA32_GS_BASE`, and if it is not that pointer it saves it in `rbx` and writes
# the pointer in; on the way out it writes the saved value back. That is right in every case: from
# ring 3, from ring 0 with the kernel's base, and from either window above with the user's base,
# because it neither trusts CS nor exchanges anything, and `IA32_KERNEL_GS_BASE` is never touched.
# The double fault does not return, but takes this path because its report prints, and printing
# takes a lock that reads `gs`.

.section .text
.code64

# ---------------------------------------------------------------------------------------------
# One stub per vector.
#
# `.altmacro` is what makes `%vec` expand to the counter's VALUE at the macro call, which is the
# only way to build 256 distinct labels from a `.rept`. Without it the label would literally be
# `isr_vec` 256 times over.
# ---------------------------------------------------------------------------------------------
.altmacro

# CFI on the 256 stubs below: each gets its own tiny region (`.cfi_startproc`/`.cfi_endproc` inside
# the macro, so it expands 256 times), because there is nothing to unwind to yet at this point --
# the hardware has pushed its five words but no general register is saved anywhere until
# `isr_common` runs, which is where the real (and reusable, since all 256 stubs hand it an
# identically-shaped stack; see the comment above `isr_common`) trap-frame CFI lives. `.cfi_undefined`
# on the return column says a stub has no caller to describe, the same call vectors.s makes for
# AArch64's trap entries. `.type`/`.size` per stub for the same reason every other function symbol
# in this tree gets one; 256 nearly-identical entries is what this ISA's IDT dispatch costs.
.macro ISR_STUB num
.global isr_\num
.type isr_\num, @function
isr_\num:
    .cfi_startproc
    .cfi_signal_frame
    .cfi_undefined rip
    # The ten vectors that push a hardware error code: #DF(8), #TS(10), #NP(11), #SS(12), #GP(13),
    # #PF(14), #AC(17), #CP(21), #VC(29), #SX(30). Everything else gets a zero so the frame the
    # common path sees is one shape.
    .if (\num != 8) && (\num != 10) && (\num != 11) && (\num != 12) && (\num != 13) && (\num != 14) && (\num != 17) && (\num != 21) && (\num != 29) && (\num != 30)
    push 0
    .endif
    push \num
    # The NMI, double fault and machine check take the paranoid path; see this file's header.
    .if (\num == 2) || (\num == 8) || (\num == 18)
    jmp isr_paranoid
    .else
    jmp isr_common
    .endif
    .cfi_endproc
    .size isr_\num, . - isr_\num
.endm

.set vec, 0
.rept 256
    ISR_STUB %vec
    .set vec, vec + 1
.endr

# The fifteen general registers, pushed and popped in `TrapFrame`'s order with their CFI, shared
# by `isr_common` and `isr_paranoid` so the two cannot disagree about the frame.
.macro PUSH_GPRS
    push r15
    .cfi_def_cfa_offset 8
    .cfi_offset r15, -8
    push r14
    .cfi_def_cfa_offset 16
    .cfi_offset r14, -16
    push r13
    .cfi_def_cfa_offset 24
    .cfi_offset r13, -24
    push r12
    .cfi_def_cfa_offset 32
    .cfi_offset r12, -32
    push r11
    .cfi_def_cfa_offset 40
    .cfi_offset r11, -40
    push r10
    .cfi_def_cfa_offset 48
    .cfi_offset r10, -48
    push r9
    .cfi_def_cfa_offset 56
    .cfi_offset r9, -56
    push r8
    .cfi_def_cfa_offset 64
    .cfi_offset r8, -64
    push rbp
    .cfi_def_cfa_offset 72
    .cfi_offset rbp, -72
    push rdi
    .cfi_def_cfa_offset 80
    .cfi_offset rdi, -80
    push rsi
    .cfi_def_cfa_offset 88
    .cfi_offset rsi, -88
    push rdx
    .cfi_def_cfa_offset 96
    .cfi_offset rdx, -96
    push rcx
    .cfi_def_cfa_offset 104
    .cfi_offset rcx, -104
    push rbx
    .cfi_def_cfa_offset 112
    .cfi_offset rbx, -112
    push rax
    .cfi_def_cfa_offset 120
    .cfi_offset rax, -120
.endm

.macro POP_GPRS
    pop rax
    .cfi_restore rax
    .cfi_def_cfa_offset 112
    pop rbx
    .cfi_restore rbx
    .cfi_def_cfa_offset 104
    pop rcx
    .cfi_restore rcx
    .cfi_def_cfa_offset 96
    pop rdx
    .cfi_restore rdx
    .cfi_def_cfa_offset 88
    pop rsi
    .cfi_restore rsi
    .cfi_def_cfa_offset 80
    pop rdi
    .cfi_restore rdi
    .cfi_def_cfa_offset 72
    pop rbp
    .cfi_restore rbp
    .cfi_def_cfa_offset 64
    pop r8
    .cfi_restore r8
    .cfi_def_cfa_offset 56
    pop r9
    .cfi_restore r9
    .cfi_def_cfa_offset 48
    pop r10
    .cfi_restore r10
    .cfi_def_cfa_offset 40
    pop r11
    .cfi_restore r11
    .cfi_def_cfa_offset 32
    pop r12
    .cfi_restore r12
    .cfi_def_cfa_offset 24
    pop r13
    .cfi_restore r13
    .cfi_def_cfa_offset 16
    pop r14
    .cfi_restore r14
    .cfi_def_cfa_offset 8
    pop r15
    .cfi_restore r15
    .cfi_def_cfa_offset 0
.endm

# ---------------------------------------------------------------------------------------------
# The common path. The push order below IS the layout of `TrapFrame` in exceptions.rs, read
# backwards: the first push lands at the highest address, so `r15` is the last field of the register
# block and `rax` is the first. Reorder one line and Rust reads a different register's value under
# the right name, silently.
# ---------------------------------------------------------------------------------------------
# CFI: this is where the real trap-frame description lives, shared by all 256 stubs (see the note
# above ISR_STUB) because every one of them hands it an identically-shaped stack: the hardware's
# five words, then one error-code word (real or the stub's dummy zero), then the vector number,
# always in that order, always seven words, regardless of which vector fired.
#
# `.cfi_signal_frame` marks it as an interrupt frame. Unlike AArch64's vector entries and RISC-V's
# trap_entry, this one CAN describe the interrupted context's PC and stack pointer, and so keep an
# unwind going past the trap rather than stopping at it: the x86-64 hardware frame puts RIP and RSP
# at fixed, known offsets from here (CFA+16 and CFA+40 below), and the SysV DWARF mapping already
# treats register 16 as the return-address column and register 7 as an ordinary describable
# register, so no special pseudo-register or GDB cooperation is needed the way AArch64's ELR_mode
# would need (see vectors.s's CFI note for that comparison, and notes/cfi-unwind.md for what this
# bought in practice).
# From here to x86_syscall_entry's end is the pinned hot section: milestone 796 (pin the hot
# trap path's placement).
# See notes/benchmarks/kernel-footprint-and-caches.md, "The hot section". A `syscall` runs only
# isr_restore and x86_syscall_entry. isr_common rides along because it falls through into
# isr_restore, and splitting them would add a `jmp` to every IDT trap.
.section ".text.hot.x86_syscall_entry", "ax"
.type isr_common, @function
isr_common:
    .cfi_startproc
    .cfi_signal_frame
    .cfi_def_cfa_offset 0
    .cfi_offset 16, 16              # rip (the return-address column): the interrupted PC
    .cfi_offset 7, 40                # rsp: the interrupted stack pointer
    PUSH_GPRS

    # Recover the kernel's per-CPU pointer if this trap crossed a privilege boundary. `cs` is field
    # 18 of the TrapFrame (offsets are asserted against `offset_of!` in exceptions.rs), and its low
    # two bits are the interrupted CPL. See this file's header for why the test is here and not a
    # bare `swapgs`.
    test byte ptr [rsp + 18*8], 3
    jz 1f
    swapgs
1:

    # The System V ABI requires DF clear on entry to a C function, and an interrupt can land while
    # a `std`-using routine holds it set. Nothing in this kernel sets DF, but the handler is not the
    # place to be relying on that.
    cld

    # The frame is 22 quadwords = 176 bytes, and the CPU aligned rsp to 16 before pushing its own
    # part, so rsp is 16-byte aligned here and `call` leaves it at the 8-mod-16 the ABI expects.
    mov rdi, rsp
    call x86_trap_dispatch

    # Restore a TrapFrame at rsp and return from the trap. Shared by the IDT path above, by the
    # `syscall` path below, and by the first-entry-to-ring-3 path after that, which is what keeps
    # "how a frame becomes running registers" a single piece of code with a single swapgs rule. The
    # RISC-V twin of this label is `trap_return`.
isr_restore:
    # ON A RETURN TO RING 3, RECORD WHERE THE NEXT TRAP FROM THIS THREAD SHOULD LAND (milestone
    # 161, roadmap item 4). Until the scheduler existed there was one user program and one kernel
    # stack, and `ring3_self_test` set both by hand; with threads there is one kernel stack per
    # thread and the two doors into the kernel have to be re-pointed every time the thread that
    # would come through them changes.
    #
    # THE FRAME'S OWN ADDRESS IS THE ANSWER, which is what makes this a rule rather than a
    # bookkeeping duty somebody has to remember at each switch. Every thread's TrapFrame lives at
    # `stack_top - 176` for the life of the thread (`user::enter_frame`; milestone 71 (the
    # thread-start fault: a user thread dispatched with `sepc` = 0)), so the top is `rsp + 176` at
    # this instant, computed from the frame we are about to load rather than from any record of who
    # is running. RISC-V does exactly this, at exactly this point, in
    # `trap_return`.
    #
    # Two writes, because x86 has two doors and they find their stack differently: a trap reads
    # `TSS.rsp0` and `syscall` reads nothing at all. This store pair IS the whole mechanism since
    # the Rust-side setter pair was deleted (2026-09-30): it runs on every return to ring 3, the
    # boot thread's first one included, so nothing else ever needs to write either slot.
    #
    # rax and rcx are free here: every general register is still in the frame below and is about to
    # be popped over.
    #
    # BOTH TARGETS ARE THIS CORE'S OWN, reached through `gs` (milestone 161's SMP item): `gs` names
    # this core's `PerCpu` block throughout this window (the guard above only swaps it back to the
    # interrupted ring's value further down, at label 2), so a `gs`-relative store can only ever
    # land on the running core's own slot. `SYSCALL_KERNEL_RSP_OFF` is a direct value (the syscall
    # path's own scratch, inside `PerCpu`); `TSS_RSP0_PTR_OFF` is a POINTER `segments::initialize` wrote
    # once, to this core's own `TSS[cpu::id()].rsp0`, because the TSS array itself is not reachable
    # through `gs` the way `PerCpu` is.
    test byte ptr [rsp + 18*8], 3   # cs, whose low two bits are the ring we are returning to
    jz 3f
    lea rax, [rsp + 176]            # this thread's kernel-stack top
    mov gs:[{SYSCALL_KERNEL_RSP_OFF}], rax
    mov rcx, gs:[{TSS_RSP0_PTR_OFF}]
    mov [rcx], rax
3:
    POP_GPRS

    # The registers are gone, so `cs` is now three quadwords up: vector, error code, rip, cs. Swap
    # back if we are returning to ring 3, and do it as late as possible: everything between here and
    # the `iretq` runs in ring 0 holding the user's GS base.
    test byte ptr [rsp + 3*8], 3
    jz 2f
    swapgs
2:

    add rsp, 16                     # discard the vector number and the error code
    iretq
    .cfi_endproc
.size isr_common, . - isr_common

# ---------------------------------------------------------------------------------------------
# The `syscall` entry (milestone 161, roadmap item 3). `IA32_LSTAR` points here.
#
# THIS IS NOT AN IDT VECTOR AND THE DIFFERENCES ALL MATTER. `syscall` is a two-cycle jump, not an
# interrupt: it does not consult the IDT, does not read `TSS.RSP0`, and therefore **does not switch
# stacks** -- `rsp` still names the user's. It also does not push anything. What it does is save
# `rip` into `rcx` and `RFLAGS` into `r11` (which is why the ABI's fourth argument rides in `r10`;
# see exceptions.rs), load `rip` from `IA32_LSTAR`, take CS/SS from `IA32_STAR[47:32]`, and clear
# every `RFLAGS` bit named by `IA32_FMASK` -- including `IF`, so this arrives with interrupts
# masked exactly as an interrupt gate would.
#
# So the first three instructions are the whole of what the hardware did not do, and their order is
# forced: nothing may touch a lock (which reads the per-CPU block through `gs`) before the `swapgs`,
# and nothing may push before `rsp` names a kernel stack.
#
# THE KERNEL STACK COMES FROM THIS CORE'S `PerCpu` BLOCK, reached through `gs` (milestone 161's SMP
# item; `gs` has just been swapped to the kernel's, so it names THIS core's own block, no other
# core's). The store pair below keeps this slot (`SYSCALL_KERNEL_RSP_OFF`) and `TSS.RSP0` in step
# on every return to ring 3, so the two mechanisms cannot name different stacks.
# ---------------------------------------------------------------------------------------------
# CFI: builds the same shape isr_common does, by hand instead of by hardware, so the same
# description applies: register 16 (rip) and register 7 (rsp) below describe the interrupted USER
# context (rip from `rcx`, rsp from the scratch slot `syscall` itself could not push), exactly the
# way isr_common's hardware-pushed words do. The CFA baseline starts AFTER the stack switch
# (`mov rsp, gs:[...]`) rather than trying to describe the two-instruction window beforehand, where
# rsp names the user's stack and nothing has been saved anywhere yet; see dispatch_on_interrupt_stack
# below for the same kind of narrow, deliberately-undescribed window.
.global x86_syscall_entry
.type x86_syscall_entry, @function
x86_syscall_entry:
    .cfi_startproc
    swapgs
    mov gs:[{SYSCALL_USER_RSP_OFF}], rsp
    mov rsp, gs:[{SYSCALL_KERNEL_RSP_OFF}]
    .cfi_signal_frame

    # Build the same 22-quadword TrapFrame the IDT stubs build, so `isr_restore` above serves this
    # path unchanged and `crate::syscall::dispatch` reads one layout. The five words at the top are
    # the ones a real trap's hardware would have pushed, reconstructed from where `syscall` put
    # them.
    push {USER_DATA}                 # ss
    push gs:[{SYSCALL_USER_RSP_OFF}]        # rsp, as it was in ring 3 (this core's own scratch slot)
    push r11                                # rflags, as `syscall` saved them
    push {USER_CODE}                 # cs, whose low two bits make the exit `swapgs` fire
    push rcx                                # rip: the instruction after the `syscall`
    push 0                                  # error code: there is none
    push {SYSCALL_VECTOR}                     # not an IDT vector; see exceptions.rs

    # The CFA baseline is declared HERE, after the seven words above, not at function entry: it is
    # the same "CFA = the point where the GP-register pushes begin" convention isr_common uses, so
    # everything from here down is copy-identical to isr_common's own offsets. Placed after rather
    # than before also means register 16 (rip) and register 7 (rsp) below describe memory that has
    # actually been written by this point (rcx and the scratch slot, just pushed), not memory this
    # function has not touched yet.
    .cfi_def_cfa_offset 0
    .cfi_offset 16, 16               # rip: the interrupted user PC (pushed from rcx, above)
    .cfi_offset 7, 40                # rsp: the interrupted user stack pointer (pushed above)

    push r15
    .cfi_def_cfa_offset 64
    .cfi_offset r15, -64
    push r14
    .cfi_def_cfa_offset 72
    .cfi_offset r14, -72
    push r13
    .cfi_def_cfa_offset 80
    .cfi_offset r13, -80
    push r12
    .cfi_def_cfa_offset 88
    .cfi_offset r12, -88
    push r11
    .cfi_def_cfa_offset 96
    .cfi_offset r11, -96
    push r10
    .cfi_def_cfa_offset 104
    .cfi_offset r10, -104
    push r9
    .cfi_def_cfa_offset 112
    .cfi_offset r9, -112
    push r8
    .cfi_def_cfa_offset 120
    .cfi_offset r8, -120
    push rbp
    .cfi_def_cfa_offset 128
    .cfi_offset rbp, -128
    push rdi
    .cfi_def_cfa_offset 136
    .cfi_offset rdi, -136
    push rsi
    .cfi_def_cfa_offset 144
    .cfi_offset rsi, -144
    push rdx
    .cfi_def_cfa_offset 152
    .cfi_offset rdx, -152
    push rcx
    .cfi_def_cfa_offset 160
    .cfi_offset rcx, -160
    push rbx
    .cfi_def_cfa_offset 168
    .cfi_offset rbx, -168
    push rax
    .cfi_def_cfa_offset 176
    .cfi_offset rax, -176

    cld
    mov rdi, rsp
    call x86_syscall_handler
    jmp isr_restore
    .cfi_endproc
.size x86_syscall_entry, . - x86_syscall_entry
.section ".text", "ax"

# RUN THE HANDLER ON THIS CPU'S INTERRUPT STACK (milestone 124, brought to this architecture by
# milestone 161's roadmap item 4).
#
#   rdi = &mut TrapFrame     rsi = the stack to run on, or 0 to stay
#
# The twin of `dispatch_on_interrupt_stack` in the other two ports' trap assembly, and the same
# contract: the frame stays where the stub built it, because a preempted thread's frame must survive
# until that thread runs again and a per-CPU stack cannot promise that; everything above it moves.
# Rust decides whether to switch, in `interrupt_stack::top_for_trap`; this only moves `rsp`.
#
# `rbp` holds the interrupted `rsp` across the call because it is callee-saved, so the handler cannot
# clobber it and it needs no slot on either stack. `x86_trap_body` returns its bool in `al`, which
# nothing here touches.
# CFI: an ordinary function (`call`ed, `ret`urns), with the same interrupt-stack-swap wrinkle
# `switch_to` and the other two ports' `dispatch_on_interrupt_stack` have. Restated in terms of
# `rbp` rather than `rsp` right after `mov rbp, rsp`, unconditionally, BEFORE the branch that
# decides whether `rsp` actually moves: `rbp` holds the correct value on both paths from that point
# on, so one restatement covers the swap-taken and swap-skipped cases alike.
.global dispatch_on_interrupt_stack
.type dispatch_on_interrupt_stack, @function
dispatch_on_interrupt_stack:
    .cfi_startproc
    .cfi_def_cfa_offset 8
    push rbp
    .cfi_def_cfa_offset 16
    .cfi_offset rbp, -16
    mov rbp, rsp
    .cfi_def_cfa_register rbp
    test rsi, rsi
    jz 4f                           # 0: stay here (from ring 3, pre-init, or nesting)
    mov rsp, rsi
4:  call x86_trap_body
    mov rsp, rbp                    # back to the interrupted stack BEFORE anything can switch away
    .cfi_def_cfa_register rsp
    pop rbp
    .cfi_restore rbp
    .cfi_def_cfa_offset 8
    ret
    .cfi_endproc
.size dispatch_on_interrupt_stack, . - dispatch_on_interrupt_stack

# ---------------------------------------------------------------------------------------------
# The paranoid path: vectors 2 (NMI), 8 (double fault) and 18 (machine check), each on its own IST
# stack. This file's header says why they cannot decide from CS; ist.rs says why they have stacks of
# their own and why an NMI never nests on its stack.
#
# The frame is isr_common's, so `x86_paranoid_dispatch` reads the same `TrapFrame`. The difference
# is the GS base. The IST entry sits 16 bytes below the top of the stack, and the CPU aligned to 16
# (it already was), so after the hardware's five words, the error code, the vector and fifteen
# registers (22 words, 176 bytes) the word at `rsp + 176` is the stack's top word: this core's
# per-CPU pointer, written by `ist::install`.
#
# `rbx` holds the interrupted GS base across the call because it is callee-saved; its own value is
# already in the frame. `rcx`, `rax` and `rdx` are `rdmsr`/`wrmsr`'s operands and are saved too.
# ---------------------------------------------------------------------------------------------
.type isr_paranoid, @function
isr_paranoid:
    .cfi_startproc
    .cfi_signal_frame
    .cfi_def_cfa_offset 0
    .cfi_offset 16, 16              # rip: the interrupted PC
    .cfi_offset 7, 40                # rsp: the interrupted stack pointer
    PUSH_GPRS

    mov ecx, 0xC0000101             # IA32_GS_BASE
    rdmsr
    shl rdx, 32
    or rax, rdx
    mov rbx, rax                    # the GS base we found, given back on the way out
    cmp rbx, [rsp + 176]
    je 1f
    mov rax, [rsp + 176]            # this core's per-CPU pointer
    mov rdx, rax
    shr rdx, 32
    wrmsr
1:

    cld
    mov rdi, rsp                    # 16-byte aligned: the IST entry is, and the frame is 11 x 16
    call x86_paranoid_dispatch

    cmp rbx, [rsp + 176]
    je 2f
    mov ecx, 0xC0000101             # IA32_GS_BASE
    mov rax, rbx
    mov rdx, rbx
    shr rdx, 32
    wrmsr
2:

    POP_GPRS
    add rsp, 16                     # discard the vector number and the error code
    iretq
    .cfi_endproc
.size isr_paranoid, . - isr_paranoid

# ---------------------------------------------------------------------------------------------
# The first entry to ring 3.
# ---------------------------------------------------------------------------------------------

# void user_return(TrapFrame *frame) -> !
#
# Load `frame` and `iretq` into it. The arch contract's first-entry-to-user path, reached only
# through `enter_user` in exceptions.rs, which is `#[inline(always)]` for the reason its RISC-V twin
# is: the frame sits at the top of this same kernel stack, so a call frame pushed here could land on
# top of it.
#
# `cli` first, for the exit-window reason in this file's header: this is the one path into
# `isr_restore` that ordinary kernel code jumps to with interrupts possibly enabled.
# CFI: reached by an ordinary `call`, so rip's return column is real on entry, but `mov rsp, rdi`
# immediately points rsp at a caller-fabricated TrapFrame, and this never returns to that caller
# (it falls into isr_restore's own `iretq`). Treated conservatively as `.cfi_undefined rip` for the
# whole function; see aarch64's `enter_userspace` (vectors.s) for the one-instruction-window
# reasoning this is skipping.
.global user_return
.type user_return, @function
user_return:
    .cfi_startproc
    .cfi_undefined rip
    cli
    mov rsp, rdi
    jmp isr_restore
    .cfi_endproc
.size user_return, . - user_return

# ---------------------------------------------------------------------------------------------
# The table of stub addresses, so `exceptions::initialize` can fill the IDT from a loop in Rust rather
# than needing 256 `extern` declarations.
# ---------------------------------------------------------------------------------------------
.macro ISR_ADDR num
    .quad isr_\num
.endm

.section .rodata
.align 8
.global ISR_STUBS
ISR_STUBS:
.set vec, 0
.rept 256
    ISR_ADDR %vec
    .set vec, vec + 1
.endr
