//! **The controller's sequences**: identify, reset, configure, start, stop.
//!
//! OpenBSD's `dwqe_attach`, `dwqe_reset`, `dwqe_up`, `dwqe_down` and `dwqe_mii_statchg`
//! (`sys/dev/ic/dwqe.c`, ISC license, notice in the crate root), in their order, with three changes
//! each argued at the place it is made: the receive buffer size is written rather than inherited,
//! jumbo frames are not enabled, and nothing is allocated (the DMA region is the caller's, laid out
//! by [`crate::layout`]). Every sequence runs against [`Hardware`], so the host tests run them
//! against `sim`'s controller.

use crate::motorcomm::{Link, Speed};
use crate::{BUFFER_SIZE, Hardware, RECEIVE_ENTRIES, TRANSMIT_ENTRIES, layout, regs};

/// Why the controller was not brought up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The software reset did not self-clear in [`RESET_LIMIT_US`]. On this controller the usual
    /// cause is a missing receive clock from the PHY, which the reset needs to complete.
    ResetTimeout,
    /// The DMA region is not somewhere this controller can address: past its address width
    /// ([`Identity::dma_bits`]), or straddling a 4 GiB boundary, which the 32-bit tail pointer
    /// registers cannot express.
    RegionOutOfReach {
        /// The controller's address width.
        dma_bits: u8,
    },
    /// The transmit queue did not finish flushing on [`stop`].
    FlushTimeout,
}

/// How long the software reset may take: OpenBSD's 30000 polls 10 µs apart.
pub(crate) const RESET_LIMIT_US: u64 = 300_000;

/// What the controller says it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    /// [`regs::MAC_VERSION`].
    pub version: u32,
    /// The four hardware feature registers.
    pub features: [u32; 4],
}

impl Identity {
    /// Read the version and feature registers.
    pub fn read(hardware: &mut impl Hardware) -> Identity {
        let mut features = [0; 4];
        for (i, f) in features.iter_mut().enumerate() {
            *f = hardware.read(regs::mac_hw_feature(i as u32));
        }
        Identity {
            version: hardware.read(regs::MAC_VERSION),
            features,
        }
    }

    /// The Synopsys core version, `0x52` for the 5.20 both trees name.
    pub const fn core_version(&self) -> u8 {
        self.version as u8
    }

    /// The DMA's address width from feature register 1's `ADDR64` field: 32, 40 or 48 bits.
    pub const fn dma_bits(&self) -> u8 {
        match (self.features[1] >> 14) & 0x3 {
            1 => 40,
            2 => 48,
            _ => 32,
        }
    }

    /// The transmit queue's size in bytes (the MTL's transmit FIFO), `128 << TXFIFOSIZE`.
    #[doc(alias = "fifo")]
    pub(crate) const fn transmit_queue_bytes(&self) -> u32 {
        queue_bytes((self.features[1] >> 6) & 0x1f)
    }

    /// The receive queue's size in bytes (the MTL's receive FIFO), `128 << RXFIFOSIZE`.
    #[doc(alias = "fifo")]
    pub(crate) const fn receive_queue_bytes(&self) -> u32 {
        queue_bytes(self.features[1] & 0x1f)
    }
}

/// `128 << field`, with a field past the largest queue (FIFO) the databook encodes (256 KiB, field
/// 11) saturating there rather than shifting a bit off the top of the word.
#[doc(alias = "fifo")]
const fn queue_bytes(field: u32) -> u32 {
    let f = if field > 11 { 11 } else { field };
    128 << f
}

/// **Reset the whole controller** and wait for the reset to finish. Returns how long it took.
pub fn soft_reset(hardware: &mut impl Hardware) -> Result<u64, Error> {
    let mode = hardware.read(regs::DMA_MODE);
    hardware.write(regs::DMA_MODE, mode | regs::DMA_MODE_SWR);
    let mut waited = 0;
    loop {
        if hardware.read(regs::DMA_MODE) & regs::DMA_MODE_SWR == 0 {
            return Ok(waited);
        }
        if waited >= RESET_LIMIT_US {
            return Err(Error::ResetTimeout);
        }
        hardware.delay_us(10);
        waited += 10;
    }
}

/// **The station address in the two address registers**, OpenBSD's `dwqe_lladdr_read` byte order,
/// or `None` when they hold nothing a station could own: all zero or all ones (the reset value),
/// or a multicast address.
pub fn station_address(hi: u32, lo: u32) -> Option<[u8; 6]> {
    let [a, b, c, d] = lo.to_le_bytes();
    let [e, f, _, _] = hi.to_le_bytes();
    let station_address = [a, b, c, d, e, f];
    if station_address == [0; 6] || station_address == [0xff; 6] || a & 1 != 0 {
        return None;
    }
    Some(station_address)
}

/// The two address register words for `station_address`: `(high, low)`, OpenBSD's
/// `dwqe_lladdr_write`.
pub(crate) const fn address_words(station_address: [u8; 6]) -> (u32, u32) {
    (
        u32::from_le_bytes([station_address[4], station_address[5], 0, 0]),
        u32::from_le_bytes([
            station_address[0],
            station_address[1],
            station_address[2],
            station_address[3],
        ]),
    )
}

/// **How the DMA meets the bus**: the `snps,*` properties of a board's node and its
/// `snps,axi-config`. [`crate::jh7110::BUS`] is radon's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bus {
    /// `snps,fixed-burst`.
    pub fixed_burst: bool,
    /// Multiply both channel burst lengths by eight (the databook's programmable burst length
    /// times 8, `PBLx8`): not `snps,no-pbl-x8`.
    #[doc(alias = "pbl")]
    pub channel_burst_length_times_8: bool,
    /// The transmit channel's programmable burst length (PBL), in beats: `snps,txpbl`.
    #[doc(alias = "pbl")]
    pub transmit_channel_burst_length: u32,
    /// The receive channel's programmable burst length (PBL), in beats: `snps,rxpbl`.
    #[doc(alias = "pbl")]
    pub receive_channel_burst_length: u32,
    /// `snps,force_thresh_dma_mode`: threshold mode rather than store and forward.
    pub threshold_mode: bool,
    /// The system bus's permitted burst lengths: the [`regs::DMA_SYSBUS_MODE`] `BLEN` bits
    /// `snps,blen` names, a different setting from the channels' burst lengths.
    pub bus_burst_lengths: u32,
    /// `snps,rd_osr_lmt`.
    pub read_outstanding: u32,
    /// `snps,wr_osr_lmt`.
    pub write_outstanding: u32,
}

/// The receive operation mode word for a queue of `queue_bytes` bytes, OpenBSD's arithmetic: the
/// queue size in 256-byte units less one, threshold or store-and-forward, and hardware flow control
/// with its thresholds once the queue is large enough for them to mean anything.
pub(crate) fn receive_queue_mode(current: u32, bus: &Bus, queue_bytes: u32) -> u32 {
    let mut mode = current;
    if bus.threshold_mode {
        mode &= !(regs::MTL_RXQ_OP_MODE_RSF | regs::MTL_RXQ_OP_MODE_RTC_MASK);
        mode |= regs::MTL_RXQ_OP_MODE_RTC_128;
    } else {
        mode |= regs::MTL_RXQ_OP_MODE_RSF;
    }
    let rqs = (queue_bytes / 256).saturating_sub(1);
    mode &= !regs::MTL_RXQ_OP_MODE_RQS_MASK;
    mode |= (rqs << regs::MTL_RXQ_OP_MODE_RQS_SHIFT) & regs::MTL_RXQ_OP_MODE_RQS_MASK;
    if queue_bytes >= 4096 {
        // EHFC, then RFD = 3 and RFA = 1: OpenBSD's flow control thresholds.
        mode |= 1 << 7;
        mode &= !((0x3f << 14) | (0x3f << 8));
        mode |= (0x3 << 14) | (0x1 << 8);
    }
    mode
}

/// The transmit operation mode word for a queue of `queue_bytes` bytes, OpenBSD's arithmetic.
pub(crate) fn transmit_queue_mode(current: u32, bus: &Bus, queue_bytes: u32) -> u32 {
    let mut mode = current;
    if bus.threshold_mode {
        mode &= !(regs::MTL_TXQ_OP_MODE_TSF | regs::MTL_TXQ_OP_MODE_TTC_MASK);
        mode |= regs::MTL_TXQ_OP_MODE_TTC_512;
    } else {
        mode |= regs::MTL_TXQ_OP_MODE_TSF;
    }
    mode &= !regs::MTL_TXQ_OP_MODE_TXQEN_MASK;
    mode |= regs::MTL_TXQ_OP_MODE_TXQEN;
    let tqs = (queue_bytes / 256).saturating_sub(1);
    mode &= !regs::MTL_TXQ_OP_MODE_TQS_MASK;
    mode |= (tqs << regs::MTL_TXQ_OP_MODE_TQS_SHIFT) & regs::MTL_TXQ_OP_MODE_TQS_MASK;
    mode
}

/// Can a controller of `dma_bits` address a region of `bytes` at `phys`, with tail pointers
/// that only carry the low 32 bits?
pub(crate) const fn region_in_reach(phys: u64, bytes: u64, dma_bits: u8) -> bool {
    let Some(end) = phys.checked_add(bytes) else {
        return false;
    };
    let last = end - 1;
    last >> dma_bits == 0 && phys >> 32 == last >> 32
}

/// **Configure a controller just reset**, with nothing started: the DMA's bus mode, channel 0's
/// burst lengths, buffer size, both rings at `region` (heads and tails at the ring bases, so
/// nothing is available until the data plane posts), both queues' modes, flow control, the
/// receive queue enable, the station address and filter, and every interrupt masked.
///
/// The FIFO sizes are the controller's own ([`Identity`]), not the tree's: radon's vendor tree
/// says `rx-fifo-depth = <262144>` and mainline says `<2048>` for the same silicon, and a queue
/// size computed from the first would overflow the field into the next. The hardware's statement
/// is the one fact about the FIFO both trees are describing.
pub fn configure(
    hardware: &mut impl Hardware,
    id: &Identity,
    bus: &Bus,
    region: u64,
    station_address: [u8; 6],
) -> Result<(), Error> {
    if !region_in_reach(region, layout::BYTES, id.dma_bits()) {
        return Err(Error::RegionOutOfReach {
            dma_bits: id.dma_bits(),
        });
    }

    let mut mode = hardware.read(regs::DMA_SYSBUS_MODE);
    if bus.fixed_burst {
        mode |= regs::DMA_SYSBUS_MODE_FB;
    }
    if id.dma_bits() > 32 {
        mode |= regs::DMA_SYSBUS_MODE_EAME;
    }
    let osr = regs::DMA_SYSBUS_MODE_OSR_LMT_MASK;
    mode &= !(osr << regs::DMA_SYSBUS_MODE_RD_OSR_LMT_SHIFT
        | osr << regs::DMA_SYSBUS_MODE_WR_OSR_LMT_SHIFT);
    mode |= (bus.read_outstanding & osr) << regs::DMA_SYSBUS_MODE_RD_OSR_LMT_SHIFT
        | (bus.write_outstanding & osr) << regs::DMA_SYSBUS_MODE_WR_OSR_LMT_SHIFT
        | bus.bus_burst_lengths;
    hardware.write(regs::DMA_SYSBUS_MODE, mode);

    let control = hardware.read(regs::CHAN_CONTROL);
    hardware.write(
        regs::CHAN_CONTROL,
        if bus.channel_burst_length_times_8 {
            control | regs::CHAN_CONTROL_PBLX8
        } else {
            control & !regs::CHAN_CONTROL_PBLX8
        },
    );
    let pbl = regs::CHAN_PBL_MASK;
    let transmit = hardware.read(regs::CHAN_TX_CONTROL) & !(pbl << regs::CHAN_TX_CONTROL_PBL_SHIFT);
    hardware.write(
        regs::CHAN_TX_CONTROL,
        transmit
            | (bus.transmit_channel_burst_length & pbl) << regs::CHAN_TX_CONTROL_PBL_SHIFT
            | regs::CHAN_TX_CONTROL_OSP,
    );
    // The buffer size is written, not inherited (OpenBSD inherits it): a size left by firmware is
    // a fact about whoever ran last, and the device writes a frame up to it.
    let rbsz = regs::CHAN_RX_CONTROL_RBSZ_MASK << regs::CHAN_RX_CONTROL_RBSZ_SHIFT;
    let receive =
        hardware.read(regs::CHAN_RX_CONTROL) & !(pbl << regs::CHAN_RX_CONTROL_PBL_SHIFT | rbsz);
    hardware.write(
        regs::CHAN_RX_CONTROL,
        receive
            | (bus.receive_channel_burst_length & pbl) << regs::CHAN_RX_CONTROL_PBL_SHIFT
            | (BUFFER_SIZE as u32) << regs::CHAN_RX_CONTROL_RBSZ_SHIFT,
    );

    let transmit_ring = region + layout::TRANSMIT_RING;
    let receive_ring = region + layout::RECEIVE_RING;
    hardware.write(regs::CHAN_TX_BASE_HI, (transmit_ring >> 32) as u32);
    hardware.write(regs::CHAN_TX_BASE, transmit_ring as u32);
    hardware.write(regs::CHAN_TX_RING_LEN, u32::from(TRANSMIT_ENTRIES) - 1);
    hardware.write(regs::CHAN_TX_TAIL, transmit_ring as u32);
    hardware.write(regs::CHAN_RX_BASE_HI, (receive_ring >> 32) as u32);
    hardware.write(regs::CHAN_RX_BASE, receive_ring as u32);
    hardware.write(regs::CHAN_RX_RING_LEN, u32::from(RECEIVE_ENTRIES) - 1);
    hardware.write(regs::CHAN_RX_TAIL, receive_ring as u32);

    let receive_mode = receive_queue_mode(
        hardware.read(regs::MTL_RXQ0_OP_MODE),
        bus,
        id.receive_queue_bytes(),
    );
    hardware.write(regs::MTL_RXQ0_OP_MODE, receive_mode);
    let transmit_mode = transmit_queue_mode(
        hardware.read(regs::MTL_TXQ0_OP_MODE),
        bus,
        id.transmit_queue_bytes(),
    );
    hardware.write(regs::MTL_TXQ0_OP_MODE, transmit_mode);

    let flow = hardware.read(regs::MAC_Q0_TX_FLOW_CTRL);
    hardware.write(
        regs::MAC_Q0_TX_FLOW_CTRL,
        flow | 0xffff << regs::MAC_Q0_TX_FLOW_CTRL_PT_SHIFT | regs::MAC_Q0_TX_FLOW_CTRL_TFE,
    );
    let flow = hardware.read(regs::MAC_RX_FLOW_CTRL);
    hardware.write(regs::MAC_RX_FLOW_CTRL, flow | regs::MAC_RX_FLOW_CTRL_RFE);
    hardware.write(regs::MAC_RXQ_CTRL0, regs::MAC_RXQ_CTRL0_Q0_DCB);

    let (hi, lo) = address_words(station_address);
    hardware.write(regs::MAC_ADDR0_HI, hi);
    hardware.write(regs::MAC_ADDR0_LO, lo);
    // Own unicast address and broadcast; no multicast, no promiscuous receive.
    hardware.write(regs::MAC_PACKET_FILTER, 0);

    hardware.write(regs::MAC_INT_EN, 0);
    hardware.write(regs::CHAN_INTR_ENA, 0);
    hardware.write(regs::MMC_RX_INT_MASK, u32::MAX);
    hardware.write(regs::MMC_TX_INT_MASK, u32::MAX);
    hardware.write(regs::CHAN_STATUS, u32::MAX);
    Ok(())
}

/// **The MAC configuration word for a link**, OpenBSD's `dwqe_mii_statchg` and `dwqe_up`: the
/// port and speed select for the negotiated speed, duplex, carrier sense off, jabber off, burst
/// on, transmitter and receiver on. `loopback` turns the MAC's own loopback on for the coherence
/// probe.
///
/// OpenBSD also sets `JE` (jumbo frames). This does not: a jumbo frame cannot fit a 2048-byte
/// buffer, would span descriptors, and would be dropped by [`crate::receive_verdict`] anyway, so
/// accepting it into the FIFO only spends ring slots on frames that are thrown away.
pub const fn mac_conf(current: u32, link: Link, loopback: bool) -> u32 {
    let mut conf =
        current & !(regs::MAC_CONF_PS | regs::MAC_CONF_FES | regs::MAC_CONF_DM | regs::MAC_CONF_LM);
    conf |= match link.speed {
        Speed::Thousand => 0,
        Speed::Hundred => regs::MAC_CONF_PS | regs::MAC_CONF_FES,
        Speed::Ten => regs::MAC_CONF_PS,
    };
    if link.full_duplex {
        conf |= regs::MAC_CONF_DM;
    }
    if loopback {
        conf |= regs::MAC_CONF_LM;
    }
    conf | regs::MAC_CONF_BE
        | regs::MAC_CONF_JD
        | regs::MAC_CONF_DCRS
        | regs::MAC_CONF_TE
        | regs::MAC_CONF_RE
}

/// **Start both DMA channels and the MAC** at `link`, OpenBSD's order: receive DMA, transmit DMA,
/// then the MAC. Nothing moves until the data plane hands the device descriptors.
pub fn start(hardware: &mut impl Hardware, link: Link, loopback: bool) {
    let receive = hardware.read(regs::CHAN_RX_CONTROL);
    hardware.write(regs::CHAN_RX_CONTROL, receive | regs::CHAN_RX_CONTROL_SR);
    let transmit = hardware.read(regs::CHAN_TX_CONTROL);
    hardware.write(regs::CHAN_TX_CONTROL, transmit | regs::CHAN_TX_CONTROL_ST);
    let conf = hardware.read(regs::MAC_CONF);
    hardware.write(regs::MAC_CONF, mac_conf(conf, link, loopback));
}

/// **Stop the MAC and both DMA channels**, OpenBSD's `dwqe_down` order: receiver off, receive DMA
/// off, transmit DMA off, flush the transmit queue, transmitter off.
pub fn stop(hardware: &mut impl Hardware) -> Result<(), Error> {
    let conf = hardware.read(regs::MAC_CONF);
    hardware.write(regs::MAC_CONF, conf & !regs::MAC_CONF_RE);
    let receive = hardware.read(regs::CHAN_RX_CONTROL);
    hardware.write(regs::CHAN_RX_CONTROL, receive & !regs::CHAN_RX_CONTROL_SR);
    let transmit = hardware.read(regs::CHAN_TX_CONTROL);
    hardware.write(regs::CHAN_TX_CONTROL, transmit & !regs::CHAN_TX_CONTROL_ST);
    let q = hardware.read(regs::MTL_TXQ0_OP_MODE);
    hardware.write(regs::MTL_TXQ0_OP_MODE, q | regs::MTL_TXQ_OP_MODE_FTQ);
    let mut flushed = false;
    for _ in 0..10_000 {
        if hardware.read(regs::MTL_TXQ0_OP_MODE) & regs::MTL_TXQ_OP_MODE_FTQ == 0 {
            flushed = true;
            break;
        }
        hardware.delay_us(1);
    }
    let conf = hardware.read(regs::MAC_CONF);
    hardware.write(regs::MAC_CONF, conf & !regs::MAC_CONF_TE);
    hardware.write(regs::CHAN_INTR_ENA, 0);
    if flushed {
        Ok(())
    } else {
        Err(Error::FlushTimeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jh7110::BUS;
    use crate::sim::{Memory, NetworkInterfaceController};

    const PHYS: u64 = 0x4800_0000;
    const STATION_ADDRESS: [u8; 6] = [0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c];

    #[test]
    fn a_reset_that_completes_reports_how_long_it_took() {
        let mut nic = NetworkInterfaceController::new(PHYS, Memory::coherent(PHYS));
        // The fifth read sees the bit clear, after four 10 µs waits.
        nic.set_reset_polls(5);
        let waited = soft_reset(&mut nic).unwrap();
        assert_eq!(waited, 40);
        assert_eq!(nic.reg(regs::DMA_MODE) & regs::DMA_MODE_SWR, 0);
    }

    #[test]
    fn a_reset_with_no_receive_clock_times_out_rather_than_hanging() {
        let mut nic = NetworkInterfaceController::new(PHYS, Memory::coherent(PHYS));
        nic.stop_receive_clock();
        assert_eq!(soft_reset(&mut nic), Err(Error::ResetTimeout));
    }

    #[test]
    fn the_station_address_round_trips_through_its_registers() {
        let (hi, lo) = address_words(STATION_ADDRESS);
        assert_eq!(hi, 0x2c47);
        assert_eq!(lo, 0x0039_cf6c);
        assert_eq!(station_address(hi, lo), Some(STATION_ADDRESS));
        assert_eq!(station_address(0xffff_ffff, 0xffff_ffff), None);
        assert_eq!(station_address(0, 0), None);
        assert_eq!(station_address(0, 1), None, "multicast");
    }

    #[test]
    fn identity_decodes_the_address_width_and_queue_sizes() {
        let id = Identity {
            version: 0x1052,
            features: [0, 1 << 14 | 4 << 6 | 4, 0, 0],
        };
        assert_eq!(id.core_version(), 0x52);
        assert_eq!(id.dma_bits(), 40);
        assert_eq!(id.transmit_queue_bytes(), 2048);
        assert_eq!(id.receive_queue_bytes(), 2048);
    }

    #[test]
    fn a_region_past_the_address_width_or_across_4_gib_is_refused() {
        let bytes = layout::BYTES;
        assert!(region_in_reach(0x4000_0000, bytes, 32));
        assert!(!region_in_reach(0x1_0000_0000, bytes, 32));
        assert!(region_in_reach(0x1_0000_0000, bytes, 40));
        assert!(
            !region_in_reach(0x1_0000_0000 - 4096, bytes, 40),
            "straddles"
        );
        assert!(!region_in_reach(u64::MAX - 10, bytes, 48));
    }

    #[test]
    fn configure_programs_the_rings_with_nothing_available_and_every_interrupt_masked() {
        let mut nic = NetworkInterfaceController::new(PHYS, Memory::coherent(PHYS));
        let id = Identity::read(&mut nic);
        configure(&mut nic, &id, &BUS, PHYS, STATION_ADDRESS).unwrap();
        assert_eq!(nic.reg(regs::CHAN_RX_BASE), PHYS as u32);
        assert_eq!(
            nic.reg(regs::CHAN_TX_BASE),
            (PHYS + layout::TRANSMIT_RING) as u32
        );
        assert_eq!(nic.reg(regs::CHAN_RX_TAIL), nic.reg(regs::CHAN_RX_BASE));
        assert_eq!(nic.reg(regs::CHAN_TX_TAIL), nic.reg(regs::CHAN_TX_BASE));
        assert_eq!(nic.reg(regs::CHAN_RX_RING_LEN), 15);
        assert_eq!(
            nic.reg(regs::CHAN_RX_CONTROL) >> 1 & regs::CHAN_RX_CONTROL_RBSZ_MASK,
            2048
        );
        assert_eq!(nic.reg(regs::CHAN_INTR_ENA), 0);
        assert_eq!(nic.reg(regs::MAC_INT_EN), 0);
        assert_eq!(nic.reg(regs::MAC_RXQ_CTRL0), regs::MAC_RXQ_CTRL0_Q0_DCB);
        assert_eq!(
            station_address(nic.reg(regs::MAC_ADDR0_HI), nic.reg(regs::MAC_ADDR0_LO)),
            Some(STATION_ADDRESS)
        );
        // radon's bus: fixed burst, no PBLx8, 16-beat bursts, threshold mode.
        assert_ne!(nic.reg(regs::DMA_SYSBUS_MODE) & regs::DMA_SYSBUS_MODE_FB, 0);
        assert_eq!(nic.reg(regs::CHAN_CONTROL) & regs::CHAN_CONTROL_PBLX8, 0);
        assert_eq!(nic.reg(regs::CHAN_TX_CONTROL) >> 16 & 0x3f, 16);
        assert_eq!(
            nic.reg(regs::MTL_RXQ0_OP_MODE) & regs::MTL_RXQ_OP_MODE_RSF,
            0,
            "threshold mode"
        );
    }

    #[test]
    fn the_queue_size_comes_from_the_hardware_not_the_vendor_trees_number() {
        // 262144 bytes, the vendor tree's rx-fifo-depth, is 1023 in a 10-bit field: it would fit,
        // and it would be a lie. The hardware's 2048 is 7.
        let mode = receive_queue_mode(0, &BUS, 2048);
        assert_eq!(mode >> regs::MTL_RXQ_OP_MODE_RQS_SHIFT & 0x3ff, 7);
        assert_eq!(mode & 1 << 7, 0, "no flow control thresholds below 4 KiB");
        let mode = transmit_queue_mode(0, &BUS, 2048);
        assert_eq!(mode >> regs::MTL_TXQ_OP_MODE_TQS_SHIFT & 0x1ff, 7);
        assert_eq!(
            mode & regs::MTL_TXQ_OP_MODE_TXQEN_MASK,
            regs::MTL_TXQ_OP_MODE_TXQEN
        );
    }

    #[test]
    fn the_mac_word_selects_speed_and_duplex_and_never_jumbo() {
        let gig = Link {
            speed: Speed::Thousand,
            full_duplex: true,
        };
        let c = mac_conf(regs::MAC_CONF_PS | regs::MAC_CONF_LM, gig, false);
        assert_eq!(c & (regs::MAC_CONF_PS | regs::MAC_CONF_FES), 0);
        assert_ne!(c & regs::MAC_CONF_DM, 0);
        assert_eq!(c & regs::MAC_CONF_LM, 0, "loopback cleared");
        assert_eq!(c & (1 << 16), 0, "JE");
        let c = mac_conf(
            0,
            Link {
                speed: Speed::Hundred,
                full_duplex: false,
            },
            true,
        );
        assert_eq!(
            c & (regs::MAC_CONF_PS | regs::MAC_CONF_FES | regs::MAC_CONF_DM),
            regs::MAC_CONF_PS | regs::MAC_CONF_FES
        );
        assert_ne!(c & regs::MAC_CONF_LM, 0);
    }

    #[test]
    fn stop_quiesces_and_reports_a_flush_that_never_finishes() {
        let mut nic = NetworkInterfaceController::new(PHYS, Memory::coherent(PHYS));
        let id = Identity::read(&mut nic);
        configure(&mut nic, &id, &BUS, PHYS, STATION_ADDRESS).unwrap();
        start(
            &mut nic,
            Link {
                speed: Speed::Thousand,
                full_duplex: true,
            },
            false,
        );
        assert!(stop(&mut nic).is_ok());
        assert_eq!(nic.reg(regs::CHAN_RX_CONTROL) & regs::CHAN_RX_CONTROL_SR, 0);
        assert_eq!(nic.reg(regs::MAC_CONF) & regs::MAC_CONF_TE, 0);
        nic.wedge_transmit_flush();
        assert_eq!(stop(&mut nic), Err(Error::FlushTimeout));
    }
    /// **A register file left by whoever ran last**: every register reads `fill` until written,
    /// and every write is logged in order. Nothing self-clears. Zero and all-ones together are the
    /// two states a read-modify-write must be right from: zero shows every bit that is set, all
    /// ones every bit that is cleared, and a sequence that only works from the reset value fails
    /// one of them.
    struct Seeded {
        fill: u32,
        regs: std::collections::BTreeMap<u32, u32>,
        writes: std::vec::Vec<(u32, u32)>,
    }

    impl Seeded {
        fn new(fill: u32) -> Seeded {
            Seeded {
                fill,
                regs: std::collections::BTreeMap::new(),
                writes: std::vec::Vec::new(),
            }
        }
        fn reg(&self, off: u32) -> u32 {
            self.regs.get(&off).copied().unwrap_or(self.fill)
        }
    }

    impl Hardware for Seeded {
        fn read(&mut self, off: u32) -> u32 {
            self.reg(off)
        }
        fn write(&mut self, off: u32, v: u32) {
            self.writes.push((off, v));
            self.regs.insert(off, v);
        }
        fn delay_us(&mut self, _us: u64) {}
    }

    /// 40-bit addressing and 2 KiB FIFOs, the simulated controller's identity.
    const FORTY_BIT: Identity = Identity {
        version: 0x1052,
        features: [0, 1 << 14 | 4 << 6 | 4, 0, 0],
    };

    /// A bus unlike radon's in every field, with values wider than their fields, so each field's
    /// mask and shift is seen: `PBLx8` on, store and forward, one burst length, and 4 KiB FIFOs
    /// (big enough for flow control) at 32 bits.
    const ODD: Bus = Bus {
        fixed_burst: false,
        channel_burst_length_times_8: true,
        transmit_channel_burst_length: 0x48,
        receive_channel_burst_length: 0x41,
        threshold_mode: false,
        bus_burst_lengths: regs::DMA_SYSBUS_MODE_BLEN_32,
        read_outstanding: 0x13,
        write_outstanding: 0x25,
    };
    const THIRTY_TWO_BIT: Identity = Identity {
        version: 0x1052,
        features: [0, 5 << 6 | 5, 0, 0],
    };

    fn configured(fill: u32, id: &Identity, bus: &Bus, region: u64) -> Seeded {
        let mut hardware = Seeded::new(fill);
        configure(&mut hardware, id, bus, region, STATION_ADDRESS).unwrap();
        hardware
    }

    /// **Every word `configure` writes, from both starting states, for radon's bus above 4 GiB.**
    /// The values are worked from the header's fields by hand: the bus mode is fixed burst,
    /// enhanced addressing, both outstanding limits 15 and the four burst lengths; channel 0
    /// carries 16-beat bursts, operate-on-second-frame, and a 2048-byte buffer in bits 14:1;
    /// the queues are in threshold mode at 2 KiB, size field 7; transmit flow control pauses for
    /// `0xffff` quanta.
    #[test]
    fn configure_writes_each_field_and_keeps_every_other_bit() {
        let region = 0x1_4800_0000;
        let zero = configured(0, &FORTY_BIT, &BUS, region);
        let ones = configured(u32::MAX, &FORTY_BIT, &BUS, region);
        for (reg, from_zero, from_ones) in [
            (regs::DMA_SYSBUS_MODE, 0x0f0f_08f1, 0xffff_ffff),
            (regs::CHAN_CONTROL, 0, 0xfffe_ffff),
            (regs::CHAN_TX_CONTROL, 0x0010_0010, 0xffd0_ffff),
            (regs::CHAN_RX_CONTROL, 0x0010_1000, 0xffd0_9001),
            (regs::MTL_RXQ0_OP_MODE, 0x0070_0018, 0xc07f_ffdf),
            (regs::MTL_TXQ0_OP_MODE, 0x0007_0078, 0xfe07_fff9),
            (regs::MAC_Q0_TX_FLOW_CTRL, 0xffff_0001, 0xffff_ffff),
            (regs::MAC_RX_FLOW_CTRL, 1, 0xffff_ffff),
        ] {
            assert_eq!(zero.reg(reg), from_zero, "{reg:#x} from zero");
            assert_eq!(ones.reg(reg), from_ones, "{reg:#x} from all ones");
        }
        for hardware in [&zero, &ones] {
            assert_eq!(hardware.reg(regs::CHAN_TX_BASE_HI), 1);
            assert_eq!(hardware.reg(regs::CHAN_TX_BASE), 0x4800_1000);
            assert_eq!(hardware.reg(regs::CHAN_RX_BASE_HI), 1);
            assert_eq!(hardware.reg(regs::CHAN_RX_BASE), 0x4800_0000);
        }
    }

    /// The same for a bus unlike radon's at 32 bits: no enhanced addressing, each field masked to
    /// its width (`0x13` is 3, `0x48` is 8), `PBLx8` set, store and forward, and the flow control
    /// thresholds a 4 KiB receive FIFO gets.
    #[test]
    fn configure_masks_each_bus_field_to_its_width() {
        let zero = configured(0, &THIRTY_TWO_BIT, &ODD, PHYS);
        let ones = configured(u32::MAX, &THIRTY_TWO_BIT, &ODD, PHYS);
        for (reg, from_zero, from_ones) in [
            (regs::DMA_SYSBUS_MODE, 0x0503_0010, 0xf5f3_ffff),
            (regs::CHAN_CONTROL, 0x0001_0000, 0xffff_ffff),
            (regs::CHAN_TX_CONTROL, 0x0008_0010, 0xffc8_ffff),
            (regs::CHAN_RX_CONTROL, 0x0001_1000, 0xffc1_9001),
            (regs::MTL_RXQ0_OP_MODE, 0x00f0_c1a0, 0xc0f0_c1ff),
            (regs::MTL_TXQ0_OP_MODE, 0x000f_000a, 0xfe0f_fffb),
        ] {
            assert_eq!(zero.reg(reg), from_zero, "{reg:#x} from zero");
            assert_eq!(ones.reg(reg), from_ones, "{reg:#x} from all ones");
        }
    }

    /// Both queue modes in both modes from both starting states, at a FIFO big enough for flow
    /// control: threshold mode clears store-and-forward and sets the threshold, store-and-forward
    /// sets its bit, the size is `bytes / 256 - 1`, and receive flow control is enabled with
    /// RFD 3 and RFA 1 in place of whatever was there.
    #[test]
    fn the_queue_modes_from_zero_and_from_all_ones() {
        let sf = Bus {
            threshold_mode: false,
            ..BUS
        };
        assert_eq!(receive_queue_mode(0, &BUS, 2048), 0x0070_0018);
        assert_eq!(receive_queue_mode(u32::MAX, &BUS, 2048), 0xc07f_ffdf);
        assert_eq!(receive_queue_mode(0, &sf, 8192), 0x01f0_c1a0);
        assert_eq!(receive_queue_mode(u32::MAX, &sf, 8192), 0xc1f0_c1ff);
        assert_eq!(transmit_queue_mode(0, &BUS, 2048), 0x0007_0078);
        assert_eq!(transmit_queue_mode(u32::MAX, &BUS, 2048), 0xfe07_fff9);
        assert_eq!(transmit_queue_mode(0, &sf, 8192), 0x001f_000a);
        assert_eq!(transmit_queue_mode(u32::MAX, &sf, 8192), 0xfe1f_fffb);
    }

    /// The MAC word from zero and from all ones: the speed and duplex bits are replaced, every
    /// bit `mac_conf` sets is set whatever was there, and nothing else moves.
    #[test]
    fn the_mac_word_from_zero_and_from_all_ones() {
        let gig = Link {
            speed: Speed::Thousand,
            full_duplex: true,
        };
        let ten_half = Link {
            speed: Speed::Ten,
            full_duplex: false,
        };
        assert_eq!(mac_conf(0, gig, false), 0x0006_2203);
        assert_eq!(mac_conf(u32::MAX, gig, false), 0xffff_2fff);
        assert_eq!(mac_conf(u32::MAX, ten_half, true), 0xffff_9fff);
        assert_eq!(
            mac_conf(
                u32::MAX,
                Link {
                    speed: Speed::Hundred,
                    full_duplex: false
                },
                false
            ),
            0xffff_cfff
        );
    }

    /// `start` sets the two start bits and `stop` clears them and the MAC's enables, from either
    /// starting state, in OpenBSD's order. Nothing here clears the flush bit, so `stop` reports
    /// the timeout after writing everything it writes.
    #[test]
    fn start_and_stop_change_only_their_own_bits() {
        let gig = Link {
            speed: Speed::Thousand,
            full_duplex: true,
        };
        for (fill, set, cleared) in [(0, 1, 0), (u32::MAX, u32::MAX, 0xffff_fffe)] {
            let mut hardware = Seeded::new(fill);
            start(&mut hardware, gig, false);
            assert_eq!(hardware.reg(regs::CHAN_RX_CONTROL), set);
            assert_eq!(hardware.reg(regs::CHAN_TX_CONTROL), set);
            let mut hardware = Seeded::new(fill);
            assert_eq!(stop(&mut hardware), Err(Error::FlushTimeout));
            assert_eq!(
                hardware.writes,
                [
                    (regs::MAC_CONF, cleared),
                    (regs::CHAN_RX_CONTROL, cleared),
                    (regs::CHAN_TX_CONTROL, cleared),
                    (regs::MTL_TXQ0_OP_MODE, fill | 1),
                    (regs::MAC_CONF, cleared & !2),
                    (regs::CHAN_INTR_ENA, 0),
                ]
            );
        }
    }

    /// A reset asked for while the bit already reads set (one still running, or a word left that
    /// way) still writes the bit set rather than toggling it off.
    #[test]
    fn a_reset_writes_the_bit_set_whatever_it_read() {
        let mut hardware = Seeded::new(u32::MAX);
        assert_eq!(soft_reset(&mut hardware), Err(Error::ResetTimeout));
        assert_eq!(hardware.writes, [(regs::DMA_MODE, u32::MAX)]);
    }

    /// The address width's every encoding, and a FIFO field past the largest the databook names
    /// saturating at 256 KiB rather than shifting bits off the word.
    #[test]
    fn every_address_width_encoding_and_a_saturating_queue() {
        let width = |bits: u32| {
            Identity {
                version: 0,
                features: [0, bits << 14, 0, 0],
            }
            .dma_bits()
        };
        assert_eq!([0, 1, 2, 3].map(width), [32, 40, 48, 32]);
        let queue_bytes = |field: u32| {
            Identity {
                version: 0,
                features: [0, field, 0, 0],
            }
            .receive_queue_bytes()
        };
        assert_eq!(queue_bytes(11), 256 * 1024);
        assert_eq!(queue_bytes(0x1f), 256 * 1024);
    }

    /// A region that ends exactly at a 4 GiB boundary is in reach: its last byte is below it.
    #[test]
    fn a_region_ending_at_4_gib_is_in_reach() {
        let bytes = layout::BYTES;
        assert!(region_in_reach(0x1_0000_0000 - bytes, bytes, 32));
    }
}
