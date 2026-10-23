# The interrupt controller drivers

The design behind `kernel/src/drivers/gic.rs`, `gicv3.rs` and `plic.rs`. What a GIC is, its three
kinds of interrupt and its backwards priorities are in [notes/interrupts.md](../interrupts.md);
spreading lines across cores is in [affinity.md](affinity.md).

## GICv2

One distributor (GICD, `0x0800_0000` on QEMU's `virt`) decides which core gets an interrupt and
whether a source is enabled at all. Each core has a CPU interface (GICC, `0x0801_0000`) at the same
address, banked by the hardware, for acknowledge, priority mask and end-of-interrupt. That banking
is what lets the hardware route an interrupt to core 3 without the software knowing.

`init_this_cpu` writes the priority mask before the enable. The other order leaves the interface
live with whatever PMR the firmware left, which on a cold boot is often 0, "deliver nothing", and
the timer is silent with no clue why. Enables are a bitmap (`ISENABLER`, 32 a register) and
priorities an array of bytes (`IPRIORITYR`), different strides in one device. `ITARGETSR` means
something only for SPIs, since PPIs and SGIs are per-core by definition. `GICC_IIDR` is read before
anything else is trusted: its `ArchitectureVersion` is 2 on a GIC-400 and on QEMU's model, and a
GICv3 redistributor frame answers zero there, so a tree naming the wrong block fails at boot.

## GICv3

Three things change from GICv2, and two of them are `gicv3.rs`:

| | GICv2 | GICv3 | where it lives |
|---|---|---|---|
| distributor | routes SPIs with an 8-bit CPU mask (`ITARGETSR`) | routes SPIs to an affinity (`IROUTER`) once `ARE` is set | `gicv3.rs` |
| per-core private state (SGIs, PPIs) | banked inside the distributor | a redistributor frame per core, each at its own address | `gicv3.rs` |
| CPU interface | banked MMIO, `GICC_*` | system registers, `ICC_*` | `arch/aarch64/gic_cpu_interface.rs` |

The split follows the first rule of §4 (kernel shape: monolithic, deferred, with two cheap rules): `msr` and `mrs` are architecture code, so the CPU interface lives
under `arch/`, and everything reached through a pointer stays a driver handed its addresses.
`arch::irq` holds both and decides which version the machine has. design/roadmap/0227-gicv3-driver.md
has the placement and what lost.

Each core has a frame pair: `RD_base` (control, identity, power) and `SGI_base` 64 KiB above it (the
enables, priorities and groups for INTIDs 0 to 31). The frames sit end to end in the region the tree
names, and which frame belongs to which core is written in the frame, in `GICR_TYPER[63:32]` as an
affinity. So a core finds its frame by walking and comparing, never by indexing with its own id, as
Linux's `gic_iterate_rdists` does. A pair is 128 KiB, or 256 KiB with virtual LPIs (`VLPIS`), and
the last frame says so. A redistributor is asleep out of reset and forwards nothing until woken.

A GICv3 delivers an interrupt only if its group is enabled at the CPU interface. This kernel runs at
non-secure EL1 and enables Group 1, so every interrupt it uses is Group 1 (`IGROUPR` all ones). One
left in Group 0 is signaled as an FIQ, which this kernel does not take.

Distributor and redistributor writes are Device-nGnRE stores, which reach one device in program
order, so no barrier sits between them. What is needed is `RWP`, register write pending: a write to
`CTLR` or to a clear-enable register has taken effect only once that bit reads zero (Arm IHI 0069;
Linux's `gic_do_wait_for_rwp`). The driver waits after the writes Linux waits after: the distributor
off and on, the private-interrupt setup, and a disable. The wait is bounded and panics past the
bound, so a controller that is not answering does not look like a boot that stopped.

The sixteen SGIs are enabled and never disabled. Linux disables them and enables them one by one as
IPIs, but QEMU's GICv2 model forces SGIs enabled (`hw/intc/arm_gic.c`), and so does the GIC-400.
The Irq capability's mask-on-fire, unmask-on-ACK protocol relies on that in its SGI tests, where the
mask and the unmask can run on different cores. An SGI is an edge a core raises on purpose and
cannot storm, so the reason to mask a line does not apply to it.

## The PLIC

A RISC-V device raises a wire into the PLIC, which routes it to a context (a hart at a privilege
level) as a supervisor external interrupt (`scause` = 9). Between a claim and its completion the
source's gateway forwards nothing more, and that is the only mask the interrupt path uses.

A completion counts only on a context that enables the source. The specification says a completion
for a source the target does not enable is ignored, and the JH7110's PLIC does ignore it, leaving
the gateway waiting for a completion already written; that source never interrupts again. QEMU's
PLIC (`hw/intc/sifive_plic.c`) completes whatever it is given, so only silicon shows the
difference. That is why the interrupt path never disables a source.

Context numbers are the board's, read from the tree's `interrupts-extended` by
`arch::irq::init_contexts`. On QEMU's `virt` S-mode is `2*hart + 1` (`2*hart` is M-mode, OpenSBI's);
on the JH7110 the disabled S7 core contributes only an M context, and S-mode is `2*hart`.

Only one register needs serializing (finding 3 of notes/arch-audit.md):

| register | shape | serialized? |
|---|---|---|
| per-source priority | one word per source, whole-word write of a constant | no: unshared |
| per-context enable bits | one word shared by 32 sources, read-modify-write | yes |
| per-context threshold | one word per context, whole-word write of `0` | no: not a read-modify-write |
| per-context claim/complete | one word per context, read claims, write completes | no: not a read-modify-write |

Two interleaved enable updates lose one, and a lost enable leaves a device masked forever. It is
reachable: `enable` runs in thread context on any hart, and affinity puts several sources on each
context. The GIC needs no such lock, because its set-enable and clear-enable registers are
write-1-to-set and write-1-to-clear. The lock is an `IrqSafeMutex` though no handler takes it: the
section is two MMIO accesses, so masking costs nothing, and a handler that took a plain spinlock
here would be the deadlock §9 (locking: IrqSafeMutex, plus a discipline) forbids. Its rank is `IRQ_CONTROLLER`, the GIC's, and it is a leaf taken holding
nothing; `bind_irq` and `create_rendezvous` drop `IPC_TABLES` before they call. It guards `()`
rather than the base, so claim and complete, the hot path, never take it.

`enable` writes the enable bit first and the priority second. A device that spoke before it was
armed has a pending bit waiting, and by the specification the PLIC delivers it the moment the bit is
enabled. QEMU's PLIC (11.1.1) re-evaluates on a priority write, a threshold write, a completion and
a rising line, but not on an enable write. With the priority first, the re-evaluation ran while the
bit was clear and the interrupt stayed pending until another line rose. Written second, the priority
write is the re-evaluation, and on a PLIC that follows the specification the order costs nothing.
notes/usb.md has the keyboard stall this caused.
