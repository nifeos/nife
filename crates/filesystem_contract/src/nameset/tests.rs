use crate::*;

// --- The directory capability (milestone 47 (navigation and naming)) ---

/// The set encoding round-trips, carries the type bit, and **refuses rather than truncates**.
///
/// The refusals are the half worth testing: `encode` is building a capability, and a capability
/// that quietly held fewer names than the command matched is the exact failure this lane is
/// supposed to make impossible.
#[test]
fn a_name_set_round_trips_and_refuses_rather_than_truncating() {
    let mut buf = [0u8; nameset::BYTES];
    let names: [(&[u8], bool); 3] = [(b"one.txt", false), (b"logs", true), (b"z", false)];
    let n = nameset::encode(&names, &mut buf).expect("three short names must fit");
    assert_eq!(nameset::count(&buf[..n]), 3);
    let want: [(&[u8], bool); 3] = [(b"one.txt", false), (b"logs", true), (b"z", false)];
    for (got, want) in nameset::iter(&buf[..n]).zip(want) {
        assert_eq!(got, want, "the set did not come back as it went in");
    }
    assert!(nameset::contains(&buf[..n], b"logs"));
    assert!(
        !nameset::contains(&buf[..n], b"log"),
        "a prefix is a different name; the caretaker's whole check is this comparison",
    );

    // A full set fits exactly, and one more name does not: BYTES is a bound that is met, not a
    // guess with slack in it. The widest set is the most names whose bytes together reach
    // NAME_BYTES, which is MAX_NAMES names sharing it.
    let each = nameset::NAME_BYTES / nameset::MAX_NAMES;
    let wide = [b'w'; nameset::NAME_BYTES / nameset::MAX_NAMES];
    let full: [(&[u8], bool); nameset::MAX_NAMES] = [(&wide[..], false); nameset::MAX_NAMES];
    assert_eq!(
        nameset::encode(&full, &mut buf),
        Some(2 * nameset::MAX_NAMES + each * nameset::MAX_NAMES + 1),
    );
    assert_eq!(
        each * nameset::MAX_NAMES,
        nameset::NAME_BYTES,
        "the full set meets the bound"
    );
    assert_eq!(
        2 * nameset::MAX_NAMES + nameset::NAME_BYTES + 1,
        nameset::BYTES,
        "the widest set this contract carries must fit its own buffer exactly",
    );
    // And the longest name travels in a set (milestone 946 (a granted file name can be longer than sixteen bytes)), twice over, with room for nothing
    // that would take the set past its bytes.
    let longest = [b'l'; grant::MAX_NAME];
    let two: [(&[u8], bool); 2] = [(&longest[..], false), (&longest[..], true)];
    let n = nameset::encode(&two, &mut buf).expect("two of the longest names fit");
    assert_eq!(nameset::iter(&buf[..n]).count(), 2);
    assert!(nameset::contains(&buf[..n], &longest));
    let spill = [b's'; nameset::NAME_BYTES - 2 * grant::MAX_NAME + 1];
    let three: [(&[u8], bool); 3] = [
        (&longest[..], false),
        (&longest[..], true),
        (&spill[..], false),
    ];
    assert_eq!(
        nameset::encode(&three, &mut buf),
        None,
        "one byte past NAME_BYTES is refused, not dropped",
    );
    let over: [(&[u8], bool); nameset::MAX_NAMES + 1] = [(b"a", false); nameset::MAX_NAMES + 1];
    assert_eq!(nameset::encode(&over, &mut buf), None, "one name too many");
    assert_eq!(
        nameset::encode(&[(&[b'n'; grant::MAX_NAME + 1][..], false)], &mut buf),
        None,
        "a name that cannot travel in a grant cannot travel in a set either",
    );
    assert_eq!(
        nameset::encode(&[(b"", false)], &mut buf),
        None,
        "an empty name is the terminator, so it cannot also be a name",
    );
    // And a buffer one byte short refuses rather than dropping the terminator, which would make
    // the reader run off the end of the set and into whatever followed it.
    let mut tight = [0u8; 9];
    assert_eq!(nameset::encode(&[(b"one.txt", false)], &mut tight), None);
    // The fit check runs before the record's first byte is laid, so a single-record refusal
    // leaves the buffer clean. (Refusing a later record can leave earlier ones behind; the
    // `None` is what the caller must honour either way.) Without this, a refusal could still
    // have scribbled most of a capability into a buffer somebody reuses.
    assert_eq!(
        tight, [0u8; 9],
        "a refused encode left partial capability bytes behind"
    );
    // A buffer too short for even the record is the same refusal, not a slice panic: the one
    // check covers the record and the terminator together.
    let mut shorter = [0u8; 8];
    assert_eq!(nameset::encode(&[(b"one.txt", false)], &mut shorter), None);
}

// The tests from here down each pin something the run of milestone 85 (mutation testing over the
// host crates) showed nothing pinned. The equivalences (every `|` over disjoint masked fields,
// where `^` cannot differ) are recorded in notes/mutation-testing.md rather than tested.

/// A name set, byte for byte, in an exact-fit buffer: the length byte and the flags byte per
/// record (`IS_DIR` in the flags), the terminator, and one byte less refused. The record cursor is
/// `n + 2 + name.len()`, and either `+` rotting turns the layout into overlapping records with
/// every length still adding up.
#[test]
fn a_nameset_lays_out_byte_for_byte() {
    let names: [(&[u8], bool); 2] = [(b"ab", false), (b"cde", true)];
    let mut out = [0u8; 10];
    assert_eq!(nameset::encode(&names, &mut out), Some(10));
    assert_eq!(
        out,
        [2, 0, b'a', b'b', 3, nameset::IS_DIR, b'c', b'd', b'e', 0]
    );
    let mut short = [0u8; 9];
    assert_eq!(nameset::encode(&names, &mut short), None);
}

/// **A record that runs off the end is not a name**, so a set cut short mid-record reads as the
/// names before it, and a length byte with no flags byte behind it reads as nothing.
#[test]
fn a_truncated_set_stops_at_the_last_whole_record() {
    let full = [2, 0, b'a', b'b', 3, nameset::IS_DIR, b'c', b'd', b'e', 0];
    assert_eq!(nameset::count(&full[..7]), 1);
    assert_eq!(nameset::count(&full[..5]), 1);
    assert_eq!(nameset::count(&full[..1]), 0);
}
