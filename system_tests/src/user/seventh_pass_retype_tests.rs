//! A spend-only (GRANT-less) budget narrows a SPLIT child and every RETYPE alike (milestone 903
//! (a seventh outsider pass attacks the confinement claim), fatal risk 7's confinement claim,
//! claim 3).
//!
//! Claim 3 ("a budget that cannot delegate cannot split itself a child that can") is proved on its
//! SPLIT side by `capability::split_never_widens_rights` and on its retype side by
//! `confinement_attack_tests::a_retype_mints_no_right_its_budget_lacks`. This module puts the two
//! side by side in one boot through `syscall::invoke`: the same WRITE-only budget shape is SPLIT
//! and RETYPEd, and both come back WRITE-only. One body serves every ISA (DECISIONS §19
//! (architectural parity is a tenet)).

//! Name: provisional (this pass's lane, 2026-10-12 UTC), for the module and the file; a rename is an
//! architect's call.

use abi::Error;

use crate::arch::exceptions::TrapFrame;
// The `cap` module and these names still carry their pre-sweep spelling
// (design/naming/capability-spelling-worklist.md renames them when the tree is quiet); aliased here so
// this file spells `capability` out everywhere a reader looks (calef, 2026-10-06).
use crate::cap::{Object, Rights, memory_region_cap_rights as memory_region_capability_rights};
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

/// The rights on the capability at `slot`, or a panic naming the test's own mistake if the slot is
/// empty (the attack body always files something first).
fn rights_at(slot: u64) -> Rights {
    current_capability(slot)
        .expect("no capability at the slot the attack just filled")
        .rights
}

/// A `WRITE`-only budget's `SPLIT` child is `WRITE`-only, and so is everything a `RETYPE` carves
/// off the same budget, a page frame and three kernel object kinds alike.
///
/// The SPLIT step is the positive control and the claim's cited property: `MemoryRegion::SPLIT`
/// mints through `Cap::mint_child`, which inherits the parent's rights, and `RETYPE` and
/// `RETYPE_OBJ` mint through the same function. Nothing blocks, so a regression fails at an
/// assertion; every minted capability is deleted and region reclaimed, keeping the ledgers flat.
///
/// Name: provisional. Renamed by milestone 824's lane on 2026-10-12 (UTC) from
/// `a_grant_less_budget_narrows_a_split_child_but_mints_full_rights_on_every_retype`, which the fix
/// made false.
///
/// Falsification: replayable `system_tests/falsifications/user.seventh_pass_retype_tests.a_grant_less_budget_narrows_a_split_child_and_every_retype_alike.patch`
#[test_case]
fn a_grant_less_budget_narrows_a_split_child_and_every_retype_alike() {
    // --- The SPLIT side: claim 3's cited property, which holds. ---------------------------------
    let region_b = crate::memory_region::create(8).expect("no region for the split side");
    let budget_b = sched::grant(memory_region_capability_rights(region_b, Rights::WRITE))
        .expect("grant the WRITE-only budget");
    assert_eq!(
        rights_at(budget_b),
        Rights::WRITE,
        "the leaf budget should be WRITE-only, with no GRANT to inherit",
    );
    let child_slot = call(budget_b, abi::memory_region::SPLIT, 2, 0, 0)
        .expect("SPLIT refused a WRITE-only budget") as u64;
    assert_eq!(
        rights_at(child_slot),
        Rights::WRITE,
        "a GRANT-less budget SPLIT itself a child carrying more than WRITE: claim 3's SPLIT side \
         broke, a non-delegating budget manufactured a delegating child",
    );
    // Teardown the split side: destroy the child region first (a parent with children refuses
    // reclaim), then the parent.
    let Object::MemoryRegion(child_region) = current_capability(child_slot).unwrap().object else {
        panic!("the SPLIT child was not a MemoryRegion");
    };
    sched::reclaim_region(child_region).expect("the split child region did not come back");
    let _ = delete_current_capability(child_slot);
    let _ = delete_current_capability(budget_b);
    sched::reclaim_region(region_b).expect("the split-side region did not come back");

    // --- The RETYPE side, for a page frame and three object kinds. ------------------------------
    let region_a = crate::memory_region::create(16).expect("no region for the retype side");
    let budget_a = sched::grant(memory_region_capability_rights(region_a, Rights::WRITE))
        .expect("grant the WRITE-only budget");
    assert_eq!(
        rights_at(budget_a),
        Rights::WRITE,
        "the leaf budget should be WRITE-only"
    );

    // A page frame.
    let frame_slot = call(budget_a, abi::memory_region::RETYPE, 0, 0, 0)
        .expect("RETYPE refused a WRITE-only budget") as u64;
    assert_eq!(
        rights_at(frame_slot),
        Rights::WRITE,
        "a WRITE-only budget RETYPEd a frame carrying more than WRITE: the retype side of claim 3 \
         is open again, and a non-delegating budget can lend what it carves",
    );

    // Three kernel object kinds via RETYPE_OBJ, the case the pass found the frame-only test did not
    // reach. Each must come back WRITE-only.
    for &(kind, name) in &[
        (abi::objtype::RENDEZVOUS, "rendezvous"),
        (abi::objtype::NOTIFICATION, "notification"),
        (abi::objtype::TIMER, "timer"),
    ] {
        let obj_slot = call(budget_a, abi::memory_region::RETYPE_OBJ, kind, 0, 0)
            .unwrap_or_else(|e| panic!("RETYPE_OBJ {name} refused a WRITE-only budget: {e:?}"))
            as u64;
        assert_eq!(
            rights_at(obj_slot),
            Rights::WRITE,
            "a WRITE-only budget RETYPE_OBJ'd a {name} carrying more than WRITE: a GRANT-less \
             budget minted a delegatable {name} in its own memory",
        );
        let _ = delete_current_capability(obj_slot);
    }

    let _ = delete_current_capability(frame_slot);
    let _ = delete_current_capability(budget_a);
    sched::reclaim_region(region_a).expect("the retype-side region did not come back");
}
