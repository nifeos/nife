//! **A client trying to upgrade itself into the server** (an eighth outsider pass at the
//! confinement claim, claim 26).
//!
//! Past the rights gate (`confinement_attack_tests`) and the live-swap usurper (`live_swap_tests`),
//! a client tries two roads through the real dispatcher. Holding `WRITE | GRANT`, it asks
//! `SEND_CAP` for a copy of its own capability that adds `READ`, which the delegation rule refuses.
//! A `BADGE`d copy is refused all four server methods, since a badge labels a sender and carries
//! exactly the source's rights. Nothing blocks. Cross-ISA (DECISIONS §19 (architectural parity is
//! a tenet)). See notes/confinement-claims/the-eighth-pass-tests.md.

use abi::Error;

use crate::arch::exceptions::TrapFrame;
// The `cap` module and these names still carry their pre-sweep spelling
// (design/naming/capability-spelling-worklist.md renames them when the tree is quiet); aliased here so
// this file spells `capability` out everywhere a reader looks (calef, 2026-10-06).
use crate::cap::{Rights, rendezvous_cap as rendezvous_capability};
use crate::sched::{
    self, current_cap as current_capability, delete_current_cap as delete_current_capability,
};
use crate::syscall::invoke;

/// `invoke` through the real dispatcher, the way a program's `svc`/`ecall` reaches it.
fn call(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, a1, a2)
}

/// **A client with every right a client can hold cannot mint itself the server's right, and a
/// badge is not one either.**
///
/// A sender stays parked throughout, so a method let through wrongly shows as a delivery and a
/// falling waiting-sender count, not a hang. The self-upgrade, the badged copies and all four
/// server methods each leave it parked. A real `READ` capability then receives the queued message,
/// the positive control that the endpoint, the sender and the dispatcher were live.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_client_cannot_upgrade_or_badge_its_way_to_the_receiving_side() {
    let region = crate::memory_region::create(4).expect("no region for the endpoint");
    let ep = sched::create_rendezvous_from(region).expect("no rendezvous");

    const SENT_WORD: u64 = 0x733;
    let sender = sched::spawn(move || {
        sched::ipc_send(ep, [SENT_WORD, 0, 0]);
    })
    .expect("spawn the sender");
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_senders(ep) == 1),
        "the sender never parked, so a let-open method would hang rather than be observed",
    );

    // The attacker: WRITE and GRANT, every right a client holds. READ, the receiving right, is
    // exactly what it lacks.
    let client = sched::grant(rendezvous_capability(
        ep,
        Rights::WRITE.union(Rights::GRANT),
    ))
    .expect("grant the client cap");

    // 1. Self-upgrade: SEND_CAP with a mask carrying READ. READ is not a subset of WRITE|GRANT, so
    //    the derivation must refuse, GRANT or no GRANT: no right comes out of the a1 register.
    let upgrade_mask = Rights::ALL.bits() as u64;
    assert_eq!(
        call(client, abi::rendezvous::SEND_CAP, client, upgrade_mask, 0),
        Err(Error::NotPermitted),
        "a client minted itself a copy of the endpoint carrying every right: the receiving side \
         was one delegation away",
    );
    assert_eq!(
        sched::rendezvous_waiting_senders(ep),
        1,
        "a refused self-upgrade still parked a sender",
    );

    // 2. Badge its own copy: the one method GRANT legitimately gates. The mint must succeed (it
    //    is the client's own capability, badged within the rules) and carry exactly the source's
    //    rights, which is the whole question.
    let badged = call(client, abi::rendezvous::BADGE, 0x5EED, 0, 0)
        .expect("BADGE refused a GRANT-holding client's own capability") as u64;
    assert_eq!(
        current_capability(badged)
            .expect("the badged slot is empty")
            .rights,
        Rights::WRITE.union(Rights::GRANT),
        "a badged copy carried a right its source did not: the badge minted authority",
    );

    // 3. Every server method, tried through the badged copy: the badge labels a sender, it is not
    //    a right. A receiver is parked first for the two blocking methods, so a let-open gate
    //    would deliver and be observed.
    assert_eq!(
        call(badged, abi::rendezvous::RECEIVE, 0, 0, 0),
        Err(Error::NotPermitted),
        "a badged client copy was let RECEIVE: a client became its server by wearing a badge",
    );
    assert_eq!(
        call(badged, abi::rendezvous::RECEIVE_CAP, 0, 0, 0),
        Err(Error::NotPermitted),
        "a badged client copy was let RECEIVE_CAP",
    );
    assert_eq!(
        call(badged, abi::rendezvous::REAP, 0, 0, 0),
        Err(Error::NotPermitted),
        "a badged client copy was let REAP",
    );
    assert_eq!(
        call(
            badged,
            abi::rendezvous::SURVEY,
            0,
            abi::survey::record::STATE,
            0
        ),
        Err(Error::NotPermitted),
        "a badged client copy was let SURVEY",
    );
    assert_eq!(
        sched::rendezvous_waiting_senders(ep),
        1,
        "a refused method still consumed the parked sender",
    );

    // Positive control: a real READ capability, through the same dispatcher, receives the parked
    // sender's message. The endpoint was live all along.
    let server = sched::grant(rendezvous_capability(ep, Rights::READ)).expect("grant the READ cap");
    assert_eq!(
        call(server, abi::rendezvous::RECEIVE, 0, 0, 0),
        Ok(SENT_WORD as i64),
        "a READ holder could not receive: the endpoint was not really live",
    );
    assert!(
        super::wait_for(|| !sched::is_thread_present(sender)),
        "the sender thread never finished after its message was collected",
    );

    let _ = delete_current_capability(client);
    let _ = delete_current_capability(badged);
    let _ = delete_current_capability(server);
    sched::reclaim_region(region).expect("the endpoint's region did not come back");
}
