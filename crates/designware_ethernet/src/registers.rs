//! **The controller's register map**: offsets from the 64 KiB window's base, and the bits this
//! driver writes or reads.
//!
//! Transcribed from OpenBSD's `sys/dev/ic/dwqereg.h` (revision of 2026-07-31, commit
//! `59ec3ab4d107`, ISC license, notice in the crate root), with [`MAC_CONFIGURATION_LOOPBACK_MODE`] from Linux's
//! `stmmac/dwmac4.h` and `CHANNEL_RECEIVE_CONTROL_BUFFER_SIZE_SHIFT` from U-Boot's `dwc_eth_qos.h`. The media
//! access control (MAC) and MTL are page `0x0000`; page `0x1000` holds the DMA channel's tail pointers beside its ring
//! bases and the controller's software reset, and there is no IOMMU, so a process given that page
//! can repoint a ring and reset the MAC.
//!
//! Each constant spells out what the source abbreviates, and the source's own spelling (`GMAC_` or
//! `EQOS_` prefix included) is in its documentation and in a `#[doc(alias)]`. A source macro that
//! takes a channel or queue number, such as `GMAC_CHAN_CONTROL(x)`, becomes the constant for
//! instance 0, here `CHANNEL_CONTROL_REGISTER`. Each constant's documentation opens with its full
//! name and its source's spelling.
//!
//! Name: ratified 2026-11-02 (UTC), #3094.

/// **MAC configuration register** (`GMAC_MAC_CONF`, `dwqereg.h`).
#[doc(alias = "mac configuration register")]
#[doc(alias = "GMAC_MAC_CONF")]
pub(crate) const MAC_CONFIGURATION_REGISTER: u32 = 0x0000;
/// **Receiver enable** (`GMAC_MAC_CONF_RE`, `dwqereg.h`).
#[doc(alias = "receiver enable")]
#[doc(alias = "GMAC_MAC_CONF_RE")]
pub(crate) const MAC_CONFIGURATION_RECEIVER_ENABLE: u32 = 1 << 0;
/// **Transmitter enable** (`GMAC_MAC_CONF_TE`, `dwqereg.h`).
#[doc(alias = "transmitter enable")]
#[doc(alias = "GMAC_MAC_CONF_TE")]
pub(crate) const MAC_CONFIGURATION_TRANSMITTER_ENABLE: u32 = 1 << 1;
/// **Disable carrier sense during transmission** (`GMAC_MAC_CONF_DCRS`, `dwqereg.h`).
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_MAC_CONF_DCRS")]
#[doc(alias = "MAC_CONF_DCRS")]
pub const MAC_CONFIGURATION_DISABLE_CARRIER_SENSE_DURING_TRANSMISSION: u32 = 1 << 9;
/// **Loopback mode** (`GMAC_CONFIG_LM`, `dwmac4.h`): the MAC returns what it transmits to its own
/// receive path. The coherence probe uses it so the measurement does not depend on a cable or a
/// link partner. Linux calls it `CONFIG_LM` in `dwmac4.h`; the media access control (MAC)
/// configuration register's bit is also spelled `MAC_CONF_LM`.
///
/// Name: ratified 2026-11-02 (UTC), #3094.
#[doc(alias = "loopback mode")]
#[doc(alias = "CONFIG_LM")]
#[doc(alias = "MAC_CONF_LM")]
pub const MAC_CONFIGURATION_LOOPBACK_MODE: u32 = 1 << 12;
/// **Full duplex** (`GMAC_MAC_CONF_DM`, `dwqereg.h`): the bit is set for full duplex and clear
/// for half. The databook calls the bit Duplex Mode.
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_MAC_CONF_DM")]
#[doc(alias = "MAC_CONF_DM")]
pub const MAC_CONFIGURATION_FULL_DUPLEX: u32 = 1 << 13;
/// **Fast Ethernet speed** (`GMAC_MAC_CONF_FES`, `dwqereg.h`). With
/// [`MAC_CONFIGURATION_PORT_SELECT`], 100 Mbit/s when set and 10 when clear.
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_MAC_CONF_FES")]
#[doc(alias = "MAC_CONF_FES")]
pub const MAC_CONFIGURATION_FAST_ETHERNET_SPEED: u32 = 1 << 14;
/// **Port select** (`GMAC_MAC_CONF_PS`, `dwqereg.h`). Set for 10 or 100 Mbit/s (MII),
/// clear for 1000 (GMII/RGMII).
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_MAC_CONF_PS")]
#[doc(alias = "MAC_CONF_PS")]
pub const MAC_CONFIGURATION_PORT_SELECT: u32 = 1 << 15;
/// **Jabber disable** (`GMAC_MAC_CONF_JD`, `dwqereg.h`).
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_MAC_CONF_JD")]
#[doc(alias = "MAC_CONF_JD")]
pub const MAC_CONFIGURATION_JABBER_DISABLE: u32 = 1 << 17;
/// **Packet burst enable** (`GMAC_MAC_CONF_BE`, `dwqereg.h`).
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_MAC_CONF_BE")]
#[doc(alias = "MAC_CONF_BE")]
pub const MAC_CONFIGURATION_PACKET_BURST_ENABLE: u32 = 1 << 18;

/// **MAC packet filter register** (`GMAC_MAC_PACKET_FILTER`, `dwqereg.h`). The receive packet
/// filter. Zero after reset: the station address and broadcast pass, multicast does not.
#[doc(alias = "mac packet filter register")]
pub(crate) const MAC_PACKET_FILTER: u32 = 0x0008;
/// **Transmit flow control register for queue 0** (`GMAC_QX_TX_FLOW_CTRL(0)`, `dwqereg.h`).
#[doc(alias = "transmit flow control register for queue 0")]
#[doc(alias = "GMAC_QX_TX_FLOW_CTRL")]
pub(crate) const TRANSMIT_FLOW_CONTROL_QUEUE_0_REGISTER: u32 = 0x0070;
/// **Pause time field shift** (`GMAC_QX_TX_FLOW_CTRL_PT_SHIFT`, `dwqereg.h`), in
/// [`TRANSMIT_FLOW_CONTROL_QUEUE_0_REGISTER`].
#[doc(alias = "pause time field shift")]
#[doc(alias = "GMAC_QX_TX_FLOW_CTRL_PT_SHIFT")]
pub(crate) const TRANSMIT_FLOW_CONTROL_QUEUE_0_PAUSE_TIME_SHIFT: u32 = 16;
/// **Transmit flow control enable** (`GMAC_QX_TX_FLOW_CTRL_TFE`, `dwqereg.h`).
#[doc(alias = "transmit flow control enable")]
#[doc(alias = "GMAC_QX_TX_FLOW_CTRL_TFE")]
pub(crate) const TRANSMIT_FLOW_CONTROL_QUEUE_0_ENABLE: u32 = 1 << 0;
/// **Receive flow control register** (`GMAC_RX_FLOW_CTRL`, `dwqereg.h`).
#[doc(alias = "receive flow control register")]
#[doc(alias = "GMAC_RX_FLOW_CTRL")]
pub(crate) const RECEIVE_FLOW_CONTROL_REGISTER: u32 = 0x0090;
/// **Receive flow control enable** (`GMAC_RX_FLOW_CTRL_RFE`, `dwqereg.h`).
#[doc(alias = "receive flow control enable")]
#[doc(alias = "GMAC_RX_FLOW_CTRL_RFE")]
pub(crate) const RECEIVE_FLOW_CONTROL_ENABLE: u32 = 1 << 0;
/// **Receive queue control register 0** (`GMAC_RXQ_CTRL0`, `dwqereg.h`): which receive queues are
/// enabled.
#[doc(alias = "receive queue control register 0")]
#[doc(alias = "GMAC_RXQ_CTRL0")]
pub(crate) const RECEIVE_QUEUE_CONTROL_0_REGISTER: u32 = 0x00a0;
/// **Receive queue 0 enabled for data center bridging traffic** (`GMAC_RXQ_CTRL0_DCB_QUEUE_EN(0)`,
/// `dwqereg.h`), the generic traffic class. Without this the MAC receives nothing on the 4.x and
/// 5.x controllers, whatever the DMA is doing.
#[doc(alias = "receive queue enable")]
#[doc(alias = "GMAC_RXQ_CTRL0_DCB_QUEUE_EN")]
pub(crate) const RECEIVE_QUEUE_CONTROL_0_DATA_CENTER_BRIDGING_QUEUE_ENABLE: u32 = 2;
/// **MAC interrupt enable register** (`GMAC_INT_EN`, `dwqereg.h`).
#[doc(alias = "mac interrupt enable register")]
#[doc(alias = "GMAC_INT_EN")]
pub(crate) const MAC_INTERRUPT_ENABLE_REGISTER: u32 = 0x00b4;
/// **Synopsys version register** (`GMAC_VERSION`, `dwqereg.h`); the low byte is the core version
/// (`0x52` for 5.20).
///
/// Name: ratified 2026-11-02 (UTC), #3094.
#[doc(alias = "synopsys version register")]
#[doc(alias = "VERSION")]
#[doc(alias = "GMAC_VERSION")]
pub const CORE_VERSION_REGISTER: u32 = 0x0110;
/// **MAC hardware feature register** (`GMAC_MAC_HW_FEATURE(x)`, `dwqereg.h`), register `i` for `i`
/// in `0..4`.
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_MAC_HW_FEATURE")]
pub const fn mac_hardware_feature_register(i: u32) -> u32 {
    0x011c + 4 * (i % 4)
}
/// **MDIO (management data input/output) address register** (`GMAC_MAC_MDIO_ADDR`, `dwqereg.h`),
/// which carries the command.
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_MAC_MDIO_ADDR")]
#[doc(alias = "MAC_MDIO_ADDR")]
pub const MAC_MANAGEMENT_DATA_INPUT_OUTPUT_ADDRESS_REGISTER: u32 = 0x0200;
/// **MDIO (management data input/output) data register** (`GMAC_MAC_MDIO_DATA`, `dwqereg.h`).
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_MAC_MDIO_DATA")]
#[doc(alias = "MAC_MDIO_DATA")]
pub const MAC_MANAGEMENT_DATA_INPUT_OUTPUT_DATA_REGISTER: u32 = 0x0204;
/// **MAC address 0, high register** (`GMAC_MAC_ADDR0_HI`, `dwqereg.h`): the station address's high
/// half (bytes 4 and 5).
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_MAC_ADDR0_HI")]
#[doc(alias = "MAC_ADDR0_HI")]
pub const MAC_ADDRESS_0_HIGH_REGISTER: u32 = 0x0300;
/// **MAC address 0, low register** (`GMAC_MAC_ADDR0_LO`, `dwqereg.h`): the station address's low
/// half (bytes 0 to 3).
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_MAC_ADDR0_LO")]
#[doc(alias = "MAC_ADDR0_LO")]
pub const MAC_ADDRESS_0_LOW_REGISTER: u32 = 0x0304;
/// **MAC management counters, receive interrupt mask** (`GMAC_MMC_RX_INT_MASK`, `dwqereg.h`). All
/// ones masks every counter interrupt.
#[doc(alias = "mac management counters, receive interrupt mask")]
#[doc(alias = "GMAC_MMC_RX_INT_MASK")]
pub(crate) const MAC_MANAGEMENT_COUNTERS_RECEIVE_INTERRUPT_MASK_REGISTER: u32 = 0x070c;
/// **MAC management counters, transmit interrupt mask** (`GMAC_MMC_TX_INT_MASK`, `dwqereg.h`).
#[doc(alias = "mac management counters, transmit interrupt mask")]
#[doc(alias = "GMAC_MMC_TX_INT_MASK")]
pub(crate) const MAC_MANAGEMENT_COUNTERS_TRANSMIT_INTERRUPT_MASK_REGISTER: u32 = 0x0710;

/// **MAC transaction layer transmit queue 0 operation mode** (`GMAC_MTL_CHAN_TX_OP_MODE(0)`,
/// `dwqereg.h`).
#[doc(alias = "mac transaction layer transmit queue 0 operation mode")]
#[doc(alias = "GMAC_MTL_CHAN_TX_OP_MODE")]
pub(crate) const TRANSMIT_QUEUE_0_OPERATION_MODE_REGISTER: u32 = 0x0d00;
/// **Flush transmit queue** (`GMAC_MTL_CHAN_TX_OP_MODE_FTQ`, `dwqereg.h`), self-clearing.
#[doc(alias = "flush transmit queue")]
#[doc(alias = "GMAC_MTL_CHAN_TX_OP_MODE_FTQ")]
pub(crate) const TRANSMIT_QUEUE_0_OPERATION_MODE_FLUSH: u32 = 1 << 0;
/// **Transmit store and forward** (`GMAC_MTL_CHAN_TX_OP_MODE_TSF`, `dwqereg.h`).
#[doc(alias = "transmit store and forward")]
#[doc(alias = "GMAC_MTL_CHAN_TX_OP_MODE_TSF")]
pub(crate) const TRANSMIT_QUEUE_0_OPERATION_MODE_STORE_AND_FORWARD: u32 = 1 << 1;
/// **Transmit queue enable field** (`GMAC_MTL_CHAN_TX_OP_MODE_TXQEN_MASK`, `dwqereg.h`).
#[doc(alias = "transmit queue enable field")]
#[doc(alias = "GMAC_MTL_CHAN_TX_OP_MODE_TXQEN_MASK")]
pub(crate) const TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_ENABLE_MASK: u32 = 0x3 << 2;
/// **Transmit queue enabled** (`GMAC_MTL_CHAN_TX_OP_MODE_TXQEN`, `dwqereg.h`).
#[doc(alias = "transmit queue enabled")]
#[doc(alias = "GMAC_MTL_CHAN_TX_OP_MODE_TXQEN")]
pub(crate) const TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_ENABLED: u32 = 2 << 2;
/// **Transmit threshold control field** (`GMAC_MTL_CHAN_TX_OP_MODE_TTC_MASK`, `dwqereg.h`).
#[doc(alias = "transmit threshold control field")]
#[doc(alias = "GMAC_MTL_CHAN_TX_OP_MODE_TTC_MASK")]
pub(crate) const TRANSMIT_QUEUE_0_OPERATION_MODE_THRESHOLD_CONTROL_MASK: u32 = 0x7 << 4;
/// **Transmit threshold control, 512 bytes** (`GMAC_MTL_CHAN_TX_OP_MODE_TTC_512`, `dwqereg.h`).
/// Transmit threshold of 512 bytes, what OpenBSD picks in threshold mode.
#[doc(alias = "transmit threshold control, 512 bytes")]
#[doc(alias = "GMAC_MTL_CHAN_TX_OP_MODE_TTC_512")]
pub(crate) const TRANSMIT_QUEUE_0_OPERATION_MODE_THRESHOLD_CONTROL_512_BYTES: u32 = 7 << 4;
/// **Transmit queue size field** (`GMAC_MTL_CHAN_TX_OP_MODE_TQS_MASK`, `dwqereg.h`).
#[doc(alias = "transmit queue size field")]
#[doc(alias = "GMAC_MTL_CHAN_TX_OP_MODE_TQS_MASK")]
pub(crate) const TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_SIZE_MASK: u32 = 0x1ff << 16;
/// **Transmit queue size shift** (`GMAC_MTL_CHAN_TX_OP_MODE_TQS_SHIFT`, `dwqereg.h`): the field
/// holds `bytes / 256 - 1`.
#[doc(alias = "transmit queue size shift")]
#[doc(alias = "GMAC_MTL_CHAN_TX_OP_MODE_TQS_SHIFT")]
pub(crate) const TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_SIZE_SHIFT: u32 = 16;
/// **MAC transaction layer receive queue 0 operation mode** (`GMAC_MTL_CHAN_RX_OP_MODE(0)`,
/// `dwqereg.h`).
#[doc(alias = "mac transaction layer receive queue 0 operation mode")]
#[doc(alias = "GMAC_MTL_CHAN_RX_OP_MODE")]
pub(crate) const RECEIVE_QUEUE_0_OPERATION_MODE_REGISTER: u32 = 0x0d30;
/// **Receive threshold control field** (`GMAC_MTL_CHAN_RX_OP_MODE_RTC_MASK`, `dwqereg.h`).
#[doc(alias = "receive threshold control field")]
#[doc(alias = "GMAC_MTL_CHAN_RX_OP_MODE_RTC_MASK")]
pub(crate) const RECEIVE_QUEUE_0_OPERATION_MODE_THRESHOLD_CONTROL_MASK: u32 = 0x3 << 3;
/// **Receive threshold control, 128 bytes** (`GMAC_MTL_CHAN_RX_OP_MODE_RTC_128`, `dwqereg.h`).
#[doc(alias = "receive threshold control, 128 bytes")]
#[doc(alias = "GMAC_MTL_CHAN_RX_OP_MODE_RTC_128")]
pub(crate) const RECEIVE_QUEUE_0_OPERATION_MODE_THRESHOLD_CONTROL_128_BYTES: u32 = 3 << 3;
/// **Receive store and forward** (`GMAC_MTL_CHAN_RX_OP_MODE_RSF`, `dwqereg.h`).
#[doc(alias = "receive store and forward")]
#[doc(alias = "GMAC_MTL_CHAN_RX_OP_MODE_RSF")]
pub(crate) const RECEIVE_QUEUE_0_OPERATION_MODE_STORE_AND_FORWARD: u32 = 1 << 5;
/// **Receive queue size field** (`GMAC_MTL_CHAN_RX_OP_MODE_RQS_MASK`, `dwqereg.h`).
#[doc(alias = "receive queue size field")]
#[doc(alias = "GMAC_MTL_CHAN_RX_OP_MODE_RQS_MASK")]
pub(crate) const RECEIVE_QUEUE_0_OPERATION_MODE_QUEUE_SIZE_MASK: u32 = 0x3ff << 20;
/// **Receive queue size shift** (`GMAC_MTL_CHAN_RX_OP_MODE_RQS_SHIFT`, `dwqereg.h`): the field
/// holds `bytes / 256 - 1`.
#[doc(alias = "receive queue size shift")]
#[doc(alias = "GMAC_MTL_CHAN_RX_OP_MODE_RQS_SHIFT")]
pub(crate) const RECEIVE_QUEUE_0_OPERATION_MODE_QUEUE_SIZE_SHIFT: u32 = 20;

/// **DMA (direct memory access) bus mode register** (`GMAC_BUS_MODE`, `dwqereg.h`).
#[doc(alias = "dma bus mode register")]
#[doc(alias = "GMAC_BUS_MODE")]
pub(crate) const DIRECT_MEMORY_ACCESS_BUS_MODE_REGISTER: u32 = 0x1000;
/// **Software reset** (`GMAC_BUS_MODE_SWR`, `dwqereg.h`) of the whole controller (MAC, MTL and
/// DMA); self-clearing. The bit that makes the DMA page an administrative page as well as a data
/// one.
#[doc(alias = "software reset")]
#[doc(alias = "GMAC_BUS_MODE_SWR")]
pub(crate) const DIRECT_MEMORY_ACCESS_BUS_MODE_SOFTWARE_RESET: u32 = 1 << 0;
/// **DMA system bus mode register** (`GMAC_SYS_BUS_MODE`, `dwqereg.h`), for the AXI bus.
#[doc(alias = "dma system bus mode register")]
#[doc(alias = "GMAC_SYS_BUS_MODE")]
pub(crate) const DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_REGISTER: u32 = 0x1004;
/// **Fixed burst** (`GMAC_SYS_BUS_MODE_FB`, `dwqereg.h`).
#[doc(alias = "fixed burst")]
#[doc(alias = "GMAC_SYS_BUS_MODE_FB")]
pub(crate) const DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_FIXED_BURST: u32 = 1 << 0;
/// **Burst length 32 permitted** (`GMAC_SYS_BUS_MODE_BLEN_32`, `dwqereg.h`), on the AXI bus.
#[doc(alias = "burst length 32 permitted")]
#[doc(alias = "GMAC_SYS_BUS_MODE_BLEN_32")]
pub(crate) const DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_32_PERMITTED: u32 = 1 << 4;
/// **Burst length 64 permitted** (`GMAC_SYS_BUS_MODE_BLEN_64`, `dwqereg.h`), on the AXI bus.
#[doc(alias = "burst length 64 permitted")]
#[doc(alias = "GMAC_SYS_BUS_MODE_BLEN_64")]
pub(crate) const DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_64_PERMITTED: u32 = 1 << 5;
/// **Burst length 128 permitted** (`GMAC_SYS_BUS_MODE_BLEN_128`, `dwqereg.h`), on the AXI bus.
#[doc(alias = "burst length 128 permitted")]
#[doc(alias = "GMAC_SYS_BUS_MODE_BLEN_128")]
pub(crate) const DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_128_PERMITTED: u32 = 1 << 6;
/// **Burst length 256 permitted** (`GMAC_SYS_BUS_MODE_BLEN_256`, `dwqereg.h`), on the AXI bus.
#[doc(alias = "burst length 256 permitted")]
#[doc(alias = "GMAC_SYS_BUS_MODE_BLEN_256")]
pub(crate) const DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_256_PERMITTED: u32 = 1 << 7;
/// **Enhanced address mode enable** (`GMAC_SYS_BUS_MODE_EAME`, `dwqereg.h`): descriptor addresses
/// wider than 32 bits.
#[doc(alias = "enhanced address mode enable")]
#[doc(alias = "GMAC_SYS_BUS_MODE_EAME")]
pub(crate) const DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_ENHANCED_ADDRESS_MODE_ENABLE: u32 = 1 << 11;
/// **Read outstanding request limit shift** (`GMAC_SYS_BUS_MODE_RD_OSR_LMT_SHIFT`, `dwqereg.h`).
#[doc(alias = "read outstanding request limit shift")]
#[doc(alias = "GMAC_SYS_BUS_MODE_RD_OSR_LMT_SHIFT")]
pub(crate) const DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_READ_OUTSTANDING_REQUEST_LIMIT_SHIFT: u32 =
    16;
/// **Write outstanding request limit shift** (`GMAC_SYS_BUS_MODE_WR_OSR_LMT_SHIFT`, `dwqereg.h`).
#[doc(alias = "write outstanding request limit shift")]
#[doc(alias = "GMAC_SYS_BUS_MODE_WR_OSR_LMT_SHIFT")]
pub(crate) const DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_WRITE_OUTSTANDING_REQUEST_LIMIT_SHIFT: u32 =
    24;
/// **Read outstanding request limit mask** (`GMAC_SYS_BUS_MODE_RD_OSR_LMT_MASK`, `dwqereg.h`).
#[doc(alias = "read outstanding request limit mask")]
#[doc(alias = "GMAC_SYS_BUS_MODE_RD_OSR_LMT_MASK")]
pub(crate) const DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_READ_OUTSTANDING_REQUEST_LIMIT_MASK: u32 =
    0xf << 16;
/// **Write outstanding request limit mask** (`GMAC_SYS_BUS_MODE_WR_OSR_LMT_MASK`, `dwqereg.h`).
#[doc(alias = "write outstanding request limit mask")]
#[doc(alias = "GMAC_SYS_BUS_MODE_WR_OSR_LMT_MASK")]
pub(crate) const DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_WRITE_OUTSTANDING_REQUEST_LIMIT_MASK: u32 =
    0xf << 24;

/// **DMA channel 0 control register** (`GMAC_CHAN_CONTROL(0)`, `dwqereg.h`).
#[doc(alias = "dma channel 0 control register")]
#[doc(alias = "GMAC_CHAN_CONTROL")]
pub(crate) const CHANNEL_CONTROL_REGISTER: u32 = 0x1100;
/// **Eight times programmable burst length** (`GMAC_CHAN_CONTROL_8XPBL`, `dwqereg.h`), applied to
/// both burst lengths. Both JH7110 trees say `snps,no-pbl-x8`.
#[doc(alias = "eight times programmable burst length")]
#[doc(alias = "GMAC_CHAN_CONTROL_8XPBL")]
pub(crate) const CHANNEL_CONTROL_EIGHT_TIMES_PROGRAMMABLE_BURST_LENGTH: u32 = 1 << 16;
/// **DMA channel 0 transmit control register** (`GMAC_CHAN_TX_CONTROL(0)`, `dwqereg.h`).
#[doc(alias = "dma channel 0 transmit control register")]
#[doc(alias = "GMAC_CHAN_TX_CONTROL")]
pub(crate) const CHANNEL_TRANSMIT_CONTROL_REGISTER: u32 = 0x1104;
/// **Start transmission** (`GMAC_CHAN_TX_CONTROL_ST`, `dwqereg.h`).
#[doc(alias = "start transmission")]
#[doc(alias = "GMAC_CHAN_TX_CONTROL_ST")]
pub(crate) const CHANNEL_TRANSMIT_CONTROL_START_TRANSMISSION: u32 = 1 << 0;
/// **Operate on second packet** (`GMAC_CHAN_TX_CONTROL_OSP`, `dwqereg.h`): fetch the next
/// descriptor before the status of this one is written.
#[doc(alias = "operate on second packet")]
#[doc(alias = "GMAC_CHAN_TX_CONTROL_OSP")]
pub(crate) const CHANNEL_TRANSMIT_CONTROL_OPERATE_ON_SECOND_PACKET: u32 = 1 << 4;
/// **Transmit programmable burst length shift** (`GMAC_CHAN_TX_CONTROL_PBL_SHIFT`, `dwqereg.h`).
#[doc(alias = "transmit programmable burst length shift")]
#[doc(alias = "GMAC_CHAN_TX_CONTROL_PBL_SHIFT")]
pub(crate) const CHANNEL_TRANSMIT_CONTROL_PROGRAMMABLE_BURST_LENGTH_SHIFT: u32 = 16;
/// **Transmit programmable burst length mask** (`GMAC_CHAN_TX_CONTROL_PBL_MASK`, `dwqereg.h`).
#[doc(alias = "transmit programmable burst length mask")]
#[doc(alias = "GMAC_CHAN_TX_CONTROL_PBL_MASK")]
pub(crate) const CHANNEL_TRANSMIT_CONTROL_PROGRAMMABLE_BURST_LENGTH_MASK: u32 = 0x3f << 16;
/// **DMA channel 0 receive control register** (`GMAC_CHAN_RX_CONTROL(0)`, `dwqereg.h`).
#[doc(alias = "dma channel 0 receive control register")]
#[doc(alias = "GMAC_CHAN_RX_CONTROL")]
pub(crate) const CHANNEL_RECEIVE_CONTROL_REGISTER: u32 = 0x1108;
/// **Start receive** (`GMAC_CHAN_RX_CONTROL_SR`, `dwqereg.h`).
#[doc(alias = "start receive")]
#[doc(alias = "GMAC_CHAN_RX_CONTROL_SR")]
pub(crate) const CHANNEL_RECEIVE_CONTROL_START_RECEIVE: u32 = 1 << 0;
/// **Receive buffer size shift** (`EQOS_DMA_CH0_RX_CONTROL_RBSZ_SHIFT`, `dwc_eth_qos.h`): the
/// field holds bytes, bits 14:1 (Linux's `DMA_RBSZ_MASK`). OpenBSD never writes it and relies on
/// the reset value; this driver writes it because a field nobody wrote is a fact about whoever
/// ran last.
#[doc(alias = "receive buffer size shift")]
#[doc(alias = "EQOS_DMA_CH0_RX_CONTROL_RBSZ_SHIFT")]
pub(crate) const CHANNEL_RECEIVE_CONTROL_BUFFER_SIZE_SHIFT: u32 = 1;
/// **Receive buffer size mask** (`EQOS_DMA_CH0_RX_CONTROL_RBSZ_MASK`, `dwc_eth_qos.h`), applied
/// after the shift.
#[doc(alias = "receive buffer size mask")]
#[doc(alias = "EQOS_DMA_CH0_RX_CONTROL_RBSZ_MASK")]
pub(crate) const CHANNEL_RECEIVE_CONTROL_BUFFER_SIZE_MASK: u32 = 0x3fff;
/// **Receive programmable burst length shift** (`GMAC_CHAN_RX_CONTROL_RPBL_SHIFT`, `dwqereg.h`).
#[doc(alias = "receive programmable burst length shift")]
#[doc(alias = "GMAC_CHAN_RX_CONTROL_RPBL_SHIFT")]
pub(crate) const CHANNEL_RECEIVE_CONTROL_PROGRAMMABLE_BURST_LENGTH_SHIFT: u32 = 16;
/// **Receive programmable burst length mask** (`GMAC_CHAN_RX_CONTROL_RPBL_MASK`, `dwqereg.h`).
#[doc(alias = "receive programmable burst length mask")]
#[doc(alias = "GMAC_CHAN_RX_CONTROL_RPBL_MASK")]
pub(crate) const CHANNEL_RECEIVE_CONTROL_PROGRAMMABLE_BURST_LENGTH_MASK: u32 = 0x3f << 16;
/// **Transmit descriptor list base address, high register** (`GMAC_CHAN_TX_BASE_ADDR_HI(0)`,
/// `dwqereg.h`), the high 32 bits.
#[doc(alias = "transmit descriptor list base address, high register")]
#[doc(alias = "GMAC_CHAN_TX_BASE_ADDR_HI")]
pub(crate) const CHANNEL_TRANSMIT_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_HIGH_REGISTER: u32 = 0x1110;
/// **Transmit descriptor list base address register** (`GMAC_CHAN_TX_BASE_ADDR(0)`, `dwqereg.h`),
/// the low 32 bits.
#[doc(alias = "transmit descriptor list base address register")]
#[doc(alias = "GMAC_CHAN_TX_BASE_ADDR")]
pub(crate) const CHANNEL_TRANSMIT_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_LOW_REGISTER: u32 = 0x1114;
/// **Receive descriptor list base address, high register** (`GMAC_CHAN_RX_BASE_ADDR_HI(0)`,
/// `dwqereg.h`), the high 32 bits.
#[doc(alias = "receive descriptor list base address, high register")]
#[doc(alias = "GMAC_CHAN_RX_BASE_ADDR_HI")]
pub(crate) const CHANNEL_RECEIVE_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_HIGH_REGISTER: u32 = 0x1118;
/// **Receive descriptor list base address register** (`GMAC_CHAN_RX_BASE_ADDR(0)`, `dwqereg.h`),
/// the low 32 bits.
#[doc(alias = "receive descriptor list base address register")]
#[doc(alias = "GMAC_CHAN_RX_BASE_ADDR")]
pub(crate) const CHANNEL_RECEIVE_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_LOW_REGISTER: u32 = 0x111c;
/// **Transmit descriptor tail pointer** (`GMAC_CHAN_TX_END_ADDR(0)`, `dwqereg.h`): the address
/// one past the last descriptor handed to the device.
///
/// Name: ratified 2026-11-02 (UTC), #3094.
#[doc(alias = "transmit descriptor tail pointer")]
#[doc(alias = "CHAN_TX_END_ADDR")]
pub const CHANNEL_TRANSMIT_DESCRIPTOR_TAIL_POINTER_REGISTER: u32 = 0x1120;
/// **Receive descriptor tail pointer** (`GMAC_CHAN_RX_END_ADDR(0)`, `dwqereg.h`), the same for
/// the receive ring.
///
/// Name: ratified 2026-11-02 (UTC), #3094.
#[doc(alias = "receive descriptor tail pointer")]
#[doc(alias = "CHAN_RX_END_ADDR")]
pub const CHANNEL_RECEIVE_DESCRIPTOR_TAIL_POINTER_REGISTER: u32 = 0x1128;
/// **Transmit descriptor ring length** (`GMAC_CHAN_TX_RING_LEN(0)`, `dwqereg.h`): the descriptor
/// count minus one.
#[doc(alias = "transmit descriptor ring length")]
#[doc(alias = "GMAC_CHAN_TX_RING_LEN")]
pub(crate) const CHANNEL_TRANSMIT_DESCRIPTOR_RING_LENGTH_REGISTER: u32 = 0x112c;
/// **Receive descriptor ring length** (`GMAC_CHAN_RX_RING_LEN(0)`, `dwqereg.h`): the descriptor
/// count minus one.
#[doc(alias = "receive descriptor ring length")]
#[doc(alias = "GMAC_CHAN_RX_RING_LEN")]
pub(crate) const CHANNEL_RECEIVE_DESCRIPTOR_RING_LENGTH_REGISTER: u32 = 0x1130;
/// **DMA channel 0 interrupt enable register** (`GMAC_CHAN_INTR_ENA(0)`, `dwqereg.h`). This driver
/// polls and writes zero.
#[doc(alias = "dma channel 0 interrupt enable register")]
#[doc(alias = "GMAC_CHAN_INTR_ENA")]
pub(crate) const CHANNEL_INTERRUPT_ENABLE_REGISTER: u32 = 0x1134;
/// **Current transmit descriptor** (`GMAC_CHAN_CUR_TX_DESC(0)`, `dwqereg.h`). The descriptor the
/// transmit DMA is on, as the device sees it.
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_CHAN_CUR_TX_DESC")]
#[doc(alias = "CHAN_CUR_TX_DESC")]
pub const CHANNEL_CURRENT_TRANSMIT_DESCRIPTOR_REGISTER: u32 = 0x1144;
/// **Current receive descriptor** (`GMAC_CHAN_CUR_RX_DESC(0)`, `dwqereg.h`). The descriptor the
/// receive DMA is on, as the device sees it.
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_CHAN_CUR_RX_DESC")]
#[doc(alias = "CHAN_CUR_RX_DESC")]
pub const CHANNEL_CURRENT_RECEIVE_DESCRIPTOR_REGISTER: u32 = 0x114c;
/// **DMA channel 0 status register** (`GMAC_CHAN_STATUS(0)`, `dwqereg.h`). Write one to clear.
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_CHAN_STATUS")]
#[doc(alias = "CHAN_STATUS")]
pub const CHANNEL_STATUS_REGISTER: u32 = 0x1160;
/// **Transmit interrupt** (`GMAC_CHAN_STATUS_TI`, `dwqereg.h`): transmit complete.
#[doc(alias = "transmit interrupt")]
#[doc(alias = "GMAC_CHAN_STATUS_TI")]
pub(crate) const CHANNEL_STATUS_TRANSMIT_INTERRUPT: u32 = 1 << 0;
/// **Transmit process stopped** (`GMAC_CHAN_STATUS_TPS`, `dwqereg.h`).
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_CHAN_STATUS_TPS")]
#[doc(alias = "CHAN_STATUS_TPS")]
pub const CHANNEL_STATUS_TRANSMIT_PROCESS_STOPPED: u32 = 1 << 1;
/// **Transmit buffer unavailable** (`GMAC_CHAN_STATUS_TBU`, `dwqereg.h`): the DMA fetched a
/// descriptor it does not own and suspended.
#[doc(alias = "transmit buffer unavailable")]
#[doc(alias = "GMAC_CHAN_STATUS_TBU")]
pub(crate) const CHANNEL_STATUS_TRANSMIT_BUFFER_UNAVAILABLE: u32 = 1 << 2;
/// **Receive interrupt** (`GMAC_CHAN_STATUS_RI`, `dwqereg.h`): receive complete.
#[doc(alias = "receive interrupt")]
#[doc(alias = "GMAC_CHAN_STATUS_RI")]
pub(crate) const CHANNEL_STATUS_RECEIVE_INTERRUPT: u32 = 1 << 6;
/// **Receive buffer unavailable** (`GMAC_CHAN_STATUS_RBU`, `dwqereg.h`): the DMA fetched a
/// receive descriptor it does not own.
#[doc(alias = "receive buffer unavailable")]
#[doc(alias = "GMAC_CHAN_STATUS_RBU")]
pub(crate) const CHANNEL_STATUS_RECEIVE_BUFFER_UNAVAILABLE: u32 = 1 << 7;
/// **Receive process stopped** (`GMAC_CHAN_STATUS_RPS`, `dwqereg.h`).
///
/// Name: ratified 2026-11-03 (UTC), #3126.
#[doc(alias = "GMAC_CHAN_STATUS_RPS")]
#[doc(alias = "CHAN_STATUS_RPS")]
pub const CHANNEL_STATUS_RECEIVE_PROCESS_STOPPED: u32 = 1 << 8;
/// **Fatal bus error** (`GMAC_CHAN_STATUS_FBE`, `dwqereg.h`): the DMA's own access to memory
/// failed.
#[doc(alias = "fatal bus error")]
#[doc(alias = "GMAC_CHAN_STATUS_FBE")]
pub(crate) const CHANNEL_STATUS_FATAL_BUS_ERROR: u32 = 1 << 12;

/// The page of the window holding channel 0's registers, which is the page the data plane is
/// mapped. Every `CHAN_*` offset, `DIRECT_MEMORY_ACCESS_BUS_MODE_REGISTER` and `DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_REGISTER` is inside it.
///
/// Name: ratified 2026-11-02 (UTC), #3094.
pub const DIRECT_MEMORY_ACCESS_REGISTER_PAGE: u32 = 0x1000;
/// The register window's size in bytes, `reg`'s `0x10000` in both JH7110 trees.
///
/// Name: ratified 2026-11-03 (UTC), #3126.
pub const REGISTER_WINDOW_BYTES: u32 = 0x1_0000;

#[cfg(test)]
mod tests {
    use super::*;

    /// Every register the data plane writes or reads is on the DMA page, which is the one page
    /// the process is mapped; a register that moved off it would be a store into nothing.
    #[test]
    fn the_data_plane_registers_are_all_on_the_direct_memory_access_register_page() {
        for r in [
            CHANNEL_TRANSMIT_DESCRIPTOR_TAIL_POINTER_REGISTER,
            CHANNEL_RECEIVE_DESCRIPTOR_TAIL_POINTER_REGISTER,
            CHANNEL_STATUS_REGISTER,
            CHANNEL_CURRENT_RECEIVE_DESCRIPTOR_REGISTER,
            DIRECT_MEMORY_ACCESS_BUS_MODE_REGISTER,
        ] {
            assert_eq!(r & !0xfff, DIRECT_MEMORY_ACCESS_REGISTER_PAGE, "{r:#x}");
        }
    }

    /// The MAC's administrative registers are not on it: the process cannot reach the MDIO bus
    /// or the station address through the page it is given.
    #[test]
    fn the_administrative_registers_are_off_the_direct_memory_access_register_page() {
        for r in [
            MAC_MANAGEMENT_DATA_INPUT_OUTPUT_ADDRESS_REGISTER,
            MAC_MANAGEMENT_DATA_INPUT_OUTPUT_DATA_REGISTER,
            MAC_ADDRESS_0_HIGH_REGISTER,
            MAC_PACKET_FILTER,
            MAC_CONFIGURATION_REGISTER,
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
                MAC_CONFIGURATION_DISABLE_CARRIER_SENSE_DURING_TRANSMISSION,
                MAC_CONFIGURATION_LOOPBACK_MODE,
                MAC_CONFIGURATION_FULL_DUPLEX,
                MAC_CONFIGURATION_FAST_ETHERNET_SPEED,
                MAC_CONFIGURATION_PORT_SELECT,
                MAC_CONFIGURATION_JABBER_DISABLE,
                MAC_CONFIGURATION_PACKET_BURST_ENABLE,
            ],
            [0x200, 0x1000, 0x2000, 0x4000, 0x8000, 0x2_0000, 0x4_0000]
        );
        assert_eq!(
            [
                TRANSMIT_QUEUE_0_OPERATION_MODE_STORE_AND_FORWARD,
                TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_ENABLE_MASK,
                TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_ENABLED,
                TRANSMIT_QUEUE_0_OPERATION_MODE_THRESHOLD_CONTROL_MASK,
                TRANSMIT_QUEUE_0_OPERATION_MODE_THRESHOLD_CONTROL_512_BYTES,
                TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_SIZE_MASK,
            ],
            [0x2, 0xc, 0x8, 0x70, 0x70, 0x01ff_0000]
        );
        assert_eq!(
            [
                RECEIVE_QUEUE_0_OPERATION_MODE_THRESHOLD_CONTROL_MASK,
                RECEIVE_QUEUE_0_OPERATION_MODE_THRESHOLD_CONTROL_128_BYTES,
                RECEIVE_QUEUE_0_OPERATION_MODE_STORE_AND_FORWARD,
                RECEIVE_QUEUE_0_OPERATION_MODE_QUEUE_SIZE_MASK,
            ],
            [0x18, 0x18, 0x20, 0x3ff0_0000]
        );
        assert_eq!(
            [
                DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_32_PERMITTED,
                DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_64_PERMITTED,
                DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_128_PERMITTED,
                DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_256_PERMITTED,
                DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_ENHANCED_ADDRESS_MODE_ENABLE,
            ],
            [0x10, 0x20, 0x40, 0x80, 0x800]
        );
        assert_eq!(
            [
                CHANNEL_CONTROL_EIGHT_TIMES_PROGRAMMABLE_BURST_LENGTH,
                CHANNEL_TRANSMIT_CONTROL_OPERATE_ON_SECOND_PACKET
            ],
            [0x1_0000, 0x10]
        );
        assert_eq!(
            [
                CHANNEL_STATUS_TRANSMIT_PROCESS_STOPPED,
                CHANNEL_STATUS_TRANSMIT_BUFFER_UNAVAILABLE,
                CHANNEL_STATUS_RECEIVE_INTERRUPT,
                CHANNEL_STATUS_RECEIVE_BUFFER_UNAVAILABLE,
                CHANNEL_STATUS_RECEIVE_PROCESS_STOPPED,
                CHANNEL_STATUS_FATAL_BUS_ERROR,
            ],
            [0x2, 0x4, 0x40, 0x80, 0x100, 0x1000]
        );
    }

    /// The four hardware feature registers are consecutive words from `0x11c`, and an index past
    /// them wraps rather than reading some other register.
    #[test]
    fn the_feature_registers_are_four_words_from_0x11c() {
        assert_eq!(
            [0, 1, 2, 3, 4, 5].map(mac_hardware_feature_register),
            [0x11c, 0x120, 0x124, 0x128, 0x11c, 0x120]
        );
    }
}
