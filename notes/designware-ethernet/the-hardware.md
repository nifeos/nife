# radon's Ethernet: the hardware facts the crate is built on

A page beside [`notes/designware-ethernet.md`](../designware-ethernet.md). It holds the reference
tables that `crates/designware_ethernet`'s module comments link to: where the controller's
registers fall, how the two device trees describe the controller and its PHY, and how the receive
ring's tail pointer is kept. Each fact here is also the reason for a line of code in that crate.

## Where the register pages fall

The register map in `regs.rs` is transcribed from OpenBSD's `sys/dev/ic/dwqereg.h` (commit
`59ec3ab4d107`, 2026-07-31). That is the header OpenBSD's JH7110 driver is built on. Two bits come
from elsewhere: Linux's `stmmac/dwmac4.h` names `MAC_CONF_LM`, which OpenBSD does not, and U-Boot's
`dwc_eth_qos.h` gives the receive buffer size field, `CHAN_RX_CONTROL_RBSZ_SHIFT`, which OpenBSD never
writes. Only what the driver uses is transcribed.

| page | holds |
|---|---|
| `0x0000` | MAC: configuration, packet filter, flow control, interrupt enables, version, hardware features, MDIO, the station address |
| `0x0c00` (still page 0) | MTL: the queue operation modes |
| `0x1000` | DMA: the bus mode (with software reset), the system bus mode, and channel 0's control, ring bases, ring lengths, tail pointers and status |

So the channel's tail pointers share a page with its ring bases and with the bit that resets the
whole controller. A process that can move a tail can repoint a ring and reset the MAC, and the
JH7110 has no IOMMU in front of this device. The parent note's "The confinement, stated plainly"
carries what follows from that, and so does the header of
`kernel/src/user/designware_ethernet_service.rs`.

## Two device trees, and radon serves the vendor's

radon's U-Boot hands the kernel its own control tree, built from StarFive's U-Boot fork, not
mainline Linux's (notes/visionfive2.md, and milestone 239 (radon's device tree does not describe the
TRNG, so a working driver never runs)'s correction). The two describe the same controller in different words:

| | vendor U-Boot (`JH7110_VisionFive2_devel`) | mainline Linux |
|---|---|---|
| `compatible` | `starfive,jh7110-eqos-5.20` | `starfive,jh7110-dwmac`, `snps,dwmac-5.20` |
| PHY | child node `ethernet-phy@0`, register values | `phy-handle` to `mdio/ethernet-phy@0`, picoseconds and microamps |
| interface select | not in the tree: board code writes `AON_SYSCON_BASE + AON_SYSCFG_12`, bits `0x1c0000` | `starfive,syscon = <&aon_syscon 0xc 0x12>` |

Both say `0x1603_0000` and `0x1604_0000`, `phy-mode = "rgmii-id"`, interrupts 7 and 78, and the
same AXI configuration (`jh7110::BUS`). `jh7110.rs` follows OpenBSD's `if_dwqe_fdt.c`
(`dwqe_fdt_attach`, `dwqe_setup_jh7110`). The controller's address picks the port, the `phy-mode`
picks the interface select, and the PHY's node supplies its delays.

## The YT8531's settings, in both spellings

The vendor tree states the PHY's register values directly, and mainline Linux states the physical
quantities they encode. For radon's `gmac0` (VisionFive 2 v1.3B) they agree:

| quantity | vendor U-Boot (`starfive_visionfive2.dts`) | mainline Linux (`...-v1.3b.dts`) | register value |
|---|---|---|---|
| receive clock delay | `rx_delay_sel = <0xa>`, `rxc_dly_en = <0>` | `rx-internal-delay-ps = <1500>` | `0xa`, delay-enable clear |
| transmit clock delay | `tx_delay_sel = <0xa>` | `tx-internal-delay-ps = <1500>` | `0xa` |
| receive clock drive | `rgmii_sw_dr_rxc = <0x6>` | `motorcomm,rx-clk-drv-microamp = <3970>` | `6` at 1.8 V |
| receive data drive | `rgmii_sw_dr = <0x3>` | `motorcomm,rx-data-drv-microamp = <2910>` | `3` at 1.8 V |
| transmit clock inverted at 1000 | `tx_inverted_1000 = <0x1>` | `motorcomm,tx-clk-1000-inverted` | set |

`motorcomm::Settings::FALLBACK_GMAC0` is the last column. It is used only when a tree states
neither spelling, and `Settings::source` says which happened.

The sequence in `motorcomm.rs` is OpenBSD's `ytphy_yt8521_init`, `ytphy_yt8531_init` and
`ytphy_yt8521_update` (`sys/dev/mii/ytphy.c`, revision 1.6 of 2024-03-12, commit `9d7a05f92003`).
It sets the RGMII delays, the receive pad drive strengths, the clock output, and the transmit clock
inversion chosen per negotiated speed. Two registers OpenBSD does not touch,
`EXT_SLEEP_CONTROL1` and `EXT_CLOCK_GATING`, come from Linux's `drivers/net/phy/motorcomm.c`. They
were read as facts about the part and not as code, and each constant's comment says why the driver
writes it.

## The receive tail pointer, and the slot that is never the device's

A tail pointer is the address one past the last descriptor the device may use. The device works
from its current descriptor up to the tail and suspends there; these are OpenBSD's
`GMAC_CHAN_RX_END_ADDR` and `GMAC_CHAN_TX_END_ADDR` writes.

On receive, one descriptor is always held back. Otherwise "the device has every descriptor" and
"the device has none" would be the same tail address. OpenBSD's
`if_rxr_init(..., DWQE_NRXDESC - 1)` makes the same choice. The held-back slot is the one just
behind `DataPlane::rx_cursor`. Each received frame reposts it and holds back the slot just
consumed.
