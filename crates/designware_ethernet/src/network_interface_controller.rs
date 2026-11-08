//! **The controller's sequences**: identify, reset, configure, start, stop.
//!
//! OpenBSD's `dwqe_attach`, `dwqe_reset`, `dwqe_up`, `dwqe_down` and `dwqe_mii_statchg`
//! (`sys/dev/ic/dwqe.c`, ISC license, notice in the crate root), in their order, with three changes
//! each argued at the place it is made: the receive buffer size is written rather than inherited,
//! jumbo frames are not enabled, and nothing is allocated (the DMA region is the caller's, laid out
//! by [`crate::layout`]). Every sequence runs against [`Hardware`], so the host tests run them
//! against `simulator`'s controller.
//!
//! Name: ratified 2026-11-03 (UTC), #3122.

use crate::motorcomm_physical_layer_device::{Link, Speed};
use crate::{BUFFER_BYTES, Hardware, RECEIVE_ENTRIES, TRANSMIT_ENTRIES, layout, registers};

/// Why the controller was not brought up.
///
/// Name: ratified 2026-11-03 (UTC), #3122.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The software reset did not self-clear in [`RESET_LIMIT_MICROSECONDS`]. On this controller the usual
    /// cause is a missing receive clock from the PHY, which the reset needs to complete.
    ResetTimeout,
    /// The DMA region is not somewhere this controller can address: past its address width
    /// ([`Identity::direct_memory_access_address_bits`]), or straddling a 4 GiB boundary, which the
    /// 32-bit tail pointer registers cannot express.
    RegionOutOfReach {
        /// The controller's address width.
        direct_memory_access_address_bits: u8,
    },
    /// The transmit queue did not finish flushing on [`stop`].
    FlushTimeout,
}

/// How long the software reset may take: OpenBSD's 30000 polls 10 µs apart.
///
/// Name: ratified 2026-11-02 (UTC), #3094.
pub const RESET_LIMIT_MICROSECONDS: u64 = 300_000;

/// What the controller says it is.
///
/// Name: ratified 2026-11-03 (UTC), #3122.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    /// [`registers::CORE_VERSION_REGISTER`].
    pub version: u32,
    /// The four hardware feature registers.
    pub features: [u32; 4],
}

impl Identity {
    /// Read the version and feature registers.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3122.
    pub fn read(hardware: &mut impl Hardware) -> Identity {
        let mut features = [0; 4];
        for (index, feature) in features.iter_mut().enumerate() {
            *feature = hardware.read(registers::mac_hardware_feature_register(index as u32));
        }
        Identity {
            version: hardware.read(registers::CORE_VERSION_REGISTER),
            features,
        }
    }

    /// The Synopsys core version, `0x52` for the 5.20 both trees name.
    ///
    /// Name: ratified 2026-11-03 (UTC), #3122.
    pub const fn core_version(&self) -> u8 {
        self.version as u8
    }

    /// The DMA's address width from feature register 1's `ADDR64` field: 32, 40 or 48 bits.
    ///
    /// Name: ratified 2026-11-02 (UTC), #3094.
    pub const fn direct_memory_access_address_bits(&self) -> u8 {
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
    let clamped_field = if field > 11 { 11 } else { field };
    128 << clamped_field
}

/// **Reset the whole controller** and wait for the reset to finish. Returns how long it took.
///
/// Name: ratified 2026-11-03 (UTC), #3122.
pub fn soft_reset(hardware: &mut impl Hardware) -> Result<u64, Error> {
    let mode = hardware.read(registers::DIRECT_MEMORY_ACCESS_BUS_MODE_REGISTER);
    hardware.write(
        registers::DIRECT_MEMORY_ACCESS_BUS_MODE_REGISTER,
        mode | registers::DIRECT_MEMORY_ACCESS_BUS_MODE_SOFTWARE_RESET,
    );
    let mut waited = 0;
    loop {
        if hardware.read(registers::DIRECT_MEMORY_ACCESS_BUS_MODE_REGISTER)
            & registers::DIRECT_MEMORY_ACCESS_BUS_MODE_SOFTWARE_RESET
            == 0
        {
            return Ok(waited);
        }
        if waited >= RESET_LIMIT_MICROSECONDS {
            return Err(Error::ResetTimeout);
        }
        hardware.delay_microseconds(10);
        waited += 10;
    }
}

/// **The station address in the two address registers**, OpenBSD's `dwqe_lladdr_read` byte order,
/// or `None` when they hold nothing a station could own: all zero or all ones (the reset value),
/// or a multicast address.
///
/// Name: ratified 2026-11-03 (UTC), #3122.
pub fn station_address(high: u32, low: u32) -> Option<[u8; 6]> {
    let [byte_0, byte_1, byte_2, byte_3] = low.to_le_bytes();
    let [byte_4, byte_5, _, _] = high.to_le_bytes();
    let station_address = [byte_0, byte_1, byte_2, byte_3, byte_4, byte_5];
    if station_address == [0; 6] || station_address == [0xff; 6] || byte_0 & 1 != 0 {
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
/// `snps,axi-config`. [`crate::jh7110::BUS_CONFIGURATION`] is radon's.
///
/// Name: ratified 2026-11-03 (UTC), #3122.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusConfiguration {
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
    /// The system bus's permitted burst lengths: the `registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_REGISTER` `BLEN` bits
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
pub(crate) fn receive_queue_mode(current: u32, bus: &BusConfiguration, queue_bytes: u32) -> u32 {
    let mut mode = current;
    if bus.threshold_mode {
        mode &= !(registers::RECEIVE_QUEUE_0_OPERATION_MODE_STORE_AND_FORWARD
            | registers::RECEIVE_QUEUE_0_OPERATION_MODE_THRESHOLD_CONTROL_MASK);
        mode |= registers::RECEIVE_QUEUE_0_OPERATION_MODE_THRESHOLD_CONTROL_128_BYTES;
    } else {
        mode |= registers::RECEIVE_QUEUE_0_OPERATION_MODE_STORE_AND_FORWARD;
    }
    let receive_queue_size = (queue_bytes / 256).saturating_sub(1);
    mode &= !registers::RECEIVE_QUEUE_0_OPERATION_MODE_QUEUE_SIZE_MASK;
    mode |= (receive_queue_size << registers::RECEIVE_QUEUE_0_OPERATION_MODE_QUEUE_SIZE_SHIFT)
        & registers::RECEIVE_QUEUE_0_OPERATION_MODE_QUEUE_SIZE_MASK;
    if queue_bytes >= 4096 {
        // EHFC, then RFD = 3 and RFA = 1: OpenBSD's flow control thresholds.
        mode |= 1 << 7;
        mode &= !((0x3f << 14) | (0x3f << 8));
        mode |= (0x3 << 14) | (0x1 << 8);
    }
    mode
}

/// The fields this word composes share no bit, so every `|` here is its `^`: store-and-forward
/// against the threshold field, and the two flow-control threshold fields against each other;
/// the compiler checks it here, and the mutation exclusion for `receive_queue_mode` names this
/// constant. Clippy's `eq_op` lint folds the second conjunct's literals to `0 == 0`; that is
/// what a gate is, allowed here for exactly that reason.
///
/// Name: ratified 2026-11-03 (UTC), #3120.
#[allow(clippy::eq_op)]
pub const RECEIVE_QUEUE_MODE_FIELDS_DO_NOT_OVERLAP: () = assert!(
    registers::RECEIVE_QUEUE_0_OPERATION_MODE_STORE_AND_FORWARD
        & registers::RECEIVE_QUEUE_0_OPERATION_MODE_THRESHOLD_CONTROL_MASK
        == 0
        && (0x3f << 14) & (0x3f << 8) == 0
);

/// The transmit operation mode word for a queue of `queue_bytes` bytes, OpenBSD's arithmetic.
pub(crate) fn transmit_queue_mode(current: u32, bus: &BusConfiguration, queue_bytes: u32) -> u32 {
    let mut mode = current;
    if bus.threshold_mode {
        mode &= !(registers::TRANSMIT_QUEUE_0_OPERATION_MODE_STORE_AND_FORWARD
            | registers::TRANSMIT_QUEUE_0_OPERATION_MODE_THRESHOLD_CONTROL_MASK);
        mode |= registers::TRANSMIT_QUEUE_0_OPERATION_MODE_THRESHOLD_CONTROL_512_BYTES;
    } else {
        mode |= registers::TRANSMIT_QUEUE_0_OPERATION_MODE_STORE_AND_FORWARD;
    }
    mode &= !registers::TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_ENABLE_MASK;
    mode |= registers::TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_ENABLED;
    let transmit_queue_size = (queue_bytes / 256).saturating_sub(1);
    mode &= !registers::TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_SIZE_MASK;
    mode |= (transmit_queue_size << registers::TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_SIZE_SHIFT)
        & registers::TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_SIZE_MASK;
    mode
}

/// Store-and-forward and the threshold field share no bit, so the one `|` here is its `^`, which
/// the compiler checks here; the mutation exclusion for `transmit_queue_mode` names this
/// constant.
///
/// Name: ratified 2026-11-03 (UTC), #3120.
pub const TRANSMIT_QUEUE_MODE_FIELDS_DO_NOT_OVERLAP: () = assert!(
    registers::TRANSMIT_QUEUE_0_OPERATION_MODE_STORE_AND_FORWARD
        & registers::TRANSMIT_QUEUE_0_OPERATION_MODE_THRESHOLD_CONTROL_MASK
        == 0
);

/// Can a controller of `direct_memory_access_address_bits` address a region of `bytes` at
/// `physical_address`, with tail pointers that only carry the low 32 bits?
pub(crate) const fn is_region_in_reach(
    physical_address: u64,
    bytes: u64,
    direct_memory_access_address_bits: u8,
) -> bool {
    let Some(end) = physical_address.checked_add(bytes) else {
        return false;
    };
    let last = end - 1;
    last >> direct_memory_access_address_bits == 0 && physical_address >> 32 == last >> 32
}

/// The system bus mode: fixed burst, enhanced addressing past 32 bits, the outstanding request
/// limits and the permitted burst lengths.
fn program_system_bus_mode(hardware: &mut impl Hardware, id: &Identity, bus: &BusConfiguration) {
    let mut mode = hardware.read(registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_REGISTER);
    if bus.fixed_burst {
        mode |= registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_FIXED_BURST;
    }
    if id.direct_memory_access_address_bits() > 32 {
        mode |= registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_ENHANCED_ADDRESS_MODE_ENABLE;
    }
    mode &= !(registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_READ_OUTSTANDING_REQUEST_LIMIT_MASK
        | registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_WRITE_OUTSTANDING_REQUEST_LIMIT_MASK);
    mode |= (bus.read_outstanding << registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_READ_OUTSTANDING_REQUEST_LIMIT_SHIFT)
        & registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_READ_OUTSTANDING_REQUEST_LIMIT_MASK
        | (bus.write_outstanding << registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_WRITE_OUTSTANDING_REQUEST_LIMIT_SHIFT)
            & registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_WRITE_OUTSTANDING_REQUEST_LIMIT_MASK
        | bus.bus_burst_lengths;
    hardware.write(
        registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_REGISTER,
        mode,
    );
}

/// The channel's control words: the burst-length multiplier, each side's burst length, operate on
/// second packet, and the receive buffer size.
fn program_channel_bursts(hardware: &mut impl Hardware, bus: &BusConfiguration) {
    let control = hardware.read(registers::CHANNEL_CONTROL_REGISTER);
    hardware.write(
        registers::CHANNEL_CONTROL_REGISTER,
        if bus.channel_burst_length_times_8 {
            control | registers::CHANNEL_CONTROL_EIGHT_TIMES_PROGRAMMABLE_BURST_LENGTH
        } else {
            control & !registers::CHANNEL_CONTROL_EIGHT_TIMES_PROGRAMMABLE_BURST_LENGTH
        },
    );
    let transmit = hardware.read(registers::CHANNEL_TRANSMIT_CONTROL_REGISTER)
        & !registers::CHANNEL_TRANSMIT_CONTROL_PROGRAMMABLE_BURST_LENGTH_MASK;
    hardware.write(
        registers::CHANNEL_TRANSMIT_CONTROL_REGISTER,
        transmit
            | (bus.transmit_channel_burst_length
                << registers::CHANNEL_TRANSMIT_CONTROL_PROGRAMMABLE_BURST_LENGTH_SHIFT)
                & registers::CHANNEL_TRANSMIT_CONTROL_PROGRAMMABLE_BURST_LENGTH_MASK
            | registers::CHANNEL_TRANSMIT_CONTROL_OPERATE_ON_SECOND_PACKET,
    );
    // The buffer size is written, not inherited (OpenBSD inherits it): a size left by firmware is
    // a fact about whoever ran last, and the device writes a frame up to it.
    let receive_buffer_size = registers::CHANNEL_RECEIVE_CONTROL_BUFFER_SIZE_MASK
        << registers::CHANNEL_RECEIVE_CONTROL_BUFFER_SIZE_SHIFT;
    let receive = hardware.read(registers::CHANNEL_RECEIVE_CONTROL_REGISTER)
        & !(registers::CHANNEL_RECEIVE_CONTROL_PROGRAMMABLE_BURST_LENGTH_MASK
            | receive_buffer_size);
    hardware.write(
        registers::CHANNEL_RECEIVE_CONTROL_REGISTER,
        receive
            | (bus.receive_channel_burst_length
                << registers::CHANNEL_RECEIVE_CONTROL_PROGRAMMABLE_BURST_LENGTH_SHIFT)
                & registers::CHANNEL_RECEIVE_CONTROL_PROGRAMMABLE_BURST_LENGTH_MASK
            | (BUFFER_BYTES as u32) << registers::CHANNEL_RECEIVE_CONTROL_BUFFER_SIZE_SHIFT,
    );
}

/// Both descriptor rings: base, length and tail, each pointing at an empty ring.
fn program_descriptor_rings(hardware: &mut impl Hardware, region: u64) {
    let transmit_ring = region + layout::TRANSMIT_RING;
    let receive_ring = region + layout::RECEIVE_RING;
    hardware.write(
        registers::CHANNEL_TRANSMIT_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_HIGH_REGISTER,
        (transmit_ring >> 32) as u32,
    );
    hardware.write(
        registers::CHANNEL_TRANSMIT_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_LOW_REGISTER,
        transmit_ring as u32,
    );
    hardware.write(
        registers::CHANNEL_TRANSMIT_DESCRIPTOR_RING_LENGTH_REGISTER,
        u32::from(TRANSMIT_ENTRIES) - 1,
    );
    hardware.write(
        registers::CHANNEL_TRANSMIT_DESCRIPTOR_TAIL_POINTER_REGISTER,
        transmit_ring as u32,
    );
    hardware.write(
        registers::CHANNEL_RECEIVE_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_HIGH_REGISTER,
        (receive_ring >> 32) as u32,
    );
    hardware.write(
        registers::CHANNEL_RECEIVE_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_LOW_REGISTER,
        receive_ring as u32,
    );
    hardware.write(
        registers::CHANNEL_RECEIVE_DESCRIPTOR_RING_LENGTH_REGISTER,
        u32::from(RECEIVE_ENTRIES) - 1,
    );
    hardware.write(
        registers::CHANNEL_RECEIVE_DESCRIPTOR_TAIL_POINTER_REGISTER,
        receive_ring as u32,
    );
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
///
/// Name: ratified 2026-11-03 (UTC), #3122.
pub fn configure(
    hardware: &mut impl Hardware,
    id: &Identity,
    bus: &BusConfiguration,
    region: u64,
    station_address: [u8; 6],
) -> Result<(), Error> {
    if !is_region_in_reach(
        region,
        layout::DIRECT_MEMORY_ACCESS_REGION_BYTES,
        id.direct_memory_access_address_bits(),
    ) {
        return Err(Error::RegionOutOfReach {
            direct_memory_access_address_bits: id.direct_memory_access_address_bits(),
        });
    }

    program_system_bus_mode(hardware, id, bus);
    program_channel_bursts(hardware, bus);
    program_descriptor_rings(hardware, region);

    let receive_mode = receive_queue_mode(
        hardware.read(registers::RECEIVE_QUEUE_0_OPERATION_MODE_REGISTER),
        bus,
        id.receive_queue_bytes(),
    );
    hardware.write(
        registers::RECEIVE_QUEUE_0_OPERATION_MODE_REGISTER,
        receive_mode,
    );
    let transmit_mode = transmit_queue_mode(
        hardware.read(registers::TRANSMIT_QUEUE_0_OPERATION_MODE_REGISTER),
        bus,
        id.transmit_queue_bytes(),
    );
    hardware.write(
        registers::TRANSMIT_QUEUE_0_OPERATION_MODE_REGISTER,
        transmit_mode,
    );

    let flow = hardware.read(registers::TRANSMIT_FLOW_CONTROL_QUEUE_0_REGISTER);
    hardware.write(
        registers::TRANSMIT_FLOW_CONTROL_QUEUE_0_REGISTER,
        flow | 0xffff << registers::TRANSMIT_FLOW_CONTROL_QUEUE_0_PAUSE_TIME_SHIFT
            | registers::TRANSMIT_FLOW_CONTROL_QUEUE_0_ENABLE,
    );
    let flow = hardware.read(registers::RECEIVE_FLOW_CONTROL_REGISTER);
    hardware.write(
        registers::RECEIVE_FLOW_CONTROL_REGISTER,
        flow | registers::RECEIVE_FLOW_CONTROL_ENABLE,
    );
    hardware.write(
        registers::RECEIVE_QUEUE_CONTROL_0_REGISTER,
        registers::RECEIVE_QUEUE_CONTROL_0_DATA_CENTER_BRIDGING_QUEUE_ENABLE,
    );

    let (high, low) = address_words(station_address);
    hardware.write(registers::MAC_ADDRESS_0_HIGH_REGISTER, high);
    hardware.write(registers::MAC_ADDRESS_0_LOW_REGISTER, low);
    // Own unicast address and broadcast; no multicast, no promiscuous receive.
    hardware.write(registers::MAC_PACKET_FILTER, 0);

    hardware.write(registers::MAC_INTERRUPT_ENABLE_REGISTER, 0);
    hardware.write(registers::CHANNEL_INTERRUPT_ENABLE_REGISTER, 0);
    hardware.write(
        registers::MAC_MANAGEMENT_COUNTERS_RECEIVE_INTERRUPT_MASK_REGISTER,
        u32::MAX,
    );
    hardware.write(
        registers::MAC_MANAGEMENT_COUNTERS_TRANSMIT_INTERRUPT_MASK_REGISTER,
        u32::MAX,
    );
    hardware.write(registers::CHANNEL_STATUS_REGISTER, u32::MAX);
    Ok(())
}

/// The register fields configure composes share no bit: the outstanding-limit masks against each
/// other and against the burst-length bits (which is what `bus_burst_lengths` carries), the
/// transmit and receive burst-length fields against the bits set beside them, the receive buffer
/// size against the receive burst-length field, and the pause time against the flow-control
/// enable; the compiler checks it here, and the mutation exclusion for `configure` and the triage
/// rows for `program_system_bus_mode` and `program_channel_bursts` name this constant. A field
/// value reaches its `|` through its own mask, and a word whose field the line above cleared takes
/// a value only inside that field; where neither holds, the configure tests kill the swap.
///
/// Name: ratified 2026-11-03 (UTC), #3120.
pub const CONFIGURED_BUS_FIELDS_DO_NOT_OVERLAP: () = assert!(
    registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_READ_OUTSTANDING_REQUEST_LIMIT_MASK
        & registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_WRITE_OUTSTANDING_REQUEST_LIMIT_MASK
        == 0
        && registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_READ_OUTSTANDING_REQUEST_LIMIT_MASK
            & (registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_256_PERMITTED
                | registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_128_PERMITTED
                | registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_64_PERMITTED
                | registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_32_PERMITTED)
            == 0
        && registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_WRITE_OUTSTANDING_REQUEST_LIMIT_MASK
            & (registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_256_PERMITTED
                | registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_128_PERMITTED
                | registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_64_PERMITTED
                | registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_32_PERMITTED)
            == 0
        && registers::CHANNEL_TRANSMIT_CONTROL_PROGRAMMABLE_BURST_LENGTH_MASK
            & registers::CHANNEL_TRANSMIT_CONTROL_OPERATE_ON_SECOND_PACKET
            == 0
        && registers::CHANNEL_RECEIVE_CONTROL_PROGRAMMABLE_BURST_LENGTH_MASK
            & (registers::CHANNEL_RECEIVE_CONTROL_BUFFER_SIZE_MASK
                << registers::CHANNEL_RECEIVE_CONTROL_BUFFER_SIZE_SHIFT)
            == 0
        && (0xffff << registers::TRANSMIT_FLOW_CONTROL_QUEUE_0_PAUSE_TIME_SHIFT)
            & registers::TRANSMIT_FLOW_CONTROL_QUEUE_0_ENABLE
            == 0
);

/// **The media access control (MAC) configuration word for a link**, OpenBSD's `dwqe_mii_statchg` and `dwqe_up`: the
/// port and speed select for the negotiated speed, duplex, carrier sense off, jabber off, burst
/// on, transmitter and receiver on. `loopback` turns the MAC's own loopback on for the coherence
/// probe.
///
/// OpenBSD also sets `JE` (jumbo frames). This does not: a jumbo frame cannot fit a 2048-byte
/// buffer, would span descriptors, and would be dropped by [`crate::receive_verdict`] anyway, so
/// accepting it into the FIFO only spends ring slots on frames that are thrown away.
///
/// Name: ratified 2026-11-02 (UTC), #3094.
pub const fn mac_configuration(current: u32, link: Link, loopback: bool) -> u32 {
    let mut configuration = current
        & !(registers::MAC_CONFIGURATION_PORT_SELECT
            | registers::MAC_CONFIGURATION_FAST_ETHERNET_SPEED
            | registers::MAC_CONFIGURATION_FULL_DUPLEX
            | registers::MAC_CONFIGURATION_LOOPBACK_MODE);
    configuration |= match link.speed {
        Speed::Thousand => 0,
        Speed::Hundred => {
            registers::MAC_CONFIGURATION_PORT_SELECT
                | registers::MAC_CONFIGURATION_FAST_ETHERNET_SPEED
        }
        Speed::Ten => registers::MAC_CONFIGURATION_PORT_SELECT,
    };
    if link.full_duplex {
        configuration |= registers::MAC_CONFIGURATION_FULL_DUPLEX;
    }
    if loopback {
        configuration |= registers::MAC_CONFIGURATION_LOOPBACK_MODE;
    }
    configuration
        | registers::MAC_CONFIGURATION_PACKET_BURST_ENABLE
        | registers::MAC_CONFIGURATION_JABBER_DISABLE
        | registers::MAC_CONFIGURATION_DISABLE_CARRIER_SENSE_DURING_TRANSMISSION
        | registers::MAC_CONFIGURATION_TRANSMITTER_ENABLE
        | registers::MAC_CONFIGURATION_RECEIVER_ENABLE
}

/// The MAC word's fields share no bit: the speed, duplex and loopback bits the function clears
/// first, and the five bits the final chain sets; the compiler checks it here, and the mutation
/// exclusion for `mac_configuration` names this constant. A swap inside either group is
/// `|`-versus-`^` over disjoint bits; the chain's first swap toggles `configuration` itself, and
/// `the_mac_word_from_zero_and_from_all_ones` kills it.
///
/// Name: ratified 2026-11-03 (UTC), #3120.
pub const MAC_CONFIGURATION_BITS_DO_NOT_OVERLAP: () = assert!(
    registers::MAC_CONFIGURATION_PORT_SELECT
        | registers::MAC_CONFIGURATION_FAST_ETHERNET_SPEED
        | registers::MAC_CONFIGURATION_FULL_DUPLEX
        | registers::MAC_CONFIGURATION_LOOPBACK_MODE
        == registers::MAC_CONFIGURATION_PORT_SELECT
            ^ registers::MAC_CONFIGURATION_FAST_ETHERNET_SPEED
            ^ registers::MAC_CONFIGURATION_FULL_DUPLEX
            ^ registers::MAC_CONFIGURATION_LOOPBACK_MODE
        && (registers::MAC_CONFIGURATION_PACKET_BURST_ENABLE
            | registers::MAC_CONFIGURATION_JABBER_DISABLE
            | registers::MAC_CONFIGURATION_DISABLE_CARRIER_SENSE_DURING_TRANSMISSION
            | registers::MAC_CONFIGURATION_TRANSMITTER_ENABLE
            | registers::MAC_CONFIGURATION_RECEIVER_ENABLE)
            == (registers::MAC_CONFIGURATION_PACKET_BURST_ENABLE
                ^ registers::MAC_CONFIGURATION_JABBER_DISABLE
                ^ registers::MAC_CONFIGURATION_DISABLE_CARRIER_SENSE_DURING_TRANSMISSION
                ^ registers::MAC_CONFIGURATION_TRANSMITTER_ENABLE
                ^ registers::MAC_CONFIGURATION_RECEIVER_ENABLE)
);

/// **Start both DMA channels and the MAC** at `link`, OpenBSD's order: receive DMA, transmit DMA,
/// then the MAC. Nothing moves until the data plane hands the device descriptors.
///
/// Name: ratified 2026-11-03 (UTC), #3122.
pub fn start(hardware: &mut impl Hardware, link: Link, loopback: bool) {
    let receive = hardware.read(registers::CHANNEL_RECEIVE_CONTROL_REGISTER);
    hardware.write(
        registers::CHANNEL_RECEIVE_CONTROL_REGISTER,
        receive | registers::CHANNEL_RECEIVE_CONTROL_START_RECEIVE,
    );
    let transmit = hardware.read(registers::CHANNEL_TRANSMIT_CONTROL_REGISTER);
    hardware.write(
        registers::CHANNEL_TRANSMIT_CONTROL_REGISTER,
        transmit | registers::CHANNEL_TRANSMIT_CONTROL_START_TRANSMISSION,
    );
    let configuration = hardware.read(registers::MAC_CONFIGURATION_REGISTER);
    hardware.write(
        registers::MAC_CONFIGURATION_REGISTER,
        mac_configuration(configuration, link, loopback),
    );
}

/// **Stop the MAC and both DMA channels**, OpenBSD's `dwqe_down` order: receiver off, receive DMA
/// off, transmit DMA off, flush the transmit queue, transmitter off.
///
/// Name: ratified 2026-11-03 (UTC), #3122.
pub fn stop(hardware: &mut impl Hardware) -> Result<(), Error> {
    let configuration = hardware.read(registers::MAC_CONFIGURATION_REGISTER);
    hardware.write(
        registers::MAC_CONFIGURATION_REGISTER,
        configuration & !registers::MAC_CONFIGURATION_RECEIVER_ENABLE,
    );
    let receive = hardware.read(registers::CHANNEL_RECEIVE_CONTROL_REGISTER);
    hardware.write(
        registers::CHANNEL_RECEIVE_CONTROL_REGISTER,
        receive & !registers::CHANNEL_RECEIVE_CONTROL_START_RECEIVE,
    );
    let transmit = hardware.read(registers::CHANNEL_TRANSMIT_CONTROL_REGISTER);
    hardware.write(
        registers::CHANNEL_TRANSMIT_CONTROL_REGISTER,
        transmit & !registers::CHANNEL_TRANSMIT_CONTROL_START_TRANSMISSION,
    );
    let queue_operation_mode = hardware.read(registers::TRANSMIT_QUEUE_0_OPERATION_MODE_REGISTER);
    hardware.write(
        registers::TRANSMIT_QUEUE_0_OPERATION_MODE_REGISTER,
        queue_operation_mode | registers::TRANSMIT_QUEUE_0_OPERATION_MODE_FLUSH,
    );
    let mut flushed = false;
    for _ in 0..10_000 {
        if hardware.read(registers::TRANSMIT_QUEUE_0_OPERATION_MODE_REGISTER)
            & registers::TRANSMIT_QUEUE_0_OPERATION_MODE_FLUSH
            == 0
        {
            flushed = true;
            break;
        }
        hardware.delay_microseconds(1);
    }
    let configuration = hardware.read(registers::MAC_CONFIGURATION_REGISTER);
    hardware.write(
        registers::MAC_CONFIGURATION_REGISTER,
        configuration & !registers::MAC_CONFIGURATION_TRANSMITTER_ENABLE,
    );
    hardware.write(registers::CHANNEL_INTERRUPT_ENABLE_REGISTER, 0);
    if flushed {
        Ok(())
    } else {
        Err(Error::FlushTimeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jh7110::BUS_CONFIGURATION;
    use crate::simulator::{Memory, NetworkInterfaceController};

    const PHYSICAL_ADDRESS: u64 = 0x4800_0000;
    const STATION_ADDRESS: [u8; 6] = [0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c];

    #[test]
    fn a_reset_that_completes_reports_how_long_it_took() {
        let mut network_interface_controller =
            NetworkInterfaceController::new(PHYSICAL_ADDRESS, Memory::coherent(PHYSICAL_ADDRESS));
        // The fifth read sees the bit clear, after four 10 µs waits.
        network_interface_controller.set_reset_polls(5);
        let waited = soft_reset(&mut network_interface_controller).unwrap();
        assert_eq!(waited, 40);
        assert_eq!(
            network_interface_controller
                .register(registers::DIRECT_MEMORY_ACCESS_BUS_MODE_REGISTER)
                & registers::DIRECT_MEMORY_ACCESS_BUS_MODE_SOFTWARE_RESET,
            0
        );
    }

    #[test]
    fn a_reset_with_no_receive_clock_times_out_rather_than_hanging() {
        let mut network_interface_controller =
            NetworkInterfaceController::new(PHYSICAL_ADDRESS, Memory::coherent(PHYSICAL_ADDRESS));
        network_interface_controller.stop_receive_clock();
        assert_eq!(
            soft_reset(&mut network_interface_controller),
            Err(Error::ResetTimeout)
        );
    }

    #[test]
    fn the_station_address_round_trips_through_its_registers() {
        let (high, low) = address_words(STATION_ADDRESS);
        assert_eq!(high, 0x2c47);
        assert_eq!(low, 0x0039_cf6c);
        assert_eq!(station_address(high, low), Some(STATION_ADDRESS));
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
        assert_eq!(id.direct_memory_access_address_bits(), 40);
        assert_eq!(id.transmit_queue_bytes(), 2048);
        assert_eq!(id.receive_queue_bytes(), 2048);
    }

    #[test]
    fn a_region_past_the_address_width_or_across_4_gibibytes_is_refused() {
        let bytes = layout::DIRECT_MEMORY_ACCESS_REGION_BYTES;
        assert!(is_region_in_reach(0x4000_0000, bytes, 32));
        assert!(!is_region_in_reach(0x1_0000_0000, bytes, 32));
        assert!(is_region_in_reach(0x1_0000_0000, bytes, 40));
        assert!(
            !is_region_in_reach(0x1_0000_0000 - 4096, bytes, 40),
            "straddles"
        );
        assert!(!is_region_in_reach(u64::MAX - 10, bytes, 48));
    }

    #[test]
    fn configure_programs_the_rings_with_nothing_available_and_every_interrupt_masked() {
        let mut network_interface_controller =
            NetworkInterfaceController::new(PHYSICAL_ADDRESS, Memory::coherent(PHYSICAL_ADDRESS));
        let id = Identity::read(&mut network_interface_controller);
        configure(
            &mut network_interface_controller,
            &id,
            &BUS_CONFIGURATION,
            PHYSICAL_ADDRESS,
            STATION_ADDRESS,
        )
        .unwrap();
        assert_eq!(
            network_interface_controller.register(
                registers::CHANNEL_RECEIVE_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_LOW_REGISTER
            ),
            PHYSICAL_ADDRESS as u32
        );
        assert_eq!(
            network_interface_controller.register(
                registers::CHANNEL_TRANSMIT_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_LOW_REGISTER
            ),
            (PHYSICAL_ADDRESS + layout::TRANSMIT_RING) as u32
        );
        assert_eq!(
            network_interface_controller
                .register(registers::CHANNEL_RECEIVE_DESCRIPTOR_TAIL_POINTER_REGISTER),
            network_interface_controller.register(
                registers::CHANNEL_RECEIVE_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_LOW_REGISTER
            )
        );
        assert_eq!(
            network_interface_controller
                .register(registers::CHANNEL_TRANSMIT_DESCRIPTOR_TAIL_POINTER_REGISTER),
            network_interface_controller.register(
                registers::CHANNEL_TRANSMIT_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_LOW_REGISTER
            )
        );
        assert_eq!(
            network_interface_controller
                .register(registers::CHANNEL_RECEIVE_DESCRIPTOR_RING_LENGTH_REGISTER),
            15
        );
        assert_eq!(
            network_interface_controller.register(registers::CHANNEL_RECEIVE_CONTROL_REGISTER) >> 1
                & registers::CHANNEL_RECEIVE_CONTROL_BUFFER_SIZE_MASK,
            2048
        );
        assert_eq!(
            network_interface_controller.register(registers::CHANNEL_INTERRUPT_ENABLE_REGISTER),
            0
        );
        assert_eq!(
            network_interface_controller.register(registers::MAC_INTERRUPT_ENABLE_REGISTER),
            0
        );
        assert_eq!(
            network_interface_controller.register(registers::RECEIVE_QUEUE_CONTROL_0_REGISTER),
            registers::RECEIVE_QUEUE_CONTROL_0_DATA_CENTER_BRIDGING_QUEUE_ENABLE
        );
        assert_eq!(
            station_address(
                network_interface_controller.register(registers::MAC_ADDRESS_0_HIGH_REGISTER),
                network_interface_controller.register(registers::MAC_ADDRESS_0_LOW_REGISTER)
            ),
            Some(STATION_ADDRESS)
        );
        // radon's bus: fixed burst, no PBLx8, 16-beat bursts, threshold mode.
        assert_ne!(
            network_interface_controller
                .register(registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_REGISTER)
                & registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_FIXED_BURST,
            0
        );
        assert_eq!(
            network_interface_controller.register(registers::CHANNEL_CONTROL_REGISTER)
                & registers::CHANNEL_CONTROL_EIGHT_TIMES_PROGRAMMABLE_BURST_LENGTH,
            0
        );
        assert_eq!(
            network_interface_controller.register(registers::CHANNEL_TRANSMIT_CONTROL_REGISTER)
                >> 16
                & 0x3f,
            16
        );
        assert_eq!(
            network_interface_controller
                .register(registers::RECEIVE_QUEUE_0_OPERATION_MODE_REGISTER)
                & registers::RECEIVE_QUEUE_0_OPERATION_MODE_STORE_AND_FORWARD,
            0,
            "threshold mode"
        );
    }

    #[test]
    fn the_queue_size_comes_from_the_hardware_not_the_vendor_trees_number() {
        // 262144 bytes, the vendor tree's rx-fifo-depth, is 1023 in a 10-bit field: it would fit,
        // and it would be a lie. The hardware's 2048 is 7.
        let mode = receive_queue_mode(0, &BUS_CONFIGURATION, 2048);
        assert_eq!(
            mode >> registers::RECEIVE_QUEUE_0_OPERATION_MODE_QUEUE_SIZE_SHIFT & 0x3ff,
            7
        );
        assert_eq!(mode & 1 << 7, 0, "no flow control thresholds below 4 KiB");
        let mode = transmit_queue_mode(0, &BUS_CONFIGURATION, 2048);
        assert_eq!(
            mode >> registers::TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_SIZE_SHIFT & 0x1ff,
            7
        );
        assert_eq!(
            mode & registers::TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_ENABLE_MASK,
            registers::TRANSMIT_QUEUE_0_OPERATION_MODE_QUEUE_ENABLED
        );
    }

    #[test]
    fn the_mac_word_selects_speed_and_duplex_and_never_jumbo() {
        let gigabit = Link {
            speed: Speed::Thousand,
            full_duplex: true,
        };
        let c = mac_configuration(
            registers::MAC_CONFIGURATION_PORT_SELECT | registers::MAC_CONFIGURATION_LOOPBACK_MODE,
            gigabit,
            false,
        );
        assert_eq!(
            c & (registers::MAC_CONFIGURATION_PORT_SELECT
                | registers::MAC_CONFIGURATION_FAST_ETHERNET_SPEED),
            0
        );
        assert_ne!(c & registers::MAC_CONFIGURATION_FULL_DUPLEX, 0);
        assert_eq!(
            c & registers::MAC_CONFIGURATION_LOOPBACK_MODE,
            0,
            "loopback cleared"
        );
        assert_eq!(c & (1 << 16), 0, "JE");
        let c = mac_configuration(
            0,
            Link {
                speed: Speed::Hundred,
                full_duplex: false,
            },
            true,
        );
        assert_eq!(
            c & (registers::MAC_CONFIGURATION_PORT_SELECT
                | registers::MAC_CONFIGURATION_FAST_ETHERNET_SPEED
                | registers::MAC_CONFIGURATION_FULL_DUPLEX),
            registers::MAC_CONFIGURATION_PORT_SELECT
                | registers::MAC_CONFIGURATION_FAST_ETHERNET_SPEED
        );
        assert_ne!(c & registers::MAC_CONFIGURATION_LOOPBACK_MODE, 0);
    }

    #[test]
    fn stop_quiesces_and_reports_a_flush_that_never_finishes() {
        let mut network_interface_controller =
            NetworkInterfaceController::new(PHYSICAL_ADDRESS, Memory::coherent(PHYSICAL_ADDRESS));
        let id = Identity::read(&mut network_interface_controller);
        configure(
            &mut network_interface_controller,
            &id,
            &BUS_CONFIGURATION,
            PHYSICAL_ADDRESS,
            STATION_ADDRESS,
        )
        .unwrap();
        start(
            &mut network_interface_controller,
            Link {
                speed: Speed::Thousand,
                full_duplex: true,
            },
            false,
        );
        assert!(stop(&mut network_interface_controller).is_ok());
        assert_eq!(
            network_interface_controller.register(registers::CHANNEL_RECEIVE_CONTROL_REGISTER)
                & registers::CHANNEL_RECEIVE_CONTROL_START_RECEIVE,
            0
        );
        assert_eq!(
            network_interface_controller.register(registers::MAC_CONFIGURATION_REGISTER)
                & registers::MAC_CONFIGURATION_TRANSMITTER_ENABLE,
            0
        );
        network_interface_controller.wedge_transmit_flush();
        assert_eq!(
            stop(&mut network_interface_controller),
            Err(Error::FlushTimeout)
        );
    }
    /// **A register file left by whoever ran last**: every register reads `fill` until written,
    /// and every write is logged in order. Nothing self-clears. Zero and all-ones together are the
    /// two states a read-modify-write must be right from: zero shows every bit that is set, all
    /// ones every bit that is cleared, and a sequence that only works from the reset value fails
    /// one of them.
    struct Seeded {
        fill: u32,
        registers: std::collections::BTreeMap<u32, u32>,
        writes: std::vec::Vec<(u32, u32)>,
    }

    impl Seeded {
        fn new(fill: u32) -> Seeded {
            Seeded {
                fill,
                registers: std::collections::BTreeMap::new(),
                writes: std::vec::Vec::new(),
            }
        }
        fn register(&self, offset: u32) -> u32 {
            self.registers.get(&offset).copied().unwrap_or(self.fill)
        }
    }

    impl Hardware for Seeded {
        fn read(&mut self, offset: u32) -> u32 {
            self.register(offset)
        }
        fn write(&mut self, offset: u32, value: u32) {
            self.writes.push((offset, value));
            self.registers.insert(offset, value);
        }
        fn delay_microseconds(&mut self, _microseconds: u64) {}
    }

    /// 40-bit addressing and 2 KiB FIFOs, the simulated controller's identity.
    const FORTY_BIT: Identity = Identity {
        version: 0x1052,
        features: [0, 1 << 14 | 4 << 6 | 4, 0, 0],
    };

    /// A bus unlike radon's in every field, with values wider than their fields, so each field's
    /// mask and shift is seen: `PBLx8` on, store and forward, one burst length, and 4 KiB FIFOs
    /// (big enough for flow control) at 32 bits.
    const ODD: BusConfiguration = BusConfiguration {
        fixed_burst: false,
        channel_burst_length_times_8: true,
        transmit_channel_burst_length: 0x48,
        receive_channel_burst_length: 0x41,
        threshold_mode: false,
        bus_burst_lengths:
            registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_BURST_LENGTH_32_PERMITTED,
        read_outstanding: 0x13,
        write_outstanding: 0x25,
    };
    const THIRTY_TWO_BIT: Identity = Identity {
        version: 0x1052,
        features: [0, 5 << 6 | 5, 0, 0],
    };

    fn configured(fill: u32, id: &Identity, bus: &BusConfiguration, region: u64) -> Seeded {
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
        let zero = configured(0, &FORTY_BIT, &BUS_CONFIGURATION, region);
        let ones = configured(u32::MAX, &FORTY_BIT, &BUS_CONFIGURATION, region);
        for (register, from_zero, from_ones) in [
            (
                registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_REGISTER,
                0x0f0f_08f1,
                0xffff_ffff,
            ),
            (registers::CHANNEL_CONTROL_REGISTER, 0, 0xfffe_ffff),
            (
                registers::CHANNEL_TRANSMIT_CONTROL_REGISTER,
                0x0010_0010,
                0xffd0_ffff,
            ),
            (
                registers::CHANNEL_RECEIVE_CONTROL_REGISTER,
                0x0010_1000,
                0xffd0_9001,
            ),
            (
                registers::RECEIVE_QUEUE_0_OPERATION_MODE_REGISTER,
                0x0070_0018,
                0xc07f_ffdf,
            ),
            (
                registers::TRANSMIT_QUEUE_0_OPERATION_MODE_REGISTER,
                0x0007_0078,
                0xfe07_fff9,
            ),
            (
                registers::TRANSMIT_FLOW_CONTROL_QUEUE_0_REGISTER,
                0xffff_0001,
                0xffff_ffff,
            ),
            (registers::RECEIVE_FLOW_CONTROL_REGISTER, 1, 0xffff_ffff),
        ] {
            assert_eq!(
                zero.register(register),
                from_zero,
                "{register:#x} from zero"
            );
            assert_eq!(
                ones.register(register),
                from_ones,
                "{register:#x} from all ones"
            );
        }
        for hardware in [&zero, &ones] {
            assert_eq!(
                hardware.register(
                    registers::CHANNEL_TRANSMIT_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_HIGH_REGISTER
                ),
                1
            );
            assert_eq!(
                hardware.register(
                    registers::CHANNEL_TRANSMIT_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_LOW_REGISTER
                ),
                0x4800_1000
            );
            assert_eq!(
                hardware.register(
                    registers::CHANNEL_RECEIVE_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_HIGH_REGISTER
                ),
                1
            );
            assert_eq!(
                hardware.register(
                    registers::CHANNEL_RECEIVE_DESCRIPTOR_LIST_BASE_PHYSICAL_ADDRESS_LOW_REGISTER
                ),
                0x4800_0000
            );
        }
    }

    /// The same for a bus unlike radon's at 32 bits: no enhanced addressing, each field masked to
    /// its width (`0x13` is 3, `0x48` is 8), `PBLx8` set, store and forward, and the flow control
    /// thresholds a 4 KiB receive FIFO gets.
    #[test]
    fn configure_masks_each_bus_field_to_its_width() {
        let zero = configured(0, &THIRTY_TWO_BIT, &ODD, PHYSICAL_ADDRESS);
        let ones = configured(u32::MAX, &THIRTY_TWO_BIT, &ODD, PHYSICAL_ADDRESS);
        for (register, from_zero, from_ones) in [
            (
                registers::DIRECT_MEMORY_ACCESS_SYSTEM_BUS_MODE_REGISTER,
                0x0503_0010,
                0xf5f3_ffff,
            ),
            (
                registers::CHANNEL_CONTROL_REGISTER,
                0x0001_0000,
                0xffff_ffff,
            ),
            (
                registers::CHANNEL_TRANSMIT_CONTROL_REGISTER,
                0x0008_0010,
                0xffc8_ffff,
            ),
            (
                registers::CHANNEL_RECEIVE_CONTROL_REGISTER,
                0x0001_1000,
                0xffc1_9001,
            ),
            (
                registers::RECEIVE_QUEUE_0_OPERATION_MODE_REGISTER,
                0x00f0_c1a0,
                0xc0f0_c1ff,
            ),
            (
                registers::TRANSMIT_QUEUE_0_OPERATION_MODE_REGISTER,
                0x000f_000a,
                0xfe0f_fffb,
            ),
        ] {
            assert_eq!(
                zero.register(register),
                from_zero,
                "{register:#x} from zero"
            );
            assert_eq!(
                ones.register(register),
                from_ones,
                "{register:#x} from all ones"
            );
        }
    }

    /// Both queue modes in both modes from both starting states, at a FIFO big enough for flow
    /// control: threshold mode clears store-and-forward and sets the threshold, store-and-forward
    /// sets its bit, the size is `bytes / 256 - 1`, and receive flow control is enabled with
    /// RFD 3 and RFA 1 in place of whatever was there.
    #[test]
    fn the_queue_modes_from_zero_and_from_all_ones() {
        let store_and_forward = BusConfiguration {
            threshold_mode: false,
            ..BUS_CONFIGURATION
        };
        assert_eq!(receive_queue_mode(0, &BUS_CONFIGURATION, 2048), 0x0070_0018);
        assert_eq!(
            receive_queue_mode(u32::MAX, &BUS_CONFIGURATION, 2048),
            0xc07f_ffdf
        );
        assert_eq!(receive_queue_mode(0, &store_and_forward, 8192), 0x01f0_c1a0);
        assert_eq!(
            receive_queue_mode(u32::MAX, &store_and_forward, 8192),
            0xc1f0_c1ff
        );
        assert_eq!(
            transmit_queue_mode(0, &BUS_CONFIGURATION, 2048),
            0x0007_0078
        );
        assert_eq!(
            transmit_queue_mode(u32::MAX, &BUS_CONFIGURATION, 2048),
            0xfe07_fff9
        );
        assert_eq!(
            transmit_queue_mode(0, &store_and_forward, 8192),
            0x001f_000a
        );
        assert_eq!(
            transmit_queue_mode(u32::MAX, &store_and_forward, 8192),
            0xfe1f_fffb
        );
    }

    /// The MAC word from zero and from all ones: the speed and duplex bits are replaced, every
    /// bit `mac_configuration` sets is set whatever was there, and nothing else moves.
    #[test]
    fn the_mac_word_from_zero_and_from_all_ones() {
        let gigabit = Link {
            speed: Speed::Thousand,
            full_duplex: true,
        };
        let ten_half = Link {
            speed: Speed::Ten,
            full_duplex: false,
        };
        assert_eq!(mac_configuration(0, gigabit, false), 0x0006_2203);
        assert_eq!(mac_configuration(u32::MAX, gigabit, false), 0xffff_2fff);
        assert_eq!(mac_configuration(u32::MAX, ten_half, true), 0xffff_9fff);
        assert_eq!(
            mac_configuration(
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
        let gigabit = Link {
            speed: Speed::Thousand,
            full_duplex: true,
        };
        for (fill, set, cleared) in [(0, 1, 0), (u32::MAX, u32::MAX, 0xffff_fffe)] {
            let mut hardware = Seeded::new(fill);
            start(&mut hardware, gigabit, false);
            assert_eq!(
                hardware.register(registers::CHANNEL_RECEIVE_CONTROL_REGISTER),
                set
            );
            assert_eq!(
                hardware.register(registers::CHANNEL_TRANSMIT_CONTROL_REGISTER),
                set
            );
            let mut hardware = Seeded::new(fill);
            assert_eq!(stop(&mut hardware), Err(Error::FlushTimeout));
            assert_eq!(
                hardware.writes,
                [
                    (registers::MAC_CONFIGURATION_REGISTER, cleared),
                    (registers::CHANNEL_RECEIVE_CONTROL_REGISTER, cleared),
                    (registers::CHANNEL_TRANSMIT_CONTROL_REGISTER, cleared),
                    (
                        registers::TRANSMIT_QUEUE_0_OPERATION_MODE_REGISTER,
                        fill | 1
                    ),
                    (registers::MAC_CONFIGURATION_REGISTER, cleared & !2),
                    (registers::CHANNEL_INTERRUPT_ENABLE_REGISTER, 0),
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
        assert_eq!(
            hardware.writes,
            [(registers::DIRECT_MEMORY_ACCESS_BUS_MODE_REGISTER, u32::MAX)]
        );
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
            .direct_memory_access_address_bits()
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
    fn a_region_ending_at_4_gibibytes_is_in_reach() {
        let bytes = layout::DIRECT_MEMORY_ACCESS_REGION_BYTES;
        assert!(is_region_in_reach(0x1_0000_0000 - bytes, bytes, 32));
    }
}
