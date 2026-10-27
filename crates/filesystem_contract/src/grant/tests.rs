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
        let mut buf = [0u8; grant::WORDS_NAME];
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

/// **What a grant can name is wider than what two words hold** (milestone 946 (a granted file name can be longer than sixteen bytes)): a 20-byte name
/// like `least_authority_demo` is grantable, and only the packer refuses it.
#[test]
fn a_granted_name_may_be_longer_than_two_words() {
    let long = [b'n'; grant::MAX_NAME];
    assert!(grant::fits(b"least_authority_demo"));
    assert!(grant::fits(&long), "the longest grantable name");
    assert!(
        !grant::fits(&[b'n'; grant::MAX_NAME + 1]),
        "one byte past it"
    );
    assert!(!grant::fits(b""));
    assert!(grant::pack_name(b"least_authority_demo").is_none());
    const { assert!(grant::WORDS_NAME < grant::MAX_NAME) };
    // The spec word's length field is one byte, so the longest name must still be sayable in it.
    assert_eq!(
        grant::spec_len(grant::spec(grant::MAX_NAME, grant::READ)),
        grant::MAX_NAME
    );
}

#[test]
fn a_grant_spec_carries_its_length_and_its_direction_apart() {
    // A read grant and a write grant of the same name differ only here, so the two fields must
    // not bleed: a 16-byte name must not look like a write bit, and write must not lengthen it.
    let ro = grant::spec(16, grant::READ);
    let rw = grant::spec(16, grant::READ | grant::WRITE);
    assert_eq!(grant::spec_len(ro), 16);
    assert_eq!(grant::spec_len(rw), 16);
    assert!(!grant::is_writable(ro));
    assert!(grant::is_writable(rw));
    // And a zero-rights spec grants nothing, rather than defaulting to something.
    assert!(!grant::is_writable(grant::spec(4, 0)));
}

// The tests from here down each pin something the run of milestone 85 (mutation testing over the
// host crates) showed nothing pinned. The equivalences (every `|` over disjoint masked fields,
// where `^` cannot differ) are recorded in notes/mutation-testing.md rather than tested.

/// The per-file grant words, byte for byte: the rights bits, the spec word, and a name packed
/// across the two argument words, up to the sixteenth byte and refused at the seventeenth.
#[test]
fn grant_words_pack_exactly() {
    assert_eq!([grant::READ, grant::WRITE], [1, 2]);
    let w = grant::spec(5, grant::WRITE);
    assert_eq!(w, 5 | (2 << 8));
    assert_eq!(grant::spec_len(w), 5);
    assert_eq!(grant::spec_rights(w), grant::WRITE);

    let (lo, hi) = grant::pack_name(b"abcdefghij").unwrap();
    assert_eq!(lo, u64::from_le_bytes(*b"abcdefgh"));
    assert_eq!(hi, u64::from_le_bytes(*b"ij\0\0\0\0\0\0"));
    let (lo16, hi16) = grant::pack_name(b"abcdefghijklmnop").unwrap();
    assert_eq!(lo16, u64::from_le_bytes(*b"abcdefgh"));
    assert_eq!(hi16, u64::from_le_bytes(*b"ijklmnop"));
    assert_eq!(grant::pack_name(b"abcdefghijklmnopq"), None);
}
