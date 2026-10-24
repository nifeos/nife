//! A `SEND_CAP` cannot widen a right out of its rights register, and cannot lend a GRANT-less
//! capability (milestone 903 (a seventh outsider pass attacks the confinement claim), fatal risk
//! 7's confinement claim, claims 1 and 2).
//!
//! The host crate proves both claims (`capability::derive_never_widens_rights`,
//! `from_bits_cannot_forge_a_right`), and `confinement_attack_tests` drives neither case; this
//! drives `SEND_CAP`'s `a1` rights register through `syscall::invoke`. `ipc_send_cap_from` refuses
//! before touching the rendezvous, so a refusal parks nothing, and a regression parks the test
//! thread and fails by the watchdog's dump. See notes/confinement-outsider-pass-7.md.

//! Name: provisional (this pass's lane, 2026-10-12 UTC). Cross-ISA: `invoke`, the delegation rule
//! (`Delegation::derive`) and the rendezvous are portable kernel code (DECISIONS §19 (architectural parity is a tenet)).

use abi::Error;

use crate::arch::exceptions::TrapFrame;
// The `cap` module and these names still carry their pre-sweep spelling
// (design/naming/capability-spelling-worklist.md renames them when the tree is quiet); aliased here so
// this file spells `capability` out everywhere a reader looks (calef, 2026-10-06).
use crate::cap::{Rights, rendezvous_cap as rendezvous_capability};
use crate::sched::{self, delete_current_cap as delete_current_capability};

const SEND_CAPABILITY: u64 = abi::rendezvous::SEND_CAP;
use crate::syscall::invoke;

/// `invoke` through the real dispatcher. Copied in shape from `confinement_attack_tests::call`.
fn call(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, a1, a2)
}

/// `SEND_CAP`'s rights register cannot widen a right, and a GRANT-less capability cannot be lent.
///
/// From `READ | GRANT`, a mask adding `WRITE` fails `Delegation::derive`'s subset check; from
/// `READ | WRITE`, `READ` alone fails the GRANT gate. The endpoint cap has `WRITE`, so each
/// `NotPermitted` is the delegation rule's, not the endpoint gate's; no sender is left waiting.
///
/// Falsification: attested 2026-10-12. On patagonia, aarch64, with the subset check in
/// `Delegation::derive` patched to `if false && ...`, the run went red by the watchdog, since the
/// derived copy parks the sender (see the module header). The `GRANT` check was not patched. Not
/// recorded as a replayable patch.
#[test_case]
fn a_delegation_cannot_widen_a_right_or_lend_a_grant_less_capability() {
    let region = crate::memory_region::create(4).expect("no region for the endpoint");
    let ep = sched::create_rendezvous_from(region).expect("no rendezvous");

    // The endpoint capability the delegations are sent over: WRITE, so SEND_CAP clears the endpoint
    // gate and whatever refuses below is the delegation rule, not "you may not send here".
    let writer =
        sched::grant(rendezvous_capability(ep, Rights::WRITE)).expect("grant the WRITE endpoint");

    // --- 1. Widening: forge WRITE out of the a1 rights register. --------------------------------
    // The source: READ and GRANT, but no WRITE. Using the same endpoint object keeps the fixture to
    // one rendezvous; what is under test is the rights on the delegated capability, not which object
    // it names.
    let src_no_write = sched::grant(rendezvous_capability(ep, Rights::READ.union(Rights::GRANT)))
        .expect("grant the READ|GRANT source");
    let widen_mask = Rights::READ
        .union(Rights::WRITE)
        .union(Rights::GRANT)
        .bits() as u64;
    assert_eq!(
        call(writer, SEND_CAPABILITY, src_no_write, widen_mask, 0),
        Err(Error::NotPermitted),
        "SEND_CAP minted a copy carrying WRITE from a source that held only READ|GRANT: a right was \
         forged out of the a1 rights register (claims 1 and 2)",
    );
    assert_eq!(
        sched::rendezvous_waiting_senders(ep),
        0,
        "a refused widening SEND_CAP still parked a sender: the rendezvous was touched before the \
         derivation refused",
    );

    // --- 2. Lending a GRANT-less capability, even narrowed to a strict subset. ------------------
    let src_no_grant = sched::grant(rendezvous_capability(ep, Rights::READ.union(Rights::WRITE)))
        .expect("grant the READ|WRITE (GRANT-less) source");
    let subset_mask = Rights::READ.bits() as u64;
    assert_eq!(
        call(writer, SEND_CAPABILITY, src_no_grant, subset_mask, 0),
        Err(Error::NotPermitted),
        "SEND_CAP lent a GRANT-less capability (even narrowed to READ): a holder passed on a thing \
         it was trusted to use but not to lend",
    );
    assert_eq!(
        sched::rendezvous_waiting_senders(ep),
        0,
        "a refused GRANT-less SEND_CAP still parked a sender: the rendezvous was touched before the \
         derivation refused",
    );

    let _ = delete_current_capability(writer);
    let _ = delete_current_capability(src_no_write);
    let _ = delete_current_capability(src_no_grant);
    sched::reclaim_region(region).expect("the endpoint's region did not come back");
}
