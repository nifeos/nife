//! **A holder that drops its construction authority cannot build again** (a ninth outsider pass at
//! the confinement claim, claim 23; DECISIONS §26 (the fault endpoint), §12 (call/reply IPC: a
//! one-shot reply capability)).
//!
//! `authority_tests::the_root_supervisor_drops_its_construction_authority_and_cannot_build_again`
//! proves this end to end through the real `root_supervisor`, which needs the initial ramdisk. This
//! drives the same mechanism through the dispatcher with no program, so it runs on every
//! architecture: `SYS_CAP_DELETE` is the progenitor's self-drop, and `RETYPE`/`SPLIT` through an
//! empty slot is its later build. See notes/confinement-outsider-pass-9.md, "The kernel tests".
//!
//! Name: provisional (lane/pass-9-tests, 2026-10-14 UTC), for the module, the file and each
//! function. Cross-ISA: untyped retype and split and the capability table are portable kernel code
//! (DECISIONS §19 (architectural parity is a tenet)).

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::cap::{Object, Rights, memory_region_cap_rights as memory_region_capability_rights};
use crate::sched::{
    self, current_cap as current_capability, delete_current_cap as delete_current_capability,
};
use crate::syscall::invoke;

/// `invoke` through the real dispatcher. Copied in shape from `ninth_pass_confinement_tests::call`.
fn call(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, a1, a2)
}

/// **Dropping the construction authority leaves no handle to build from** (claim 23).
///
/// A budget carrying `Rights::ALL` is a full construction authority: it can `SPLIT` itself and
/// `RETYPE` pages into objects, the two powers a progenitor builds with. Both succeed first (the
/// positive control), and the products are torn down so the ledgers stay flat. The budget is then
/// deleted on the `SYS_CAP_DELETE` path, and the identical calls answer `NoSuchSlot`: the slot
/// resolves to nothing, which is what "the authority was dropped" has to mean for the claim.
///
/// Falsification: unfalsified. Written by the ninth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn dropping_the_construction_authority_leaves_nothing_to_build_from() {
    let region = crate::memory_region::create(16).expect("no region");
    let budget = sched::grant(memory_region_capability_rights(region, Rights::ALL))
        .expect("grant the construction authority");

    // Positive control 1: the authority can retype a page into a frame.
    let frame = call(budget, abi::memory_region::RETYPE, 0, 0, 0)
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
        call(budget, abi::memory_region::RETYPE, 0, 0, 0),
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
