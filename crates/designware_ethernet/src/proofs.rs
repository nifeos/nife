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
        assert!(n <= MAXIMUM_FRAME);
        assert!(n as u64 + CRC_BYTES as u64 <= BUFFER_SIZE);
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
    assert!(layout::receive_buffer(i) + BUFFER_SIZE <= layout::TRANSMIT_BUFFERS);
    assert!(layout::transmit_buffer(i) + BUFFER_SIZE <= layout::DIRECT_MEMORY_ACCESS_REGION_BYTES);
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
