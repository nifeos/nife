//! The crate's host tests.

use std::vec;
use std::vec::Vec;

use super::*;

const PHYS: u64 = 0x8000_0000;

/// A simulated 82574L: the region as bytes, the tails as the driver wrote them, and a device
/// side that consumes posted receive descriptors and completed transmit ones the way the
/// hardware does (head chases tail).
struct Sim {
    mem: Vec<u8>,
    rdt: u32,
    tdt: u32,
    rdh: u16,
    tdh: u16,
    barriers: usize,
    sent: Vec<Vec<u8>>,
}

impl Region for Sim {
    fn read_u64(&self, off: u64) -> u64 {
        let o = off as usize;
        u64::from_le_bytes(self.mem[o..o + 8].try_into().unwrap())
    }
    fn write_u64(&mut self, off: u64, v: u64) {
        let o = off as usize;
        self.mem[o..o + 8].copy_from_slice(&v.to_le_bytes());
    }
    fn read_bytes(&self, off: u64, out: &mut [u8]) {
        let o = off as usize;
        out.copy_from_slice(&self.mem[o..o + out.len()]);
    }
    fn write_bytes(&mut self, off: u64, src: &[u8]) {
        let o = off as usize;
        self.mem[o..o + src.len()].copy_from_slice(src);
    }
}

struct Regs<'a>(&'a mut u32, &'a mut u32, &'a mut usize);
impl Tails for Regs<'_> {
    fn set_rx_tail(&mut self, v: u32) {
        *self.0 = v;
    }
    fn set_tx_tail(&mut self, v: u32) {
        *self.1 = v;
    }
    fn barrier(&mut self) {
        *self.2 += 1;
    }
}

impl Sim {
    fn new() -> Sim {
        Sim {
            mem: vec![0xa5; layout::BYTES as usize],
            rdt: 0,
            tdt: 0,
            rdh: 0,
            tdh: 0,
            barriers: 0,
            sent: Vec::new(),
        }
    }
    fn with<R>(&mut self, f: impl FnOnce(&mut Sim, &mut Regs<'_>) -> R) -> R {
        let (mut rdt, mut tdt, mut b) = (self.rdt, self.tdt, self.barriers);
        let r = f(self, &mut Regs(&mut rdt, &mut tdt, &mut b));
        (self.rdt, self.tdt, self.barriers) = (rdt, tdt, b);
        r
    }
    /// The device receives `frame` off the wire with `status` and `errors`, if it owns a
    /// descriptor; false when the ring is full (head has caught the tail).
    fn deliver(&mut self, frame: &[u8], len: u16, status: u8, errors: u8) -> bool {
        if u32::from(self.rdh) == self.rdt {
            return false;
        }
        let d = layout::rx_descriptor(self.rdh);
        let addr = self.read_u64(d);
        let off = addr - PHYS;
        assert_eq!(
            off,
            layout::rx_buffer(self.rdh),
            "a descriptor names a foreign buffer"
        );
        self.write_bytes(off, frame);
        self.write_u64(
            d + 8,
            len as u64 | (status as u64) << 32 | (errors as u64) << 40,
        );
        self.rdh = (self.rdh + 1) % RX_ENTRIES;
        true
    }
    /// The device sends every descriptor up to the tail and writes `DD` back.
    fn drain_tx(&mut self) {
        while u32::from(self.tdh) != self.tdt {
            let d = layout::tx_descriptor(self.tdh);
            let addr = self.read_u64(d);
            let word = self.read_u64(d + 8);
            assert_eq!((word >> 24) as u8, TX_COMMAND);
            let len = (word & 0xffff) as usize;
            let mut f = vec![0; len];
            self.read_bytes(addr - PHYS, &mut f);
            self.sent.push(f);
            self.write_u64(d + 8, word | (TX_STATUS_DD as u64) << 32);
            self.tdh = (self.tdh + 1) % TX_ENTRIES;
        }
    }
}

fn started() -> (Sim, DataPlane) {
    let mut sim = Sim::new();
    let mut dp = DataPlane::new(PHYS);
    sim.with(|s, r| dp.start(s, r));
    (sim, dp)
}

fn frame(tag: u8, len: usize) -> Vec<u8> {
    (0..len).map(|k| tag ^ k as u8).collect()
}

const GOOD: u8 = RX_STATUS_DD | RX_STATUS_EOP;

#[test]
fn start_posts_every_receive_buffer_but_one_and_names_only_its_own_buffers() {
    let (sim, _) = started();
    assert_eq!(sim.rdt, u32::from(RX_ENTRIES - 1));
    for i in 0..RX_ENTRIES {
        let d = layout::rx_descriptor(i);
        assert_eq!(sim.read_u64(d), PHYS + layout::rx_buffer(i));
        assert_eq!(
            sim.read_u64(d + 8),
            0,
            "a stale status would read as a frame"
        );
    }
    assert!(
        sim.barriers >= 1,
        "the tail was written with no barrier before it"
    );
}

#[test]
fn frames_arrive_in_order_across_many_laps_of_the_ring() {
    let (mut sim, mut dp) = started();
    let mut out = [0u8; MAX_FRAME];
    for n in 0..5 * RX_ENTRIES as usize {
        let f = frame(n as u8, 60 + n % 1400);
        assert!(
            sim.deliver(&f, f.len() as u16, GOOD, 0),
            "ring full at frame {n}"
        );
        let got = sim.with(|s, r| dp.receive(s, r, &mut out));
        assert_eq!(got, Some(f.len()));
        assert_eq!(&out[..f.len()], &f[..]);
        assert_eq!(sim.with(|s, r| dp.receive(s, r, &mut out)), None);
    }
    assert_eq!(dp.dropped, 0);
}

#[test]
fn a_full_ring_holds_all_but_one_and_drains_completely() {
    let (mut sim, mut dp) = started();
    let mut delivered = 0;
    while sim.deliver(&frame(1, 64), 64, GOOD, 0) {
        delivered += 1;
    }
    assert_eq!(delivered, RX_ENTRIES - 1);
    let mut out = [0u8; MAX_FRAME];
    for _ in 0..delivered {
        assert_eq!(sim.with(|s, r| dp.receive(s, r, &mut out)), Some(64));
    }
    // Every slot was handed back, so the device can fill the ring again.
    let mut again = 0;
    while sim.deliver(&frame(2, 64), 64, GOOD, 0) {
        again += 1;
    }
    assert_eq!(again, RX_ENTRIES - 1);
}

#[test]
fn a_lying_or_bad_descriptor_is_dropped_and_the_ring_moves_on() {
    let (mut sim, mut dp) = started();
    let mut out = [0u8; MAX_FRAME];
    assert!(sim.deliver(&frame(1, 64), 0xffff, GOOD, 0)); // longer than any buffer
    assert!(sim.deliver(&frame(2, 64), 64, RX_STATUS_DD, 0)); // no end of packet
    assert!(sim.deliver(&frame(3, 64), 64, GOOD, 0x01)); // CRC error
    assert!(sim.deliver(&frame(4, 8), 8, GOOD, 0)); // shorter than a header
    let f = frame(5, 99);
    assert!(sim.deliver(&f, 99, GOOD, 0));
    assert_eq!(sim.with(|s, r| dp.receive(s, r, &mut out)), Some(99));
    assert_eq!(&out[..99], &f[..]);
    assert_eq!(dp.dropped, 4);
}

#[test]
fn a_frame_larger_than_the_callers_room_is_dropped_not_truncated() {
    let (mut sim, mut dp) = started();
    assert!(sim.deliver(&frame(1, 600), 600, GOOD, 0));
    let mut small = [0u8; 576];
    assert_eq!(sim.with(|s, r| dp.receive(s, r, &mut small)), None);
    assert_eq!(dp.dropped, 1);
}

#[test]
fn a_device_that_overwrote_the_buffer_address_gets_it_back_on_repost() {
    let (mut sim, mut dp) = started();
    assert!(sim.deliver(&frame(1, 64), 64, GOOD, 0));
    sim.write_u64(layout::rx_descriptor(0), 0xdead_0000);
    let mut out = [0u8; MAX_FRAME];
    assert_eq!(sim.with(|s, r| dp.receive(s, r, &mut out)), Some(64));
    assert_eq!(
        sim.read_u64(layout::rx_descriptor(0)),
        PHYS + layout::rx_buffer(0)
    );
}

#[test]
fn transmit_sends_exact_bytes_and_reports_full_until_the_device_catches_up() {
    let (mut sim, mut dp) = started();
    let mut queued = Vec::new();
    loop {
        let f = frame(queued.len() as u8, 60 + queued.len() * 90);
        match sim.with(|s, r| dp.transmit(s, r, &f)) {
            Ok(()) => queued.push(f),
            Err(TxError::Full) => break,
            Err(e) => panic!("{e:?}"),
        }
    }
    assert_eq!(queued.len(), TX_ENTRIES as usize - 1);
    assert!(!dp.can_transmit(&mut sim));
    sim.drain_tx();
    assert_eq!(sim.sent, queued);
    assert!(dp.can_transmit(&mut sim));
    // And the ring keeps going round.
    for n in 0..3 * TX_ENTRIES as usize {
        let f = frame(n as u8, MAX_FRAME);
        sim.with(|s, r| dp.transmit(s, r, &f)).unwrap();
        sim.drain_tx();
        assert_eq!(sim.sent.last(), Some(&f));
    }
}

#[test]
fn transmit_refuses_empty_and_oversized_frames() {
    let (mut sim, mut dp) = started();
    assert_eq!(
        sim.with(|s, r| dp.transmit(s, r, &[])),
        Err(TxError::BadLength)
    );
    let big = vec![0u8; MAX_FRAME + 1];
    assert_eq!(
        sim.with(|s, r| dp.transmit(s, r, &big)),
        Err(TxError::BadLength)
    );
    assert_eq!(sim.tdt, 0, "a refused frame moved the tail");
}

#[test]
fn the_layout_fits_its_pages_and_the_rings_fit_theirs() {
    assert_eq!(layout::PAGES, 18);
    assert!(RX_ENTRIES as u64 * 16 <= PAGE && TX_ENTRIES as u64 * 16 <= PAGE);
    assert_eq!(
        RX_ENTRIES as u64 * 16 % 128,
        0,
        "RDLEN must be a multiple of 128"
    );
    assert_eq!(
        TX_ENTRIES as u64 * 16 % 128,
        0,
        "TDLEN must be a multiple of 128"
    );
    assert!(MAX_FRAME as u64 <= BUFFER_SIZE);
    assert_eq!(layout::RX_BUFFERS % PAGE, 0);
    assert_eq!(layout::TX_BUFFERS % PAGE, 0);
}

#[test]
fn the_queue_pages_hold_the_tails_and_not_the_control_registers() {
    assert_eq!(regs::RDT & !0xfff, RX_QUEUE_PAGE);
    assert_eq!(regs::TDT & !0xfff, TX_QUEUE_PAGE);
    assert_eq!(regs::TDT - TX_QUEUE_PAGE, TAIL_IN_PAGE);
    // The finding: base and tail share a page, so the split cannot keep ring placement.
    assert_eq!(regs::RDBAL & !0xfff, RX_QUEUE_PAGE);
    for r in [
        regs::CTRL,
        regs::STATUS,
        regs::RCTL,
        regs::TCTL,
        regs::IMC,
        regs::RAL0,
        regs::RAH0,
        regs::RFCTL,
    ] {
        let page = r & !0xfff;
        assert!(
            page != RX_QUEUE_PAGE && page != TX_QUEUE_PAGE,
            "{r:#x} is mapped"
        );
    }
}

#[test]
fn the_mac_address_reads_from_entry_zero_and_refuses_an_invalid_one() {
    // QEMU's default, 52:54:00:12:34:56, as RAL/RAH hold it.
    let ral = u32::from_le_bytes([0x52, 0x54, 0x00, 0x12]);
    let rah = u32::from_le_bytes([0x34, 0x56, 0, 0]) | RAH_AV;
    assert_eq!(
        mac_from_receive_address(ral, rah),
        Some([0x52, 0x54, 0x00, 0x12, 0x34, 0x56])
    );
    assert_eq!(mac_from_receive_address(ral, rah & !RAH_AV), None);
    assert_eq!(mac_from_receive_address(0, RAH_AV), None);
    assert_eq!(
        mac_from_receive_address(0x01, RAH_AV),
        None,
        "a group address"
    );
}

#[test]
fn the_handoff_role_never_collides_with_net_stacks_other_roles() {
    for role in 0..=255u64 {
        assert!(!Handoff::is_role(role));
    }
    let h = Handoff {
        mac: [0x52, 0x54, 0, 0x12, 0x34, 0x56],
        data_plane_phys: PHYS,
    };
    assert!(Handoff::is_role(h.pack()[0]));
}

#[test]
fn the_claimed_ids_are_intel_and_qemus_is_among_them() {
    assert!(is_supported(0x8086, 0x10d3));
    assert!(is_supported(0x8086, 0x15b7));
    assert!(
        !is_supported(0x10ec, 0x10d3),
        "a Realtek part with a colliding id"
    );
    assert!(
        !is_supported(0x8086, 0x100e),
        "the older e1000, a different register map"
    );
    assert_eq!(model(0x10d3), Some("82574L (QEMU's e1000e)"));
}

#[test]
fn the_control_words_carry_the_bits_the_device_needs() {
    assert_ne!(rctl() & 0x2, 0, "RCTL.EN");
    assert_ne!(rctl() & 0x8000, 0, "broadcast, for the DHCP offer");
    assert_eq!(rctl() & 0x18, 0, "never promiscuous");
    assert_eq!(rctl() & 0x0003_0000, 0, "2048-byte buffers");
    assert_ne!(tctl() & 0x2, 0, "TCTL.EN");
}

/// The three control words as whole values, so a field that lands at the wrong shift is caught
/// as well as a missing enable bit. `TCTL`: EN, PSP, CT 15 at bit 4, COLD 63 at bit 12. `TIPG`:
/// IPGT 8, IPGR1 8 at bit 10, IPGR2 6 at bit 20 (Linux's `DEFAULT_82543_TIPG_*`).
#[test]
fn the_control_words_are_linuxs_values_exactly() {
    assert_eq!(rctl(), 0x0400_8002);
    assert_eq!(tctl(), 0x0003_f0fa);
    assert_eq!(tctl() >> 4 & 0xff, 15, "collision threshold");
    assert_eq!(tctl() >> 12 & 0x3ff, 63, "collision distance");
    assert_eq!(tipg(), 0x0060_2008);
    assert_eq!(
        (tipg() & 0x3ff, tipg() >> 10 & 0x3ff, tipg() >> 20 & 0x3ff),
        (8, 8, 6)
    );
}

/// Each of the five frame-error bits drops the frame on its own, so no one of them can fall out
/// of the mask unnoticed: CRC, symbol, sequence, carrier extension and RX data errors.
#[test]
fn every_frame_error_bit_drops_the_frame() {
    let good = |errors: u8| 60 | (GOOD as u64) << 32 | (errors as u64) << 40;
    assert_eq!(rx_verdict(good(0), 1514), RxVerdict::Frame(60));
    for bit in [0x01, 0x02, 0x04, 0x10, 0x80] {
        assert_eq!(
            rx_verdict(good(bit), 1514),
            RxVerdict::Drop,
            "error bit {bit:#04x}"
        );
    }
    // Bits 3 and 6 are not frame errors (TCP/UDP and IP checksum), and checksum offload is off.
    assert_eq!(rx_verdict(good(0x08 | 0x40), 1514), RxVerdict::Frame(60));
}

/// A transmit descriptor's command byte asks for all three things the ring depends on: end of
/// packet, the FCS appended, and a status write-back that `reclaim` waits for.
#[test]
fn the_transmit_command_is_eop_ifcs_and_rs() {
    assert_eq!(TX_COMMAND & 0x01, 0x01, "EOP");
    assert_eq!(TX_COMMAND & 0x02, 0x02, "IFCS");
    assert_eq!(TX_COMMAND & 0x08, 0x08, "RS");
    assert_eq!(TX_COMMAND, 0x0b);
}

/// Every buffer and descriptor index is reduced modulo its ring, and successive slots sit one
/// buffer or one descriptor apart inside their own range of the region, so two slots never
/// share memory and no index reaches past its ring.
#[test]
fn ring_slots_are_distinct_evenly_spaced_and_wrap() {
    for i in 0..2 * RX_ENTRIES {
        let next = (i + 1) % RX_ENTRIES;
        assert_eq!(layout::rx_buffer(i), layout::rx_buffer(i % RX_ENTRIES));
        assert_eq!(
            layout::rx_descriptor(i),
            layout::rx_descriptor(i % RX_ENTRIES)
        );
        assert!((layout::RX_BUFFERS..layout::TX_BUFFERS).contains(&layout::rx_buffer(i)));
        assert!((layout::RX_RING..layout::TX_RING).contains(&layout::rx_descriptor(i)));
        if next != 0 {
            assert_eq!(
                layout::rx_buffer(next) - layout::rx_buffer(i % RX_ENTRIES),
                BUFFER_SIZE
            );
            assert_eq!(
                layout::rx_descriptor(next) - layout::rx_descriptor(i % RX_ENTRIES),
                16
            );
        }
    }
    for i in 0..2 * TX_ENTRIES {
        let next = (i + 1) % TX_ENTRIES;
        assert_eq!(layout::tx_buffer(i), layout::tx_buffer(i % TX_ENTRIES));
        assert_eq!(
            layout::tx_descriptor(i),
            layout::tx_descriptor(i % TX_ENTRIES)
        );
        assert!((layout::TX_BUFFERS..layout::BYTES).contains(&layout::tx_buffer(i)));
        assert!((layout::TX_RING..layout::RX_BUFFERS).contains(&layout::tx_descriptor(i)));
        if next != 0 {
            assert_eq!(
                layout::tx_buffer(next) - layout::tx_buffer(i % TX_ENTRIES),
                BUFFER_SIZE
            );
            assert_eq!(
                layout::tx_descriptor(next) - layout::tx_descriptor(i % TX_ENTRIES),
                16
            );
        }
    }
    assert_eq!(layout::rx_buffer(0), layout::RX_BUFFERS);
    assert_eq!(layout::tx_buffer(0), layout::TX_BUFFERS);
    assert_eq!(layout::rx_descriptor(0), layout::RX_RING);
    assert_eq!(layout::tx_descriptor(0), layout::TX_RING);
}

/// `start` clears the whole transmit ring, so no descriptor holds the region's stale bytes
/// (the simulation fills it with `0xa5`), and the receive cursor names the slot `receive`
/// reads next.
#[test]
fn start_clears_the_transmit_ring_and_the_cursor_follows_receive() {
    let (mut sim, mut dp) = started();
    for i in 0..TX_ENTRIES {
        let d = layout::tx_descriptor(i);
        assert_eq!(
            (sim.read_u64(d), sim.read_u64(d + 8)),
            (0, 0),
            "descriptor {i}"
        );
    }
    assert_eq!(dp.rx_cursor(), 0);
    let mut out = [0u8; MAX_FRAME];
    for n in 1..=3u16 {
        assert!(sim.deliver(&frame(1, 64), 64, GOOD, 0));
        assert_eq!(sim.with(|s, r| dp.receive(s, r, &mut out)), Some(64));
        assert_eq!(dp.rx_cursor(), n);
    }
}

/// The handoff refuses each bad field on its own: a role word with a valid-looking MAC but the
/// wrong tag, and a group MAC under the right tag.
#[test]
fn the_handoff_refuses_a_wrong_tag_and_a_group_address_each_alone() {
    let h = Handoff {
        mac: [0x52, 0x54, 0, 0x12, 0x34, 0x56],
        data_plane_phys: PHYS,
    };
    let [role, phys] = h.pack();
    assert_eq!(Handoff::unpack(role, phys), Some(h));
    assert_eq!(Handoff::unpack(role ^ 1, phys), None, "wrong tag");
    let group = Handoff {
        mac: [0x53, 0x54, 0, 0x12, 0x34, 0x56],
        ..h
    };
    assert_eq!(
        Handoff::unpack(group.pack()[0], phys),
        None,
        "group address"
    );
}
