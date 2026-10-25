//! The crate's host tests, kept apart from `lib.rs` so the crate root stays an index of its
//! modules as the tests grow.

use std::vec;
use std::vec::Vec;

use super::*;
use crate::sim::{Memory, Nic};

const PHYS: u64 = 0x4800_0000;

fn frame(len: usize, seed: u8) -> Vec<u8> {
    (0..len).map(|i| seed.wrapping_add(i as u8)).collect()
}

#[test]
fn start_hands_the_device_every_receive_buffer_but_one() {
    let mut nic = Nic::new(PHYS, Memory::coherent(PHYS));
    nic.program_rings_and_start(PHYS);
    let mut dp = DataPlane::new(PHYS);
    nic.with(|r, t| dp.start(r, t));
    assert_eq!(
        nic.rx_tail(),
        (PHYS + layout::rx_descriptor(RX_ENTRIES - 1)) as u32
    );
    assert_eq!(nic.rx_available(), usize::from(RX_ENTRIES - 1));
    assert!(nic.barriers() >= 1);
}

#[test]
fn frames_arrive_in_order_through_more_than_one_lap_of_the_ring() {
    let mut nic = Nic::new(PHYS, Memory::coherent(PHYS));
    nic.program_rings_and_start(PHYS);
    let mut dp = DataPlane::new(PHYS);
    nic.with(|r, t| dp.start(r, t));
    let mut out = vec![0u8; MAX_FRAME];
    for k in 0..(3 * RX_ENTRIES as usize) {
        let f = frame(60 + k, k as u8);
        assert!(nic.wire_in(&f), "the device had a buffer for frame {k}");
        let n = nic.with(|r, t| dp.receive(r, t, &mut out)).unwrap();
        assert_eq!(&out[..n], &f[..]);
    }
    assert_eq!(nic.with(|r, t| dp.receive(r, t, &mut out)), None);
    assert_eq!(dp.dropped, 0);
}

#[test]
fn a_ring_the_driver_does_not_drain_fills_to_one_short_and_then_refuses() {
    let mut nic = Nic::new(PHYS, Memory::coherent(PHYS));
    nic.program_rings_and_start(PHYS);
    let mut dp = DataPlane::new(PHYS);
    nic.with(|r, t| dp.start(r, t));
    let accepted = (0..usize::from(RX_ENTRIES) + 4)
        .filter(|&k| nic.wire_in(&frame(64, k as u8)))
        .count();
    assert_eq!(accepted, usize::from(RX_ENTRIES - 1));
    // Draining one makes room for exactly one more.
    let mut out = vec![0u8; MAX_FRAME];
    assert!(nic.with(|r, t| dp.receive(r, t, &mut out)).is_some());
    assert!(nic.wire_in(&frame(64, 0xee)));
    assert!(!nic.wire_in(&frame(64, 0xef)));
}

#[test]
fn a_bad_frame_is_dropped_and_counted_without_stalling_the_ring() {
    let mut nic = Nic::new(PHYS, Memory::coherent(PHYS));
    nic.program_rings_and_start(PHYS);
    let mut dp = DataPlane::new(PHYS);
    nic.with(|r, t| dp.start(r, t));
    nic.wire_in_with_error(&frame(64, 1));
    let good = frame(80, 9);
    nic.wire_in(&good);
    let mut out = vec![0u8; MAX_FRAME];
    let n = nic.with(|r, t| dp.receive(r, t, &mut out)).unwrap();
    assert_eq!(&out[..n], &good[..]);
    assert_eq!(dp.dropped, 1);
}

#[test]
fn transmitted_frames_leave_in_order_and_the_ring_is_reclaimed() {
    let mut nic = Nic::new(PHYS, Memory::coherent(PHYS));
    nic.program_rings_and_start(PHYS);
    let mut dp = DataPlane::new(PHYS);
    nic.with(|r, t| dp.start(r, t));
    let mut sent = Vec::new();
    for k in 0..(3 * TX_ENTRIES as usize) {
        let f = frame(42 + k, k as u8);
        nic.with(|r, t| dp.transmit(r, t, &f)).unwrap();
        sent.push(f);
        nic.step();
    }
    assert_eq!(nic.wire_out(), sent);
}

#[test]
fn a_full_transmit_ring_refuses_rather_than_overwriting_an_owned_descriptor() {
    let mut nic = Nic::new(PHYS, Memory::coherent(PHYS));
    nic.program_rings_and_start(PHYS);
    let mut dp = DataPlane::new(PHYS);
    nic.with(|r, t| dp.start(r, t));
    for k in 0..usize::from(TX_ENTRIES - 1) {
        nic.with(|r, t| dp.transmit(r, t, &frame(60, k as u8)))
            .unwrap();
    }
    assert_eq!(
        nic.with(|r, t| dp.transmit(r, t, &frame(60, 0))),
        Err(TxError::Full)
    );
    nic.step();
    assert!(nic.with(|r, _| dp.can_transmit(r)));
    assert_eq!(nic.wire_out().len(), usize::from(TX_ENTRIES - 1));
}

#[test]
fn a_frame_of_no_length_or_too_much_is_refused() {
    let mut nic = Nic::new(PHYS, Memory::coherent(PHYS));
    let mut dp = DataPlane::new(PHYS);
    nic.with(|r, t| dp.start(r, t));
    assert_eq!(
        nic.with(|r, t| dp.transmit(r, t, &[])),
        Err(TxError::BadLength)
    );
    assert_eq!(
        nic.with(|r, t| dp.transmit(r, t, &[0; MAX_FRAME + 1])),
        Err(TxError::BadLength)
    );
}

#[test]
fn the_verdict_strips_the_crc_and_refuses_what_the_device_could_not_mean() {
    let ok = RDES3_FD | RDES3_LD;
    assert_eq!(rx_verdict(ok | 64, MAX_FRAME), RxVerdict::Frame(60));
    assert_eq!(rx_verdict(ok | 1518, MAX_FRAME), RxVerdict::Frame(1514));
    assert_eq!(rx_verdict(ok | 1519, MAX_FRAME), RxVerdict::Drop);
    assert_eq!(rx_verdict(ok | 3, MAX_FRAME), RxVerdict::Drop);
    assert_eq!(rx_verdict(ok | 64, 59), RxVerdict::Drop);
    assert_eq!(rx_verdict(RDES3_FD | 64, MAX_FRAME), RxVerdict::Drop);
    assert_eq!(rx_verdict(ok | RDES3_ES | 64, MAX_FRAME), RxVerdict::Drop);
    assert_eq!(rx_verdict(ok | RDES3_CTXT | 64, MAX_FRAME), RxVerdict::Drop);
}

#[test]
fn descriptor_words_put_ownership_in_the_high_word_of_the_second_store() {
    let [a, b] = rx_descriptor_words(0x1_2345_6000);
    assert_eq!(a, 0x1_2345_6000);
    assert_eq!((b >> 32) as u32, RDES3_OWN | RDES3_IOC | RDES3_BUF1V);
    let [a, b] = tx_descriptor_words(0x4000_0800, 60);
    assert_eq!(a, 0x4000_0800);
    assert_eq!(b as u32, 60 | TDES2_IC);
    assert_eq!((b >> 32) as u32, TDES3_OWN | TDES3_FS | TDES3_LS | 60);
}

#[test]
fn the_handoff_refuses_a_multicast_or_absent_address() {
    let h = Handoff {
        mac: [0x01, 0, 0, 0, 0, 1],
        data_plane_phys: 0x4000_0000,
    };
    let [r, p] = h.pack();
    assert_eq!(Handoff::unpack(r, p), None);
    let [r, p] = Handoff { mac: [0; 6], ..h }.pack();
    assert_eq!(Handoff::unpack(r, p), None);
    assert!(!Handoff::is_role(0xe1e0));
}

/// A handoff survives its own packing, and each refusal holds on its own: a role word that is
/// not this server's is refused with a good base beside it, as is a good role word with no
/// base or a base off a page boundary.
#[test]
fn the_handoff_round_trips_and_each_refusal_stands_alone() {
    let h = Handoff {
        mac: [0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c],
        data_plane_phys: 0x4800_0000,
    };
    let [r, p] = h.pack();
    assert!(Handoff::is_role(r));
    assert_eq!(Handoff::unpack(r, p), Some(h));
    assert_eq!(Handoff::unpack(r ^ 1, p), None, "not this server's role");
    assert_eq!(Handoff::unpack(r, 0), None, "no base");
    assert_eq!(Handoff::unpack(r, p + 8), None, "not page aligned");
}

/// **The region's layout, in bytes**: two pages of rings, sixteen receive buffers, sixteen
/// transmit buffers. The kernel allocates `layout::PAGES` from this, so a slip here maps a
/// region too small for the buffers the rings point at.
#[test]
fn the_region_is_two_ring_pages_then_the_buffers() {
    assert_eq!(layout::RX_BUFFERS, 0x2000);
    assert_eq!(layout::TX_BUFFERS, 0xa000);
    assert_eq!(layout::BYTES, 0x1_2000);
    assert_eq!(layout::rx_buffer(1), 0x2800);
    assert_eq!(
        layout::rx_buffer(RX_ENTRIES + 1),
        0x2800,
        "reduced mod the ring"
    );
    assert_eq!(layout::tx_buffer(1), 0xa800);
    assert_eq!(
        layout::tx_buffer(TX_ENTRIES + 1),
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
    let mut nic = Nic::new(PHYS, Memory::coherent(PHYS));
    nic.with(|r, _| {
        for i in 0..TX_ENTRIES {
            r.write_u64(layout::tx_descriptor(i), u64::MAX);
            r.write_u64(layout::tx_descriptor(i) + 8, u64::MAX);
        }
    });
    let mut dp = DataPlane::new(PHYS);
    nic.with(|r, t| dp.start(r, t));
    nic.with(|r, _| {
        assert_eq!(r.read_u64(layout::rx_descriptor(RX_ENTRIES - 1) + 8), 0);
        assert_ne!(r.read_u64(layout::rx_descriptor(RX_ENTRIES - 2) + 8), 0);
        for i in 0..TX_ENTRIES {
            assert_eq!(r.read_u64(layout::tx_descriptor(i)), 0, "{i}");
            assert_eq!(r.read_u64(layout::tx_descriptor(i) + 8), 0, "{i}");
        }
    });
}

/// The receive cursor counts frames taken and wraps with the ring, so it always names a slot.
#[test]
fn the_receive_cursor_names_the_next_slot_and_wraps() {
    let mut nic = Nic::new(PHYS, Memory::coherent(PHYS));
    nic.program_rings_and_start(PHYS);
    let mut dp = DataPlane::new(PHYS);
    nic.with(|r, t| dp.start(r, t));
    let mut out = vec![0u8; MAX_FRAME];
    for k in 1..=usize::from(RX_ENTRIES) + 2 {
        assert!(nic.wire_in(&frame(64, k as u8)));
        nic.with(|r, t| dp.receive(r, t, &mut out)).unwrap();
        assert_eq!(usize::from(dp.rx_cursor()), k % usize::from(RX_ENTRIES));
    }
}

/// A frame of exactly the largest size is carried, and `can_transmit` tells a ring with room
/// from a full one before anything is refused.
#[test]
fn the_largest_frame_is_sent_and_a_full_ring_says_so_in_advance() {
    let mut nic = Nic::new(PHYS, Memory::coherent(PHYS));
    nic.program_rings_and_start(PHYS);
    let mut dp = DataPlane::new(PHYS);
    nic.with(|r, t| dp.start(r, t));
    assert!(nic.with(|r, _| dp.can_transmit(r)));
    nic.with(|r, t| dp.transmit(r, t, &frame(MAX_FRAME, 1)))
        .unwrap();
    for k in 1..usize::from(TX_ENTRIES - 1) {
        assert!(nic.with(|r, _| dp.can_transmit(r)), "{k}");
        nic.with(|r, t| dp.transmit(r, t, &frame(60, k as u8)))
            .unwrap();
    }
    assert!(!nic.with(|r, _| dp.can_transmit(r)));
    nic.step();
    assert!(nic.with(|r, _| dp.can_transmit(r)));
    assert_eq!(nic.wire_out()[0].len(), MAX_FRAME);
}
