---
status: DECIDED
raised: 2026-10-07
decided: 2026-10-07
ratified_by: calef
---

# 260. Radon's block server serves a USB drive

*Raised for milestone 53 (the board's own peripherals: network and storage on real silicon).*

## The ruling

Milestone 53 owed an architect's call: which part of radon's storage the booted system's block
server serves. It is a fact that whatever writes the storage and the kernel must agree on. calef
ruled on 2026-10-07 (UTC): a USB drive.

> Realistically for using radon the future I will want D because that's what I have and that's
> what I would use. So lets go with USB even though it is more work. I need that work done anyways.

## The options

| | Option | Outcome |
|---|---|---|
| A | A second partition on the microSD card, beside the FAT one U-Boot boots from | Refused. The shortest path, but not what a user would run. |
| B | The eMMC socket | Refused. No module is known to be fitted. |
| C | NVMe in the M.2 slot | Refused for now. calef has USB drives, not an NVMe one. It stays possible later on the same root complex. |
| D | A USB drive | **Chosen.** |

## What D costs, accepted knowingly

On radon the USB 3 ports sit behind a VL805 controller on PCIe (notes/visionfive2.md), so the
block server cannot reach a drive until three things exist:

1. Milestone 163 (the JH7110's PCIe root complex), still NOT-STARTED on a hardware gate.
2. An xHCI driver. Milestone 242 (USB host and HID) is building one for xenon, and radon shares it.
3. A USB mass-storage driver (bulk-only transport carrying SCSI), which no milestone covered. It is
   milestone 859 (a USB mass-storage driver), so that radon's storage
   has an owner.

The microSD bench step in notes/designware-mobile-storage.md still stands. It proves the MSHC
driver on silicon. It is no longer the block server's target, and `PROVEN_ON_SILICON` for that
driver does not make the booted system start it.

## What this supersedes

The storage-ordering half of the 2026-08-15 "NVMe first" ruling. Its reason was the backup
workload (milestone 55 (Time Machine: SMB3 with Apple's extensions, and mDNS)), which was removed on 2026-08-30. The other half of that ruling, that a
real PCIe root-complex driver compounds into milestone 87 (the x86_64 bare-metal machine), survives: D needs the
same root complex.

## Consequences

- Milestone 53's storage-target item is done, and its dependencies name 163 and 242. The new
  proposal is not a numbered milestone, so it is named in prose until the integrator mints it.
- Milestones 163 and 242 carry a note that radon's storage now depends on them.
- The booted system's block server on radon stays unstarted until a USB drive can be read.
