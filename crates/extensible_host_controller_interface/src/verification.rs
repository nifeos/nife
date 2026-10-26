use super::*;

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
