# Milestone 447 appendix: what RISC-V cost, and the boot path it exposed

An appendix to [milestone 447 (a thread's vector registers are its
own)](../0447-vector-registers-are-a-thread-s-own.md). The block is complete without it. It holds
the working behind the block's RISC-V summary.

Of the three rows in the block's [mechanism
table](../0447-vector-registers-are-a-thread-s-own.md#three-architectures-three-mechanisms-one-rule),
two cost this milestone real time. Both are written down where the next reader meets them, in the
code, and not only here.

## RISC-V does not tell you, so this retries rather than decodes

`sstatus.FS` is the best control register of the three. It is a four-state machine (Off / Initial /
Clean / Dirty) with a hardware-maintained dirty bit, designed by people who had context switching
in mind. What it does not do is say why an instruction was illegal. An FP instruction under
`FS == Off` arrives as `scause` 2, exactly like a bad opcode. `stval` is permitted to be zero for
that cause, and is on some parts.

The handler therefore does not decode the instruction; it retries it. `FS == Off` in the frame is
the whole guard: open the unit, leave `sepc` where it is, and let the `sret` re-execute whatever
trapped. A genuinely illegal instruction traps a second time with `FS` no longer Off. It fails the
guard and falls through to the fault it deserved. The cost of being wrong is one extra trap on a
path that is already killing a thread.

The alternative was reading user memory at `sepc`, with `SUM` set and a page fault to handle. It
would decode seven major opcodes plus the compressed forms, to answer a question the retry answers
for free.

## And `sstatus` travels in the trap frame, which the other two do not

`TrapFrame::sstatus` is captured on trap entry. `trap.s` writes it back with `csrw sstatus, t0` on
the way out. A handler that set `FS` in the live CSR and returned therefore had its own write
undone by its own return. The first-use handler sets the frame's copy as well.

Every later switch is safe without thinking about it. `hand_over` sets the live CSR from the
incoming thread's `live`, and that thread's frame carries the same value for the same reason.

## What it found: one architecture's boot path does not run `arch::init`

`arch::fp::init` shuts the unit for a core before any thread exists. It does not live in
`arch::init`, the obvious home, because there it is green on aarch64 and x86_64 and silently wrong
on RISC-V.

`main`'s RISC-V tour installs `stvec` by calling `arch::exceptions::init()` directly. It reaches
`sched::init` and never passes through `arch::init` at all. OpenSBI hands the kernel a hart with
`sstatus.FS` already set. So no thread would take the first-use trap, `live` would stay false on
every thread, `hand_over` would take its early return every time, and two threads would share a
register file with nothing in the system saying so.

The answer is not a third call site. The invariant is about threads, so it lives where threads begin
to exist: `sched::init` and `sched::adopt_secondary_idle`. Both are portable, and both are on the
path of every core that will ever own a thread. That is AGENTS.md's ladder read downward. The
rung-four version (remember to call it from each architecture's bring-up) is what fails. The
property and the mechanism are in the same function.

`the_first_floating_point_instruction_takes_a_trap` guards this, and it does not merely

assert that FP works. It distinguishes three outcomes and names each one. "A brand-new thread
started with the unit already open" and "an FP instruction executed without opening the unit" are
different bugs with the same symptom.
