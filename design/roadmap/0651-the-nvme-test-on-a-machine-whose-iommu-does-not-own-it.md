---
status: NOT-STARTED
raised: 2026-09-24
promoted_from: the-nvme-test-on-a-machine-whose-iommu-does-not-own-it
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 651. The NVMe boot test on a machine whose IOMMU does not own the controller

*Title and slug are drafts.*
Raised by milestone 261 (the NVMe driver leaves the kernel)'s bench rehearsal
(`notes/risk-6-bench-evening.md`). Name provisional.

Everything it needs exists under QEMU.

## What it is

The NVMe boot test asserts `confined_by_iommu`, which is `iommu::scope_of(rid).is_confining()`,
not `iommu::is_active()` (true whenever any IOMMU unit was translating, whoever owned the
controller). Nothing yet shows the assertion can go red: every leg the suite
runs has one unit that owns the whole bus.

`cargo xtask disk-throughput --case bypass` already builds the machine that would show it: `-machine
default_bus_bypass_iommu=on` under OVMF makes QEMU's DMAR name no unit for the root bus, and the
bench boot then prints `preflight 1/2 dmar scope : FAIL`. The work is to boot the ordinary test
kernel on that machine, run only the NVMe case (`NIFE_TEST_FILTER`), and require it to fail on the
`confined_by_iommu` assertion. Record that as the claim's replayable falsification, in the convention
of milestone 202 (every confinement test is a ritual until somebody breaks the confinement), at
`notes/confinement-claims.md`.

## Why it is worth a milestone

The previous assertion would have stayed green on that machine, and very likely on xenon. A
confinement test that cannot come back red is the failure risk 7's entry already found three times.

## Index row

The NVMe boot test asserts the controller is confined by the IOMMU, but no leg can make that assertion fail. Proposed: boot the test kernel on a QEMU machine whose DMAR names no unit for the root bus, so the assertion is seen to go red.
