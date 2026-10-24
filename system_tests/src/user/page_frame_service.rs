use super::*;
use crate::cap::{Rights, memory_region_cap, memory_region_lending_capability, rendezvous_cap};
use crate::sched::RendezvousId;

/// Spawn the pair, each with its own untyped budget, and return the endpoint the consumer
/// reports its verdict on. Eight pages of untyped apiece covers one frame plus the page tables
/// each side needs to map it.
///
/// `endpoints` is a region of at least two pages the caller owns and reclaims once it has the
/// verdict; both endpoints are carved from it.
pub fn wire(endpoints: u64) -> RendezvousId {
    // Two binaries, agreeing on `capability_witness_contract::PAGE_FRAME_SENTINEL` rather than on a
    // constant in one file.
    let producer = program("page_frame_producer").expect("no page_frame_producer in the archive");
    let consumer = program("page_frame_consumer").expect("no page_frame_consumer in the archive");
    let channel = crate::sched::create_rendezvous_from(endpoints).expect("no channel rendezvous");
    let report = crate::sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let prod_ut = crate::memory_region::create(8).expect("no untyped for the frame producer");
    let cons_ut = crate::memory_region::create(8).expect("no untyped for the frame consumer");

    crate::sched::spawn(move || {
        run(
            producer,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    // slot 0: retype the frame + page tables. Every right, because the frame is
                    // lent and a frame carries its budget's rights (milestone 824 (a retype
                    // mints no right its budget lacks)).
                    memory_region_lending_capability(prod_ut),
                    rendezvous_cap(channel, Rights::WRITE), // slot 1: delegate the frame
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the frame producer");

    crate::sched::spawn(move || {
        run(
            consumer,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(channel, Rights::READ), // slot 0: receive the frame
                    memory_region_cap(cons_ut),            // slot 1: page tables for its mappings
                    rendezvous_cap(report, Rights::WRITE), // slot 2: report the verdict
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the frame consumer");

    report
}
