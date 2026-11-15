//! **Machine-checked proofs (Kani) of the inert-configuration page's reader**: `read_field` never
//! panics, whatever the length byte says, and reports absence for a length past its field. Behind
//! `#[cfg(kani)]`, so only `cargo kani` (script/verify) compiles them. See notes/verification.md.
//!
//! Name: provisional, after the module every other crate's harnesses live in.

use super::*;
/// **`field_bytes`, which `read_field` is built on, never panics for any length byte, and a length
/// past the field reads as absent.** (`read_field` adds only `core::str::from_utf8`, which is total.)
/// **`read_field` never panics for any length byte, and a length past the field reads as absent.**
/// The page is the real size, `TERM`'s field is the last on it, and the length byte is free, so the
/// case that once sliced past the end of the page (a length of 255) is among the inputs.
/// Falsification: replayable `crates/environment_contract/falsifications/verification.field_bytes_never_panics_for_any_length_byte.patch`
#[kani::proof]
#[kani::unwind(2)]
fn field_bytes_never_panics_for_any_length_byte() {
    let mut page = [0u8; PAGE_BYTES];
    let length: u8 = kani::any();
    page[TERMINAL_TYPE_LENGTH_OFFSET_BYTES] = length;
    let read = field_bytes(
        &page,
        TERMINAL_TYPE_LENGTH_OFFSET_BYTES,
        TERMINAL_TYPE_OFFSET_BYTES,
        MAXIMUM_TERMINAL_TYPE_BYTES,
    );
    if usize::from(length) > MAXIMUM_TERMINAL_TYPE_BYTES {
        assert!(read.is_none());
    }
}
