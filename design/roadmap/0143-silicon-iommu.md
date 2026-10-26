---
status: NOT-STARTED
raised: 2026-08-21
milestone_dependencies: none
decision_dependencies: none
machine_requirements: riscv64 silicon with a RISC-V IOMMU (v1.0 or later)
specific_machine: none
needs_person: no
---
# 143. Silicon IOMMU: carrying 16b's driver to a board that ships the ratified spec

The VisionFive 2 (JH7110, 2022-era silicon) predates the ratified RISC-V IOMMU
specification (v1.0.1, ratified 2024) and has no IOMMU at all. This milestone is the carry-over step
that 16b's block always recorded as "a hardware fact nobody can schedule."

**The hardware fact changed on 2026-09-17, and the gate did not.** The survey below, re-run that
day, found a buyable board that ships the ratified spec: the SpacemiT K3, about $299. What now
blocks the milestone is two pieces of our own engineering rather than the silicon, a platform
discovery path beside 16b's PCI one and an APLIC/IMSIC driver. The gate stays HARDWARE only
because nobody has bought the board (milestone 241, `Gate: DECISION`, holds that call). The original
claim, that no such board exists at any price, is withdrawn and kept below as written.

Split out of milestone 16 on 2026-08-20, because 16a (first silicon on the VisionFive 2) is PARTIAL
with lane-sized engineering remaining, while this is an indefinite hardware wait. The two were
bundled under one number since the original milestone was written, and the split makes the hardware
fact honest. 16's remaining work is engineering that can be done now. This is a purchase that has
not been made and cannot be scheduled.

## What this is

16b built IOMMU-backed DMA isolation in QEMU emulation on both ISAs: SMMUv3 on aarch64 and the
ratified RISC-V IOMMU (v1.0.1) on riscv. The portable DMA-domain seam (`crate::iommu` over
`paging::domain`), both arch drivers, boot bring-up, the `iommu_platform=on` enablement with the
confinement test, and the disk and attacker suites all pass behind the IOMMU on QEMU.

This milestone is the carry-over: when a board ships the ratified RISC-V IOMMU spec, 16b's riscv
driver runs on it. The emulate-then-carry pattern is the one the kernel was built on: the driver
was written against the ratified spec in emulation, so the carry is "boot it and fix what the
silicon got wrong," not "write it for the first time."

## What would trigger it

A RISC-V board or SoC that:
1. Ships the ratified RISC-V IOMMU specification (v1.0.1 or later).
2. Exposes it as a PCI function (the QEMU emulation enumerates it this way; the spec allows it).
3. Speaks the rest of the firmware contract the kernel already handles (OpenSBI, SBI HSM, PLIC,
   Sv39 or Sv57).

### The survey, re-run 2026-09-17

The 2026-08-20 answer was "no board at any price." That answer is now wrong on requirement 1 and
still right overall, for a different reason than before. A board exists, it is buyable for about
$299, and what blocks it is our driver's discovery path and our interrupt controller, not the
silicon. Every claim below carries the source that establishes it and the date it was fetched
(2026-09-17 throughout).

The three states are kept apart deliberately, because conflating them is how this question goes
wrong: ratified (implements the RISC-V IOMMU spec's own register interface), vendor
(something the datasheet calls an IOMMU that is not the ratified spec), and **none**. A candidate
whose status could not be established is recorded as **unconfirmed**, which is not a maybe-yes.

| Candidate | IOMMU state | Buyable | Source |
|---|---|---|---|
| **SpacemiT K3** (X100, RVA23) | **Ratified**, as `spacemit,k3-t100` with a `riscv,iommu` fallback | **Yes**, ~$275-$299, multiple vendors | [dt-bindings patch](https://lkml.iu.edu/2602.3/08975.html), [CNX 2026-05-11](https://www.cnx-software.com/2026/05/11/rva23-pico-itx-sbc-spacemit-k3-octa-core-risc-v-ai-soc-up-to-32gb-ram-256gb-ufs/) |
| **SpacemiT V100** (server) | **Ratified**, as `spacemit,v100-t100`, same binding | No: first small-scale cluster deployments late 2026 | [dt-bindings patch](https://lkml.iu.edu/2602.3/08975.html), [RISC-V International](https://riscv.org/blog/spacemit-develops-server-cpu-chip-v100-for-next-gen-ai-applications/) |
| **SiFive P870-D** | Vendor material says "distributed IOMMU"; spec conformance not established | No: IP, no board | [SiFive P800 page](https://www.sifive.com/cores/performance-p800) |
| **Ventana Veyron V2** | Unconfirmed | No: chiplets and IP, not a board a demonstrator can order | [Ventana](https://www.ventanamicro.com/ventana-introduces-veyron-v2/) |
| **ESWIN EIC7700X** (Milk-V Megrez, SiFive P550) | **Unconfirmed.** No IOMMU node or binding found in the upstream EIC7700 device tree work | Yes, ~$199 | [EIC7700 dts series](https://lkml.iu.edu/hypermail/linux/kernel/2504.1/05615.html), [Milk-V](https://milkv.io/megrez) |
| **SOPHGO SG2042 / SG2044** (T-Head C910/C920) | **Unconfirmed.** Nothing found either way | SG2042 yes | [SOPHGO roadmap](http://riscv.epcc.ed.ac.uk/assets/files/hpcasia24/hpc_asia_wang.pdf) |
| **StarFive JH8800** | Unconfirmed | No. Still not shipping; StarFive's own site lists only JH7110 | [StarFive hardware index](https://www.starfivetech.com/en/index.php?s=hardware&c=category&id=3) |
| **lowRISC / revflex** | Unconfirmed. Nothing found either way | No | (nothing found; recorded as a gap, not as a negative) |
| **StarFive JH7110** (the VisionFive 2, which we own) | **None** | Yes, owned | 16a's block |

The spec itself. v1.0.1 was ratified 2024-09-11 and is archived under that date; the current
rendering carries a 2026-02-22 stamp on the same v1.0.1 version number, so the block's "ratified
2024" is right and the newer date is a re-publication rather than a new version.
([v20240911 archive](https://docs.riscv.org/reference/hardware/iommu/v20240911/_attachments/riscv-iommu.pdf),
[current](https://docs.riscv.org/reference/iommu/_attachments/riscv-iommu.pdf))

### The K3, measured against the three requirements

1. Ratified spec: yes, and this is the strong claim. The Linux dt-binding for the SpacemiT T100
   lists `spacemit,k3-t100` and `spacemit,v100-t100`, each falling back through `spacemit,t100` to
   the generic **`riscv,iommu`**. That fallback is the evidence: it is the compatible the generic
   RISC-V IOMMU driver binds to, so the K3's IOMMU answers at the spec's own register interface and
   the vendor part is an extension rather than a substitute. The extension is a distributed
   architecture: one main IOMMU with up to 64 IOATCs caching IOTLBs beside the DMA masters, each
   with its own performance counters and interrupt vector. That is why the binding raises the
   interrupt count from 4 to 68. Author Lv Zheng, 2026-02-28, v5 of the series.
   ([patch](https://lkml.iu.edu/2602.3/08975.html))

2. Reachable as a PCI function: no, and this is what actually blocks us. The K3's IOMMU is a
   device-tree platform device, not a PCI function. The spec allows both and QEMU emulates the PCI
   form, which is the form 16b's bring-up path was written against
   ([QEMU riscv-iommu](https://www.qemu.org/docs/master/specs/riscv-iommu.html); Linux carries both
   backends, `iommu-pci.c` and `iommu-platform.c`). So requirement 2 is not a hardware fact at all.
   It is a gap in our driver, and a small one: the register interface is identical, only
   discovery differs.

3. Firmware contract: half yes. OpenSBI has upstream K3 platform support, Linux detects SBI
   v3.0 and the HSM extension, and all 16 harts come online, so the SBI half is satisfied
   ([OpenSBI series](https://ratatoskr.run/opensbi/2026/09/17546622/t)). **The interrupt controller
   is not.** The K3 is RVA23 and uses AIA: APLIC and IMSIC, and this kernel's riscv64
   `arch::interrupt_controller` resolves to `drivers::plic` with nothing else behind it. A PLIC node
   was not found in
   the upstream K3 device tree. ([K3 DT series](https://patchew.org/linux/20260115-k3-basic-dt-v5-0-6990ac9f4308@riscstar.com/))

**What could not be established, said plainly.** The K3 datasheet is an image-only PDF and did not
yield text, so the IOMMU, PCIe and MMU claims here rest on the kernel patches and press coverage
rather than on the vendor's own document
([datasheet](https://cdn-resource.spacemit.com/file/chip/K3/k3_datasheet_en.pdf)). No `iommu` node
wired into the upstream K3 `.dtsi` was found, only the binding and the HPM driver, so whether the
T100 is enabled on a shipping K3 board or only described is unconfirmed. Sv39 is assumed from the
RVA23 S-mode baseline and was not verified against the part. And the EIC7700X, SG2042/SG2044 and
lowRISC entries are gaps in the search, not established negatives.

### The PCIe-card and FPGA shape of the answer, which nobody had asked

There is no RISC-V IOMMU on a PCIe add-in card. The only non-SoC implementations found are RTL IP
cores, validated on a Genesys2 FPGA inside a CVA6 SoC and explicitly "not formally verified and very
likely to have bugs"
([zero-day-labs/riscv-iommu](https://github.com/zero-day-labs/riscv-iommu)). That is a different
project, not a purchase: it requires building a soft SoC, and a bug in the IP would be
indistinguishable from a bug in our driver, which is exactly the ambiguity this milestone exists to
avoid.

### Why this is worth more on riscv64 than on the other two ISAs

riscv64 is the only one of the three architectures that puts MSI remapping inside the IOMMU's own
device context. `kernel/src/arch/riscv64/iommu.rs` reads `CAP_MSI_FLAT` and widens the device
context from 32 to 64 bytes when the IOMMU reports it. On x86_64 interrupt remapping is a separate
IOMMU feature (`intremap=on`, off in every boot here) and on aarch64 it is a separate device (the
GICv3 ITS, absent here).

So a ratified IOMMU on riscv64 silicon closes **two** open questions with one purchase: this
milestone's DMA-isolation wait, and the MSI-confinement gap that notes/confinement-claims.md records
as a claim stated nowhere. The K3 sharpens that rather than softening it, because an AIA part routes
its interrupts as MSI writes into IMSIC files, which is precisely the traffic the IOMMU's MSI page
table exists to confine. That argument was written down nowhere before this survey.

### The verdict, and what would change it

As of 2026-09-17: still no, but the reason moved from the silicon to us. A board shipping the
ratified RISC-V IOMMU is buyable today for about $299. What stands between it and 16b's driver is
two pieces of our own engineering, neither of them a hardware wait:

- A platform (MMIO, device-tree) discovery path for the riscv64 IOMMU driver, beside the PCI one
  16b built. The register interface is the same; only the bring-up differs.
- An APLIC/IMSIC driver for riscv64, because an RVA23 part does not speak PLIC. This is larger
  than the first and is not specific to this milestone.

What to watch. An `iommu` node landing in the upstream SpacemiT K3 `.dtsi`, which is the thing
that would confirm the T100 is enabled on shipping silicon rather than merely described. An
`riscv,iommu` PCI-function implementation in any part, which would remove the first gap entirely.
And the SpacemiT V100's late-2026 deployments, since the server variant is the one with coherent
page-table walk and is where a PCI-attached IOMMU would most plausibly appear.

What would make this question worth asking again. The K3 gaps being closed by other work (an
APLIC/IMSIC driver arriving for its own reasons would change the price of this milestone sharply),
or a confirmed IOMMU on a part we already own or would buy anyway. Note that **whether to buy a
fourth board is a decision this survey does not make**: milestone 241 holds it, `Gate: DECISION`,
deferred 2026-09-05.

## What it would cost

Near the floor, because the driver is built. The work is:
1. Boot the kernel on the board (the firmware contract is already spoken).
2. Enumerate the IOMMU as a PCI function (16b's bring-up path already does this).
3. Run the disk and attacker suites behind the IOMMU on silicon.
4. Fix what the silicon got wrong, and record whether it was our bug or the board's (the same
   discipline 16b applied to QEMU's emulation).

One lane, assuming the board boots. The unknown is whether the silicon matches the spec the driver
was written against, which is the thing only the board can answer.

## What this does NOT include

- SMMUv3 on aarch64 silicon. That is a separate hardware wait (a Pi 5 or similar ARM board
  with SMMUv3). 16b's aarch64 IOMMU driver carries over the same way, but the aarch64 board story
  is weaker (notes/target-hardware.md flags it) and not bundled here.
- The shadow descriptor ring. It stays as defense in depth everywhere, on silicon and in
  emulation, regardless of whether the IOMMU is present.

## Prior art

This is the emulate-then-carry pattern: seL4 developed against QEMU and carried to hardware; the
same pattern this kernel's riscv port used (Sv39 in QEMU, then the VisionFive 2). 16b's block has
the full argument.

## Index row

Split out of milestone 16 on 2026-08-20. 16b built IOMMU-backed DMA isolation in QEMU emulation on
both ISAs; this is the carry-over to silicon, which waits on a RISC-V board that ships the
ratified IOMMU spec (v1.0.1). The driver is built; the work was "boot it and fix what the silicon
got wrong." Survey re-run 2026-09-17: such a board now exists and is buyable (SpacemiT K3, ~$299,
ratified per its `riscv,iommu` dt-binding fallback). So the remaining gaps are ours, a platform
discovery path beside 16b's PCI one and an APLIC/IMSIC driver for an RVA23 part that does not speak
PLIC.
