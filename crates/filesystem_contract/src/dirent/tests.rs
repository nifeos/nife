use crate::*;

// The crate is `no_std`, so its own tests have to say where the allocating types come from. They
// are here only to build fixtures; nothing under test allocates.
extern crate std;
use std::vec;

// --- The directory capability ---

/// A listing round-trips: every name comes back byte for byte, with its kind, in order.
#[test]
fn a_directory_listing_round_trips_through_the_page() {
    let entries: [(&[u8], bool); 4] = [
        (b"a", false),
        (b"deeper", true),
        (b"inner", false),
        (b"a-name-that-is-quite-a-lot-longer-than-the-others", false),
    ];
    let mut page = [0u8; 128];
    let mut at = 0;
    for (name, is_dir) in entries {
        at += dirent::encode(&mut page[at..], name, is_dir).expect("encode");
    }
    let mut seen = 0;
    for (i, (name, is_dir)) in dirent::iter(&page[..at]).enumerate() {
        assert_eq!(name, entries[i].0, "entry {i}'s name");
        assert_eq!(is_dir, entries[i].1, "entry {i}'s kind");
        seen += 1;
    }
    assert_eq!(seen, entries.len());
}

/// **A record is never split**, and a truncated buffer yields the entries that are whole rather
/// than a fragment of a name. A reader that returned half a name would let a client act on a
/// name that was never in the directory.
#[test]
fn a_listing_that_does_not_fit_encodes_nothing_and_reads_back_nothing_partial() {
    let mut tiny = [0u8; 4];
    assert_eq!(dirent::encode(&mut tiny, b"ab", false), Some(4));
    assert_eq!(
        dirent::encode(&mut tiny, b"abc", false),
        None,
        "a record that does not fit must be refused, not clipped",
    );

    // A buffer cut in the middle of a name yields only the whole records before it.
    let mut page = [0u8; 32];
    let n = dirent::encode(&mut page, b"one", false).unwrap()
        + dirent::encode(&mut page[5..], b"two", false).unwrap();
    for cut in 0..n {
        let seen = dirent::iter(&page[..cut]).count();
        assert!(seen <= 1, "a cut at {cut} produced a torn entry");
    }
    assert_eq!(dirent::iter(&page[..n]).count(), 2);
}

/// **A 255-byte name fills the length byte exactly, and 256 is refused.** The boundary is
/// load-bearing because `encode` writes `name.len() as u8`: a check that let 256 through would
/// write a length byte of 0, and the far side would read an empty name followed by 256 bytes of
/// somebody else's records.
#[test]
fn a_dirent_name_fills_the_length_byte_exactly_or_is_refused() {
    let mut page = [0u8; 512];
    let longest = [b'n'; 255];
    let n = dirent::encode(&mut page, &longest, false).expect("255 fits a length byte");
    assert_eq!(n, dirent::record_len(255));
    let (name, is_dir) = dirent::iter(&page[..n]).next().expect("readable");
    assert_eq!(name, &longest[..]);
    assert!(!is_dir);
    let over = [b'n'; 256];
    assert_eq!(
        dirent::encode(&mut page, &over, false),
        None,
        "a length byte cannot say 256"
    );
}

// The tests from here down each pin something a mutation run over this crate showed nothing
// pinned. The equivalences (every `|` over disjoint masked fields, where `^` cannot differ) are
// recorded as such rather than tested.

/// A directory entry at both of its limits: the longest encodable name in an exact-fit
/// buffer, one byte over either limit refused, and the flag byte pinned.
#[test]
fn a_dirent_encodes_at_its_limits_and_not_past_them() {
    let name = vec![b'n'; 255];
    let mut out = vec![0u8; dirent::record_len(255)];
    assert_eq!(dirent::encode(&mut out, &name, true), Some(257));
    assert_eq!(out[0], dirent::IS_DIR);
    assert_eq!(out[1], 255);
    let mut short = vec![0u8; 256];
    assert_eq!(
        dirent::encode(&mut short, &name, true),
        None,
        "a buffer one byte short"
    );
    let long = vec![b'n'; 256];
    let mut out = vec![0u8; dirent::record_len(256)];
    assert_eq!(
        dirent::encode(&mut out, &long, false),
        None,
        "a name one byte too long"
    );
}
