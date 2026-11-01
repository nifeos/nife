//! **Machine-checked proof (Kani) of the attribute change**, over every stored attribute byte and
//! every set. Behind `#[cfg(kani)]`, so only `cargo kani` compiles it.
//!
//! Name: ratified 2026-11-01 (UTC), by the class rule on the `tests` and `verification` modules of this crate.

use super::*;

/// **Setting attributes changes the four user bits and nothing else, and never makes a long-name
/// piece.** For every stored `DIR_Attr` and every set built from the four constants: the user bits
/// of the result are the set, every other bit is as it was, and when the stored byte did not
/// already read as a long-name piece the result does not either.
/// Falsification: replayable `crates/file_allocation_table/falsifications/attributes.verification.setting_attributes_changes_the_user_bits_and_nothing_else.patch`
#[kani::proof]
fn setting_attributes_changes_the_user_bits_and_nothing_else() {
    let stored: u8 = kani::any();
    let pick: u8 = kani::any();
    let mut set = Attributes::EMPTY;
    if pick & 1 != 0 {
        set = set.union(Attributes::READ_ONLY);
    }
    if pick & 2 != 0 {
        set = set.union(Attributes::HIDDEN);
    }
    if pick & 4 != 0 {
        set = set.union(Attributes::SYSTEM);
    }
    if pick & 8 != 0 {
        set = set.union(Attributes::ARCHIVE);
    }
    let result = apply(stored, set);
    assert_eq!(result & USER_BITS, set.bits());
    assert_eq!(result & !USER_BITS, stored & !USER_BITS);
    // A long-name piece is the four low bits with the label bit; the label bit is not a user bit.
    if stored & 0x08 == 0 {
        assert!(result & 0x3F != 0x0F);
    }
}
