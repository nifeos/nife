// The crate is `no_std`, so its own tests have to say where the allocating types come from. They
// are here only to build fixtures; nothing under test allocates.
extern crate std;
use std::format;
use std::vec::Vec;

/// **The crash fixture's two payloads must be the same length**, and the millisecond test that
/// says so is not pedantry: `WRITE` does not truncate, so the moment B is shorter than A the
/// recovered file is A's tail with B's head on it, and "exactly A or exactly B" becomes a
/// question with no answer. That failure was corrected four times before it was understood, and it cost a
/// day when it was discovered from the far end instead of pinned here.
#[test]
fn the_crash_payloads_replace_each_other_completely() {
    use crate::crash;
    assert_eq!(
        crash::A.len(),
        crash::B.len(),
        "a shorter payload leaves the other's tail behind, so the verifier could see neither",
    );
    assert!(
        crash::INITIAL.len() <= crash::A.len(),
        "the image's initial contents must not outlive the first write either",
    );
    assert_ne!(crash::A, crash::B);
    assert_ne!(crash::A, crash::INITIAL);
}

#[test]
fn the_escape_bits_are_distinct() {
    // The attacker reports a bitmap and the test asserts an expected set, so two outcomes
    // sharing a bit would hide one of them and make a wrong verdict read as a right one.
    use crate::escape::*;
    let bits = [
        SECOND_FILE,
        WROTE,
        TRUNCATED,
        CREATED,
        FORGED_HANDLE,
        GRANTED_READ_FAILED,
        WROTE_ATTR,
        GRANTED_ATTRS_FAILED,
    ];
    let mut seen = 0u64;
    for b in bits {
        assert_ne!(b, 0, "zero is the pass; it cannot also be a breach");
        assert_eq!(seen & b, 0, "two escapes share a bit");
        seen |= b;
    }
}

// --- The directory capability ---

/// The escape bits must not overlap, for the reason the per-file ones must not: the test asserts
/// an expected set, so two outcomes on one bit make a wrong verdict read as a right one.
#[test]
fn the_directory_escape_bits_are_distinct() {
    use crate::dirscape::*;
    let bits = [
        REACHED_PARENT,
        REACHED_SIBLING,
        WALKED_UP,
        WIDENED,
        ENUMERATED,
        DESCENDED,
        CREATED,
        WROTE,
        RENAMED,
        MADE_A_DIR,
        ENUMERATED_A_STRANGER,
        FORGED_HANDLE,
        OPENED_ITS_OWN,
        GRANTED_ACCESS_FAILED,
        READ_ATTRS,
        SET_AN_ATTR,
        REACHED_AN_UNMATCHED_NAME,
    ];
    let mut seen = 0u64;
    for b in bits {
        assert_ne!(b, 0, "zero is the pass; it cannot also be a breach");
        assert_eq!(seen & b, 0, "two escapes share a bit");
        seen |= b;
    }
}

/// **The absolute forms of the two probe names are the relative ones with a slash on the
/// front**, and nothing else. Written down because they are two constants that must agree, and
/// a drift between them would not fail: the witness would simply open nothing and report a
/// clear bit, which reads exactly like the escape it is looking for having been prevented.
#[test]
fn the_absolute_probe_names_are_the_relative_ones_rooted() {
    use crate::tree::*;
    assert_eq!(ABS_INNER, format!("/{INNER}"));
    assert_eq!(ABS_SECRET, format!("/{SECRET}"));
}

/// **The two-grant fixture's bits**, which had no distinctness test of its own at all.
#[test]
fn the_two_grant_bits_are_distinct() {
    use crate::twodir::*;
    // A witness bit of zero turns its probe into one that always passes. Two of these bits are
    // the structural finding that an endpoint is the boundary.
    let bits = [
        OPENED_A,
        OPENED_B,
        REACHED_B_VIA_A,
        REACHED_A_VIA_B,
        DOT_DOT_CROSSED_MOUNTS,
        GRANTED_ACCESS_FAILED,
    ];
    let mut seen = 0u64;
    for b in bits {
        assert_ne!(
            b, 0,
            "zero is the empty report; it cannot also be an outcome"
        );
        assert_eq!(seen & b, 0, "two two-grant outcomes share a bit");
        seen |= b;
    }
}

/// **Every phase tag the bench boot can be handed has its own name**, and a tag it was not
/// handed has none. Nothing in this crate called `throughput::name`, so a mutation run
/// found nine mutants alive in it at once: the whole function as `None`, as `Some("")` and as
/// `Some("xyzzy")`, and each of its six arms deleted. The bench boot prints these beside the
/// numbers, so two phases sharing a name is two rows a reader cannot tell apart.
#[test]
fn every_throughput_phase_tag_has_its_own_name() {
    use crate::throughput::*;
    let named = [
        (SEQ_WRITE, "fs_seq_write"),
        (SEQ_READ, "fs_seq_read"),
        (RAND_READ, "fs_rand_read"),
        (RAND_WRITE, "fs_rand_write"),
        (RECORD_READ, "fs_record_read"),
        (PAYLOAD_FILL, "fs_payload_fill"),
    ];
    assert_eq!(named.len(), PHASES, "one name per phase the boot waits for");
    let mut seen: Vec<&str> = Vec::new();
    for (tag, want) in named {
        assert_eq!(name(tag), Some(want), "tag {tag}");
        assert!(!seen.contains(&want), "two phases print as {want}");
        seen.push(want);
    }
    // A tag this table does not know is `None` rather than somebody else's name.
    assert_eq!(name(u64::MAX), None);
}

/// The navigating shell's bits, for the same reason every witness set here has such a test, and
/// one more that is specific to them: the headline test reads a property off **two** reports, so
/// a bit that meant two things would let one shell's success stand in for the other's failure.
#[test]
fn the_navigation_bits_are_distinct() {
    use crate::navscape::*;
    let bits = [
        PWD_IS_ROOT,
        CLAMPED_AT_ROOT,
        WALKED_UP,
        ABSOLUTE_IS_MY_ROOT,
        REACHED_INNER,
        REACHED_SECRET,
        LISTED,
        SAW_INNER,
        SAW_SECRET,
        DESCENDED,
        RETURNED,
        CREATED,
        MADE_DIR,
        UNLINKED,
        HOLDER_KEPT_READING,
        NAME_GONE_AFTER_UNLINK,
        UNLINK_REFUSED_A_DIRECTORY,
        NAVIGATION_FAILED,
        RMDIR_REFUSED_NON_EMPTY,
        RMDIR_REMOVED_EMPTY,
        ABSOLUTE_REACHED_INNER,
        ABSOLUTE_REACHED_SECRET,
        ABSOLUTE_CLAMPED_AT_ROOT,
        // Every bit `navscape` declares belongs in this list, and nothing checks that a new one is.
        TOUCH_CREATED,
        TOUCH_PRESERVED,
        TOUCH_MTIME_ADVANCED,
        TOUCH_AT_ROUND_TRIPPED,
        TOUCH_NOW_NEEDS_ONLY_WRITE,
        TOUCH_AT_REFUSED_WITHOUT_SETTIME,
        // With nothing covering them, `1 << 29`, `1 << 30` and `1 << 31` could each become
        // `1 >> n`, which is zero, and a witness bit of zero reports nothing while passing.
        BIND_REACHED_TARGET,
        BIND_ASCEND_REACHES_REAL_PARENT,
        BIND_STOPS_AT_TRUE_ROOT,
    ];
    let mut seen = 0u64;
    for b in bits {
        assert_ne!(
            b, 0,
            "zero is the empty report; it cannot also be an outcome"
        );
        assert_eq!(seen & b, 0, "two navigation outcomes share a bit");
        seen |= b;
    }
}

// The tests from here down each pin something a mutation run showed nothing pinned. The
// equivalences (every `|` over disjoint masked fields, where `^` cannot differ) are recorded as
// such rather than tested.

/// Every witness verdict module is distinct single bits. Both ends of each QEMU test build
/// their words from these same constants, so a shifted-to-zero bit is invisible at runtime:
/// the claim it carried silently stops being checked. Distinct powers of two here is the
/// whole defence.
#[test]
fn the_witness_bits_are_distinct_single_bits() {
    fn all_distinct_bits(bits: &[u64]) {
        for (i, &a) in bits.iter().enumerate() {
            assert!(a.is_power_of_two(), "bit {i} is {a:#x}, not a single bit");
            for &b in &bits[i + 1..] {
                assert_ne!(a, b, "two claims share bit {a:#x}");
            }
        }
    }
    all_distinct_bits(&[
        crate::escape::SECOND_FILE,
        crate::escape::WROTE,
        crate::escape::TRUNCATED,
        crate::escape::CREATED,
        crate::escape::FORGED_HANDLE,
        crate::escape::GRANTED_READ_FAILED,
        crate::escape::WROTE_ATTR,
        crate::escape::GRANTED_ATTRS_FAILED,
    ]);
    all_distinct_bits(&[
        crate::dirscape::REACHED_PARENT,
        crate::dirscape::REACHED_SIBLING,
        crate::dirscape::WALKED_UP,
        crate::dirscape::WIDENED,
        crate::dirscape::ENUMERATED,
        crate::dirscape::DESCENDED,
        crate::dirscape::CREATED,
        crate::dirscape::WROTE,
        crate::dirscape::RENAMED,
        crate::dirscape::ENUMERATED_A_STRANGER,
        crate::dirscape::FORGED_HANDLE,
        crate::dirscape::GRANTED_ACCESS_FAILED,
        crate::dirscape::MADE_A_DIR,
        crate::dirscape::OPENED_ITS_OWN,
        crate::dirscape::READ_ATTRS,
        crate::dirscape::SET_AN_ATTR,
        crate::dirscape::REACHED_AN_UNMATCHED_NAME,
    ]);
    all_distinct_bits(&[
        crate::navscape::PWD_IS_ROOT,
        crate::navscape::CLAMPED_AT_ROOT,
        crate::navscape::WALKED_UP,
        crate::navscape::ABSOLUTE_IS_MY_ROOT,
        crate::navscape::REACHED_INNER,
        crate::navscape::REACHED_SECRET,
        crate::navscape::LISTED,
        crate::navscape::SAW_INNER,
        crate::navscape::SAW_SECRET,
        crate::navscape::DESCENDED,
        crate::navscape::RETURNED,
        crate::navscape::CREATED,
        crate::navscape::MADE_DIR,
        crate::navscape::UNLINKED,
        crate::navscape::HOLDER_KEPT_READING,
        crate::navscape::NAME_GONE_AFTER_UNLINK,
        crate::navscape::UNLINK_REFUSED_A_DIRECTORY,
        crate::navscape::NAVIGATION_FAILED,
        crate::navscape::RMDIR_REFUSED_NON_EMPTY,
        crate::navscape::RMDIR_REMOVED_EMPTY,
        crate::navscape::ABSOLUTE_REACHED_INNER,
        crate::navscape::ABSOLUTE_REACHED_SECRET,
        crate::navscape::ABSOLUTE_CLAMPED_AT_ROOT,
        crate::navscape::TOUCH_CREATED,
        crate::navscape::TOUCH_PRESERVED,
        crate::navscape::TOUCH_MTIME_ADVANCED,
        crate::navscape::TOUCH_AT_ROUND_TRIPPED,
        crate::navscape::TOUCH_NOW_NEEDS_ONLY_WRITE,
        crate::navscape::TOUCH_AT_REFUSED_WITHOUT_SETTIME,
    ]);
    all_distinct_bits(&[
        crate::globscape::EXPANDED,
        crate::globscape::AGREED,
        crate::globscape::EXCLUDED_A_STRANGER,
        crate::globscape::NO_MATCH_REFUSED,
        crate::globscape::PATTERN_IN_PATH_REFUSED,
        crate::globscape::TEXT_UNTOUCHED,
        crate::globscape::GLOB_FAILED,
    ]);
    all_distinct_bits(&[
        crate::attrs::SET_AND_READ_BACK,
        crate::attrs::LISTED,
        crate::attrs::SURVIVED_RENAME,
        crate::attrs::GONE_AFTER_REMOVE,
        crate::attrs::GONE_AFTER_UNLINK,
        crate::attrs::OVERSIZE_REFUSED,
        crate::attrs::STORE_UNNAMEABLE,
        crate::attrs::STORE_UNLISTED,
        crate::attrs::ATTRS_FAILED,
    ]);
    assert_eq!(
        crate::attrs::EXPECTED,
        255,
        "the eight required claims, bits 0 through 7"
    );
}

// ---- survivor triage of the 2026-10-03 census: the shared fixture's arithmetic ---------------

/// A sized file's bytes are printable letters cycling through the alphabet, with a newline in
/// every 64th place. The host check and the guest agree on them without a file, so the pattern
/// is an interface.
#[test]
fn a_sized_file_cycles_the_alphabet_and_breaks_every_sixty_four() {
    use crate::walk::sized_byte;
    assert_eq!(sized_byte(0), b'a');
    assert_eq!(sized_byte(25), b'z');
    assert_eq!(sized_byte(26), b'a');
    assert_eq!(sized_byte(62), b'k');
    assert_eq!(sized_byte(63), b'\n');
    assert_eq!(sized_byte(64), b'm');
    assert_eq!(sized_byte(127), b'\n');
}

/// The wide directory's names are `n` and three decimal digits, so 128 of them sort and list in
/// numeric order and none collides.
#[test]
fn the_wide_directory_names_are_n_and_three_digits() {
    use crate::walk::wide_name;
    assert_eq!(&wide_name(0), b"n000");
    assert_eq!(&wide_name(7), b"n007");
    assert_eq!(&wide_name(45), b"n045");
    assert_eq!(&wide_name(123), b"n123");
    assert_eq!(&wide_name(987), b"n987");
}

/// **The match list's three regions meet where the shape says**, spelled out by hand: the chain's
/// top and bottom, the wide directory's first and last, and the narrow file. The kernel harness and
/// the host's `rg` both compare against [`crate::walk::rg_match_path`], so a wrong path here
/// would be agreed on by both sides and fail neither.
#[test]
fn the_rg_match_paths_meet_at_the_region_boundaries() {
    use crate::walk::*;
    let path = |i| {
        let mut buf = [0u8; 32];
        let len = rg_match_path(i, &mut buf);
        std::string::String::from_utf8(buf[..len].to_vec()).unwrap()
    };
    assert_eq!(path(0), "chain/f");
    assert_eq!(path(1), "chain/c/f");
    assert_eq!(path(8), "chain/c/c/c/c/c/c/c/c/f");
    assert_eq!(path(9), "wide/n000");
    assert_eq!(path(10), "wide/n001");
    assert_eq!(path(136), "wide/n127");
    assert_eq!(path(137), "narrow/n000");
}

/// What a full walk of the priced tree must find, derived by hand from the shape: four region
/// directories and eight chain levels below the first make 12 directories; nine chain files, 128
/// wide, one narrow and three sized make 141 files. The programs assert their counts against
/// these, so a wrong constant here makes the walk fail on the wrong thing.
#[test]
fn the_walk_totals_are_what_the_shape_adds_up_to() {
    use crate::walk::*;
    assert_eq!(WALK_DIRECTORIES, 12);
    assert_eq!(WALK_FILES, 141);
    assert_eq!(WALK_ENTRIES, 153);
    // 138 small files of 16 bytes, plus 4 KiB, 64 KiB and 256 KiB.
    assert_eq!(WALK_BYTES, 138 * 16 + 4096 + 65_536 + 262_144);
    // `rg` finds its pattern once in each of those 138 small files and never in the alphabet.
    assert_eq!(RG_MATCHES, 138);
    // Four regions at depth 1; chain levels 0..=7 hold two entries at depth i + 2 (2 * (2 + ... + 9)
    // = 88) and the last holds one at depth 10; 132 files at depth 2 in the other regions.
    assert_eq!(WALK_COMPONENTS, 4 + 88 + 10 + 2 * 132);
}

/// The two-trees report is a bitmap the kernel test compares as an exact set, so every flag has
/// to be its own single bit. A flag shifted to nothing would be absent from every run and the
/// set could still match.
#[test]
fn every_two_trees_flag_is_its_own_single_bit() {
    use crate::twotrees::*;
    let flags = [
        PWD_STARTS_AT_ROOT,
        LISTED_ROOT,
        MOVED_INTO_MOUNT,
        LISTED_MOUNT,
        OPENED_RELATIVE_IN_MOUNT,
        OPENED_ROOT_FROM_MOUNT,
        UP_FROM_MOUNT_IS_PARENT,
        ROOT_CLAMPED,
        REDIRECTED_FROM_MOUNT,
        HOME_IS_ROOT,
        BOUND_INTO_MOUNT,
        RM_IN_MOUNT_REFUSED,
        MOVED_ON_REFUSAL,
        CLIMBED_OUT,
        REACHED_ACROSS,
        TWO_TREES_FAILED,
    ];
    let mut seen = 0u64;
    for f in flags {
        assert_eq!(f.count_ones(), 1, "{f:#x} is not one bit");
        assert_eq!(seen & f, 0, "{f:#x} is used twice");
        seen |= f;
    }
}

// --- The directory capability: the fixture against the contract it rides on ---

/// The fixture's names must all fit a grant's two argument words, and must be distinct: a
/// sibling that happened to be spelled like the granted directory would make the confinement
/// test pass for the wrong reason.
#[test]
fn the_subtree_fixture_is_grantable_and_unambiguous() {
    use crate::tree::*;
    use filesystem_contract::grant;
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
/// stream with the byte-sink contract's `eof()` and its status and count ride in the two words that message
/// leaves free; the same property is pinned over there by `eof_is_not_a_byte_count`. What is
/// checked here is the word the attackers and the shell witnesses still use.
#[test]
fn a_text_frame_and_a_verdict_are_told_apart_by_the_first_word() {
    // A frame's first word is a byte count into the two payload words, so 16 is its ceiling.
    // Checked at COMPILE time rather than here: both sides are constants, so a change that broke
    // it should fail the build rather than wait for someone to run the suite. Same discipline as
    // the clock service's step-bound assertion.
    const _: () = assert!(
        crate::VERDICT > 16,
        "a verdict word that could be a byte count would make a diagnostic read as a verdict",
    );
    // And the caretakers' two answers to a descent are told apart the same way, which is what
    // lets init read one word and know whether it holds a capability or a refusal.
    const _: () = assert!(
        filesystem_contract::READY != filesystem_contract::DESCENT_REFUSED,
        "a caretaker's two possible answers must not be the same word",
    );
    // And a non-zero status is the errno, so the two ends agree about what failed rather than
    // only that something did.
    assert_eq!(filesystem_contract::rm::status(filesystem_contract::dir::EROFS), filesystem_contract::dir::EROFS as u64);
    assert_ne!(filesystem_contract::rm::status(filesystem_contract::dir::EROFS), filesystem_contract::rm::OK);
    assert_eq!(filesystem_contract::rm::OK, 0, "success is zero, as every shell expects");
}

// --- Extended attributes ---

/// The attribute witness's bits must not overlap, for the reason every other witness's must not:
/// the kernel test asserts an exact set, so two outcomes on one bit make a wrong verdict read as
/// a right one. `EXPECTED` is pinned against them here so the two ISA legs cannot assert
/// something the bits no longer spell.
#[test]
fn the_attribute_witness_bits_are_distinct_and_its_expectation_matches_them() {
    use crate::attrs::*;
    use filesystem_contract::{grant, xattr};
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
    assert!(VALUE.len() <= xattr::MAXIMUM_VALUE);
    // Checked at COMPILE time: both sides are constants, so a fixture that fell back to the
    // default kind should fail the build rather than wait for the suite. The same discipline
    // `a_text_frame_and_a_verdict_are_told_apart_by_the_first_word` uses.
    const _: () = assert!(
        crate::attrs::KIND != xattr::RAW,
        "a default kind would hide a layer that dropped it",
    );
}
