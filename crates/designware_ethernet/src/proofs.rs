use super::*;

/// **No word a device can write makes the driver read outside a buffer or past its caller's
/// space.** Word 3 of a receive descriptor is entirely the device's; this is every value of it.
///
/// Falsification: replayable `crates/designware_ethernet/falsifications/proofs.a_delivered_frame_always_fits_the_buffer_and_the_room.patch`
#[kani::proof]
fn a_delivered_frame_always_fits_the_buffer_and_the_room() {
    let des3: u32 = kani::any();
    let room: usize = kani::any();
    if let RxVerdict::Frame(n) = rx_verdict(des3, room) {
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
    assert!(layout::rx_descriptor(i) + DESCRIPTOR_BYTES <= layout::TX_RING);
    assert!(layout::tx_descriptor(i) + DESCRIPTOR_BYTES <= layout::RX_BUFFERS);
    assert!(layout::rx_buffer(i) + BUFFER_SIZE <= layout::TX_BUFFERS);
    assert!(layout::tx_buffer(i) + BUFFER_SIZE <= layout::BYTES);
}
