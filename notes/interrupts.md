# Interrupts: the GIC and the timer

Milestone 5 (the GIC and the timer: the kernel is preemptible). A timer interrupt can land between any two
instructions.

Which means every piece of the locking discipline we wrote in
[DECISIONS](../design/decisions/0009-irq-safe-locking.md) §9 stops being a hypothesis.

The evidence behind this note is in [interrupts/](interrupts/README.md).

## The GIC: the multiplexer in front of the CPU

The CPU has one IRQ input line. That's all. Priorities, masking individual sources and routing to a particular core all live in the interrupt
controller, not in the CPU.

Two halves, and the split *is* the design:

| | Where | Shared? | Does what |
|---|---|---|---|
| Distributor (GICD) | `0x0800_0000` | one per machine | which core gets an interrupt, and whether a source is enabled at all |
| CPU interface (GICC) | `0x0801_0000` | one per core (banked) | this core's own view: acknowledge, priority mask, end-of-interrupt |

N cores see their *own* CPU interface at the *same address*: the hardware banks the registers
per core, so "deliver this to core 3" needs no software help.

Both addresses come from the [device tree](device-tree.md) (`intc@8000000`), not a constant.

## Three kinds of interrupt, and the numbering isn't arbitrary

| INTID | Kind | |
|---|---|---|
| 0–15 | SGI: Software Generated | one core kicking another. This is how SMP bringup and TLB shootdown work. |
| 16–31 | PPI: Private Peripheral | per-core. The timer is one. |
| 32+ | SPI: Shared Peripheral | the UART, the disk. Any core may service them. |

The timer is a PPI (INTID 30), and it has to be: a timer on only one core could not preempt the
others. Every core has its own timer, its own countdown, and
its own interrupt, all wearing the same number.

The device tree says so: `interrupts = <1 14 ...>` on the timer node. Type 1 means PPI, 14 is
the PPI number, PPIs start at 16, so `16 + 14 = 30`.

## Priorities are backwards

Lower value = higher priority, and `GICC_PMR` is a *mask*: an interrupt is delivered only if
its priority is strictly less than PMR.

So `PMR = 0xff` means "let everything through" and `PMR = 0` means "let nothing through."

Get that comparison backwards and the machine takes no interrupts and gives no clue why. It's also
why `gic::init` sets PMR before enabling the CPU interface:
the other order leaves a window where the interface is live with whatever the firmware left in
PMR, which on a cold boot is often zero.

## Acknowledge, then end-of-interrupt

```
IAR  (read)   -> "which interrupt?"   ...and READING IT IS WHAT TAKES IT.
EOIR (write)  -> "I'm done with it"
```

`IAR` has a side effect. Reading it acknowledges. Once per interrupt.

And until `EOIR` is written, the GIC will not deliver another interrupt of equal or lower
priority. **Forget it and the timer fires exactly once and then never again**, which looks
nothing like "you forgot to write a register."

INTID 1023 is spurious: the GIC raised the line and then changed its mind (another core took
it, or it got masked). Do nothing; in particular do **not** write EOIR: signaling
completion for an interrupt you never took corrupts the GIC's priority stack.

## IRQs dispatch by vector slot, not by ESR

`exception_dispatch` gets both the trap frame and *which of the sixteen vector slots fired*
([exceptions.md](exceptions.md)). For a fault we decode `ESR_EL1`. For an IRQ we must not.

`ESR_EL1` describes a synchronous exception: what instruction did what wrong. An IRQ is
*asynchronous*. It has nothing to do with the instruction it interrupted, and `ESR_EL1` still
holds whatever the last *synchronous* exception left there. Reading it in an IRQ handler is
reading a stale answer to a question nobody asked.

## The bug we shipped and then measured

The timer is one-shot. It fires, and then sits there with its status bit set, holding the
interrupt line high until the handler sets a new deadline.

There are two registers to do that with, and the difference is not cosmetic:

| | |
|---|---|
| `CNTP_TVAL_EL0` | a relative countdown. "Fire N ticks from *now*." |
| `CNTP_CVAL_EL0` | an absolute deadline. "Fire when the counter reaches exactly this." |

Re-arming with `TVAL = interval` in the handler makes the real period

```
    interval  +  however long it took to get into the handler
```

Every tick starts its countdown *late*, and **the lateness is never recovered**: the clock runs slow
forever, silently.

Measured in QEMU at 100 Hz configured:

```
  +250ms: 17 ticks fired   <- should be 25.  ~70 Hz.  30% of our preemptions, gone.
```

`CVAL` puts the deadlines on a fixed grid: `next = previous + interval`, anchored at boot. A
slow handler makes *one* tick late; it does not push the next one out too.

```
  +250ms: 25 ticks fired   <- correct
```

One register.

### The safety valve

If we fall so far behind that the next deadline is *already in the past*, `previous + interval`
would fire immediately, and again, and we'd spin in the handler forever paying down a debt we
cannot pay.

So: give up on the missed ticks and re-anchor the grid to now. Every kernel does this (dropping
ticks), and it is worth counting: a nonzero count means the handler is taking longer than a
whole tick period.

## Uptime comes from the counter, not the tick count

`uptime_ms()` reads `CNTPCT_EL0` and divides. Deliberately not `ticks * 10`.

If a tick is ever missed (a long critical section, a slow handler), the tick count undercounts
and *time appears to slow down*. The hardware counter cannot lie.

This is `Instant`. It is the thing `core` could never give us, and the reason is exact:
nothing in `core` knows what time it is.

## The test the whole locking discipline was written for

Everything in [locking.md](locking.md) exists to prevent one thing: a timer interrupt landing
inside a critical section, taking the same lock, and spinning forever waiting for code that
cannot run until it returns. On one core. Permanently.

Until this milestone there were no interrupts, so that was a hypothesis.

`holding_a_lock_masks_the_timer`:

1. confirm ticks are flowing
2. take a lock, and busy-wait across three whole tick periods
3. assert not one tick landed
4. release, and watch them resume

Step 2 works because `spin_for` reads `CNTPCT_EL0`, which keeps counting while interrupts are
masked. A tick-based delay would simply hang there, which is its own kind of proof.

## And the cost of masking, made visible

`a_long_critical_section_costs_a_tick` asserts that holding a lock across two tick periods
loses a tick. The deadline passes while we cannot service it, we re-arm to a deadline
already in the past, and the only sane move is to drop it.

That is the bill for deadlock prevention, and why "keep critical sections short"
(DECISIONS §9) has teeth rather than being good manners. At milestone 6, a lost tick is a thread
that didn't get preempted.

It is a strange thing to *assert*, until you notice: if that cost stopped being real,
`IrqSafeMutex` would have stopped masking, and the deadlock would be back.

---

*Add to this file as new interrupt sources come up.*

---

# Milestone 9a: an interrupt becomes a message

DECISIONS §10 promised this and notes/capabilities.md sketched it. Here it is.

## The problem a userspace driver has with interrupts

A driver at EL0 (milestone 8 put one there) cannot install an interrupt handler: handlers run at
EL1, in the kernel's vector table. And the kernel cannot handle a device interrupt itself, because
it does not know what the device is, which was the point of moving the driver out.

So the interrupt reaches the driver as something it *can* receive: a message.

## The shape

```
  device raises INTID  ──►  kernel handle_irq:  mask INTID at the GIC
                                                turn it into a notification
                                                EOI
                                                        │
                       driver blocked in WAIT  ◄────────┘  wakes
                                                │
                       reads the device, quiets its interrupt
                                                │
                       invoke(irq_cap, ACK)  ──►  kernel: re-enable INTID at the GIC
```

The kernel's half does nothing device-specific, and everything that knows what a *virtio block
device* is lives in userspace.

## Why the interrupt gets masked the instant it fires

Device interrupts are usually level-triggered: the device holds its interrupt line asserted
until the driver does something to quiet it (for virtio, reads `InterruptStatus` and writes
`InterruptACK`). If the kernel left the line enabled and just EOI'd, the GIC would re-deliver it forever: a storm
the machine never climbs out of, because only the driver can quiet the device and it never runs.

So `handle_irq` masks the INTID at the distributor (`gic::disable`) the moment it fires. The driver
services the device, then calls `ACK` on its `Irq` capability, and only then does the kernel
re-enable the line (`gic::enable`). Until then the interrupt is held off. This is exactly seL4's
IRQHandler protocol, and it is what lets a process that holds no privilege safely own an
interrupt.

## An interrupt is not a rendezvous

IPC on a rendezvous, the milestone 7e (user mode) shape, is synchronous: a sender waits for a receiver. An interrupt
cannot wait. It fires whether or not the driver happens to be blocked in `WAIT` at that instant,
and it must not be lost if the driver is a hair late.

So the notification is asynchronous, and the mechanism is one counter, the rendezvous's `pending` count. If
a thread is waiting, the interrupt wakes it. If not, `pending` is incremented, and the next `WAIT`
drains it instead of blocking. An interrupt that fires one instruction before the driver calls
`WAIT` is remembered, not dropped. There is a test named exactly that.

## The capability

`Object::Irq(intid)`. Its holder can:

- `WAIT`: block until the interrupt fires (internally, `RECEIVE` on the endpoint the kernel routed
  the interrupt to).
- `ACK`: re-enable the interrupt at the GIC, after quieting the device.

A driver holding this capability receives one specific interrupt and nothing else: it cannot mask
other interrupts, touch the GIC or see another device's line.

## Testing it with no device

A software-generated interrupt (SGI) exercises the whole path: `gic::send_sgi` raises INTID 1 with no
hardware behind it. A thread blocks in `WAIT`, the test raises the
SGI, the handler routes it, the thread wakes. Deterministic, and it needs no disk. The virtio
driver (9b) uses the same path with a real device interrupt.

## Testing it on RISC-V, which has no SGI (§19 (architectural parity), 2026-07-31)

The two tests (`kernel::sched::tests::an_interrupt_becomes_a_message` and
`an_interrupt_that_arrives_before_the_wait_is_not_lost`) trigger with an SGI on aarch64. On RISC-V
the trigger is the console UART's own
transmit-empty interrupt, behind `arm_test_irq`, `raise_test_irq` and `quiet_test_irq`. The SBI IPI
was refused because it never reaches `irq_route`, and writing the PLIC's pending bits does nothing.
The two legs are not twins: aarch64's needs no device, and RISC-V's covers more of the controller.
Why each option lost, and the unbacked claim the old doc comment made, are in
[interrupts/riscv-test-trigger.md](interrupts/riscv-test-trigger.md).

---

# IRQ affinity: spreading device lines off core 0 (fix/irq-delivery, 2026-07-29)

Device lines no longer all target core 0. `arch::irq::enable` gives each SPI a target core the
first time it is enabled, round-robin over the online cores, and keeps it (`IRQ_TARGET`), because
the `Irq` capability's ACK re-enables the line on every completion. The GIC driver is told which
core and does not decide (rule #2). The PLIC does the same per hart (`SOURCE_CTX`), with each hart
claiming and completing against its own context.

The PLIC's enable bits share one 32-bit word per 32 sources, so setting one is a read-modify-write
that two harts can interleave, and a lost update masks a device forever. It goes through one
`IrqSafeMutex` at `rank::IRQ_CONTROLLER`, found by the assembly audit
([arch-audit.md](arch-audit.md), finding 3).

On the PLIC the claim is the mask and the driver's ACK is the completion. The handler claims a
routed source and leaves it enabled; the gateway forwards nothing more from it until the ACK
completes it against the context that claimed (`arch::irq::hold_claim`). It must stay enabled,
because the specification ignores a completion for a source its target does not enable. The
JH7110's PLIC does and QEMU's does not, so a handler that disables before it completes passes
every gate and shows only on radon, as one keystroke per boot (milestone 890).

Spreading interrupts moves a wake; it does not parallelize a chain of IPC rendezvous. The `std_net`
hang under SMP was fixed by a bounded wait and §28 (SMP placement)'s wake split, not by affinity. The
race, the per-hart context numbering and that diagnosis are in [interrupts/affinity.md](interrupts/affinity.md).

## Two GICs: which one this machine has, and how the kernel knows

*(Milestone 227, 2026-09-19. Until then this section was headed "BUGS: this is a GICv2 driver", and
the measurement that made that heading dangerous is kept below.)*

The kernel drives GICv2 and GICv3, and picks between them at boot from the device tree. For an
aarch64 board the question is still which interrupt controller it has, but the answer is now a
driver the tree carries rather than a port. A GIC-400 (Raspberry Pi 4, the Jetson TX1 that is argon)
and a GIC-500 or GIC-600 (i.MX8M, most server
parts) are both covered. Apple Silicon has AIC, not a GIC, so an Apple core meets this kernel only under a hypervisor that
emulates one.

| | GICv2 | GICv3 |
|---|---|---|
| found by | `compatible` = `arm,gic-400`, `arm,cortex-a15-gic` or `arm,cortex-a7-gic` | `compatible` = `arm,gic-v3` |
| distributor | `drivers/gic.rs` | `drivers/gicv3.rs` |
| per-core private interrupts | banked in the distributor | a redistributor frame per core, found by its `GICR_TYPER` affinity (`drivers/gicv3.rs`) |
| CPU interface | banked MMIO, `drivers/gic.rs` | `ICC_*` system registers, `arch/aarch64/gic_cpu_interface.rs` |
| SPI routing | `GICD_ITARGETSR`, an 8-bit core mask | `GICD_IROUTER`, the target core's affinity |
| SGI | an `GICD_SGIR` store | an `ICC_SGI1R_EL1` write, `dsb ishst` before it |
| confirmed at boot by | `GICC_IIDR.ArchitectureVersion` from the block the tree calls the CPU interface | `GICD_PIDR2.ArchRev`, then `ICC_SRE_EL1.SRE` sticking |

One place holds both. `arch/aarch64/irq.rs` reads the version (`machine_discovery::gic`,
host-tested against QEMU's own GICv2, GICv3 and HVF trees), asks the hardware whether the tree is
right, and only then initializes a driver. Every other caller in the kernel (the IRQ handler, the
timer, the scheduler's reschedule SGI, the tests) names `arch::irq` and never a driver, so the
choice is made once. The split between `drivers/` and `arch/` is DECISIONS §4 rule 1: the ICC
registers are `msr`/`mrs`, so they are architecture code, while everything behind a pointer stays a
driver that is handed its addresses. `design/roadmap/0227-gicv3-driver.md` has the placement and what
lost.

### How to run each

```sh
script/test --arch aarch64                    # GICv2, the TCG default and the one CI runs
NIFE_GIC=3 script/test --arch aarch64         # GICv3 under TCG
script/test --hvf                             # GICv3 on the physical core; HVF refuses a GICv2
NIFE_GIC=3 cargo xtask boot-check --arch aarch64   # the boot self-test's verdict, one GICv3 boot
```

The boot line says which one it found:

```
  interrupts      : GICv2, distributor 0x0000000008000000, cpu interface 0x0000000008010000
  interrupts      : GICv3, distributor 0x0000000008000000, redistributors 0x00000000080a0000, cpu interface in system registers
```

Why TCG stays at 2. Every recorded TCG number (the icount tripwire's baselines, the fastpath
footprint, the benchmark history) was taken on a GICv2, and argon is a GIC-400. HVF has no choice:
QEMU 11.1.1 refuses `gic-version=2` with HVF, so the runner asks for 3 there.

### The gate against silent loss

A GIC assumption that stops being true now fails loudly. An unknown `intc@` binding, a tree the
hardware contradicts, or a core missing its redistributor or `ICC_SRE_EL1.SRE` each panics with a
name. A test holds every core to taking its own ticks. The three points are listed in
[interrupts/gicv3-history.md](interrupts/gicv3-history.md).

### BUGS

- `ID_AA64PFR0_EL1.GIC` reads zero under HVF on an Apple core while QEMU emulates the GICv3
  system registers behind it (measured 2026-09-19: `0x1101000010110011`). The first draft of the
  boot check required that field and refused to boot there. The check now rests on the
  distributor's revision and on `SRE` sticking, which is what Linux measures too.
- No ITS, so no LPIs and no MSI translation on a GICv3. Milestone 317 wants it for interrupt
  remapping; under HVF QEMU offers a GICv2m frame instead of an ITS anyway.
- The GICv2 SGI has no barrier before its `SGIR` store, which the GICv3 path has. Recorded in
  `drivers/gic.rs`, unchanged so the GICv2 path's counts stay still.
- The EL2 half (`boot.s` opening `ICC_SRE_EL2`) cannot be proven on QEMU, whose `ICC_SRE_EL2`
  reads as set whether or not the step runs. It is Linux's sequence, and the EL1 assertion is what
  would catch a board where it matters.

### Before milestone 227 (a GICv3 driver), and what no CPU matrix catches

Milestone 222 (a leg that fails instead of skipping) ran `-machine virt,gic-version=3` and found no error message: the kernel booted and
silently lost every interrupt, because it wrote `GICC_*` registers into the redistributor frame.
That made the `gic-version=2` pin load-bearing, and it is the shape the gate exists to make
impossible. Milestone 227 built what 222 priced, plus two findings: SGIs stay permanently enabled,
and HVF hides the GIC ID field. Why aarch64 has no CPU-model matrix like RISC-V's, and the memory
ordering no matrix catches, are in [interrupts/gicv3-history.md](interrupts/gicv3-history.md).

# Interrupts or polling: why every confined driver in this tree polls

calef asked on 2026-09-17 why the tree polls, the third time in two days it had come up. The answer
is confinement, not performance. An MSI or MSI-X message is a memory write, and plain DMA remapping
does not confine it. A component that can write a device's MSI-X table can aim an interrupt at a
vector it was never given (§86 (whether an NVMe driver can leave the kernel)). MSI remapping is offered but never enabled on `x86_64`, and absent
on `aarch64` (no GICv3 ITS is driven); only `riscv64` drives it. So an IRQ-driven EL0 driver would not
be confined on two of three architectures, and milestones 159 and 261 poll. `Object::Irq` already
confines a line-based interrupt; MSI-X is the case it does not cover.

Who owns the page holding the MSI-X table is open on purpose: calef declined to mint it on
2026-09-17 because no experiment stands behind it. Milestone 317 (the interrupt-remapping flags)
made it live. The ordinary trade, the per-architecture measurements, the three options and the
RISC-V asymmetry are in [interrupts/polling.md](interrupts/polling.md).