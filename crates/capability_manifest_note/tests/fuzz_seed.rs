//! The committed fuzz seeds are the descriptors this crate encodes, and each decodes.
//!
//! `fuzz/fuzz_targets/capability_manifest_note_decode` starts from `fuzz/seeds/
//! capability_manifest_note_decode/`. A seed that stops decoding leaves the fuzzer starting from
//! refusals while it still reports no crashes, and Kani stops at 57 bytes, so the seeds are what
//! put the fuzzer inside the version 2 and 3 zone walk. They are written by the encoders here, not
//! by hand: this test fails when a committed file differs from what the crate encodes now.
//!
//! Regenerate them, after a deliberate change to the layout, with
//! `cargo test -p capability_manifest_note --test fuzz_seed -- --ignored write_the_seeds`.

use std::path::PathBuf;

use capability_manifest_note::{
    Scope, SubtreeGrantsNote, decode, decode_subtree_grants, decode_verbs, encode, encode_serviced,
    encode_verbs, encode_zoned, serviced_len, service, verbs_len, zoned_len, zones,
};
use grant_plan::{
    ManifestAuthorities, NO_NOTE_MANIFEST, Program, UNVOUCHED_STD_MANIFEST, VerbGrant,
};

/// The longest zone a descriptor may hold: 63 + 1 + 63 + 1 + 63 + 1 + 61 = 253 bytes.
const LONGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.\
    bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.\
    ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc.\
    ddddddddddddddddddddddddddddddddddddddddddddddddddddd";

const EIGHT: &[&str] = &[
    "", "a", "ab", "example.com", "nifeos.org", "org", "x.org", "zz.example.com",
];
const ONE: &[&str] = &["basalt.nifeos.org"];
const TWO: &[&str] = &["", "basalt.nifeos.org"];
const LONG: &[&str] = &[LONGEST];

const VERBS: &[VerbGrant] = &[
    VerbGrant {
        verb: "install",
        authorities: ManifestAuthorities::ALL,
    },
    VerbGrant {
        verb: "list",
        authorities: ManifestAuthorities::from_bits(1).unwrap(),
    },
    VerbGrant {
        verb: "list -u",
        authorities: ManifestAuthorities::from_bits(3).unwrap(),
    },
];
const ONE_VERB: &[VerbGrant] = &[VerbGrant {
    verb: "0123456789abcdef",
    authorities: ManifestAuthorities::from_bits(0).unwrap(),
}];

/// Every seed: its file name and its bytes.
fn seeds() -> Vec<(&'static str, Vec<u8>)> {
    let m = Program::Uptime.manifest();
    vec![
        ("manifest_v1_uptime", encode(&m).to_vec()),
        ("manifest_v1_unvouched_std", encode(&UNVOUCHED_STD_MANIFEST).to_vec()),
        (
            "manifest_v2_one_zone",
            encode_zoned::<{ zoned_len(ONE) }>(&m, ONE).to_vec(),
        ),
        (
            "manifest_v2_root_and_one",
            encode_zoned::<{ zoned_len(TWO) }>(&NO_NOTE_MANIFEST, TWO).to_vec(),
        ),
        (
            "manifest_v2_eight_zones",
            encode_zoned::<{ zoned_len(EIGHT) }>(&UNVOUCHED_STD_MANIFEST, EIGHT).to_vec(),
        ),
        (
            "manifest_v2_longest_zone",
            encode_zoned::<{ zoned_len(LONG) }>(&m, LONG).to_vec(),
        ),
        (
            "manifest_v3_no_zones_service",
            encode_serviced::<{ serviced_len(&[]) }>(&m, &[], true).to_vec(),
        ),
        (
            "manifest_v3_no_zones_plain",
            encode_serviced::<{ serviced_len(&[]) }>(&m, &[], false).to_vec(),
        ),
        (
            "manifest_v3_zones_service",
            encode_serviced::<{ serviced_len(TWO) }>(&UNVOUCHED_STD_MANIFEST, TWO, true).to_vec(),
        ),
        (
            "manifest_v3_eight_zones",
            encode_serviced::<{ serviced_len(EIGHT) }>(&m, EIGHT, false).to_vec(),
        ),
        (
            "subtree_grants",
            subtree_descriptor(),
        ),
        (
            "verbs_three",
            encode_verbs::<{ verbs_len(3) }>(VERBS).to_vec(),
        ),
        (
            "verbs_one_longest_word",
            encode_verbs::<{ verbs_len(1) }>(ONE_VERB).to_vec(),
        ),
        ("verbs_none", encode_verbs::<{ verbs_len(0) }>(&[]).to_vec()),
    ]
}

fn subtree_descriptor() -> Vec<u8> {
    let note = SubtreeGrantsNote::of(Scope::SubtreeScope);
    note.0[note.0.len() - capability_manifest_note::SUBTREE_GRANTS_LEN..].to_vec()
}

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/seeds/capability_manifest_note_decode")
}

#[test]
fn the_committed_seeds_are_what_the_crate_encodes_and_each_decodes() {
    let want = seeds();
    let mut files: Vec<String> = std::fs::read_dir(directory())
        .expect("fuzz/seeds/capability_manifest_note_decode/ must exist")
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    files.sort();
    let mut names: Vec<String> = want.iter().map(|(n, _)| format!("{n}.bin")).collect();
    names.sort();
    assert_eq!(files, names, "the directory holds exactly the seeds listed here");
    for (name, bytes) in want {
        let on_disk = std::fs::read(directory().join(format!("{name}.bin"))).unwrap();
        assert_eq!(on_disk, bytes, "{name}.bin is stale: regenerate the seeds");
        let accepted = decode(&bytes).is_ok()
            || decode_subtree_grants(&bytes).is_ok()
            || decode_verbs(&bytes).is_ok();
        assert!(accepted, "{name}.bin must decode, or the fuzzer starts from nothing");
    }
}

#[test]
fn the_seeds_reach_the_zone_walk_past_the_count_byte() {
    let zoned = seeds()
        .into_iter()
        .filter(|(n, _)| n.starts_with("manifest_v2") || n.starts_with("manifest_v3"))
        .filter(|(_, b)| b.len() > 57)
        .count();
    assert!(zoned >= 6, "Kani stops at 57 bytes; the seeds must go past it");
    for (name, bytes) in seeds() {
        if name.starts_with("manifest_v3") {
            decode(&bytes).unwrap();
            assert_eq!(service(&bytes), Ok(bytes[bytes.len() - 1] == 1));
        }
        if name == "manifest_v2_eight_zones" {
            assert_eq!(zones(&bytes).unwrap().iter().count(), 8);
        }
    }
}

#[test]
#[ignore = "writes the committed seeds; run it by name after a deliberate layout change"]
fn write_the_seeds() {
    std::fs::create_dir_all(directory()).unwrap();
    for (name, bytes) in seeds() {
        std::fs::write(directory().join(format!("{name}.bin")), bytes).unwrap();
    }
}
