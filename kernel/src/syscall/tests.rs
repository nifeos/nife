use super::*;
use crate::arch::mmu;

/// **A run that fails partway through leaves nothing mapped**, driven through the real `MAP_INTO`
/// handler: after the error, page 0 is not mapped and the squatter's page still is.
///
/// The run's middle virtual address is occupied first, so the loop's second iteration takes
/// `AlreadyMapped`: the cheapest deterministic mid-run failure, and one that depends on no
/// arithmetic about region sizes.
#[test_case]
fn a_partly_mapped_run_is_rolled_all_the_way_back() {
    let mut trap = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);

    let space_region = crate::memory_region::create(16).expect("no space region");
    let name = crate::user::user_address_space_create(space_region).expect("no address space");
    let root = crate::user::user_address_space_root(name).expect("no root");
    let space_slot = sched::grant(crate::cap::address_space_cap(name, Rights::ALL))
        .expect("grant the address space");

    let frame_region = crate::memory_region::create(8).expect("no frame region");
    let base = crate::memory_region::retype_page(frame_region).expect("retype 0");
    let second = crate::memory_region::retype_page(frame_region).expect("retype 1");
    let third = crate::memory_region::retype_page(frame_region).expect("retype 2");
    assert_eq!(second, base + paging::PAGE_SIZE, "run must be contiguous");
    assert_eq!(third, second + paging::PAGE_SIZE, "run must be contiguous");
    let frame_slot = sched::grant(crate::cap::page_frame_run_cap(
        base,
        crate::cap::page_frame_run_len(3),
        Rights::ALL,
    ))
    .expect("grant the run");

    // Occupy the middle of where the run wants to go, out of a page the run does not name.
    let va = 0x40_0000u64;
    let squatter = crate::memory_region::retype_page(frame_region).expect("retype squatter");
    crate::user::user_address_space_map(
        name,
        va + paging::PAGE_SIZE,
        squatter,
        paging::Flags::user_data(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .expect("the squatter maps");

    let outcome = invoke(
        &mut trap,
        space_slot,
        abi::address_space::MAP_INTO,
        va,
        frame_slot,
        abi::address_space::MAP_RW,
    );
    assert_eq!(
        outcome,
        Err(Error::BadPointer),
        "MAP_INTO over an occupied page must refuse",
    );
    assert!(
        mmu::translate_at(root, va).is_none(),
        "a failed MAP_INTO left the run's first page mapped: silent partial state",
    );
    assert_eq!(
        mmu::translate_at(root, va + paging::PAGE_SIZE).map(|(phys, _)| phys),
        Some(squatter),
        "the rollback unmapped a page the failed call never mapped",
    );

    // The space's region is pinned by its root page, so it comes back through `reclaim_region`,
    // which reaps the space, not through `destroy`.
    let _ = sched::delete_current_cap(space_slot);
    let _ = sched::delete_current_cap(frame_slot);
    let _ = sched::reclaim_region(space_region);
    crate::memory_region::destroy(frame_region);
}

/// **A slice maps only its window** (milestone 599, calef's option-4 ruling of 2026-09-27).
/// The progenitor's pool is one capability over every client's window; `SLICE` is what lets it
/// hand a client exactly one. Through the real handlers: slice page 2 of a four-page pool,
/// `MAP_INTO` a space with the slice, and the space holds that page and none of its
/// neighbours. Then the refusals: out of range, empty, and without `GRANT`.
#[test_case]
fn a_slice_maps_only_its_window() {
    let mut trap = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    let space_region = crate::memory_region::create(16).expect("no space region");
    let name = crate::user::user_address_space_create(space_region).expect("no address space");
    let root = crate::user::user_address_space_root(name).expect("no root");
    let space_slot = sched::grant(crate::cap::address_space_cap(name, Rights::ALL))
        .expect("grant the address space");

    let frame_region = crate::memory_region::create(4).expect("no frame region");
    let region_slot =
        sched::grant(crate::cap::memory_region_root_cap(frame_region)).expect("grant");
    let pool = invoke(&mut trap, region_slot, abi::memory_region::RETYPE, 4, 0, 0)
        .expect("a four-page pool") as u64;
    let Object::PageFrame(base, _) = sched::current_cap(pool).expect("the pool").object else {
        panic!("RETYPE must mint a PageFrame");
    };

    let slice = invoke(&mut trap, pool, abi::page_frame::SLICE, 2, 1, 0)
        .expect("page 2 of four is inside the pool") as u64;
    let cap = sched::current_cap(slice).expect("the slice");
    assert_eq!(
        cap.object,
        Object::PageFrame(
            base + 2 * paging::PAGE_SIZE,
            crate::cap::page_frame_run_len(1)
        ),
        "a slice names exactly the pages asked for",
    );

    // Map the slice, then look at the window and both neighbours' would-be addresses.
    let va = 0x40_0000u64;
    invoke(
        &mut trap,
        space_slot,
        abi::address_space::MAP_INTO,
        va,
        slice,
        abi::address_space::MAP_RW,
    )
    .expect("the slice maps");
    assert_eq!(
        mmu::translate_at(root, va).map(|(phys, _)| phys),
        Some(base + 2 * paging::PAGE_SIZE),
        "the slice mapped its own page",
    );
    for off in [va - paging::PAGE_SIZE, va + paging::PAGE_SIZE] {
        assert!(
            mmu::translate_at(root, off).is_none(),
            "a slice's mapping reached past its window",
        );
    }

    // Refusals: past the end, empty, overflowing, and a source without GRANT.
    for (first, len) in [(3, 2), (4, 1), (1, 0), (u64::MAX, 2)] {
        assert_eq!(
            invoke(&mut trap, pool, abi::page_frame::SLICE, first, len, 0),
            Err(Error::BadPointer),
            "slice ({first}, {len}) of a four-page pool",
        );
    }
    let narrow = sched::grant(crate::cap::page_frame_run_cap(
        base,
        crate::cap::page_frame_run_len(4),
        Rights::WRITE,
    ))
    .expect("a pool without GRANT");
    assert_eq!(
        invoke(&mut trap, narrow, abi::page_frame::SLICE, 0, 1, 0),
        Err(Error::NotPermitted),
        "slicing without GRANT",
    );

    for s in [space_slot, slice, narrow, pool, region_slot] {
        let _ = sched::delete_current_cap(s);
    }
    let _ = sched::reclaim_region(space_region);
    crate::memory_region::destroy(frame_region);
}

/// **`RETYPE` mints a run, `0` still means one page, and a run that does not fit moves
/// nothing** (calef's ruling of 2026-09-26, option A of
/// design/roadmap/0659-a-region-retypes-a-frame-run.md). Through the real handler, so the
/// argument reaches the proved arithmetic and the capability names the whole run.
#[test_case]
fn retype_mints_a_run_and_a_refused_run_moves_nothing() {
    let mut trap = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    let region = crate::memory_region::create(4).expect("a region to retype from");
    let slot = sched::grant(crate::cap::memory_region_root_cap(region)).expect("grant it");

    let run = invoke(&mut trap, slot, abi::memory_region::RETYPE, 3, 0, 0)
        .expect("three pages fit in four") as u64;
    let Object::PageFrame(phys, count) = sched::current_cap(run).expect("the run's cap").object
    else {
        panic!("RETYPE must mint a PageFrame");
    };
    assert_eq!(count.get(), 3, "one capability names the whole run");
    // SAFETY: the run's pages are ours, retyped above; reading the direct map is how the zeroing
    // is observed rather than assumed.
    let zeroed = (0..3 * paging::PAGE_SIZE).step_by(512).all(|off| unsafe {
        core::ptr::read_volatile((mmu::phys_to_virt(phys) + off) as *const u64) == 0
    });
    assert!(
        zeroed,
        "every page of the run must be zeroed, not only the first"
    );
    assert_eq!(crate::memory_region::usage(region), Some((3, 4)));

    assert_eq!(
        invoke(&mut trap, slot, abi::memory_region::RETYPE, 2, 0, 0),
        Err(Error::OutOfMemory),
        "two pages do not fit in one",
    );
    assert_eq!(
        crate::memory_region::usage(region),
        Some((3, 4)),
        "a refused run moved the region's watermark",
    );

    let one = invoke(&mut trap, slot, abi::memory_region::RETYPE, 0, 0, 0)
        .expect("the last page, asked for the way every existing caller asks") as u64;
    let Object::PageFrame(last, one_count) = sched::current_cap(one).expect("cap").object else {
        panic!("RETYPE must mint a PageFrame");
    };
    assert_eq!(one_count.get(), 1, "0 means one page");
    assert_eq!(
        last,
        phys + 3 * paging::PAGE_SIZE,
        "the watermark is contiguous"
    );

    let _ = sched::delete_current_cap(run);
    let _ = sched::delete_current_cap(one);
    let _ = sched::delete_current_cap(slot);
    crate::memory_region::destroy(region);
}

/// **`SPLIT` never widens rights: a spend-only untyped splits into spend-only children.** SPLIT
/// gates only on `WRITE`, and mints a fresh capability to the child budget, so it must honor the
/// derive-never-widens invariant by hand or a process could manufacture authority it was denied:
/// hold a deliberately `GRANT`-less untyped, `SPLIT` it, and receive a `GRANT`-bearing child over
/// the same memory, then delegate what its own capability could not. This drives the real syscall
/// path (not the region-level `memory_region::split`, which carries no rights) and pins the child's
/// rights to the parent capability's, at the mint site the Kani proofs do not cover. The contrast
/// arm shows the delegable root does pass `GRANT` down, so the inheritance is real, not a blanket
/// deny. Milestone 31; see DECISIONS §16 amendment.
#[test_case]
fn split_inherits_the_parent_capabilitys_rights_never_widening() {
    // `for_user_entry` is the portable frame constructor (both ISAs); SPLIT does not read it.
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);

    // A spend-only (every right but GRANT) untyped, exactly what a leaf child is handed.
    let spend_only_region = crate::memory_region::create(8).expect("a region to split");
    let parent =
        sched::grant(crate::cap::memory_region_cap(spend_only_region)).expect("grant parent");
    assert!(
        !sched::current_cap(parent)
            .unwrap()
            .rights
            .allows(Rights::GRANT),
        "the parent capability must lack GRANT for this test to mean anything",
    );

    // SPLIT through the real handler. WRITE permits it; the child must inherit WRITE only.
    let child_slot = invoke(&mut frame, parent, abi::memory_region::SPLIT, 2, 0, 0)
        .expect("split succeeds: the parent holds WRITE") as u64;
    let child = sched::current_cap(child_slot).expect("the child capability exists");
    assert!(
        !child.rights.allows(Rights::GRANT),
        "escalation: a GRANT-less untyped split into a GRANT-bearing child",
    );
    // Without GRANT it cannot be delegated: `SEND_CAP` and `CAP_INSERT` both refuse.
    assert!(
        !child.rights.allows(Rights::GRANT),
        "a spend-only child must be un-delegatable",
    );
    let Object::MemoryRegion(child_region) = child.object else {
        panic!("SPLIT must mint an untyped");
    };

    // Contrast: the delegable root (READ|WRITE|GRANT, what the progenitor holds) passes GRANT to its
    // children, so a spawner can hand a budget on. Inheritance, not a blanket deny.
    let root_region = crate::memory_region::create(8).expect("a root region");
    let root = sched::grant(crate::cap::memory_region_root_cap(root_region)).expect("grant root");
    let root_child_slot = invoke(&mut frame, root, abi::memory_region::SPLIT, 2, 0, 0)
        .expect("split the root") as u64;
    let root_child = sched::current_cap(root_child_slot).expect("root child exists");
    assert!(
        root_child.rights.allows(Rights::GRANT),
        "a delegable root must split into delegable children",
    );
    let Object::MemoryRegion(root_child_region) = root_child.object else {
        panic!("SPLIT must mint an untyped");
    };

    // Children before parents: each child is the top of its parent's watermark.
    sched::reclaim_region(child_region).expect("reclaim the spend-only child");
    sched::reclaim_region(spend_only_region).expect("reclaim the spend-only parent");
    sched::reclaim_region(root_child_region).expect("reclaim the root child");
    sched::reclaim_region(root_region).expect("reclaim the root parent");
    for slot in [parent, child_slot, root, root_child_slot] {
        let _ = sched::delete_current_cap(slot);
    }
}

/// **A `SPLIT` refused for a full capability table leaves the parent as it found it.** The
/// child is minted before its capability, so a caller whose table has no free slot used to get
/// `OutOfMemory` with an orphaned child still live: a region slot nobody could name, and a
/// parent that `reclaim_region` refused for the rest of the boot. The last assertion is the
/// one that failed before the fix; the `usage` check pins that the carve was given back too,
/// not merely uncounted. Milestone 601 (the region table prints its peak).
#[test_case]
fn split_refused_for_a_full_capability_table_orphans_no_child() {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    let region = crate::memory_region::create(8).expect("a region to split");
    let parent = sched::grant(crate::cap::memory_region_root_cap(region)).expect("grant");

    // Fill every remaining slot. Bounded by the table's capacity, which `grant` enforces.
    let mut filler = [0u64; crate::cap::CAPABILITY_TABLE_SLOTS];
    let mut filled = 0;
    while let Ok(slot) = sched::grant(crate::cap::memory_region_root_cap(region)) {
        filler[filled] = slot;
        filled += 1;
    }

    let refused = invoke(&mut frame, parent, abi::memory_region::SPLIT, 2, 0, 0);
    assert_eq!(
        refused,
        Err(Error::OutOfMemory),
        "no slot for the child's capability"
    );
    assert!(
        !crate::memory_region::has_children(region),
        "the refused split left a child nobody holds a capability to",
    );
    assert_eq!(
        crate::memory_region::usage(region),
        Some((0, 8)),
        "the child's pages went back to the parent",
    );

    for &slot in filler[..filled].iter().chain([&parent]) {
        let _ = sched::delete_current_cap(slot);
    }
    sched::reclaim_region(region).expect("the parent is reclaimable, as if SPLIT never ran");
}
