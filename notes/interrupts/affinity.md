# IRQ affinity: spreading device lines off core 0

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.*

An appendix to [notes/interrupts.md](../interrupts.md), moved there on 2026-10-11 (UTC) under §212
(a prose budget). It is the record of fix/irq-delivery (2026-07-29), the PLIC parity work and the
`std_net` hang that turned out not to be an interrupt bug.

Until now every SPI targeted core 0: `gic::enable` wrote `ITARGETSR[intid] = 1` (bit 0). Under SMP
that funnels every device interrupt onto one core. The handler that turns an interrupt into a
message runs on the core that took it (`handle_irq` calls `irq_notify`, which wakes the driver onto
`cpu::current`), so the driver wake lands on core 0 too. Every disk and NIC completion, and
the driver work it triggers, re-concentrates on core 0 no matter where the threads were spawned.

The fix distributes SPI lines across the online cores. The policy lives in `arch::irq::enable`
(it may read `smp::online_count`; it is arch glue, not a driver). Each SPI is assigned a target core
the first time it is enabled, round-robin over the online cores. That assignment is stable
(`IRQ_TARGET`, an atomic per-INTID slot). Stability matters because the `Irq` capability's ACK
re-enables the line on every completion; re-rolling the target each time would make the line hop
cores on every interrupt. The mechanism stays in the driver: `gic::enable(intid, target_cpu)`
writes `ITARGETSR[intid] = 1 << target_cpu`. PPIs and SGIs are per-core, so the target is ignored
for them (the timer PPI, the reschedule SGI). Rule #2 holds: the GIC driver is told which core, it
does not decide.

What this does and does not buy, measured honestly:

- It does move each device's interrupt (and the wake it causes) off core 0 onto its assigned
  core. Verified: the full aarch64 suite (disk, PCIe disk, both DHCP round trips) stays green with
  the lines spread, and a diskless boot's device IRQs land on cores other than 0.
- It does not, on its own, make the heavy `std_net` pipeline (smoltcp in `net_stack`, plus the std
  program) go faster under SMP. That pipeline is a chain of IPC rendezvous, and a rendezvous
  wake still lands on the *waker's* core (`cpu::current`). Spreading the interrupt moves the whole
  chain to the interrupt's core; it does not parallelize it. Parallelizing the pipeline needs the
  rendezvous/device-IRQ wake placement to be load-aware, which is the scheduler's call
  (DECISIONS §28 territory), not the interrupt controller's. See the `std_net` note below.

## The riscv PLIC side, done the same way (parity §19)

The PLIC equivalent spreads device sources across harts, the same round-robin-with-a-stable-target
shape as the GIC. The PLIC delivers a source to a context (a hart at a privilege level). The
hart-to-context numbering is the board's, read out of the device tree's `interrupts-extended` at
boot (`arch::irq::init_contexts`, backed by `isa::plic`). It is `2*hart+1` for S-mode on QEMU `virt`,
and `2*hart` on the JH7110, whose disabled S7 monitor core contributes only an M context (see
notes/visionfive2.md). The pieces are:

- Every hart sets `SEIE` early (`arch::irq::init_this_cpu` now unmasks supervisor external
  interrupts alongside the software-interrupt IPI source). This is safe before the PLIC base is even
  known: `SEIE` with no source enabled for the hart's context delivers nothing. It sidesteps the
  ordering hazard that a secondary comes online (running `init_this_cpu`) *before* the boot path
  calls `plic::init`; the CSR is unmasked now, the context is set up later.
- The boot hart opens a target hart's context and routes the source to it (`target_context` in
  `arch/riscv64/irq.rs`). The threshold and enable registers are global PLIC MMIO, so the boot hart,
  which runs every `enable` (test wiring, driver spawn), can open any hart's context and enable a
  source on it. The target is chosen round-robin over the online harts and is stable per source
  (`SOURCE_CTX`). The reason is the GIC's: the ACK re-enables the line on every
  completion, and the mask and the re-enable have to name the same context.
- Each hart claims and completes against its own context (`this_s_context`, the context table's
  entry for `cpu::id()`,
  passed to `plic::claim`/`complete`/`disable` from the external-interrupt handler). A source targets
  exactly one hart, so the hart that takes it is the hart it is enabled on, and the mask/complete land
  on the right context. The PLIC driver stays mechanism-only: it is told the context, it does not read
  the hartid (rule #2, DECISIONS §4).
- The enable bits are serialized; nothing else in the driver is. The sentence two bullets up, "the
  threshold and enable registers are global PLIC MMIO," is exactly what makes this necessary, and the
  assembly audit ([arch-audit.md](../arch-audit.md), finding 3) is where it was caught. One enable
  register carries 32 sources of a context, so setting one source's bit is a read-modify-write over
  a word its neighbors share, and the boot hart running `enable` can collide with another hart's
  handler running `disable` on a neighbor. A lost update either masks a device forever (its driver
  blocks on an interrupt that never arrives) or leaves a level-triggered source live after the handler
  masked it (an interrupt storm on that hart). So `enable`/`disable` go through one helper holding an
  `IrqSafeMutex` at `rank::IRQ_CONTROLLER`, the same rank the GIC's lock takes.

  It has to be `IrqSafeMutex` rather than a plain lock, and that is the §9 deadlock rather than
  caution. `disable` runs *in the handler* while `enable` runs in thread context with interrupts on.
  A plain spinlock would let a thread take the word on a hart and then have that hart's own
  handler spin on it forever. Nothing else in the driver takes the lock, deliberately.
  Claim/complete is per-context and therefore hart-local (the hot path stays lock-free). The
  per-source priority register is unshared. The threshold write is a whole-word store of `0`, so
  it is idempotent rather than an RMW. aarch64 never needed this: the GIC's
  `ISENABLER`/`ICENABLER` are write-1-to-set and write-1-to-clear, so one store touches one line and
  the architecture supplies the atomicity that the PLIC's plain read/write bits do not.

### The lost update, step by step

The abstract phrase "read-modify-write" hides the bug, so here it is concretely. Say the disk is
source 8 and the NIC is source 10. Both bits live in the same 32-bit enable word (sources 0..31 of
that context). A driver touching *its own* source still writes the neighbor's bit back, because
the only way to change one bit of that word is to store all 32.

Hart 0 is in thread context enabling the disk. Hart 1 is inside its handler masking the NIC. Without
the lock they interleave:

| step | hart 0 (`enable(8)`)        | hart 1 (`disable(10)`)      | word in the PLIC |
|------|----------------------------|-----------------------------|------------------|
| 1    | read → `0b0100` (bit 10)   |                             | `0b0100_0000_0000` |
| 2    |                            | read → `0b0100` (bit 10)    | `0b0100_0000_0000` |
| 3    | or in bit 8                |                             | `0b0100_0000_0000` |
| 4    | write `bits 8,10`          |                             | `0b0101_0000_0000` |
| 5    |                            | clear bit 10 from ITS copy  | `0b0101_0000_0000` |
| 6    |                            | write `0b0000`              | `0b0000_0000_0000` |

Hart 1's copy of the word was read at step 2, before hart 0's store at step 4. Its store at step 6
is computed from that stale copy, so it does not just clear bit 10, it *un-sets bit 8 as collateral*.
Hart 0's enable is gone, and nothing anywhere reports an error: the PLIC has no notion of a
disagreement, it just holds whichever word was written last.

Both directions of that race are bad, and they fail differently, which is why neither is tolerable:

- A lost `enable` (above) silently masks a device forever. The driver blocks in `Irq::WAIT` on an
  interrupt the PLIC will never route to it. The symptom is a hang in code that has no bug.
- A lost `disable` is the mirror: the handler thinks it masked a level-triggered source, but the
  bit is back on and the line is still asserted, so the PLIC re-delivers immediately, forever. That
  is an interrupt storm, and the hart it lands on stops making progress.

This is also why the window is small but not negligible. It is the few instructions between the read
and the write, hit only when two harts touch the same 32-source word at once. That is exactly the
kind of race that passes a thousand test runs and then fails in front of an audience. The audit found
it by reading for the pattern rather than by waiting for it to bite ([arch-audit.md](../arch-audit.md),
finding 3).

The fix is one `IrqSafeMutex` around the read and the write together, so steps 1-4 and 2-6 cannot
interleave: whichever hart takes the lock second re-reads the word *after* the first hart's store and
computes from current state. It is the smallest possible critical section (a read, an or/and-not, a
write) and it is off the hot path, so it costs nothing measurable.

Proven the same way as the GIC: the riscv suite is green at the 4-hart boot with device sources
spread across harts. That covers the disk read and the interrupt-driven redoxfs_server block server,
both routed to whatever hart the round-robin picked, plus the SMP placement tests. As on aarch64, this distributes the
interrupt and the wake it causes; it does not by itself parallelize the `std_net` pipeline, which
needs the load-aware rendezvous wake (see below).

## The `std_net` SMP hang was not an interrupt-delivery bug (resolved)

Recorded so it is not re-diagnosed as one, and left in place with its resolution because the wrong
diagnosis was tempting and the right one took two agents to reach.

The `std_net` test (smoltcp in `net_stack` serving a std program's `UdpSocket`/`TcpStream`) used to hang
under the 4-core boot on both ISAs, watchdog-killed at 60 s. In the same run the hand-built
DHCP round trips (`virtio_net`, `virtio_net_pci`) passed and the interrupt-driven redoxfs_server block
server passed. That asymmetry was the tell: interrupt delivery under SMP was sound, and the hang was
specific to the heavier, longer, timer-driven smoltcp pipeline.

Two things were wrong, and only one of them was scheduling:

1. A real deadlock. `net_stack` blocked on the NIC interrupt while smoltcp still had a retransmit
   pending, so neither side would move. The timer that would have retransmitted was never polled
   because the thread was parked in `Irq::WAIT`, and no packet was coming to wake it. That is a
   mutual-idle deadlock, not slowness, and no amount of core placement fixes it. `net_stack` now bounds
   its wait by smoltcp's own next-poll deadline.
2. Serialization on one core. With the deadlock gone the pipeline ran, but slowly: its threads
   are woken by a mix of device IRQ and IPC rendezvous, and both wakes pinned to one core. DECISIONS
   §28's wake split fixed the half that mattered (device-IRQ wakes go load-aware, IPC rendezvous
   wakes stay local, because a rendezvous partner is about to run on the caller's core and moving it
   only adds a migration).

An intermediate hypothesis of mine was wrong and worth keeping written down: I expected IRQ
affinity alone to fix it. It cannot. Spreading interrupts across harts relocates where a wake lands;
it does not parallelize a chain of request/response rendezvous, which is serial by construction. The
agent disproved it by measurement rather than argument.

`std_net` now passes on both ISAs. It is also the longest honest test in the suite at roughly 300 to
344 s on aarch64, because it is a real DHCP lease plus TCP and UDP round trips through an emulated NIC
under TCG. That length is legitimate work, not a symptom, which is precisely what makes it awkward for
the watchdogs: see the per-test ceiling discussion in [scheduler.md](../scheduler.md).
