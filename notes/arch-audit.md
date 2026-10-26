# Auditing the hand-written architecture assembly

Done 2026-07-29, prompted by the exception-return race that milestone 22 phase B.2 turned up (the
full account of that bug is in [exceptions.md](exceptions.md); the fix is commit `9368657`). That
bug was a privilege escalation reachable from a normal boot, it failed about one suite run in four,
and it had been sitting in `RESTORE_CONTEXT` since milestone 7. So the question this note answers is
not "was that bug fixed" but "how many more of its siblings are there?"

## Why this is the code that needs auditing by hand

The Kani proofs cover the pure-logic crates: `paging`, `page_frames`, `elf`, `capability`, the
allocators, the ring validators. That is deliberate and it is most of the interesting logic. But it
leaves the hand-written architecture assembly as the least-verified code in the trusted computing
base, and milestone 20 (design/roadmap/0020-portable-hal.md) says so out loud. There is no tool in this project that
can prove `vectors.s` or `trap.s`.

That matters more than the line count suggests, because the assembly is where the invariants are
*hardware* invariants: single-copy registers, masking state, translation regimes. A Rust type error
is caught; a two-instruction window in which `SPSR_EL1` holds one thing and `ELR_EL1` holds another
is caught by nobody. The whole arch tree is about 6,200 lines. An audit by reading is affordable, and
in the absence of a prover it is the compensating control. This note is that audit, so the reasoning
survives and the next person does not have to redo it from scratch.

## The bug class, stated generally

> Kernel code that stages state in single-copy hardware registers, or in any per-CPU location that
> the exception path itself clobbers, across more than one instruction, while an exception,
> interrupt, or preemption can land in the middle.

The archetype, and what made the original bug rare enough to hide: a path that enters a
save/restore sequence from ordinary kernel code with interrupts enabled, rather than from a trap
that had already masked them. On a normal trap return the window is closed for free, because taking
the exception set `PSTATE.DAIF` (or cleared `sstatus.SIE`). It is the *first-entry* paths,
`enter_userspace` and `user_return`, that branch into the restore with interrupts live.

Which is why the interesting question for every candidate below is (d): is it reachable, or is it a
window that exists in the instruction stream and that nothing can ever land in?

## What was audited

Everything under `kernel/src/arch/`, read in full, both ISAs:

| File | aarch64 | riscv64 |
|---|---|---|
| Exception/trap vector and restore | `vectors.s` | `trap.s` |
| Trap frame, dispatch, fault handling | `exceptions.rs` | `exceptions.rs` |
| Context switch and the two trampolines | `context.s`, `context.rs` | `context.s`, `context.rs` |
| Boot and secondary bring-up | `boot.s`, `image_header.s` | `boot.s` |
| MMU, TTBR/SATP, ASID, TLB | `mmu.rs` | `mmu.rs` |
| Interrupt masking primitives | `interrupts.rs` | `interrupts.rs` |
| Interrupt-controller adapter | `interrupt_controller.rs` | `interrupt_controller.rs` |
| Timer | `timer.rs` | `timer.rs` |
| Per-CPU pointer, SMP calls, barriers | `mod.rs` | `mod.rs` |
| IOMMU / DMA domains | `iommu.rs` | `iommu.rs` |
| Host exit | `semihosting.rs` | `semihosting.rs` |

Plus the code on the other side of the seam that the class reaches into: `drivers/gic.rs`,
`drivers/plic.rs`, the switch path in `sched.rs` (`schedule`, `finish_switch`), `cpu.rs`'s per-CPU
plumbing, `smp.rs`'s `secondary_main`, and `user.rs`'s trap-frame placement (read only; milestone 29
owns that file).

Two structural facts fell out of the sweep and are worth recording because they bound the whole
search:

- There is exactly one `eret` in the kernel and exactly one `sret`, both inside the restore
  sequence the fix hardened. There is no second way out of the kernel to audit.
- There are exactly two places that fabricate a trap frame, `TrapFrame::for_user_entry` on each
  ISA. Every other frame is written by the trap entry itself.

## Findings

### 1. The RISC-V interrupt mask in `trap_return` is not self-sufficient (safe today, hardened)

(a) The window. `trap_return` masks interrupts with `csrci sstatus, 2`, then stages `sepc`, then
executes `csrw sstatus, t0` with the frame's saved status word, then ~30 loads, then `sret`.

(b) What can land in it. On RISC-V, `sstatus` is one register holding *both* the staged
return fields (`SPP`, `SPIE`) and the **live** interrupt-enable bit (`SIE`). So `csrw sstatus, t0`
writes the frame's `SIE` into the live bit. If a frame carried `SIE = 1`, interrupts would be back on
for the ~32 instructions before the `sret`, with `sepc` already staged, and any interrupt in there
would clobber `sepc` and `sstatus` exactly as in the original bug. The `csrci` would have bought
nothing.

(c) The corrupted state. Identical to the original: the nested handler's own `trap_return`
restores the S-mode values, our `sret` then runs with `sepc` pointing into the middle of
`trap_return` and `SPP = 1`, so it returns to S-mode at that address and spins there. A U-mode thread
never enters U-mode at all.

(d) Reachable? No, and that is the finding. It is closed by an invariant, not by the
instruction: every trap frame carries `SIE = 0`. Real traps get it from the hardware, which clears
`SIE` on trap entry (moving it to `SPIE`) before `trap_entry` reads the register. The one fabricated
frame, `TrapFrame::for_user_entry`, composes `SPIE | UXL_64` and happens not to set `SIE`.

So the code is correct. What was wrong was the **record**: the `RESTORE_CONTEXT` comment says the
mask lives in the macro "so that any future path reaching here with interrupts on is covered by
construction," and `trap_return`'s comment points at it. That claim is true on aarch64, where the
staged `SPSR_EL1` is a physically different register from the live PSTATE and nothing before the
`eret` can re-enable interrupts. It is false on RISC-V, and a comment that invites you to read it
across the two ISAs is worse than no comment, because the next person to add a frame-fabrication
site would trust it.

Fixed by making the invariant checked rather than hoped for. A `const _: () = assert!(...)` next to
`for_user_entry` fails the build if a fabricated frame ever sets `SIE`. A comment in
`trap.s` states the invariant, names its two sources, and says plainly that the aarch64
"by construction" claim does not transfer. Zero instructions added; nothing on the hot path moved.

The general lesson generalizes past this file, and it is the one worth keeping: when the same bug
is fixed on two architectures, check whether the fix is load-bearing for the same reason on both.
Here it was a complete fix on one ISA and a partial one on the other, and the symmetry of the two
patches concealed that.

### 2. RISC-V `trap_entry` parks a user-controlled value in `sscratch` for ~50 instructions

(a) The window. `trap_entry` opens with `csrrw t0, sscratch, t0`: `t0` becomes `&TrapStash` and
`sscratch` is left holding the interrupted `t0`. It stays that way for the whole frame build,
about fifty instructions, until the `csrw sscratch, t0` that restores it.

(b) What can land in it. Not an interrupt: the hardware cleared `SIE` on the way in. Only a
*synchronous* fault, and the only faultable instructions in the window are the ~35 stores building
the frame on the kernel stack. So: a kernel-stack overflow, or a bad `stash.kernel_sp`.

(c) The corrupted state, and this is the part worth knowing. The nested `trap_entry` executes its
own `csrrw t0, sscratch, t0` and gets the interrupted `t0` as its `&TrapStash`. It then does
`sd t1, 16(t0)` and `sd sp, 24(t0)`. If the outer trap came from U-mode, `t0` is a register the user
program chose, so the kernel performs two 8-byte stores to a **user-chosen address**. That is a
memory-safety failure mode, not merely a lost register.

**(d) Reachable?** Only behind a kernel-stack overflow, which is already a fatal kernel bug and which
the guard page is there to catch. A user program cannot choose the kernel's stack depth at the moment
it traps; it would need the kernel to be independently near the end of a stack. So: not reachable
from userspace on its own, and correctly classified as a double-fault hardening issue rather than
a live vulnerability. Recorded, not fixed, because the fix is a protocol change rather than a closed
window. Options, if it is ever worth doing:

1. Park the interrupted `t0` in the stash instead of in `sscratch`. Add a `scratch2` word and
   restore `sscratch` after four instructions, none of which can fault (they all target the
   always-mapped stash), instead of after fifty. Costs one extra store and one extra load per trap on
   the hottest path in the kernel, so it would move both icount baselines.
2. A separate per-hart trap stack, entered when `SPP = 0`, which is what a kernel that wants to
   survive its own stack overflow eventually needs anyway. Larger, and it belongs with whatever
   milestone takes stack-overflow recovery seriously rather than being smuggled in here.
3. Leave it, and say so, which is what this note does. aarch64 has the same class of problem in
   the same place (a fault inside `SAVE_CONTEXT` loses `ELR_EL1`/`SPSR_EL1` before they reach the
   frame) and it is unrecoverable there too; every kernel has a double-fault story, and ours is
   currently "report and halt."

### 3. The PLIC's enable bits are a lock-free read-modify-write over shared MMIO (CLOSED)

Not a state-staging bug, but it is the interrupt-controller interleaving the audit went looking for,
and it is a parity gap.

(a) The window. `drivers/plic.rs`:

```rust
pub fn enable(source: u32, context: usize)  { ... write(word, read(word) | bit); }
pub fn disable(source: u32, context: usize) { ... write(word, read(word) & !bit); }
```

Two MMIO accesses, no lock. `word` is one 32-bit register carrying the enable bits for 32 sources
of one PLIC context. These are the only two MMIO read-modify-write sites in the driver and arch tree.

(b) What can land in it. Another hart executing `enable` or `disable` for a *different* source in
the same 32-source group on the same context. `arch::interrupt_controller::enable` is called from kernel-thread
context with interrupts **enabled** (the driver-spawn paths in `user.rs`), while
`plic::disable(source, ctx)` is called from another hart's external-interrupt handler. Nothing
serializes them.

(c) The corrupted state. A lost update, either direction. A lost *enable* leaves a device source
masked forever, so its driver blocks on an interrupt that never arrives. A lost *disable* leaves a
level-triggered source enabled after its handler masked it, so the line re-fires the instant
interrupts reopen and that hart drowns in an interrupt storm. Both are liveness failures; neither is
a privilege or memory-safety failure.

(d) Reachable? Yes. `target_context` spreads sources round-robin over the online harts, and with
up to nine sources (virtio-mmio 1..8, the UART at 10) over four harts several sources share a context
and therefore share the enable word. The window is two MMIO accesses wide and the enables happen a
handful of times per boot, which is consistent with never having observed it.

**Why aarch64 does not have this.** The GIC's `ISENABLER` / `ICENABLER` are write-1-to-set and
write-1-to-clear, so enabling one line is a single store that cannot disturb its neighbors: the
architecture gives you atomicity for free. And `drivers/gic.rs` takes a lock on top of that anyway.
The PLIC has plain read/write enable bits, which forces the read-modify-write, and the RISC-V driver
never grew the lock the GIC has. Under rule 5 that asymmetry is the bug, independent of how likely it
is to fire.

#### Closed

Reported first, then fixed on review, as execution inside decided architecture rather than a design
fork. §9 (locking: IrqSafeMutex, plus a discipline) already establishes `InterruptSafeMutex` plus a rank order. `drivers/gic.rs` already takes a lock
for this exact operation, and 16b added `rank::IOMMU` without escalating, so a rank is precedented.

`enable` and `disable` now share one helper that holds an `InterruptSafeMutex` across the read-modify-write,
at rank `rank::INTERRUPT_CONTROLLER`. It must be interrupt-safe, because `disable`
runs in the handler and a plain spinlock would deadlock one hart against itself. Both icount
baselines are byte-identical, so nothing hot was locked. No test can catch the race itself; the tests
pin the neighbor-preserving read-modify-write and the irqsave behavior instead. Why the lock is that
kind, at that rank and scoped to that register, and what each test proves, is in
[`arch-audit/plic-lock.md`](arch-audit/plic-lock.md).

## Candidates cleared, and why each is safe

Recorded because "we looked and it is fine" is the other half of an audit, and because each of these
is a place a future change could break something. The full reasoning for each is in
[`arch-audit/candidates-cleared.md`](arch-audit/candidates-cleared.md). What it found, candidate by
candidate:

- aarch64 `RESTORE_CONTEXT` after the fix is safe by construction. Nothing between `msr daifset`
  and the `eret` can alter live PSTATE. This is the claim finding 1 shows does *not* hold on RISC-V.
- Nothing stages `SPSR_EL1`/`ELR_EL1` or `sepc`/`sstatus` outside the restore macro, and `SP_EL0`
  is dead to a kernel running at EL1 with `SPSel = 1`.
- The context switch on both ISAs has no window: `schedule()` masks interrupts across the decision
  and the switch, and the trampolines deliberately do not unmask before `finish_switch`.
- The RISC-V `tp` migration hazard, already fixed, was verified. Nothing else round-trips a
  hart-local value through a saved frame.
- `set_percpu` on RISC-V has a real window that is unreachable, because it runs before the hart can
  take a trap at all. That precondition is stronger than the one `cpu.rs` documents. CPU hotplug
  would make it reachable and catastrophic.
- MMU enable, the TTBR/SATP switches and every barrier after a system-register write were checked,
  and each is present or legitimately unneeded.
- `PAR_EL1` in `translate_as_el0` was already masked, with a comment naming this class: the
  discipline predates the bug.
- GIC acknowledge/EOI and PLIC claim/complete cannot cross cores, the timers touch only per-core or
  M-mode state, secondary bring-up runs masked, and the cross-core inbox is a real `InterruptSafeMutex`.

## The honest summary

Three findings from a full read of both ISAs' arch trees, and their dispositions:

| | What | Disposition |
|---|---|---|
| 1 | The RISC-V mask was safe by an unstated invariant, not by construction, and the comment claimed otherwise | **Fixed**, and the invariant is now a compile-time assertion |
| 2 | `trap_entry` leaves a user-controlled value in `sscratch` across the faultable frame stores | **Left documented**, on review: paying hot-path cost to harden behind a kernel-stack overflow that is already fatal is a bad trade |
| 3 | The PLIC's enable-bit read-modify-write is unserialized, a reachable liveness bug and a parity gap | **Fixed** on review, with an `InterruptSafeMutex` at the GIC's rank |

Nothing found was a live privilege or memory-safety hole; the two that were are the two already
fixed. Finding 3 is the one worth remembering for its shape rather than its severity. It was not a
missing barrier or a mis-ordered instruction. It was **one ISA quietly getting a guarantee from its
hardware that the other does not**, and the RISC-V driver having been written as if it did. That is
the same shape as finding 1, and both are the shape rule 5 exists to catch.

## BUGS: this audit covers two of the three architectures

Added 2026-08-27 by the sweep in [architecture-list-sweep.md](architecture-list-sweep.md), which
went looking for exactly this shape and found it here.

**Everything above is aarch64 and riscv64. `kernel/src/arch/x86_64/` has never been read this way.**
The scope table has two columns because there were two architectures when this was written
(2026-07-29); milestone 161 brought up the third and this note was not revisited. The size is the
part that should decide how urgent it is: that tree is 18 files and 6,797 lines, larger than the
roughly 6,200 lines this audit read in full across both other ISAs. It carries its own `trap.s`,
`context.s`, `boot.s`, an AP bring-up path, a segment and TSS layer, and a VT-d driver, none of
which has an analogue that was covered here.

That matters more than an ordinary coverage gap because of this note's own argument. Hand-written
architecture assembly is the least-verified code in the trusted computing base. No tool in this
project can prove it, and an audit by reading is the compensating control. On a third of the arch
tree there is currently neither.

The audit cadence (`script/audits`, `.github/workflows/audit-cadence.yml`) will not raise this. It
counts audits against elapsed time and shipped components; it has no notion of an architecture, so
a whole unaudited ISA reads to it as a tree in good standing.

Recorded rather than fixed here, since reading 6,797 lines is its own lane. It is item 8 of the
sweep's table and is folded into the milestone proposed there.

## The original audit's limit

That is a reassuring result, and it should be read with its limit attached: an audit by reading
finds what the reader thinks to look for. The original bug was found by a failure, not by
inspection, and it had survived every previous reading of that file. Until something can prove
assembly, the honest position is that `vectors.s` and `trap.s` are trusted rather than verified, and
notes like this one are how we pay for that.

---

*See also [exceptions.md](exceptions.md) for the bug that prompted this, [threads.md](threads.md) for
the switch-path invariants, [locking.md](locking.md) and DECISIONS §9 for the masking discipline, and
[verification.md](verification.md) for what the proofs do and do not cover.*
