---
status: BUILT
raised: 2026-09-17
built: 2026-10-04
---
# 320. Every PCI bus the machine has, not just bus zero

Built and gated under QEMU, and confirmed on xenon (2026-10-04 UTC). The bench boots find the
Micron NVMe at `01:00.0` behind root port `00:1b.0`, the topology this block predicted:
`disk-throughput` preflight 1 names it (`bench/xenon-2026-10-04/boot-e-main-clflush-1.log`) and
`boot-c-diag.log` names the bridge.

## What was wrong

```rust
// kernel/src/arch/x86_64/mmu.rs:246
pub const PCI_ECAM_BUSES: u16 = 1;
```

The kernel mapped one megabyte of configuration space and enumerated one bus. ACPI's MCFG described
`buses 0..=127` and the boot printed exactly that, two lines apart, and the discrepancy sat in plain
sight for a year because both sentences were true.

On `q35` an NVMe controller hangs directly off bus 0, so one bus had always been enough. On xenon the
M.2 slot is believed to be behind a PCIe root port, so the controller sits on a secondary bus and
`pci::find_nvme_device()` searched a bus the disk was never on. The confinement test did not fail,
it skipped, reporting `NIFE_NVME not set on this leg?`, which is QEMU's explanation for an
absence with an entirely different cause.

Transcript: `bench/xenon-2026-09-17/nvme-attempt-2-no-controller-found.log`.

## What the census found, and what it did not

**The census is built and prints; no transcript of its own lines from xenon is committed.** The
bench boots found the NVMe through the walk (above), but the photographs relayed only the
`disk-throughput` lines, so the census's full output on xenon is unrecorded. `pci::survey` prints bus, device, function,
vendor id, device id and class for every function, and for every bridge the buses behind it. What it
printed under QEMU, which is the machine a lane can reach:

```
  pci         : ecam at 0xb0000000 (buses 0..=255), decode was already on
                00:00.0 8086:29c0 class 060000
                00:01.0 1234:1111 class 030000
                00:02.0 8086:10d3 class 020000
                00:03.0 1b36:0010 class 010802
                00:1f.0 8086:2918 class 060100
                00:1f.2 8086:2922 class 010601
                00:1f.3 8086:2930 class 0c0500
                7 function(s) over 1 bus(es) of the 256 described; mapping 1024 KiB of config space
```

Flat, as expected: `q35` has no bridge, and the NVMe at `00:03.0` is the class `010802` function on
bus 0. With `NIFE_PCIE_ROOT_PORT=1`, the same boot, the same code:

```
                00:03.0 1b36:000c class 060400 bridge to buses 01..=01
                01:00.0 1b36:0010 class 010802
                8 function(s) over 2 bus(es) of the 256 described; mapping 2048 KiB of config space
```

The block's premise is therefore neither confirmed nor refuted. It said to stop if the census
found the NVMe on bus 0 all along; under QEMU it does find it on bus 0, and that is the machine
being flat rather than evidence about xenon. The refutation this block asked for can only come from
a xenon boot, and the census is what makes that boot decisive in one line instead of a day. It now
either prints a `class 010802` function behind a bridge, or it prints fifteen functions and no mass
storage anywhere. Those are two different diagnoses that used to look identical.

**The BAR pressure the block predicted did not materialize under QEMU**: `0 with a BAR this kernel
can neither use nor adopt`, both flat and bridged. The number to watch is still xenon's, which read
13 of 15 outside the window before milestone 256 changed what that count means.

## The design fork, decided, and what the measurement changed

Option 2, the bridge walk. `crates/pci_configuration_space` grew `bridge_buses` (the type-1 header's primary,
secondary and subordinate bus numbers), `Function` (what a census needs to print), and `walk`. The
walk descends breadth-first over a 256-bit visited set so that a cycle in firmware's bus numbering
terminates rather than recursing. `enumerate` is now `walk` with the topology dropped.

The cost argument the block leaned on turned out to be weak, and that is worth recording because
it nearly decided the fork. Measured rather than asserted, on `q35` under QEMU with 256 MiB:

| ECAM mapped | page tables |
|---|---|
| 1 bus (1 MiB), the old behavior | 560 KiB |
| 2 buses (the root-port topology) | 560 KiB, no measurable change |
| 128 buses (128 MiB), what xenon's MCFG describes | 812 KiB |

Option 1 would have cost 252 KiB on a boot spending 32,840 KiB on page tables, which is 0.8%,
not the catastrophe "128 MB of mapping" sounds like. So the recommendation stands on the reason the
block gave rather than the one it implied: would we still choose the bridge walk if both cost the
same? Yes, because a flat scan over a range ACPI happens to describe is the same species of
assumption that produced this bug, and because it issues configuration reads to bus numbers no
bridge on the machine decodes.

Option 3 (lazy mapping) was not needed and its machinery was not written. The reason is an ordering
fact rather than a cleverness: `pci::survey` runs from `kernel_main` before `arch::mmu::init`.
At that point the x86 boot tables still cover the low 4 GiB indiscriminately, so every bus the
MCFG describes is already readable at no mapping cost at all. `map_everything` runs afterwards and
maps exactly what the survey found. `arch::mmu::PCI_ECAM_BUSES` stops being the answer and becomes
the floor; `pci::ecam_buses()` is the answer, and all three architectures' `map_everything` ask it.

## What could not be tested, and why

- The real topology. Only xenon has it. `NIFE_PCIE_ROOT_PORT=1` on
  `helpers/qemu-runner-x86_64.sh` (provisional name) puts the NVMe behind a `pcie-root-port`, which
  is the *shape* xenon is believed to have, and `cargo xtask test --arch x86_64` runs one three-second
  boot against it. That gates the walk. It does not confirm anything about xenon.
- A working disk behind a bridge. A bridge forwards memory only inside the window its own
  base/limit registers describe; those are firmware's to write and QEMU's PVH boot has no firmware,
  so the controller on bus 1 enumerates and its BAR does not decode. On xenon, where firmware wrote
  both the bus numbers and the windows, milestone 256's adoption arm keeps every BAR inside a window
  that already works. See `design/roadmap/proposals/a-bridge-window-the-kernel-programs-itself.md`.
- A sparse bus numbering. The mapped range is contiguous (`0..=highest`), which is a waste and
  not a hazard on any machine measured. `notes/pcie.md`'s BUGS section carries it.

## Done when

- [x] The census prints every function on every bus the machine describes. **Built;
      xenon's own census lines were not relayed**, but the walk it feeds found the NVMe there.
- [x] `pci::find_nvme_device()` finds the Micron, or the census proves it is not there. Found on
      xenon 2026-10-04 at `01:00.0`, behind `00:1b.0`, and driven from EL0 in three bench boots.
- [x] `script/test` green on all three architectures; host tests for the bridge arithmetic in
      `crates/pci_configuration_space`, plus a Kani proof that the walk's queue stays inside its array whatever firmware
      wrote in the bridges.
- [x] The roadmap block carries what the census actually found.

## Index row

The kernel mapped one megabyte of PCI configuration space and enumerated one bus, so a controller
behind a root port was invisible to it. `pci::walk` reads the topology from the bridges, where
firmware wrote it, and the boot prints every function on every bus. The xenon confirmation is
outstanding.

## Follow-on

- **Milestone 325.** A bridge memory window the kernel programs itself
  (), raised by this lane. A
  device behind a bridge firmware left unconfigured enumerates and does not work, and nothing in the
  boot line says why.
- **Done.** (2026-10-04) The xenon boot that closes this block: the bench boots of
  `bench/xenon-2026-10-04/` found the Micron behind a root port.
