---
status: IN-PROGRESS
raised: 2026-08-25
branch: milestone/163-jh7110-pcie-root-complex
milestone_dependencies: none
decision_dependencies: none
machine_requirements: JH7110 silicon
specific_machine: radon (the first run, with a drive in the M.2 slot)
needs_person: yes
---
# 163. The JH7110's PCIe root complex: a real driver for the PLDA XpressRICH controller

Milestone 53 (board peripherals), `notes/visionfive2.md`'s "PCIe" row and §86 (the NVMe driver
leaves the kernel) each name this as needing its own milestone. This block is that milestone.

## What it needs

Drive the JH7110's PCIe root complex, a PLDA XpressRICH controller wrapped in StarFive glue
(`starfive,jh7110-pcie` in mainline device trees), so that the kernel's existing PCI path
(`memory::pci_regions`, `mmu::map_everything` step 9, `kernel/src/pci.rs`) finds the devices behind
it. QEMU's `pci-host-ecam-generic` is a different device: firmware there leaves a working
Enhanced Configuration Access Mechanism (ECAM) window, and nothing on the JH7110 does until the
bridge is brought up and its address translation is programmed.

The JH7110 has two ports. On a VisionFive 2, port 1 (`pcie@9c0000000`) is the M.2 M-key slot and
port 0 (`pcie@940000000`) carries the VL805 USB 3 controller. The first target is port 1, because
the NVMe path behind it is already built (§86, option 2a) and polls, so it needs no interrupt to
give a first data point. Port 0 is what §260 (radon's block server serves a USB drive) waits for.

## Why it matters, and what it unblocks

- §86 asks for a second data point beyond QEMU, to tell which parts of the NVMe capability contract
  are QEMU artifacts. A real NVMe drive behind a real root complex is that data point.
- Milestone 53's storage half on the VisionFive 2 is NVMe over PCIe.
- §260's storage path runs through port 0 and the VL805.

## Sources

Every register and every step is transcribed from mainline Linux at `torvalds/linux`
`cbbddb6b08cc`, the revision the JH7110 crates already pin:

- `drivers/pci/controller/plda/pcie-plda.h` and `pcie-plda-host.c`: the XpressRICH bridge
  registers, the address translation (ATR) table, the interrupt bits.
- `drivers/pci/controller/plda/pcie-starfive.c`: the StarFive glue and the bring-up order.
- `drivers/phy/starfive/phy-jh7110-pcie.c`: the PHY's mode and tuning writes.
- `arch/riscv/boot/dts/starfive/jh7110.dtsi` and `jh7110-common.dtsi`: windows, clocks, resets,
  the PERST# GPIO.
- `include/dt-bindings/clock/starfive,jh7110-crg.h` and `.../reset/starfive,jh7110-crg.h`: the STG
  clock and reset numbers.

No BSD has a driver for this controller, so the spelled-out register names carry the Linux
spelling as their `doc(alias)`.

## The bring-up, in order

As `starfive_pcie_host_init` and `plda_pcie_host_init` do it:

1. PHY into PCIe mode: tuning words at PHY `0x28` and `0x80`. Port 0 also clears the STG syscon's
   mode, bus-width and USB 3 PHY bits and the SYS syscon's split-reset bit. Port 1's PHY has no
   syscon arguments in the tree, so only the tuning writes apply.
2. STG syscon: set `K_RP_NEP`, select the reference clock source (2), set `CLKREQ`.
3. Clocks on and resets released: `NOC_BUS_STG_AXI` in SYS, and the port's `TL`, `AXI_MST0` and
   `APB` clocks and six resets in STG.
4. Assert PERST# (the port's `perst-gpios`, active low).
5. Disable PHY functions 1 to 3 through the STG syscon's `AR` and `AW` selects and the bridge's
   `PCI_MISC`, then clear both selects.
6. Bridge: enable the root port (`GEN_SETTINGS`), zero the root port's BAR0 and BAR1, set the class
   to PCI-to-PCI bridge, disable LTR messages, advertise 64-bit prefetchable windows.
7. Wait 100 ms, release PERST#, wait 100 ms, then poll the STG syscon's `DATA_LINK_ACTIVE`.
8. ATR window 0 maps the `cfg` window as the configuration interface, which makes it ECAM-shaped.
   Window 1 maps the 32-bit memory range one to one; window 2 the 64-bit prefetchable range.
9. Root port at `00:00.0`: primary bus 0, secondary 1, subordinate 1; memory base and limit
   covering the 32-bit window; memory decoding and bus mastering on. Nothing in the kernel assigns
   bridge buses today (`pci.rs` reads them), and the NVMe controller sits at bus 1.
10. Record `(cfg, mem32)` with `memory::record_pci_regions`, so step 9 of `map_everything` maps them
    and `pci::find_nvme_device` finds the controller.

## Steps that need no hardware

- [x] The register layout and the bring-up as a pure crate, `crates/jh7110_pci_express`
      (name provisional). It holds the offsets and bits, the ATR window encoder, the ordered
      sequence over an injected register bus, the link-up check, and root-port bus and window
      assignment.
- [x] A simulated root complex in that crate's tests. Its link comes up only after a correct
      sequence, and its `cfg` window decodes as ECAM only once ATR window 0 maps it. Its NVMe
      endpoint at `01:00.0` answers only once the root port forwards bus 1, and the tests run
      `pci_configuration_space::walk` over it.
- [x] Kani harnesses for the ATR window encoder (size field, alignment, enable bit) and the bridge
      bus-number word.
- [ ] `machine_discovery::jh7110::pci_express`: each enabled `starfive,jh7110-pcie` node's `cfg` and
      `apb` windows by `reg-names`, its 32-bit and prefetchable ranges, its PERST# GPIO, and which
      port it is.
- [ ] `jh7110_clock_and_reset`: the PCIe clocks and resets of each port as constants and plans.
- [ ] Kernel wiring: `memory` records the port, `mmu::map_everything` maps its bridge, syscon, PHY
      and GPIO pages, a kernel driver walks the plan before the PCI survey, and records the regions.
      QEMU's `virt` names no such node, so every CI boot is unchanged.
- [ ] `notes/jh7110-pci-express-bench.md`: the bench steps for a person.

## Steps that need the board

- [ ] The vendor U-Boot's tree: whether it carries a `starfive,jh7110-pcie` node in mainline's shape
      (`reg-names`, `ranges`, `perst-gpios`). If not, the kernel needs a fallback to the constants,
      as the clock and reset discovery has.
- [ ] Link up on port 1 with a drive fitted: the `DATA_LINK_ACTIVE` poll succeeds.
- [ ] Enumeration: `00:00.0` reads as a bridge and `01:00.0` as class `0x010802`.
- [ ] The NVMe path's identify and a read, through `pci::find_nvme_device`.
- [ ] Port 0 and the VL805, for §260.

## Open decisions

These are the lane's provisional choices. Each is a two-way door: nothing outside the tree depends
on it yet.

1. **Where the bring-up runs.** Recommended: in the kernel, beside `pci.rs`, on the boot thread
   before the PCI survey. Enumeration, BAR placement and the ECAM mapping are already kernel work,
   and the bring-up is one pass at boot. The alternative is a root-complex server under milestone
   1062's device control planes, which would also move enumeration out; that is a larger change
   than this milestone and can follow it.
2. **One port or two.** Recommended: port 1 only for now. `memory::pci_regions` holds one host, so
   two ports need `pci.rs` to carry two ECAM windows and two segment numbers. Port 0 follows with
   that change.
3. **Clocks and resets through the kernel's own walk.** Recommended: the kernel's
   `drivers::jh7110_clock_and_reset::bring_up`, as the watchdog uses it, rather than the clock and
   reset server, because the bring-up runs before any server is spawned. The single-writer argument
   that walk documents still holds: both run on the boot thread.
4. **PERST# through the GPIO controller's output registers directly.** The tree has no JH7110 GPIO
   driver. Recommended: the crate writes the one pin's output and output-enable fields, as
   `pinctrl-starfive-jh7110.c` does, rather than adding a GPIO driver for one pin.

## Index row

The JH7110's PLDA XpressRICH root complex, brought up from the kernel so that the existing PCI path
finds the devices behind it: the M.2 NVMe slot first, then the VL805 USB 3 controller. It gives
§86 its real-hardware data point and milestone 53 and §260 their storage path on the VisionFive 2.
