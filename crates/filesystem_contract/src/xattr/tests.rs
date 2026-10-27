use crate::*;

// The crate is `no_std`, so its own tests have to say where the allocating types come from. They
// are here only to build fixtures; nothing under test allocates.
extern crate std;
use std::vec::Vec;
use std::{format, vec};

// --- Extended attributes (milestone 57 (extended attributes)) ---

/// Scratch big enough for any blob this store can hold, so a test never fails on its own buffer.
fn scratch() -> Vec<u8> {
    vec![0u8; xattr::store::MAX_BYTES]
}

/// **A name and a value survive the store round trip, at every length the limits allow**,
/// including the two that a `u8` length byte and a `u16` value length are most likely to get
/// wrong. A value read back short is a file that looks intact and is not, which is the exact
/// failure DECISIONS §42 says must never be silent.
#[test]
fn an_attribute_round_trips_at_the_edges_of_both_limits() {
    use xattr::store;
    let widest_name = vec![b'n'; xattr::MAX_NAME];
    let widest_value = vec![0xABu8; xattr::MAX_VALUE];
    for (name, value) in [
        (&b"user.DOSATTRIB"[..], &b"0x20"[..]),
        (&b"a"[..], &b""[..]),
        (&widest_name[..], &widest_value[..]),
        (
            &b"user.com.apple.metadata:_kMDItemUserTags"[..],
            &[0u8; 1][..],
        ),
    ] {
        let mut out = scratch();
        let n = store::set(&[], name, xattr::RAW, value, &mut out).expect("set");
        assert_eq!(store::count(&out[..n]), 1);
        let (kind, got) = store::get(&out[..n], name).expect("the name we just wrote");
        assert_eq!(kind, xattr::RAW);
        assert_eq!(got, value, "the value did not come back as it went in");
    }
}

/// **The kind is carried and never interpreted**, which is the whole of "do not foreclose
/// indexing" (`design/haiku-bfs-and-packages.md`). A BFS-style four-character code has to survive
/// the store and the reply word, and a code with bit 31 set has to be refused rather than
/// truncated into a type nobody wrote.
#[test]
fn a_type_code_survives_the_store_and_the_reply_word() {
    use xattr::store;
    // 'CSTR' and 'LONG', BFS's own spellings, plus the largest code the reply word can carry.
    for kind in [xattr::RAW, 0x4353_5452, 0x4C4F_4E47, xattr::MAX_KIND] {
        let mut out = scratch();
        let n = store::set(&[], b"typed", kind, b"body", &mut out).expect("set");
        assert_eq!(store::get(&out[..n], b"typed").unwrap().0, kind);

        let r = xattr::reply(kind, 4);
        assert!(r >= 0, "a reply word must never read as an error");
        assert_eq!(xattr::reply_kind(r), kind, "kind through the reply word");
        assert_eq!(
            xattr::reply_value_len(r),
            4,
            "length through the reply word"
        );
    }
    // Bit 31 is not the store's to give away: refuse it where it is written.
    let mut out = scratch();
    assert_eq!(
        store::set(&[], b"toobig", xattr::MAX_KIND + 1, b"x", &mut out),
        Err(22),
        "a kind that cannot ride home must be refused, not truncated",
    );
}

/// **Every limit refuses rather than truncating**, and each with its own errno, so a caller can
/// tell "your name is wrong" from "your value is too big" from "this file is full". One errno for
/// all three would make the honest failure §42 demands unactionable.
#[test]
fn each_limit_answers_with_its_own_word_and_nothing_is_clipped() {
    use xattr::store;
    let mut out = scratch();
    assert_eq!(
        store::set(&[], b"", xattr::RAW, b"v", &mut out),
        Err(xattr::ERANGE)
    );
    assert_eq!(
        store::set(
            &[],
            &vec![b'x'; xattr::MAX_NAME + 1],
            xattr::RAW,
            b"v",
            &mut out
        ),
        Err(xattr::ERANGE),
    );
    assert_eq!(
        store::set(&[], b"has\0nul", xattr::RAW, b"v", &mut out),
        Err(xattr::ERANGE),
        "a NUL would come back as a shorter name than went out",
    );
    assert_eq!(
        store::set(
            &[],
            b"big",
            xattr::RAW,
            &vec![0u8; xattr::MAX_VALUE + 1],
            &mut out
        ),
        Err(xattr::E2BIG),
    );

    // Fill a node to its ceiling, then ask for one more.
    let mut blob: Vec<u8> = Vec::new();
    for i in 0..xattr::MAX_COUNT {
        let mut next = scratch();
        let n = store::set(
            &blob,
            format!("a{i}").as_bytes(),
            xattr::RAW,
            b"v",
            &mut next,
        )
        .expect("within the count");
        blob = next[..n].to_vec();
    }
    assert_eq!(store::count(&blob), xattr::MAX_COUNT);
    assert_eq!(
        store::set(&blob, b"one-too-many", xattr::RAW, b"v", &mut out),
        Err(xattr::ENOSPC),
    );
    // And the ceiling is on NEW names only: replacing one of the sixteen still works, which is
    // what keeps a full file usable rather than frozen.
    let n = store::set(&blob, b"a0", xattr::RAW, b"replaced", &mut out).expect("replace at cap");
    assert_eq!(store::count(&out[..n]), xattr::MAX_COUNT);
    assert_eq!(store::get(&out[..n], b"a0").unwrap().1, b"replaced");
}

/// A replacement lands **in place**, so a listing does not shuffle when a value changes, and a
/// new name appends. A client that walked a listing while updating values would otherwise see
/// names move under it for no reason it could observe.
#[test]
fn a_replacement_keeps_its_position_and_a_new_name_appends() {
    use xattr::store;
    let (mut blob, mut out) = (Vec::new(), scratch());
    for name in [&b"one"[..], b"two", b"three"] {
        let n = store::set(&blob, name, xattr::RAW, b"v1", &mut out).unwrap();
        blob = out[..n].to_vec();
    }
    let names = |b: &[u8]| store::iter(b).map(|(n, ..)| n.to_vec()).collect::<Vec<_>>();
    assert_eq!(
        names(&blob),
        [b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]
    );

    // A longer value in the middle: the order holds and only that record changes.
    let n = store::set(&blob, b"two", 7, b"a much longer value", &mut out).unwrap();
    assert_eq!(
        names(&out[..n]),
        [b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]
    );
    assert_eq!(
        store::get(&out[..n], b"two"),
        Some((7, &b"a much longer value"[..]))
    );
    assert_eq!(
        store::get(&out[..n], b"one"),
        Some((xattr::RAW, &b"v1"[..]))
    );
}

/// Removing takes exactly one attribute and leaves the rest byte for byte, and removing what is
/// not there is [`xattr::ENODATA`] rather than a quiet success. The caller asked about one
/// specific thing.
#[test]
fn removing_takes_one_attribute_and_says_so_when_there_is_none() {
    use xattr::store;
    let (mut blob, mut out) = (Vec::new(), scratch());
    for name in [&b"keep-a"[..], b"drop", b"keep-b"] {
        let n = store::set(&blob, name, xattr::RAW, name, &mut out).unwrap();
        blob = out[..n].to_vec();
    }
    let n = store::remove(&blob, b"drop", &mut out).expect("remove");
    assert_eq!(store::count(&out[..n]), 2);
    assert_eq!(store::get(&out[..n], b"drop"), None);
    assert_eq!(
        store::get(&out[..n], b"keep-a"),
        Some((xattr::RAW, &b"keep-a"[..]))
    );
    assert_eq!(
        store::get(&out[..n], b"keep-b"),
        Some((xattr::RAW, &b"keep-b"[..]))
    );
    assert_eq!(
        store::remove(&out[..n], b"drop", &mut scratch()),
        Err(xattr::ENODATA)
    );
    // The last one out leaves an empty blob, which is how the server knows to delete the file.
    let mut b = out[..n].to_vec();
    for name in [&b"keep-a"[..], b"keep-b"] {
        let m = store::remove(&b, name, &mut out).unwrap();
        b = out[..m].to_vec();
    }
    assert_eq!(b.len(), 0);
}

/// **A truncated or zero-padded blob yields fewer attributes, never a torn one.** This is the
/// property that matters after a crash: RedoxFS gives us whole transactions, but the reader must
/// not be the weak link, and a value that came back with the next record's bytes on the end of it
/// would be corruption the checksums could not see.
#[test]
fn a_torn_blob_reads_back_short_rather_than_wrong() {
    use xattr::store;
    let mut out = scratch();
    let mut blob = Vec::new();
    for name in [&b"one"[..], b"two"] {
        let n = store::set(&blob, name, xattr::RAW, b"value", &mut out).unwrap();
        blob = out[..n].to_vec();
    }
    let whole = store::count(&blob);
    assert_eq!(whole, 2);
    for cut in 0..blob.len() {
        let seen = store::iter(&blob[..cut]).count();
        assert!(
            seen < whole,
            "a cut at {cut} produced a whole second record"
        );
        for (name, _, value) in store::iter(&blob[..cut]) {
            assert!(
                !name.is_empty() && value == b"value",
                "a torn record was read as data"
            );
        }
    }
    // A zero-padded tail (what a partially written block leaves) stops the walk rather than
    // producing empty-named attributes.
    let mut padded = blob.clone();
    padded.extend_from_slice(&[0u8; 64]);
    assert_eq!(store::count(&padded), whole);
}

/// A listing round-trips through the page encoding, and the widest listing this contract permits
/// fits one page **exactly**, which is what lets `LISTXATTR` answer without a cursor. If that
/// stops being true, the verb needs a cursor and the note needs rewriting.
#[test]
fn the_widest_listing_fits_one_page_and_round_trips() {
    use xattr::store;
    assert_eq!(
        xattr::list::MAX_BYTES,
        PAGE,
        "the no-cursor property is this equality"
    );

    let mut blob = Vec::new();
    let mut out = scratch();
    let names: Vec<Vec<u8>> = (0..xattr::MAX_COUNT)
        .map(|i| {
            let mut n = vec![b'a' + i as u8; xattr::MAX_NAME];
            n[0] = b'0' + i as u8;
            n
        })
        .collect();
    for name in &names {
        let n = store::set(&blob, name, xattr::RAW, b"", &mut out).unwrap();
        blob = out[..n].to_vec();
    }
    let mut page = [0u8; PAGE];
    let used = store::list(&blob, &mut page);
    assert_eq!(used, PAGE, "sixteen widest names fill the page and no more");
    let back: Vec<Vec<u8>> = xattr::list::iter(&page[..used])
        .map(|n| n.to_vec())
        .collect();
    assert_eq!(back, names, "the listing did not come back as it went in");

    // A short buffer stops before the record that will not fit rather than splitting a name.
    let mut tiny = [0u8; 300];
    let n = store::list(&blob, &mut tiny);
    assert_eq!(xattr::list::iter(&tiny[..n]).count(), 1);
}

/// The two packed words carry their fields without bleeding, at the extremes. A value length
/// that spilled into the kind would hand a caller somebody else's type; a kind that spilled into
/// the length would hand it somebody else's bytes.
#[test]
fn the_attribute_words_carry_a_kind_and_a_length_without_bleeding() {
    for &(kind, len) in &[
        (xattr::RAW, 0usize),
        (1, xattr::MAX_VALUE),
        (xattr::MAX_KIND, xattr::MAX_VALUE),
        (0xFFFF_FFFF, 1), // a caller that ignores MAX_KIND: the reply masks rather than lies
    ] {
        let w1 = xattr::spec(kind, len as u64);
        assert_eq!(xattr::spec_kind(w1), kind, "kind through the request word");
        assert_eq!(
            xattr::spec_value_len(w1),
            len,
            "length through the request word"
        );
        let r = xattr::reply(kind, len);
        assert!(r >= 0);
        assert_eq!(xattr::reply_kind(r), kind & xattr::MAX_KIND);
        assert_eq!(xattr::reply_value_len(r), len);
    }
}

/// The store's file names are the node ids, and two nodes never share one. That equality is what
/// makes rename free and what makes a leaked blob dangerous, so it is pinned rather than assumed.
#[test]
fn a_store_file_is_named_for_exactly_one_node() {
    use xattr::store::file_name;
    assert_eq!(&file_name(0), b"00000000");
    assert_eq!(&file_name(1), b"00000001");
    assert_eq!(&file_name(0xdead_beef), b"deadbeef");
    assert_eq!(&file_name(u32::MAX), b"ffffffff");
    let mut seen = std::collections::BTreeSet::new();
    for id in [0u32, 1, 2, 16, 255, 4096, 0x0100_0000, u32::MAX] {
        assert!(
            seen.insert(file_name(id)),
            "two node ids share a store file"
        );
    }
}

/// **One record's exact bytes, as the format doc publishes them.** Recovery tooling reads this
/// layout without this crate, so the bytes are the contract, not the code: a one-byte name
/// length, four bytes of kind and two of value length, little-endian, then the name, then the
/// value. The round-trip tests cannot pin this, because a writer and reader that agreed on a
/// wrong header width would pass them together.
#[test]
fn an_attribute_record_lays_out_exactly_as_published() {
    use xattr::store;
    assert_eq!(store::RECORD_HEADER, 7);
    let mut out = scratch();
    let n = store::set(&[], b"ab", 0x0102_0304, b"xyz", &mut out).expect("set");
    assert_eq!(n, store::record_len(2, 3));
    assert_eq!(
        &out[..n],
        &[
            2, 0x04, 0x03, 0x02, 0x01, 3, 0, b'a', b'b', b'x', b'y', b'z'
        ],
    );
}

/// **An exactly-sized buffer is enough, and a short one refuses rather than crashes.** `set`'s
/// doc sizes the caller's buffer at `blob.len() + record_len(...)`, so "exactly enough" has to
/// mean enough; and the FS server hands `set` a buffer it sized itself, so a one-byte error
/// there must come back [`xattr::ENOSPC`] instead of tearing down the serve loop on a slice
/// bound.
#[test]
fn a_record_that_exactly_fits_is_written_and_a_short_buffer_refuses() {
    use xattr::store;
    let n = store::record_len(1, 1);
    let mut exact = vec![0u8; n];
    assert_eq!(store::set(&[], b"n", xattr::RAW, b"v", &mut exact), Ok(n));
    let mut short = vec![0u8; n - 1];
    assert_eq!(
        store::set(&[], b"n", xattr::RAW, b"v", &mut short),
        Err(xattr::ENOSPC),
    );
}

// The tests from here down each pin something the run of milestone 85 (mutation testing over the
// host crates) showed nothing pinned. The equivalences (every `|` over disjoint masked fields,
// where `^` cannot differ) are recorded in notes/mutation-testing.md rather than tested.

/// **A record wider than the contract is refused when a blob is rewritten, not carried forward.**
///
/// [`xattr::store::set`] and [`xattr::store::remove`] re-emit every untouched record with lengths
/// read off the blob, not from a bounds-checked caller. A value length is a `u16`, so a torn or
/// forged blob reaches 65535 where [`xattr::MAX_VALUE`] is 3072, and `write_record`'s own limit is
/// all that stops it. Every other test builds blobs through `set`, whose `E2BIG` fires first.
#[test]
fn a_record_wider_than_the_contract_is_refused_when_a_blob_is_rewritten() {
    use xattr::store::{self, RECORD_HEADER};
    let over = xattr::MAX_VALUE + 1;
    let mut blob = vec![0u8; store::record_len(1, over)];
    blob[0] = 1;
    blob[5..7].copy_from_slice(&(over as u16).to_le_bytes());
    blob[RECORD_HEADER] = b'a';
    // The walker has to hand the oversized record over, or the rewrite path is never reached
    // and the refusals below would be about an empty blob instead.
    assert_eq!(store::iter(&blob).count(), 1);

    let mut out = vec![0u8; blob.len() + store::record_len(1, 1)];
    assert_eq!(
        store::set(&blob, b"b", xattr::RAW, b"v", &mut out),
        Err(xattr::ENOSPC),
    );
    assert_eq!(store::remove(&blob, b"b", &mut out), Err(xattr::ENOSPC));
}
