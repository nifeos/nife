# AMD-Vi: confining DMA on an AMD machine

*Name: provisional (lane `amd-vi`, provisional milestone 764 (AMD-Vi confines device DMA on
x86_64)). An architect names things.*

An AMD PC has no DMAR. Until this lane the x86_64 kernel looked only for one, printed `skipped, no
DMAR`, and ran every driver unconfined: the NVMe driver tracked the fact honestly, the xHCI driver
refused to start, and nothing else noticed. AMD's IOMMU is AMD-Vi, described by the ACPI IVRS. This
note is how the kernel drives it, what QEMU does differently from the specification, and what was
proved where. The code is `kernel/src/arch/x86_64/amd_vi.rs`; its header and BUGS are the detail.

Every format here is from AMD document 48882 revision 2.62 (February 2015), read for this work. That
revision predates several features (newer IVHD types, a 5-level walk on more parts), none of which
this driver uses. QEMU's model is `hw/i386/amd_iommu.c` at QEMU 11.1.1, the version `.qemu-version`
pinned when it was read. The pin moved to 11.1.2 on 2026-10-10 (UTC), and the model was not re-read.

## How it fits behind the seam VT-d already uses

The four confine callers (NVMe, e1000e, xHCI, virtio) call `crate::iommu::confine` and
`crate::iommu::scope_of`, and nothing in them changed. `kernel/src/arch/x86_64/iommu.rs` is still
the module the seam reaches; each public entry point there asks `amd_vi::is_active()` first and
hands the call over. The file was not split into a dispatcher and a VT-d driver because its path is
cited across `design/` as where x86's IOMMU lives, and a lane may not edit those citations.

Two seam changes were forced by the hardware, and both reach the other architectures:

- **The DMA page-table format is chosen at run time,** not fixed per architecture. An x86 machine
  has VT-d or AMD-Vi and they walk different formats, so each architecture's `iommu::build_domain`
  names its format, and x86's asks which unit is up.
- **`PageFormat::table_entry` takes the level.** An AMD-Vi directory entry carries `NextLevel`
  (bits 11:9), the level of the table it points at. There is no level-free encoding of it, and a
  wrong value is a skipped level rather than a typo. `paging::x86_64::AmdVi` is the format.

## The device table, and the one entry that denies everything

AMD-Vi is driven through memory: one 256-bit entry per 16-bit device id, a command ring, an event
log. `init` fills every entry with the blocked entry before setting `IommuEn`: valid, translating,
four levels, rooted at one all-zero table, read and write both clear. A device nobody confined walks
to a not-present entry, is target-aborted, and is logged. The table is sized to the highest device id
the IVRS names; the hardware target-aborts any id past its end (section 2.2.2).

The blocked entry is also §163 (where a confined device's IOMMU fault is delivered)'s quarantine:
`amd_vi::quarantine` writes it. Milestone 102 (what a confined device's fault reaches) is the
caller, on every architecture.

Table 7 offers a simpler blocking entry, `Mode` 0 with `IR` = `IW` = 0. It was refused because
QEMU does not implement it (next section), and an entry that blocks on silicon and passes everything
under QEMU is a default-deny claim no test here could check.

No cache maintenance is needed, unlike VT-d on a unit with `ECAP.C` clear. Device-table reads are
snooped while `Control.Coherent` is 1, its reset value. Page-table walks are snooped while a
device's `SD` bit is 0, and the command ring is always read coherently.

## Where QEMU 11.1.1 differs from the specification

Each of these was read in QEMU's source, and the last two were found by a test failing.

| What | The specification | QEMU 11.1.1 | What this tree does |
|---|---|---|---|
| Translation at all | always, once `IommuEn` is set | only with `-device amd-iommu,dma-remap=on`; otherwise every device bypasses the unit | the runner sets it |
| When a device starts being translated | at enable | only after `INVALIDATE_DEVTAB_ENTRY` names it | `init` invalidates every entry, as Linux does |
| `Mode` 0 entry | access controlled by `IR`/`IW` | pass-through, `IR`/`IW` ignored | the blocked entry uses a four-level walk |
| Translation cache | tagged by domain id | keyed by device id and page | changing an entry flushes the domain it leaves |
| Event record address | the faulting address | always zero in this build (a shift of 64, fixed upstream in 4adfb431c0, not in 11.1.2) | `Fault::addr` is `None`; tests use the requester id |
| Event record device id | `bus:dev.fn` | `devfn` only | harmless while every QEMU device is on bus 0 |

**The stale translation was a real escape under QEMU.** The virtio escape test failed on its first
AMD-Vi run with a fault at address 0. QEMU's own trace showed why. An earlier registration of the same disk had mapped a frame that the
allocator later handed to the test as its victim, and QEMU's cache still held that mapping under the
device's id. The device read the victim; the
fault at 0 was the next access. Section 2.4.2 says `INVALIDATE_DEVTAB_ENTRY` does not flush
translations and that software should flush the domain, and the driver now flushes the one a device
leaves on every change to its entry. On silicon the stale entry would have been unreachable (the cache
is tagged by domain), but it would still have been sitting there.

## What was proved, and where

All on patagonia, QEMU 11.1.1, `q35` under TCG. `NIFE_IOMMU=amd` (provisional) is the runner knob.

| Boot | Result |
|---|---|
| `script/test --arch x86_64`, which now also boots both test images on the AMD-Vi machine | kernel suite 129 passed, 5 skipped; system suite 237 passed, 44 skipped, the same 44 the VT-d machine skips |
| the virtio DMA-escape test, AMD-Vi | passes |
| the NVMe DMA-escape test (`run_dma_escape`, `confinement_attackers`), AMD-Vi | passes: fault for the controller, canary intact |
| the same suite on the default (VT-d) machine | unchanged |
| the boot tour with `NIFE_IOMMU=none` | boots, and says `iommu : NONE: ... no device's DMA is confined` |
| `amd_vi::tests`, AMD-Vi, provisional milestone 767 (AMD-Vi hardening before the first AMD boot) | the exclusion registers read zero after boot and a range the test sets is cleared; two ids behind the runner's `pci-bridge` never share its alias; a quarantine blocks the alias too |
| `virtio::tests::a_device_write_to_the_read_only_shadow_is_refused`, every IOMMU | passes: the device's write into its shadow page does not land |

The falsification turns `translating_dte`'s `Mode` from 4 to 0, keeping `IR` and `IW`: the
device reaches everything, untranslated. With it applied, on the AMD-Vi machine:

- `virtio::tests::the_iommu_faults_a_dma_that_escapes_the_domain` fails with "the IOMMU recorded no
  fault". This is the replayable record,
  `kernel/falsifications/virtio.tests.the_iommu_faults_a_dma_that_escapes_the_domain.patch`, and the
  first to carry an `Environment:` line, which `script/falsifications` now reads.
- the NVMe escape test fails with "the IOMMU recorded no fault", both escapes completing with status
  0. Attested 2026-10-05, not replayable: that test already carries its one record, which widens the
  domain and works on every unit.

## The exclusion range and aliases

An exclusion range is a window of device addresses the unit does not translate. With `Allow` set
in the Exclusion Base register, every device's access inside `[base, limit]` goes straight to
memory, untranslated and unchecked, whatever its device table entry says (section 3.3.1, MMIO 0020h
and 0028h; section 2.1.4.4 for translation requests into the range). Firmware sets one to keep a
region it DMAs into reachable, and nothing clears it at reset. A range left in force would be a
hole in the default deny that no entry the driver writes can close. So `clear_exclusion_range`
zeroes both registers, `ExEn` and `Allow` with them, and `Unit` cannot be built without the record
it returns. A range the firmware still needs is not lost: an IVMD with `ExclusionRange` set is
mapped into the named devices' domains as a read-write identity region instead, as Linux does. A
range set with no IVMD naming it is firmware's alone, and the boot line says it was cleared, in
capitals.

QEMU 11.1.1 stores the limit register and drops every write to the base register
(`amdvi_mmio_write` has no case for MMIO 0020h), and no walk consults either. Its firmware sets no
range. So under QEMU the test proves the limit half of the programming and that the boot left both
registers zero. The base half and the effect on a device's DMA are provable only on silicon. The
range the test writes is the last 4 KiB of the 52-bit address space, which no machine backs with
memory, so on silicon the moment between the write and the clear excludes nothing.

A device behind a PCIe-to-PCI bridge is translated under its alias's id, so `attach` writes the
device's own entry and its alias's, as Linux does (`clone_aliases`, from memory). Two functions
behind one bridge reach the unit under one id, so no pair of entries can confine them apart. The
second attach therefore fails closed: every entry either device's DMA arrives under is set to the
blocked entry, and the boot prints the pair in capitals. Both devices stop doing DMA. Giving such a
pair one shared domain on purpose would be an alias group, which is recorded below.

## What a first AMD board should show

A boot on silicon is the outstanding piece. Read these lines:

- `amd-vi unit at ...` in the ACPI tour, with the IVHD type (10h, 11h or 40h) the kernel chose.
- `amd-vi : unit ... up`, with the device-table size. A Ryzen part's IVRS usually carries an "all"
  entry or wide ranges, so expect 65,536 entries (2 MiB of contiguous frames).
- whether the unit sets `EFR.IASup`. Without it `init` sends 65,536 `INVALIDATE_IOMMU_PAGES`, one
  per domain id, which is correct and slow; the time is unmeasured.
- any IVMD lines. None has been seen; the IVMD path has run zero times.
- the NVMe line saying it is confined, and the escape test, which on silicon reports a real address.
- the end of the `up` line: `no firmware exclusion range`, or the range firmware left and the word
  `CLEARED`. A cleared range that a device still needed shows up as that device's faults.
- any `BOTH REACH THE UNIT AS` line: two devices behind one PCIe-to-PCI bridge, both blocked.

## BUGS

- Nothing has run on AMD silicon.
- Interrupt remapping is never enabled (every entry leaves `IV` clear), the same posture as VT-d.
- No fault interrupt; `take_fault` is drained only by tests. Milestone 102 owns it.
- `notes/confinement-claims.md` has no AMD-Vi row, because that note is over its word budget and a
  row would grow it. This note is where the AMD-Vi claims are recorded until it is trimmed.
- Milestone 633 (an outside agent attacks the confinement claim)'s second pass recorded five gaps
  in `amd_vi.rs`'s BUGS. Milestone 767 (design/roadmap/0767-amd-vi-hardening-before-the-first-amd-boot.md)
  closed four: the exclusion range, aliases, read-only mappings and the entry proof. Production
  revocation stays open, with what QEMU could not show of the other four; the driver's list below
  has both. The claims the row should make are proposed in notes/confinement-outsider-pass-2.md.

### The driver's BUGS

- QEMU confines nothing without `dma-remap=on` (the runner sets it), writes no address into event
  records (`take_fault` reports `None`), and records `devfn` without the bus.
- At enable, a stale cached entry can be used until the invalidations run, as on Linux.
- No fault interrupt: only tests drain the event log, until milestone 102 (what a confined device's
  fault reaches).
- A type 20h IVMD is added to every domain but pre-attaches no device; no IVMD has ever run.
- Domain ids are never reused; past 65,535 attaches a boot panics.
- Nothing revokes a device's domain in production, and a re-attach leaks the old tables
  (`kernel/src/iommu.rs`), the same gap every IOMMU driver here has at the seam.
- The exclusion clear is half proved under QEMU, which ignores those registers.
- Two devices behind one alias cannot both work: the second attach blocks the alias, loudly.
  `attach` assumes an alias is served by its device's unit and tracks at most `MAX_ALIASES` per
  unit, failing closed past that.
- The root is checked against the 52-bit field, not the unit's real address width.
