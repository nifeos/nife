//! **The eighth outsider pass's host attacks on the geometry gates** (claims 10 and 11): the
//! split-edge sweep the cited per-format tests do not shape, and the cross-format W^X sweep over
//! every public constructor. Name: provisional (this pass's lane, 2026-10-13 UTC).

use super::*;

/// **The user-va gate is exact at every format's own split, from both sides.**
///
/// The cited test checks one aligned low address, one unaligned one and one kernel-half address
/// per format. This sweeps the boundary itself: the last page of the low half is admitted, the
/// first page of the high half (page-aligned, so alignment alone cannot save the gate) is
/// refused, the unaligned neighbors on both sides are refused, and the all-ones address is
/// refused. A split constant off by one page, or a gate testing `>=` where it means `>`, moves
/// one of these and nothing else.
#[test]
fn the_user_va_gate_is_exact_at_each_formats_own_split() {
    fn edges<F: PageFormat>() {
        let split = 1u64 << F::SPLIT_SHIFT;
        assert!(is_user_page_va::<F>(0), "the null page is low and aligned");
        assert!(is_user_page_va::<F>(PAGE_SIZE));
        assert!(!is_user_page_va::<F>(1));
        assert!(
            is_user_page_va::<F>(split - PAGE_SIZE),
            "the last page of the low half must be admitted",
        );
        assert!(
            !is_user_page_va::<F>(split - 1),
            "unaligned at the top of the low half"
        );
        assert!(
            !is_user_page_va::<F>(split),
            "the first page of the high half is page-aligned: only the half test refuses it",
        );
        assert!(!is_user_page_va::<F>(split | PAGE_SIZE));
        assert!(!is_user_page_va::<F>(u64::MAX));
        assert!(
            !is_user_page_va::<F>(0x7fff_ffff_ffff_f000),
            "a high-ish address with bit 63 clear is not in the low half",
        );
    }
    edges::<Aarch64>();
    edges::<Sv39>();
    edges::<Ia32e>();
}

/// **No leaf of any format is both writable and executable, over every public constructor.**
///
/// The cited tests assert this per format against that format's own raw bits. This one decodes
/// through `leaf_flags`, the portable read side, so it holds the whole set at once: a
/// constructor or encoder that manufactures the combination in one format only is caught by
/// the same loop. `Vtd` is in the set too, where the claim is vacuous but free: VT-d has no
/// execute bit to set and the assertion says so.
#[test]
fn no_leaf_of_any_format_is_both_writable_and_executable() {
    fn sweep<F: PageFormat>() {
        for flags in [
            Flags::kernel_code(),
            Flags::kernel_rodata(),
            Flags::kernel_data(),
            Flags::device(),
            Flags::write_combining(),
            Flags::user_code(),
            Flags::user_rodata(),
            Flags::user_data(),
            Flags::user_device(),
            Flags::user_write_combining(),
        ] {
            let leaf = F::leaf_entry(0x10_0000, flags);
            let decoded = F::leaf_flags(leaf);
            assert!(
                !(decoded.is_writable()
                    && (decoded.is_kernel_executable() || decoded.is_user_executable())),
                "{flags:?} encoded a leaf both writable and executable: {leaf:#x}",
            );
        }
    }
    sweep::<Aarch64>();
    sweep::<Sv39>();
    sweep::<Ia32e>();
    sweep::<Vtd>();
}
