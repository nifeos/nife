# The JH7110's PCI Express root complex

*Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.*

The JH7110 has two PCI Express root ports, each a PLDA XpressRICH controller inside StarFive glue.
On a VisionFive 2, port 1 is the M.2 M-key slot and port 0 carries the VL805 USB 3 controller.
`crates/jh7110_pci_express` holds what it takes to bring one up, as logic over an injected register
bus. Milestone 163 (the JH7110's PCIe root complex) holds the plan and what still needs the board.

## Why the kernel's PCI path needs a bring-up first

Under QEMU, the `pci-host-ecam-generic` window answers configuration reads from the first
instruction, because QEMU emulates a host bridge that firmware left working. Nothing on the JH7110
runs before the kernel that does the same. Until the bridge is a root port with its link trained
and its address translation programmed, a read of the `cfg` window decodes as nothing.

Three things have to happen, in this order:

1. The bring-up (`bring_up`): the PHY into PCI Express mode, the glue told it is a root port,
   clocks on, PERST# held and released, the link polled.
2. The translation table: entry 0 maps the `cfg` window as the configuration interface, which
   makes it ECAM-shaped. Entries 1 and 2 map the 32-bit and prefetchable memory ranges one to one.
3. The root port (`root_port::configure`): bus numbers 0, 1, 1 and a memory window over the
   32-bit range. Firmware does this on a PC. Here the root port reads zero bus numbers and forwards
   nothing, so the walk in `pci_configuration_space` sees only `00:00.0` until this runs.

After the third step, `pci_configuration_space::walk` finds the drive at `01:00.0` exactly as it
finds QEMU's devices, and `kernel/src/pci.rs` places its BAR in the 32-bit range.

## Where the kernel does it

`memory` records port 1 from its tree node, or the reason it took none. `mmu::map_everything` step
6f maps the pages `drivers::jh7110_pci_express::regions` names: two pages of the bridge, one each of
the STG system controller, the PHY and the SYS GPIO controller, two buses of `cfg`, and the BAR
slice of the 32-bit range. `pci::bring_up_jh7110_root_port` runs on the boot thread before the
IOMMU starts. It walks the bring-up through `drivers::jh7110_pci_express::Controller`, configures
the root port, and only then calls `memory::record_pci_regions`, so no PCI walk ever reads a host
that does not answer. Every line it prints starts with `jh7110 pcie:`, and
[jh7110-pci-express-bench.md](jh7110-pci-express-bench.md) reads them. QEMU's `virt` names no such
node, so the function returns at once there.

## The sequence, and where each step comes from

The order is Linux's `starfive_pcie_host_init` followed by `plda_pcie_host_init`, at mainline
`cbbddb6b08cc`. The crate documentation lists the files. No BSD drives this controller, so each
spelled-out constant carries its Linux name as a `doc(alias)`, and searching the documentation for
`GEN_SETTINGS` or `STG_SYSCON_K_RP_NEP` finds the constant.

| step | block | what |
|---|---|---|
| PHY | PHY, STG, SYS | KVCO tuning on both ports; port 0 also leaves USB 3 mode |
| glue | STG | root port not endpoint, reference clock source 2, CLKREQ# |
| clocks | SYS, STG | `NOC_BUS_STG_AXI`, then the port's three clocks and six resets |
| PERST# | SYS GPIO | driven low, output enabled |
| functions | STG, bridge | functions 1 to 3 disabled, then both selects cleared |
| bridge | bridge | root port enabled, BARs zero, class `0x0604`, LTR off, 64-bit prefetchable |
| link | SYS GPIO, STG | 100 ms, PERST# high, 100 ms, then up to ten reads 90 ms apart |
| windows | bridge | translation entries 0, 1 and 2 |

A link that never comes up is not a failure of the bring-up: Linux continues and so does the crate.
The root port still answers at `00:00.0`, which tells an empty slot from a dead controller.

## The simulation

The crate's tests run against `simulation::Simulated`, a model of the five register windows. Its
link trains only when the PHY is in PCI Express mode, the glue says root port with clock source
2, the clocks are on, the bridge is a root port, and PERST# was held for 100 ms and released. Its
`cfg` window decodes only once entry 0 maps it as configuration space. Its drive at `01:00.0`
answers only once the root port forwards bus 1.

The model encodes what Linux's sequence implies the silicon needs. It cannot show that the silicon
needs nothing more, and only the board can show that.

## BUGS

- The PERST# pad's input enable, Schmitt trigger and bias stay as reset leaves them. Linux clears
  them when it makes the pin an output.
- The root port forwards one bus, so a switch behind it would need more bus numbers.
- No interrupts: the NVMe path polls. The bridge's interrupt registers are in `bridge` for the
  work that adds them.
- One port at a time: `memory::pci_regions` holds one host, so port 0 and the VL805 wait for
  `pci.rs` to carry two ECAM windows.
- No fallback to constants: a tree that names the port under other `reg-names`, as StarFive's own
  kernel tree has, leaves the port off. The bench runbook's step 1 records what the board's tree
  says.
- The walk has no only-one-child rule. If this bridge answers every device number on bus 1, the
  drive appears 32 times.
- Nobody has measured whether an unsupported request reads as all ones or raises a bus error, and
  the walk reads every device number on bus 1.
- Nobody has measured the PCI Express master's DMA coherence. A measurement on the same board
  found the Ethernet controller's coherent.
