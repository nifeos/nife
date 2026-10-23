use super::address_space::run_end_va;
use super::*;

/// The page size the run arithmetic steps by, spelled once so the harnesses below and
/// `run_end_va` cannot drift apart.
const PAGE: u128 = paging::PAGE_SIZE as u128;

/// **`run_end_va` is exact, and it refuses exactly the runs that do not fit.**
///
/// A wrapping multiply would admit a run spanning the address space for a measure-zero set of
/// `count`s near multiples of 2^52, which no unit test writes down; the solver covers the domain.
/// The claim is stated in `u128`, which cannot wrap here, so the harness does not repeat the
/// implementation's own expression back to it.
///
/// Falsification: replayable `kernel/falsifications/syscall.proofs.the_run_end_is_exact_and_refuses_exactly_what_does_not_fit.patch`
#[kani::proof]
fn the_run_end_is_exact_and_refuses_exactly_what_does_not_fit() {
    let va: u64 = kani::any();
    let count: u64 = kani::any();
    // Assumed, not proved: `count` is a `NonZeroU64` on the capability (DECISIONS §102 (a Frame
    // names a run of pages)), and the type is the mechanism.
    kani::assume(count >= 1);

    let want = va as u128 + (count as u128 - 1) * PAGE;

    match run_end_va(va, count) {
        Some(last) => {
            assert!(
                last as u128 == want,
                "the last page of the run is va + (count - 1) * PAGE_SIZE, with no wrap"
            );
            assert!(last >= va, "the run cannot end below where it starts");
        }
        None => assert!(
            want > u64::MAX as u128,
            "a run that fits in the address space must not be refused"
        ),
    }
}

/// **Checking the two ends of a run is enough to check every page in it.**
///
/// Both mapping paths (`page_frame_map` and `MAP_INTO`) guard `va` and `run_end_va(va, count)`,
/// then walk the pages between unchecked, sound because the user half is a contiguous prefix. `k`
/// is the solver's, so every page of every run is covered without an unwind bound.
///
/// # BUGS
/// - The guard below models the two call sites, and it can go stale: nothing keeps it faithful.
///
/// Falsification: replayable `kernel/falsifications/syscall.proofs.every_page_between_the_checked_ends_is_itself_a_user_page.patch`
#[kani::proof]
fn every_page_between_the_checked_ends_is_itself_a_user_page() {
    let va: u64 = kani::any();
    let count: u64 = kani::any();
    let k: u64 = kani::any();
    kani::assume(count >= 1);
    kani::assume(k < count);

    // The guard the call sites apply, verbatim.
    let Some(last_va) = run_end_va(va, count) else {
        return;
    };
    kani::assume(paging::is_user_page_va::<crate::arch::mmu::Format>(va));
    kani::assume(paging::is_user_page_va::<crate::arch::mmu::Format>(last_va));

    // What the loop body computes. In a release build this is wrapping arithmetic; the claim
    // is that it never gets the chance to wrap.
    let offset = k as u128 * PAGE;
    assert!(
        va as u128 + offset <= last_va as u128,
        "no page of the run is computed past the end the guard checked"
    );
    let page_va = va + k * paging::PAGE_SIZE;
    assert!(
        paging::is_user_page_va::<crate::arch::mmu::Format>(page_va),
        "every page the map loop touches is an aligned user-half page"
    );
}
