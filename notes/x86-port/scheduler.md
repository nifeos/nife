# The x86_64 port: the scheduler and real processes

*An appendix to [`notes/x86-port.md`](../x86-port.md), which is the page to read. This file holds
the trap-path split, `TSS.RSP0`, the self-IPI, and the portable-code bugs a third architecture
found. It exists to verify or challenge the main page. A reader who only needs to build, boot or
test the x86_64 port should not have to open it. The directory `notes/x86-port/` and this file's
stem are provisional names; naming is calef's.*

*Records cited below: milestone 161 (the x86_64 kernel port), milestone 71 (the thread-start fault)
and §9 (locking).*

## The scheduler, and what "a process" cost that "a program at CPL 3" did not

Milestone 161's roadmap item 4, built 2026-08-24. Item 3 ended with a hand-assembled probe entered
from the boot thread; this is the distance between that and a process, and the distance was real.

Almost none of it was new x86 code. `kmem`, `untyped`, `sched` and `user::AddressSpace` are portable
and came up on this architecture by being compiled for it. What had to be written was the arch layer
underneath them, and what had to be *found* was four places where portable code encoded an
assumption that held on two architectures and not on three.

### The trap path grew the split both other ports already had

`x86_trap_handler` became `x86_trap_dispatch` (outer) plus `x86_trap_body` (inner), with
`dispatch_on_interrupt_stack` in trap.s between them. The outer half stays on the interrupted
thread's stack and is where the deferred `schedule()` runs; the inner half may run on this CPU's
interrupt stack. That is not an optimization: `schedule()` parks the running `rsp` in the outgoing
thread's `Context`. So calling it from a per-CPU stack would park a per-CPU address in a thread and
the thread would later resume on bytes the next interrupt had spent. See kernel/src/interrupt_stack.rs.

The timer arm now calls `sched::on_tick` and returns `true`, which is DECISIONS §9's record-and-defer
with the deferral one frame out.

### `TSS.RSP0` is recomputed from the frame on every return to ring 3

x86 has two doors into the kernel from ring 3 and they find their stack differently: a trap reads
`TSS.RSP0`, and `syscall` reads nothing at all and has to be told separately. With one user program
there was one kernel stack and `ring3_self_test` set both by hand. With a scheduler there is one per
thread, and the pair has to be re-pointed every time the thread that would come through them changes.

The frame's own address is the answer. Every thread's `TrapFrame` lives at `stack_top - 176` for
the life of the thread (milestone 71, `user::enter_frame`). So at the top of `isr_restore` the top is
`rsp + 176`, computed from the frame about to be loaded rather than from any record of who is
running. RISC-V does exactly this, at exactly this point, in `trap_return`. It costs four
instructions on a path that already tests the same `cs` for `swapgs`, and it makes the wrong state
unrepresentable rather than a duty somebody has to remember at each context switch.

### A self-IPI is this architecture's software-generated interrupt

`sched`'s two interrupt-delivery tests need a way to raise an interrupt by hand. aarch64 has an SGI
and needs no device; RISC-V can raise nothing at all and has to assert its console UART's transmit
line. x86 is aarch64's case: the local APIC's Interrupt Command Register has a "self" destination
shorthand, so any vector can be delivered to the CPU executing the write, through the real path (the
ICR, the IRR, the ISR, an EOI). `irq::raise_self_interrupt` is the whole of it, and the same ICR code
is `irq::send_reschedule`, which stopped being `unimplemented!()`.

The intid for such a source is its vector, and that is the naming rule this architecture needs
because `irq::enable` takes a *legacy IRQ number* instead. The two domains cannot collide: legacy
IRQs are 0..15 and the local APIC's own vectors are 0x20..0x2f.

A device line still cannot become a message here, and the missing piece is an inversion rather
than a mechanism. The flat vector map makes the GSI recoverable by subtraction, but a legacy IRQ is
not. GSI 0 is the 8259 cascade and has no legacy owner, so an inversion falling back to the
GSI would answer 0 for both it and the PIT's IRQ 0. Nothing needs it until there is a userspace
driver here.

### Four things portable code got right for two architectures and wrong for three

Worth listing together, because they are one shape: a default arm, or an expression, that names one
of two answers and is silently wrong when a third exists.

- `thread.rs`'s stack area. `KERNEL_VA_BASE | 0x10_0000_0000`, "64 GiB above the direct map",
  which is right where the kernel base is a *half* base with room above it. `KERNEL_VA_BASE` here is
  `0xffffffff80000000` and already carries that bit, so the OR was the identity: every kernel
  thread stack would have been mapped at the kernel image's own base, over `.text`. It is now each
  `arch::mmu::THREAD_STACK_AREA`, and x86's is Linux's `VMALLOC_START`. Found by reading; it would
  have surfaced as `kmem: no memory to wire one`, arbitrarily far from the cause.
- `crates/elf`'s `EXPECTED_MACHINE`. `#[cfg(not(target_arch = "riscv64"))] EM_AARCH64`, and
  `not(riscv64)` catches x86_64: the x86 kernel was compiled to accept aarch64 binaries and refuse
  its own. Now a three-arm `cfg`.
- `xtask`'s `ArchLegs`. `fn aarch64(self) { self != Riscv64 }`, which answers `true` for every
  leg the moment there is a third variant. Now explicit `matches!`.
- `smp::bring_up_secondaries`. It computed the secondary entry with `virt_to_phys` *before*
  checking whether any core can be started, and on x86 that panics. `secondary_boot` is in `.boot`.
  The linker places that section at its physical address because the trampoline starts in 32-bit protected
  mode and cannot name a 64-bit one. The conversion moved below the refusal. (It is not the right
  conversion for x86 SMP either, when that lands: that entry is already physical.)

The x86 tour also has to *call* `bring_up_secondaries`, even though it can start nothing. That
function marks the boot core in `ONLINE_MASK` before it refuses, and everything that broadcasts
(`online_cpus`, `nth_online`, the shootdown loops) reads that mask. Not calling it left the online
set empty while `online_count` said one, which the suite caught on its first run.

### What the boot tour shows now

```
  scheduler   : up on 1 cpu, preempting at 100 Hz (idle thread registered)
  smp: none yet (x86 uses INIT-SIPI-SIPI via the local APIC; milestone 161)
  smp: 1 core(s) online
  kernel task : a spawned thread ran and carried its captured state (0x16100004)
  userspace   : a process built from untyped ran at cpl 3 and sent 0x1610004 on a granted cap
                thread 8589934594 died at pc 0x400005 on addr 0xa50000, delivered to its supervisor
                two children cost 0 frames the first round and 0 the second (steady state)
```

Two children, because the two halves of "a process" fail differently. One is built out of a single
untyped region (address space, code page, stack page, TCB, two endpoints), granted a capability,
dispatched to ring 3 by the scheduler, invokes that capability, and exits. The other loads from an
unmapped address and its death arrives on its supervision endpoint naming the thread, the pc and the
address. Then both regions are destroyed and the frame count is compared.

The round runs twice, and that is what makes the frame number evidence. A first round pays
first-use carves that are not leaks; a system in steady state charges the second round zero. Two
mistakes in the demo itself would show as frames going missing each round. One is a reporting
child's corpse left uncollected, so its region still holds a live TCB when `destroy` refuses it
(silently, because `destroy` has nowhere to report). The other is endpoints drawn from the kernel's
shared pool rather than from the region being reclaimed.
