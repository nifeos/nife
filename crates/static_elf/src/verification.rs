use super::*;

/// **The per-segment bounds check never panics.** `check_segment_bounds` does the arithmetic a
/// hostile program header weaponizes (`offset + filesz`, `vaddr + memsz`). This proves it never
/// panics or overflows, for any file length and any field values. This is the leaf of the
/// parser's panic surface, factored out of the loop so bounded model checking can reach it (the
/// whole-parse proof cannot).
/// Falsification: replayable `crates/static_elf/falsifications/verification.check_segment_bounds_never_panics.patch`
#[kani::proof]
fn check_segment_bounds_never_panics() {
    let _ = check_segment_bounds(
        kani::any(),
        kani::any(),
        kani::any(),
        kani::any(),
        kani::any(),
    );
}

/// **A passing check yields a valid, in-file range.** If `check_segment_bounds` returns
/// `Ok(end)` then `p_offset <= end <= file_len`, so the caller's slice `bytes[p_offset..end]` is
/// in bounds. This is the guarantee that makes `segment_at`'s slice safe, proved for every input,
/// which is what the intractable whole-parse totality proof was really reaching for.
/// Falsification: replayable `crates/static_elf/falsifications/verification.a_passing_check_yields_an_in_bounds_range.patch`
#[kani::proof]
fn a_passing_check_yields_an_in_bounds_range() {
    let file_len: usize = kani::any();
    let p_offset: usize = kani::any();
    if let Ok(end) = check_segment_bounds(file_len, p_offset, kani::any(), kani::any(), kani::any())
    {
        assert!(p_offset <= end);
        assert!(end <= file_len);
    }
}

/// **A passing check guarantees the virtual range does not overflow.** If it returns `Ok`, then
/// `vaddr + memsz` did not wrap, so the later unchecked `seg.vaddr + seg.memsz` in `validate`
/// cannot panic. This proves the cross-function invariant by hand-off through the type.
/// Falsification: replayable `crates/static_elf/falsifications/verification.a_passing_check_has_no_address_overflow.patch`
#[kani::proof]
fn a_passing_check_has_no_address_overflow() {
    let vaddr: u64 = kani::any();
    let memsz: u64 = kani::any();
    if check_segment_bounds(kani::any(), kani::any(), kani::any(), vaddr, memsz).is_ok() {
        assert!(vaddr.checked_add(memsz).is_some());
    }
}

/// **`page_range` is panic-free and ordered for any segment and any page size.** `Segment` is
/// `pub`, so its helper must be safe on its own, without `parse`'s guarantees. For every `vaddr`
/// and `memsz`, including the near-`u64::MAX` values a hostile file names, and every page size a
/// `PageBytes` can hold (each power of two from 1 to 2^63), the saturating arithmetic neither
/// panics nor returns an inverted range.
/// Falsification: replayable `crates/static_elf/falsifications/verification.page_range_is_panic_free_and_ordered.patch`
#[kani::proof]
fn page_range_is_panic_free_and_ordered() {
    let seg = Segment {
        vaddr: kani::any(),
        paddr: kani::any(),
        memsz: kani::any(),
        flags: kani::any(),
        data: &[],
    };
    let page_bytes = PageBytes::new(1 << (kani::any::<u8>() % 64)).unwrap();
    let (start, end) = seg.page_range(page_bytes);
    assert!(start <= end);
}
