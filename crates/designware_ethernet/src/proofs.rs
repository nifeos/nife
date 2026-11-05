//! **The Kani proofs**: claims that hold for every value a device or a register can present.
//!
//! Name: ratified 2026-11-03 (UTC), #3122.

use super::*;

/// **No word a device can write makes the driver read outside a buffer or past its caller's
/// space.** Word 3 of a receive descriptor is entirely the device's; this is every value of it.
///
/// Falsification: replayable `crates/designware_ethernet/falsifications/proofs.a_delivered_frame_always_fits_the_buffer_and_the_room.patch`
#[kani::proof]
fn a_delivered_frame_always_fits_the_buffer_and_the_room() {
    let descriptor_word_3: u32 = kani::any();
    let room: usize = kani::any();
    if let ReceiveVerdict::Frame(n) = receive_verdict(descriptor_word_3, room) {
        assert!(n <= room);
        assert!(n <= MAXIMUM_FRAME_BYTES);
        assert!(n as u64 + CRC_BYTES as u64 <= BUFFER_BYTES);
    }
}

/// **Every descriptor and buffer offset, for any index, lies inside the region.**
///
/// Falsification: replayable `crates/designware_ethernet/falsifications/proofs.every_ring_offset_is_inside_the_region.patch`
#[kani::proof]
fn every_ring_offset_is_inside_the_region() {
    let i: u16 = kani::any();
    assert!(layout::receive_descriptor(i) + DESCRIPTOR_BYTES <= layout::TRANSMIT_RING);
    assert!(layout::transmit_descriptor(i) + DESCRIPTOR_BYTES <= layout::RECEIVE_BUFFERS);
    assert!(layout::receive_buffer(i) + BUFFER_BYTES <= layout::TRANSMIT_BUFFERS);
    assert!(layout::transmit_buffer(i) + BUFFER_BYTES <= layout::DIRECT_MEMORY_ACCESS_REGION_BYTES);
}

/// **Whatever the PHY answers, the link decode returns a link only the registers justify.** All
/// six registers are the device's, so each is every value. A link needs the status bit up and the
/// PHY not isolated; with autonegotiation on it also needs autonegotiation complete, and the
/// speed and duplex are abilities both ends advertise; with it off they are the forced bits. The
/// decode also ends, with no panic, on every combination, which Kani checks of every function it
/// runs.
///
/// Falsification: replayable `crates/designware_ethernet/falsifications/proofs.a_link_is_decoded_only_from_what_the_registers_say.patch`
#[kani::proof]
fn a_link_is_decoded_only_from_what_the_registers_say() {
    use management_data_input_output as mdio;
    let bmsr: u16 = kani::any();
    let bmcr: u16 = kani::any();
    let anar: u16 = kani::any();
    let anlpar: u16 = kani::any();
    let gtcr: u16 = kani::any();
    let gtsr: u16 = kani::any();
    if let Some(link) =
        motorcomm_physical_layer_device::link_from_registers(bmsr, bmcr, anar, anlpar, gtcr, gtsr)
    {
        assert!(bmsr & mdio::BMSR_LSTATUS != 0);
        assert!(bmcr & mdio::BMCR_ISO == 0);
        if bmcr & mdio::BMCR_ANENABLE == 0 {
            assert!(link.full_duplex == (bmcr & mdio::BMCR_FDX != 0));
        } else {
            assert!(bmsr & mdio::BASIC_MODE_STATUS_AUTONEGOTIATION_COMPLETE != 0);
            let common = anar & anlpar;
            let gigabit_full =
                gtcr & mdio::GTCR_ADV_1000TFDX != 0 && gtsr & mdio::GTSR_LP_1000TFDX != 0;
            let gigabit_half =
                gtcr & mdio::GTCR_ADV_1000THDX != 0 && gtsr & mdio::GTSR_LP_1000THDX != 0;
            match link.speed {
                motorcomm_physical_layer_device::Speed::Thousand => {
                    assert!(gigabit_full || gigabit_half);
                    assert!(link.full_duplex == gigabit_full);
                }
                motorcomm_physical_layer_device::Speed::Hundred => {
                    assert!(!gigabit_full && !gigabit_half);
                    assert!(common & (mdio::ANLPAR_TX_FD | mdio::ANLPAR_TX | mdio::ANLPAR_T4) != 0);
                }
                motorcomm_physical_layer_device::Speed::Ten => {
                    assert!(!gigabit_full && !gigabit_half);
                    assert!(common & (mdio::ANLPAR_TX_FD | mdio::ANLPAR_TX | mdio::ANLPAR_T4) == 0);
                    assert!(common & (mdio::ANLPAR_10_FD | mdio::ANLPAR_10) != 0);
                }
            }
        }
    }
}

/// **A transmit descriptor carries the buffer address untouched and a length that fits both of its
/// fields, for every address and every length.** The device owns it, it is the first and last
/// segment, word 3's length is the frame length masked to its 15 bits, and word 2's buffer length
/// is the same length masked to its 14 bits. The length never reaches the flag bits above it.
///
/// Falsification: replayable `crates/designware_ethernet/falsifications/proofs.a_transmit_descriptor_keeps_its_length_inside_its_fields.patch`
#[kani::proof]
fn a_transmit_descriptor_keeps_its_length_inside_its_fields() {
    let address: u64 = kani::any();
    let length: u16 = kani::any();
    let [low, high] = transmit_descriptor_words(address, length);
    let word_2 = high as u32;
    let word_3 = (high >> 32) as u32;
    assert!(low == address);
    let frame_length = length as u32 & TRANSMIT_DESCRIPTOR_WORD_3_FRAME_LENGTH_MASK;
    assert!(word_3 & TRANSMIT_DESCRIPTOR_WORD_3_FRAME_LENGTH_MASK == frame_length);
    assert!(word_3 & TRANSMIT_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE != 0);
    assert!(word_3 & TRANSMIT_DESCRIPTOR_WORD_3_FIRST_SEGMENT != 0);
    assert!(word_3 & TRANSMIT_DESCRIPTOR_WORD_3_LAST_SEGMENT != 0);
    assert!(
        word_3
            & !(TRANSMIT_DESCRIPTOR_WORD_3_FRAME_LENGTH_MASK
                | TRANSMIT_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE
                | TRANSMIT_DESCRIPTOR_WORD_3_FIRST_SEGMENT
                | TRANSMIT_DESCRIPTOR_WORD_3_LAST_SEGMENT)
            == 0
    );
    assert!(
        word_2 & TRANSMIT_DESCRIPTOR_WORD_2_BUFFER_1_LENGTH_MASK
            == frame_length & TRANSMIT_DESCRIPTOR_WORD_2_BUFFER_1_LENGTH_MASK
    );
    assert!(word_2 & TRANSMIT_DESCRIPTOR_WORD_2_INTERRUPT_ON_COMPLETION != 0);
}

/// **An MDIO command word holds its two addresses in their own five-bit fields, whatever address
/// the caller passes.** An address above 31 is reduced and never spills into the neighbouring
/// field, the operation bits say read or write as asked, and the busy bit that starts the command
/// is set.
///
/// Falsification: replayable `crates/designware_ethernet/falsifications/proofs.an_mdio_command_word_keeps_each_address_in_its_own_field.patch`
#[kani::proof]
fn an_mdio_command_word_keeps_each_address_in_its_own_field() {
    use management_data_input_output as mdio;
    let device: u8 = kani::any();
    let register: u8 = kani::any();
    let write: bool = kani::any();
    let word = mdio::command_word(device, register, write);
    assert!((word >> mdio::PHYSICAL_LAYER_DEVICE_ADDRESS_SHIFT) & 0x1f == (device & 0x1f) as u32);
    assert!((word >> mdio::REGISTER_ADDRESS_SHIFT) & 0x1f == (register & 0x1f) as u32);
    assert!((word >> mdio::CLOCK_RANGE_SHIFT) & 0xf == mdio::CLOCK_RANGE_500_TO_800_MEGAHERTZ);
    assert!(word & mdio::COMMAND_BUSY != 0);
    let operation = word & (0x3 << 2);
    if write {
        assert!(operation == mdio::OPERATION_COMMAND_WRITE);
    } else {
        assert!(operation == mdio::OPERATION_COMMAND_READ);
    }
    assert!(word >> 26 == 0);
}

/// **The coherence verdict says "coherent" only when every account agrees, and a bus error is never
/// called anything else.** From any observation: a fatal bus error in the status word gives the bus
/// error verdict; `Coherent` needs the receive descriptor handed back, the payload to match the
/// probe frame byte for byte, and the delivered length to be the probe's.
///
/// Falsification: replayable `crates/designware_ethernet/falsifications/proofs.a_coherent_verdict_needs_every_account_to_agree.patch`
#[kani::proof]
fn a_coherent_verdict_needs_every_account_to_agree() {
    use cache_coherence::{classify, Observation, Payload, Verdict};
    let payload = match kani::any::<u8>() % 3 {
        0 => Payload::Matches,
        1 => Payload::Poison,
        _ => Payload::Other,
    };
    let observation = Observation {
        receive_descriptor_word_3: kani::any(),
        transmit_descriptor_word_3: kani::any(),
        status: kani::any(),
        payload,
    };
    let verdict = classify(&observation);
    if observation.status & registers::CHAN_STATUS_FBE != 0 {
        assert!(verdict == Verdict::BusError);
    }
    if verdict == Verdict::Coherent {
        assert!(observation.status & registers::CHAN_STATUS_FBE == 0);
        assert!(
            observation.receive_descriptor_word_3 & RECEIVE_DESCRIPTOR_WORD_3_OWNED_BY_DEVICE == 0
        );
        assert!(observation.payload == Payload::Matches);
        assert!(
            receive_verdict(
                observation.receive_descriptor_word_3,
                cache_coherence::PROBE_LENGTH
            ) == ReceiveVerdict::Frame(cache_coherence::PROBE_LENGTH)
        );
    }
}
