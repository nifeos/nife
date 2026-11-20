---
status: BUILT
raised: 2026-09-15
built: 2026-09-15
---
# 299. The x86 port-range capability: the serial console becomes a userspace driver

calef ruled option 1, the port-range capability, straight rather than sequencing through §149 (may
the kernel answer on an endpoint)'s kernel thread (2026-09-15).

What was built. `Object::PortRange(base, count)` on the capability surface (semantics recorded in
DECISIONS §152, provisional), enforced by the **lazy** TSS I/O-bitmap write on the context switch
(`arch::segments::set_port_grant`, installed from `sched::schedule`). A switch that crosses no port
holder costs one comparison and writes nothing (`tss_iomap_lazy_nop`, +216 ticks/iter over a bare
switch). And a holder crossing writes only the bits that move, not the 8 KiB the naive always-write
§121 (what a device capability is when the device has no page) priced at ~2,682 ns cost every switch (`notes/benchmarks.md`, 2026-09-15). Revocation
(`PortRange::REVOKE`, and the kernel's own whole-machine sweep) deletes the capability, clears the
cached grant, and resets the core's TSS, so a revoked holder faults on its next `in`/`out`. The x86
console and input drivers are userspace processes holding COM1's `(0x3F8, 8)` ports, delegated by the
progenitor. `swish` reaches an interactive prompt on x86_64 over serial, its console served by the
userspace driver's `out`, with zero processes trapping (the milestone-268 gap closed). Two
load-bearing tests pass under QEMU (`kernel/src/user/x86_port_tests.rs`): a non-holder faults on `out`
(and a holder's grant does not leak across the switch to it), and a revoked holder faults on its next
`out`.

One scope note, recorded rather than hidden. The x86 input driver polls COM1 (reading the
port it holds and yielding between reads) rather than blocking on the receive interrupt, because x86
does not yet route a device line (COM1's legacy IRQ 4) to a userspace waiter. The kernel delivers only
self-directed vectors to a driver today (`kernel/src/arch/x86_64/exceptions.rs`). Output (the prompt,
and everything the shell prints) is unaffected and interrupt-independent. Interrupt-driven x86 input,
which routes IRQ 4 through the IO APIC to the input driver's `Irq` capability and lets it block, wants
its own milestone. The register layout it will use is already in `components/src/input.rs`'s x86 arm.

## What this is

On aarch64 and riscv64 a device driver is a userspace process holding the device's registers as a
memory-mapped capability. x86's legacy serial port has no page: COM1 is port I/O, reached only by
`in`/`out`, which ring 3 cannot execute unless the CPU's TSS I/O permission bitmap allows it. §121
kept the driver in the kernel for that reason. §121's amendment reverses it: x86 gets a second kind
of device capability, a port-range capability, so the serial console and input drivers move to
userspace and "every driver is a userspace process" holds on x86 with no exception, matching seL4,
Genode and L4Re.

## What to build

1. A port-range capability, a new `Object` kind naming a `(base, count)` range of I/O ports.
   This is system call surface (DECISIONS §10, §16), the irreversible category, so it is designed against
   the one real consumer it has (the console driver) rather than in the abstract. Granularity is a
   port range because a 16550 is eight consecutive ports, and the semantics are the honest analogue
   of the mapping-based device capability the other two architectures already use.
2. TSS I/O-bitmap enforcement on the context-switch path, the LAZY form. DECISIONS §121's own
   refinement (2026-08-25) is binding here: do **not** write the bitmap on every switch. Set the
   TSS `iomap_base` to a real bitmap only when a thread that holds a port capability is on either
   side of the switch. Every other thread switches with `iomap_base` pointing past the segment limit
   (no bitmap, all ports denied at ring 3). In a system where realistically one process holds a port
   capability, that is near-zero cost for the overwhelming majority of switches. The naive
   always-write was measured at ~2,682 ns/switch release (`tss_iomap_switch`, notes/benchmarks.md);
   this must not pay that on switches that do not involve a holder.
3. Revocation. A revoked port capability must clear the bits and reach every CPU that might hold
   a stale bitmap, a shootdown in the shape of the TLB shootdown this tree already has. Enforced,
   not assumed: a test that a revoked holder faults on the next `in`/`out`.
4. Move the x86 console and input drivers to userspace. They exist today as ring-3 stubs that
   trap on first port access (milestone 268's x86 boot builds them and they trap, which is the gap
   this closes). Grant each the port capability for its device's ports (`3F8h`-`3FFh` for COM1) at
   spawn, the way aarch64/riscv64 grant the UART's page. `swish` then reaches a userspace console
   server on x86 identical to the other two, and x86 boots to a prompt.

## The first measurement, because §121 named it

Before the capability is wired to real drivers, take the number §121's refinement asked for.
The lazy write's real cost, a per-thread "holds any port capability" bit checked on switch-in with
the bitmap written only when set, timed against a bare switch, the way `ipc_rtt` and `null_system_call`
are measured. This is not a second run of `tss_iomap_switch` (which prices the naive always-write);
it prices what this milestone actually ships. `script/bench --x86` is TCG-only on this host, so the
number is statistical, not silicon cycles, and the block should say so.

## The proof

`swish` reaches an interactive prompt on x86_64 under QEMU over serial, its console and input served
by userspace drivers holding port capabilities, and a revoked capability makes its holder fault on
the next port access. `board-check`'s x86 leg reaches the prompt (the top rung milestone 268 left
Outstanding on x86). Parity, per DECISIONS §19: the console is a userspace driver on all three
architectures, no scope note.

## What this does not do

- **It does not touch the mapping-based capability for MMIO devices.** This adds a second device
  capability for the one class that has no page. PCI/PCIe devices are unchanged.
- **It does not revive §121 option 3** (the I/O-port broker, a system call per register access), refused
  on its measured cost and not reopened.
- **It does not require xenon.** The real-hardware confirmation on xenon's COM1 is a later bench
  step, not a gate on this build. QEMU enforces the same TSS mechanism.

## BUGS

- This is the first new object on the capability surface since the model settled. Getting the
  granularity or the revocation semantics wrong is expensive to unwind (§10, §16), which is why the
  design is pinned to the one consumer that exists rather than generalized.
- The lazy-write correctness is subtle. A thread that holds a port capability switching to one
  that does not must leave the TSS denying ports, or the second thread inherits the first's access.
  The revocation test and a "non-holder cannot touch the port" test are both load-bearing, not
  nice-to-have.

## Follow-on

- **Recorded.** x86 input polls COM1 rather than blocking on the receive interrupt, recorded in
  `components/src/input.rs`'s x86 `uart` arm and this block's own scope note. COM1's legacy IRQ 4 is
  not yet routed to a userspace waiter (the kernel delivers only self-directed vectors to a driver,
  `kernel/src/arch/x86_64/exceptions.rs`). Wiring it needs the device-line delivery path (mask on
  fire, `irq_route`/`irq_notify`, EOI, unmask on ACK) that x86 has for the timer but not yet for a
  device. Output, and the prompt, are interrupt-independent, so this is a limitation of input
  latency and CPU spent polling, not of whether the milestone's claim holds.
- **Decision.** `design/decisions/0152-port-range-capability.md` (provisional number), the `PortRange`
  object and its `REVOKE` method on the capability surface, an architect's to ratify.
- **Recorded.** Two limits are in §152's BUGS beside the feature: a thread caches one port range, not
  a set (every real consumer holds one), and a `PortRange` delegated to an already-running thread by
  `SEND_CAP` is not cached (no consumer does that). Both are within the design's pinned consumer.
- **Done.** The x86 prompt that milestones 268 and 182 left Outstanding: a default x86_64 boot now
  reaches an interactive `swish` prompt over serial, its console served by a userspace driver holding
  COM1 as a port capability, with zero processes trapping. Checked 2026-09-15 by booting it under
  QEMU `q35`; the prompt and the shell banner are in the transcript above the hand-over line.

## Index row

The x86 port-range capability. DECISIONS §121 was reversed 2026-09-15 once the serial console became
the customer it had assumed away (a prompt on every architecture, headless x86 driven over serial).
x86's console and input drivers move to userspace holding a `(base, count)` port capability enforced
by the TSS I/O bitmap, lazily, so "every driver is a userspace process" holds on x86 with no
exception. Consumes milestone 268/182's x86 userspace boot; reaches the x86 prompt that milestone
left Outstanding.
