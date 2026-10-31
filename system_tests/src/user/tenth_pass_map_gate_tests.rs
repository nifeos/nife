//! **The user/kernel boundary of the mapping paths, and W^X against an alias** (a tenth outsider
//! pass at the confinement claim, claims 10 and 11, and milestone 969 (no frame is writable and
//! executable in one address space)).
//!
//! The boundary *edges* of `AddressSpace::MAP_INTO` (the last user page, a run whose second page
//! lands in the kernel half, a straddling run's all-or-none undo), the same-address overlay, and
//! the alias one level above the per-leaf proofs: one frame mapped writable at one address and as
//! code at another in the same space. The mapping hold refuses that second mapping, through
//! whichever path it comes, with `NotPermitted`; the pairs that do not make a page writable and
//! executable in one space pass, and so does the cross-space pair, which claim 11 states it does
//! not cover.
//!
//! Name: provisional (the tenth pass's lane, 2026-10-27 UTC, and milestone 969's lane, 2026-10-31
//! UTC), for the module, the file and each function. Cross-ISA: `MAP_INTO`, `PageFrame::MAP`, the
//! gate and the flags are instantiated at the running architecture's `Format` (DECISIONS §19
//! (architectural parity is a tenet)).
//!
//! Attacks rows: 10, 11

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::arch::mmu;
use crate::capability_object::{
    Rights, address_space_capability, memory_region_capability,
    memory_region_capability_rights as region_capability_rights, page_frame_run_capability,
    page_frame_run_len,
};
use crate::sched;
use crate::syscall::invoke;

/// `invoke` through the real dispatcher, the way a program's `svc`/`ecall` reaches it.
fn call(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, a1, a2)
}

/// The leaf flags at `va` in the space named `name`, or `None` when nothing is mapped there.
fn flags_at(name: u64, va: u64) -> Option<paging::Flags> {
    let root = crate::user::user_address_space_root(name).expect("the space stopped resolving");
    mmu::translate_at(root, va).map(|(_, flags)| flags)
}

/// Map one page at `va` shaped by `shape` through the dispatcher.
fn map_one_page(space: u64, frame: u64, va: u64, shape: u64) -> Result<(), Error> {
    call(space, abi::address_space::MAP_INTO, va, frame, shape).map(|_| ())
}

/// **`MAP_INTO` admits the last user page and refuses the kernel half, a straddling run and a
/// wrapping run, leaving nothing mapped behind a refusal** (claim 10).
///
/// The gate proved per format in `crates/paging` answers by construction; this drives its edges
/// through the dispatcher on the running architecture. A run whose *second* page would land in
/// the kernel half must refuse and must not leave its first page mapped: `MAP_INTO` promises
/// all-or-none, and a half-mapped run would put a writable page exactly at the boundary.
///
/// Falsification: unfalsified. Written by the tenth outsider pass's reviewer.
#[test_case]
fn map_into_refuses_the_kernel_half_straddling_and_wrapping_runs() {
    let frame_region = crate::memory_region::create(8).expect("no frame region");
    let budget = sched::grant(region_capability_rights(frame_region, Rights::WRITE))
        .expect("grant the frame budget");
    let one_page =
        call(budget, abi::memory_region::RETYPE, 0, 0, 0).expect("no one-page frame") as u64;
    let two_pages =
        call(budget, abi::memory_region::RETYPE, 2, 0, 0).expect("no two-page frame") as u64;

    let space_region = crate::memory_region::create(8).expect("no space region");
    let space_name = crate::user::user_address_space_create(space_region).expect("no space");
    let space = sched::grant(address_space_capability(space_name, Rights::WRITE))
        .expect("grant the space capability");

    let page = paging::PAGE_SIZE;
    // The running format's own split (bit 48 on aarch64, 38 on Sv39): the gate's user half ends
    // one page below it, which is not `KERNEL_VA_BASE - PAGE` on every architecture.
    let split = 1u64 << <crate::arch::mmu::Format as paging::PageFormat>::SPLIT_SHIFT;
    let last_user_page = split - page;

    // A one-page control at the last-but-one user page (writable), leaving the last user page
    // free so the straddling run below starts on an unmapped address.
    let below = last_user_page - page;
    map_one_page(space, one_page, below, abi::address_space::MAP_RW)
        .expect("MAP_INTO refused the last-but-one user page");
    assert!(
        flags_at(space_name, below).is_some_and(|f| f.is_writable()),
        "the control page did not land writable",
    );

    // A two-page run starting at the last user page: its second page lands in the kernel half,
    // so the run is refused, and its first page must not survive the refusal (all or none).
    assert_eq!(
        call(
            space,
            abi::address_space::MAP_INTO,
            last_user_page,
            two_pages,
            abi::address_space::MAP_RW
        ),
        Err(Error::BadPointer),
        "a run whose second page lands in the kernel half was mapped (claim 10)",
    );
    assert!(
        flags_at(space_name, last_user_page).is_none(),
        "the straddling run left its first page mapped after refusing: not all-or-none, and a \
         writable page sits at the kernel boundary",
    );
    // The last user page by itself is inside, so the refusal above was the run's shape and not
    // the address: this is the one-page positive control at the boundary itself.
    map_one_page(space, one_page, last_user_page, abi::address_space::MAP_RW)
        .expect("MAP_INTO refused the last user page itself");

    // The kernel half's first page, by itself.
    assert_eq!(
        call(
            space,
            abi::address_space::MAP_INTO,
            mmu::KERNEL_VA_BASE,
            one_page,
            abi::address_space::MAP_RW
        ),
        Err(Error::BadPointer),
        "MAP_INTO mapped a page at the kernel half's base (claim 10)",
    );

    // An unaligned address inside the user half.
    assert_eq!(
        call(
            space,
            abi::address_space::MAP_INTO,
            below + 8,
            one_page,
            abi::address_space::MAP_RW
        ),
        Err(Error::BadPointer),
        "MAP_INTO mapped an unaligned page (claim 10)",
    );

    // A run at the top of the address space: the half gate refuses it here (the checked
    // end-of-run arithmetic is the host proofs' half; no wrap is reachable from the low half).
    let top = u64::MAX - page * 2 + 1; // deliberately unaligned too, the worst address a caller can name
    assert_eq!(
        call(
            space,
            abi::address_space::MAP_INTO,
            top,
            two_pages,
            abi::address_space::MAP_RW
        ),
        Err(Error::BadPointer),
        "a run at the very top of the address space was mapped (claim 10)",
    );

    // Teardown: unmap what mapped, then the two regions come back.
    for va in [below, last_user_page] {
        assert_eq!(
            call(space, abi::address_space::UNMAP, va, 0, 0),
            Ok(0),
            "UNMAP refused a page this test mapped",
        );
    }
    let _ = sched::delete_current_capability(two_pages);
    let _ = sched::delete_current_capability(one_page);
    let _ = sched::delete_current_capability(budget);
    let _ = sched::delete_current_capability(space);
    assert!(
        sched::reclaim_region(frame_region).is_ok(),
        "the frame region did not come back",
    );
    assert!(
        sched::reclaim_region(space_region).is_ok(),
        "the space region did not come back",
    );
}

/// **A refused overlay leaves the code page exactly executable, and a refused duplicate leaves no
/// record behind** (claim 11).
///
/// The overlay maps code and then asks for read/write at the same address: the mapping hold
/// refuses it as the writable-and-executable pair (`NotPermitted`) before the tables are touched,
/// and the page stays executable and not writable. The duplicate asks for code again at the same
/// address: the hold files the record, the leaf refuses as already mapped (`BadPointer`), and the
/// record must go with it, or the second `UNMAP` below would find it and answer 0.
///
/// Falsification: unfalsified. Written by the tenth outsider pass's reviewer.
#[test_case]
fn a_refused_overlay_leaves_the_code_page_exactly_executable() {
    let frame_region = crate::memory_region::create(8).expect("no frame region");
    // READ and WRITE, so the frame clears both shapes' rights gates and the rights model cannot
    // be the thing that refuses: the question is the overlay, not the capability.
    let budget = sched::grant(region_capability_rights(
        frame_region,
        Rights::READ.union(Rights::WRITE),
    ))
    .expect("grant the frame budget");
    let frame = call(budget, abi::memory_region::RETYPE, 0, 0, 0).expect("no frame") as u64;

    let space_region = crate::memory_region::create(8).expect("no space region");
    let space_name = crate::user::user_address_space_create(space_region).expect("no space");
    let space = sched::grant(address_space_capability(space_name, Rights::WRITE))
        .expect("grant the space capability");

    // The overlay: code first, then read/write asked at the same address. Derived from the
    // format's split so the address is inside the user half on every architecture.
    let page = paging::PAGE_SIZE;
    let split = 1u64 << <crate::arch::mmu::Format as paging::PageFormat>::SPLIT_SHIFT;
    let overlay_va = split - page;
    map_one_page(space, frame, overlay_va, abi::address_space::MAP_CODE)
        .expect("MAP_INTO refused the code mapping of a READ|WRITE frame");
    assert_eq!(
        call(
            space,
            abi::address_space::MAP_INTO,
            overlay_va,
            frame,
            abi::address_space::MAP_RW
        ),
        Err(Error::NotPermitted),
        "MAP_INTO overlaid a writable mapping on a live code page, or refused it as something \
         other than the writable-and-executable pair (claim 11)",
    );
    let overlay_flags = flags_at(space_name, overlay_va).expect("the code page vanished");
    assert!(
        overlay_flags.is_user_executable() && !overlay_flags.is_writable(),
        "after the refused overlay the page is no longer exactly executable-not-writable: \
         {overlay_flags:?}",
    );

    assert_eq!(
        call(
            space,
            abi::address_space::MAP_INTO,
            overlay_va,
            frame,
            abi::address_space::MAP_CODE
        ),
        Err(Error::BadPointer),
        "MAP_INTO mapped code twice at one address",
    );

    // Teardown: unmap what mapped, then the regions come back. The second `UNMAP` finds neither a
    // leaf nor a record, which is what shows the refused duplicate took its record back.
    assert_eq!(
        call(space, abi::address_space::UNMAP, overlay_va, 0, 0),
        Ok(0),
        "UNMAP refused a page this test mapped",
    );
    assert_eq!(
        call(space, abi::address_space::UNMAP, overlay_va, 0, 0),
        Err(Error::BadPointer),
        "a second UNMAP found a record the refused duplicate mapping left behind",
    );
    let _ = sched::delete_current_capability(frame);
    let _ = sched::delete_current_capability(budget);
    let _ = sched::delete_current_capability(space);
    assert!(
        sched::reclaim_region(frame_region).is_ok(),
        "the frame region did not come back",
    );
    assert!(
        sched::reclaim_region(space_region).is_ok(),
        "the space region did not come back",
    );
}

/// **One physical page is never writable through one virtual address and executable through
/// another in the same space** (claim 11): `MAP_RW` at one address, then `MAP_CODE` of the same
/// frame at the next, both through `AddressSpace::MAP_INTO`.
///
/// Each leaf would be honest and the page both writable and executable, so the mapping hold refuses
/// the second mapping with `NotPermitted` and nothing lands at the executable address. The pass's
/// reproduction used these addresses: the top two user pages of the running format.
///
/// Falsification: replayable `system_tests/falsifications/user.tenth_pass_map_gate_tests.one_physical_page_is_never_writable_here_and_executable_there.patch`
#[test_case]
fn one_physical_page_is_never_writable_here_and_executable_there() {
    let frame_region = crate::memory_region::create(8).expect("no frame region");
    let budget = sched::grant(region_capability_rights(
        frame_region,
        Rights::READ.union(Rights::WRITE),
    ))
    .expect("grant the frame budget");
    let frame = call(budget, abi::memory_region::RETYPE, 0, 0, 0).expect("no frame") as u64;

    let space_region = crate::memory_region::create(8).expect("no space region");
    let space_name = crate::user::user_address_space_create(space_region).expect("no space");
    let space = sched::grant(address_space_capability(space_name, Rights::WRITE))
        .expect("grant the space capability");

    let page = paging::PAGE_SIZE;
    let split = 1u64 << <crate::arch::mmu::Format as paging::PageFormat>::SPLIT_SHIFT;
    let executable_va = split - page;
    let writable_va = split - page * 2;
    map_one_page(space, frame, writable_va, abi::address_space::MAP_RW)
        .expect("MAP_INTO refused the writable mapping of a READ|WRITE frame");
    assert_eq!(
        map_one_page(space, frame, executable_va, abi::address_space::MAP_CODE),
        Err(Error::NotPermitted),
        "MAP_INTO mapped as code a frame the space maps writable at {writable_va:#x}, or refused \
         it as something other than the writable-and-executable pair (claim 11)",
    );
    assert!(
        flags_at(space_name, executable_va).is_none(),
        "one physical page is writable at {writable_va:#x} and executable at {executable_va:#x}: \
         every leaf is honest and the page is both, which is W^X defeated by alias (claim 11)",
    );

    assert_eq!(
        call(space, abi::address_space::UNMAP, writable_va, 0, 0),
        Ok(0),
        "UNMAP refused a page this test mapped",
    );
    let _ = sched::delete_current_capability(frame);
    let _ = sched::delete_current_capability(budget);
    let _ = sched::delete_current_capability(space);
    assert!(
        sched::reclaim_region(frame_region).is_ok(),
        "the frame region did not come back",
    );
    assert!(
        sched::reclaim_region(space_region).is_ok(),
        "the space region did not come back",
    );
}

/// Spin, yielding, until `cond` holds or two seconds pass.
fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
    let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if cond() {
            return true;
        }
        sched::yield_now();
    }
    cond()
}

/// A syscall answer as one word for an atomic: 0 for success, else the error's negative value.
fn encode(answer: Result<i64, Error>) -> u64 {
    match answer {
        Ok(_) => 0,
        Err(e) => e as i64 as u64,
    }
}

/// **The pair is refused whichever path makes its second half** (claim 11): code through
/// `AddressSpace::MAP_INTO` and then writable through `PageFrame::MAP`, and the reverse.
///
/// `PageFrame::MAP` maps into the caller's own space and never through `MAP_INTO`'s dispatcher, so
/// a check there alone would admit the first order. The thread here holds `WRITE` on its own space,
/// as the progenitor, `net_stack` and `domain_name_resolver` do. Each order uses its own frame, and
/// each refusal must be `NotPermitted` with nothing mapped at the second address.
///
/// Falsification: replayable `system_tests/falsifications/user.tenth_pass_map_gate_tests.the_pair_is_refused_through_page_frame_map_in_either_order.patch`
#[test_case]
fn the_pair_is_refused_through_page_frame_map_in_either_order() {
    static WRITABLE_AFTER_CODE: AtomicU64 = AtomicU64::new(1);
    static WRITABLE_LANDED: AtomicBool = AtomicBool::new(true);
    static CODE_AFTER_WRITABLE: AtomicU64 = AtomicU64::new(1);
    static CODE_LANDED: AtomicBool = AtomicBool::new(true);
    static DONE: AtomicBool = AtomicBool::new(false);
    DONE.store(false, Ordering::SeqCst);

    let frames = crate::memory_region::create(4).expect("no frame region");
    let first = crate::memory_region::retype_page(frames).expect("no frame");
    let second = crate::memory_region::retype_page(frames).expect("no frame");
    let tables = crate::memory_region::create(8).expect("no page-table region");

    let mapper = sched::spawn(move || {
        let name =
            sched::adopt_address_space(crate::user::AddressSpace::new(16).expect("no space"));
        let root = mmu::current_user_root();
        let own = sched::grant(address_space_capability(name, Rights::WRITE))
            .expect("grant the own space");
        let tables_slot = sched::grant(memory_region_capability(tables)).expect("grant tables");
        let rights = Rights::READ.union(Rights::WRITE);
        let one = page_frame_run_len(1);
        let f1 = sched::grant(page_frame_run_capability(first, one, rights)).expect("grant f1");
        let f2 = sched::grant(page_frame_run_capability(second, one, rights)).expect("grant f2");
        let page = paging::PAGE_SIZE;
        let base = 0x40_0000;

        // Code through MAP_INTO, then writable through PageFrame::MAP.
        call(
            own,
            abi::address_space::MAP_INTO,
            base,
            f1,
            abi::address_space::MAP_CODE,
        )
        .expect("MAP_INTO refused the code mapping");
        WRITABLE_AFTER_CODE.store(
            encode(call(f1, abi::page_frame::MAP, base + page, 1, tables_slot)),
            Ordering::SeqCst,
        );
        WRITABLE_LANDED.store(
            mmu::translate_at(root, base + page).is_some(),
            Ordering::SeqCst,
        );

        // Writable through PageFrame::MAP, then code through MAP_INTO.
        call(f2, abi::page_frame::MAP, base + 2 * page, 1, tables_slot)
            .expect("PageFrame::MAP refused the writable mapping");
        CODE_AFTER_WRITABLE.store(
            encode(call(
                own,
                abi::address_space::MAP_INTO,
                base + 3 * page,
                f2,
                abi::address_space::MAP_CODE,
            )),
            Ordering::SeqCst,
        );
        CODE_LANDED.store(
            mmu::translate_at(root, base + 3 * page).is_some(),
            Ordering::SeqCst,
        );

        for slot in [own, tables_slot, f1, f2] {
            let _ = sched::delete_current_capability(slot);
        }
        DONE.store(true, Ordering::SeqCst);
    })
    .expect("no mapper thread");

    assert!(
        wait_for(|| DONE.load(Ordering::SeqCst)),
        "the mapper never finished"
    );
    let refused = Error::NotPermitted as i64 as u64;
    assert_eq!(
        WRITABLE_AFTER_CODE.load(Ordering::SeqCst),
        refused,
        "PageFrame::MAP mapped writable a frame the space maps as code, or refused it as something \
         other than the writable-and-executable pair (claim 11)",
    );
    assert!(
        !WRITABLE_LANDED.load(Ordering::SeqCst),
        "a refused PageFrame::MAP left its writable leaf in place (claim 11)",
    );
    assert_eq!(
        CODE_AFTER_WRITABLE.load(Ordering::SeqCst),
        refused,
        "MAP_INTO mapped as code a frame PageFrame::MAP had mapped writable in the same space, or \
         refused it as something other than the writable-and-executable pair (claim 11)",
    );
    assert!(
        !CODE_LANDED.load(Ordering::SeqCst),
        "a refused MAP_INTO left its code leaf in place (claim 11)",
    );

    assert!(
        wait_for(|| !sched::is_thread_present(mapper)),
        "the mapper was never reaped"
    );
    crate::memory_region::destroy(frames);
    crate::memory_region::destroy(tables);
}

/// **The narrow rule refuses only the pair, and only within one space** (milestone 969's width
/// and scope, ruled 2026-10-31 UTC).
///
/// In one space, one frame read-only twice, read-only beside writable, read-only beside code and
/// writable twice all map. Across two spaces, the frame writable in one and code in the other
/// also maps: claim 11 covers one address space, and the row states that a holder of `WRITE` on
/// a frame can change code another space runs. Loading relies on that
/// (`process_builder::fill_and_map`).
///
/// Falsification: replayable `system_tests/falsifications/user.tenth_pass_map_gate_tests.the_narrow_rule_admits_every_other_pair_and_every_other_space.patch`
#[test_case]
fn the_narrow_rule_admits_every_other_pair_and_every_other_space() {
    let frame_region = crate::memory_region::create(8).expect("no frame region");
    let budget = sched::grant(region_capability_rights(
        frame_region,
        Rights::READ.union(Rights::WRITE),
    ))
    .expect("grant the frame budget");
    let read_only = call(budget, abi::memory_region::RETYPE, 0, 0, 0).expect("no frame") as u64;
    let shared = call(budget, abi::memory_region::RETYPE, 0, 0, 0).expect("no frame") as u64;

    let space_regions = [
        crate::memory_region::create(8).expect("no space region"),
        crate::memory_region::create(8).expect("no space region"),
    ];
    let names = space_regions.map(|r| crate::user::user_address_space_create(r).expect("no space"));
    let [here, there] = names.map(|n| {
        sched::grant(address_space_capability(n, Rights::WRITE)).expect("grant the space")
    });

    let page = paging::PAGE_SIZE;
    let base = 0x40_0000;
    let admitted = [
        (
            here,
            read_only,
            base,
            abi::address_space::MAP_RO,
            "read-only",
        ),
        (
            here,
            read_only,
            base + page,
            abi::address_space::MAP_RO,
            "read-only twice",
        ),
        (
            here,
            read_only,
            base + 2 * page,
            abi::address_space::MAP_RW,
            "writable beside read-only",
        ),
        (
            here,
            shared,
            base + 3 * page,
            abi::address_space::MAP_CODE,
            "code",
        ),
        (
            here,
            shared,
            base + 4 * page,
            abi::address_space::MAP_RO,
            "read-only beside code",
        ),
        (
            there,
            shared,
            base,
            abi::address_space::MAP_RW,
            "writable in another space",
        ),
        (
            there,
            shared,
            base + page,
            abi::address_space::MAP_RW,
            "writable twice there",
        ),
    ];
    for (space, frame, va, shape, what) in admitted {
        assert_eq!(
            map_one_page(space, frame, va, shape),
            Ok(()),
            "the narrow rule refused {what}, which leaves no page writable and executable in one \
             space",
        );
    }
    assert!(
        flags_at(names[0], base + 3 * page).is_some_and(|f| f.is_user_executable())
            && flags_at(names[1], base).is_some_and(|f| f.is_writable()),
        "the cross-space pair did not land as code here and writable there",
    );

    for (space, va) in [
        (here, base),
        (here, base + page),
        (here, base + 2 * page),
        (here, base + 3 * page),
        (here, base + 4 * page),
        (there, base),
        (there, base + page),
    ] {
        assert_eq!(
            call(space, abi::address_space::UNMAP, va, 0, 0),
            Ok(0),
            "UNMAP refused a page this test mapped",
        );
    }
    for slot in [shared, read_only, budget, here, there] {
        let _ = sched::delete_current_capability(slot);
    }
    assert!(
        sched::reclaim_region(frame_region).is_ok(),
        "the frame region did not come back",
    );
    for region in space_regions {
        assert!(
            sched::reclaim_region(region).is_ok(),
            "a space region did not come back",
        );
    }
}
