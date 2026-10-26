use super::*;

/// **No word a device can write makes the driver read outside a buffer or past its caller's
/// space.** The descriptor's second word is entirely the device's; this is every value of it.
///
/// Falsification: replayable `crates/e1000e/falsifications/proofs.a_delivered_frame_always_fits_the_buffer_and_the_room.patch`
#[kani::proof]
fn a_delivered_frame_always_fits_the_buffer_and_the_room() {
    let word: u64 = kani::any();
    let room: usize = kani::any();
    if let ReceiveVerdict::Frame(n) = receive_verdict(word, room) {
        assert!(n <= room);
        assert!(n as u64 <= BUFFER_SIZE);
        assert!(n <= MAX_FRAME);
    }
}

/// **Every descriptor and buffer offset, for any index, lies inside the region.**
///
/// Falsification: replayable `crates/e1000e/falsifications/proofs.every_ring_offset_is_inside_the_region.patch`
#[kani::proof]
fn every_ring_offset_is_inside_the_region() {
    let i: u16 = kani::any();
    assert!(layout::receive_descriptor(i) + 16 <= layout::TRANSMIT_RING);
    assert!(layout::transmit_descriptor(i) + 16 <= layout::RECEIVE_BUFFERS);
    assert!(layout::receive_buffer(i) + BUFFER_SIZE <= layout::TRANSMIT_BUFFERS);
    assert!(layout::transmit_buffer(i) + BUFFER_SIZE <= layout::BYTES);
}
