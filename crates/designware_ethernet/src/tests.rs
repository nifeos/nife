//! The crate's host tests, kept apart from `lib.rs` so the crate root stays an index of its
//! modules as the tests grow.

use std::vec;
use std::vec::Vec;

use super::*;
use crate::sim::{Memory, NetworkInterfaceController};

const PHYSICAL_ADDRESS: u64 = 0x4800_0000;

fn frame(length: usize, seed: u8) -> Vec<u8> {
    (0..length).map(|i| seed.wrapping_add(i as u8)).collect()
}

#[test]
fn start_hands_the_device_every_receive_buffer_but_one() {
    let mut nic =
        NetworkInterfaceController::new(PHYSICAL_ADDRESS, Memory::coherent(PHYSICAL_ADDRESS));
    nic.program_rings_and_start(PHYSICAL_ADDRESS);
    let mut data_plane = DataPlane::new(PHYSICAL_ADDRESS);
    nic.with(|r, t| data_plane.start(r, t));
    assert_eq!(
        nic.receive_tail(),
        (PHYSICAL_ADDRESS + layout::receive_descriptor(RECEIVE_ENTRIES - 1)) as u32
    );
    assert_eq!(nic.receive_available(), usize::from(RECEIVE_ENTRIES - 1));
    assert!(nic.barriers() >= 1);
}

#[test]
fn frames_arrive_in_order_through_more_than_one_lap_of_the_ring() {
    let mut nic =
        NetworkInterfaceController::new(PHYSICAL_ADDRESS, Memory::coherent(PHYSICAL_ADDRESS));
    nic.program_rings_and_start(PHYSICAL_ADDRESS);
    let mut data_plane = DataPlane::new(PHYSICAL_ADDRESS);
    nic.with(|r, t| data_plane.start(r, t));
    let mut out = vec![0u8; MAXIMUM_FRAME];
    for k in 0..(3 * RECEIVE_ENTRIES as usize) {
        let f = frame(60 + k, k as u8);
        assert!(nic.wire_in(&f), "the device had a buffer for frame {k}");
        let n = nic.with(|r, t| data_plane.receive(r, t, &mut out)).unwrap();
        assert_eq!(&out[..n], &f[..]);
    }
    assert_eq!(nic.with(|r, t| data_plane.receive(r, t, &mut out)), None);
    assert_eq!(data_plane.dropped, 0);
}

#[test]
fn a_ring_the_driver_does_not_drain_fills_to_one_short_and_then_refuses() {
    let mut nic =
        NetworkInterfaceController::new(PHYSICAL_ADDRESS, Memory::coherent(PHYSICAL_ADDRESS));
    nic.program_rings_and_start(PHYSICAL_ADDRESS);
    let mut data_plane = DataPlane::new(PHYSICAL_ADDRESS);
    nic.with(|r, t| data_plane.start(r, t));
    let accepted = (0..usize::from(RECEIVE_ENTRIES) + 4)
        .filter(|&k| nic.wire_in(&frame(64, k as u8)))
        .count();
    assert_eq!(accepted, usize::from(RECEIVE_ENTRIES - 1));
    // Draining one makes room for exactly one more.
    let mut out = vec![0u8; MAXIMUM_FRAME];
    assert!(
        nic.with(|r, t| data_plane.receive(r, t, &mut out))
            .is_some()
    );
    assert!(nic.wire_in(&frame(64, 0xee)));
    assert!(!nic.wire_in(&frame(64, 0xef)));
}

#[test]
fn a_bad_frame_is_dropped_and_counted_without_stalling_the_ring() {
    let mut nic =
        NetworkInterfaceController::new(PHYSICAL_ADDRESS, Memory::coherent(PHYSICAL_ADDRESS));
    nic.program_rings_and_start(PHYSICAL_ADDRESS);
    let mut data_plane = DataPlane::new(PHYSICAL_ADDRESS);
    nic.with(|r, t| data_plane.start(r, t));
    nic.wire_in_with_error(&frame(64, 1));
    let good = frame(80, 9);
    nic.wire_in(&good);
    let mut out = vec![0u8; MAXIMUM_FRAME];
    let n = nic.with(|r, t| data_plane.receive(r, t, &mut out)).unwrap();
    assert_eq!(&out[..n], &good[..]);
    assert_eq!(data_plane.dropped, 1);
}

#[test]
fn transmitted_frames_leave_in_order_and_the_ring_is_reclaimed() {
    let mut nic =
        NetworkInterfaceController::new(PHYSICAL_ADDRESS, Memory::coherent(PHYSICAL_ADDRESS));
    nic.program_rings_and_start(PHYSICAL_ADDRESS);
    let mut data_plane = DataPlane::new(PHYSICAL_ADDRESS);
    nic.with(|r, t| data_plane.start(r, t));
    let mut sent = Vec::new();
    for k in 0..(3 * TRANSMIT_ENTRIES as usize) {
        let f = frame(42 + k, k as u8);
        nic.with(|r, t| data_plane.transmit(r, t, &f)).unwrap();
        sent.push(f);
        nic.step();
    }
    assert_eq!(nic.wire_out(), sent);
}

#[test]
fn a_full_transmit_ring_refuses_rather_than_overwriting_an_owned_descriptor() {
    let mut nic =
        NetworkInterfaceController::new(PHYSICAL_ADDRESS, Memory::coherent(PHYSICAL_ADDRESS));
    nic.program_rings_and_start(PHYSICAL_ADDRESS);
    let mut data_plane = DataPlane::new(PHYSICAL_ADDRESS);
    nic.with(|r, t| data_plane.start(r, t));
    for k in 0..usize::from(TRANSMIT_ENTRIES - 1) {
        nic.with(|r, t| data_plane.transmit(r, t, &frame(60, k as u8)))
            .unwrap();
    }
    assert_eq!(
        nic.with(|r, t| data_plane.transmit(r, t, &frame(60, 0))),
        Err(TransmitError::Full)
    );
    nic.step();
    assert!(nic.with(|r, _| data_plane.can_transmit(r)));
    assert_eq!(nic.wire_out().len(), usize::from(TRANSMIT_ENTRIES - 1));
}

#[test]
fn a_frame_of_no_length_or_too_much_is_refused() {
    let mut nic =
        NetworkInterfaceController::new(PHYSICAL_ADDRESS, Memory::coherent(PHYSICAL_ADDRESS));
    let mut data_plane = DataPlane::new(PHYSICAL_ADDRESS);
    nic.with(|r, t| data_plane.start(r, t));
    assert_eq!(
        nic.with(|r, t| data_plane.transmit(r, t, &[])),
        Err(TransmitError::BadLength)
    );
    assert_eq!(
        nic.with(|r, t| data_plane.transmit(r, t, &[0; MAXIMUM_FRAME + 1])),
        Err(TransmitError::BadLength)
    );
}

#[test]
fn the_verdict_strips_the_crc_and_refuses_what_the_device_could_not_mean() {
    let ok = RDES3_FD | RDES3_LD;
    assert_eq!(
        receive_verdict(ok | 64, MAXIMUM_FRAME),
        ReceiveVerdict::Frame(60)
    );
    assert_eq!(
        receive_verdict(ok | 1518, MAXIMUM_FRAME),
        ReceiveVerdict::Frame(1514)
    );
    assert_eq!(
        receive_verdict(ok | 1519, MAXIMUM_FRAME),
        ReceiveVerdict::Drop
    );
    assert_eq!(receive_verdict(ok | 3, MAXIMUM_FRAME), ReceiveVerdict::Drop);
    assert_eq!(receive_verdict(ok | 64, 59), ReceiveVerdict::Drop);
    assert_eq!(
        receive_verdict(RDES3_FD | 64, MAXIMUM_FRAME),
        ReceiveVerdict::Drop
    );
    assert_eq!(
        receive_verdict(ok | RDES3_ES | 64, MAXIMUM_FRAME),
        ReceiveVerdict::Drop
    );
    assert_eq!(
        receive_verdict(ok | RDES3_CTXT | 64, MAXIMUM_FRAME),
        ReceiveVerdict::Drop
    );
}

#[test]
fn descriptor_words_put_ownership_in_the_high_word_of_the_second_store() {
    let [a, b] = receive_descriptor_words(0x1_2345_6000);
    assert_eq!(a, 0x1_2345_6000);
    assert_eq!((b >> 32) as u32, RDES3_OWN | RDES3_IOC | RDES3_BUF1V);
    let [a, b] = transmit_descriptor_words(0x4000_0800, 60);
    assert_eq!(a, 0x4000_0800);
    assert_eq!(b as u32, 60 | TDES2_IC);
    assert_eq!((b >> 32) as u32, TDES3_OWN | TDES3_FS | TDES3_LS | 60);
}

/// **The region's layout, in bytes**: two pages of rings, sixteen receive buffers, sixteen
/// transmit buffers. The kernel allocates `layout::PAGES` from this, so a slip here maps a
/// region too small for the buffers the rings point at.
#[test]
fn the_region_is_two_ring_pages_then_the_buffers() {
    assert_eq!(layout::RECEIVE_BUFFERS, 0x2000);
    assert_eq!(layout::TRANSMIT_BUFFERS, 0xa000);
    assert_eq!(layout::BYTES, 0x1_2000);
    assert_eq!(layout::receive_buffer(1), 0x2800);
    assert_eq!(
        layout::receive_buffer(RECEIVE_ENTRIES + 1),
        0x2800,
        "reduced mod the ring"
    );
    assert_eq!(layout::transmit_buffer(1), 0xa800);
    assert_eq!(
        layout::transmit_buffer(TRANSMIT_ENTRIES + 1),
        0xa800,
        "reduced mod the ring"
    );
}

/// The descriptor bits as the databook places them, written out: the driver and the simulated
/// device both read these names, so only a literal tells a misplaced bit from a right one.
#[test]
fn the_descriptor_bits_are_where_the_databook_puts_them() {
    assert_eq!(
        [
            RDES3_OWN,
            RDES3_IOC,
            RDES3_BUF1V,
            RDES3_CTXT,
            RDES3_FD,
            RDES3_LD,
            RDES3_ES
        ],
        [
            1 << 31,
            0x4000_0000,
            0x0100_0000,
            0x4000_0000,
            0x2000_0000,
            0x1000_0000,
            0x8000
        ]
    );
    assert_eq!(
        [TDES2_IC, TDES3_FS, TDES3_LS, TDES3_OWN, TDES3_ES],
        [0x8000_0000, 0x2000_0000, 0x1000_0000, 0x8000_0000, 0x8000]
    );
}

/// The held-back slot is truly held back: descriptor 15 is not the device's after `start`.
/// The tail alone would not show it, since the simulated device stops at the tail either way.
/// And a transmit ring a previous run left owned is cleared whole, so the first reclaim does
/// not wait on a frame nobody queued.
#[test]
fn start_holds_the_last_receive_slot_back_and_clears_a_stale_transmit_ring() {
    let mut nic =
        NetworkInterfaceController::new(PHYSICAL_ADDRESS, Memory::coherent(PHYSICAL_ADDRESS));
    nic.with(|r, _| {
        for i in 0..TRANSMIT_ENTRIES {
            r.write_u64(layout::transmit_descriptor(i), u64::MAX);
            r.write_u64(layout::transmit_descriptor(i) + 8, u64::MAX);
        }
    });
    let mut data_plane = DataPlane::new(PHYSICAL_ADDRESS);
    nic.with(|r, t| data_plane.start(r, t));
    nic.with(|r, _| {
        assert_eq!(
            r.read_u64(layout::receive_descriptor(RECEIVE_ENTRIES - 1) + 8),
            0
        );
        assert_ne!(
            r.read_u64(layout::receive_descriptor(RECEIVE_ENTRIES - 2) + 8),
            0
        );
        for i in 0..TRANSMIT_ENTRIES {
            assert_eq!(r.read_u64(layout::transmit_descriptor(i)), 0, "{i}");
            assert_eq!(r.read_u64(layout::transmit_descriptor(i) + 8), 0, "{i}");
        }
    });
}

/// The receive cursor counts frames taken and wraps with the ring, so it always names a slot.
#[test]
fn the_receive_cursor_names_the_next_slot_and_wraps() {
    let mut nic =
        NetworkInterfaceController::new(PHYSICAL_ADDRESS, Memory::coherent(PHYSICAL_ADDRESS));
    nic.program_rings_and_start(PHYSICAL_ADDRESS);
    let mut data_plane = DataPlane::new(PHYSICAL_ADDRESS);
    nic.with(|r, t| data_plane.start(r, t));
    let mut out = vec![0u8; MAXIMUM_FRAME];
    for k in 1..=usize::from(RECEIVE_ENTRIES) + 2 {
        assert!(nic.wire_in(&frame(64, k as u8)));
        nic.with(|r, t| data_plane.receive(r, t, &mut out)).unwrap();
        assert_eq!(
            usize::from(data_plane.receive_cursor()),
            k % usize::from(RECEIVE_ENTRIES)
        );
    }
}

/// A frame of exactly the largest size is carried, and `can_transmit` tells a ring with room
/// from a full one before anything is refused.
#[test]
fn the_largest_frame_is_sent_and_a_full_ring_says_so_in_advance() {
    let mut nic =
        NetworkInterfaceController::new(PHYSICAL_ADDRESS, Memory::coherent(PHYSICAL_ADDRESS));
    nic.program_rings_and_start(PHYSICAL_ADDRESS);
    let mut data_plane = DataPlane::new(PHYSICAL_ADDRESS);
    nic.with(|r, t| data_plane.start(r, t));
    assert!(nic.with(|r, _| data_plane.can_transmit(r)));
    nic.with(|r, t| data_plane.transmit(r, t, &frame(MAXIMUM_FRAME, 1)))
        .unwrap();
    for k in 1..usize::from(TRANSMIT_ENTRIES - 1) {
        assert!(nic.with(|r, _| data_plane.can_transmit(r)), "{k}");
        nic.with(|r, t| data_plane.transmit(r, t, &frame(60, k as u8)))
            .unwrap();
    }
    assert!(!nic.with(|r, _| data_plane.can_transmit(r)));
    nic.step();
    assert!(nic.with(|r, _| data_plane.can_transmit(r)));
    assert_eq!(nic.wire_out()[0].len(), MAXIMUM_FRAME);
}
