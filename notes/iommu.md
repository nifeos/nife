# Confining DMA with an IOMMU

notes/dma.md closed the DMA hole in software: the kernel validates every descriptor and the device
reads a shadow copy the driver cannot touch. This note is the hardware version, milestone 16b: an
IOMMU that confines the device generically, with no block-device or even transport knowledge, on
both architectures behind one seam. The shadow ring stays, demoted to defense in depth.

## What an IOMMU is, and why it is the clean answer

The MMU confines a process: a thread at EL0/U-mode cannot touch a byte it was not mapped. But a
device is a second bus master. It reads descriptors and does DMA against raw *physical* addresses,
and page-table permissions do not apply to it (notes/dma.md spells this out). An IOMMU is the MMU
for devices: it sits between the device and memory and translates every address the device emits,
through page tables the kernel programs, confining the device to exactly the frames in those tables.
An address the device was never granted has no mapping, so the IOMMU faults instead of letting the
DMA through. It is generic: the kernel needs to know nothing about what the device is, only which
frames it may reach.

## The seam: one domain builder, two page-table formats

The payoff of the format-generic `paging` crate (DECISIONS §17) shows up a second time here. An
IOMMU walks the *CPU's own* page-table format: the aarch64 SMMUv3 walks VMSAv8-64, the ratified
RISC-V IOMMU (v1.0.1) walks Sv39. Those are the same two formats `Mapper` already builds for process
address spaces. So a device's DMA domain is not a new kind of table. It is a `Mapper` filled with an
identity map (IOVA == PA) over exactly the frames the device is allowed to reach:
`paging::domain::build_identity_domain`, host-tested on both formats.

Identity, because the userspace virtio driver already puts *physical* addresses into its descriptors
(its DMA region's frames, and the kernel's shadow page). With an IOMMU in front, the device emits
those same numbers as IOVAs, and the domain translates each to the identical PA. The driver's ABI
does not change, because IOVA == PA means the addresses it computes still name the right memory. The
domain is an allow-list of frames, expressed as a page table.

`kernel/src/iommu.rs` is the portable seam over that builder. `confine(rid, regions)` asks the arch
driver's `build_domain` for a root (it names the format: `Aarch64` here, `Sv39` there, and on
`x86_64` VT-d's or AMD-Vi's, whichever unit the machine has), and hands the root to the arch
driver's `attach`. One call site, every format, which is the whole point. AMD-Vi is
[notes/amd-vi.md](amd-vi.md).

One asymmetry worth stating: single-stage RISC-V translation (`iosatp`, no process context) faults
on a leaf PTE whose U bit is clear, because a device does not "request supervisor privilege." So the
domain is built with `Flags::user_data`, which sets U on Sv39 and read/write on both formats, or
with `Flags::user_rodata` for a region the device may only read (`DmaRegion::writable`, milestone
767 (AMD-Vi hardening before the first AMD boot)). The virtio shadow page is the one such region
today. Neither is executable.

## The two arch drivers, structural twins

Under `arch/` per rule #1: `kernel/src/arch/{aarch64,riscv64}/iommu.rs`. They rhyme deliberately.
Each is driven almost entirely through memory, not registers:

| role | SMMUv3 (aarch64) | RISC-V IOMMU (riscv) |
| --- | --- | --- |
| per-device table, keyed by requester id | stream table (STE) | device directory (device context) |
| the IOMMU's copy of TTBR/`satp` | context descriptor (CD) | device context `fsc`/`iosatp` |
| how config changes take effect | command queue + CMD_SYNC | command queue + IOFENCE.C |
| where faults are reported | event queue | fault queue |

Each driver: `init` installs an all-invalid table and enables translation, so an unattached device
is denied by default. The SMMU needs an explicit `GBPA.ABORT` for the pre-enable window. The RISC-V
IOMMU's reset `ddtp` mode is already `Off`, which blocks, so it fails closed by architecture.
`attach`
writes the device's table entry pointing at a domain root, then invalidates the cached STE/CD/context
and the translation cache so a re-attach cannot use a stale table. `take_fault` pops one fault record.

## The requester id is the key

A PCIe function stamps a 16-bit requester id (`bus:8 | dev:5 | fn:3`) on every memory transaction
it issues (`pci::Bdf::requester_id`). Both `virt` boards publish an identity `iommu-map` in the
device tree, so that id is exactly what the IOMMU looks a device up by (SMMU StreamID, RISC-V
`device_id`). It is threaded from `pci::find_block_device` through `virtio::register`, which gained an
`Option<u32>` argument: `Some(rid)` for a PCI device, `None` for virtio-mmio (no IOMMU fronts the
mmio bus on either board). `confine` runs inside `register`, before the device is entered in the
transport table and before it is ever rung, so the domain is installed the moment the device could
DMA. The regions a virtio device's domain covers are its DMA region (data buffers, used ring) and the
kernel's shadow page (the descriptor table and available ring the device reads).

## Discovery differs by arch; the rest is portable

The SMMUv3 is a device-tree platform node, found in `memory::smmu_region` and mapped by `mmu::init`;
`arch::iommu::init(base)` is called from the aarch64 boot when the node is present. The RISC-V IOMMU
is itself a PCI function (`riscv-iommu-pci`, Red Hat 1b36:0014), so `pci::init_iommu` enumerates it.
It places its BAR from a now-shared cursor (two functions need BARs placed, so the cursor can no
longer reset per device), and hands the base to `arch::iommu::init`. The BAR-placement change is the
one
piece of PCI plumbing this milestone touched.

## Making the machine tell the truth: the confinement test

`iommu_platform=on` is what puts a virtio-pci device behind the IOMMU. The device then emits IOVAs
the IOMMU translates, and offers VIRTIO_F_ACCESS_PLATFORM (feature bit 33). The driver negotiates it
only when offered (so one driver binary works on the bare mmio disk and the IOMMU-fronted PCIe disk).
A device *without* the flag silently bypasses translation. That is the same manufactured-fact hazard
the runners already fail loudly on for a missing disk: a plausible-looking success that is actually a
configuration gap.

So the confinement is not trusted, it is proven. `the_iommu_faults_a_dma_that_escapes_the_domain`
(kernel/src/virtio.rs, runs on both ISAs) confines the real PCIe disk, then points its available ring
at a frame the domain does not map and kicks it. The device's first act on the kick is to read the
available ring, at an out-of-domain IOVA, so the IOMMU faults and records it; the test drains the
fault queue and asserts a fault at that exact frame. If translation were absent (a missing
`iommu=smmuv3` / `riscv-iommu-pci`, or a dropped `iommu_platform=on`), the escaping read would succeed
and no fault would appear, so the assertion fails rather than passing on a fiction. The test goes
around the software shadow ring deliberately. The shadow ring would refuse an out-of-region
descriptor before the device was ever rung, so a normal path can never reach the hardware. The point
is to prove the hardware would stop the device even if that software were bypassed.

## QEMU vs ours

The RISC-V IOMMU emulation is newer than the SMMUv3's, so the honest record notes which is which:
during this build both behaved exactly as their specs describe, and no bug (QEMU's or ours) surfaced.
The full existing suite runs behind the IOMMU on both boards: the disk read, both DMA-escape attacker
tests, and the new hardware-fault test (aarch64 118 kernel tests, riscv 60).

## Honest limits

- **QEMU tier only.** Silicon carries the riscv driver over when a board ships the ratified spec, the
  emulate-then-carry pattern the kernel was built on. Parity is claimed at the QEMU tier.
- **Frame-granular.** The domain is an identity map over whole frames, so it cannot confine below a
  page. That is fine for a device's DMA region, which is page-aligned by construction.
- **No fault handler yet.** `take_fault` is drained by the confinement test; routing IOMMU faults to
  a handler in a production boot is future work.
- **Not free even when you have one.** Someone still programs the domain that confines the device,
  and that someone is the kernel. The IOMMU buys generality (no transport knowledge needed), not the
  absence of a trusted DMA policy. The shadow ring (notes/dma.md) stays as defense in depth: the
  transport still refuses a format it cannot police, so a regression in the IOMMU path is caught by
  the software layer and vice versa.

## What is proved, as against tested (milestone 255)

The confinement test above proves the hardware stops an escaping device. It cannot prove the kernel
wrote the *right* entry, because a wrong entry that still confines this device on this board is
invisible to it, and this board is the only one either driver has ever run on.

`arch/aarch64/iommu.rs` now carries two Kani harnesses over the entry-building arithmetic, beside
the code, and they were falsified before they were believed. One is
`the_smmu_is_handed_exactly_the_tables_the_kernel_built` (both physical addresses survive their
split across two 32-bit words, and neither reaches the control bits sharing the low word). The other
is
`no_stream_can_reach_another_streams_tables` (the `StreamID` bound is exactly strong enough for the
stride it protects). notes/kernel-proofs.md has the properties and the stub boundary.

Two limits worth naming here rather than only there. **The register offsets and bit constants are
not proved and cannot be**: nothing in this tree can check `CR0_SMMUEN` or the position of `CONFIG`
against Arm IHI 0070. So a misreading of the document makes the code and the proof wrong together,
and the boot-time confinement test is what stands against that. And **the RISC-V IOMMU's proofs are
not ports of these**: it writes its device context in 64-bit stores with no split, so the property
above does not apply to it.

The RISC-V side has its own two since milestone 432 (the RISC-V IOMMU driver has no counterpart to
the SMMU's proofs). They are proved from an aarch64 host because no host here is riscv64
(notes/kernel-proofs.md has how, and the caveat that comes with it).
`the_iommu_is_handed_exactly_the_domain_the_kernel_built`: `iosatp`'s PPN is the table root entire,
its MODE is Sv39 for every root (MODE 0 would be Bare, translation off), and `ta.PSCID` is the
domain's tag and nothing else. `no_device_can_reach_another_devices_context`: the directory bound
agrees with the stride in both context formats, including the 32-byte one that has never run, and
the `IODIR.INVAL_DDT` that follows an attach names exactly that device. Both were falsified; the
patches are in `kernel/falsifications/`. The same limit on constants applies, against the RISC-V
IOMMU specification instead of Arm's.

## The VT-d driver on x86_64

`kernel/src/arch/x86_64/iommu.rs` drives Intel's VT-d. Unlike the SMMUv3 and the RISC-V IOMMU, the
legacy interface every unit supports is driven through registers. Invalidation is a register write
and a poll, and faults land in Fault Recording Registers, with no command or fault queue. The root
table (one entry per bus) points at per-bus context tables (one entry per device and function).
Each context entry names a domain and the second-level root that `crate::iommu::confine` built.
Both tables are allocated lazily and default absent. `init` points `RTADDR_REG` at an all-absent
root before setting `GCMD.TE`, so no device is ever unconstrained by omission.

Every DRHD the DMAR names gets its own root table. `attach` goes to the unit the DMAR says owns the
device (`machine_discovery::acpi::DmarUnits::owner_index`, VT-d 4.1 section 8.3). `GCMD` is written
per section 11.4.4: read `GSTS`, clear the one-shot bits, change one bit. RMRRs are identity-mapped
for their devices before any unit translates, and they stay in every domain those devices join
(section 3.16). So a USB controller's legacy buffers and the GPU's stolen memory are mapped before
anything could fault them. `init` prints one `vt-d` line per unit.

A device no unit owns gets no context entry, and `scope_of` answers `Elsewhere { owner: None, .. }`.
Linux leaves such a device untranslated too. Refusing its bus mastering would confine nothing and
would silence the preflight that reports the gap.

The file is also x86_64's IOMMU front door. Every public entry point first hands the call to
`amd_vi` if an AMD-Vi unit is up. It stays one file because the tree cites this path as where
x86_64's IOMMU lives, §163 (where a confined device's IOMMU fault is delivered) among the citations.

### Cache maintenance on a non-coherent unit

When `ECAP.C` is clear, VT-d 4.1 section 6.1 leaves cache maintenance of the remapping structures to
software (read from the field's definition). QEMU cannot show the difference, because it reads guest
memory directly whatever `C` says. xenon's catch-all unit reports `ecap 0xf050da`, with `C` clear.
Its first fault without the flush was reason 0x01, root entry not present, on the NVMe's admin
queue, while the CPU's view held the root entry: the unit read DRAM the cache had not reached yet.

`Unit::publish` flushes with `clflush` per 64-byte line. The other choice, `wbinvd`, writes back and
invalidates every cache in the package, uninterruptibly, for a few bytes of table. Linux does the
same (`__iommu_flush_cache`, from memory). The caller fences once with `mfence` after its last
flush and before the register write that makes the unit read, because `sfence` does not order
`clflush`.

### One fault register, and its overflow

`take_fault` reads the first Fault Recording Register only; every unit this driver has met reports
one (`CAP.NFR`). An overflow (`FSTS.PFO`) blinds the unit until it is cleared. It was seen when a
device still running a queue from an earlier registration filled the one record. QEMU then set
`PFO` and dropped every later fault, including the one the confinement test provoked, so the test
read a stale record. `take_fault` clears `PFO` first, so the next fault lands.

### BUGS of the VT-d driver

- Only PCI segment 0 is brought up; a unit elsewhere is reported and refused.
- A unit whose scope names only absent devices is still brought up, and `init` panics on its first
  status poll if its register file is not really there. Linux ignores such a unit.
- An RMRR naming a bridge is mapped only once the device below it is confined; until then that
  device's firmware DMA faults.
- The graphics unit translates with only its RMRR mapped, so scanout survives only if the RMRR
  covers it. This is unread on xenon (notes/risk-6-bench-evening.md).
- There is no interrupt remapping. `ECAP.IR` is only reported (`interrupt_remapping_available`), and
  no `IRTE` is ever written (`design/roadmap/0317-interrupt-remapping-flags.md`).
- Invalidation is always global, which is correct and expensive with many devices.
- `RWBF` and protected-memory disabling have never run, because QEMU sets neither.
- One Fault Recording Register per unit is read, shared by every requester. A repeating fault from
  one device can hide another's, and a burst past it is lost (`FSTS.PFO`). A caller reading one
  device's faults must first quiesce every other device on the bus.
