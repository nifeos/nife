//! **Pass 9 (an outside security reviewer attacks the confinement claim): four fresh dispatcher
//! attacks on the rights model** (fatal risk 7's confinement claim).
//!
//! Each test here is a hostile program reduced to the kernel operations it would issue, driven
//! through the real dispatcher (`syscall::invoke`), the way a program's `svc`/`ecall` reaches it.
//! None of them is a replay of an existing test: they come at the claims from angles the suite did
//! not already have.
//!
//! - [`a_consumed_capability_cannot_be_reused`] (claim 4): a `PageFrame` carved from a budget is
//!   sliced once (the positive control), the capability is deleted with `cap_delete`, and the same
//!   slot is invoked again. The answer is `NoSuchSlot`, not a second slice: a consumed capability
//!   names nothing. The existing `revocation_in_flight_tests` attack the *hand-off slot*; this is
//!   the plain `cap_delete` path, which nothing drove through the dispatcher.
//! - [`deleting_one_capability_leaves_its_neighbours`] (claim 5): three capabilities are filed, the
//!   middle one is deleted, and the other two still resolve and still invoke. `capability`'s host
//!   proof `delete_touches_only_its_slot` is over the table type; this is the live kernel table
//!   under the real `SYS_CAP_DELETE` path.
//! - [`a_badge_is_grant_gated_and_mints_no_wider_right`] (claim 26, the delegation-class power
//!   `Rendezvous::BADGE` is): a `WRITE`-only holder cannot mint a badged view at all (no `GRANT`),
//!   and a `WRITE | GRANT` holder's badged copy carries exactly `WRITE | GRANT`, never a bit more.
//!   `BADGE` is the one method that mints a fresh capability to an endpoint from userspace, so a
//!   widening here would be a client minting itself authority; nothing attacked it in a confinement
//!   test.
//! - [`an_out_of_range_or_empty_slot_names_nothing`] (claim 2, no ambient authority): a slot number
//!   past the table and an empty slot inside it both answer `NoSuchSlot`. `confinement_attack_tests`
//!   drives an empty slot; the out-of-range index is the other half, and the one a program reaching
//!   past its table would use.
//!
//! **Nothing here blocks**, so a regression is red at an assertion rather than a watchdog hang: no
//! `SEND`/`RECEIVE`/`CALL` with no peer is issued. Each capability is deleted and each region
//! reclaimed, so the suite's frame and region ledgers are flat across the module.
//!
//! Name: provisional (pass 9's lane, 2026-10-13 UTC), for the module, the file and each function.
//! Cross-ISA: `invoke`, the rights model, the delegation rule and the capability table are portable
//! kernel code, so the same bodies run on each architecture (DECISIONS §19 (architectural parity is
//! a tenet)).

use abi::Error;

use crate::arch::exceptions::TrapFrame;
// The `cap` module and these names still carry their pre-sweep spelling
// (design/naming/capability-worklist.md renames them when the tree is quiet); aliased here so this
// file spells `capability` out everywhere a reader looks (calef, 2026-10-06).
use crate::cap::{
    self as kernel_capability, Object, Rights,
    memory_region_cap_rights as region_capability_rights, rendezvous_cap as rendezvous_capability,
};
use crate::sched::{
    self, current_cap as current_capability, delete_current_cap as delete_current_capability,
};
use crate::syscall::invoke;

/// `invoke` through the real dispatcher. Copied in shape from `confinement_attack_tests::call`.
fn call(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, a1, a2)
}

/// **A capability deleted with `cap_delete` cannot be invoked again** (claim 4, §12 (call/reply
/// IPC: a one-shot reply capability), the decision "a consumed capability cannot be used again"
/// lives under).
///
/// A `PageFrame` is carved from a budget holding `Rights::ALL`, so it carries `GRANT` and
/// `PageFrame::SLICE` is permitted: the positive control is one successful slice. The frame
/// capability is then deleted through the same path `SYS_CAP_DELETE` takes
/// (`delete_current_capability`), and the identical `SLICE` is issued again at the same slot. The
/// second answer is `NoSuchSlot`: there is nothing there, which is what a consumed capability is
/// from the inside. `current_capability` agrees the slot is empty.
///
/// Nothing blocks: `SLICE` does not touch the IPC path.
#[test_case]
fn a_consumed_capability_cannot_be_reused() {
    let region = crate::memory_region::create(8).expect("no region");
    let budget = sched::grant(region_capability_rights(region, Rights::ALL)).expect("grant budget");

    // A two-page frame, so a one-page slice at offset 0 is in range.
    let frame = call(budget, abi::memory_region::RETYPE, 2, 0, 0).expect("RETYPE refused") as u64;

    // Positive control: the frame is real and slicing is permitted (it carries GRANT).
    let slice = call(frame, abi::page_frame::SLICE, 0, 1, 0).expect("SLICE refused a GRANT frame");
    let _ = delete_current_capability(slice as u64);

    // Consume the frame capability, the plain cap_delete a program issues on a handle it is done
    // with.
    delete_current_capability(frame).expect("delete of a held capability failed");

    // The slot now names nothing: the identical invocation answers NoSuchSlot, not a second slice.
    assert_eq!(
        call(frame, abi::page_frame::SLICE, 0, 1, 0),
        Err(Error::NoSuchSlot),
        "a deleted capability was invoked again and the dispatcher acted on it: a consumed \
         capability was reused",
    );
    assert!(
        current_capability(frame).is_err(),
        "the slot of a deleted capability still resolves to an object",
    );

    let _ = delete_current_capability(budget);
    sched::reclaim_region(region).expect("the region did not come back");
}

/// **Dropping one capability disturbs no other** (claim 5, §12).
///
/// Three capabilities are filed into this thread's own table: two endpoint capabilities and a
/// `PageFrame` carved from a budget (the latter so a *working invocation*, not just a table read,
/// proves a neighbour survived). The middle slot is deleted. The surviving two still resolve, and
/// the frame still slices, which is the live-kernel form of `capability::delete_touches_only_its_slot`.
#[test_case]
fn deleting_one_capability_leaves_its_neighbours() {
    let region = crate::memory_region::create(8).expect("no region");
    let budget = sched::grant(region_capability_rights(region, Rights::ALL)).expect("grant budget");
    let ep_region = crate::memory_region::create(4).expect("no endpoint region");
    let ep = sched::create_rendezvous_from(ep_region).expect("no rendezvous");

    let first = sched::grant(rendezvous_capability(ep, Rights::READ)).expect("grant first");
    let middle = sched::grant(rendezvous_capability(ep, Rights::WRITE)).expect("grant middle");
    let frame = call(budget, abi::memory_region::RETYPE, 2, 0, 0).expect("RETYPE refused") as u64;

    // Delete the middle capability only.
    delete_current_capability(middle).expect("delete of the middle capability failed");
    assert!(
        current_capability(middle).is_err(),
        "the deleted middle slot still resolves",
    );

    // The neighbours are untouched: they resolve, and the frame still performs its gated operation.
    assert!(
        current_capability(first).is_ok(),
        "deleting the middle capability emptied a neighbour's slot (claim 5 broke)",
    );
    let slice = call(frame, abi::page_frame::SLICE, 0, 1, 0)
        .expect("a neighbour frame could not slice after an unrelated delete");
    let _ = delete_current_capability(slice as u64);

    let _ = delete_current_capability(first);
    let _ = delete_current_capability(frame);
    let _ = delete_current_capability(budget);
    sched::reclaim_region(ep_region).expect("the endpoint region did not come back");
    sched::reclaim_region(region).expect("the region did not come back");
}

/// **`Rendezvous::BADGE` is `GRANT`-gated and mints no wider right than its source** (claim 26, the
/// delegation-class power of minting a fresh view of an endpoint; §14 (userspace cannot forge a
/// right)).
///
/// `BADGE` is the one method a program can reach that mints a *new* capability to an endpoint it
/// holds, so it is exactly where a client would try to manufacture authority. Two refusals and one
/// bounded success, each through the real dispatcher:
///
/// 1. A `WRITE`-only holder (no `GRANT`) is refused outright: minting a delegatable view is a
///    delegation-class power, gated by `GRANT`.
/// 2. A `WRITE | GRANT` holder mints a badged copy, and that copy carries exactly `WRITE | GRANT`,
///    never `READ` or any bit the source lacked. A badged view is a narrower-or-equal view, as a
///    delegation is.
/// 3. The badged copy cannot be badged again (a badge is set once), and a zero badge is refused,
///    so the method cannot be used to launder rights through repeated minting.
///
/// Nothing blocks: `BADGE` files a capability in the caller's own table and returns the slot.
#[test_case]
fn a_badge_is_grant_gated_and_mints_no_wider_right() {
    let region = crate::memory_region::create(4).expect("no endpoint region");
    let ep = sched::create_rendezvous_from(region).expect("no rendezvous");

    // 1. WRITE alone: no GRANT, so BADGE is refused. The new badge is non-zero and the source is
    //    unbadged, so the only thing that can refuse is the GRANT gate.
    let write_only = sched::grant(rendezvous_capability(ep, Rights::WRITE)).expect("grant write");
    assert_eq!(
        call(write_only, abi::rendezvous::BADGE, 0x9001, 0, 0),
        Err(Error::NotPermitted),
        "a GRANT-less holder minted a badged view: minting a delegatable capability without the \
         right to delegate",
    );

    // 2. WRITE | GRANT: BADGE succeeds, and the minted copy carries exactly WRITE | GRANT.
    let grantor = sched::grant(rendezvous_capability(
        ep,
        Rights::WRITE.union(Rights::GRANT),
    ))
    .expect("grant write|grant");
    let badged = call(grantor, abi::rendezvous::BADGE, 0x9002, 0, 0)
        .expect("BADGE refused a GRANT holder") as u64;
    let minted = current_capability(badged).expect("nothing at the minted slot");
    assert_eq!(
        minted.rights,
        Rights::WRITE.union(Rights::GRANT),
        "a badged copy carried rights its source lacked: BADGE widened on minting (claim 26/§14)",
    );
    match minted.object {
        Object::Rendezvous(named, badge) => {
            assert_eq!(named, ep, "the badged copy named a different endpoint");
            assert_eq!(
                badge, 0x9002,
                "the badge the kernel stamped was not the one asked for"
            );
        }
        other => panic!("BADGE minted a non-rendezvous object: {other:?}"),
    }

    // 3. A badge is set once: the already-badged copy refuses a second badge, and a zero badge is
    //    refused, so the method cannot be run in a loop to launder a rights set.
    assert_eq!(
        call(badged, abi::rendezvous::BADGE, 0x9003, 0, 0),
        Err(Error::NotPermitted),
        "an already-badged capability was badged again: a badge was not write-once",
    );
    assert_eq!(
        call(grantor, abi::rendezvous::BADGE, 0, 0, 0),
        Err(Error::NotPermitted),
        "a zero (unbadged) badge was accepted",
    );

    let _ = delete_current_capability(write_only);
    let _ = delete_current_capability(grantor);
    let _ = delete_current_capability(badged);
    sched::reclaim_region(region).expect("the endpoint region did not come back");
}

/// **A slot past the table, and an empty slot inside it, both name nothing** (claim 2, no ambient
/// authority).
///
/// `confinement_attack_tests` drives an empty in-range slot; this adds the out-of-range index, the
/// one a program reaching past the authority it was handed would actually use. Both answer
/// `NoSuchSlot` (not `NotPermitted`): the lookup is a bounds check into this thread's own table, and
/// there is nothing there in either case. A method that acts is chosen (`SEND`) so a let-through
/// would be observable, but no object exists to act on, so the dispatcher cannot even reach the
/// rights gate.
#[test_case]
fn an_out_of_range_or_empty_slot_names_nothing() {
    // Past the table: the index is >= the slot count, so it can never be occupied.
    let out_of_range = kernel_capability::CAPABILITY_TABLE_SLOTS as u64;
    assert!(
        current_capability(out_of_range).is_err(),
        "an out-of-range slot resolved to an object",
    );
    assert_eq!(
        call(out_of_range, abi::rendezvous::SEND, 1, 0, 0),
        Err(Error::NoSuchSlot),
        "invoking a slot past the table named an object: ambient authority past the table's edge",
    );

    // Empty but in range: the highest valid index, chosen because the test's own fixtures fill from
    // the bottom, so the top is free.
    let empty = kernel_capability::CAPABILITY_TABLE_SLOTS as u64 - 1;
    assert!(
        current_capability(empty).is_err(),
        "the slot chosen as empty was occupied; the fixture filled the top of the table",
    );
    assert_eq!(
        call(empty, abi::rendezvous::SEND, 1, 0, 0),
        Err(Error::NoSuchSlot),
        "invoking an empty in-range slot named an object: ambient authority",
    );
}
