---
status: NOT-STARTED
raised: 2026-09-18
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 325. Four things the PCI scan reads and does not yet program, each found by a different lane

Four proposals from four lanes, all in the discovery-and-enumeration seam, carried as one block.

Every part is reachable from a machine this project already runs: QEMU's `q35` with
`-device intel-iommu` for parts 1 and 4, `NIFE_PCIE_ROOT_PORT=1` on the x86_64 runner for part 2, and
a boot line rather than hardware for part 3. No decision is owed on any of them.

Why a cluster. Milestones 256, 303, 308 and 320 each hit a different edge of the same surface
while doing something else, and each correctly filed rather than widened. They share a shape: the
scan reads what the machine says and then does less with it than the machine allows, and every one
of them is invisible until a machine is bigger or stranger than QEMU's default.

That is the shape `design/fatal-risks/README.md` risk 9 is about. Milestone 87's first light on xenon
already produced one of this family (`AlreadyMapped`, because the firmware's map does not describe
the MMIO hole), and it was machine-specific and fixed inside `arch/x86_64/mmu.rs`. These four are the
ones found *before* the machine that punishes them.

## The four parts

1. **An unclaimed PCI function behind the IOMMU is refused one request at a time, not confined
   once.** Milestone 303's lane put a second `virtio-blk-pci` function on `q35`'s bus, watched the
   first fault, and spent an hour proving the fault was correct rather than a confinement gap. Both
   `virt` boards have the same shape with their own units. Found by milestone 303's lane.
2. A bridge memory window nothing programs. QEMU's PVH boot leaves a root port's window
   unprogrammed, which one environment variable reproduces. Found by milestone 320's lane.
3. The BAR window is sized by a constant rather than by the bus. The sizing pass already exists
   in `pci::read_bars` and needs moving earlier; the leaf size is a value in `crates/page_tables`'s format
   trait. Found by milestone 256's lane.
4. Only the first IO APIC the MADT lists is kept, so half a multi-socket machine's interrupts have
   nowhere to go. Found by milestone 308's lane while writing the `BUGS` entry recording that 308's
   own fix ships unexecuted. Found by milestone 308's lane.

## What it is not

**Not one refactor of the scan.** The four touch different code and could land in any order. The
cluster is a briefing unit, not an implementation plan, and a lane that does two and reports is doing
it right.

Not blocked on a bigger machine, with one honest exception. Part 4 has no local witness: no
machine here has two IO APICs, so it is built against a QEMU `q35` configured with more than one, or
not at all. That is stated in its proposal and is the reason it is last here.

## Index row

The PCI and interrupt discovery path reads more than it programs, and four lanes each found a
different instance while doing something else. They are an unclaimed function behind the IOMMU
refused per request rather than confined once, a bridge window nothing programs, and a BAR window
sized by a constant instead of by the bus. Only the first IO APIC of a multi-socket machine is kept.
Every one is
reachable from QEMU configurations this tree already runs, so none waits on hardware; they wait on
somebody looking. This is `design/fatal-risks/README.md` risk 9's family, found before the machine that
would punish them rather than after.
