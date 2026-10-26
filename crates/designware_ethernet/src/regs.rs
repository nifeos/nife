//! **The controller's register map**: offsets from the 64 KiB window's base, and the bits this
//! driver writes or reads.
//!
//! Transcribed from OpenBSD's `sys/dev/ic/dwqereg.h` (revision of 2026-07-31, commit
//! `59ec3ab4d107`, ISC license, notice in the crate root), with [`MAC_CONF_LM`] from Linux's
//! `stmmac/dwmac4.h` and [`CHAN_RX_CONTROL_RBSZ_SHIFT`] from U-Boot's `dwc_eth_qos.h`. The MAC and
//! MTL are page `0x0000`; page `0x1000` holds the DMA channel's tail pointers beside its ring bases
//! and the controller's software reset, and there is no IOMMU, so a process given that page can
//! repoint a ring and reset the MAC.

/// MAC configuration.
pub(crate) const MAC_CONF: u32 = 0x0000;
/// Receiver enable.
pub(crate) const MAC_CONF_RE: u32 = 1 << 0;
/// Transmitter enable.
pub(crate) const MAC_CONF_TE: u32 = 1 << 1;
/// Disable carrier sense during transmission.
pub const MAC_CONF_DCRS: u32 = 1 << 9;
/// **Loopback mode**: the MAC returns what it transmits to its own receive path (Linux
/// `GMAC_CONFIG_LM`, `dwmac4.h`). The coherence probe uses it so the measurement does not depend
/// on a cable or a link partner.
pub const MAC_CONF_LM: u32 = 1 << 12;
/// Full duplex.
pub const MAC_CONF_DM: u32 = 1 << 13;
/// Speed: with [`MAC_CONF_PS`], 100 Mbit/s when set and 10 when clear.
pub const MAC_CONF_FES: u32 = 1 << 14;
/// Port select: set for 10 or 100 Mbit/s (MII), clear for 1000 (GMII/RGMII).
pub const MAC_CONF_PS: u32 = 1 << 15;
/// Jabber disable.
pub const MAC_CONF_JD: u32 = 1 << 17;
/// Packet burst enable.
pub const MAC_CONF_BE: u32 = 1 << 18;

/// The receive packet filter. Zero after reset: the station address and broadcast pass,
/// multicast does not.
pub(crate) const MAC_PACKET_FILTER: u32 = 0x0008;
/// Transmit flow control for queue 0.
pub(crate) const MAC_Q0_TX_FLOW_CTRL: u32 = 0x0070;
/// Pause time field shift in [`MAC_Q0_TX_FLOW_CTRL`].
pub(crate) const MAC_Q0_TX_FLOW_CTRL_PT_SHIFT: u32 = 16;
/// Transmit flow control enable.
pub(crate) const MAC_Q0_TX_FLOW_CTRL_TFE: u32 = 1 << 0;
/// Receive flow control.
pub(crate) const MAC_RX_FLOW_CTRL: u32 = 0x0090;
/// Receive flow control enable.
pub(crate) const MAC_RX_FLOW_CTRL_RFE: u32 = 1 << 0;
/// Receive queue control 0: which receive queues are enabled.
pub(crate) const MAC_RXQ_CTRL0: u32 = 0x00a0;
/// Receive queue 0 enabled for generic (DCB) traffic. **Without this the MAC receives nothing**
/// on the 4.x and 5.x controllers, whatever the DMA is doing.
pub(crate) const MAC_RXQ_CTRL0_Q0_DCB: u32 = 2;
/// The MAC interrupt enable register.
pub(crate) const MAC_INT_EN: u32 = 0x00b4;
/// Synopsys version register; the low byte is the core version (`0x52` for 5.20).
pub const MAC_VERSION: u32 = 0x0110;
/// Hardware feature register `i`, for `i` in `0..4`.
pub const fn mac_hw_feature(i: u32) -> u32 {
    0x011c + 4 * (i % 4)
}
/// MDIO address and command.
pub const MAC_MDIO_ADDR: u32 = 0x0200;
/// MDIO data.
pub const MAC_MDIO_DATA: u32 = 0x0204;
/// Station address 0, high half (bytes 4 and 5).
pub const MAC_ADDR0_HI: u32 = 0x0300;
/// Station address 0, low half (bytes 0 to 3).
pub const MAC_ADDR0_LO: u32 = 0x0304;
/// The MMC receive counters' interrupt mask: all ones masks every counter interrupt.
pub(crate) const MMC_RX_INT_MASK: u32 = 0x070c;
/// The MMC transmit counters' interrupt mask.
pub(crate) const MMC_TX_INT_MASK: u32 = 0x0710;

/// MTL transmit queue 0 operation mode.
pub(crate) const MTL_TXQ0_OP_MODE: u32 = 0x0d00;
/// Flush the transmit queue; self-clearing.
pub(crate) const MTL_TXQ_OP_MODE_FTQ: u32 = 1 << 0;
/// Transmit store and forward.
pub(crate) const MTL_TXQ_OP_MODE_TSF: u32 = 1 << 1;
/// Transmit queue enable field.
pub(crate) const MTL_TXQ_OP_MODE_TXQEN_MASK: u32 = 0x3 << 2;
/// Transmit queue enabled.
pub(crate) const MTL_TXQ_OP_MODE_TXQEN: u32 = 2 << 2;
/// Transmit threshold control field.
pub(crate) const MTL_TXQ_OP_MODE_TTC_MASK: u32 = 0x7 << 4;
/// Transmit threshold of 512 bytes, what OpenBSD picks in threshold mode.
pub(crate) const MTL_TXQ_OP_MODE_TTC_512: u32 = 7 << 4;
/// Transmit queue size field.
pub(crate) const MTL_TXQ_OP_MODE_TQS_MASK: u32 = 0x1ff << 16;
/// Transmit queue size shift: the field holds `bytes / 256 - 1`.
pub(crate) const MTL_TXQ_OP_MODE_TQS_SHIFT: u32 = 16;
/// MTL receive queue 0 operation mode.
pub(crate) const MTL_RXQ0_OP_MODE: u32 = 0x0d30;
/// Receive threshold control field.
pub(crate) const MTL_RXQ_OP_MODE_RTC_MASK: u32 = 0x3 << 3;
/// Receive threshold of 128 bytes.
pub(crate) const MTL_RXQ_OP_MODE_RTC_128: u32 = 3 << 3;
/// Receive store and forward.
pub(crate) const MTL_RXQ_OP_MODE_RSF: u32 = 1 << 5;
/// Receive queue size field.
pub(crate) const MTL_RXQ_OP_MODE_RQS_MASK: u32 = 0x3ff << 20;
/// Receive queue size shift: the field holds `bytes / 256 - 1`.
pub(crate) const MTL_RXQ_OP_MODE_RQS_SHIFT: u32 = 20;

/// The DMA bus mode.
pub(crate) const DMA_MODE: u32 = 0x1000;
/// **Software reset** of the whole controller (MAC, MTL and DMA); self-clearing. The bit that
/// makes the DMA page an administrative page as well as a data one.
pub(crate) const DMA_MODE_SWR: u32 = 1 << 0;
/// The DMA system bus (AXI) mode.
pub(crate) const DMA_SYSBUS_MODE: u32 = 0x1004;
/// Fixed burst.
pub(crate) const DMA_SYSBUS_MODE_FB: u32 = 1 << 0;
/// AXI burst length 32 permitted.
pub(crate) const DMA_SYSBUS_MODE_BLEN_32: u32 = 1 << 4;
/// AXI burst length 64 permitted.
pub(crate) const DMA_SYSBUS_MODE_BLEN_64: u32 = 1 << 5;
/// AXI burst length 128 permitted.
pub(crate) const DMA_SYSBUS_MODE_BLEN_128: u32 = 1 << 6;
/// AXI burst length 256 permitted.
pub(crate) const DMA_SYSBUS_MODE_BLEN_256: u32 = 1 << 7;
/// Enhanced address mode: descriptor addresses wider than 32 bits.
pub(crate) const DMA_SYSBUS_MODE_EAME: u32 = 1 << 11;
/// Outstanding read requests limit field.
pub(crate) const DMA_SYSBUS_MODE_RD_OSR_LMT_SHIFT: u32 = 16;
/// Outstanding write requests limit field.
pub(crate) const DMA_SYSBUS_MODE_WR_OSR_LMT_SHIFT: u32 = 24;
/// Both limit fields' width.
pub(crate) const DMA_SYSBUS_MODE_OSR_LMT_MASK: u32 = 0xf;

/// DMA channel 0 control.
pub(crate) const CHAN_CONTROL: u32 = 0x1100;
/// Multiply both programmable burst lengths by eight. Both JH7110 trees say `snps,no-pbl-x8`.
pub(crate) const CHAN_CONTROL_PBLX8: u32 = 1 << 16;
/// Channel 0 transmit control.
pub(crate) const CHAN_TX_CONTROL: u32 = 0x1104;
/// Start transmit DMA.
pub(crate) const CHAN_TX_CONTROL_ST: u32 = 1 << 0;
/// Operate on a second frame: fetch the next descriptor before the status of this one is written.
pub(crate) const CHAN_TX_CONTROL_OSP: u32 = 1 << 4;
/// Transmit programmable burst length shift.
pub(crate) const CHAN_TX_CONTROL_PBL_SHIFT: u32 = 16;
/// Both programmable burst length fields' mask, after the shift.
pub(crate) const CHAN_PBL_MASK: u32 = 0x3f;
/// Channel 0 receive control.
pub(crate) const CHAN_RX_CONTROL: u32 = 0x1108;
/// Start receive DMA.
pub(crate) const CHAN_RX_CONTROL_SR: u32 = 1 << 0;
/// The receive buffer size field, in bytes, bits 14:1 (U-Boot's `EQOS_DMA_CH0_RX_CONTROL_RBSZ_*`,
/// Linux's `DMA_RBSZ_MASK`). OpenBSD never writes it and relies on the reset value; this driver
/// writes it because a field nobody wrote is a fact about whoever ran last.
pub(crate) const CHAN_RX_CONTROL_RBSZ_SHIFT: u32 = 1;
/// The receive buffer size field's mask, after the shift.
pub(crate) const CHAN_RX_CONTROL_RBSZ_MASK: u32 = 0x3fff;
/// Receive programmable burst length shift.
pub(crate) const CHAN_RX_CONTROL_PBL_SHIFT: u32 = 16;
/// Transmit descriptor ring base, high 32 bits.
pub(crate) const CHAN_TX_BASE_HI: u32 = 0x1110;
/// Transmit descriptor ring base, low 32 bits.
pub(crate) const CHAN_TX_BASE: u32 = 0x1114;
/// Receive descriptor ring base, high 32 bits.
pub(crate) const CHAN_RX_BASE_HI: u32 = 0x1118;
/// Receive descriptor ring base, low 32 bits.
pub(crate) const CHAN_RX_BASE: u32 = 0x111c;
/// **Transmit tail pointer**: the address one past the last descriptor handed to the device.
pub const CHAN_TX_TAIL: u32 = 0x1120;
/// **Receive tail pointer**, the same for the receive ring.
pub const CHAN_RX_TAIL: u32 = 0x1128;
/// Transmit ring length: the descriptor count minus one.
pub(crate) const CHAN_TX_RING_LEN: u32 = 0x112c;
/// Receive ring length: the descriptor count minus one.
pub(crate) const CHAN_RX_RING_LEN: u32 = 0x1130;
/// Channel 0 interrupt enable. This driver polls and writes zero.
pub(crate) const CHAN_INTR_ENA: u32 = 0x1134;
/// The descriptor the transmit DMA is on, as the device sees it.
pub const CHAN_CUR_TX_DESC: u32 = 0x1144;
/// The descriptor the receive DMA is on, as the device sees it.
pub const CHAN_CUR_RX_DESC: u32 = 0x114c;
/// Channel 0 status. Write one to clear.
pub const CHAN_STATUS: u32 = 0x1160;
/// Transmit complete.
pub(crate) const CHAN_STATUS_TI: u32 = 1 << 0;
/// Transmit process stopped.
pub const CHAN_STATUS_TPS: u32 = 1 << 1;
/// **Transmit buffer unavailable**: the DMA fetched a descriptor it does not own and suspended.
pub(crate) const CHAN_STATUS_TBU: u32 = 1 << 2;
/// Receive complete.
pub(crate) const CHAN_STATUS_RI: u32 = 1 << 6;
/// **Receive buffer unavailable**: the DMA fetched a receive descriptor it does not own.
pub(crate) const CHAN_STATUS_RBU: u32 = 1 << 7;
/// Receive process stopped.
pub const CHAN_STATUS_RPS: u32 = 1 << 8;
/// Fatal bus error: the DMA's own access to memory failed.
pub(crate) const CHAN_STATUS_FBE: u32 = 1 << 12;

/// The page of the window holding channel 0's registers, which is the page the data plane is
/// mapped. Every `CHAN_*` and `DMA_*` offset above is inside it.
pub const DMA_PAGE: u32 = 0x1000;
/// The window's size, `reg`'s `0x10000` in both JH7110 trees.
pub const WINDOW: u32 = 0x1_0000;

#[cfg(test)]
mod tests {
    use super::*;

    /// Every register the data plane writes or reads is on the DMA page, which is the one page
    /// the process is mapped; a register that moved off it would be a store into nothing.
    #[test]
    fn the_data_plane_registers_are_all_on_the_dma_page() {
        for r in [
            CHAN_TX_TAIL,
            CHAN_RX_TAIL,
            CHAN_STATUS,
            CHAN_CUR_RX_DESC,
            DMA_MODE,
        ] {
            assert_eq!(r & !0xfff, DMA_PAGE, "{r:#x}");
        }
    }

    /// The MAC's administrative registers are not on it: the process cannot reach the MDIO bus
    /// or the station address through the page it is given.
    #[test]
    fn the_mdio_bus_and_the_station_address_are_not_on_the_dma_page() {
        for r in [
            MAC_MDIO_ADDR,
            MAC_MDIO_DATA,
            MAC_ADDR0_HI,
            MAC_PACKET_FILTER,
            MAC_CONF,
        ] {
            assert_ne!(r & !0xfff, DMA_PAGE, "{r:#x}");
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
                MAC_CONF_LM,
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
                MTL_TXQ_OP_MODE_TSF,
                MTL_TXQ_OP_MODE_TXQEN_MASK,
                MTL_TXQ_OP_MODE_TXQEN,
                MTL_TXQ_OP_MODE_TTC_MASK,
                MTL_TXQ_OP_MODE_TTC_512,
                MTL_TXQ_OP_MODE_TQS_MASK,
            ],
            [0x2, 0xc, 0x8, 0x70, 0x70, 0x01ff_0000]
        );
        assert_eq!(
            [
                MTL_RXQ_OP_MODE_RTC_MASK,
                MTL_RXQ_OP_MODE_RTC_128,
                MTL_RXQ_OP_MODE_RSF,
                MTL_RXQ_OP_MODE_RQS_MASK,
            ],
            [0x18, 0x18, 0x20, 0x3ff0_0000]
        );
        assert_eq!(
            [
                DMA_SYSBUS_MODE_BLEN_32,
                DMA_SYSBUS_MODE_BLEN_64,
                DMA_SYSBUS_MODE_BLEN_128,
                DMA_SYSBUS_MODE_BLEN_256,
                DMA_SYSBUS_MODE_EAME,
            ],
            [0x10, 0x20, 0x40, 0x80, 0x800]
        );
        assert_eq!([CHAN_CONTROL_PBLX8, CHAN_TX_CONTROL_OSP], [0x1_0000, 0x10]);
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
