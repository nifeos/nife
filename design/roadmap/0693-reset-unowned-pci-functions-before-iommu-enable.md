---
status: NOT-STARTED
raised: 2026-09-27
promoted_from: reset-unowned-pci-functions-before-iommu-enable
milestone_dependencies: 612
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 693. Should the kernel clear Bus Master Enable on every PCI function it does not own, before the IOMMU turns on?

*Title and slug are drafts.*
Raised 2026-09-27 (UTC), a follow-on from milestone 612 (the IOMMU escape-fault test could lose its
fault to an unconfined neighbor disk). That milestone found and fixed a symptom: an unconfined,
never-registered second virtio-blk-pci disk could occupy the VT-d unit's one fault-recording
register and starve a test's own fault. calef asked whether the cause is QEMU-specific or a real
production gap, and to check cheaply before writing anything.

## The check

A read-only PCI config-space read (no `bring_up`, no write of any kind) of every virtio-blk
function's `COMMAND` register. It is the very first thing this kernel's test suite does on
`x86_64`, before this boot calls `bring_up` (which sets Bus Master Enable) on anything:

```
DIAG: virtio-blk at 00:03.0 COMMAND=0x0107 (bus-master true, memory-space true) before this kernel brings anything up
DIAG: virtio-blk at 00:04.0 COMMAND=0x0107 (bus-master true, memory-space true) before this kernel brings anything up
```

Bus Master Enable is already set on both functions before this kernel ever touches them.
Milestone 612's "QEMU device-model quirk, not chased into QEMU's own source" framing was
premature. The premise it should have checked and did not: whether the neighbor disk's DMA came
from something QEMU's virtio-blk-pci model does spontaneously, or from a function that was already
allowed to master the bus before the guest ran. It is the second one. This does not need to be a
QEMU peculiarity at all. Any firmware that probes boot disks (SeaBIOS, OVMF, or on real hardware a
disk controller's own option ROM) sets Bus Master Enable as part of that probe. Nothing turns
it back off before a kernel that never asked for that device takes over the machine. `xenon` boots
through real firmware; `nife` there never registers most of what it finds, and by this same
mechanism, some of what firmware touched can still be bus-mastering when the IOMMU turns on.

## The production question

`arch::iommu::init` brings VT-d up with an all-invalid root table (default-deny for translation:
a device with no context entry faults). It does not touch Bus Master Enable on any function. A
device that firmware left driving DMA, with BME already set and a queue firmware configured, can
therefore attempt real transactions the moment translation turns on, and (milestone 612) can occupy
`CAP.NFR`'s one fault-recording register indefinitely. This is not a confinement bypass: default-
deny still blocks every one of that device's addresses. It is a fault-*visibility* problem, and it
would sit in the way of any future code that reads faults expecting to see its own device's, the
same way it sat in the way of milestone 612's test.

Should the kernel clear Bus Master Enable (`COMMAND` bit 2) on every PCI function it does not
immediately confine, as part of bringing the IOMMU up, before `GCMD.TE`? `pci::bring_up` already
sets BME as its own last step, deliberately ("DMA permission is granted at the final moment, after
the transport the confinement layer owns is fully described"): the kernel already agrees that BME
should only be on for a device this kernel drives. The gap is only ever OTHER functions, the ones
this kernel never brings up at all.

## What the tree already does in the analogous case

This driver's "A device no unit owns" BUGS entry (`kernel/src/arch/x86_64/iommu.rs`) already faces
a related question and answers it the way Linux does: a device no DRHD scope names is left alone,
DMA untranslated, matching `intel_iommu_probe_device`'s `-ENODEV`. Linux does not blanket-clear BME
for devices outside IOMMU scope either. That is a *narrower* question than this one (no scope at
all, vs. an owned function nobody has driven yet). It is still the closest precedent in this tree
and in the driver Linux ships, and it points the same way. Clearing BME universally would be new
ground, not a pattern this tree or its nearest prior art already follows.

## Options

- **A. Clear BME for every PCI function this kernel does not immediately bring up**, in
  `arch::iommu::init` or the PCI bus-walk beside it (`pci::bar_census`/`pci::survey` already read
  every function on the bus this way, read-only; this would add one write per function). Cheap and
  reversible as code. The risk is not reversibility, it is correctness on real hardware. A function
  mid-transaction when firmware hands off (a legacy USB controller under SMM "USB legacy support",
  an NVMe or AHCI controller firmware left initializing) may not tolerate having its bus mastering
  pulled out from under it. That failure mode is a hang or a chipset-level SMI, not a clean
  fault. This wants a real-hardware test (xenon) before it ships, not just a QEMU boot.
- **B. Function-Level Reset (FLR)** on every unowned function that advertises the capability,
  going further than BME alone. Same shape as A, strictly more invasive, same real-hardware risk,
  and adds a capability-list walk this driver does not do today.
- **C. Leave it recorded.** Milestone 612's fix (reset every other *block* device the test itself
  cares about) already closes the test's own symptom. This option does nothing for a production
  boot where firmware left an unrelated device bus-mastering; it only says so, here, until an
  architect decides the tradeoff in A or B is worth it.

No recommendation is made among these. A and B are irreversible in the sense that matters here, not
because the code is hard to revert. A real machine's firmware handoff is not something a lane can
safely test without the actual hardware, and a wrong default could turn a diagnostic gap into a
boot hang. This is an architect's call.

## What is blocked until this is answered

Nothing. Milestone 612 is BUILT and does not depend on this being decided. This is filed so the
gap is on record rather than rediscovered.

## Index row

An unowned, never-registered PCI function can occupy the VT-d fault register and starve a test's own fault. Proposed: decide whether the kernel clears Bus Master Enable on every function it does not own before the IOMMU turns on.
