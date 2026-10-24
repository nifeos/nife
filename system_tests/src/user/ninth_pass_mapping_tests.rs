//! **The map gate admits only the aligned low half, and no mapping path yields a writable
//! executable page** (a ninth outsider pass at the confinement claim, claims 10 and 11).
//!
//! `crates/paging` proves both claims per page-table format
//! (`the_user_va_gate_admits_only_the_aligned_low_half`,
//! `no_encoded_leaf_is_both_writable_and_executable`). These attack them one layer up, through the
//! live dispatcher, from a confined caller holding an ordinary memory budget. Nothing blocks or
//! maps into a live space; the one budget is deleted and its region reclaimed.
//!
//! Name: provisional (lane/pass-9-tests, 2026-10-14 UTC). Cross-ISA: `invoke`, the map gate and the
//! flag constructors are portable kernel code, and `is_user_page_va` is instantiated at the running
//! architecture's `Format`, so the body runs on each ISA (DECISIONS §19 (architectural parity is a
//! tenet)).

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::arch::mmu::Format;
use crate::cap::{Rights, memory_region_cap_rights as region_capability_rights};
use crate::sched::{self, delete_current_cap as delete_current_capability};
use crate::syscall::invoke;

/// `invoke` through the real dispatcher. Copied in shape from `confinement_attack_tests::call`.
fn call(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, a1, a2)
}

/// A page-aligned address with every top bit set: the kernel's half on every supported format
/// (aarch64 splits at bit 48, Sv39 at 38, Ia32e at 47/48, and all of those see these bits as ones).
const KERNEL_HALF_PAGE: u64 = 0xFFFF_FFFF_FFFF_F000;
/// A low-half address that is not page-aligned.
const UNALIGNED_LOW: u64 = 0x4001;
/// A low-half, page-aligned address: what the gate must admit, so the refusals mean something.
const VALID_LOW_PAGE: u64 = 0x4000_0000;

/// **A confined caller cannot map outside the aligned low half** (claim 10).
///
/// `MemoryRegion::MAP` at a kernel-half and at an unaligned low-half address each answers
/// `BadPointer`, refused before the budget is spent, so a confined program can neither aim a
/// mapping at the kernel's half nor leak a page per bad request.
///
/// Falsification: unfalsified. Written by the ninth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_confined_map_cannot_escape_the_aligned_low_half() {
    // The gate the dispatcher consults, checked directly first so the dispatcher refusals below are
    // demonstrably the gate and not some unrelated error, and so the test is not vacuous: the gate
    // admits the valid address.
    assert!(
        !paging::is_user_page_va::<Format>(KERNEL_HALF_PAGE),
        "the VA gate admitted a kernel-half address",
    );
    assert!(
        !paging::is_user_page_va::<Format>(UNALIGNED_LOW),
        "the VA gate admitted an unaligned address",
    );
    assert!(
        paging::is_user_page_va::<Format>(VALID_LOW_PAGE),
        "the VA gate rejected a valid low-half aligned address, so a refusal below proves nothing",
    );

    let region = crate::memory_region::create(8).expect("no region");
    let budget =
        sched::grant(region_capability_rights(region, Rights::ALL)).expect("grant the budget");

    // A kernel-half target: refused, and before any page is retyped (the dispatcher checks the gate
    // first). A confined program cannot aim a mapping into the kernel's half.
    assert_eq!(
        call(budget, abi::memory_region::MAP, KERNEL_HALF_PAGE, 0, 0),
        Err(Error::BadPointer),
        "MemoryRegion::MAP accepted a kernel-half address: a confined caller mapped into the \
         kernel's half (claim 10)",
    );
    // An unaligned low-half target: refused for the other half of the gate.
    assert_eq!(
        call(budget, abi::memory_region::MAP, UNALIGNED_LOW, 0, 0),
        Err(Error::BadPointer),
        "MemoryRegion::MAP accepted an unaligned address (claim 10)",
    );

    let _ = delete_current_capability(budget);
    sched::reclaim_region(region).expect("the region did not come back");
}

/// **No mapping flag is both writable and user-executable** (claim 11, W^X).
///
/// The four flag sets a mapping path can install, asserted where a leaf is encoded from: the
/// kernel-visible twin of `paging`'s per-format `no_encoded_leaf_is_both_writable_and_executable`,
/// covering every `Flags` that `MemoryRegion::MAP`, `PageFrame::MAP` and `AddressSpace::MAP_INTO`
/// reach. `MAP` installs `user_data` (W, not X); `MAP_INTO`'s `MAP_CODE` installs `user_code` (X,
/// not W, gated on READ, not WRITE).
///
/// Falsification: unfalsified. Written by the ninth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn no_mapping_flag_is_both_writable_and_user_executable() {
    for (flags, name) in [
        (paging::Flags::user_data(), "user_data"),
        (paging::Flags::user_rodata(), "user_rodata"),
        (paging::Flags::user_code(), "user_code"),
        (paging::Flags::user_device(), "user_device"),
    ] {
        assert!(
            !(flags.is_writable() && flags.is_user_executable()),
            "{name} is both writable and user-executable: a W^X violation a mapping path can \
             install (claim 11)",
        );
    }
}
