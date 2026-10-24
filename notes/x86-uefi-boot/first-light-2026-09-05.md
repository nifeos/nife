# First light on xenon, 2026-09-05, and what xenon still has to establish

An appendix to [notes/x86-uefi-boot.md](../x86-uefi-boot.md). It holds the first boot on the
OptiPlex, what its boot menu recorded, the panic it found and its fix, the session that confirms the
fix, and the questions only xenon can answer.

## The machine talked without a serial cable

It has now been done. Photograph: `art/bench/xenon-2026-09-05-first-light.jpg`, which is the
transcript. patagonia could not be moved to the bench, and the Dell's video output was the only
channel.

The framebuffer console of milestone 243 (a machine with no serial port has no way to say
anything) carried the whole boot tour on its first contact with real
firmware. It had never run outside OVMF:

```
screen : 1920x1080 bgrx @ 0xd0000000, 274x135 cells (boot cmdline)
```

How far it got, all of it read off the screen:

- long mode, the high-half kernel, and the long-mode jump landed;
- a breakpoint caught and stepped over;
- 38 memory regions from the PVH handoff;
- ACPI revision 2, with a real XSDT at `0xcae090b8` and the full table list;
- 4 cores enumerated, 4 enabled;
- PCI ECAM, and TSC and PIT-measured timers at 100 Hz;
- 17,119 MiB total.

Three things are firsts against real firmware rather than OVMF, and each was predicted:

- `8259s present (must be masked)`. Real legacy PICs.
- `PCI_ECAM_PHYS says 0xb0000000` while the MCFG reports `0xe0000000`. The hardcoded constant
  disagrees with the firmware table, and the kernel follows the table. That is exactly what
  [measured-under-ovmf.md](measured-under-ovmf.md) says the UEFI path exercises and the PVH path
  cannot.
- 38 regions against OVMF's 118 and PVH's 9. *(Read as 148 when this section was first written, and
  corrected 2026-09-04 against both photographs: the count is 38 in the first-light shot, and 38
  again on the second boot. The misreading was mine. It propagated into the memory-map fixture's own
  doc comment, where it understated how much of the machine that fixture covers.)*

## What the boot menu documents, which is more than it looks

`art/bench/xenon-2026-09-05-boot-menu.jpg`, the F12 menu, is the first record of this machine's own
identity rather than of the model:

- OptiPlex 7050, BIOS revision 1.27.0.
- Boot mode UEFI, Secure Boot OFF, which is what the bench's steps 1 and 2 ask for, and confirms they
  were reachable as written.
- The stick enumerates as `UEFI:` and its vendor and model, so the removable-media fallback
  at `\EFI\BOOT\BOOTX64.EFI` is found with no boot entry created.
- A `Windows Boot Manager` entry, so this machine dual-boots and its internal disk holds somebody's
  installation. Worth knowing before anything here writes to a disk.
- `UEFI: Micron 2450 NVMe 256GB`.

That last line matters beyond the bench. DECISIONS §86 (whether an NVMe driver can leave the kernel,
and what capability would let it) was decided on 2026-09-03. Its own research recorded that no board
this project owns has an IOMMU in front of a real NVMe controller. xenon has both. The requirements
list of milestone 87 (the x86_64 bare-metal machine) says it was selected partly for VT-d, and this
photograph shows the NVMe. So the confined-driver experiment §86 exists to enable has real hardware
to run on, which nobody had established.

## And then it panicked, in the one place a bigger machine would find

```
[PANIC] panicked at kernel/src/arch/x86_64/mmu.rs:325:33:
failed to build the kernel page tables: AlreadyMapped
```

It was diagnosed on 2026-09-05 from this photograph, and the leading hypothesis was wrong. It read the
framebuffer aperture at `0xd0000000` against 17 GB of RAM, and concluded the two had met. They had
not, and the memory map on the screen is what says so. The aperture sits in the 32-bit MMIO hole,
which this firmware's map does not describe at all. The last entry below it ends at `0xd0000000`, the
next begins at `0xf0000000`, and the aperture is in no RAM region and never was.

What actually collided is the local APIC, and the mechanism is one line older than milestone 243.
`map_firmware_regions` direct-mapped every loader-reserved entry below the top of RAM, cacheably. Its
own comment said why that bound was the load-bearing part: the reserved entries *above* the top of
RAM are the MMIO windows, and those must be device-typed. That is a true statement about a machine
whose RAM ends below the hole, which is every machine this tree had booted. xenon's RAM ends at
`0x42e000000`, so *every* MMIO window it has is below the bound:

| Reserved entry, from the photograph | What it is | Below `0x42e000000`? |
|---|---|---|
| `0xcbe00000..0xd0000000` | firmware's carve-out at the top of low DRAM | yes |
| `0xf0000000..0xf8000000` | PCH decode | yes |
| `0xfe000000..0xfe011000` | PCH | yes |
| `0xfec00000..0xfec01000` | IO APIC | yes |
| `0xfee00000..0xfee01000` | local APIC | yes |
| `0xff000000..0x100000000` | SPI flash | yes |

So the cacheable fill claimed `0xfee00000` a few lines before step 5 asked for the same page
device-typed, and the mapper refused, which is exactly what it is for. The framebuffer never got that
far: the local APIC is the first window step 5 maps.

Two things are worth separating here. The panic is the smaller half. The larger half is that on this
machine the fill was also mapping the IO APIC, the SPI flash and 128 MiB of PCH decode cacheably. That
is a write that can sit in a cache line and never reach the device. Nothing had touched those yet, so
nothing had failed; the panic is what made it visible.

The fix, on `maintainer/already-mapped-on-real-ram`. The fill's bound is now the address at which the
firmware's map stops describing memory, walked as a chain upward from the low megabyte. DRAM and the
carve-outs firmware takes out of it are contiguous, and the MMIO hole above them is a gap. On xenon
that is `0xd0000000`. On every machine whose RAM ends below the hole it is the same number the old
bound produced. The device windows are also enumerated before the fill now, so device typing wins by
construction rather than by that bound being right.

And the panic now names both ranges. `AlreadyMapped` on its own cost this session a diagnosis.
`map_everything` maps eleven kinds of range, and the message distinguished none of them. Every
direct-map range now comes out of one enumeration that the failure path also walks. So the message is
`AlreadyMapped mapping local apic 0xfee00000..0xfee01000: 0xfee00000 is also claimed by firmware reservation 0xfee00000..0xfee01000`.
That is the difference between a bench session that ends in a diagnosis and one that ends in a
hypothesis. It is the half worth keeping regardless of whether the fix is right.

## Confirming the fix, which only xenon can do

A green QEMU run is not a confirmation, and should not be reported as one. The runner boots with
2 GiB, and the parent's comparison table was taken at `-m 256M`; neither reaches a memory map with RAM
above the MMIO hole. `-m 17G` on patagonia (16 GB) swaps rather than reproduces. What the suite does
prove is that the rule answers the old bound's number on a small machine. The `x86_64` kernel leg
carries xenon's map as a fixture (`map_tests` in `kernel/src/arch/x86_64/mmu.rs`, transcribed from the
photograph), so the 17 GiB case is asserted without the machine.

The bench session that confirms it, in the shape of the stick procedure:

1. Build the stick exactly as the procedure says, from `maintainer/already-mapped-on-real-ram`, or
   from `main` once it has landed.
2. Boot it. The line to watch for is `mmu`, which the boot tour prints after `map_everything`
   returns:
   `mmu : fine W^X 4-level map installed (cr3 ...), image 0xffffffff80000000, direct map 0xffff888000000000`,
   followed by the page-table cost in KiB. Reaching that line at all is the confirmation: it is one
   line past where the machine stopped on 2026-09-05.
3. Photograph the whole screen anyway, not just that line. The page-table cost on the second line is
   the number nobody has ever seen from a real machine. This module's `BUGS` prices 4 KiB leaves at
   0.2% of RAM, so ~33 MiB is the prediction to check against.
4. If it panics again, the message is now the deliverable. Photograph it and stop. It names the two
   ranges, so no further bench time is needed to say what happened.
5. The tour continues past the `mmu` line into ring 3 and the scheduler. Everything after it is new
   ground on this machine, and none of it has been seen on real firmware. So expect the next stop
   somewhere else, and treat that as progress rather than as this fix failing.

## What it settles about the procedure

The procedure worked, including its warning that firmware-menu wording would differ. calef also
photographed every page of the BIOS configuration, which is the first real record of this machine's
settings. Those 70 photographs are now transcribed in `notes/xenon-firmware.md`. The `BUGS` entry
saying every setting name came from Dell's documentation is retired to the extent the transcription
earns, and the four firmware steps now say which of them the machine agreed with.

And one thing it changes. Step 1's expectation that the loader's two lines are "the only sign the
stick was read at all" is no longer true. With milestone 243 the screen carries the whole tour, so a
serial-less bring-up is a real bench session rather than a stare.

## What xenon still has to establish, and what it no longer has to

Milestone 215 listed three things only the OptiPlex could confirm. Two of them are now answered on
patagonia, because the suite runs under firmware with the same devices the PVH runner attaches:

- A PCI function's MSI-X table, once *firmware* rather than this kernel placed its BARs. OVMF
  enumerates the bus before nife exists. `pci::bar_census` reports 5 of 8 functions with a BAR outside
  the window `mmu::map_everything` maps, so `place_bars` moved five rather than assigning them. The
  two tests of milestone 215 (a PCI function's interrupt reaches nothing on x86_64) that reach a `virtio-blk-pci` function through its MSI-X table pass on the
  far side of that move.
- A machine with more than one local APIC still delivering to the boot core's id. The tour boots at
  two cores under OVMF. Its `device irq` line shows the PIT's interrupt arriving 20 times in 0.2 s at
  100 Hz, with two APICs in the MADT.

The third is still open, but it is no longer the shape this gave it: whether the OptiPlex's firmware
leaves interrupt remapping off. This said the answer is a setting in somebody else's firmware, and
the 2026-09-04 transcription found no such setting exists. The whole of the 7050's Virtualization
Support menu is three pages (`Virtualization`, `VT for Direct I/O`, `Trusted Execution`), and
`VT for Direct I/O` is enabled. So the answer is not in a menu. It is in the DMAR the firmware
publishes, which any kernel can read and which QEMU synthesizes too. That moves the question off the
bench and into code: `design/roadmap/0378-read-the-dmar-on-xenon.md`.

And milestone 195 (finish the UEFI boot path) added one of its own for the bench, which is the more interesting of the two:
whether the Dell's firmware leaves 32 MiB free. OVMF's low-memory habits are OVMF's. If it does not,
the loader now says which ranges it wants and which descriptors are in the way. That message is the
whole difference between a bring-up and a stare.
