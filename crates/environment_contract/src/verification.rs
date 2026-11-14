//! **Machine-checked proofs (Kani) of the inert-configuration page's reader**: `read_field` never
//! panics, whatever the length byte says, and reports absence for a length past its field. Behind
//! `#[cfg(kani)]`, so only `cargo kani` (script/verify) compiles them. See notes/verification.md.
//!
//! Name: provisional, after the module every other crate's harnesses live in.

use super::*;
/// **`field_bytes`, which `read_field` is built on, never panics for any length byte, and a length
/// past the field reads as absent.** (`read_field` adds only `core::str::from_utf8`, which is total,
/// and is left out because a symbolic UTF-8 scan is the expensive part and not the bound.)
/// The page is the real size, `TERM`'s field is the last on it, and the length byte is free, so the
/// case that once sliced past the end of the page (a length of 255) is among the inputs.
/// Falsification: replayable `crates/environment_contract/falsifications/verification.field_bytes_never_panics_for_any_length_byte.patch`
#[kani::proof]
#[kani::unwind(2)]
fn field_bytes_never_panics_for_any_length_byte() {
    let mut page = [0u8; PAGE_BYTES];
    let length: u8 = kani::any();
    page[TERMINAL_TYPE_LENGTH_OFFSET] = length;
    let read = field_bytes(
        &page,
        TERMINAL_TYPE_LENGTH_OFFSET,
        TERMINAL_TYPE_OFFSET,
        MAXIMUM_TERMINAL_TYPE_BYTES,
    );
    if usize::from(length) > MAXIMUM_TERMINAL_TYPE_BYTES {
        assert!(read.is_none());
    }
}

/// **A written field reads back whole.** For any ASCII value of one to 24 bytes, `write_field` then
/// `field_bytes` returns the same bytes, so the writer, the layout and the reader agree on the
/// offset and the length of the last field on the page. The value is a symbolic ASCII array, so the
/// proof stays on the bound and not on the domain lists.
/// Falsification: replayable `crates/environment_contract/falsifications/verification.a_written_field_reads_back_whole.patch`
#[kani::proof]
#[kani::unwind(26)]
fn a_written_field_reads_back_whole() {
    let bytes: [u8; MAXIMUM_TERMINAL_TYPE_BYTES] = kani::any();
    let length: usize = kani::any();
    kani::assume(length >= 1 && length <= MAXIMUM_TERMINAL_TYPE_BYTES);
    let value = &bytes[..length];
    kani::assume(value.is_ascii());
    let text = core::str::from_utf8(value).unwrap();
    let mut page = [0u8; PAGE_BYTES];
    write_field(
        &mut page,
        TERMINAL_TYPE_LENGTH_OFFSET,
        TERMINAL_TYPE_OFFSET,
        MAXIMUM_TERMINAL_TYPE_BYTES,
        Some(text),
    );
    assert!(
        field_bytes(
            &page,
            TERMINAL_TYPE_LENGTH_OFFSET,
            TERMINAL_TYPE_OFFSET,
            MAXIMUM_TERMINAL_TYPE_BYTES
        ) == Some(value)
    );
}

/// **A value an accessor returns is a member of its key's domain, whatever the length byte says.**
/// The page carries the member `UTC` and a free time zone length byte, so a length that cuts the
/// member short, or runs into the zeroes after it, must read as absent and never as a prefix.
/// The length is bounded at eight, which covers the member, the cap-free cut points and five zero
/// bytes after it, because a length past that only adds more zero bytes and no domain member holds
/// a zero byte (the host test `no_domain_member_holds_a_zero_byte` checks that). The bound keeps the
/// symbolic text scan small enough to prove in minutes.
/// Falsification: replayable `crates/environment_contract/falsifications/verification.an_accessor_returns_only_a_domain_member.patch`
#[kani::proof]
#[kani::unwind(22)]
fn an_accessor_returns_only_a_domain_member() {
    let mut page = PageBuilder::new().tz("UTC").unwrap().build();
    let length: u8 = kani::any();
    kani::assume(length <= 8);
    page[TIME_ZONE_LENGTH_OFFSET] = length;
    // SAFETY: `page` is a live array of exactly `PAGE_BYTES`, alive for this function.
    let read = unsafe { ConfigPage::new(page.as_ptr() as u64) };
    if let Some(value) = read.tz() {
        assert!(domain::KNOWN_TIME_ZONES.contains(&value));
    }
}
