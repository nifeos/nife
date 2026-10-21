# What each option costs, board by board

An appendix to [milestone 192 (a keyboard on real silicon)](../0192-keyboard-on-real-silicon.md).
It prices options A and B on argon, radon and xenon from public device trees, the boards' own
schematics and the tree's drivers. Nothing here was run on a board. Read 2026-10-21 (UTC) against
nife `6d83f1838`.

## The finding that reframes the question

Option B's driver exists. Milestone 242 (USB host and HID) built it under QEMU on all three
architectures in one pull request, #1629, opened 2026-10-04 20:03 and merged 22:49 (UTC). How
many hours the lane spent before opening it git does not record, so this is not an hour count. It
does settle that the QEMU half is not a months-sized job. The stack is about 4,250 lines:

| Piece | Lines (with comments) |
|---|---|
| `crates/extensible_host_controller_interface` (logic, host tests, Kani) | 1,940 |
| `crates/usb` (descriptors, requests, boot reports) | 1,001 |
| `components/src/usb_keyboard_driver.rs` (the EL0 driver) | 899 |
| `kernel/src/extensible_host_controller_interface.rs` and `kernel/src/user/usb_keyboard_service.rs` | 410 |

So the price of B is no longer the xHCI driver. It is what stands between that driver and each
board's port. That runs from no known code on xenon to several thousand lines and an open design
question on radon and argon.

## How the line estimates are scaled

Linux is the only reference with drivers for all three boards. Two drivers exist in both trees,
and they bound how nife's line count compares with Linux's for the same device. A third pair,
against U-Boot, checks the xHCI-sized end:

| Device | Reference | nife | Ratio |
|---|---|---|---|
| JH7110 watchdog | Linux `starfive-wdt.c`, 615 | `crates/jh7110_watchdog` lib and the kernel half, 411 | 0.7 |
| JH7110 TRNG | Linux `jh7110-trng.c`, 399 | crate, component and EL0 service, 1,887 | 4.7 |
| xHCI core | U-Boot `xhci.c`, `xhci-mem.c`, `xhci-ring.c`, 3,418 | the xHCI crate, the driver and the kernel half, 3,027 | 0.9 |

The low end is a register poke with no confinement. The high end carries Kani proofs, a
falsification record and a confined EL0 service around a small device. The xHCI pair is confined
and proved too, and still lands at 0.9, so the proof overhead shrinks as the device grows. Every
estimate below is a reference count times 0.7 to 4.7, and none is better than that range. Linux
and U-Boot counts are `master` as fetched on 2026-10-21 (UTC).

## xenon (OptiPlex 7050 Micro, x86_64)

- Controller: one Intel 200-series PCH xHCI, `00:14.0 8086:a2af class 0c0330`
  (`bench/xenon-2026-10-04/install-and-disk-boot.log`). It is the only USB function in that
  survey, so every port is on it. Whether a port reaches it through a hub a PCI survey cannot say,
  since a hub is a USB device; Intel's PCH design puts the ports on the controller's own root
  ports. 242 records it as MSI with no MSI-X, which is what its QEMU gate models.
- What nife has: all of it. PCI enumeration, the VT-d units this controller sits behind (milestone
  594, BUILT on xenon 2026-10-04, whose bench boot records the screen intact), and the xHCI driver.
- Option A: the serial half is built (COM1, milestones 299 and 505), and the boot reaches xenon's
  monitor (594's bench boot). A `graphical_terminal` session on x86_64 is unproven even under QEMU,
  because the runners have no virtio-gpu (milestone 270 (wire `virtio-gpu-pci` and
  `virtio-input` into the x86_64 test runner)), so A on xenon is a QEMU leg and a bench
  run away.
- Option B: no new code is known to be needed. What is left is 242's bench step, one evening of
  calef's.
- Confidence: high that no new subsystem is needed. Medium that it works on the first boot. The
  driver has never met real silicon, every timeout is the specification's, and cheap keyboards are
  where boot protocol breaks (242's BUGS).

## radon (VisionFive 2, JH7110, riscv64)

- Controllers: the JH7110 has one on-chip Cadence USB controller. The board's device tree runs it
  as a peripheral (`&usb0 { dr_mode = "peripheral"; }`), so it is no host path. The four USB ports
  are on a VIA VL805 xHCI behind the SoC's first PCIe root port; the M.2 slot is on the second
  (VisionFive 2 V1.2A schematic, U-Boot's board doc). No DWC3 is involved.
- What nife has: the xHCI driver, which would drive a VL805 as it stands. Linux needs two quirks
  for it (`xhci-pci.c`): a 36-bit DMA mask, which radon's memory fits under, and `TRB_OVERFETCH`,
  the controller reading past a ring segment's end. The second matters to confinement, since the
  overread must still land inside the driver's region. Upstream Linux loads no VL805 firmware on
  this board, so it is taken to run from its own flash (inferred from absence, not read).
- What it lacks, in order:
  1. Milestone 163 (the JH7110's PCIe root complex), the PLDA XpressRICH controller. Linux's is
     1,190 lines (`pcie-starfive.c` 505, `pcie-plda-host.c` 685), so 800 to 5,600 here, plus the
     bench work 163 says a lane cannot do. Medium confidence on size; the hardware risk is unpriced.
  2. Confinement. The JH7110 has no IOMMU, and the kernel refuses to start the xHCI driver without
     one, by design. The way out that keeps the design is a validator at the doorbell (§86 (whether
     an NVMe driver can leave the kernel)'s option 4). For NVMe that is two range checks per entry.
     For xHCI it is not. The controller follows pointers in the device context array, every input
     context, the command ring, each transfer ring and its link TRBs, the event ring segment table
     and the scratchpad array. It also reads rings on its own schedule rather than once per
     doorbell. The validator would have to own the rings the controller reads, which moves most of
     the ring code into the kernel's trust. Its nearest size reference is
     `crates/direct_memory_access_validator`, 1,340 lines, for one virtqueue shape. One to three
     times that, 1,300 to 4,000. Low confidence, and the larger cost is a design call, not lines.
- Option A: waits on milestone 157 (U-Boot's `simple-framebuffer` handoff), NOT-STARTED. 192's
  bench procedure is written for radon.
- Option B: 163, then a confinement answer that does not exist yet, then the bench. This is the
  board where B is genuinely large, and its storage (§260 (radon's block server serves a
  USB drive)) waits on the same two items.

## argon (Jetson TX1, Tegra210, aarch64)

- Controllers: one Tegra XUSB host at `0x70090000`, an xHCI behind NVIDIA glue, which also owns the
  micro-USB port as OTG beside a device-only controller (`tegra210-p2597.dtsi`). The pad controller
  gives the Type-A port the pair usb2-2 and usb3-1. The board's gigabit Ethernet is a USB device on
  usb2-1 and usb3-0, so a keyboard driver and the network share one controller. No hub, no PCIe in
  the path.
- What nife has: the xHCI ring logic, and nothing else on this path. The kernel finds xHCI only by
  PCI class code (`find_xhci_device`), and XUSB is a device-tree node with no config space. nife's
  aarch64 IOMMU is SMMUv3, and Tegra's SMMU is NVIDIA's own design (notes/aarch64-board-survey.md).
  nife does not boot on argon at all yet: milestone 127 (the seL4 machine) is NOT-STARTED and the
  kernel is linked for QEMU's memory map (notes/boot-stick/argon.md).
- What XUSB needs before its xHCI registers answer, from Linux:
  - The Falcon microcontroller loaded with NVIDIA's `nvidia/tegra210/xusb.bin` (linux-firmware,
    redistributable under NVIDIA's license), by DMA, through the `fpci` and `ipfs` windows.
    `xhci-tegra.c` is 2,877 lines for four SoCs, its power management included.
  - The pad controller and its PLLs: `xusb.c` 1,551 and `xusb-tegra210.c` 3,299, much of it the
    PCIe and SATA lanes a keyboard does not need.
  - Clocks, resets and two power partitions (`xusb_host`, `xusb_ss`) through Tegra's clock
    controller and power management controller, which nife has no driver for.
  - VBUS for the Type-A port, a regulator the board switches by GPIO.
  - Tegra's SMMU, to meet the confinement rule: `tegra-smmu.c` 1,192 plus the Tegra210 client
    table. The memory controller lists an `XUSB_HOST` client group, but mainline's device tree puts
    no `iommus` on the XUSB node, so XUSB behind the SMMU is supported by the tables and
    unexercised upstream. nife's own SMMUv3 driver, 593 lines, is the shape to compare.
- Mainline U-Boot for this board (`p2371-2180_defconfig`) builds the Tegra EHCI driver and no
  xHCI, and its device tree enables only `usb@7d000000`, in OTG mode: the micro-USB port. So
  whatever U-Boot leaves running is not XUSB, and nife cannot inherit a warm controller.
- An alternative that avoids XUSB: a keyboard on the micro-USB port through an OTG adapter, driven
  by that EHCI controller. It needs no firmware and no Falcon, and U-Boot shows the pad bring-up
  fits in `ehci-tegra.c`, 983 lines. Its `HOSTPC1_PSPD` field reports a full-speed device on the
  root port directly, which reads as an integrated transaction translator (read from U-Boot, not
  the manual). The cost is a second host controller driver: U-Boot's `ehci-hcd.c` is 1,763 lines,
  so 1,600 to 2,000 at the xHCI pair's ratio, plus the Tegra glue. Whether Tegra's SMMU covers this
  controller was not read. Medium-low confidence; it is a cheaper path to one keystroke and a dead
  end for USB 3, and it is not an option 192 records.
- Estimate for the glue alone, taking a third of the Linux Tegra lines as the keyboard's share:
  roughly 2,600 Linux lines, so 1,800 to 12,000 here. The SMMU driver adds 600 to 1,500, scaled
  from nife's SMMUv3 rather than Linux. Low confidence. All of it sits after 127.
- Option A: needs 127 first, then a framebuffer handoff on argon. 157 is written for radon's
  display path, and whether argon's U-Boot hands over a framebuffer was not read.
- Option B: the largest of the three, and the one whose firmware nife does not write.

## Summary

| | Option A | Option B |
|---|---|---|
| xenon | a QEMU graphical leg on x86_64 (270), then the bench | built; 242's bench evening (high confidence no code) |
| radon | milestone 157, then the bench | 163 (800 to 5,600 lines plus bench), then a confinement design for xHCI without an IOMMU (1,300 to 4,000, low) |
| argon | 127, then a framebuffer path nothing owns | 127, then XUSB glue (1,800 to 12,000, low), a Tegra SMMU (600 to 1,500, low) and NVIDIA's firmware; or an EHCI driver on the micro-USB port (1,600 to 2,000 plus glue, medium-low) |

calef's bar for 192 is one machine with a keyboard and a monitor plugged into it. On that bar the
cheapest finished story is option B on xenon, which costs a bench evening and no new code.

## BUGS

- Every line range is a ratio applied to Linux, set by two small drivers; the one DMA pair is
  against U-Boot, not Linux. A Linux DMA driver nife also has would narrow the range.
- The VL805's firmware source on radon and the Tegra SMMU's coverage of XUSB are read from absence
  in Linux, not from a datasheet. The JH7110 and Tegra210 technical reference manuals were not
  read.
- No price is in hours. #1629's elapsed time is the only timing datum, and git does not record when
  its lane began.
