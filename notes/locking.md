# Locking

## The deadlock, and why one core doesn't save you

A plain spinlock in a kernel that takes interrupts is a hang waiting for a schedule.

```
  kernel code:  ALLOCATOR.lock()   <- acquired
                ...working...
       ⚡ TIMER INTERRUPT
  handler:      ALLOCATOR.lock()   <- spins
                                      spins waiting for a lock that only the code it
                                      interrupted can release, and that code cannot run
                                      until the handler returns.
```

This is not a race. It is not "sometimes, under load." It is a *deterministic* hang the
moment the timing lines up, and it will look exactly like the mystery in
[stack.md](stack.md) that cost us two hours.

And note what it does not need: a second core. A single-core design would not protect against it at
all. One core, one lock, one interrupt, dead machine. More cores add races; they do not remove this
one.

## The fix

Mask interrupts for as long as the lock is held. The interrupt cannot fire, so it
cannot try to take the lock. Linux calls this `spin_lock_irqsave`; ours is `IrqSafeMutex`
in `kernel/src/sync.rs`.

On aarch64 that means the `I` bit of `PSTATE.DAIF`:

| Bit | Name | Masks |
|---|---|---|
| 9 | `D` | Debug exceptions |
| 8 | `A` | SError |
| **7** | **`I`** | **IRQ** |
| 6 | `F` | FIQ |

Set the bit to *mask*, clear it to *unmask*. Note the polarity: `1` means "masked," which
is the opposite of how most people read a flag called `I`.

`msr daifset, #2` masks IRQs; `msr daifclr, #2` unmasks them. Single instructions that
touch only the bits you name, so there is no read-modify-write window to lose a race in.

## Two orderings that are the entire point

Acquire: mask interrupts FIRST, then take the lock.

```rust
let irqs_were_enabled = interrupts::disable();   // <- first
let guard = self.inner.lock();                   // <- second
```

The other order leaves a window where you hold the lock with interrupts still live. That
window is one instruction wide. It is also the exact deadlock.

Release: drop the lock FIRST, then restore interrupts.

```rust
unsafe { ManuallyDrop::drop(&mut self.guard) };  // <- first
interrupts::restore(self.irqs_were_enabled);     // <- second
```

Same window, arrived at from the other side. An interrupt fires, the handler wants the
lock, and you still hold it.

Both windows are a couple of instructions. Both are fatal. Both work perfectly in testing
for months.

## Restore is not the same as enable

`IrqSafeGuard::drop` restores the interrupt state that was in effect when the lock was
taken. It does not simply enable interrupts.

The difference bites when a lock is taken inside a context that *already* had interrupts
masked: an interrupt handler, or inside an outer lock. Blindly enabling on release would
unmask interrupts inside an interrupt handler, and the resulting fault is one you will
not enjoy explaining.

This is precisely why Linux's is called `irqsave`/`irqrestore` and not `irqoff`/`irqon`,
and it is the single easiest thing to get wrong here. There is a test named
`irq_safe_mutex_restores_rather_than_enables` whose only job is to catch it, and it was
verified against a deliberately broken `restore()`.

## Why we didn't build per-CPU reserves

Considered seriously, and it turned out to be an answer to a different question.

Per-CPU page caches (Linux's PCP lists, slab's per-CPU caches) exist for scalability
(on 64 cores, one global lock is a catastrophe) and cache locality (a frame this core
just freed is warm in this core's L1). They are not an interrupt-safety mechanism, and
Linux still wraps them in `local_irq_save`. Same core, same structure, same deadlock.

They belong to the SMP conversation, where the problem is lock *contention*, not deadlock.

## Why masking is genuinely sufficient

Not a compromise. Walk the handlers we will actually have:

| Handler | Milestone | Allocates? |
|---|---|---|
| Timer tick | 5 | No. Bump a counter, flag "reschedule wanted." |
| UART receive | 5 | No. Push a byte into a pre-allocated ring. |
| virtio completion | 8 | No. Mark an I/O done, wake a thread. |
| **Page fault** | 4, 7 | **Yes.** Demand paging, copy-on-write. |

So the whole question is the page fault. And a page fault is synchronous: taken from the
exact instruction that touched the bad address, on behalf of that context. Which gives the
rule real kernels use:

> **Kernel memory is never demand-paged.** Kernel pages are mapped eagerly. A page fault
> taken from **EL1 is a bug** and is fatal (already true in our `fatal()`).

Then every allocating fault comes from EL0, and userspace was holding no kernel locks,
because it *cannot*. There is nothing to deadlock against, and no reserve pool is needed.

## The panic path must be able to break the lock

If we fault in the middle of a `println!`, the fault handler's own attempt to print takes
the console lock again and hangs. We lose the one message that mattered, at the exact
moment we needed it.

So the panic handler and the fatal exception path both call `console::force_unlock()`
first. Output may be spliced. That is a fine price for getting the message out at all.

Linux does the same thing and calls it `bust_spinlocks`.

`force_unlock` is `unsafe` for a real reason: it means "whatever the previous holder was
doing is now half-finished, and its data may be inconsistent." Acceptable when the
alternative is a silent hang. Unacceptable at literally any other time.

## The ordering rule, enforced

The other deadlock, and the nastier one: AB-BA. Thread 1 takes lock A then wants B; thread
2 takes B then wants A. Neither can proceed. Unlike the interrupt deadlock, this one is a
*real race*: it needs the timing to line up, so it passes tests for months.

We wrote "define a global order and always take them in it," and then relied on remembering.

Now every lock carries a rank, and `lock()` asserts:

> **You may only acquire a lock strictly LOWER than everything you currently hold.**

The ranks in force, and why each sits where it does, are in [the ranks](#the-ranks-and-why-each-sits-where-it-does)
below.

If every acquisition strictly decreases the rank, a cycle is unrepresentable. Not unlikely.
Impossible. Look at [deadlock.md](deadlock.md): this destroys condition 4, circular wait,
outright.

That makes it prevention, not detection. Linux's `lockdep` builds a dependency graph at
runtime and hunts for cycles: powerful, and expensive. Ranking costs three instructions and
*cannot be wrong*. FreeBSD (WITNESS) and Solaris use the same mechanism.

Two locks at the same rank may never nest (`R < R` is false), which is exactly right: equal
rank means we have declared no order between them, so nesting them would be choosing one at
random.

### A design it would have caught

`memory::ram_regions()` used to return an iterator that held the RAM lock while the caller
iterated. `mmu::map_everything` iterates it and *allocates frames inside the loop*, so it
would have held RAM (30) while taking FRAMES (30), and `30 < 30` is false. The ranking would
have failed it on the spot. (We happened to fix it for other reasons first, which is not a
system.)

### The panic path must reset it

If we panic while holding the console lock, `HELD_RANK` is 10, and the panic handler's own
print would try to take rank 10 again. `10 < 10` is false, so the ranking would fire a
violation inside the panic handler, and we'd lose the original message to a recursive
panic.

So the panic path calls `sync::force_reset_ranks()` alongside `console::force_unlock()`.

**The bookkeeping is a debugging aid. It must never be the thing that stops us saying what went
wrong.**

## The ranks, and why each sits where it does

`kernel/src/sync.rs`'s `rank` module holds the numbers. A lock's rank is set by what it must be
able to take while held and by what holds it.

| rank | lock | why there |
|---|---|---|
| 61 | `ADDRESS_SPACES` | A `MAP_INTO` holds it while drawing page tables from the space's region and writing the revocation record. It never takes `IPC_TABLES`; capability grants happen after release. |
| 60 | `IPC_TABLES` | Above everything that frees, because the reaper drops a dead thread's kernel stack while holding it. Nothing under it allocates (intrusive queues, a static TCB pool), so `schedule()` is safe from the timer interrupt. |
| 59 | `INBOX` | Placing a thread on a remote core happens under `IPC_TABLES`. A core locks at most one inbox at a time, the target's. |
| 59 | `KMEM` | It carves and retypes from its region while held, and a kernel stack's `new` and `Drop` run from spawn and the reaper, under `IPC_TABLES`. |
| 59 | `MAPPINGS` | Recording a mapping retypes a log page from the payer's region while held. Never held with `IPC_TABLES` in either order: the capability sweep and the unmap sweep run one after the other, and the reaper drops address spaces outside `IPC_TABLES` (`finish_switch`). |
| 58 | `MEMORY_REGION` | Below the 59s, and below `IPC_TABLES` so a syscall with no thread-table business can take it alone. |
| 57 | `CAPABILITY_TABLE` | One lock per thread, in its TCB page. The order is below. |
| 56 | `VIRTIO` | Taken from a syscall with no other lock held. The rank records where that syscall sits, not a nesting need. |
| 55 | `STACK_VA` | A `KernelStack`'s `Drop` runs from the reaper, under `IPC_TABLES`. |
| 54 | `IOMMU` | A leaf. The DMA domain's page-table frames are allocated before it is taken, so it is never held across an allocation. Below `VIRTIO` because a PCI device is confined from the bring-up path that registers its transport, though the two never nest. |
| 45 | `KERNEL_MMU` | Below `IPC_TABLES` (a stack's `Drop` unmaps under the reaper) and `STACK_VA` (a stack's `new` maps pages), above the allocators (mapping allocates table frames). |
| 30 | `PAGE_FRAMES`, `RAM` | The physical memory map. |
| 20 | `IRQ_CONTROLLER` | The IRQ handler takes it and may print a diagnostic while holding it. |
| 15 | `ASIDS` | Taken alone at address-space creation and teardown. |
| 11 to 13 | `RECLAIM_REFUSAL`, `ISA`, `ACPI` | Leaves above the console, because a reader may print while holding one. |
| 10 | `CONSOLE` | The leaf. A panic prints while holding any lock. |

The three locks at 59 are never nested with one another. Inbox traffic is scheduling, the mapping
registry is user bookkeeping, and `KMEM` is the kernel buying its own pages. Equal rank refuses the
nesting, so this is checked rather than hoped.

`IRQ_CONTROLLER` is one rank for two drivers. The GIC on aarch64 and the PLIC on RISC-V are
compiled for their own ISA only, so they never coexist and need no order between them. `x86_64`
never takes it: the local APIC's claim is one register read with no shared state.

The capability table's order is what makes a capability lookup cheap:

- The running thread's own lookups (`sched::current_cap`, `grant`, `grant_at`) take its table's
  lock alone, without `IPC_TABLES`. Taking the global lock there made 41% of lookups wait at four
  busy cores on radon ([job-mix/null-syscall-under-load.md](job-mix/null-syscall-under-load.md)).
- Anything that touches another thread's table (an IPC delivering a capability, `CAP_INSERT`, the
  revocation sweeps) takes it under `IPC_TABLES`. That keeps a capability in transit, or a sweep
  across every table, atomic against every other cross-table operation.
- A holder takes nothing. The table is a fixed array of `Copy` entries, so nothing on it
  allocates, frees or prints. Two tables never nest (equal rank), and `IPC_TABLES` is never taken
  under one (60 > 57), so an AB-BA between a lookup and a sweep is unrepresentable.

It sits below `MEMORY_REGION` and the 59s not because anything nests them today, but so a holder
of one of those could still look a capability up. It sits above everything a holder might reach on
a panic path, which is only `CONSOLE`.

## Why the lock path carries inline hints

`IrqSafeMutex::lock` is `#[inline]`. Unmarked, the generic is emitted once and can inline only in
the codegen unit that owns it. Which unit that is depends on a metadata hash Cargo derives partly
from the host triple. One commit then built two aarch64 kernels 25 KiB of text apart, and
`script/fastpath-footprint` reported a shrink that was only the hash. With the hint each call site
gets the same cost decision on every host, checked by building under two metadata seeds that
split the old way. `#[inline(always)]` was refused there: it pushed
`memory::init`'s frame past the 4 KiB guard page.

`lock_masked`, the half after masking, is `#[inline(always)]` so `lock` is one call in the gate's
-O0 build. Split out as a second frame, it cost a riscv64 yield round trip 56 instructions.

The guard's `drop` is `#[inline(always)]`, because every IPC drops one on the hot path and it must
land in `.text.hot` with its caller. Left to LLVM it was outlined on CI builds of all three ISAs
and refused by `script/fastpath-footprint`, while a macOS build of the same commit inlined it. A
local pass of that gate does not show the outlining.

The lock-order panic is out of line and `#[cold]`. Inline, its formatted message cost about 50 bytes
at every inlined `lock()` site, which made it one of the largest single lines in the fastpath
closures on all three ISAs.

## The rules

See [DECISIONS](../design/decisions/0009-irq-safe-locking.md) §9 for the full table. The short version:

1. All kernel locks are `IrqSafeMutex`.
2. Mask, then lock. Unlock, then restore.
3. Restore, never blindly enable.
4. Keep critical sections short: interrupts are off for the whole of it.
5. Never allocate, block, or `wfi` while holding a lock.
6. Two locks? Define a global order and always take them in it. Otherwise AB-BA deadlock,
   which is a *real* race and far nastier than the one this note is about.
7. Interrupt handlers record and defer. They do not do work.

---

*Add to this file as new locking concerns come up.*
