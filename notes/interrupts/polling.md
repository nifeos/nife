# Interrupts or polling: why every confined driver in this tree polls

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.*

An appendix to [notes/interrupts.md](../interrupts.md), moved there on 2026-10-11 (UTC) under §212
(a prose budget). The main page keeps the conclusion; this is the argument and the measurements
behind it.

calef, 2026-09-17: *"What are the advantages and disadvantages of Irq vs polling? This has come up
multiple times."* It had, three times in two days, and the answer had never been written down, which
is why it kept coming up. This section is the record.

## The ordinary trade, which is not what decides it here

Polling wins on latency at depth (no delivery, no context switch, no EOI), determinism (an
icount tripwire can read a polled loop and cannot read an interrupt's arrival), and simplicity (no
vector allocation, no masking, no affinity).

Polling loses on three counts. It burns a core while idle, which is ruinous for a device asked for
64 bytes once a boot. It does not scale, because N polling drivers are N spinning threads. And it
cannot express *wake me when something happens*, which is the whole of milestone 103 (`^C` stops spinning).

None of that is why this tree polls.

## What actually decides it: an MSI is a memory write

An MSI or MSI-X message is a memory write to an architecturally special address. Plain DMA
remapping does not confine it: a component that can write a device's MSI-X table can aim an
interrupt at a vector it was never given. [DECISIONS §86 (whether an NVMe driver can leave the kernel)](../../design/decisions/0086-el0-nvme-driver.md)
found this and named the axis; `notes/confinement-claims.md` carries it as a claim stated nowhere.

Linux draws the same line and says so in its API: VFIO refuses to hand a device to an untrusted
userspace driver on a machine without interrupt remapping, and its escape hatch is named
`allow_unsafe_interrupts`. (§86 marks that as confirmed by search paraphrase rather than a fetched
source, and this note inherits the caveat rather than laundering it.)

Measured on the machines this project runs, not asserted:

| | where MSI remapping lives | state here |
|---|---|---|
| `x86_64` | VT-d interrupt remapping | offered, never enabled: `ECAP.IR` reads set and `arch/x86_64/iommu.rs` never writes `GCMD.IRE` (`notes/confinement-claims.md`) |
| `aarch64` | a separate *device*, the GICv3 ITS | absent: the TCG runner uses `gic-version=2`, which has none, and milestone 227 (a GICv3 driver) does not drive the ITS a `gic-version=3` machine offers |
| `riscv64` | inside the IOMMU's own device context (`CAP_MSI_FLAT`, widening it 32 → 64 bytes) | already driven, `arch/riscv64/iommu.rs` handles both formats |

So on two of three architectures an IRQ-driven EL0 driver would not be confined, whatever the
IOMMU does for DMA (on x86_64, ours). That is why milestone 159 (a real hardware entropy source) polls and why milestone 261 (the NVMe driver leaves the kernel)
does: in 261's lane's words, *"polling keeps `Object::Irq` off the grant list, which is less
authority."* **A confinement choice wearing a performance choice's clothes**, and worth saying out
loud because the next person meeting a polled driver will reasonably read it as a shortcut.

## What is not in question

An interrupt is already a capability here. `Object::Irq(u32)` has existed since milestone 9 (an interrupt becomes a message): the
kernel masks the line, `READ` lets the holder `WAIT` and `ACK`, and everything that knows what the
*device* is lives in the userspace driver. That is seL4's `IRQHandler` shape, and
[DECISIONS §101 (notification objects)](../../design/decisions/0101-notification-objects.md) already specifies `bind_irq(intid,
ep)` on the delivery side, with a prior-art survey covering seL4's bound notifications, Fuchsia's
`zx_port`, Mach port sets and why Linux's `epoll` model was refused.

`Object::Irq` confines a line-based interrupt perfectly well. The kernel masks it; the holder
can only wait and acknowledge; there is no way to aim it. MSI-X is the case it does not cover.

## The open question, and why it is deliberately not a decision yet

Who owns the page holding the MSI-X table? calef declined to mint that on 2026-09-17, and the
reason is the one `design/fatal-risks/README.md`'s own rule 1 gives: nothing is blocked on it, both EL0
drivers poll, and **there was no experiment behind it**. A decision with no experiment is a worry
rather than a choice.

Milestone 317 (the interrupt-remapping flags) made the question live. There are three options. EL0 drivers get `Irq` for line-based interrupts only, and MSI-X stays
kernel-owned. Or remapping is turned on, and a driver may own its table because the platform
confines it. Or it is allowed unconfined and said so, which is Linux's `allow_unsafe_interrupts`
and is a claim this tree should not make quietly.

**One asymmetry worth carrying forward**: on `riscv64` the IOMMU and the interrupt-confinement story
are the same hardware, so a board with a ratified RISC-V IOMMU would close milestone 143 *and* this
gap in one purchase. Milestone 143's 2026-09-17 survey found such a board buyable.
