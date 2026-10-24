# The x86_64 TLB shootdown, and why it had to be an NMI

*(Milestone 161's SMP-crash lane, 2026-08-25. What a two-core kernel test suite was actually
crashing on, why the obvious mechanism deadlocks on this architecture, and the one property that
decides the design. The RISC-V twin of this note is notes/riscv-tlb-shootdown.md; read that one
first if you want the shape of the problem stated by the port that solved it earlier.)*

## The crash, and what it was really saying

`script/test --arch x86_64` at `NIFE_SMP=2` faulted every run. Two forms, both from the trap dump:

```
=== x86_64 trap: vector 13 (general protection fault) ===
  rip        : 0x5afe57ac5afe57ac   cs : 0x0008
```

and a page fault at `rip 0x0`. The first value is the tell, and it is not garbage: `0x5AFE57AC5AFE57AC`
is `stack::PAINT`, the pattern this kernel writes into a fresh kernel stack before anything real
occupies it (milestone 84's high-water instrument). A `ret` landing on paint means something read a
saved `Context` back from a stack location that was never written with one.

It reproduced 10 times in 10, which is the useful kind of bug.

## The cause was one function's own doc comment

`arch::x86_64::mmu::flush_tlb` invalidated the calling CPU's TLB and nothing else, because that is
what `invlpg` does. The comment above it, written when this port had one core, said so:

> a multi-CPU kernel needs a software shootdown protocol (an IPI), the same problem RISC-V solves
> with SBI RFENCE. There is one CPU here (roadmap item 5), so the local invalidate is the whole of
> it, and this is the line that will need company.

Item 5 landed a second core. The line never got its company. That is the whole bug, and the reason
it is worth writing down is that nothing failed when SMP arrived: the gap was invisible until a
workload actually placed threads on two cores.

**The other two architectures are not lucky, they are different.** aarch64 needs no protocol at all,
because `tlbi vaae1is` is broadcast across the inner-shareable domain by the hardware. RISC-V needs
one and has had it since milestone 58. So the same portable `sched`/`thread` code that runs clean at
`-smp 4` on both was meeting, on x86, an arch layer that quietly did half of what it promised.

## How a stale entry becomes paint

The mechanism is exact, which is what makes it fixable rather than mysterious.

1. A thread exits. `thread::KernelStack::drop` unmaps its six stack pages, recycles the physical
   frames to `kmem`, and pushes the address range onto `FREE_STACK_ADDRESS_SPACE` so the next
   thread lands in page tables that already exist. That reuse is deliberate and documented; without
   it every 2 MiB of address space costs an L2 and an L3 forever.
2. The unmap invalidates the reaping core's TLB. The *other* core still translates that range to the
   old frames.
3. A new thread takes the recycled range and gets different frames. `KernelStack::new` paints
   them, and `Context::for_kernel_thread` writes a real context at `top - 56`.
4. The stale core is handed that thread, and `switch_to` does `mov rsp, rsi` then six `pop`s and a
   `ret`, through a translation naming the *old* frames. Those frames have been recycled into
   somebody else's fresh, painted stack. So `ret` jumps to `PAINT`.

`rip 0x0` is the same story with a zeroed frame instead of a painted one.

**Confirmed before a line of the fix was written**, which is the part worth copying. Making a stale
entry harmless is a two-line experiment: never reuse stack address space, so no virtual address is
ever remapped onto a new frame. The fault vanished, 8 runs of 8, leaving only that test's own reuse
assertion failing (correctly, since the experiment broke reuse on purpose). That rules the
hypothesis in without the fix being able to flatter itself.

## Why an ordinary IPI cannot do this job here

This is the design point, and it is not a preference.

`mmu::unmap_page` takes `KERNEL_MMU`, which is an `IrqSafeMutex`: it masks interrupts and then
acquires. So the core sending a shootdown has interrupts off, and so does every other core running
the same code. Now the deadlock, which is certain rather than theoretical because two cores spawning
and reaping threads do this continuously:

- Core A holds `KERNEL_MMU`, sends a maskable IPI, and spins waiting for core B to acknowledge.
- Core B is spinning to acquire `KERNEL_MMU`, with interrupts off.
- B cannot take the message that would let A finish. A cannot release the lock B is waiting for.

Moving the remote flush outside `KERNEL_MMU` does not save it, only narrows it: any outer lock held
across an unmap reproduces the same shape one level up, and "remember never to unmap while holding a
lock another core takes" is a rule nothing enforces.

notes/riscv-tlb-shootdown.md already named the property that makes its own protocol work, and it is
exactly the one at issue:

> **The IPI arrives as an M-mode software interrupt**, so a hart with S-mode interrupts masked still
> services it. That is not a footnote: without it, any kernel code that disables interrupts and spins
> would deadlock whoever was flushing, and this kernel disables interrupts routinely.

RISC-V gets that from a privilege level below the kernel's. aarch64 never needs it. **x86 has exactly
one delivery mode `cli` cannot suppress, and it is the NMI.** So the choice is forced by the
architecture rather than made by taste.

## The protocol

`mmu::shoot_down_others`, and it is small on purpose.

- A raw `AtomicBool` lock, so one round is in flight at a time. Not an `IrqSafeMutex`: interrupts are
  already masked by the caller, there is nothing left to mask and no rank to check, and a core
  spinning here is still reachable by the only message that matters.
- `SHOOTDOWN_VA`, one page or a sentinel meaning "everything" (`flush_asid`'s case, which reloads
  `CR3` because with `CR4.PGE` clear that discards every entry).
- `SHOOTDOWN_PENDING`, a bitmask of cpu ids rather than a countdown. The sender sets it, each
  target clears its own bit, the sender spins until it reads zero. A mask so that an NMI from any
  other source, or a late one from the previous round, finds its bit already clear and is a no-op
  instead of an acknowledgement nobody owed.
- Ordering: the `Release` store to `PENDING` publishes `VA`, and the handler's `Acquire` load is what
  makes seeing its own bit imply seeing the address that bit is about. The handler's `Release` on the
  way out is what stops the sender observing the acknowledgement before the invalidate has happened,
  which matters because the sender's very next act is to hand the frame away.

Deadlock is now impossible rather than avoided: whatever a target is doing (spinning for
`KERNEL_MMU`, spinning for the shootdown lock itself, halted in `hlt`), the NMI arrives and the
acknowledgement follows.

### The handler may not touch anything reached through `gs`

An NMI lands at an arbitrary instruction boundary, and `trap.s` documents a window where
`IA32_GS_BASE` still holds the *user's* value while `cs` says ring 0 (between the exit `swapgs` and
the `iretq`). In that window `cpu::id()`, which reads that MSR, would answer with somebody else's
arithmetic. So the handler names its own core from the local APIC's id register instead, which is
hardware ground truth and needs no per-CPU pointer. `smp::seat_cpus_from_acpi` seats every core at
the slot its own APIC id names, so on this port the two numbers are the same, and the sender
`debug_assert`s that rather than assuming it.

The NMI is also served before the trap path picks a stack and before the deferred `schedule()`,
and both matter. It may not move to the per-CPU interrupt stack, because it can arrive inside a
handler already running there and would overwrite the frames underneath it. And it may not owe a
context switch, because its target is routinely mid-critical-section with interrupts masked.

## What proves it, and what does not

**`user::tests::an_asid_flush_reaches_the_other_cores` is the gate**, and it was already in the tree:
milestone 58 wrote it, portable, for exactly this property. On x86 at two cores it fails without the
shootdown:

```
[PANIC] assertion `left == right` failed: STALE TLB ON ANOTHER CORE: core 1 still translates
```

and passes with it. The suite then reaches `test result: ok. 177 passed` at `NIFE_SMP=2`, and 20
further runs produced no paint fault at all.

It is gated: `helpers/qemu-runner-x86_64.sh` defaults `NIFE_SMP` to 2, so every `script/test --arch
x86_64` exercises this code. The two-core failures and their fixes are recorded in
`arch::x86_64::ap_boot`'s `BUGS`.

## A second rider: taking a port grant off another core

Milestone 315, 2026-09-23. `PortRange::REVOKE` deleted the capability and cleared the cached grant
under `sched::IPC_TABLES`, then reset the TSS I/O permission bitmap **on the revoker's core only**,
which was the whole machine when it was written and stopped being so when `smp::seat_cpus_from_acpi`
landed. A holder running on the other core kept that core's bitmap until its next context switch, so
its `in`/`out` went on succeeding against a capability that no longer existed.

It rides this protocol, with `SHOOTDOWN_KIND` saying which of the two steps the round is for and
`SHOOTDOWN_VA` carrying a packed `(base, count)` instead of an address. Reusing it rather than
writing a second one is the point: the acknowledgement mask, the single-round lock and the NMI
delivery are the parts that are hard, and there is no second copy of them to keep in step. And the
NMI is forced here for the same reason as for a page: the target core is routinely spinning for
`IPC_TABLES` with interrupts masked, because the revoker is holding it.

The lock is what makes the far end safe, and it is worth stating as a rule because the handler
writes a TSS from an arbitrary instruction boundary. A core's port bitmap is written only by a
thread holding `IPC_TABLES`, or by an NMI such a thread sent. Milestone 315 moved `schedule`'s
`install_port_grant` inside the locked region to make that true. Before it, a core could read a
grant the revoke sweep had already cleared and install it behind the broadcast's back. The NMI
could land in the middle of an install.

The evidence was a snapshot of `INSTALLED_PORT_GRANT` taken at the revoke and printed by the test.
On both captured failures it read "revoker on cpu 0, cpu 1 holds a grant", and every passing run
either had no grant installed anywhere or had the holder switch away in time.

## BUGS

- One page per round trip, outside two batched paths. A thread reap unmaps
  `crate::thread::STACK_PAGES` pages and pays for six full shootdowns, where a batched protocol
  would pay for one. The two paths that move whole runs batch: `revoke::revoke_region` through
  `mmu::unmap_user_pages`, and `PageFrame::MAP` through `mmu::UserMapBatch`. Each keeps the
  obligation inside the arch layer (a closure, a `Drop`) rather than handing its caller an
  undischarged one, which is what `paging::TlbFlush` exists to prevent. The reap is worth the same
  move once `script/bench` says it matters; there are no numbers yet, because the bench path pins a
  single hart.
- Nothing shoots down a core that is online-but-not-yet-listed. A secondary between installing
  the kernel `CR3` and setting its bit in `ONLINE_MASK` is not a target. Its TLB is fresh and the
  only addresses it touches in that window are the direct map and its own never-recycled boot stack,
  so there is nothing stale for it to hold. The window is narrow rather than closed, and
  `smp::secondary_main` sets the bit before enabling interrupts precisely to keep it that way.
- **The spurious-NMI count is written and never read.** `exceptions::NMIS_UNCLAIMED` should stay at
  zero, because nothing else on this machine sends an NMI, but no test asserts that.
