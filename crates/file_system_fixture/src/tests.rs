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

// --- The directory capability ---

/// **The absolute forms of the two probe names are the relative ones with a slash on the
/// front**, and nothing else. Written down because they are two constants that must agree, and
/// a drift between them would not fail: the witness would simply open nothing and report a
/// clear bit, which reads exactly like the escape it is looking for having been prevented.
#[test]
fn the_absolute_probe_names_are_the_relative_ones_rooted() {
    use crate::tree::*;
    assert_eq!(ABSOLUTE_INNER, format!("/{INNER}"));
    assert_eq!(ABSOLUTE_SECRET, format!("/{SECRET}"));
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
        (SEQUENTIAL_WRITE, "fs_seq_write"),
        (SEQUENTIAL_READ, "fs_seq_read"),
        (RANDOM_READ, "fs_rand_read"),
        (RANDOM_WRITE, "fs_rand_write"),
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

// The tests from here down each pin something a mutation run showed nothing pinned. The
// equivalences (every `|` over disjoint masked fields, where `^` cannot differ) are recorded as
// such rather than tested.

/// Assert that each value is a single bit and that no two values share one.
fn all_distinct_bits(bits: &[u64]) {
    for (i, &a) in bits.iter().enumerate() {
        assert!(a.is_power_of_two(), "bit {i} is {a:#x}, not a single bit");
        for &b in &bits[i + 1..] {
            assert_ne!(a, b, "two claims share bit {a:#x}");
        }
    }
}

/// **Every witness's verdict bits are distinct single bits.** A witness reports a bitmap and its
/// test asserts an exact set, so two outcomes on one bit would make a wrong verdict read as a right
/// one, and a bit shifted to zero reports nothing while passing. Both ends of each test build their
/// words from these constants, so a fault here is invisible at run time and this test is the
/// whole defense. Each module's `ALL` is the one list of its bits.
#[test]
fn every_witness_bit_is_a_distinct_single_bit() {
    let witnesses: [(&str, &[u64]); 7] = [
        ("escape", &crate::escape::ALL),
        ("directory_escape", &crate::directory_escape::ALL),
        ("endpoint_boundary", &crate::endpoint_boundary::ALL),
        ("mounted_tree", &crate::mounted_tree::ALL),
        ("navigation", &crate::navigation::ALL),
        ("globbing", &crate::globbing::ALL),
        ("attributes", &crate::attributes::ALL),
    ];
    for (name, bits) in witnesses {
        assert!(!bits.is_empty(), "{name} lists no bits");
        all_distinct_bits(bits);
    }
}

/// **Each witness's `ALL` names every bit constant its module declares, in declaration order.**
/// The arrays are written by hand, so this reads the crate's own source and holds each list to the
/// `pub const NAME: u64 = 1 << n;` lines above it. A bit added to a module and not to its `ALL`
/// fails here rather than escaping the distinctness test.
#[test]
fn every_witness_all_lists_each_bit_its_module_declares() {
    let source = include_str!("lib.rs");
    for module in ["escape", "directory_escape", "endpoint_boundary", "mounted_tree", "navigation",
                   "globbing", "attributes"] {
        let start = source.find(&format!("\npub mod {module} {{\n")).expect("module declared");
        let body = &source[start..start + source[start..].find("\n}\n").expect("module closed")];
        let (declarations, listed) = body.split_once("    pub const ALL: [u64; ").expect("ALL");
        let declared: Vec<&str> = declarations
            .lines()
            .filter_map(|line| line.strip_prefix("    pub const "))
            .filter_map(|rest| rest.split_once(": u64 = 1"))
            .filter(|(_, value)| *value == ";" || value.starts_with(" << "))
            .map(|(name, _)| name)
            .collect();
        let listed: Vec<&str> = listed
            .lines()
            .skip(1)
            .take_while(|line| *line != "    ];")
            .map(|line| line.trim().trim_end_matches(','))
            .collect();
        assert_eq!(listed, declared, "{module}::ALL does not list exactly its declared bits");
    }
}

// ---- the shared fixture's arithmetic ----------------------------------------------------------

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
    assert_eq!(&wide_name(127), b"n127");
}

/// A name past the wide directory would wrap onto one already used, so it is refused rather than
/// returned.
#[test]
#[should_panic]
fn a_wide_name_past_the_last_file_is_refused() {
    let _ = crate::walk::wide_name(crate::walk::WIDE_FILES);
}

/// The match list ends at `RG_MATCHES`; one past it would repeat the narrow file.
#[test]
#[should_panic]
fn a_match_path_past_the_last_match_is_refused() {
    let mut buffer = [0u8; crate::walk::MATCH_PATH_BYTES];
    let _ = crate::walk::rg_match_path(crate::walk::RG_MATCHES, &mut buffer);
}

/// **The match list's three regions meet where the shape says**, spelled out by hand: the chain's
/// top and bottom, the wide directory's first and last, and the narrow file. The kernel harness and
/// the host's `rg` both compare against [`crate::walk::rg_match_path`], so a wrong path here
/// would be agreed on by both sides and fail neither.
#[test]
fn the_rg_match_paths_meet_at_the_region_boundaries() {
    use crate::walk::*;
    let path = |i| {
        let mut buffer = [0u8; 32];
        let len = rg_match_path(i, &mut buffer);
        std::string::String::from_utf8(buffer[..len].to_vec()).unwrap()
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

// --- The directory capability: the fixture against the contract it rides on ---

/// The fixture's names must all fit a grant's two argument words, and must be distinct: a
/// sibling that happened to be spelled like the granted directory would make the confinement
/// test pass for the wrong reason.
#[test]
fn the_subtree_fixture_is_grantable_and_unambiguous() {
    use file_system_contract::grant;

    use crate::tree::*;
    for name in [
        SUB,
        INNER,
        DEEPER,
        LEAF,
        OTHER,
        SECRET,
        MADE,
        MADE_DIRECTORY,
        MOVED,
        NAVIGATION_KEPT,
        NAVIGATION_GONE,
        NAVIGATION_DIRECTORY,
        NAVIGATION_EMPTY,
        NAVIGATION_INSIDE,
        // The `rm -r` subtree. Its names ride in a grant too: the program is *started* with the
        // one name it is to remove, packed the way every other grant is.
        RMTREE,
        RM_KEEP,
        RM_DOOMED,
        RM_ONE,
        RM_TWO,
        RM_NESTED,
        RM_LEAF,
        RM_SOLO,
        RM_MISSING,
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
        MADE,
        MADE_DIRECTORY,
        MOVED,
        NAVIGATION_KEPT,
        NAVIGATION_GONE,
        NAVIGATION_DIRECTORY,
        NAVIGATION_EMPTY,
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
/// `rm` does not use this word. It declares the sink contract, so it ends its report with the
/// byte-sink contract's `(end_of_stream(), 0, 0)`, pinned over there by `end_of_stream_is_not_a_byte_count`, and sends
/// its status and count after that on an endpoint of its own (`file_system_contract::rm`). What is
/// checked here is the word the attackers and the shell witnesses use.
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
        file_system_contract::READY != file_system_contract::DESCENT_REFUSED,
        "a caretaker's two possible answers must not be the same word",
    );
    // And a non-zero status is the errno, so the two ends agree about what failed rather than
    // only that something did.
    assert_eq!(
        file_system_contract::rm::status(file_system_contract::directory::EROFS),
        file_system_contract::directory::EROFS as u64
    );
    assert_ne!(
        file_system_contract::rm::status(file_system_contract::directory::EROFS),
        file_system_contract::rm::OK
    );
    assert_eq!(
        file_system_contract::rm::OK,
        0,
        "success is zero, as every shell expects"
    );
}

// --- Extended attributes ---

/// The attribute witness's `EXPECTED` is every outcome bit and no failure bit, pinned here so the
/// two instruction-set legs cannot assert something the bits no longer spell.
/// `every_witness_bit_is_a_distinct_single_bit` checks the bits themselves.
#[test]
fn the_attribute_witness_expectation_matches_its_bits() {
    use file_system_contract::{extended_attribute, grant};

    use crate::attributes::*;
    let seen = ALL.iter().fold(0, |word, bit| word | bit);
    assert_eq!(
        EXPECTED,
        seen & !ATTRIBUTES_FAILED,
        "a correct run reports every outcome and no failure",
    );
    assert_eq!(EXPECTED, 0xFF, "the eight required claims are bits 0 through 7");
    // The witness's fixture has to be usable: two distinct names it can create and rename
    // between, and a name and value the store will actually accept.
    assert_ne!(PROBE, PROBE_MOVED);
    assert!(grant::fits(PROBE.as_bytes()) && grant::fits(PROBE_MOVED.as_bytes()));
    assert!(extended_attribute::is_valid_name(NAME));
    assert!(VALUE.len() <= extended_attribute::MAXIMUM_VALUE_BYTES);
    // Checked at COMPILE time: both sides are constants, so a fixture that fell back to the
    // default kind should fail the build rather than wait for the suite. The same discipline
    // `a_text_frame_and_a_verdict_are_told_apart_by_the_first_word` uses.
    const _: () = assert!(
        crate::attributes::KIND != extended_attribute::RAW,
        "a default kind would hide a layer that dropped it",
    );
}
