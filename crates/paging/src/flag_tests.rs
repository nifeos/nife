use super::*;

const ALL: [Flags; 8] = [
    Flags::kernel_code(),
    Flags::kernel_rodata(),
    Flags::kernel_data(),
    Flags::device(),
    Flags::user_code(),
    Flags::user_rodata(),
    Flags::user_data(),
    Flags::user_device(),
];

/// **W^X, as a property of the type rather than of our discipline.** No constructor returns a
/// page that is both writable and executable, over every constructor there is.
#[test]
fn nothing_is_both_writable_and_executable() {
    for f in ALL {
        assert!(
            !(f.is_writable() && (f.is_kernel_executable() || f.is_user_executable())),
            "{f:?} is both writable and executable",
        );
    }
}

/// **The execute split.** Anything user-reachable is not kernel-executable (a wild kernel jump
/// into a user page faults instead of running user-chosen instructions in supervisor mode), and
/// anything kernel-only is not user-executable.
#[test]
fn no_page_is_executable_across_the_privilege_split() {
    for f in ALL {
        if f.is_user_accessible() {
            assert!(
                !f.is_kernel_executable(),
                "{f:?} user-reachable yet kernel-exec"
            );
        } else {
            assert!(!f.is_user_executable(), "{f:?} kernel-only yet user-exec");
        }
    }
}

#[test]
fn user_rodata_is_readable_and_nothing_else() {
    let f = Flags::user_rodata();
    assert!(f.is_user_accessible());
    assert!(!f.is_writable());
    assert!(!f.is_user_executable());
    assert!(!f.is_kernel_executable());
}

#[test]
fn user_device_is_device_typed_user_accessible_and_never_executable() {
    let f = Flags::user_device();
    assert!(f.is_user_accessible());
    assert!(f.is_writable());
    assert!(!f.is_user_executable() && !f.is_kernel_executable());
    assert!(f.is_device(), "a driver's MMIO must be device-typed");
}

/// **The ASID/global tagging split** (milestone 15): everything user-reachable is non-global
/// (its TLB entries are tagged, so a context switch flushes nothing), everything kernel-only is
/// global.
#[test]
fn user_mappings_are_tagged_and_kernel_mappings_are_global() {
    for f in ALL {
        assert_eq!(
            f.is_global(),
            !f.is_user_accessible(),
            "{f:?} is on the wrong side of the ASID split",
        );
    }
}

/// **`bits()` returns the word, not a constant.** The formats encode through the predicates,
/// so nothing else in this crate reads the raw word back, and a `bits()` that returned 0 or 1
/// would survive every other test here. Expected values hand-assembled from the CAP_* bit
/// positions above: write 0, user 1, user-exec 2, kernel-exec 3, global 4, device 5.
#[test]
fn bits_returns_the_exact_capability_word() {
    assert_eq!(Flags::kernel_code().bits(), 0b01_1000);
    assert_eq!(Flags::kernel_rodata().bits(), 0b01_0000);
    assert_eq!(Flags::kernel_data().bits(), 0b01_0001);
    assert_eq!(Flags::device().bits(), 0b11_0001);
    assert_eq!(Flags::user_code().bits(), 0b00_0110);
    assert_eq!(Flags::user_rodata().bits(), 0b00_0010);
    assert_eq!(Flags::user_data().bits(), 0b00_0011);
    assert_eq!(Flags::user_device().bits(), 0b10_0011);
    assert_eq!(Flags::write_combining().bits(), 0b111_0001);
    assert_eq!(Flags::user_write_combining().bits(), 0b110_0011);
}

/// **The pair the kernel refuses is exactly writable beside executable**, in either order and for
/// either privilege's execute bit, over every constructor. The expected sets are written out by
/// name rather than through the predicates the function reads, so a predicate that drifted would
/// not drift the answer with it.
#[test]
fn only_writable_beside_executable_makes_the_pair() {
    let every = [
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
    ];
    let writable = [
        Flags::kernel_data(),
        Flags::device(),
        Flags::write_combining(),
        Flags::user_data(),
        Flags::user_device(),
        Flags::user_write_combining(),
    ];
    let executable = [Flags::kernel_code(), Flags::user_code()];
    for a in every {
        for b in every {
            let expected = (writable.contains(&a) && executable.contains(&b))
                || (writable.contains(&b) && executable.contains(&a));
            assert_eq!(
                a.makes_writable_executable_pair(b),
                expected,
                "{a:?} beside {b:?}",
            );
        }
    }
    // The cases the narrow rule exists to let through, named.
    assert!(!Flags::user_rodata().makes_writable_executable_pair(Flags::user_data()));
    assert!(!Flags::user_rodata().makes_writable_executable_pair(Flags::user_code()));
    assert!(!Flags::user_data().makes_writable_executable_pair(Flags::user_data()));
    assert!(!Flags::user_code().makes_writable_executable_pair(Flags::user_code()));
}

/// A write-combining mapping is still a device mapping: every rule keyed on `is_device` (no
/// caching, pages rather than blocks, no inner shareability) must keep applying to it.
#[test]
fn write_combining_is_a_kind_of_device_memory() {
    let f = Flags::write_combining();
    assert!(f.is_device() && f.is_write_combining() && f.is_writable() && f.is_global());
    assert!(!f.is_user_accessible() && !f.is_kernel_executable() && !f.is_user_executable());
    assert!(!Flags::device().is_write_combining());
}
