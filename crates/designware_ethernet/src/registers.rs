//! **The controller's register map**: offsets from the 64 KiB window's base, and the bits this
//! driver writes or reads.
//!
//! Transcribed from OpenBSD's `sys/dev/ic/dwqereg.h` (revision of 2026-07-31, commit
//! `59ec3ab4d107`, ISC license, notice in the crate root), with [`CONFIG_LM`] from Linux's
//! `stmmac/dwmac4.h` and [`DMA_CH0_RX_CONTROL_RBSZ_SHIFT`] from U-Boot's `dwc_eth_qos.h`. The MAC
//! and MTL are page `0x0000`; page `0x1000` holds the DMA channel's tail pointers beside its ring
//! bases and the controller's software reset, and there is no IOMMU, so a process given that page
//! can repoint a ring and reset the MAC.
//!
//! Each constant keeps its source's name (ruled 2026-10-26 UTC) without the family prefix, `GMAC_`
//! or `EQOS_`. A source macro that takes a channel or queue number, such as `GMAC_CHAN_CONTROL(x)`,
//! becomes the constant for instance 0 under the macro's name, here `CHAN_CONTROL`. Each
//! constant's documentation opens with its full name and its source's spelling.
//!
//! Name: provisional. Ruled 2026-10-26 (UTC): every `regs` module this tree names becomes
//! `registers`, replacing `regs`.

/// **MAC configuration register** (`GMAC_MAC_CONF`, `dwqereg.h`).
#[doc(alias = "mac configuration register")]
pub(crate) const MAC_CONF: u32 = 0x0000;
/// **Receiver enable** (`GMAC_MAC_CONF_RE`, `dwqereg.h`).
#[doc(alias = "receiver enable")]
pub(crate) const MAC_CONF_RE: u32 = 1 << 0;
/// **Transmitter enable** (`GMAC_MAC_CONF_TE`, `dwqereg.h`).
#[doc(alias = "transmitter enable")]
pub(crate) const MAC_CONF_TE: u32 = 1 << 1;
/// **Disable carrier sense during transmission** (`GMAC_MAC_CONF_DCRS`, `dwqereg.h`).
#[doc(alias = "disable carrier sense during transmission")]
pub const MAC_CONF_DCRS: u32 = 1 << 9;
/// **Loopback mode** (`GMAC_CONFIG_LM`, `dwmac4.h`): the MAC returns what it transmits to its own
/// receive path. The coherence probe uses it so the measurement does not depend on a cable or a
/// link partner.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): Decision 37c, the source's name copied strictly,
/// replacing `MAC_CONF_LM`.
#[doc(alias = "loopback mode")]
pub const CONFIG_LM: u32 = 1 << 12;
/// **Duplex mode** (`GMAC_MAC_CONF_DM`, `dwqereg.h`). Full duplex.
#[doc(alias = "duplex mode")]
pub const MAC_CONF_DM: u32 = 1 << 13;
/// **Fast Ethernet speed** (`GMAC_MAC_CONF_FES`, `dwqereg.h`). With [`MAC_CONF_PS`], 100
/// Mbit/s when set and 10 when clear.
#[doc(alias = "fast ethernet speed")]
pub const MAC_CONF_FES: u32 = 1 << 14;
/// **Port select** (`GMAC_MAC_CONF_PS`, `dwqereg.h`). Set for 10 or 100 Mbit/s (MII),
/// clear for 1000 (GMII/RGMII).
#[doc(alias = "port select")]
pub const MAC_CONF_PS: u32 = 1 << 15;
/// **Jabber disable** (`GMAC_MAC_CONF_JD`, `dwqereg.h`).
#[doc(alias = "jabber disable")]
pub const MAC_CONF_JD: u32 = 1 << 17;
/// **Packet burst enable** (`GMAC_MAC_CONF_BE`, `dwqereg.h`).
#[doc(alias = "packet burst enable")]
pub const MAC_CONF_BE: u32 = 1 << 18;

/// **MAC packet filter register** (`GMAC_MAC_PACKET_FILTER`, `dwqereg.h`). The receive packet
/// filter. Zero after reset: the station address and broadcast pass, multicast does not.
#[doc(alias = "mac packet filter register")]
pub(crate) const MAC_PACKET_FILTER: u32 = 0x0008;
/// **Transmit flow control register for queue 0** (`GMAC_QX_TX_FLOW_CTRL(0)`, `dwqereg.h`).
#[doc(alias = "transmit flow control register for queue 0")]
pub(crate) const QX_TX_FLOW_CTRL: u32 = 0x0070;
/// **Pause time field shift** (`GMAC_QX_TX_FLOW_CTRL_PT_SHIFT`, `dwqereg.h`), in
/// [`QX_TX_FLOW_CTRL`].
#[doc(alias = "pause time field shift")]
pub(crate) const QX_TX_FLOW_CTRL_PT_SHIFT: u32 = 16;
/// **Transmit flow control enable** (`GMAC_QX_TX_FLOW_CTRL_TFE`, `dwqereg.h`).
#[doc(alias = "transmit flow control enable")]
pub(crate) const QX_TX_FLOW_CTRL_TFE: u32 = 1 << 0;
/// **Receive flow control register** (`GMAC_RX_FLOW_CTRL`, `dwqereg.h`).
#[doc(alias = "receive flow control register")]
pub(crate) const RX_FLOW_CTRL: u32 = 0x0090;
/// **Receive flow control enable** (`GMAC_RX_FLOW_CTRL_RFE`, `dwqereg.h`).
#[doc(alias = "receive flow control enable")]
pub(crate) const RX_FLOW_CTRL_RFE: u32 = 1 << 0;
/// **Receive queue control register 0** (`GMAC_RXQ_CTRL0`, `dwqereg.h`): which receive queues are
/// enabled.
#[doc(alias = "receive queue control register 0")]
pub(crate) const RXQ_CTRL0: u32 = 0x00a0;
/// **Receive queue 0 enabled for data center bridging traffic** (`GMAC_RXQ_CTRL0_DCB_QUEUE_EN(0)`,
/// `dwqereg.h`), the generic traffic class. Without this the MAC receives nothing on the 4.x and
/// 5.x controllers, whatever the DMA is doing.
#[doc(alias = "receive queue enable")]
pub(crate) const RXQ_CTRL0_DCB_QUEUE_EN: u32 = 2;
/// **MAC interrupt enable register** (`GMAC_INT_EN`, `dwqereg.h`).
#[doc(alias = "mac interrupt enable register")]
pub(crate) const INT_EN: u32 = 0x00b4;
/// **Synopsys version register** (`GMAC_VERSION`, `dwqereg.h`); the low byte is the core version
/// (`0x52` for 5.20).
///
/// Name: provisional. Ruled 2026-10-26 (UTC): Decision 37c, the source's name copied strictly,
/// replacing `MAC_VERSION`.
#[doc(alias = "synopsys version register")]
pub const VERSION: u32 = 0x0110;
/// **MAC hardware feature register** (`GMAC_MAC_HW_FEATURE(x)`, `dwqereg.h`), register `i` for `i`
/// in `0..4`.
#[doc(alias = "mac hardware feature register")]
pub const fn mac_hw_feature(i: u32) -> u32 {
    0x011c + 4 * (i % 4)
}
/// **MDIO (management data input/output) address register** (`GMAC_MAC_MDIO_ADDR`, `dwqereg.h`),
/// which carries the command.
#[doc(alias = "mdio address register")]
pub const MAC_MDIO_ADDR: u32 = 0x0200;
/// **MDIO (management data input/output) data register** (`GMAC_MAC_MDIO_DATA`, `dwqereg.h`).
#[doc(alias = "mdio data register")]
pub const MAC_MDIO_DATA: u32 = 0x0204;
/// **MAC address 0, high register** (`GMAC_MAC_ADDR0_HI`, `dwqereg.h`): the station address's high
/// half (bytes 4 and 5).
#[doc(alias = "mac address 0, high register")]
pub const MAC_ADDR0_HI: u32 = 0x0300;
/// **MAC address 0, low register** (`GMAC_MAC_ADDR0_LO`, `dwqereg.h`): the station address's low
/// half (bytes 0 to 3).
#[doc(alias = "mac address 0, low register")]
pub const MAC_ADDR0_LO: u32 = 0x0304;
/// **MAC management counters, receive interrupt mask** (`GMAC_MMC_RX_INT_MASK`, `dwqereg.h`). All
/// ones masks every counter interrupt.
#[doc(alias = "mac management counters, receive interrupt mask")]
pub(crate) const MMC_RX_INT_MASK: u32 = 0x070c;
/// **MAC management counters, transmit interrupt mask** (`GMAC_MMC_TX_INT_MASK`, `dwqereg.h`).
#[doc(alias = "mac management counters, transmit interrupt mask")]
pub(crate) const MMC_TX_INT_MASK: u32 = 0x0710;

/// **MAC transaction layer transmit queue 0 operation mode** (`GMAC_MTL_CHAN_TX_OP_MODE(0)`,
/// `dwqereg.h`).
#[doc(alias = "mac transaction layer transmit queue 0 operation mode")]
pub(crate) const MTL_CHAN_TX_OP_MODE: u32 = 0x0d00;
/// **Flush transmit queue** (`GMAC_MTL_CHAN_TX_OP_MODE_FTQ`, `dwqereg.h`), self-clearing.
#[doc(alias = "flush transmit queue")]
pub(crate) const MTL_CHAN_TX_OP_MODE_FTQ: u32 = 1 << 0;
/// **Transmit store and forward** (`GMAC_MTL_CHAN_TX_OP_MODE_TSF`, `dwqereg.h`).
#[doc(alias = "transmit store and forward")]
pub(crate) const MTL_CHAN_TX_OP_MODE_TSF: u32 = 1 << 1;
/// **Transmit queue enable field** (`GMAC_MTL_CHAN_TX_OP_MODE_TXQEN_MASK`, `dwqereg.h`).
#[doc(alias = "transmit queue enable field")]
pub(crate) const MTL_CHAN_TX_OP_MODE_TXQEN_MASK: u32 = 0x3 << 2;
/// **Transmit queue enabled** (`GMAC_MTL_CHAN_TX_OP_MODE_TXQEN`, `dwqereg.h`).
#[doc(alias = "transmit queue enabled")]
pub(crate) const MTL_CHAN_TX_OP_MODE_TXQEN: u32 = 2 << 2;
/// **Transmit threshold control field** (`GMAC_MTL_CHAN_TX_OP_MODE_TTC_MASK`, `dwqereg.h`).
#[doc(alias = "transmit threshold control field")]
pub(crate) const MTL_CHAN_TX_OP_MODE_TTC_MASK: u32 = 0x7 << 4;
/// **Transmit threshold control, 512 bytes** (`GMAC_MTL_CHAN_TX_OP_MODE_TTC_512`, `dwqereg.h`).
/// Transmit threshold of 512 bytes, what OpenBSD picks in threshold mode.
#[doc(alias = "transmit threshold control, 512 bytes")]
pub(crate) const MTL_CHAN_TX_OP_MODE_TTC_512: u32 = 7 << 4;
/// **Transmit queue size field** (`GMAC_MTL_CHAN_TX_OP_MODE_TQS_MASK`, `dwqereg.h`).
#[doc(alias = "transmit queue size field")]
pub(crate) const MTL_CHAN_TX_OP_MODE_TQS_MASK: u32 = 0x1ff << 16;
/// **Transmit queue size shift** (`GMAC_MTL_CHAN_TX_OP_MODE_TQS_SHIFT`, `dwqereg.h`): the field
/// holds `bytes / 256 - 1`.
#[doc(alias = "transmit queue size shift")]
pub(crate) const MTL_CHAN_TX_OP_MODE_TQS_SHIFT: u32 = 16;
/// **MAC transaction layer receive queue 0 operation mode** (`GMAC_MTL_CHAN_RX_OP_MODE(0)`,
/// `dwqereg.h`).
#[doc(alias = "mac transaction layer receive queue 0 operation mode")]
pub(crate) const MTL_CHAN_RX_OP_MODE: u32 = 0x0d30;
/// **Receive threshold control field** (`GMAC_MTL_CHAN_RX_OP_MODE_RTC_MASK`, `dwqereg.h`).
#[doc(alias = "receive threshold control field")]
pub(crate) const MTL_CHAN_RX_OP_MODE_RTC_MASK: u32 = 0x3 << 3;
/// **Receive threshold control, 128 bytes** (`GMAC_MTL_CHAN_RX_OP_MODE_RTC_128`, `dwqereg.h`).
#[doc(alias = "receive threshold control, 128 bytes")]
pub(crate) const MTL_CHAN_RX_OP_MODE_RTC_128: u32 = 3 << 3;
/// **Receive store and forward** (`GMAC_MTL_CHAN_RX_OP_MODE_RSF`, `dwqereg.h`).
#[doc(alias = "receive store and forward")]
pub(crate) const MTL_CHAN_RX_OP_MODE_RSF: u32 = 1 << 5;
/// **Receive queue size field** (`GMAC_MTL_CHAN_RX_OP_MODE_RQS_MASK`, `dwqereg.h`).
#[doc(alias = "receive queue size field")]
pub(crate) const MTL_CHAN_RX_OP_MODE_RQS_MASK: u32 = 0x3ff << 20;
/// **Receive queue size shift** (`GMAC_MTL_CHAN_RX_OP_MODE_RQS_SHIFT`, `dwqereg.h`): the field
/// holds `bytes / 256 - 1`.
#[doc(alias = "receive queue size shift")]
pub(crate) const MTL_CHAN_RX_OP_MODE_RQS_SHIFT: u32 = 20;

/// **DMA (direct memory access) bus mode register** (`GMAC_BUS_MODE`, `dwqereg.h`).
#[doc(alias = "dma bus mode register")]
pub(crate) const BUS_MODE: u32 = 0x1000;
/// **Software reset** (`GMAC_BUS_MODE_SWR`, `dwqereg.h`) of the whole controller (MAC, MTL and
/// DMA); self-clearing. The bit that makes the DMA page an administrative page as well as a data
/// one.
#[doc(alias = "software reset")]
pub(crate) const BUS_MODE_SWR: u32 = 1 << 0;
/// **DMA system bus mode register** (`GMAC_SYS_BUS_MODE`, `dwqereg.h`), for the AXI bus.
#[doc(alias = "dma system bus mode register")]
pub(crate) const SYS_BUS_MODE: u32 = 0x1004;
/// **Fixed burst** (`GMAC_SYS_BUS_MODE_FB`, `dwqereg.h`).
#[doc(alias = "fixed burst")]
pub(crate) const SYS_BUS_MODE_FB: u32 = 1 << 0;
/// **Burst length 32 permitted** (`GMAC_SYS_BUS_MODE_BLEN_32`, `dwqereg.h`), on the AXI bus.
#[doc(alias = "burst length 32 permitted")]
pub(crate) const SYS_BUS_MODE_BLEN_32: u32 = 1 << 4;
/// **Burst length 64 permitted** (`GMAC_SYS_BUS_MODE_BLEN_64`, `dwqereg.h`), on the AXI bus.
#[doc(alias = "burst length 64 permitted")]
pub(crate) const SYS_BUS_MODE_BLEN_64: u32 = 1 << 5;
/// **Burst length 128 permitted** (`GMAC_SYS_BUS_MODE_BLEN_128`, `dwqereg.h`), on the AXI bus.
#[doc(alias = "burst length 128 permitted")]
pub(crate) const SYS_BUS_MODE_BLEN_128: u32 = 1 << 6;
/// **Burst length 256 permitted** (`GMAC_SYS_BUS_MODE_BLEN_256`, `dwqereg.h`), on the AXI bus.
#[doc(alias = "burst length 256 permitted")]
pub(crate) const SYS_BUS_MODE_BLEN_256: u32 = 1 << 7;
/// **Enhanced address mode enable** (`GMAC_SYS_BUS_MODE_EAME`, `dwqereg.h`): descriptor addresses
/// wider than 32 bits.
#[doc(alias = "enhanced address mode enable")]
pub(crate) const SYS_BUS_MODE_EAME: u32 = 1 << 11;
/// **Read outstanding request limit shift** (`GMAC_SYS_BUS_MODE_RD_OSR_LMT_SHIFT`, `dwqereg.h`).
#[doc(alias = "read outstanding request limit shift")]
pub(crate) const SYS_BUS_MODE_RD_OSR_LMT_SHIFT: u32 = 16;
/// **Write outstanding request limit shift** (`GMAC_SYS_BUS_MODE_WR_OSR_LMT_SHIFT`, `dwqereg.h`).
#[doc(alias = "write outstanding request limit shift")]
pub(crate) const SYS_BUS_MODE_WR_OSR_LMT_SHIFT: u32 = 24;
/// **Read outstanding request limit mask** (`GMAC_SYS_BUS_MODE_RD_OSR_LMT_MASK`, `dwqereg.h`).
#[doc(alias = "read outstanding request limit mask")]
pub(crate) const SYS_BUS_MODE_RD_OSR_LMT_MASK: u32 = 0xf << 16;
/// **Write outstanding request limit mask** (`GMAC_SYS_BUS_MODE_WR_OSR_LMT_MASK`, `dwqereg.h`).
#[doc(alias = "write outstanding request limit mask")]
pub(crate) const SYS_BUS_MODE_WR_OSR_LMT_MASK: u32 = 0xf << 24;

/// **DMA channel 0 control register** (`GMAC_CHAN_CONTROL(0)`, `dwqereg.h`).
#[doc(alias = "dma channel 0 control register")]
pub(crate) const CHAN_CONTROL: u32 = 0x1100;
/// **Eight times programmable burst length** (`GMAC_CHAN_CONTROL_8XPBL`, `dwqereg.h`), applied to
/// both burst lengths. Both JH7110 trees say `snps,no-pbl-x8`.
#[doc(alias = "eight times programmable burst length")]
pub(crate) const CHAN_CONTROL_8XPBL: u32 = 1 << 16;
/// **DMA channel 0 transmit control register** (`GMAC_CHAN_TX_CONTROL(0)`, `dwqereg.h`).
#[doc(alias = "dma channel 0 transmit control register")]
pub(crate) const CHAN_TX_CONTROL: u32 = 0x1104;
/// **Start transmission** (`GMAC_CHAN_TX_CONTROL_ST`, `dwqereg.h`).
#[doc(alias = "start transmission")]
pub(crate) const CHAN_TX_CONTROL_ST: u32 = 1 << 0;
/// **Operate on second packet** (`GMAC_CHAN_TX_CONTROL_OSP`, `dwqereg.h`): fetch the next
/// descriptor before the status of this one is written.
#[doc(alias = "operate on second packet")]
pub(crate) const CHAN_TX_CONTROL_OSP: u32 = 1 << 4;
/// **Transmit programmable burst length shift** (`GMAC_CHAN_TX_CONTROL_PBL_SHIFT`, `dwqereg.h`).
#[doc(alias = "transmit programmable burst length shift")]
pub(crate) const CHAN_TX_CONTROL_PBL_SHIFT: u32 = 16;
/// **Transmit programmable burst length mask** (`GMAC_CHAN_TX_CONTROL_PBL_MASK`, `dwqereg.h`).
#[doc(alias = "transmit programmable burst length mask")]
pub(crate) const CHAN_TX_CONTROL_PBL_MASK: u32 = 0x3f << 16;
/// **DMA channel 0 receive control register** (`GMAC_CHAN_RX_CONTROL(0)`, `dwqereg.h`).
#[doc(alias = "dma channel 0 receive control register")]
pub(crate) const CHAN_RX_CONTROL: u32 = 0x1108;
/// **Start receive** (`GMAC_CHAN_RX_CONTROL_SR`, `dwqereg.h`).
#[doc(alias = "start receive")]
pub(crate) const CHAN_RX_CONTROL_SR: u32 = 1 << 0;
/// **Receive buffer size shift** (`EQOS_DMA_CH0_RX_CONTROL_RBSZ_SHIFT`, `dwc_eth_qos.h`): the
/// field holds bytes, bits 14:1 (Linux's `DMA_RBSZ_MASK`). OpenBSD never writes it and relies on
/// the reset value; this driver writes it because a field nobody wrote is a fact about whoever
/// ran last.
#[doc(alias = "receive buffer size shift")]
pub(crate) const DMA_CH0_RX_CONTROL_RBSZ_SHIFT: u32 = 1;
/// **Receive buffer size mask** (`EQOS_DMA_CH0_RX_CONTROL_RBSZ_MASK`, `dwc_eth_qos.h`), applied
/// after the shift.
#[doc(alias = "receive buffer size mask")]
pub(crate) const DMA_CH0_RX_CONTROL_RBSZ_MASK: u32 = 0x3fff;
/// **Receive programmable burst length shift** (`GMAC_CHAN_RX_CONTROL_RPBL_SHIFT`, `dwqereg.h`).
#[doc(alias = "receive programmable burst length shift")]
pub(crate) const CHAN_RX_CONTROL_RPBL_SHIFT: u32 = 16;
/// **Receive programmable burst length mask** (`GMAC_CHAN_RX_CONTROL_RPBL_MASK`, `dwqereg.h`).
#[doc(alias = "receive programmable burst length mask")]
pub(crate) const CHAN_RX_CONTROL_RPBL_MASK: u32 = 0x3f << 16;
/// **Transmit descriptor list base address, high register** (`GMAC_CHAN_TX_BASE_ADDR_HI(0)`,
/// `dwqereg.h`), the high 32 bits.
#[doc(alias = "transmit descriptor list base address, high register")]
pub(crate) const CHAN_TX_BASE_ADDR_HI: u32 = 0x1110;
/// **Transmit descriptor list base address register** (`GMAC_CHAN_TX_BASE_ADDR(0)`, `dwqereg.h`),
/// the low 32 bits.
#[doc(alias = "transmit descriptor list base address register")]
pub(crate) const CHAN_TX_BASE_ADDR: u32 = 0x1114;
/// **Receive descriptor list base address, high register** (`GMAC_CHAN_RX_BASE_ADDR_HI(0)`,
/// `dwqereg.h`), the high 32 bits.
#[doc(alias = "receive descriptor list base address, high register")]
pub(crate) const CHAN_RX_BASE_ADDR_HI: u32 = 0x1118;
/// **Receive descriptor list base address register** (`GMAC_CHAN_RX_BASE_ADDR(0)`, `dwqereg.h`),
/// the low 32 bits.
#[doc(alias = "receive descriptor list base address register")]
pub(crate) const CHAN_RX_BASE_ADDR: u32 = 0x111c;
/// **Transmit descriptor tail pointer** (`GMAC_CHAN_TX_END_ADDR(0)`, `dwqereg.h`): the address
/// one past the last descriptor handed to the device.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): Decision 37c, the source's name copied strictly,
/// replacing `CHAN_TX_TAIL`.
#[doc(alias = "transmit descriptor tail pointer")]
pub const CHAN_TX_END_ADDR: u32 = 0x1120;
/// **Receive descriptor tail pointer** (`GMAC_CHAN_RX_END_ADDR(0)`, `dwqereg.h`), the same for
/// the receive ring.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): Decision 37c, the source's name copied strictly,
/// replacing `CHAN_RX_TAIL`.
#[doc(alias = "receive descriptor tail pointer")]
pub const CHAN_RX_END_ADDR: u32 = 0x1128;
/// **Transmit descriptor ring length** (`GMAC_CHAN_TX_RING_LEN(0)`, `dwqereg.h`): the descriptor
/// count minus one.
#[doc(alias = "transmit descriptor ring length")]
pub(crate) const CHAN_TX_RING_LEN: u32 = 0x112c;
/// **Receive descriptor ring length** (`GMAC_CHAN_RX_RING_LEN(0)`, `dwqereg.h`): the descriptor
/// count minus one.
#[doc(alias = "receive descriptor ring length")]
pub(crate) const CHAN_RX_RING_LEN: u32 = 0x1130;
/// **DMA channel 0 interrupt enable register** (`GMAC_CHAN_INTR_ENA(0)`, `dwqereg.h`). This driver
/// polls and writes zero.
#[doc(alias = "dma channel 0 interrupt enable register")]
pub(crate) const CHAN_INTR_ENA: u32 = 0x1134;
/// **Current transmit descriptor** (`GMAC_CHAN_CUR_TX_DESC(0)`, `dwqereg.h`). The descriptor the
/// transmit DMA is on, as the device sees it.
#[doc(alias = "current transmit descriptor")]
pub const CHAN_CUR_TX_DESC: u32 = 0x1144;
/// **Current receive descriptor** (`GMAC_CHAN_CUR_RX_DESC(0)`, `dwqereg.h`). The descriptor the
/// receive DMA is on, as the device sees it.
#[doc(alias = "current receive descriptor")]
pub const CHAN_CUR_RX_DESC: u32 = 0x114c;
/// **DMA channel 0 status register** (`GMAC_CHAN_STATUS(0)`, `dwqereg.h`). Write one to clear.
#[doc(alias = "dma channel 0 status register")]
pub const CHAN_STATUS: u32 = 0x1160;
/// **Transmit interrupt** (`GMAC_CHAN_STATUS_TI`, `dwqereg.h`): transmit complete.
#[doc(alias = "transmit interrupt")]
pub(crate) const CHAN_STATUS_TI: u32 = 1 << 0;
/// **Transmit process stopped** (`GMAC_CHAN_STATUS_TPS`, `dwqereg.h`).
#[doc(alias = "transmit process stopped")]
pub const CHAN_STATUS_TPS: u32 = 1 << 1;
/// **Transmit buffer unavailable** (`GMAC_CHAN_STATUS_TBU`, `dwqereg.h`): the DMA fetched a
/// descriptor it does not own and suspended.
#[doc(alias = "transmit buffer unavailable")]
pub(crate) const CHAN_STATUS_TBU: u32 = 1 << 2;
/// **Receive interrupt** (`GMAC_CHAN_STATUS_RI`, `dwqereg.h`): receive complete.
#[doc(alias = "receive interrupt")]
pub(crate) const CHAN_STATUS_RI: u32 = 1 << 6;
/// **Receive buffer unavailable** (`GMAC_CHAN_STATUS_RBU`, `dwqereg.h`): the DMA fetched a
/// receive descriptor it does not own.
#[doc(alias = "receive buffer unavailable")]
pub(crate) const CHAN_STATUS_RBU: u32 = 1 << 7;
/// **Receive process stopped** (`GMAC_CHAN_STATUS_RPS`, `dwqereg.h`).
#[doc(alias = "receive process stopped")]
pub const CHAN_STATUS_RPS: u32 = 1 << 8;
/// **Fatal bus error** (`GMAC_CHAN_STATUS_FBE`, `dwqereg.h`): the DMA's own access to memory
/// failed.
#[doc(alias = "fatal bus error")]
pub(crate) const CHAN_STATUS_FBE: u32 = 1 << 12;

/// The page of the window holding channel 0's registers, which is the page the data plane is
/// mapped. Every `CHAN_*` and `DMA_*` offset above is inside it.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `dma` spelled out as `direct_memory_access`,
/// replacing `DMA_PAGE`.
#[doc(alias = "dma_page")]
pub const DIRECT_MEMORY_ACCESS_REGISTER_PAGE: u32 = 0x1000;
/// The window's size, `reg`'s `0x10000` in both JH7110 trees.
pub const WINDOW: u32 = 0x1_0000;

#[cfg(test)]
mod tests {
    use super::*;

    /// Every register the data plane writes or reads is on the DMA page, which is the one page
    /// the process is mapped; a register that moved off it would be a store into nothing.
    #[test]
    fn the_data_plane_registers_are_all_on_the_direct_memory_access_register_page() {
        for r in [
            CHAN_TX_END_ADDR,
            CHAN_RX_END_ADDR,
            CHAN_STATUS,
            CHAN_CUR_RX_DESC,
            BUS_MODE,
        ] {
            assert_eq!(r & !0xfff, DIRECT_MEMORY_ACCESS_REGISTER_PAGE, "{r:#x}");
        }
    }

    /// The MAC's administrative registers are not on it: the process cannot reach the MDIO bus
    /// or the station address through the page it is given.
    #[test]
    fn the_management_data_input_output_bus_and_the_station_address_are_not_on_the_direct_memory_access_register_page()
     {
        for r in [
            MAC_MDIO_ADDR,
            MAC_MDIO_DATA,
            MAC_ADDR0_HI,
            MAC_PACKET_FILTER,
            MAC_CONF,
        ] {
            assert_ne!(r & !0xfff, DIRECT_MEMORY_ACCESS_REGISTER_PAGE, "{r:#x}");
        }
    }

    /// **The bits as the header spells them**, written out rather than shifted, so a slip in one
    /// constant is caught here and not by a controller that ignores a write. The driver and the
    /// simulated controller both read these names, so only a literal can tell a wrong bit from a
    /// right one. The `1 << 0` bits are absent: their shift cannot go wrong.
    #[test]
    fn the_bits_are_where_dwqereg_puts_them() {
        assert_eq!(
            [
                MAC_CONF_DCRS,
                CONFIG_LM,
                MAC_CONF_DM,
                MAC_CONF_FES,
                MAC_CONF_PS,
                MAC_CONF_JD,
                MAC_CONF_BE,
            ],
            [0x200, 0x1000, 0x2000, 0x4000, 0x8000, 0x2_0000, 0x4_0000]
        );
        assert_eq!(
            [
                MTL_CHAN_TX_OP_MODE_TSF,
                MTL_CHAN_TX_OP_MODE_TXQEN_MASK,
                MTL_CHAN_TX_OP_MODE_TXQEN,
                MTL_CHAN_TX_OP_MODE_TTC_MASK,
                MTL_CHAN_TX_OP_MODE_TTC_512,
                MTL_CHAN_TX_OP_MODE_TQS_MASK,
            ],
            [0x2, 0xc, 0x8, 0x70, 0x70, 0x01ff_0000]
        );
        assert_eq!(
            [
                MTL_CHAN_RX_OP_MODE_RTC_MASK,
                MTL_CHAN_RX_OP_MODE_RTC_128,
                MTL_CHAN_RX_OP_MODE_RSF,
                MTL_CHAN_RX_OP_MODE_RQS_MASK,
            ],
            [0x18, 0x18, 0x20, 0x3ff0_0000]
        );
        assert_eq!(
            [
                SYS_BUS_MODE_BLEN_32,
                SYS_BUS_MODE_BLEN_64,
                SYS_BUS_MODE_BLEN_128,
                SYS_BUS_MODE_BLEN_256,
                SYS_BUS_MODE_EAME,
            ],
            [0x10, 0x20, 0x40, 0x80, 0x800]
        );
        assert_eq!([CHAN_CONTROL_8XPBL, CHAN_TX_CONTROL_OSP], [0x1_0000, 0x10]);
        assert_eq!(
            [
                CHAN_STATUS_TPS,
                CHAN_STATUS_TBU,
                CHAN_STATUS_RI,
                CHAN_STATUS_RBU,
                CHAN_STATUS_RPS,
                CHAN_STATUS_FBE,
            ],
            [0x2, 0x4, 0x40, 0x80, 0x100, 0x1000]
        );
    }

    /// The four hardware feature registers are consecutive words from `0x11c`, and an index past
    /// them wraps rather than reading some other register.
    #[test]
    fn the_feature_registers_are_four_words_from_0x11c() {
        assert_eq!(
            [0, 1, 2, 3, 4, 5].map(mac_hw_feature),
            [0x11c, 0x120, 0x124, 0x128, 0x11c, 0x120]
        );
    }
}
