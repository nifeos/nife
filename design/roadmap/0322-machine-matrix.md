---
status: NOT-STARTED
raised: 2026-09-18
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 322. One machine matrix for three architectures, because a board is a memory map and not a CPU

Every machine named below is in the pinned QEMU already. Nothing has to be bought,
ported or decided first; `script/cpu-matrix` is the shape to copy and milestone 59 already argued
the general case for narrowing an emulator rather than forking one.

In brief. Milestone 59 built a matrix over `-cpu` and left `-machine` at `virt` on every
architecture. Every RISC-V, aarch64 and x86_64 result this project has is from one board per
architecture. That is the same blind spot 59 found in `-cpu rv64` one axis over: a single machine
that has never told the kernel anything it did not want to hear.

## The evidence that this is not hypothetical

A machine option already boots the whole tour and silently loses every interrupt. Milestone 222
measured `-machine virt,gic-version=3` on 2026-09-02, and the result is the most dangerous shape a
failure has: the kernel boots, brings four cores online, prints `timer: 100 Hz tick, interrupts ON`,
and then takes zero interrupts and zero preemptions, with nothing faulting and nothing said.
`memory::gic_regions()` matches `intc@` by name and takes the first two `reg` blocks, so a GICv3 node
hands it the distributor and the redistributor, and `gic::initialize_this_cpu` writes `GICC_PMR` and
`GICC_CTLR` into registers that are not there.

That was found by a person running a command by hand, not by a gate, and it is why the runner's
`gic-version=2` pin is load-bearing rather than tidy. A machine matrix is the gate that would have
found it, and it is the same failure this project keeps meeting from other directions: a run that
is indistinguishable from a passing one.

And a machine's firmware has already hidden a missing boot path. `notes/x86-port/boot.md`: PVH is a
hypervisor protocol and no real firmware speaks it, so the kernel booted under QEMU for weeks by a
route the OptiPlex could not offer. QEMU's convenience concealed the gap until real hardware refused
it. A second machine profile is the cheap half of noticing that class earlier.

## What it is not

**It is not "run every machine".** The pinned QEMU offers 112 aarch64, 43 x86_64 and 10 riscv64
machines (measured 2026-09-18 on 11.0.2), and a matrix over them would be a very slow way to test one
memory map many times. The axis worth
covering is what a machine makes the kernel negotiate: its memory map, its interrupt controller,
whether it describes itself by device tree or ACPI, and how its PCI configuration space is reached.
Each row earns its place by answering one of those differently from `virt`.

**It is not a substitute for silicon, and nothing here should be read as one.**
notes/cpu-models.md's `BUGS` is the standard: *"a narrower QEMU model is still QEMU"* and *"a green
matrix is not a portable kernel. It is the absence of one specific class of failure."* Cache
behavior, real memory maps, errata and real firmware stay silicon's. Risk 5's own defining claim (a
defect QEMU cannot show) has no confirmed instance today; the argument that silicon still matters
does not need one to hold, and stands on the general case notes/cpu-models.md already makes.

## The rows, and why each one differs from `virt`

Confirmed against the pinned QEMU 11.0.2 on 2026-09-18: every machine and option below exists on
the pin.

**aarch64**

- `virt,gic-version=3`, and then `4`. The interrupt controller, which is the case above. This row is
  the gate on milestone 227 (a GICv3 driver, because GICv2 boots and silently loses every
  interrupt). The pin accepts `2, 3, 4, host and max`.
- `virt,its=off`. The pin offers `auto, gicv2m, its, off`, so interrupt translation is a machine
  option too, and every result so far is from the default.

`virt,iommu=smmuv3` is deliberately not a row here, because every leg already boots behind an IOMMU.
`helpers/qemu-runner-aarch64.sh` sets `iommu=smmuv3` on both the TCG and HVF paths unconditionally;
`helpers/qemu-runner-riscv64.sh` carries `-device riscv-iommu-pci` with `iommu_platform=on` on its
PCI disk, net and GPU; and `helpers/qemu-runner-x86_64.sh` carries `intel-iommu` with a recorded
analysis of its `intremap` capability bits. All three architectures already boot behind an IOMMU,
which is why `script/qemu-check` gates on `riscv-iommu-pci` existing in the pin at all.

On that axis the finding is not a machine row: the two IOMMU drivers rhyme, and their proofs must
too. `arch/aarch64/iommu.rs` carries two Kani harnesses over its entry-building arithmetic.
`arch/riscv64/iommu.rs` carries two harnesses of its own, proved from an aarch64 host (milestone 432
(design/roadmap/0432-the-riscv-iommu-driver-has-no-proof.md)): a parity question under §19
(architectural parity is a tenet) rather than a coverage gap this matrix could close.
- `sbsa-ref`. Describes itself by ACPI with no device tree, which is the discovery seam x86_64
  already exercises and aarch64 never has.
- `raspi3b`, and `raspi4b` which is also on the pin. A real SoC memory map with a different UART,
  which is the shape milestone 20's portable HAL claims to make cheap.

**riscv64**

- `sifive_u`. A real SoC map rather than `virt`'s, closest in kind to the U74 on radon.
- `spike`, `microchip-icicle-kit`. Different again, and cheap; keep whichever says something new.

And a thing the confirmation settles rather than proposes: the pin has no JH7110 or VisionFive 2
machine at all. Its ten riscv64 machines are the three above plus `virt`, `sifive_e`, `shakti_c`,
`none`, and three this block does not want (`amd-microblaze-v-generic`, `boston-aia`,
`xiangshan-kunminghu`). So there is no upstream model of radon's own SoC to adopt, which is the fact
the question "should we feed our silicon learnings back into QEMU" turns on: adopting a model is
cheap, writing one is a project in somebody else's tree.

x86_64

- `pc` (i440fx) beside `q35`. Legacy PCI configuration against PCIe ECAM, which is exactly what
  milestones 165, 215 and 314 reason about, tested today on one of the two.

## Whether more CPU models belong here too

Only where a model negotiates differently, and the criterion is the deliverable. Milestone 59's
five models earn their keep by saying no to distinct things: `svadu` (hardware A/D bits, absent on
`sifive-u54` and `thead-c906`, inverted on the RVA profiles), Sv39 against `rv64`'s Sv57. And
`sstc` (timer compare in a CSR against an SBI call). A sixth model that refuses nothing the five already
refuse buys runtime and no coverage.

So this milestone should write that criterion down and apply it once per architecture. That is
work neither 59 nor this block has done: aarch64 and x86_64 have no CPU matrix at all, and whether
they want one is the same question answered with their own feature lists rather than by analogy.
The pin offers 37 aarch64, 27 riscv64 and 243 x86_64 CPU models, so the criterion is doing the whole
of the work; a count is no help at all.

The riscv64 list is larger on the pin than 59's five suggest, and applying the criterion to it is
part of this milestone rather than assumed settled. `rv64i` is the base integer ISA with none of
`imac`, which refuses more than any model 59 tests and may simply refuse to run the kernel at all.
That outcome is itself worth knowing once rather than guessing.

## Index row

Every result on every architecture comes from one QEMU machine, `virt`, so the kernel has never been
asked to cope with a second interrupt controller, memory map, discovery mechanism or PCI
configuration route. That is milestone 59's blind spot one axis over, and it is not hypothetical:
`virt,gic-version=3` already boots the whole tour and then takes zero interrupts with nothing said,
found by hand rather than by a gate. This adds the `-machine` axis on all three architectures, with
each row earning its place by negotiating something `virt` does not, and records the criterion for
when another CPU model is worth its runtime.
