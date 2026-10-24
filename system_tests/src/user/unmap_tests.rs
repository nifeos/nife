//! `unmap_tests`: `abi::address_space::UNMAP` (milestone 95 (an unmap primitive), DECISIONS §162
//! (whether a holder can give up a mapping), option A), driven
//! through the real syscall dispatcher against a real `Object::AddressSpace`, with what the space
//! maps read back from its page tables (`arch::mmu::translate_at`) and its mapping record
//! (`AddressSpace::LIST`) rather than taken from the call's return value. `pmap_tests`' shape and
//! its `tidy` discipline, one method over.
//!
//! Cross-ISA, because the whole of the method is portable kernel code over `mmu::unmap_user_at`,
//! and a divergence here means something under `arch/` disagrees about what unmapping a page is.

//! # BUGS
//!
//! - **No test here watches a TLB on another core.** These four build spaces no thread runs in.
//!   `running_space_tests` has the ones that do, since §249 (a running address space stays
//!   nameable) made a running space nameable: a reader spinning on another core faults once its
//!   page is given up.
//! - **Intermediate tables are not checked.** `UNMAP` leaves them linked on purpose, and this module
//!   does not assert either way.

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::cap::Rights;
use crate::sched;
use crate::syscall::invoke;

/// The space's own budget: root, intermediate tables, mapping log. Nothing here maps more than two
/// pages at once, so this is slack rather than a bound.
const SPACE_PAGES: u64 = 24;

/// Two neighbouring pages in the low half, aligned and far from anything else a test maps.
const VA: u64 = address_space_map::pair_page(0x0060_0000);
const NEXT: u64 = VA + paging::PAGE_SIZE;

/// A fresh space backed by its own region. Returns its name, its root, and the region.
fn space() -> (u64, u64, u64) {
    let region = crate::memory_region::create(SPACE_PAGES).expect("no space region");
    let name = crate::user::user_address_space_create(region).expect("no address space");
    let root = crate::user::user_address_space_root(name).expect("no root");
    (name, root, region)
}

/// A capability to the space `name` carrying `rights` alone: `WRITE` the way a builder holds it,
/// `ENUMERATE` the way `pmap` does.
fn hold(name: u64, rights: Rights) -> u64 {
    sched::grant(crate::cap::address_space_cap(name, rights)).expect("grant the address space")
}

/// A fresh one-page frame from its own region. Returns the slot, the page, and the region.
fn page_frame() -> (u64, u64, u64) {
    let region = crate::memory_region::create(1).expect("no frame region");
    let phys = crate::memory_region::retype_page(region).expect("no frame");
    let slot =
        sched::grant(crate::cap::page_frame_cap(phys, Rights::ALL)).expect("grant the frame");
    (slot, phys, region)
}

/// `pmap_tests::tidy` verbatim: every slot back, then every region, because the suite shares one
/// running kernel.
fn tidy(slots: &[u64], regions: &[u64]) {
    for &s in slots {
        let _ = sched::delete_current_cap(s);
    }
    for &r in regions {
        sched::reclaim_region(r).expect("a region this test carved did not come back");
    }
}

fn map_into(aspace: u64, va: u64, frame: u64) -> Result<i64, Error> {
    let mut fr = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(
        &mut fr,
        aspace,
        abi::address_space::MAP_INTO,
        va,
        frame,
        abi::address_space::MAP_RW,
    )
}

/// `invoke(cap, UNMAP, va, _, _)`, through the real dispatcher.
fn unmap(aspace: u64, va: u64) -> Result<i64, Error> {
    let mut fr = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut fr, aspace, abi::address_space::UNMAP, va, 0, 0)
}

/// What the page tables say `va` maps to in the space rooted at `root`.
fn maps(root: u64, va: u64) -> Option<u64> {
    crate::arch::mmu::translate_at(root, va).map(|(pa, _)| pa)
}

/// Every `va` the space's mapping record lists, through `LIST`'s own engine. Needs `ENUMERATE`,
/// so it is asked through a second, viewer-only capability.
fn listed(name_slot: u64) -> [Option<u64>; 4] {
    let mut out = [None; 4];
    let mut cursor = 0;
    for row in out.iter_mut() {
        let mut fr = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
        let next = invoke(&mut fr, name_slot, abi::address_space::LIST, cursor, 0, 0)
            .expect("LIST refused a viewer");
        if next == 0 {
            return out;
        }
        *row = Some(fr.arg(1));
        cursor = next as u64;
    }
    panic!("the listing outgrew this test's buffer, so it is not the space");
}

/// **`UNMAP` takes exactly one page, and the frame capability survives it.**
///
/// Two frames at neighbouring addresses; `UNMAP` the first. The first is gone from the tables and
/// from the record, the second is untouched in both, and the frame capability the first was mapped
/// under still maps it again at the same address, which is what "the capability survives" has to
/// mean to be worth saying (provisional semantic two, `notes/unmap.md`).
///
/// Falsification: replayable `system_tests/falsifications/user.unmap_tests.unmap_takes_one_page_and_the_frame_capability_survives_it.patch`
#[test_case]
fn unmap_takes_one_page_and_the_frame_capability_survives_it() {
    let (name, root, space_region) = space();
    let aspace = hold(name, Rights::WRITE);
    let viewer = hold(name, Rights::ENUMERATE);
    let (a, a_phys, a_region) = page_frame();
    let (b, b_phys, b_region) = page_frame();

    assert_eq!(
        map_into(aspace, VA, a),
        Ok(0),
        "premise: mapping the first page"
    );
    assert_eq!(
        map_into(aspace, NEXT, b),
        Ok(0),
        "premise: mapping the second page"
    );
    assert_eq!(
        maps(root, VA),
        Some(a_phys),
        "premise: the first page is in the tables"
    );

    assert_eq!(
        unmap(aspace, VA),
        Ok(0),
        "UNMAP of a mapped page was refused"
    );

    assert_eq!(
        maps(root, VA),
        None,
        "UNMAP returned 0 and the page is still in the tables"
    );
    assert_eq!(
        maps(root, NEXT),
        Some(b_phys),
        "UNMAP of one page reached its neighbour"
    );
    assert_eq!(
        listed(viewer),
        [Some(NEXT), None, None, None],
        "the mapping record disagrees with the tables after UNMAP",
    );

    assert!(
        sched::current_cap(a).is_ok(),
        "UNMAP consumed the frame capability, which it names nowhere",
    );
    assert_eq!(
        map_into(aspace, VA, a),
        Ok(0),
        "the surviving frame capability could not map its page again where it was given up",
    );
    assert_eq!(maps(root, VA), Some(a_phys), "the remap did not land");

    tidy(&[aspace, viewer, a, b], &[space_region, a_region, b_region]);
}

/// **A `va` with nothing mapped is refused, and nothing changes** (provisional semantic one,
/// `notes/unmap.md`): never mapped, mapped and already given up, misaligned, and kernel-half all
/// answer `BadPointer`, and the one real mapping in the space survives every one of them.
///
/// Falsification: replayable `system_tests/falsifications/user.unmap_tests.unmap_of_a_va_with_nothing_mapped_is_refused_and_changes_nothing.patch`
#[test_case]
fn unmap_of_a_va_with_nothing_mapped_is_refused_and_changes_nothing() {
    let (name, root, space_region) = space();
    let aspace = hold(name, Rights::WRITE);
    let (a, a_phys, a_region) = page_frame();
    let (b, _, b_region) = page_frame();
    assert_eq!(map_into(aspace, VA, a), Ok(0), "premise: mapping the page");
    assert_eq!(
        map_into(aspace, NEXT, b),
        Ok(0),
        "premise: mapping the page to give up"
    );
    assert_eq!(
        unmap(aspace, NEXT),
        Ok(0),
        "premise: giving the second page up"
    );

    for (va, what) in [
        (VA + 16 * paging::PAGE_SIZE, "a va never mapped"),
        (NEXT, "a va already given up"),
        (VA + 8, "a misaligned va"),
        (0xffff_ffff_ffff_0000, "a kernel-half va"),
    ] {
        assert_eq!(
            unmap(aspace, va),
            Err(Error::BadPointer),
            "UNMAP of {what} did not answer BadPointer",
        );
    }
    assert_eq!(
        maps(root, VA),
        Some(a_phys),
        "a refused UNMAP took a page it did not name"
    );

    tidy(&[aspace, a, b], &[space_region, a_region, b_region]);
}

/// **`UNMAP` takes `WRITE`, the right `MAP_INTO` takes, and a viewer is refused it.** A capability
/// holding `ENUMERATE` alone can list the page and cannot give it up.
///
/// Falsification: replayable `system_tests/falsifications/user.unmap_tests.a_viewer_cannot_unmap.patch`
#[test_case]
fn a_viewer_cannot_unmap() {
    let (name, root, space_region) = space();
    let aspace = hold(name, Rights::WRITE);
    let viewer = hold(name, Rights::ENUMERATE);
    let (a, a_phys, a_region) = page_frame();
    assert_eq!(map_into(aspace, VA, a), Ok(0), "premise: mapping the page");

    assert_eq!(
        unmap(viewer, VA),
        Err(Error::NotPermitted),
        "a viewer was let UNMAP"
    );
    assert_eq!(
        maps(root, VA),
        Some(a_phys),
        "a refused UNMAP still took the page"
    );

    tidy(&[aspace, viewer, a], &[space_region, a_region]);
}

/// **A page given up and the address reused for another frame: revoking the first frame must not
/// reach the second.**
///
/// This is the half of `UNMAP` nobody would see go missing. Taking the page out of the tables
/// alone passes every test above. The record is what a revoke reads, and a record left naming
/// `(a, VA)` sends `PageFrame::REVOKE` of `a` to unmap `VA`, which by then is `b`'s: a mapping made
/// under a capability nobody revoked, taken by a revoke of a different one (DECISIONS §132 (what `PageFrame::REVOKE` owes an overlapping run)).
///
/// Falsification: replayable `system_tests/falsifications/user.unmap_tests.an_unmapped_va_remapped_to_another_frame_survives_the_old_frames_revoke.patch`
/// (drops the record half of `UNMAP`; this test then fails on its last assertion).
#[test_case]
fn an_unmapped_va_remapped_to_another_frame_survives_the_old_frames_revoke() {
    let (name, root, space_region) = space();
    let aspace = hold(name, Rights::WRITE);
    let (a, a_phys, a_region) = page_frame();
    let (b, b_phys, b_region) = page_frame();

    assert_eq!(
        map_into(aspace, VA, a),
        Ok(0),
        "premise: mapping the first frame"
    );
    assert_eq!(unmap(aspace, VA), Ok(0), "premise: giving it up");
    assert_eq!(
        map_into(aspace, VA, b),
        Ok(0),
        "premise: the address reused for a second frame"
    );
    assert_eq!(
        maps(root, VA),
        Some(b_phys),
        "premise: the second frame is what VA maps"
    );

    crate::revoke::revoke_page_frame(a_phys);

    assert_eq!(
        maps(root, VA),
        Some(b_phys),
        "revoking the frame UNMAP gave up took the frame mapped at that address since",
    );

    tidy(&[aspace, a, b], &[space_region, a_region, b_region]);
}
