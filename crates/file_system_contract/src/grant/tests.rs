use crate::*;

#[test]
fn a_granted_name_survives_the_two_argument_words() {
    // A name that fits rides in START arguments rather than a frame, so the grant costs no page.
    // Every length up to the limit has to come back exactly, including the 8-byte boundary where
    // it splits across the two words.
    for name in [
        &b"a"[..],
        b"motd",
        b"scratch",
        b"12345678",
        b"123456789",
        b"sixteen-bytes!!!",
    ] {
        assert!(grant::fits_words(name), "{name:?} should fit two words");
        let (lo, hi) = grant::pack_name(name).expect("it fits two words");
        let mut buf = [0u8; grant::MAXIMUM_WORDS_NAME_BYTES];
        let n = grant::unpack_name(lo, hi, name.len(), &mut buf);
        assert_eq!(&buf[..n], name, "{name:?} did not survive the round trip");
    }
    assert!(!grant::fits_words(b""), "an empty name designates nothing");
    assert!(
        !grant::fits_words(b"seventeen-bytes!!"),
        "a name past two words must be refused where it is packed, not truncated silently",
    );
}

/// **`pack_name` refuses past the limit even when the caller did not check.** A prefix would be
/// total and would name a different file, so the packer is the check, and no caller can start a
/// process on a truncated name.
#[test]
fn pack_name_refuses_what_two_words_cannot_hold() {
    assert_eq!(grant::pack_name(b"sixteen-bytes!!!X"), None);
    assert_eq!(grant::pack_name(b""), None);
    assert!(grant::pack_name(b"sixteen-bytes!!!").is_some());
}

/// **What a grant can name is wider than what two words hold**: a 20-byte name
/// like `least_authority_demo` is grantable, and only the packer refuses it.
#[test]
fn a_granted_name_may_be_longer_than_two_words() {
    let long = [b'n'; grant::MAXIMUM_NAME_BYTES];
    assert!(grant::fits(b"least_authority_demo"));
    assert!(grant::fits(&long), "the longest grantable name");
    assert!(
        !grant::fits(&[b'n'; grant::MAXIMUM_NAME_BYTES + 1]),
        "one byte past it"
    );
    assert!(!grant::fits(b""));
    assert!(grant::pack_name(b"least_authority_demo").is_none());
    const { assert!(grant::MAXIMUM_WORDS_NAME_BYTES < grant::MAXIMUM_NAME_BYTES) };
    // The spec word's length field is one byte, so the longest name must still be sayable in it.
    assert_eq!(
        grant::specification_len(grant::specification(
            grant::MAXIMUM_NAME_BYTES,
            grant::rights::READ
        )),
        grant::MAXIMUM_NAME_BYTES
    );
}

#[test]
fn a_grant_specification_carries_its_length_and_its_direction_apart() {
    // A read grant and a write grant of the same name differ only here, so the two fields must
    // not bleed: a 16-byte name must not look like a write bit, and write must not lengthen it.
    let ro = grant::specification(16, grant::rights::READ);
    let rw = grant::specification(16, grant::rights::READ | grant::rights::WRITE);
    assert_eq!(grant::specification_len(ro), 16);
    assert_eq!(grant::specification_len(rw), 16);
    assert!(!grant::is_writable(ro));
    assert!(grant::is_writable(rw));
    // And a zero-rights spec grants nothing, rather than defaulting to something.
    assert!(!grant::is_writable(grant::specification(4, 0)));
}

// The tests from here down each pin something a mutation run over this crate showed nothing
// pinned. The equivalences (every `|` over disjoint masked fields, where `^` cannot differ) are
// recorded as such rather than tested.

/// The per-file grant words, byte for byte: the rights bits, the spec word, and a name packed
/// across the two argument words, up to the sixteenth byte and refused at the seventeenth.
#[test]
fn grant_words_pack_exactly() {
    assert_eq!([grant::rights::READ, grant::rights::WRITE], [1, 2]);
    let w = grant::specification(5, grant::rights::WRITE);
    assert_eq!(w, 5 | (2 << 8));
    assert_eq!(grant::specification_len(w), 5);
    assert_eq!(grant::specification_rights(w), grant::rights::WRITE);

    let (lo, hi) = grant::pack_name(b"abcdefghij").unwrap();
    assert_eq!(lo, u64::from_le_bytes(*b"abcdefgh"));
    assert_eq!(hi, u64::from_le_bytes(*b"ij\0\0\0\0\0\0"));
    let (lo16, hi16) = grant::pack_name(b"abcdefghijklmnop").unwrap();
    assert_eq!(lo16, u64::from_le_bytes(*b"abcdefgh"));
    assert_eq!(hi16, u64::from_le_bytes(*b"ijklmnop"));
    assert_eq!(grant::pack_name(b"abcdefghijklmnopq"), None);
}

/// **The root of the shell's namespace is an empty name with the rights beside it.** `fits` refuses an empty name, so zero cannot be a real directory's length, and a
/// caretaker reads it as "narrow the handle you hold" with the rights from the same word.
#[test]
fn a_root_grant_is_an_empty_name_carrying_its_rights() {
    assert!(!grant::fits(b""));
    assert!(grant::pack_name(b"").is_none());
    let specification = grant::specification(0, crate::directory::rights::REMOVE);
    let mut buf = [0u8; grant::MAXIMUM_WORDS_NAME_BYTES];
    assert_eq!(
        grant::unpack_name(0, 0, grant::specification_len(specification), &mut buf),
        0
    );
    assert_eq!(
        grant::specification_rights(specification),
        crate::directory::rights::REMOVE
    );
}

/// **A granted name of any length up to the limit comes back from the grant page whole**, beside a
/// name set at its own offset, and an empty name reads as the empty name, not as a refusal.
#[test]
fn a_grant_page_carries_every_name_length_up_to_the_limit() {
    let long = [b'n'; grant::MAXIMUM_NAME_BYTES];
    let mut set = [0u8; nameset::MAXIMUM_BYTES];
    let n = nameset::encode(&[(&long[..], false), (b"b", true)], &mut set).unwrap();
    for len in [0, 1, 16, 17, 64, grant::MAXIMUM_NAME_BYTES] {
        let mut page = [0xa5u8; grant::PAGE_USED_BYTES];
        grant::write_page(&long[..len], true, &set[..n], &mut page).unwrap();
        assert_eq!(grant::page_name(&page), Some(&long[..len]), "length {len}");
        assert!(nameset::contains(grant::page_name_set(&page), &long));
        assert!(nameset::contains(grant::page_name_set(&page), b"b"));
    }
    let mut page = [0xa5u8; grant::PAGE_USED_BYTES];
    grant::write_page(b"dir", true, &[], &mut page).unwrap();
    assert_eq!(nameset::iter(grant::page_name_set(&page)).count(), 0);
}

/// **The writer refuses rather than truncates**: a name past the limit, a set past its maximum,
/// or a page too short for both.
#[test]
fn a_grant_page_that_does_not_fit_is_refused() {
    let too_long = [b'n'; grant::MAXIMUM_NAME_BYTES + 1];
    let mut page = [0u8; grant::PAGE_USED_BYTES];
    assert_eq!(grant::write_page(&too_long, true, &[], &mut page), None);
    let big_set = [1u8; nameset::MAXIMUM_BYTES + 1];
    assert_eq!(grant::write_page(b"d", true, &big_set, &mut page), None);
    let mut short = [0u8; grant::PAGE_USED_BYTES - 1];
    assert_eq!(grant::write_page(b"d", true, &[], &mut short), None);
}

/// **A malformed granted name is `None`, never the empty name.** An empty name narrows nothing, so
/// a page that read as one by accident would hand the caretaker its whole namespace.
#[test]
fn a_malformed_grant_page_name_is_refused_not_read_as_the_root() {
    // A second record after the first.
    let mut two = [0u8; 16];
    nameset::encode(&[(b"a", true), (b"b", true)], &mut two).unwrap();
    assert_eq!(grant::page_name(&two), None);
    // A record that runs off the page.
    assert_eq!(grant::page_name(&[5, 1, b'a', b'b']), None);
    // No terminator after the record.
    assert_eq!(grant::page_name(&[1, 1, b'a']), None);
    // A length past the limit, though the bytes are there.
    let mut over = [0u8; 300];
    over[0] = 253;
    over[1] = 1;
    assert_eq!(grant::page_name(&over), None);
    // A flags bit nothing defines.
    assert_eq!(grant::page_name(&[1, 0x80, b'a', 0]), None);
    assert_eq!(grant::page_name(&[]), None);
    assert_eq!(grant::page_name(&[0]), Some(&b""[..]));
}

/// **START words that still carry a name are told apart from the ones a grant page goes with.**
#[test]
fn start_words_carrying_a_name_are_recognized() {
    let spec = grant::specification(0, 3);
    assert!(grant::words_carry_no_name(0, 0, spec));
    assert!(!grant::words_carry_no_name(1, 0, spec));
    assert!(!grant::words_carry_no_name(0, 1, spec));
    assert!(!grant::words_carry_no_name(
        0,
        0,
        grant::specification(4, 3)
    ));
}
