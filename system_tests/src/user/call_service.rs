//! Name: ratified 2026-09-15 (calef, the doc on `wire`, moved here 2026-09-24), as
//! `call_reply_service`. The wiring folds onto the same `call_reply` stem as the two fixtures it
//! spawns, so `git grep call_reply` finds the pair and their kernel-side wiring as one family.
//! Module/file rename pending the batched 291 sweep.

use super::*;
use crate::capability_object::{Rights, rendezvous_capability};
use crate::sched::RendezvousId;

/// Spawn the pair, sharing one request endpoint. Returns `(client reply report, server one-shot
/// report)`: the client publishes the reply it got, the server publishes whether a second reply
/// was refused.
///
/// **Two images, one endpoint.** These were roles 14 and 15 of the `hello` multiplexer until
/// milestone 291, so this took one image and two role numbers; they are `fixtures/src/call_server.rs`
/// and `fixtures/src/call_client.rs` now, and neither reads `x0`.
///
/// `endpoints` is a region of at least three pages the caller owns and reclaims once it has both
/// verdicts; every endpoint here is carved from it.
pub fn wire(endpoints: u64) -> (RendezvousId, RendezvousId) {
    let server = program("call_server").expect("no call_server program in the archive");
    let client = program("call_client").expect("no call_client program in the archive");
    let ep = crate::sched::create_rendezvous_from(endpoints).expect("no ep rendezvous"); // client CALL <-> server RECEIVE_CAPABILITY
    let call_report =
        crate::sched::create_rendezvous_from(endpoints).expect("no call_report rendezvous");
    let oneshot_report =
        crate::sched::create_rendezvous_from(endpoints).expect("no oneshot_report rendezvous");

    crate::sched::spawn(move || {
        run(
            server,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_capability(ep, Rights::READ), // slot 0: RECEIVE calls
                    rendezvous_capability(oneshot_report, Rights::WRITE), // slot 1: report the verdict
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the call server");

    crate::sched::spawn(move || {
        run(
            client,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_capability(ep, Rights::WRITE), // slot 0: CALL
                    rendezvous_capability(call_report, Rights::WRITE), // slot 1: report the reply
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the call client");

    (call_report, oneshot_report)
}
