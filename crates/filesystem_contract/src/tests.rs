use super::*;

#[test]
fn every_opcode_is_distinct_and_fits_its_field() {
    // Two verbs sharing a number is a wire bug the packing tests cannot see: each would pack and
    // unpack perfectly and mean the wrong thing. Cheap to assert, so assert it.
    let operations = [
        ("OPEN", fs::OPEN),
        ("READ", fs::READ),
        ("WRITE", fs::WRITE),
        ("CLOSE", fs::CLOSE),
        ("FSTAT", fs::FSTAT),
        ("CREATE", fs::CREATE),
        ("TRUNCATE", fs::TRUNCATE),
    ];
    for (i, (na, a)) in operations.iter().enumerate() {
        assert!(*a <= 0xff, "{na} does not fit the 8-bit opcode field");
        assert_ne!(
            *a, 0,
            "0 is not a verb: a zeroed word must not decode as one"
        );
        for (nb, b) in &operations[i + 1..] {
            assert_ne!(a, b, "{na} and {nb} share an opcode");
        }
    }
    // The blk protocol is a separate namespace on a separate endpoint, so its numbers may and do
    // overlap. Asserted so nobody "fixes" the overlap and renumbers a live wire.
    assert_eq!(
        blk::READ,
        fs::OPEN,
        "the two protocols are deliberately independent"
    );
}

#[test]
fn errors_are_negated_errnos_and_invert() {
    // The boundary convention: -errno on the wire, invertible, and successes read as no error.
    assert_eq!(reply_err(2), -2); // ENOENT
    assert_eq!(reply_err(5), -5); // EIO
    assert_eq!(reply_errno(reply_err(9)), Some(9)); // EBADF round trips
    assert_eq!(reply_errno(0), None);
    assert_eq!(reply_errno(4096), None); // a byte count is a success, not an error
}

// --- The directory capability (milestone 47 (navigation and naming)) ---

/// The fixture's names must all fit a grant's two argument words, and must be distinct: a
/// sibling that happened to be spelled like the granted directory would make the confinement
/// test pass for the wrong reason.
#[test]
fn the_subtree_fixture_is_grantable_and_unambiguous() {
    use filesystem_fixture::tree::*;
    for name in [
        SUB, INNER, DEEPER, LEAF, OTHER, SECRET, MADE, MADE_DIR, MOVED, NAV_KEPT, NAV_GONE,
        NAV_DIR, NAV_EMPTY, NAV_INSIDE,
        // The `rm -r` subtree. Its names ride in a grant too: the program is *started* with the
        // one name it is to remove, packed the way every other grant is.
        RMTREE, RM_KEEP, RM_DOOMED, RM_ONE, RM_TWO, RM_NESTED, RM_LEAF, RM_SOLO, RM_MISSING,
    ] {
        assert!(
            grant::fits(name.as_bytes()),
            "{name} cannot ride in a grant"
        );
    }
    // The post-run host check identifies the attacker's creations by PREFIX, because a run
    // index is appended to each. A fixture name that started with one of those prefixes would
    // be read as an escape, or would hide one.
    let probes = [
        MADE, MADE_DIR, MOVED, NAV_KEPT, NAV_GONE, NAV_DIR, NAV_EMPTY,
    ];
    for fixture in ROOT_ENTRIES
        .iter()
        .chain(&[INNER, DEEPER, LEAF, SECRET, RM_KEEP, RM_DOOMED])
    {
        for probe in probes {
            assert!(
                !fixture.starts_with(probe),
                "{fixture} collides with the prefix `{probe}` the confinement check searches for",
            );
        }
    }
    // And the three probes must not shadow each other, because the check counts them apart: a
    // rename that looked like a creation would make "REMOVE was enforced" unfalsifiable.
    for (i, a) in probes.iter().enumerate() {
        for b in &probes[i + 1..] {
            assert!(
                !a.starts_with(b) && !b.starts_with(a),
                "`{a}` and `{b}` cannot be told apart by prefix",
            );
        }
    }
    assert_ne!(SUB, OTHER, "the sibling must be a different directory");
    // The `rm -r` fixture's own shape. `RM_MISSING` is the load-bearing one: the whole of `-f`
    // is that a name which is not there is not an error, so a fixture that accidentally shipped
    // it would make the `-f` run and the plain run indistinguishable.
    for staged in [
        RM_KEEP, RM_DOOMED, RM_ONE, RM_TWO, RM_NESTED, RM_LEAF, RM_SOLO,
    ] {
        assert_ne!(
            staged, RM_MISSING,
            "the name `rm -f` is pointed at must not be one the fixture stages",
        );
    }
    assert_ne!(RMTREE, SUB, "the rm grant must not be the shells' subtree");
}

/// **A verdict word cannot be mistaken for a line of text**, which every witness in this fixture
/// rests on: a receiver reads "the first message is the verdict" as "this run printed nothing".
///
/// **`rm` no longer rides this word** (2026-08-17). It declares the sink contract, so it ends its
/// stream with `byte_sink_contract::eof()` and its status and count ride in the two words that message
/// leaves free; the same property is pinned over there by `eof_is_not_a_byte_count`. What is
/// checked here is the word the attackers and the shell witnesses still use.
#[test]
fn a_text_frame_and_a_verdict_are_told_apart_by_the_first_word() {
    // A frame's first word is a byte count into the two payload words, so 16 is its ceiling.
    // Checked at COMPILE time rather than here: both sides are constants, so a change that broke
    // it should fail the build rather than wait for someone to run the suite. Same discipline as
    // the clock service's step-bound assertion (§43).
    const _: () = assert!(
        filesystem_fixture::VERDICT > 16,
        "a verdict word that could be a byte count would make a diagnostic read as a verdict",
    );
    // And the caretakers' two answers to a descent are told apart the same way, which is what
    // lets init read one word and know whether it holds a capability or a refusal.
    const _: () = assert!(
        crate::READY != crate::DESCENT_REFUSED,
        "a caretaker's two possible answers must not be the same word",
    );
    // And a non-zero status is the errno, so the two ends agree about what failed rather than
    // only that something did.
    assert_eq!(crate::rm::status(dir::EROFS), dir::EROFS as u64);
    assert_ne!(crate::rm::status(dir::EROFS), crate::rm::OK);
    assert_eq!(crate::rm::OK, 0, "success is zero, as every shell expects");
}

/// **The set the globbing lane grants is exactly what the pattern matches**, checked against the
/// matcher rather than asserted twice.
///
/// The kernel test states its set literally (a `[(&[u8], bool); 2]` it hands to
/// [`nameset::encode`]), which is the only way to build a grant without an enumeration. That
/// literal is only trustworthy if it *is* the expansion, so this pins it: the pattern matches the
/// two names in the set and neither of the two controls, over the fixture as staged.
#[test]
fn the_glob_fixture_is_matched_by_the_pattern_it_is_staged_for() {
    use filesystem_fixture::tree;
    let p = tree::GLOB_PATTERN;
    for name in [tree::GLOB_ONE, tree::GLOB_TWO] {
        assert!(
            glob::matches(p, name.as_bytes()),
            "{name} is in the granted set and the pattern does not match it",
        );
    }
    for name in [tree::GLOB_MISS, tree::GLOB_DIR, tree::GLOB_INNER] {
        assert!(
            !glob::matches(p, name.as_bytes()),
            "{name} is a control and the pattern matches it, so it proves nothing",
        );
    }
    // Every name in the set has to be able to travel in one, or the grant cannot be built.
    for name in [
        tree::GLOB_ONE,
        tree::GLOB_TWO,
        tree::GLOB_MISS,
        tree::GLOB_DIR,
    ] {
        assert!(grant::fits(name.as_bytes()), "{name} does not fit a grant");
    }
}

/// **The batching fixture is over the bound, and the two batches printed in the gate are the
/// ones the rule produces** (milestone 109).
///
/// `script/swish-check` asserts two literal strings at a real prompt, and a literal is only
/// worth asserting if it is the answer rather than a second opinion about it. So this pins all
/// three claims: the pattern matches every name and not the control, there are more names than
/// one grant can carry, and the batch boundary falls where [`filesystem_fixture::tree::MANY_FIRST_BATCH`]
/// says it does.
#[test]
fn the_batching_fixture_is_over_the_bound_and_splits_where_the_gate_says() {
    use filesystem_fixture::tree;
    let p = tree::MANY_PATTERN;
    for name in tree::MANY_NAMES {
        assert!(
            glob::matches(p, name.as_bytes()),
            "{name} is staged for the sweep and the pattern does not match it",
        );
        assert!(grant::fits(name.as_bytes()), "{name} does not fit a grant");
    }
    assert!(
        !glob::matches(p, tree::MANY_MISS.as_bytes()),
        "the control matches, so 'the batches are the match' proves nothing",
    );
    assert!(
        tree::MANY_NAMES.len() > nameset::MAX_NAMES,
        "the fixture fits in one grant, so nothing here is batched",
    );
    // The names are staged in sorted order, which is what lets the two batches below be read off
    // the array rather than sorted at the assertion.
    assert!(tree::MANY_NAMES.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(
        tree::MANY_FIRST_BATCH,
        tree::MANY_NAMES[..nameset::MAX_NAMES].join(" "),
    );
    assert_eq!(
        tree::MANY_SECOND_BATCH,
        tree::MANY_NAMES[nameset::MAX_NAMES..].join(" "),
    );
}

// --- Extended attributes (milestone 57 (extended attributes)) ---

/// The attribute witness's bits must not overlap, for the reason every other witness's must not:
/// the kernel test asserts an exact set, so two outcomes on one bit make a wrong verdict read as
/// a right one. `EXPECTED` is pinned against them here so the two ISA legs cannot assert
/// something the bits no longer spell.
#[test]
fn the_attribute_witness_bits_are_distinct_and_its_expectation_matches_them() {
    use filesystem_fixture::attrs::*;
    let bits = [
        SET_AND_READ_BACK,
        LISTED,
        SURVIVED_RENAME,
        GONE_AFTER_REMOVE,
        GONE_AFTER_UNLINK,
        OVERSIZE_REFUSED,
        STORE_UNNAMEABLE,
        STORE_UNLISTED,
        ATTRS_FAILED,
    ];
    let mut seen = 0u64;
    for b in bits {
        assert_ne!(
            b, 0,
            "zero is the empty report; it cannot also be an outcome"
        );
        assert_eq!(seen & b, 0, "two attribute outcomes share a bit");
        seen |= b;
    }
    assert_eq!(
        EXPECTED,
        seen & !ATTRS_FAILED,
        "a correct run reports every outcome and no failure",
    );
    // The witness's fixture has to be usable: two distinct names it can create and rename
    // between, and a name and value the store will actually accept.
    assert_ne!(PROBE, PROBE_MOVED);
    assert!(grant::fits(PROBE.as_bytes()) && grant::fits(PROBE_MOVED.as_bytes()));
    assert!(xattr::is_valid_name(NAME));
    assert!(VALUE.len() <= xattr::MAX_VALUE);
    // Checked at COMPILE time: both sides are constants, so a fixture that fell back to the
    // default kind should fail the build rather than wait for the suite. The same discipline
    // `a_text_frame_and_a_verdict_are_told_apart_by_the_first_word` uses.
    const _: () = assert!(
        filesystem_fixture::attrs::KIND != xattr::RAW,
        "a default kind would hide a layer that dropped it",
    );
}
