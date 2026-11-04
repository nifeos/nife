---
status: BUILT
raised: 2026-09-03
built: 2026-09-04
---
# 256. x86_64 places PCI BARs in a hardcoded window, and on xenon that window is RAM

Built on 2026-09-04. Minted the same day by calef, from
`design/roadmap/0256-a-bar-window-the-machine-agreed-to.md` (written 2026-09-03 by the
milestone 247 sweep, out of milestone 165's Follow-on), on the evening xenon proved it from the
bench. *(Number provisional until the merge queue lands it.)*

## It stopped being a prediction on 2026-09-04

Milestone 165 wrote this on 2026-09-02, before any x86_64 kernel had met real firmware:

> That is the number to read first on xenon's first boot: **a machine whose RAM reaches above
> `0xc000_0000` would have this kernel move most of its bus on top of memory**, and the line says so
> before anything is driven.

Two days later, on xenon's second boot, with #738's local-APIC fix in:

```
[PANIC] panicked at kernel/src/arch/x86_64/mmu.rs:349:9:
failed to build the kernel page tables: AlreadyMapped mapping pci bar window
0xc0000000..0xc0200000: 0xc0000000 is also claimed by ram above the image
0xb9fbf000..0xc8940000
```

and one line above it, the census reading exactly what 165 said to read first:

```
pci : 15 function(s) on the bus, 13 with a BAR outside 0xc0000000..0xc0200000
```

The prediction, the instrument that would show it, and the machine that confirmed it were all
written down before the confirmation arrived. That is the shape this project is supposed to
produce, and it is worth stating plainly because the failures get recorded far more often than
this does.

**The panic is the lucky half and should not be mistaken for the risk.** `map_everything` refused
because the window collided with a range the mapper had already claimed, so the machine stopped. Had
xenon's RAM ended just below `0xc0000000` instead of at `0xc8940000`, nothing would have collided,
and `place_bars` would have relocated thirteen of fifteen functions on top of memory the allocator
believes it owns. The failure would have arrived later as corruption somewhere unrelated. The
proposal named that outcome a day before the boot: *"not a device that fails to enumerate, it is a
device writing into RAM the allocator believes it owns."*

## Why the constant was right and is not

`PCI_BAR_PHYS` is `0xc000_0000`, q35's conventional 32-bit PCI hole. Its doc comment is honest about
its evidence, and the evidence names the boundary it does not cross:

> confirmed disjoint from RAM, the ECAM window, the HPET and both APICs by reading QEMU's own
> `info mtree` on 2026-08-24 with **`-m 256M`**

On a 256 MiB machine `0xc000_0000` is empty. xenon has 16 GiB and its low DRAM runs to
`0xc894_0000`, straight through the window. The constant was checked against the only machine that
existed at the time and correct by luck anywhere else. That is the class this tree has now hit three
times on one architecture: a constant that is right on emulated machines and has no firmware source. ECAM is the same class and is fine, because ACPI's MCFG names it and the kernel follows
the table (`PCI_ECAM_PHYS says 0xb0000000` while xenon's MCFG says `0xf0000000`, and the kernel took
the table's answer). The BAR window's equivalent lives in the PCI host bridge's `_CRS`, which is
AML, which milestone 165 refused and this block does not reopen.

## What to build

Take the window from the machine, from two sources that can disagree.

- The firmware memory map, already parsed at boot, which names the gaps between regions. On
  xenon the gap is visible in the photograph: the map's last entry below the hole ends at
  `0xd000_0000` and the next begins at `0xf000_0000`.
- **`TOLUD`** (top of low usable DRAM), which Intel's host bridge reports in its own configuration
  space, reachable with the config-space reads `kernel/src/pci.rs` already performs.

Fail loudly when they disagree rather than falling back to the constant. That is milestone 215's
posture for the analogous case, and its reason carries: a silent fallback to the old behavior is
the original bug wearing the clothes of a graceful degradation.

**And handle the asymmetry that this boot exposed, which the proposal did not name.** QEMU hands the
kernel **unassigned** BARs and real firmware hands it **placed** ones. `place_bars` relocates
everything into the kernel's window unconditionally, which is why the census reads 13 of 15 rather
than 0. A BAR firmware has already placed in a window firmware itself chose is a BAR that wants
adopting, not moving; only genuinely unassigned BARs need placing. Whether that split belongs in
this milestone or in a second one is the lane's call to make and to say out loud.

## The measure

`pci::bar_census`'s second number reaching zero on xenon, or the window being one that machine
agreed to. Under QEMU the census is the same instrument and is already wired to the boot line, so
the work is testable on patagonia and only *confirmed* at the bench.

It is also one line further into the tour. The boot stopped at the `panic!` in `initialize` in `kernel/src/arch/x86_64/mmu/kernel_map.rs` on 2026-09-04 and
everything past it is unseen on real firmware, so the honest expectation is that this milestone buys
the next stop rather than the end of the tour.

## What was built

The window is derived, and the constant is gone. `arch::x86_64::mmu::PCI_BAR_PHYS` no longer
exists; `arch::x86_64::mmu::memory_mapped_io_window` answers instead, and the boot tour panics rather than
booting if it cannot.

Two sources, exactly as the block asked, and one distinction the block did not anticipate:

- The firmware memory map, through a new `firmware_mmio_hole`, which is
  `firmware_fill_ceiling` asked from the other side. The fill's ceiling is the floor of the hole:
  one walk of the map answers both "how far may the cacheable direct map follow memory" and "where
  does the 32-bit MMIO hole begin", because they are the same boundary. The hole's ceiling is the
  next thing the firmware describes above it.
- `TOLUD`, through a new `arch::x86_64::machine::top_of_low_dram`, read from the Intel host
  bridge's own configuration space with the legacy `0xcf8`/`0xcfc` ports `enable_pcie_ecam` already
  drives. The vendor id is checked first, so a non-Intel bridge answers `None` instead of having
  one vendor's register read out of it.
- A disagreement is a panic naming both numbers, `MemoryMappedIoWindowError::Disagreement`, with no arm
  that quietly picks one and no arm that reaches a constant.

And the derived window steps over what the kernel already knows decodes in the hole, which is
the part that turned out to matter most. `window_in_hole` takes the lowest aligned span that
collides with none of the framebuffer, the ECAM aperture, both APICs, or VT-d's register file.
xenon's framebuffer is at `0xd0000000`, byte for byte the floor of its hole. A derivation that
took the floor and asked nothing else would have put the first BAR it placed on top of the console
the panic prints on. **That is not only a prediction about a machine nobody here can boot**: the
same collision reproduces under OVMF on patagonia, and the boot line there says so.

The asymmetry is handled here rather than deferred, because the measure needs it. The block
left that to the lane, and the honest answer is that deriving the window alone does not move the
census. Xenon's thirteen misplaced functions would still have been thirteen, and `place_bars` would
then have tried to move all of them into a 2 MiB window, exhausting it on the first BAR that wanted
a megabyte. So `pci::place_bars` grew an adoption arm. A nonzero BAR that overlaps no RAM region
is mapped device-typed where it stands (`mmu::map_page`, after the fine map exists) and left alone.
Overlapping RAM is the one refusal and it is the whole safety argument. Whether an address decodes
to the bus is the machine's claim, and it made it by writing the BAR. But a device window mapped over
memory is a device answering where the frame allocator allocates, which is what this milestone is
about.

`pci::bar_census`'s second number therefore means something narrower and better, and the change
is stated in its own doc comment. It counted every BAR outside the kernel's window, which was the
right question while every one of them was about to be relocated onto a window nobody had checked.
Now "outside the window" is ordinary and "outside the window and on top of RAM" is the whole
problem. Zero is the passing answer.

## What was measured

| Path | Derived window | Functions | Census, second number |
|---|---|---|---|
| QEMU q35, PVH, `-m 256M` | `0x10000000..0x10200000` | 8 | **0** (was 5 of 8) |
| QEMU q35, OVMF, bare tour | `0x80400000..0x80600000` | 6 | **0** (was 3 of 6) |
| QEMU q35, OVMF, suite devices | `0x80400000..0x80600000` | 8 | **0** (was 5 of 8) |
| xenon, real UEFI | unconfirmed, see BUGS | 15 | was 13 of 15 |

Three measurements are worth stating on their own, because each one settled a design question that
would otherwise have been argued:

- QEMU models `TOLUD` nowhere. It reads zero at offset `0xbc` on the PVH path *and* under OVMF,
  on QEMU 11.1.1, 2026-09-04. Offset `0xb0`, where the older 82G33/Q35-era chipsets put a 16-bit
  `TOLUD`, reads zero too. So the two-source check can never fire under emulation, and a design that
  treated absence as disagreement would have failed every boot on the only machine this lane could
  run. **Absence is not disagreement**, and that is not a fallback to the constant: the firmware map
  is still the machine's own answer.
- The derived floor decodes. The PVH window is `0x10000000`, the top of low DRAM on a 256 MiB
  machine, which is 2.7 GiB below the retired constant and had never had a BAR in it. The full
  suite passes there, including the two milestone 215 tests that reach a `virtio-blk-pci` function
  through its MSI-X table, so q35 routes from the top of low DRAM upward and not from `0x80000000`
  as the retired constant's neighborhood implied.
- OVMF's framebuffer sits at its hole floor, which is why that window is `0x80400000` and not
  `0x80000000`. The aperture-at-the-floor case was written as a prediction about xenon and turned
  out to be reproducible on this machine, which is the only reason it is tested rather than
  believed.

Five test cases were added to `mmu.rs`'s `map_tests`, against the xenon fixture that was already
there: both ends of the 17 GiB machine's hole and the window stepping over a framebuffer at the
floor. An empty hole gives up its floor, a hole with no room answers `None` rather than squeezing,
and the disagreement message names both numbers. `script/test` passes on all three architectures;
`script/lint` exits 0.

## BUGS
- The fix is unconfirmed on xenon, which is the one fact that matters. Everything above was
  measured on patagonia under QEMU. xenon is a bench session that needs calef at the machine with a
  camera, because patagonia cannot be moved to it and there is no serial. What is checkable here has
  been checked; what is not, is not.
- The honest expectation is still the next stop rather than the end of the tour. The boot
  stopped at the `panic!` in `initialize` in `kernel/src/arch/x86_64/mmu/kernel_map.rs` on 2026-09-04 and everything past it is unseen on real firmware. This
  block predicts the panic is gone and predicts nothing about what the boot finds after it.
- **`TOLUD` is Intel's register, not the architecture's.** A non-Intel host bridge answers `None`
  and the firmware map is then the only source, silently. That is correct behavior and it is also
  a smaller check than it looks: on such a machine the "two sources that must agree" is one source
  that cannot be contradicted. Said where the read happens, in `top_of_low_dram`'s own BUGS.
- Only offset `0xbc` is read, the Core-era location. The older 82G33/Q35 chipsets put a 16-bit
  `TOLUD` at `0xb0` and this does not look there, deliberately: a register that is something else
  on an older chipset would answer with a plausible address. `None` is better than a number
  nobody checked.
- **A gap in the firmware map is still not a promise that nothing decodes there.** It is a promise
  that firmware did not call it RAM. `memory_mapped_io_window` subtracts the windows this kernel knows about
  (the framebuffer, ECAM, both APICs, VT-d). It cannot subtract the ones it does not: the SPI flash,
  the LPC decode ranges, and anything else in a real machine's hole that no table this kernel reads
  will name. A BAR the machine itself placed is stronger evidence than this derivation, which is
  why adoption exists, and the residual risk is confined to BARs the kernel has to place itself.
- Adoption trusts the machine about decoding and checks it only about RAM. A firmware-placed BAR
  in a hole this kernel cannot see into is mapped where it is. That is the right trade (firmware
  read the `_CRS`; this kernel cannot) and it is a trust boundary rather than a proof.
- The census asks about a BAR's first page only. Its length needs sizing writes, and a census
  that wrote to every function on the bus would not be a census. A BAR that starts clear of RAM and
  runs into it is refused by `place_bars` instead, on a span it has sized, so nothing is mapped over
  RAM either way; the count is what is approximate, not the safety.
- `window_in_hole` is quadratic in its avoid list, which has never had more than five entries.
  Said at the function rather than fixed.
- `PCI_BAR_MAPPED` is still 2 MiB and still a constant. Its *placement* is now the machine's
  answer; its *size* is not, and a machine whose unassigned BARs need more than 2 MiB would exhaust
  it and print so. It stays small on purpose: every byte is mapped with 4 KiB leaves at boot, this
  module's first recorded BUG, and adoption means almost nothing is drawn from it on real firmware.

## Follow-on

- **Done.** The assigned/unassigned asymmetry the block left to the lane's judgment. Built here
  rather than proposed as a second milestone, because the block's own measure (the census reaching
  zero) cannot be met without it: deriving the window moves no BAR that firmware already placed.
- **Recorded.** Every limitation above sits in a `BUGS` section beside the thing it limits.
  `top_of_low_dram`'s Intel-specificity and its offset sit at the register read, the map-gap caveat at
  `firmware_mmio_hole`, the first-page approximation at `bar_census`, the avoid list's cost at
  `window_in_hole`, and `PCI_BAR_MAPPED`'s fixed size at the constant.
- **Recorded.** Confirming any of this on xenon is not work a lane can take: it needs calef at the
  bench with a camera, because patagonia cannot be moved to it and there is no serial. It is the
  first entry in this block's BUGS rather than a promise here, and `art/bench/` plus
  `notes/x86-uefi-boot.md` are where the next transcript goes.
- **Milestone 325.** for the case
  `PCI_BAR_MAPPED` cannot serve: a machine whose unassigned BARs want more than 2 MiB, where the
  answer is a window sized from what the bus asks for rather than a constant. And 2 MiB leaves so
  that a larger window is not a megabyte of page tables. Nothing forces it today, which is why it
  is a proposal and not a milestone.

## Index row

milestone 165 predicted it on 2026-09-02 and xenon confirmed it on 2026-09-04: `PCI_BAR_PHYS` was
q35's hole checked once at `-m 256M`, and a 16 GiB machine's RAM runs through it. The constant is
gone: `mmu::memory_mapped_io_window` derives the window from the firmware map's first gap above
low DRAM and from Intel's `TOLUD`. It panics naming both numbers when they disagree, and steps over
the windows this kernel already knows decode in the hole. Xenon's framebuffer at the hole's exact
floor is why. `pci::place_bars` adopts a BAR firmware already placed instead of moving it,
mapping it where it stands unless it overlaps RAM. The census's second number now counts only what
can be neither used nor adopted, and reads 0 on every path measured (was 5 of 8 under PVH, 3
of 6 under OVMF, 13 of 15 on xenon). QEMU models `TOLUD` nowhere, so absence is distinguished from
disagreement and the map is the only source under emulation. Unconfirmed on xenon, which needs a
bench session with a camera.
