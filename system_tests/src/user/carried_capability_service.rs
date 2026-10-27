//! **A reply that carries a capability, at EL0** (§255 (each socket is its own capability),
//! milestone 649 (every client of a network stack shares its socket numbers)): the wiring for `carried_capability_server` and `carried_capability_client`.
//! Name provisional, 2026-10-07 (UTC).

use super::*;
use crate::capability_object::Rights;
use crate::sched::RendezvousId;

/// Spawn the pair. Returns `(server report, client report)`. Every endpoint comes out of
/// `endpoints`, a region of at least four pages the caller owns and reclaims.
pub fn wire(endpoints: u64) -> (RendezvousId, RendezvousId) {
    let server = program("carried_capability_server").expect("no carried_capability_server");
    let client = program("carried_capability_client").expect("no carried_capability_client");
    let ep = crate::sched::create_rendezvous_from(endpoints).expect("no request rendezvous");
    let probe = crate::sched::create_rendezvous_from(endpoints).expect("no probe rendezvous");
    let server_report = crate::sched::create_rendezvous_from(endpoints).expect("no report");
    let client_report = crate::sched::create_rendezvous_from(endpoints).expect("no report");
    let endpoint = crate::capability_object::rendezvous_capability;

    crate::sched::spawn(move || {
        run(
            server,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    endpoint(ep, Rights::READ),             // slot 0: RECEIVE calls
                    endpoint(server_report, Rights::WRITE), // slot 1: the verdict
                    endpoint(probe, Rights::READ),          // slot 2: what the carried copy names
                    endpoint(probe, Rights::WRITE.union(Rights::GRANT)), // slot 3: carried
                    endpoint(probe, Rights::WRITE),         // slot 4: usable, not passable
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the carried-capability server");

    crate::sched::spawn(move || {
        run(
            client,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    endpoint(ep, Rights::WRITE),            // slot 0: CALL
                    endpoint(client_report, Rights::WRITE), // slot 1: the verdict
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the carried-capability client");

    (server_report, client_report)
}
