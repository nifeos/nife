use super::*;

/// **The gate is a conjunction.** The aligned high-half address is the case that tells `&&`
/// from `||`: it passes the alignment test, so an or-gate would admit a kernel address to a
/// user MAP request. One aligned low address per format keeps the gate from being shorted to
/// `false`, one unaligned low address keeps it from `true`.
#[test]
fn the_user_va_gate_admits_only_aligned_low_half_addresses() {
    assert!(is_user_page_va::<Aarch64>(0x1000));
    assert!(is_user_page_va::<Sv39>(0x1000));
    assert!(!is_user_page_va::<Aarch64>(0x1001));
    assert!(!is_user_page_va::<Sv39>(0x1001));
    // Page-aligned, but in the kernel half: all-ones above each format's split bit.
    assert!(!is_user_page_va::<Aarch64>(0xffff_0000_0000_1000));
    assert!(!is_user_page_va::<Sv39>(0xffff_ffc0_0000_1000));
}

/// **A leaf size fits only when `va` and `pa` are both aligned to it.** Each alone is not
/// enough, and the case that shows it is two addresses whose misaligned bits differ: their
/// intersection is aligned when neither is. The Kani harnesses prove this for every input;
/// this is the one `cargo test` (and so the mutation run) can see.
#[test]
fn a_leaf_fits_only_when_both_addresses_are_aligned_to_it() {
    let two = 2 << 20;
    let (va, pa) = (two + 0x1000, two + 0x2000);
    assert_eq!(
        va & pa & (two - 1),
        0,
        "the fixture: aligned together, not apart"
    );
    assert_eq!(
        PageSize::largest_fitting(va, pa, 4 * two, PageSize::Size1GiB),
        PageSize::Size4KiB
    );
    assert_eq!(
        PageSize::largest_fitting(two, two, 4 * two, PageSize::Size1GiB),
        PageSize::Size2MiB
    );
}

/// **The half bases, pinned.** `half_base` exists for the reader's model and the walk never
/// consults it, so only an exact value notices its shifts going the wrong way. The high base
/// is all-ones above the split: bit 48 for aarch64 (where TTBR1 takes over), bit 38 for Sv39
/// (the sign-extended top VA bit).
#[test]
fn half_bases_are_zero_and_the_sign_extended_top() {
    assert_eq!(Aarch64::half_base(Half::Low), 0);
    assert_eq!(Sv39::half_base(Half::Low), 0);
    assert_eq!(Aarch64::half_base(Half::High), 0xffff_0000_0000_0000);
    assert_eq!(Sv39::half_base(Half::High), 0xffff_ffc0_0000_0000);
}

/// **`root()` reports the frame the mapper was built on.** That value is what the kernel
/// writes into TTBR0/satp, and nothing in this crate reads it back, so a constant here would
/// install the wrong table in silicon while every walk test still passed.
#[test]
fn the_mapper_reports_the_root_it_was_built_on() {
    // SAFETY: nothing here walks the tables; new() stores its arguments and root() reads one
    // back, so the inert closures are never called.
    let m = unsafe {
        Mapper::<_, _, Aarch64>::new(0x8_2000, Half::Low, || None, |_| core::ptr::null_mut())
    };
    assert_eq!(m.root(), 0x8_2000);
}
