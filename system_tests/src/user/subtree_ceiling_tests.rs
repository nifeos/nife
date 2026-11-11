//! **Whether a region subtree's ceilings hold through the real `RETYPE_OBJECT` and `SPLIT`.**
//!
//! A root made as the progenitor's construction budget is made, with
//! `memory_region::progenitor_ceilings`, and one region split from it. The test thread holds that
//! region as a program would and creates every kind of kernel object in it through the real
//! dispatcher until each refuses. Each kind must stop at the split child's sixteenth of the root's
//! ceiling while the region still has pages, and a child built in a sibling region afterwards must
//! still start. Reclaiming the subtree must give every count back. See notes/memory-regions.md,
//! "Subtree ceilings".
//!
//! Name: provisional, for the module and the file.

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::capability_object::{Object, memory_region_capability};
use crate::memory_region::{Counted, ObjectKind};
use crate::sched;
use crate::syscall::invoke;

/// A one-instruction loop, as `memory_grant_reclaim_tests` has it: the sibling's child.
#[cfg(target_arch = "aarch64")]
const SPIN_STUB: &[u32] = &[0x1400_0000]; // b .
#[cfg(target_arch = "riscv64")]
const SPIN_STUB: &[u32] = &[0x0000_006F]; // j .  (jal x0, 0)
#[cfg(target_arch = "x86_64")]
const SPIN_STUB: &[u32] = super::x86_programs::SPIN;

/// The root: room for the filled region and the sibling with pages to spare.
const ROOT_PAGES: u64 = 512;
/// The filled region: more pages than every kind's ceiling together, so a ceiling refuses first.
const FILLED_PAGES: u64 = 256;
/// The sibling: one small child.
const SIBLING_PAGES: u64 = 64;
/// More attempts than any split child's ceiling, so a ceiling that fails to hold ends the loop.
const ATTEMPTS: usize = 64;

/// `invoke(slot, method, a0, a1)` through the real dispatcher.
fn call(slot: u64, method: u64, a0: u64, a1: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, a1, 0)
}

/// A split child's ceiling for `counted`, from the root's.
fn share(counted: Counted) -> u32 {
    crate::memory_region::progenitor_ceilings().get(counted) / 16
}

/// `RETYPE_OBJECT` of `objtype` from the region in `budget` until it refuses, deleting each new
/// capability (the object stays in its table, as a program's would), and how many it made.
fn fill(budget: u64, objtype: u64) -> u32 {
    let mut made = 0;
    for _ in 0..ATTEMPTS {
        match call(budget, abi::memory_region::RETYPE_OBJECT, objtype, 0) {
            Ok(slot) => {
                let _ = sched::delete_current_capability(slot as u64);
                made += 1;
            }
            Err(e) => {
                assert_eq!(e, Error::OutOfMemory, "objtype {objtype} refused for {e:?}");
                break;
            }
        }
    }
    made
}

/// Make processes, each from a fresh space, until a ceiling refuses, then bare spaces until the
/// space ceiling does: `(processes, spaces)` made. A process consumes a space, so both ceilings
/// meet here. Each capability is deleted as it is made.
fn fill_processes_and_spaces(budget: u64) -> (u32, u32) {
    let mut processes = 0;
    for _ in 0..ATTEMPTS {
        let Ok(space) = call(
            budget,
            abi::memory_region::RETYPE_OBJECT,
            abi::objtype::ADDRESS_SPACE,
            0,
        ) else {
            break;
        };
        match call(
            budget,
            abi::memory_region::RETYPE_OBJECT,
            abi::objtype::PROCESS,
            space as u64,
        ) {
            Ok(slot) => {
                let _ = sched::delete_current_capability(slot as u64);
                processes += 1;
            }
            Err(e) => {
                assert_eq!(e, Error::OutOfMemory);
                let _ = sched::delete_current_capability(space as u64);
                break;
            }
        }
    }
    (
        processes,
        processes + fill(budget, abi::objtype::ADDRESS_SPACE),
    )
}

/// `SPLIT` one page from the region in `budget` until it refuses, deleting each new capability and
/// writing the names into `below`: how many it made.
fn split_until_refused(budget: u64, below: &mut [u64; ATTEMPTS]) -> usize {
    let mut splits = 0;
    for _ in 0..ATTEMPTS {
        let Ok(slot) = call(budget, abi::memory_region::SPLIT, 1, 0) else {
            break;
        };
        let Object::MemoryRegion(name) = sched::current_capability(slot as u64)
            .expect("the split's slot")
            .object
        else {
            panic!("SPLIT filed something other than a region");
        };
        below[splits] = name;
        splits += 1;
        let _ = sched::delete_current_capability(slot as u64);
    }
    splits
}

/// **A filled subtree stops at its ceilings with pages to spare, a sibling still builds, and the
/// reclaim returns every count.**
///
/// Falsification: replayable `system_tests/falsifications/user.subtree_ceiling_tests.a_region_filled_to_its_subtree_ceilings_leaves_its_siblings_able_to_build.patch`
#[test_case]
fn a_region_filled_to_its_subtree_ceilings_leaves_its_siblings_able_to_build() {
    let root = crate::memory_region::create_with_ceilings(
        ROOT_PAGES,
        crate::memory_region::progenitor_ceilings(),
    )
    .expect("no root region");
    let filled = crate::memory_region::split(root, FILLED_PAGES).expect("no region to fill");
    let budget = sched::grant(memory_region_capability(filled)).expect("grant the filled region");

    for (objtype, kind) in [
        (abi::objtype::RENDEZVOUS, ObjectKind::Rendezvous),
        (abi::objtype::NOTIFICATION, ObjectKind::Notification),
        (abi::objtype::TIMER, ObjectKind::Timer),
        (abi::objtype::THREAD_CONTROL_BLOCK, ObjectKind::Thread),
    ] {
        assert_eq!(
            fill(budget, objtype),
            share(Counted::Object(kind)),
            "{kind:?} did not stop at the split child's ceiling",
        );
    }

    let (processes, spaces) = fill_processes_and_spaces(budget);
    assert_eq!(spaces, share(Counted::Object(ObjectKind::AddressSpace)));
    assert_eq!(
        processes,
        share(Counted::Object(ObjectKind::Process)).min(spaces)
    );

    let mut below = [0u64; ATTEMPTS];
    let splits = split_until_refused(budget, &mut below);
    assert_eq!(splits as u32, share(Counted::Region));

    let (spent, pages) = crate::memory_region::usage(filled).expect("the filled region");
    assert!(
        spent < pages,
        "the region ran out of pages ({spent} of {pages}), so no ceiling was shown to refuse",
    );

    // Outside the filled subtree, a program still gets every object a start needs.
    let sibling = crate::memory_region::split(root, SIBLING_PAGES).expect("no sibling region");
    let _child = super::supervision_tests::build_child_with(sibling, SPIN_STUB, &[], None);

    // Tidy, and the counts must all come home.
    let _ = sched::delete_current_capability(budget);
    for &name in &below[..splits] {
        sched::reclaim_region(name).expect("a one-page region split from the filled one");
    }
    sched::reclaim_region(filled).expect("the filled region");
    let deadline = crate::arch::timer::now() + crate::arch::timer::frequency();
    while sched::reclaim_region(sibling).is_err() {
        assert!(
            crate::arch::timer::now() < deadline,
            "the sibling's child was never reclaimed"
        );
        sched::yield_now();
    }
    for counted in [
        Counted::Object(ObjectKind::Rendezvous),
        Counted::Object(ObjectKind::AddressSpace),
        Counted::Object(ObjectKind::Thread),
        Counted::Object(ObjectKind::Notification),
        Counted::Object(ObjectKind::Timer),
        Counted::Object(ObjectKind::Process),
        Counted::Region,
    ] {
        assert_eq!(
            crate::memory_region::live_and_ceiling(root, counted).map(|(live, _)| live),
            Some(0),
            "{counted:?} outlived the subtree that held it",
        );
    }
    sched::reclaim_region(root).expect("the root");
}
