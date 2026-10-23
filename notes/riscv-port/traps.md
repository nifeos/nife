# The riscv64 trap path

An appendix to [notes/riscv-port.md](../riscv-port.md). It carries the design behind
`kernel/src/arch/riscv64/exceptions.rs` and `irq.rs`, the S-mode analog of aarch64's `VBAR` table
and `ESR` decode. The portable side is [notes/exceptions.md](../exceptions.md).

## One entry, and a frame trap.s fills

RISC-V has a single trap entry, `stvec`, not aarch64's 16-slot table. Interrupt versus exception is
the top bit of `scause`, and the syscall is the `ecall` cause. `stvec`'s low two bits are its MODE
field: 0 sends every trap to one handler, 1 sends interrupts to `base + 4*cause`. A 2-byte-aligned
entry would select vectored mode without a word of warning, so trap.s aligns `trap_entry` to 4 and a
test asserts both the base and the mode. aarch64's twin test checks `VBAR_EL1`'s 2,048-byte
alignment, the same class of rule at another scale.

`TrapFrame` is 288 bytes, and trap.s spells that number by hand in four places. A `const` assertion
holds the Rust side to it. The frame lives at `stack_top - 288` on first entry and on every trap
after it, as on aarch64, so `user_pc` can read where a wedged thread is for the watchdog dump. A
wrong offset in trap.s's save and restore scrambles a register while still returning to the right
address. It corrupts a caller's state and blames innocent code thousands of instructions later,
which `registers_survive_a_trap` exists to catch.

The portable syscall dispatcher names no register. It reads `a7` and `a0`..`a5` through
`TrapFrame::{syscall_nr, arg, set_arg}`, where aarch64 maps `x8` and `x0`..`x5` (DECISIONS §17).
`sepc` points at the `ecall`, which the hardware does not step past, so the handler adds 4.

## A fabricated frame for the first entry to U-mode

`TrapFrame::for_user_entry` builds the frame a new thread drops to U-mode through. Its `sstatus`
has SPP = 0 (return to U-mode), SPIE = 1 (interrupts on after `sret`, the analog of aarch64's
DAIF = 0, so a tight user loop stays preemptible) and UXL = 2. `trap_return` writes the whole word
into `sstatus`, so a missing UXL would clear the U-mode XLEN and fault on the first user
instruction.

SIE must be clear, and a `const` assertion holds it. `trap_return` masks interrupts on entry, for
the exception-return race in [notes/exceptions.md](../exceptions.md). RISC-V's `sstatus` holds the
live interrupt enable beside the staged fields, so a frame with SIE = 1 would re-enable interrupts
just after `sepc` is staged. A real trap cannot carry it, because the hardware clears SIE on entry.
aarch64 needs no such check: its staged `SPSR_EL1` is a different register from PSTATE.

`tp` is the thread's own thread pointer (§269 fork 4), zero for a thread nobody gave one. RISC-V's
`tp` is a general register, so the kernel's per-hart pointer must not ride into U-mode; trap.s
restores it from the per-hart stash through `sscratch` on the way in. The frame reads it while
still a value, because once the frame sits at the top of the kernel stack no call may run.

`enter_user` is `#[inline(always)]` for the same reason: a pushed call frame would overwrite the
trap frame before `user_return`'s `mv sp, a0`. It refuses `sepc == 0` with a load and a branch. A
thread dispatched there takes an instruction page fault and dies, everything waiting on it blocks,
and the run ends 60 s later in the lost-wakeup watchdog, far from the cause. CI produced that hang
in three tests on three CPU models and it never reproduced locally, so the check turns it into a
panic that names the thread.

## Dispatch

`riscv_trap_dispatch` is the outer half and stays on the interrupted stack. A U-mode trap runs on
its thread's kernel stack, because the syscall may block and an interrupt stack may not. A kernel
trap with an interrupt stack available moves there through `dispatch_on_interrupt_stack`. The
common case calls `riscv_trap_body` directly; on aarch64, routing every `ecall` through the
trampoline cost 8.6 instructions per `null_syscall`. Preemption happens after the body returns,
on the interrupted thread's stack, because `schedule()` may run only on a stack that thread owns
(`kernel/src/interrupt_stack.rs`). The switch returns through `trap_return` to exactly the
interrupted instruction (DECISIONS §5).

A device interrupt is claimed against this hart's own PLIC context, since affinity may route a
source to any hart. The claim is the mask until the routed driver's completion, and the source
stays enabled; `kernel/src/drivers/plic.rs` says why. A reschedule IPI pends `sip.SSIP`, which must
be cleared or it re-fires the moment interrupts reopen. One IPI carries both reasons a hart pokes
another (DECISIONS §28): a thread handed over, and a steal request.

## First use of the FP unit

An FP instruction under `sstatus.FS == Off` raises an ordinary illegal instruction. Nothing in
`scause` or `stval` separates it from a bad opcode, where aarch64 and x86 each have their own
exception. So the handler does not decode the instruction; it retries it. It opens the unit, leaves
`sepc` alone, and lets `sret` run the instruction again. A genuinely illegal instruction traps a
second time with `FS` on and falls through to the fault path. Being wrong costs one extra trap on a
path already killing a thread. Decoding would need the instruction from user memory, since `stval`
may be zero for this cause, and seven major opcodes plus the compressed forms.

The handler sets the frame's `FS`, not only the live CSR, because trap.s writes the frame's copy
back on the way out. `crate::fp::enable_for_current` has just loaded a register file, so the frame
gets Dirty. The arm serves S-mode too: milestone 447's concurrency proof runs as kernel threads,
because every userspace target is soft-float.

## Breakpoints and faults

A breakpoint from S-mode is the trap self-test: counted and stepped over, by 2 bytes for the
compressed `c.ebreak` and 4 otherwise. From U-mode it is a fault, because every userspace panic
handler ends in `ebreak` expecting to die. The self-test is also this ISA's S-mode witness. S-mode
cannot read its own privilege, but the breakpoint arm counts only when `sstatus.SPP == 1`, and in
M-mode OpenSBI's `mtvec` would own the trap. A count that rose is a machine executing in S-mode.

A user fault kills the thread and keeps the machine. `user_fault` classifies the fault first, while
the thread's space is still installed, then stores the record, then raises `USER_FAULTS` with
Release. Tests find the record by watching that counter rise and calling `last_user_fault`, whose
Acquire fence pairs with it. A counter raised first would hand a reader an earlier fault's record,
or on the boot's first fault a zero that reads as "nothing faulted".

`scause` has one page-fault code per access kind and no field saying why the walk refused: a
missing leaf and a leaf with `U` clear are the same `scause` 13. aarch64 gets the distinction in
`ESR_EL1`'s fault status code. Here `classify` walks the page table and reads "there is a
translation" as a permission fault. aarch64's answer is a measurement and RISC-V's is an inference.

A user page fault's kill report prints the live `satp` and the leaf at `stval`, and in
`system_tests` builds whether the root is the thread's own (`sched::thread_space_root` exists only
there). It is cold and out of line because `script/fastpath-footprint` counts the trap dispatcher
in `syscall_entry`. A new function in `sched` is not neutral either: one added for this report
moved the codegen-unit split and grew riscv64's `syscall_entry` from 2,010 to 2,604 bytes.

An exception from S-mode is a kernel bug and panics. It first says whether `stval` is a guard page,
using the interrupted `sp` from `x[2]`, since the live `sp` may be this hart's interrupt stack.

## The PLIC adapter

`irq.rs` is the arch side of the portable `arch::irq` surface, the twin of aarch64's GIC adapter,
and thin by design: `drivers::plic` does the work.

Each hart's S-mode PLIC context comes from the device tree's `interrupts-extended`, held in a
table indexed by hart id. `2*hart + 1` is QEMU `virt`'s layout and not a law. The JH7110's disabled
S7 core (hart 0) contributes only an M context, so there hart h's S context is `2h`. A kernel using
the formula programs its neighbor's context: enables, claims and completes all land one over, and
every external interrupt sits pending forever. A tree that does not state the layout leaves the
formula in force, and the boot prints a line only when the machine differs from it. The machine
description prints the context beside the PLIC address because it is the number that is not
architectural.

The boot hart's context is derived too, never fixed at 1: OpenSBI elects the boot hart by lottery,
and a kernel that programs hart 0's context while running on hart 3 leaves every interrupt pending
with all harts parked in `wfi`. The PLIC base is stored before `sie.SEIE` is set, so no claim goes
through a zero base, and every source stays disabled until `enable` routes it.

Sources spread round-robin over the online harts, the twin of the GIC's `ITARGETSR`. The k-th
online hart is chosen, not hart k: the VisionFive 2's online set is {1,2,3}, and an index would
route a source to parked hart 0. Each source's target is assigned once and kept, with the first
writer winning a race, so a claim and its completion always meet on one context. The boot hart
opens the target's threshold before the first source lands, which it can because threshold
registers are global MMIO. Every hart sets `SEIE` as it joins the scheduler, which delivers nothing
until a source is routed to it.

A claim's context is recorded until the driver's ACK, which completes on that context. A source can
be enabled on more than one context, because the hand-over pins the console's to the boot hart, and
only a context that enables a source may complete it. An ACK with no claim outstanding arms the
source at its target instead.

PCI functions on QEMU `virt` raise INTx through the standard swizzle onto PLIC inputs 32 to 35,
which the machine's device tree states and `crates/pci`'s fixture test holds the formula against.
So `alloc_msi_vector` answers `None` and `kernel/src/pci.rs` falls back to INTx. That is not "MSI is
unsupported on RISC-V": the AIA's IMSIC is the same mechanism, and a board with one would implement
it there. Nothing here needs it, and a routing path with no consumer cannot be known to work
([milestone 215](../../design/roadmap/0215-x86-64-pci-interrupt-routing.md) says why x86_64 differs).

No device here interrupts at bring-up without input or a disk, so the self-test's `deviceirq` check
reports itself absent. A device line is proved where one exists: the user-level input driver at the
hand-over, and the `blockdev` check's driver when a disk is attached.

## BUGS

- `classify` walks the tables after the fault, a few hundred cycles late. Another hart unmapping
  the page in that gap turns a permission fault into a translation fault, and a concurrent map does
  the reverse. Nothing unmaps a live user page today, and the tests fault a thread that owns its
  space alone, but no care here closes the gap.
