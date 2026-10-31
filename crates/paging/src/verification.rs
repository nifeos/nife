use super::*;

/// The three leaf sizes, spelled as literals rather than through [`PageSize::bytes`]: a harness
/// that measured the choice with the same function that made it would be satisfied by any
/// `bytes` at all, which is the trap milestone 211 and milestone 307 each found once.
fn literal_bytes(size: PageSize) -> u64 {
    match size {
        PageSize::Size4KiB => 0x1000,
        PageSize::Size2MiB => 0x20_0000,
        PageSize::Size1GiB => 0x4000_0000,
    }
}

fn any_size() -> PageSize {
    let i: usize = kani::any();
    kani::assume(i < 3);
    PageSize::ALL[i]
}

/// **Soundness, the security direction: every leaf `map_span` writes is aligned at both ends
/// and lies wholly inside what is left of the span**, and is never larger than the caller
/// allowed, for every page-aligned `va`, `pa` and length. With this, the blocks cover no byte
/// the equivalent run of pages would not.
/// Falsification: replayable `crates/paging/falsifications/verification.a_chosen_leaf_is_aligned_and_inside_the_span.patch`
#[kani::proof]
fn a_chosen_leaf_is_aligned_and_inside_the_span() {
    let va: u64 = kani::any();
    let pa: u64 = kani::any();
    let remaining: u64 = kani::any();
    kani::assume(
        va.is_multiple_of(0x1000) && pa.is_multiple_of(0x1000) && remaining.is_multiple_of(0x1000),
    );
    kani::assume(remaining >= 0x1000);
    let largest = any_size();

    let size = PageSize::largest_fitting(va, pa, remaining, largest);
    let bytes = literal_bytes(size);
    assert_eq!(
        size.bytes(),
        bytes,
        "PageSize::bytes disagrees with the architecture"
    );
    assert!(size <= largest, "a leaf larger than the caller allowed");
    assert_eq!(va % bytes, 0, "the leaf's virtual start is misaligned");
    assert_eq!(pa % bytes, 0, "the leaf's physical start is misaligned");
    assert!(bytes <= remaining, "the leaf runs past the end of the span");
}

/// **Completeness, the direction the 0.2% is about: no fitting leaf is passed over.** For
/// every size the caller allowed that would have fitted, the choice is at least that large.
/// Soundness alone is satisfied by always answering 4 KiB, which is correct and is exactly the
/// direct map this milestone exists to stop building.
/// Falsification: replayable `crates/paging/falsifications/verification.the_chosen_leaf_is_the_largest_that_fits.patch`
#[kani::proof]
fn the_chosen_leaf_is_the_largest_that_fits() {
    let va: u64 = kani::any();
    let pa: u64 = kani::any();
    let remaining: u64 = kani::any();
    let largest = any_size();
    let candidate = any_size();
    let want = literal_bytes(candidate);
    kani::assume(candidate <= largest);
    kani::assume(va.is_multiple_of(want) && pa.is_multiple_of(want) && remaining >= want);

    let size = PageSize::largest_fitting(va, pa, remaining, largest);
    assert!(
        literal_bytes(size) >= want,
        "a fitting larger leaf was passed over"
    );
}

/// **No pair of mappings the kernel admits beside each other is writable through one and
/// executable through the other**, over every access word either side can carry: the predicate
/// `MappingHold::record_mapping` refuses by answers true for every such pair, in either order.
///
/// Stated in raw bits written as literals (write is bit 0, user execute bit 2, kernel execute bit
/// 3), never through `is_writable` or the access constants: a predicate that drifted with its own
/// helpers would otherwise prove itself.
/// Falsification: replayable `crates/paging/falsifications/verification.no_admitted_pair_is_writable_here_and_executable_there.patch`
#[kani::proof]
fn no_admitted_pair_is_writable_here_and_executable_there() {
    let a = Flags::from_access(kani::any());
    let b = Flags::from_access(kani::any());
    let writes = |f: Flags| f.bits() & 0b1 != 0;
    let executes = |f: Flags| f.bits() & 0b1100 != 0;
    if !a.makes_writable_executable_pair(b) {
        assert!(!(writes(a) && executes(b)) && !(writes(b) && executes(a)));
    }
}
