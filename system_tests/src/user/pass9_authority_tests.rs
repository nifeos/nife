//! **Pass 9 (an outside security reviewer attacks the confinement claim): a holder that drops its
//! construction authority cannot build again** (fatal risk 7's confinement claim, claim 23;
//! DECISIONS §26, §12 (a consumed capability cannot be used again)).
//!
//! `authority_tests::init_drops_its_construction_authority_and_cannot_build_again` boots the real
//! `root_supervisor` program, which deletes its untyped and reports that a later build answered
//! `NoSuchSlot`. That end-to-end test needs the initial ramdisk. This comes at the same claim
//! through the dispatcher, with no program: a thread holds an untyped budget carrying `Rights::ALL`
//! (the construction authority the progenitor is handed), *uses* it to build (the positive control:
//! a `RETYPE` and a `SPLIT` both succeed), deletes the budget capability (the drop the progenitor
//! performs on itself), and then issues the identical build operations again. Both answer
//! `NoSuchSlot`: the slot is empty, not merely restricted, so there is no authority left to rebuild
//! from. A capability-model system where "drop your authority" left a usable handle behind would be
//! a progenitor that could rebuild the world after promising it had thrown the key away.
//!
//! This is the mechanism `authority_tests` exercises end to end, driven one layer down so it runs on
//! every architecture without a ramdisk: `SYS_CAP_DELETE` is what the progenitor's self-drop calls,
//! and `RETYPE`/`SPLIT` through an empty slot is what its later build hits.
//!
//! Nothing here blocks (no `SEND`/`RECEIVE`/`CALL` with no peer), so a regression is red at an
//! assertion rather than a watchdog hang, and each capability and region is reclaimed, so the
//! suite's ledgers are flat across the module.
//!
//! Name: provisional (pass 9's lane, 2026-10-13 UTC), for the module, the file and each function.
//! Cross-ISA: untyped retype and split and the capability table are portable kernel code
//! (DECISIONS §19 (architectural parity is a tenet)).

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::cap::{Object, Rights, memory_region_cap_rights as memory_region_capability_rights};
use crate::sched::{
    self, current_cap as current_capability, delete_current_cap as delete_current_capability,
};
use crate::syscall::invoke;

/// `invoke` through the real dispatcher. Copied in shape from `pass9_confinement_tests::call`.
fn call(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, a1, a2)
}

/// **Dropping the construction authority leaves no handle to build from** (claim 23).
///
/// The budget carries `Rights::ALL`, so it is a full construction authority: it can split itself and
/// retype its pages into objects, which are the two powers a progenitor builds the system with. Both
/// succeed first (the positive control: the authority is real and building works), and the products
/// are torn down so the ledgers stay flat. Then the budget capability is deleted through the same
/// path `SYS_CAP_DELETE` takes, and the identical `RETYPE` and `SPLIT` are issued at the now-empty
/// slot. Both answer `NoSuchSlot`, the slot resolving to nothing, which is what "the authority was
/// dropped" has to mean for the claim to hold.
#[test_case]
fn dropping_the_construction_authority_leaves_nothing_to_build_from() {
    let region = crate::memory_region::create(16).expect("no region");
    let budget = sched::grant(memory_region_capability_rights(region, Rights::ALL))
        .expect("grant the construction authority");

    // Positive control 1: the authority can retype a page into a frame.
    let frame = call(budget, abi::memory_region::RETYPE, 2, 0, 0)
        .expect("RETYPE refused a full construction authority") as u64;

    // Positive control 2: the authority can split itself into a child budget.
    let child = call(budget, abi::memory_region::SPLIT, 2, 0, 0)
        .expect("SPLIT refused a full construction authority") as u64;
    let Object::MemoryRegion(child_region) = current_capability(child)
        .expect("the SPLIT child is gone")
        .object
    else {
        panic!("the SPLIT child was not a MemoryRegion");
    };

    // Tear the products down so the ledgers are flat: the child region before the parent, the frame
    // capability back to its region.
    sched::reclaim_region(child_region).expect("the split child region did not come back");
    let _ = delete_current_capability(child);
    let _ = delete_current_capability(frame);

    // Drop the construction authority: the self-delete the progenitor performs.
    delete_current_capability(budget).expect("delete of the construction authority failed");
    assert!(
        current_capability(budget).is_err(),
        "the dropped authority's slot still resolves to an object",
    );

    // Build again. Both powers are gone, and the slot names nothing: NoSuchSlot, not a restricted
    // success and not a different error.
    assert_eq!(
        call(budget, abi::memory_region::RETYPE, 2, 0, 0),
        Err(Error::NoSuchSlot),
        "a RETYPE succeeded through the dropped construction authority: the progenitor could \
         rebuild after promising it had thrown the authority away",
    );
    assert_eq!(
        call(budget, abi::memory_region::SPLIT, 2, 0, 0),
        Err(Error::NoSuchSlot),
        "a SPLIT succeeded through the dropped construction authority: the drop left a usable \
         handle behind",
    );

    sched::reclaim_region(region).expect("the region did not come back");
}
