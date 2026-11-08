//! Name: ratified 2026-10-25 (#2720).

use super::*;
use crate::registers::find_extended;

/// **A producer ring never places a TRB on its link slot or past the end**, and places the link
/// exactly on the last slot, from every reachable state.
/// Falsification: replayable `crates/extensible_host_controller_interface/falsifications/verification.a_producer_ring_stays_in_bounds.patch`
#[kani::proof]
fn a_producer_ring_stays_in_bounds() {
    let entries: u16 = kani::any();
    let enqueue: u16 = kani::any();
    kani::assume(entries >= 2 && enqueue < entries - 1);
    let mut ring = ProducerRing {
        base: kani::any(),
        entries,
        enqueue,
        cycle: kani::any(),
    };
    let p = ring.enqueue(TransferRequestBlock(kani::any()));
    assert!(p.index < entries - 1);
    if let Some((at, link)) = p.link {
        assert_eq!(at, entries - 1);
        assert_eq!(link.kind(), transfer_request_block_type::LINK);
    }
    assert!(ring.enqueue < entries - 1);
}

/// **The event ring's dequeue index stays inside the segment, and its cycle flips exactly when the
/// index wraps**, from every reachable state. The physical address handed to `ERDP` stays inside
/// the segment too, for a segment that does not run past the end of the address space.
/// Falsification: replayable `crates/extensible_host_controller_interface/falsifications/verification.the_event_ring_stays_in_bounds_and_flips_only_at_the_wrap.patch`
#[kani::proof]
fn the_event_ring_stays_in_bounds_and_flips_only_at_the_wrap() {
    let entries: u16 = kani::any();
    let dequeue: u16 = kani::any();
    kani::assume(entries > 0 && dequeue < entries);
    let base: u64 = kani::any();
    // The segment is a physical range that exists, so its end does not wrap the address space.
    kani::assume(base.checked_add(u64::from(entries) * 16).is_some());
    let cycle: bool = kani::any();
    let mut ring = EventRing {
        base,
        entries,
        dequeue,
        cycle,
    };
    ring.advance();
    assert!(ring.dequeue < entries);
    assert_eq!(ring.cycle != cycle, dequeue == entries - 1);
    assert_eq!(ring.dequeue == 0, dequeue == entries - 1);
    let event_ring_dequeue_pointer = ring.dequeue_pointer();
    assert!(
        event_ring_dequeue_pointer >= base
            && event_ring_dequeue_pointer < base + u64::from(entries) * 16
    );
}

/// **The extended-capability walk ends within 64 reads, reads only inside `limit`, moves forward on
/// every hop, and returns only an entry it read whose id matched**, whatever dword the controller
/// returns at each offset. The register file is modeled as arbitrary per read, which includes every
/// fixed register file.
/// Falsification: replayable `crates/extensible_host_controller_interface/falsifications/verification.the_capability_walk_ends_and_stays_inside_the_limit.patch`
#[kani::proof]
#[kani::unwind(66)]
fn the_capability_walk_ends_and_stays_inside_the_limit() {
    let first: u64 = kani::any();
    let limit: u64 = kani::any();
    let id: u8 = kani::any();
    let mut reads = 0u32;
    let mut last: Option<(u64, u32)> = None;
    let found = {
        let mut read = |at: u64| {
            reads += 1;
            assert!(reads <= 64);
            assert!(at != 0 && at.checked_add(4).is_some_and(|end| end <= limit));
            if let Some((before, _)) = last {
                assert!(at > before);
            }
            let head: u32 = kani::any();
            last = Some((at, head));
            head
        };
        find_extended(first, limit, id, &mut read)
    };
    if let Some(at) = found {
        let (read_at, head) = last.unwrap();
        assert_eq!(at, read_at);
        assert_eq!(head as u8, id);
    }
}

/// **The device context index of any endpoint address is a real index, and its offsets stay in the
/// context.** Endpoint 0 is index 1 in either direction; every other endpoint number and
/// direction has its own index from 2 to 31; and for either context size the xHCI allows (32 or 64
/// bytes) a device context holds the offset of every index and an input context one more.
/// Falsification: replayable `crates/extensible_host_controller_interface/falsifications/verification.a_device_context_index_is_one_to_thirty_one_and_distinct.patch`
#[kani::proof]
fn a_device_context_index_is_one_to_thirty_one_and_distinct() {
    let address: u8 = kani::any();
    let other: u8 = kani::any();
    let index = context::endpoint_index(address);
    assert!((1..=31).contains(&index));
    if address & 0x0f == 0 {
        assert_eq!(index, 1);
    }
    let same_endpoint = address & 0x8f == other & 0x8f;
    if address & 0x0f != 0 && other & 0x0f != 0 && !same_endpoint {
        assert_ne!(index, context::endpoint_index(other));
    }
    let context_bytes: u64 = if kani::any() { 32 } else { 64 };
    assert!(context::device_offset(index, context_bytes) + context_bytes <= 32 * context_bytes);
    assert!(context::input_offset(index + 1, context_bytes) + context_bytes <= 33 * context_bytes);
}

/// **A slot context and an endpoint context hold each field in its own bits, for every input.**
/// What goes in comes back out of exactly the field the specification gives it (xHCI 1.2 sections
/// 6.2.2 and 6.2.3), and no field spills into a neighbor: the speed and the context count are
/// masked to their widths, the endpoint's dequeue pointer is reassembled from its two dwords, and
/// the reserved dwords stay zero.
/// Falsification: replayable `crates/extensible_host_controller_interface/falsifications/verification.the_slot_and_endpoint_contexts_keep_each_field_in_its_own_bits.patch`
#[kani::proof]
fn the_slot_and_endpoint_contexts_keep_each_field_in_its_own_bits() {
    let speed: u8 = kani::any();
    let port: u8 = kani::any();
    let entries: u8 = kani::any();
    let slot = context::slot(speed, port, entries);
    assert_eq!((slot[0] >> 20) & 0xf, speed as u32 & 0xf);
    assert_eq!(slot[0] >> 27, entries as u32 & 0x1f);
    assert_eq!(slot[0] & !((0xf << 20) | (0x1f << 27)), 0);
    assert_eq!(slot[1], (port as u32) << 16);
    assert_eq!(slot[2..], [0; 6]);

    let kind: u8 = kani::any();
    let maximum_packet: u16 = kani::any();
    let interval: u8 = kani::any();
    let dequeue: u64 = kani::any();
    let average_trb: u16 = kani::any();
    let endpoint = context::endpoint(kind, maximum_packet, interval, dequeue, average_trb);
    assert_eq!(endpoint[0], (interval as u32) << 16);
    assert_eq!((endpoint[1] >> 1) & 0x3, 3);
    assert_eq!((endpoint[1] >> 3) & 0xff, kind as u32);
    assert_eq!(endpoint[1] >> 16, maximum_packet as u32);
    assert_eq!(endpoint[1] & 1, 0);
    assert_eq!(endpoint[2] as u64 | (endpoint[3] as u64) << 32, dequeue);
    assert_eq!(endpoint[4] & 0xffff, average_trb as u32);
    let maximum_service_interval_payload = if kind == context::CONTROL {
        0
    } else {
        maximum_packet as u32
    };
    assert_eq!(endpoint[4] >> 16, maximum_service_interval_payload);
    assert_eq!(endpoint[5..], [0; 3]);
}

/// **A write to the port status register never writes back a change bit or the Port Enabled bit
/// the caller did not ask for, and the speed id is the four bits it comes from.** Whatever the
/// register read, the written value is the preserved bits of it plus exactly the bits asked for,
/// and writing nothing writes no write-one-to-clear bit.
/// Falsification: replayable `crates/extensible_host_controller_interface/falsifications/verification.a_port_write_never_acknowledges_what_nobody_asked_to.patch`
#[kani::proof]
fn a_port_write_never_acknowledges_what_nobody_asked_to() {
    let portsc: u32 = kani::any();
    let bits: u32 = kani::any();
    let written = port::write_value(portsc, bits);
    assert_eq!(written & !bits, portsc & port::PRESERVE & !bits);
    assert_eq!(written & bits, bits);
    let nothing = port::write_value(portsc, 0);
    assert_eq!(nothing & port::CHANGES, 0);
    assert_eq!(nothing & port::ENABLED, 0);
    assert_eq!(nothing & port::RESET, 0);
    assert!(port::speed_of(portsc) <= 0xf);
    assert_eq!(port::speed_of(portsc) as u32, (portsc >> 10) & 0xf);
}

/// **The interrupt interval is the exponent the specification asks for, and the default control
/// packet is known exactly for the five speeds that exist.** For a full- or low-speed device it is
/// the largest exponent whose period (`2^n` microframes) does not exceed the device's
/// millisecond period, between 3 and 10; for the others it is the device's exponent less one,
/// at most 15. `default_control_packet` answers for speeds 1 to 5 and for no other id.
/// Falsification: replayable `crates/extensible_host_controller_interface/falsifications/verification.the_interrupt_interval_is_the_exponent_the_specification_asks_for.patch`
#[kani::proof]
fn the_interrupt_interval_is_the_exponent_the_specification_asks_for() {
    let speed: u8 = kani::any();
    let b_interval: u8 = kani::any();
    let interval = context::interrupt_interval(speed, b_interval);
    if speed == port::speed::FULL || speed == port::speed::LOW {
        let microframes = (if b_interval == 0 { 1 } else { b_interval }) as u32 * 8;
        assert!((3..=10).contains(&interval));
        assert!(1u32 << interval <= microframes);
        assert!(interval == 10 || interval == 3 || (1u32 << (interval + 1)) > microframes);
    } else {
        assert!(interval <= 15);
        let wanted = if b_interval == 0 {
            1
        } else {
            b_interval.min(16)
        };
        assert_eq!(interval + 1, wanted);
    }
    assert_eq!(
        port::default_control_packet(speed).is_some(),
        (1..=5).contains(&speed)
    );
}
