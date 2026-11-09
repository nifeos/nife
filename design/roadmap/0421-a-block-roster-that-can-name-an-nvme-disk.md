---
status: BUILT
raised: 2026-09-17
built: 2026-10-18
promoted_from: a-block-roster-that-can-name-an-nvme-disk
milestone_dependencies: none
decision_dependencies: 193
machine_requirements: none
specific_machine: none
needs_person: no
---
# 421. The block roster names an NVMe disk

`block_roster_contract` is the read-only page a process holds when it may know which block devices exist
without holding any of them (milestone 57 (partitioning and formatting a real drive),
`notes/block-devices.md`). It lists NVMe controllers beside the virtio disks, so `disk_surveyor`
sees the one disk in this tree whose driver is a confined process (milestone 261 (the NVMe driver
leaves the kernel)).

## What is built

- The kinds, as §193 (what a block-roster entry calls an NVMe disk) rules.
  `TRANSPORT_VIRTIO_MEMORY_MAPPED_IO = 0`, `TRANSPORT_VIRTIO_PCI = 1` and
  `TRANSPORT_NON_VOLATILE_MEMORY_EXPRESS = 2`, in `crates/block_roster_contract`. An entry is 8 bytes for
  every kind: an ordinal and a kind. A host test pins the three wire values as literals.
- The printed names. `transport_name` prints `virtio-mmio`, `virtio-pci` and `nvme`. `nvme`
  is what QEMU calls the device model (`-device nvme`), what Linux calls the driver and its
  device nodes, and what this tree's own NVMe diagnostics already print (`diag nvme ...`,
  `disk-throughput`'s preflight).
- The entry. `kernel/src/user/disk_service.rs` lists NVMe controllers after every virtio
  disk, from `pci::count_non_volatile_memory_express_controllers`. That count reads config space
  and nothing else, for the reason the virtio-pci count does: `pci::find_nvme_device` places BARs
  and enables bus mastering, and bring-up resets the controller, none of which a listing may
  cause. Both walk the bus through one function, so the controller
  `non_volatile_memory_express_service` serves is always the roster's NVMe ordinal 0.
- The listing. `disk_surveyor` reports one count per kind (virtio-mmio, virtio-pci, nvme). The
  kernel side compares their sum with what it wrote, so an entry of a kind the surveyor does not
  count still shows.
- The gate. `disk_tests::the_disk_surveyor_reads_a_table_gptfdisk_wrote` asserts exactly one
  NVMe entry, last in the roster, at ordinal 0 and printed `nvme`, whenever `NIFE_NVME` was set
  for the build. `cargo xtask test` sets it on all three legs. `system_tests/build.rs` bakes it in
  as `NON_VOLATILE_MEMORY_EXPRESS_ATTACHED`, since the guest cannot read the host's environment.
  So any leg where the block roster cannot name an NVMe disk that is attached goes red.

## What it settles

Milestone 57's claim is that listing the disks and holding one are different powers. It now holds
for two kinds of disk behind two drivers, one in the kernel's virtio path and one a confined EL0
server. The roster's shape is about block devices, not about virtio.

## Follow-on

- **Recorded.** No program is handed the NVMe endpoint because the roster lists it, and the FS
  server's default disk is still virtio-blk. Which program holds which device is decided at wiring
  time (§86 (whether an NVMe driver can leave the kernel), milestone 57). `notes/non-volatile-memory-express.md`'s `BUGS` says so beside the
  driver.
- **Decision.** An NVMe ordinal counts controllers, which identifies a disk only while each
  controller has one namespace, as every controller this tree drives does.
  `design/decisions/0193-nvme-in-the-block-roster.md` names a controller with two namespaces as the
  measurement that would make an entry say which.

## Index row

`block_roster_contract`, the read-only page that says which disks exist without granting any, lists NVMe
controllers beside virtio disks under `TRANSPORT_NON_VOLATILE_MEMORY_EXPRESS = 2`, printed `nvme`,
with the two virtio kinds named `TRANSPORT_VIRTIO_MEMORY_MAPPED_IO` and `TRANSPORT_VIRTIO_PCI`
(§193). The kernel counts controllers from config space alone, so a listing never brings one up,
and ordinal 0 is the controller the EL0 NVMe server drives. `disk_surveyor` reports a count per
kind, and the roster test fails on any leg where `NIFE_NVME` is set and the NVMe entry is missing.
