use super::*;
use crate::capability_object::{Rights, memory_region_lending_capability, rendezvous_capability};
use crate::sched::RendezvousId;

/// Spawn the pair; returns the report endpoint carrying the word that crossed the minted
/// endpoint.
///
/// `endpoints` is a region of at least two pages the caller owns and reclaims once it has the
/// report; both kernel-made endpoints are carved from it.
pub fn wire(endpoints: u64) -> RendezvousId {
    // Roles 17 and 18 of the `hello` multiplexer until milestone 291; two binaries now.
    let minter = program("rendezvous_minter").expect("no rendezvous_minter in the archive");
    let peer = program("rendezvous_peer").expect("no rendezvous_peer in the archive");
    let channel = crate::sched::create_rendezvous_from(endpoints).expect("no channel rendezvous");
    let report = crate::sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let region = crate::memory_region::create(4).expect("no region for the maker's budget");

    crate::sched::spawn(move || {
        run(
            minter,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    memory_region_lending_capability(region), // slot 0: the budget to mint from
                    rendezvous_capability(channel, Rights::WRITE), // slot 1: delegate the mint here
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the endpoint maker");

    crate::sched::spawn(move || {
        run(
            peer,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_capability(channel, Rights::READ), // slot 0: receive the delegation
                    rendezvous_capability(report, Rights::WRITE), // slot 1: report the word
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the endpoint user");

    report
}
