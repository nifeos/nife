use super::*;
use crate::capability_object::{Rights, memory_region_lending_capability, rendezvous_capability};
use crate::sched::RendezvousId;

/// Spawn the revoker with an 8-page memory region; returns the endpoint it reports its verdict on.
///
/// It was role 16 of the `hello` multiplexer until milestone 291 and is
/// `fixtures/src/frame_revoker.rs` now, which reads nothing from `x0`.
///
/// `endpoints` is a region the caller owns and reclaims once it has the verdict.
pub fn wire(endpoints: u64) -> RendezvousId {
    let image = program("frame_revoker").expect("no frame_revoker program in the archive");
    let region = crate::memory_region::create(8).expect("no untyped for the revoke demo");
    let report = crate::sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: 0, // one job, no role selector
                arg1: 0,
                arg2: 0,
                grants: &[
                    memory_region_lending_capability(region), // slot 0: retype + page tables
                    rendezvous_capability(report, Rights::WRITE), // slot 1: report the verdict
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the revoke demo");
    report
}
