---
status: NOT-STARTED
promoted_from: linux-runs-as-a-guest-of-nife
raised: 2026-10-12
milestone_dependencies: 16, 227
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 926. Linux runs as a guest, with nife as the hypervisor

Raised on 2026-10-12 (UTC) by the lane `maintainer/proposal-linux-guests`. calef asked that day,
"What would it take to be able to host Linux on nife?", and chose the hypervisor answer: an
unmodified Linux kernel boots as a guest virtual machine, and nife owns the hardware underneath it.

The other answer is [milestone 621 (a Linux persona server)](0621-a-linux-persona-server.md),
which runs Linux programs by translating their system calls onto nife capabilities. It is not this
proposal. How the two relate is below.

Reuse: the mechanism is written here, because it is kernel code at EL2, and §46 (thin primitives
or whole subsystems; we write everything in between) asks the kernel to be ours. The shapes are
borrowed. seL4's Arm hypervisor mode is the model for the objects and the exit path, and KVM's
arm64 port is the model for the EL2 register handling. Both are from memory and
were not re-read for this proposal. The guest is stock upstream Linux, unmodified.

## What the tree already has

Each of these was read on 2026-10-12, not taken from the brief.

- [design/driver-domains.md](../driver-domains.md) sketches nife at EL2 running one driver per
  virtual machine, and sizes it as "comparable in size to the whole EL0 story we already built". Its
  list of required mechanism is this proposal's rungs 1, 2 and 4: EL2 setup, stage-2 tables, exit
  handling, a virtual GIC and a vCPU model.
- [Milestone 127 (the seL4 machine)](0127-the-sel4-machine.md) built the EL2 entry on
  2026-09-02, as its prerequisite 1. `boot.s` reads `CurrentEL`, and if it is EL2 it configures the
  EL2 registers and drops to EL1. `NIFE_EL2=1` in `helpers/qemu-runner-aarch64.sh` rehearses it on
  QEMU with `virtualization=on`. Nothing in CI or `script/` sets `NIFE_EL2`, so the EL2 entry is
  tested only when somebody runs it by hand.
- [Milestone 227 (GICv3 driver)](0227-gicv3-driver.md) is BUILT. It drives the distributor,
  the redistributors and the `ICC_*` CPU interface. It writes `ICH_HCR_EL2 = 0` on the way down
  from EL2 and nothing else in the virtualization half. No list registers, no maintenance interrupt.
- [Milestone 16 (real hardware and IOMMU)](0016-real-hardware-iommu.md) is BUILT, with an SMMUv3
  driver in `kernel/src/arch/aarch64/iommu.rs`. Every stream table entry it writes is stage-1
  translate. On x86_64, VT-d confinement exists too; [milestone 612 (the IOMMU escape-fault test
  could lose its fault to an unconfined neighbor disk)](0612-x86-iommu-first-dma.md) is BUILT
  on it.

### Milestone 449's refusal was stale when it was written

[Milestone 449 (the EL2 to EL1 drop)](0449-the-el2-to-el1-drop.md) is REFUSED. It was backfilled
on 2026-09-20 from a 2026-09-03 bullet in milestone 2 (exception vectors, and a fault that tells
you what it was), which said QEMU's `virt` enters at EL1 and so
the drop would have no caller. The drop had been built and rehearsed on 2026-09-02, by milestone
127, with QEMU's `virtualization=on` as the caller. Its revisit condition ("an aarch64 machine whose
firmware hands this kernel EL2") was met on QEMU before 449 existed. So the refused work exists and
is on `main`. The record wants correcting to say so; this proposal does not edit 449.

The drop is also the thing a hypervisor undoes. A host kernel stays at EL2, so rung 1 turns the drop
into a choice: drop when there is no reason to stay, stay when the machine will host guests.

### HVF offers EL2 on an M3, which the tree says it cannot

The runner refuses `NIFE_EL2` under HVF, saying HVF "has no nested virtualization to offer".
`design/driver-domains.md` says the same. On 2026-10-12 a 36-byte probe was run on an Apple M3,
macOS 26.6.2, QEMU 11.1.2. It reads `CurrentEL`, writes the level as a digit to the PL011, and
waits. The result:

| machine | printed |
|---|---|
| `accel=tcg,virtualization=on`, `-cpu max` | `2` |
| `accel=hvf`, `-cpu host` | `1` |
| `accel=hvf,virtualization=on`, `-cpu host` | `2` |

So QEMU 11.1.2 on this host enters a guest at EL2 under HVF. Apple documents the API
([`hv_vm_config_set_el2_enabled`](https://developer.apple.com/documentation/hypervisor/apple-silicon)).
A third-party report says Apple enables it on M3 and later only
([LWN comment, 2025-09-20](https://lwn.net/Articles/1038917)), which was not checked against an
older chip. What was not measured: whether stage-2 translation, the virtual timer and the GIC's
virtualization interface work for a hypervisor running inside it. A QEMU commit from June 2025
still described HVF as unable to offer EL2
([commit](https://qemu.googlesource.com/qemu/+/72846594b089f03bed1bb46958c910afcb859d49)), so
the change is recent and its edges are unknown.

## The shape

### Two new kernel objects

Names are provisional, and an architect names them.

- `GuestAddressSpace`: a stage-2 translation root and its VMID. It maps a guest-physical address to
  a `PageFrame` the holder already has. Its tables are retyped from the holder's `MemoryRegion`,
  like an `AddressSpace`'s, so the kernel allocates nothing for a guest.
- `VirtualCpu`: the EL1 register file of one guest CPU, its virtual timer state and its virtual GIC
  list registers. It is bound to one `ThreadControlBlock` and one `GuestAddressSpace`. When the
  scheduler runs that thread, it enters the guest instead of EL0.

"Virtual CPU" is the spoken phrase, and CPU stays whole under §154 (the acronym test is whether
the phrase is spoken, applied recursively), hence `VirtualCpu` rather than
`Vcpu`. seL4 reuses its VSpace object for stage-2; a separate type here keeps a stage-1 capability
from ever being mapped as a guest's physical memory.

### The exit path is the fault path

A guest exit the kernel does not handle itself suspends the thread. The kernel then sends one
message on the thread's existing fault endpoint (the reserved slot `CAPABILITY_TABLE_SLOTS - 1`).
The message carries the syndrome, the faulting guest-physical address and the access size. Those
exits are a stage-2 abort on an unmapped address, an `hvc`, an `smc` and a trapped system register.
The VMM replies with register writes and a resume.

The kernel handles `wfi` (block the thread until a virtual interrupt is pending) and the virtual
timer, because sending those to userspace costs an IPC round trip per guest tick.

### The syscall surface

No new syscall number. Everything is `SYSCALL_INVOKE` on the two new objects. The method list is
provisional: `GuestAddressSpace::MAP` and `UNMAP`; `VirtualCpu::BIND`, `READ_REGISTERS`,
`WRITE_REGISTERS` and `INJECT`. The exit message layout is a wire format. Two new object types are
the deliberate growth `kernel/src/cap.rs` says each `Object` variant is, so they need a section in
`design/decisions/`. That is why `decision_dependencies` is `unwritten`.

### The VMM is an ordinary process

A virtual machine monitor (VMM) is a nife program like any other, spawned and confined by its
grants. It holds the `GuestAddressSpace`, one `VirtualCpu` per guest CPU, the guest's memory as
`PageFrame`s, and endpoints to the servers it fronts. It maps the guest's memory into its own
address space as well, which is how it reads a virtqueue. That is the model of §10 (process model:
capability-based, microkernel), where IPC carries control and shared memory carries data, with the
guest as one of the parties.

Every device the guest sees is emulated in the VMM, never in the kernel. The kernel learns that
virtual machines exist; it never learns that Linux exists.

## The rungs

### Rung 1: one guest CPU boots Linux to its root-filesystem panic

nife stays at EL2 when asked to host. The two new objects exist. Exits reach a VMM. The VMM emulates
a PL011 and a GICv3 distributor through stage-2 aborts, and answers PSCI over `hvc`. The kernel
injects the virtual timer's interrupt through one list register.

Linux needs some interrupt controller even to reach its panic: arm64 panics with "No interrupt
controller found" without one. So rung 1 carries the smallest virtual GIC that delivers one private
interrupt to one CPU.

- Done when: the arm64 kernel from Ubuntu's newest LTS release, pinned by checksum, with a device tree from the VMM boots on one guest CPU
  under `NIFE_EL2=1 NIFE_CPU=max NIFE_GIC=3` on TCG. It prints its banner and its timer calibration
  through the emulated PL011 to nife's console, then panics with `VFS: Unable to mount root fs`. A
  test reads that line. The existing suite passes on both entries, EL1 and EL2.
- Depends on: milestone 227 (built); the decision section for the objects and the exit format; VHE,
  per ruling 1 below.
- Refuses: device emulation in the kernel. AArch32 guests. A guest at EL2 (nested virtualization).
  Any Linux-specific hook in the kernel, and any paravirtual patch to the guest.

### Rung 2: a real virtual GIC, and guests with several CPUs

List-register save and restore on every switch, the maintenance interrupt, more pending interrupts
than list registers, virtual SGIs between guest CPUs, and a `VirtualCpu` that migrates between
physical cores with its thread.

- Done when: a guest given four `VirtualCpu`s on `-smp 4` reports four CPUs online and reaches the
  same panic. A test makes a guest CPU send an SGI to another and sees it taken. A host-crate test
  holds the list-register overflow logic.
- Depends on: rung 1.
- Refuses: a GICv2 virtual interface (GICH and GICV), which ruling 1 leaves out. GICv4
  direct injection. An ITS and LPIs for the guest, which wait for rung 4's MSIs.

### Rung 3: virtio devices served by the VMM against nife's servers

The VMM offers virtio-mmio devices to the guest and serves each from a server nife already runs.
virtio-console goes to the console server, virtio-blk to a file on nifefs through the filesystem
server, and virtio-net to the network stack as one more interface.

- Done when: Linux boots to a shell from a root filesystem on virtio-blk. A file the guest writes
  survives a guest reboot with nife still up. The guest reaches nife's network stack with a ping. A
  guest killed mid-write leaves nife's servers serving. `script/bench` gains one row: the round trip
  of a virtqueue notify through the exit path, the VMM and back.
- Depends on: rung 2.
- Refuses: an in-kernel virtio backend (vhost's shape). An emulated PCI bus or legacy devices.
  virtiofs and 9p, which are later work if a guest needs files shared with a running nife.

### Rung 4: device passthrough behind SMMU stage-2

A guest drives a real device itself, and the SMMU confines that device's DMA to the guest's memory.
This rung and rung 1 are the whole of driver domains.

The brief named milestone 143 as this rung's dependency, and that is wrong.
[Milestone 143 (silicon IOMMU)](0143-silicon-iommu.md) is the RISC-V IOMMU on a RISC-V board, and
it says outright that SMMUv3 on aarch64 silicon is not in it. The real dependency is milestone 16's
SMMUv3 driver, which needs stage-2 stream table entries pointing at a `GuestAddressSpace`. QEMU's
SMMUv3 only fronts the PCIe bus, so the passed-through device is a virtio-pci or e1000e function on
the bus §18 (the PCIe transport) already enumerates. QEMU 11.1.2's `arm-smmuv3` lists a `stage`
property; which values it accepts was not checked.

- Done when: a PCI function is assigned to a Linux guest, Linux's own driver for it works, and a
  test aims the device at memory outside the guest and sees the SMMU fault it.
- Depends on: rung 2 (the guest needs MSIs or an interrupt routed through the virtual GIC);
  milestone 16. Silicon needs an aarch64 board with an SMMUv3, which no milestone holds today.
- Refuses: passthrough without SMMU confinement, under any flag. Sharing one page table between the
  CPU's stage-2 and the SMMU's, which is an optimization for after it works.

### Rung 5: x86_64 guests

VT-x with EPT, a VMCS per `VirtualCpu`, and the same exit path. xenon's Core i5-7500T has VT-x
and EPT, from Intel's published specification as recalled, not re-checked. The test leg is the
open problem. To recollection, QEMU's TCG emulates AMD's SVM and not Intel's VMX, so an emulated leg
would exercise a different extension from the one xenon has.

- Done when: rung 1's outcome on x86_64, on xenon, with a test leg calef accepts.
- Depends on: rung 3, so the object model and the VMM are settled on one architecture first.
- Refuses: legacy PC device emulation beyond what a modern Linux needs to reach its panic.

### The §19 scope note

A host capability on aarch64 alone is the gap §19 (architectural parity is a tenet) says must be
recorded with a plan. riscv64's answer is the hypervisor extension. QEMU's TCG implements it;
radon's JH7110 does not have it, so there is no silicon to carry it to. The plan is rung 5's shape
again, with the objects unchanged: a second architecture after the first works, not three at once.

## How the tree would test it

- Host crates first. Stage-2 table math joins the `paging` crate, and the exit-message codec gets
  its own crate. Both run in milliseconds.
- A hand-written guest of a few dozen instructions, like the probe above. It takes each exit kind in
  turn (`hvc`, an MMIO write, `wfi`, a timer tick) and the kernel test checks each arrives decoded.
  That proves the mechanism with no Linux in the loop.
- One Linux boot leg per rung, on TCG with `NIFE_EL2=1 NIFE_CPU=max NIFE_GIC=3`, reading the panic
  or the shell prompt from the console. Its time budget is set when rung 1 measures it.
- An EL2 leg in CI from rung 1 on. Today nothing runs `NIFE_EL2` automatically, and a host kernel
  that only works when someone remembers to try it is rung zero.
- An HVF leg once someone measures stage-2 and the virtual GIC under it. Until then it is a probe,
  not a gate.

`NIFE_CPU=max` is required because `cortex-a72`, the TCG default, is Armv8.0 and has no VHE.

## Cost and risk

The size is driver-domains.md's: about the whole EL0 story again. Rung 1 is the largest, because it
is where the kernel first runs as a host. Rung 3 is mostly VMM code outside the kernel. No line
count is offered because none was measured.

The risks, plainly:

- No aarch64 silicon in hand can run the VHE design. argon is a TX1 with Cortex-A57 cores, Armv8.0,
  so it has EL2 but no VHE, and ruling 1 leaves it a non-host. Until a newer board exists, rungs 1
  to 3 run on QEMU and maybe on HVF.
- The privileged core grows. driver-domains.md names this tension and argues the growth is general
  mechanism. That holds only if every device stays in the VMM, which is why each rung refuses
  in-kernel emulation.
- An EL2 bug is a whole-machine bug. Every guest and the host share one EL2, and nothing is above it
  on QEMU to catch it.
- The tests need a Linux `Image`, tens of megabytes, from outside the tree. That is a §46
  dependency, and ruling 3 trusts Ubuntu's build of it.
- It does not rank itself. The customer path is vacant, and hosting Linux is a reason a customer
  might run nife. It is also a reason nobody needs nife's own programs. Both are calef's to weigh.

## How this relates to milestone 621

Milestone 621 runs Linux programs on nife with no Linux kernel; the persona translates. This runs
the Linux kernel itself, and nife knows only that a virtual machine exists.

The persona is cheaper per guest and keeps a program inside nife's capability model. It covers only
the system calls somebody writes, and 621 refuses the subsystems podman needs (namespaces, cgroups,
overlayfs). A virtual machine runs all of Linux, podman included, at the price of a whole guest
kernel and its memory. The two do not compete for mechanism: 621 needs no EL2, and this needs no
syscall translation. They answer different customers. One wants a Linux binary to run on nife; the
other wants a Linux machine next to nife's own programs.

## Rulings

calef answered the four questions on 2026-10-12 (UTC), relayed on #2004.

1. VHE only, no split design: "Only VHE. We want to target nife to newer hardware." argon does not
   host guests.
2. Exits arrive on the thread's existing fault endpoint: "Fault endpoint."
3. The test `Image` is the arm64 kernel from Ubuntu's most recent LTS release, pinned by checksum:
   "Use Ubuntu's kernel from their most recent long term release." The pin moves when a newer LTS
   ships.
4. Rung 1 waits behind the customer path and milestone 621: "Wait".

## Found while writing this

- Milestone 449 is REFUSED for work milestone 127 built. Its record wants correcting.
- The runner's HVF refusal for `NIFE_EL2`, and the same claim in `design/driver-domains.md`, are
  wrong on an M3 with QEMU 11.1.2, by the probe above.
- `NIFE_EL2` has no CI leg.

All three are bookkeeping or a test leg, and rung 1's lane is their home if no earlier lane takes
them.

## Index row

nife as a hypervisor at EL2, running an unmodified Linux kernel as a guest. Two new objects (a
stage-2 address space and a virtual CPU), exits delivered on the existing fault path to an ordinary
VMM process, and every device emulated outside the kernel. Five rungs: one CPU to the root-fs panic,
a real virtual GIC and SMP, virtio served by nife's own servers, passthrough behind SMMU stage-2,
then x86_64. It shares all of its mechanism with driver domains and none with milestone 621's
persona.
