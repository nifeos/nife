/// The word the receiver sends back through the delegated capability, so a test can confirm a
/// capability minted by one process works when invoked by another.
///
/// **One definition, in `capability_witness_contract`.** This was a `pub const` here with a matching
/// literal in `fixtures/src/hello.rs` and a "must match" comment beside it, which is the shape
/// AGENTS.md rule 7 exists to remove; milestone 291 split the receiver into its own binary and the
/// duplicate became a crate.
pub use capability_witness_contract::USED_WORD;

use super::*;
use crate::capability_object::{Rights, rendezvous_capability};
use crate::sched::RendezvousId;

/// Spawn the pair and return `(resource endpoint, report endpoint)`. The granter delegates its
/// `resource` capability (held `WRITE | GRANT`) to the receiver, narrowed to `WRITE`. The
/// receiver `SEND`s [`USED_WORD`] on the received capability (a `RECEIVE` on `resource` collects
/// it) and reports a two-bit verdict on `report`.
///
/// `endpoints` is a region of at least four pages the caller owns and reclaims once it has the
/// verdict; every endpoint here is carved from it.
pub fn wire(endpoints: u64) -> (RendezvousId, RendezvousId) {
    let granter = program("delegation_granter").expect("no delegation_granter in the archive");
    let receiver = program("delegation_receiver").expect("no delegation_receiver in the archive");
    let channel = crate::sched::create_rendezvous_from(endpoints).expect("no channel rendezvous"); // granter SEND_CAPABILITY -> receiver RECEIVE_CAPABILITY
    let resource = crate::sched::create_rendezvous_from(endpoints).expect("no resource rendezvous"); // the capability being delegated
    let loopback = crate::sched::create_rendezvous_from(endpoints).expect("no loopback rendezvous"); // the receiver's refused re-delegation target
    let report = crate::sched::create_rendezvous_from(endpoints).expect("no report rendezvous"); // the receiver's verdict

    crate::sched::spawn(move || {
        run(
            granter,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_capability(channel, Rights::WRITE), // slot 0: SEND_CAPABILITY over it
                    rendezvous_capability(resource, Rights::WRITE.union(Rights::GRANT)), // slot 1: delegate this
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the delegation granter");

    crate::sched::spawn(move || {
        run(
            receiver,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_capability(channel, Rights::READ), // slot 0: RECEIVE_CAPABILITY
                    rendezvous_capability(report, Rights::WRITE), // slot 1: report the verdict
                    rendezvous_capability(loopback, Rights::WRITE), // slot 2: attempt re-delegation here
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the delegation receiver");

    (resource, report)
}
