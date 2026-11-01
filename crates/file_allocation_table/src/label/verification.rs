//! **Machine-checked proof (Kani) of the label check**, over every label of up to twelve bytes.
//! Behind `#[cfg(kani)]`, so only `cargo kani` compiles it.
//!
//! Name: ratified 2026-11-01 (UTC), by the class rule on the `tests` and `verification` modules of this crate.

use crate::format::label_from_bytes;
use crate::long_name::is_short_byte;

/// **An accepted label is eleven legal bytes holding the text and then spaces.** For every byte
/// string of up to twelve bytes that parses as text: the check accepts exactly the non-empty ones
/// of at most eleven bytes that a short name admits, and an accepted label is the text followed by
/// spaces to eleven.
/// Falsification: replayable `crates/file_allocation_table/falsifications/label.verification.an_accepted_label_is_legal_bytes_then_spaces.patch`
#[kani::proof]
#[kani::unwind(14)]
fn an_accepted_label_is_legal_bytes_then_spaces() {
    let bytes: [u8; 12] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= 12);
    let legal = (1..=11).contains(&len) && bytes[..len].iter().all(|b| is_short_byte(*b));
    match label_from_bytes(&bytes[..len]) {
        Some(label) => {
            assert!(legal);
            assert_eq!(label[..len], bytes[..len]);
            assert!(label[len..].iter().all(|b| *b == b' '));
        }
        None => assert!(!legal),
    }
}
