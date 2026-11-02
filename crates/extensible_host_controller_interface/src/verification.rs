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
    let erdp = ring.dequeue_pointer();
    assert!(erdp >= base && erdp < base + u64::from(entries) * 16);
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
