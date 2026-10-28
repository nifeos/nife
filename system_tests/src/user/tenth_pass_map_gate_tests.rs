//! **The user/kernel boundary of the mapping paths, and W^X against an alias** (a tenth outsider
//! pass at the confinement claim, claims 10 and 11).
//!
//! Pass 9 refused kernel-half and unaligned `MemoryRegion::MAP` addresses; the angles here are
//! the boundary *edges* of `AddressSpace::MAP_INTO` (the last user page, a run whose second page
//! lands in the kernel half, a straddling run's all-or-none undo), the same-address overlay
//! (`MAP_CODE`, then `MAP_RW` at the same `va`, refused leaving the page exactly executable), and
//! the alias question one level above the per-leaf proofs: the same frame mapped `MAP_RW` at one
//! address and `MAP_CODE` at another. That last one is this pass's escape: the second mapping is
//! accepted, so the *page* is writable and executable through two honest leaves, and the witness
//! is the opt-in red test below.
//!
//! Name: provisional (this pass's lane, 2026-10-27 UTC), for the module, the file and each
//! function. Cross-ISA: `MAP_INTO`, the gate and the flags are instantiated at the running
//! architecture's `Format` (DECISIONS §19 (architectural parity is a tenet)).
//!
//! Attacks rows: 10, 11

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::arch::mmu;
use crate::capability_object::{
    Rights, address_space_capability, memory_region_capability_rights as region_capability_rights,
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

/// **One physical page is never writable through one virtual address and executable through
/// another** (claim 11): the alias attack, plus the same-address overlay.
///
/// The per-leaf proofs (`no_encoded_leaf_is_both_writable_and_executable`) hold for every leaf;
/// this asks the question one level up, where a builder holding a writable frame and a space can
/// map the *same* frame twice. `MAP_RW` at one `va` and `MAP_CODE` at another would leave every
/// leaf honest while the page is both writable and executable, so the second mapping must be
/// refused or must not leave the pair live. The overlay arm maps code and then asks for
/// read/write at the same address: the refusal must leave the page executable and not writable.
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
        Err(Error::BadPointer),
        "MAP_INTO overlaid a writable mapping on a live code page: a refused overlay must not \
         reach the tables (claim 11)",
    );
    let overlay_flags = flags_at(space_name, overlay_va).expect("the code page vanished");
    assert!(
        overlay_flags.is_user_executable() && !overlay_flags.is_writable(),
        "after the refused overlay the page is no longer exactly executable-not-writable: \
         {overlay_flags:?}",
    );

    // Teardown: unmap what mapped, then the regions come back.
    assert_eq!(
        call(space, abi::address_space::UNMAP, overlay_va, 0, 0),
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

/// **Whether the alias escape witness runs**: this pass's one red test is opt-in, the shape the
/// reviewer's brief asks for, so the default suite stays green while the record of the escape
/// stays bootable. Export `NIFE_ESCAPE_WITNESS=1` before `script/test` to run it.
const ESCAPE_WITNESS: bool = match option_env!("NIFE_ESCAPE_WITNESS") {
    Some(v) => !v.is_empty(),
    None => false,
};

/// **One physical page must never be writable through one virtual address and executable through
/// another** (claim 11). The escape witness: RED today.
///
/// `AddressSpace::MAP_INTO` maps the frame a capability names, and nothing checks that the same
/// physical page is not already mapped in the same space with different permissions. Two legal
/// mappings, `MAP_RW` at one `va` and `MAP_CODE` at another, leave every page-table leaf honest
/// while the *page* is both writable and executable: a builder holding `WRITE` on a space and a
/// `READ | WRITE` frame can hand a child self-modifying code without touching a single check the
/// per-leaf proofs cover. The per-leaf W^X the cited harnesses prove survives; the claim as
/// published ("no page is both writable and executable") does not.
///
/// This test is opt-in (`NIFE_ESCAPE_WITNESS=1`) because it is red, and the brief asks that a red
/// test which would break the default suite say so and stay out of it. The pass's note,
/// `notes/confinement-outsider-pass-10.md`, records where it booted red. The falsification patch
/// turns the opt-in on, so the sweep replays the escape itself until the fix lands; the fix
/// milestone re-cuts it to undo the fix instead.
///
/// Falsification: replayable `system_tests/falsifications/user.tenth_pass_map_gate_tests.one_physical_page_is_never_writable_here_and_executable_there.patch`
#[test_case]
fn one_physical_page_is_never_writable_here_and_executable_there() {
    if !ESCAPE_WITNESS {
        crate::testing::skip!(
            "opt-in: the alias escape witness, red on this kernel; run with NIFE_ESCAPE_WITNESS=1"
        );
    }
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
    let _aliased = call(
        space,
        abi::address_space::MAP_INTO,
        executable_va,
        frame,
        abi::address_space::MAP_CODE,
    );
    let writable_alias = flags_at(space_name, writable_va).is_some_and(|f| f.is_writable());
    let executable_alias =
        flags_at(space_name, executable_va).is_some_and(|f| f.is_user_executable());
    assert!(
        !(writable_alias && executable_alias),
        "one physical page is writable at {writable_va:#x} and executable at {executable_va:#x}: \
         every leaf is honest and the page is both, which is W^X defeated by alias (claim 11)",
    );
}
