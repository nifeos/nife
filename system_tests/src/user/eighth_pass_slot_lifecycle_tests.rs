//! **A consumed capability stays consumed, and deleting one disturbs nothing else** (an eighth
//! outsider pass at the confinement claim, claims 4, 5 and 2's slot half).
//!
//! Claims 4 and 5 are proved on host logic (`capability::a_deleted_capability_stays_deleted`,
//! `delete_touches_only_its_slot`). This module drives the live kernel, `delete_current_cap` then
//! `syscall::invoke`, with a stale slot number, an out-of-range index forged in a register, and a
//! deleted slot the table hands out again. Nothing blocks, and the frame and region ledgers end
//! flat. Cross-ISA (DECISIONS §19 (architectural parity is a tenet)). See
//! notes/confinement-claims/the-eighth-pass-tests.md.

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

/// `invoke` through the real dispatcher, the way a program's `svc`/`ecall` reaches it. Copied in
/// shape from `confinement_attack_tests::call`.
fn call(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, a1, a2)
}

/// **A deleted capability cannot be used again by its number, and a number the table never held
/// names nothing.**
///
/// A receiver is parked, so a use past the deletion shows as a delivery, not a hang. After the
/// delete, the slot's own number, an index past the table and `u64::MAX` must each answer
/// `NoSuchSlot`, and a second delete must fail. The control: a `READ` capability granted after the
/// deletion still receives what the deleted one sent, so only the one capability was consumed.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_consumed_capability_cannot_be_used_again_even_by_number() {
    let region = crate::memory_region::create(4).expect("no region for the endpoint");
    let ep = sched::create_rendezvous_from(region).expect("no rendezvous");

    let writer =
        sched::grant(rendezvous_capability(ep, Rights::WRITE)).expect("grant the WRITE cap");

    // A receiver parked on the endpoint, so a use-after-delete that slipped through would deliver
    // to it and drop the waiting-receiver count rather than block.
    let receiver = sched::spawn(move || {
        let _ = sched::ipc_receive(ep);
    })
    .expect("spawn the receiver");
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_receivers(ep) == 1),
        "the receiver never parked, so a let-open forgery would hang rather than be observed",
    );

    delete_current_capability(writer).expect("delete the WRITE cap");
    assert_eq!(
        call(writer, abi::rendezvous::SEND, 1, 0, 0),
        Err(Error::NoSuchSlot),
        "a deleted capability's slot number still invoked an object: a consumed capability was \
         used again",
    );
    assert_eq!(
        sched::rendezvous_waiting_receivers(ep),
        1,
        "a forged use of the deleted slot still reached the parked receiver",
    );
    assert_eq!(
        call(
            crate::cap::CAPABILITY_TABLE_SLOTS as u64,
            abi::rendezvous::SEND,
            1,
            0,
            0
        ),
        Err(Error::NoSuchSlot),
        "an index past the whole table named an object: authority out of a register",
    );
    assert_eq!(
        call(u64::MAX, abi::rendezvous::SEND, 1, 0, 0),
        Err(Error::NoSuchSlot),
        "the widest slot index a register can hold named an object",
    );
    assert!(
        current_capability(writer).is_err(),
        "the deleted slot still held a capability",
    );
    assert_eq!(
        delete_current_capability(writer),
        Err(crate::cap::Error::NoSuchSlot),
        "deleting the same capability a second time succeeded: the first delete left something",
    );

    // Positive control: a fresh READ capability receives through the same dispatcher a message
    // queued after the deletion. The parked receiver is released first with a kernel-side send
    // (the deleted capability is gone; nothing of this test may send through it again), so the
    // dispatcher's RECEIVE below has a sender to collect rather than an empty endpoint to block on.
    sched::ipc_send(ep, [3, 0, 0]);
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_receivers(ep) == 0),
        "the parked receiver never collected the release message",
    );
    assert!(
        super::wait_for(|| !sched::is_thread_present(receiver)),
        "the receiver thread never finished after its release",
    );
    let reader = sched::grant(rendezvous_capability(ep, Rights::READ)).expect("grant the READ cap");
    let sender = sched::spawn(move || {
        sched::ipc_send(ep, [7, 0, 0]);
    })
    .expect("spawn the sender");
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_senders(ep) == 1),
        "the sender never parked, so the READ cap's receive would hang rather than answer",
    );
    assert_eq!(
        call(reader, abi::rendezvous::RECEIVE, 0, 0, 0),
        Ok(7),
        "a READ holder could not receive after a neighbor's capability was deleted: deletion \
         disturbed more than its own slot",
    );
    assert_eq!(
        sched::rendezvous_waiting_senders(ep),
        0,
        "the dispatcher's receive left the sender parked",
    );

    let _ = delete_current_capability(reader);
    assert!(
        super::wait_for(|| !sched::is_thread_present(sender) && !sched::is_thread_present(receiver)),
        "a parked thread never finished after the endpoint drained",
    );
    sched::reclaim_region(region).expect("the endpoint's region did not come back");
}

/// **Deleting a capability leaves every other capability's rights byte for byte, and a slot the
/// table hands out again is minted fresh.**
///
/// Two capabilities on one object sit in the table; the middle one is deleted; the survivor must
/// still be present with exactly the rights it had and still work through the dispatcher. Then the
/// freed slot is filled again with a strictly narrower capability, and the regrant must carry
/// exactly the narrow rights: no bit of the deleted capability's rights may survive in the slot.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn deleting_one_capability_leaves_its_neighbors_alone_and_mints_nothing_stale() {
    let region = crate::memory_region::create(4).expect("no region for the endpoint");
    let ep = sched::create_rendezvous_from(region).expect("no rendezvous");

    let first = sched::grant(rendezvous_capability(ep, Rights::READ)).expect("grant the READ cap");
    let second =
        sched::grant(rendezvous_capability(ep, Rights::WRITE)).expect("grant the WRITE cap");
    let rights_before = current_capability(first)
        .expect("the READ cap vanished")
        .rights;

    delete_current_capability(second).expect("delete the WRITE cap");

    let survivor = current_capability(first).expect("deleting one slot emptied another");
    assert_eq!(
        survivor.rights, rights_before,
        "deleting one capability changed another's rights",
    );

    // The survivor must still work, not merely be present. A sender is queued and received through
    // the dispatcher with the deleted slot's neighbor.
    let sender = sched::spawn(move || {
        sched::ipc_send(ep, [9, 0, 0]);
    })
    .expect("spawn the sender");
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_senders(ep) == 1),
        "the sender never parked, so the survivor's receive would hang rather than answer",
    );
    assert_eq!(
        call(first, abi::rendezvous::RECEIVE, 0, 0, 0),
        Ok(9),
        "a capability neighboring a deleted one no longer works through the dispatcher",
    );

    // The freed slot, filled again, carries exactly what the new grant asked: nothing of the old
    // tenant survives. The regrant lands in the just-freed slot (first-free), and that is the
    // worst case for staleness, so it is asserted rather than hoped for.
    let regranted =
        sched::grant(rendezvous_capability(ep, Rights::READ)).expect("grant the narrow cap");
    assert_eq!(
        regranted, second,
        "the regrant did not land in the just-freed slot; the staleness attack needs that slot",
    );
    assert_eq!(
        current_capability(regranted)
            .expect("the regranted slot is empty")
            .rights,
        Rights::READ,
        "a slot regranted with READ alone carried a right from its deleted tenant: stale rights \
         survived a delete and a regrant",
    );

    let _ = delete_current_capability(first);
    let _ = delete_current_capability(regranted);
    assert!(
        super::wait_for(|| !sched::is_thread_present(sender)),
        "the sender thread never finished after its message was collected",
    );
    sched::reclaim_region(region).expect("the endpoint's region did not come back");
}
