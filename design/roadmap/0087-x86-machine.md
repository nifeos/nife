---
status: BUILT
raised: 2026-08-03
built: 2026-09-17
---
# 87. The x86_64 bare-metal machine

Raised 2026-08-03. xenon printed `nife self-test: 5 of 5 passed` at
09:55 UTC on 2026-09-17, which is the criterion calef set that morning. Transcript:
`bench/xenon-2026-09-17/first-light-095500.log`.

nife now runs on real hardware on all three architectures, aarch64 only under HVF (corrected 2026-10-06).

The gate as it read to the end, kept as written:

> **Gate: HARDWARE.** It is now the only gate. The hardware side finished 2026-08-23 (the
> OptiPlex arrived 2026-08-15; the Dell C4PDJ serial module and the dev-side RS-232 chain arrived and
> are installed). The *software* blocker closed 2026-08-30: the kernel could not be started by any
> real firmware at all until then, because the x86_64 port boots by PVH, a hypervisor direct-boot
> protocol no machine speaks. That is what "What was built" below fixed.

## The boot that closed it, 2026-09-17

```
  mmu             : fine W^X 4-level map installed (cr3 0x187000), image 0xffffffff80000000
                  : 32824 KiB of page tables, no identity map, guard pages are holes
  cycles      : IA32_PERF_FIXED_CTR1 (unhalted core cycles), 48 bits, perfmon v4
  iommu           : VT-d drhd at 0x00000000fed90000, root table default-deny, translating
nife machine: x86_64, 4 processor(s), 17119 MiB, 100 Hz
  self-test       : exceptions ok · mapping ok · frames ok · timer ok · scheduler ok
nife self-test: 5 of 5 passed
```

*Corrected 2026-09-25, pull request #1275:* the `iommu` line names the DMAR's first unit.
On the OptiPlex 7040, at xenon's addresses, that unit covers only integrated graphics: the line
shows a unit translating, not a device confined. Unverified until xenon's DMAR is read.

The `AlreadyMapped` fix held: the boot went straight past `mmu : fine W^X 4-level map installed`,
the line this machine died before reaching on 2026-09-04.

Three numbers nobody had read from real x86_64 hardware:

- 32,824 KiB of page tables, against `mmu.rs`'s `BUGS` prediction of 0.2% of RAM, which is about
  33 MiB of this machine's 17 GB. The estimate was right.
- `IA32_PERF_FIXED_CTR1`, 48 bits, perfmon v4. Milestone 309's probe, merged hours earlier,
  reported `NoPerfmonLeaf` under QEMU and found the real counter here. x86_64 now reports the same
  quantity riscv64 does, unhalted core cycles rather than TSC ticks, which is the parity gap
  milestone 74's scope note names.
- TSC 2714 MHz by PIT calibration, and the timer self-test measured against it.

One address disagreement, reported rather than assumed:

```
pcie ecam 0xf0000000, buses 0..=127 (mmu::PCI_ECAM_PHYS says 0xb0000000)
```

The ACPI MCFG puts the ECAM window at `0xf0000000`; the constant says `0xb0000000`. Nothing failed,
because the discovered value was used, but a constant that disagrees with the first real firmware
to check it deserves a look before something trusts it.

## And it refused to hand over, which is the gate working and a defect in our own build

```
nife: handing the system to the userspace progenitor.
  MEASURED BOOT REFUSED: no measurement for the archive entry 'progenitor'
```

This is the second time this exact defect has reached a bench. `cargo xtask uefi-image` built the
kernel before the archive. Packing the archive regenerates `target/init-measure-x86_64.txt`, the
manifest `kernel/build.rs` compiles in as the measured-boot trust root, so a kernel built first
vouches for the *previous* archive and the gate refuses the pair at handover.

`script/board-image` had the same defect for riscv64 and the VisionFive 2 refused the pair on
2026-08-15 (boot 12). The fix there carries a comment reading *"QEMU never hit it because xtask
orders these correctly"*, which was true of the riscv64 path and false of this one, and nothing checked.

QEMU does not catch it because one tree usually has both fresh. It bites when the kernel is
already built, as it was here, after a lane compiled it earlier in the session.

Fixed in `xtask::uefi_image` on 2026-09-17, archive first, with the reasoning at the call site
rather than in a note, because a comment in the other script had already asserted this was handled.
Verified under OVMF: the corrected pair prints `progenitor: every program measured against the
archive table` and reaches a ring-3 shell. Everything past the handover is ground this kernel has
never covered on this machine.

## What the screen showed, and a finding that was nearly invented

The framebuffer console worked: the tour was legible on the panel during boot (calef, at the bench).
A photograph taken after the halt (`IMG_4143`, filed with the off-tree originals per
`notes/xenon-firmware.md`'s convention) shows a sparse dotted grid, which is the panel after the
machine stopped rather than anything nife drew.

Recorded because a maintainer, reading that photograph alone, had begun drafting a finding that
the framebuffer console was broken on xenon, citing `uefi_loader`'s own stride warning. calef's
correction, that text had been on the screen before it, alone stopped a fabricated defect. A
photograph of a halted machine is evidence about a halted machine.

First light happened on 2026-09-04, and this block went on reading as though it had not, which
misled a maintainer on 2026-09-16 into saying three times that xenon had never booted nife at all.
The cause was the completion sentence below ("completes when the machine has printed a byte over
serial"): bytes were printed, so the sentence was satisfied while the milestone was not.

What happened: the UEFI loader ran from the stick, the kernel started under the machine's own
firmware, the tour printed, and it panicked in the mapper:

```
[PANIC] panicked at kernel/src/arch/x86_64/mmu.rs:325:33:
failed to build the kernel page tables: AlreadyMapped
```

`notes/x86-uefi-boot.md` carries the session and the diagnosis; `notes/xenon-firmware.md` carries
the 70 photographs of firmware settings taken the same day. The diagnosis corrected its own first
hypothesis (the framebuffer aperture had not met RAM; the firmware's map does not describe the
32-bit MMIO hole at all). It found something larger than the panic. The fill was mapping the IO
APIC, the SPI flash and 128 MiB of PCH decode **cacheably**, which is a write that can sit in a
cache line and never reach the device. Nothing had touched those yet, so nothing had failed; the
panic is what made it visible.

The fix is `memory_mapped_io_window`, on `main`. `notes/x86-uefi-boot.md`'s step list has the
procedure and what to do on a new panic, which is progress rather than a failure of the fix.

The completion criterion is the self-test, ruled by calef on 2026-09-17, replacing "printed a
byte over serial":

> This milestone is `BUILT` when xenon prints `nife self-test: N of N passed`.

**Why that line and not the obvious alternatives.** "The tour completing" was proposed first and
withdrawn. Milestone 267 (the tour is three things wearing one name) established as much, and
deleted one of them (the narrative program). So "the tour" would cite something that partly does not exist.

The self-test is the right bound for **this** milestone. It is a machine-readable line that
`script/soak` and `crates/serial_console` already judge board runs by, so nothing new has to learn
to read it. Passing it means exceptions, mapping, frames, timer and scheduler all work on the
hardware. That is "this machine runs nife" with a definite answer rather than a liveness signal.

The progenitor handover (`nife: handing the system to the userspace progenitor`) is a stronger
claim, but it drags in the archive, ELF loading and the FS service, which are the scope of
milestone 161 (the x86_64 kernel port). That line belongs to 161 or 182, not here.

The sentence below is kept as written because it is what the block promised; rewriting a promise
to match an outcome is how a record stops being one. Believe this paragraph instead.

The bench procedure and its failure triage are in notes/x86-uefi-boot.md's "The bench".

## What was built (2026-08-30)

A UEFI entry, chosen over GRUB Multiboot 2 on a fork this lane priced rather than argued. Both
were real and both could coexist; two commands decided it. OVMF, the open-source UEFI
implementation, ships with the QEMU this project already pins
(`/opt/homebrew/share/qemu/edk2-x86_64-code.fd`), and QEMU's `vvfat` driver synthesizes the FAT
filesystem out of a host directory, so the whole path is testable today with nothing installed.
GRUB is not installable on the development machine at all (`brew info grub`: no formula), so that
path could have been written on patagonia but not *proved* there. The OptiPlex is also UEFI-native,
so UEFI is the shorter path at both ends. GRUB stays cheap to add for a BIOS-only machine.

**The kernel is not modified**, by design rather than economy. `uefi_loader` places
the kernel at its `p_paddr`, synthesizes an `hvm_start_info` out of what the firmware knows, leaves
long mode, and enters **the same `_start`** with the same register contract QEMU's PVH loader
delivers. One entry point, one handoff structure, one decoder, one set of tests; two of each would
have diverged, and the divergence would first show up on hardware nobody can attach a debugger to.
It also means `script/test --arch x86_64` cannot regress under it, which was this milestone's
sharpest hazard.

**Proved under OVMF, and it exercised four paths that had never run**, because a hypervisor never
takes them. The ACPI root pointer arrives non-zero (so the BIOS-area `"RSD PTR "` scan is skipped).
It is revision 2 with an **XSDT** root rather than revision 0 with an RSDT. The MCFG's ECAM window
is `0xe0000000` where the hardcoded constant says `0xb0000000` (so "read the table" is finally
distinguishable from "used the constant", which milestone 165 (x86_64 PCI enumeration) could not show). The memory map is
118 regions against PVH's nine. The userspace archive arrives too, through the module list the
loader writes.

`cargo xtask uefi-boot` gates it and runs inside `script/test --arch x86_64`. See
notes/x86-uefi-boot.md for the whole account, the measured numbers, and the honest limitations
(the bench procedure itself is untested, the suite has not been run under firmware, and SMP under
UEFI has never been exercised).

Purchased 2026-08-15 (calef), all arrived and installed as of 2026-08-23. The OptiPlex 7050 Micro
(i5-7500T, 16GB, 256GB NVMe, with its AC adapter, $139). The Dell C4PDJ serial module with its cable
($18.88, the with-cable check the earlier draft flagged, answered by the listing's own title). And
the dev-side RS-232 chain (FTDI USB adapter at 1.5 ft, $15.96, plus a StarTech NM9FF
null-modem barrel, $7.98, chosen over a cable so the desk carries eighteen inches of serial, not
three feet). About $182 all-in against the $194 estimate, twelve days after selection. The $129
machine tier had aged out and nothing else moved. This milestone completes when the machine has
printed a byte over serial. The x86_64 port itself is milestone 161's scope and is not gated on
the purchase, because it starts under QEMU TCG the way riscv64 did. The RS-232 and 3.3 V TTL
bench note is under Follow-on.

DECISIONS §19 (architectural parity is a tenet) names x86_64 as the third ISA target. The second ISA's lesson (milestone 16 (real hardware + IOMMU-backed driver isolation), the
VisionFive 2) is that the board should be chosen and ordered before the port needs it, from
requirements the port derives rather than from specs. Bare-metal bring-up is a loop of hang,
power-cycle, retry, so the machine must be dedicated and consequence-free. cordoba is disqualified
for bare metal on exactly those grounds: it is the production server, and a 2013 desktop board has
no BMC, no serial-over-LAN, no remote power. It remains the KVM and VT-d *virtualized* test
host for the same port.

The requirements, each traced to something this tree already does:

- A real 16550 COM port. Early bring-up output exists before anything else works, and QEMU's
  q35 machine emulates the same legacy UART, so one driver spans emulator and silicon. This is the
  NS16550/PL011 pattern both existing ISAs follow, and it eliminates most modern consumer hardware.
- VT-d, because IOMMU-backed driver isolation (milestone 16) is a parity theme (§19), and the
  x86 side of the DMA-confinement story needs real hardware eventually.
- A NIC QEMU can stand in for. QEMU 11.0.2 (checked against the pinned binary, not the docs)
  emulates two modern Intel families: `e1000e` (I217/I218/I219) and `igb` (82576, whose driver
  family covers i210/i211/i350). It does **not** emulate `igc` (i225/i226), and upstream has
  nothing in flight. An i226 machine is therefore acceptable but taxed: the driver core gets
  written against QEMU's `igb` (igc is igb's descendant, so rings and descriptors carry over) and
  the igc deltas are ported on hardware. A minimal driver is 1,500-3,000 lines against Intel's
  public datasheet; the plumbing around it (PCI decode, DMA confinement, the userspace net server)
  already exists.
- Four real cores for the per-CPU scheduler, and any Intel core has the PMU that milestone 25's
  `sel4bench` comparison was deferred to real hardware for.
- Remote power cycling by smart plug, not by management firmware. A plug is $15 and works on
  anything.

The selection: a used Dell OptiPlex 7050 Micro plus the Dell C4PDJ serial module (calef, 2026-08-03,
settled after a full pass over the new market). i5-7500T with 16GB was $129 with the module at $35,
~$194 all-in with the dev-side serial gear and the smart plug. The used-hardware risk was weighed
deliberately and priced. eBay's money-back guarantee bounds "does it work" to return friction. At
real configured prices every new machine cost $150-350 more. The 7050
keeps the fastest cores in the field and the I219 NIC in QEMU's `e1000e` family, so the
one-driver-spans-emulator-and-silicon property holds with no caveats. The module is Dell P/N
C4PDJ (fits 3050/7040/7050 MFF, snaps into the rear punch-out, cables to a motherboard
header; check the listing includes the cable). Used units essentially never ship with it, so buy it
separately rather than hunting for a factory-configured unit.

The market at selection time, so the next reader knows what was weighed. The closest contender
was a **new Protectli VP2430** ($300 configured with coreboot). It is a real vendor with published
datasheets, open-source firmware aligned with `measured_boot`'s future on x86, console cable
included. But it has i226-V NICs in the `igc` family QEMU does not emulate, and costs $150 over the
used route. It stays **the recorded alternative** if the used machine disappoints or when open
firmware becomes the point. Configured industrial N100 boxes on Amazon ran $500-730 and are
dominated by the VP2430 at every point. A used PC Engines apu2 deserves a correction from the
first draft of this entry: its i210 NICs are `igb` family. So QEMU's igb model gives it the
one-driver property this entry originally credited only to the 7050. It stays a runner-up for its
EOL status and slow Jaguar cores, not its NIC. If netboot iteration becomes worth it, cordoba
hosts the PXE/TFTP end.

## Scope note

This milestone is the machine and the serial link proven, nothing else; the port is milestone
161's (above). Buying early was cheap insurance against the VisionFive 2 pattern
(ordered 2026-07, arrived 2026-08-21) of the board being the long pole, and it paid off: the
hardware side finished before the code side needed it.
## Follow-on

- **Done.** The kernel suite runs under real firmware. Milestone 195 added a UEFI test task, run by
  `script/test --arch x86_64` after the tour: 192 passed and 68 skipped under OVMF, the same names
  as under PVH, with the runner's virtio disk, NVMe and IOMMU attached.
- **Done.** SMP under UEFI is exercised. `notes/x86-uefi-boot.md` records two cores up under OVMF
  five runs out of five, gated, after the loader started asking firmware for the trampoline page by
  name instead of relying on OVMF's habits.
- **Done.** The bench procedure is run and proved: 2026-09-04 first light and 2026-09-17's closing
  boot both followed `notes/x86-uefi-boot.md`'s "The bench" section, and `notes/xenon-firmware.md`
  carries the firmware settings read off the machine rather than off Dell's documentation.
- **Done.** The completion condition is met, and it changed on the way: calef ruled on 2026-09-17
  that `BUILT` means `nife self-test: N of N passed` rather than a byte over serial, because a byte
  was printed on 2026-09-04 while the milestone plainly was not done. xenon printed the self-test
  line on 2026-09-17.
- **Recorded.** The smart plug this block's requirements list prices at $15, and includes in its
  $194 estimate, appears in no purchase record in the tree, so a bring-up loop of hang,
  power-cycle, retry has no remote power today. That limitation lives beside the decision it
  belongs to, milestone 224, where calef ruled on 2026-09-04 that board power stays manual.
- **Recorded.** The suite under firmware still runs at one core, so nothing exercises UEFI AP
  bring-up together with the scheduler's cross-core tests. That is the x86_64 port's open two-core
  defect rather than a firmware fact.
- **Recorded.** The image is placed at one link-time address and 32 MiB of low memory is not a
  guarantee; the loader now names the descriptors in the way, which is the difference between a
  load error and a sentence.
- **Recorded.** This port's serial chain is real RS-232 and the two boards' adapters are 3.3 V TTL.
  They share a desk and must be labeled so they are never swapped.
- **Recorded.** The Protectli VP2430 stays the named alternative if the used machine disappoints or
  when open firmware becomes the point, priced in this block at $150 over the used route.
- **Milestone 161.** The `igc` driver deltas QEMU cannot emulate belong to the x86_64 port rather
  than to the machine purchase.

## Index row

Milestone 161's third ISA needs what milestone 16's second needed: a dedicated, brickable board,
selected before the port so the requirements drive the purchase. Selected: a used OptiPlex 7050
Micro plus the C4PDJ serial module, ~$194 all-in; every new option cost $150-350 more at real
prices. Machine, serial module and RS-232 chain all arrived and installed as of 2026-08-23. **The
software blocker closed 2026-08-30**: the port boots by PVH, a hypervisor protocol no firmware
speaks, so `uefi_loader` now places the kernel and enters its existing `_start` with PVH's own
register contract, proved under OVMF and gated by `cargo xtask uefi-boot`. What remains is one
person, one FAT32 stick and a serial console; the procedure is written out in
notes/x86-uefi-boot.md
