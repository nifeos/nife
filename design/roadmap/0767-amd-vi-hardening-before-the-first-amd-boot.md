---
status: BUILT
raised: 2026-10-05
built: 2026-10-05
promoted_from: amd-vi-hardening-before-the-first-amd-boot
milestone_dependencies: 764
decision_dependencies: 20
machine_requirements: an AMD machine with an IOMMU, to confirm on silicon; QEMU for the rest
specific_machine: none
needs_person: no
---
# 767. AMD-Vi hardening before the first AMD boot

Raised by milestone 633 (an outside agent attacks the confinement claim)'s second pass; calef
approved it. *(Title, slug and every name below are drafts.)*

Milestone 764 (AMD-Vi confines device DMA on x86-64) built the driver. The second outsider pass
read it as a confinement boundary and recorded five gaps in `kernel/src/arch/x86_64/amd_vi.rs`'s
`BUGS`. Three are things a first AMD board would meet on its first boot, and QEMU models none of
the firmware state behind them. This milestone is those three, so the first AMD boot is not also
the first time they are discovered.

Reuse: the VT-d driver's `VTD_PERMITTED_BITS` literal and its Kani harness are the model for
the entry proof. QEMU's `pci-bridge` device makes the alias item testable before a board. Linux's
AMD-Vi driver clears the exclusion registers at init, the prior art for item 1 (from memory, not
re-read). Nothing external is taken.

## Acceptance items

1. Clear the firmware exclusion range. `set_up` writes the Exclusion Base and Limit registers to
   zero before enabling the unit, and the boot line prints what it found.
2. Confine and quarantine through aliases. Two devices sharing a source id never share a domain
   silently, and a quarantine resets every entry the device's DMA can arrive under.
3. Honor an IVMD's read-only bit. A DMA mapping carries the rights its source asks for, rather
   than always read-write.

## What it does

Every format is from AMD document 48882 revision 2.62, read for this work.

1. `clear_exclusion_range` zeroes MMIO 0020h and 0028h (section 3.3.1) with the unit off. `Unit`
   has a required field only that function produces, so a `set_up` without the clear does not
   compile. The boot line ends `no firmware exclusion range`, or prints the range and `CLEARED`.
   An IVMD flagged as an exclusion range is still mapped read-write into its devices' domains.
2. Each unit records which device holds each alias entry. An attach whose alias another device
   holds blocks every entry either device's DMA arrives under and prints both ids in capitals.
   `quarantine` blocks the alias too.
3. `DmaRegion` gains a required `writable` field. An IVMD without `IW` maps read-only, and so does
   the virtio shadow page on every architecture, since the device only reads it.

It also did the cheap fifth item. The device table entry builders moved to `page_tables::AmdVi`, refuse
a root the field cannot hold instead of masking it, and carry a Kani proof over every root and
domain. A correction came with it: the driver cited its registers as section 3.4, which revision
2.62 does not have. They are section 3.3.1, by offset.

## Proof

On patagonia, QEMU 11.1.1, 2026-10-05 (UTC). Each test below fails under a replayable patch in
`kernel/falsifications/` or `crates/page_tables/falsifications/`:

| Test | Machine | The defect its patch restores |
|---|---|---|
| `amd_vi::tests::a_firmware_exclusion_range_is_cleared` | `NIFE_IOMMU=amd` | the clear writes nothing |
| `amd_vi::tests::two_devices_behind_one_alias_never_share_a_domain` | `NIFE_IOMMU=amd` | the conflict check is skipped |
| `amd_vi::tests::a_quarantine_blocks_the_alias_too` | `NIFE_IOMMU=amd` | quarantine skips the alias |
| `virtio::tests::a_device_write_to_the_read_only_shadow_is_refused` | all three architectures | the shadow is writable |
| `page_tables::x86_64::no_amd_vi_device_table_entry_sets_a_reserved_bit` (Kani) | any host | the root is masked, not refused |

The shadow patch was also measured failing on the VT-d machine and on aarch64's SMMUv3. Host tests
cover `UnityRegion::device_may_write` and a read-only region on all four DMA formats.

What QEMU decides is written beside each test. It drops writes to the Exclusion Base register and
consults neither exclusion register, so only the limit half and the post-boot zeros are proved
here. Its runner's bridge is empty, so the alias tests read the device table rather than a device.

## Follow-on

- **Milestone 764.** The first boot on AMD silicon, still outstanding there, is where items 1 and 2
  meet real firmware; notes/amd-vi.md's "What a first AMD board should show" now lists both lines.
- **Recorded.** Production revocation (`confine` has no inverse at the seam) and the half of the
  exclusion clear QEMU cannot show are in `kernel/src/arch/x86_64/amd_vi.rs`'s BUGS.
- **Recorded.** Two devices behind one alias are both blocked rather than grouped, in
  `kernel/src/arch/x86_64/amd_vi.rs`'s BUGS.

## BUGS

- A write-only IVMD is mapped readable too, because no DMA format here says write without read.
- NVMe, xHCI and e1000e grants stay read-write throughout, though some of their queues are
  device-read only.

## Index row

An AMD machine's firmware can leave a hole in the IOMMU (an exclusion range), alias two devices onto
one table entry, or ask for memory to be read-only, and the AMD-Vi driver honored none of it. This
closes all three before the first AMD board boots.
