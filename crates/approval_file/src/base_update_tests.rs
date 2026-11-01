//! **The tests of the base-update grant**: the section's grammar, the version form, and one test
//! per condition the check refuses, each flipping only that condition.
//!
//! Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-7-the-base-update-grant.

use alloc::format;

use super::base_update::{Refusal, Request, Starter, Version};
use super::*;

const ALL_ON: &str = "approval = \"fleet\"\n\n[base-update]\nallow = true\nunattended = true\naccepts = [\"release-1\"]\n";

fn version(text: &str) -> Version {
    Version::parse(text).unwrap()
}

/// A request that passes unattended against [`ALL_ON`]; each test breaks one thing.
fn good() -> Request<'static> {
    Request {
        starter: Starter::Unattended,
        writes_spare_slot_only: true,
        signed_by: Some("release-1"),
        trust_root_keys: &["release-1", "release-2"],
        candidate: version("1.2.4"),
        running: version("1.2.3"),
        slot_on_trial: false,
    }
}

fn refusal(text: &str, request: &Request<'_>) -> Result<(), Refusal> {
    File::parse(text).unwrap().base_update().check(request)
}

/// **A file without the section grants nothing**, and the section's switches default to false.
#[test]
fn no_section_grants_nothing() {
    let file =
        File::parse("approval = \"f\"\n[package.a]\nupdate = false\nremove = false\n").unwrap();
    let grant = file.base_update();
    assert!(!grant.allow() && !grant.unattended() && grant.accepts().is_empty());
    assert_eq!(grant.check(&good()), Err(Refusal::NotAllowed));
    let empty = File::parse("approval = \"f\"\n[base-update]\n").unwrap();
    assert!(!empty.base_update().allow() && !empty.base_update().unattended());
}

/// **The section parses its three keys**, before or after the package sections.
#[test]
fn the_section_parses() {
    let file = File::parse(ALL_ON).unwrap();
    let grant = file.base_update();
    assert!(grant.allow() && grant.unattended());
    assert_eq!(grant.accepts(), ["release-1"]);
    let mixed = "approval = \"f\"\n[package.a]\nupdate = false\nremove = false\n\
                 [base-update]\nallow = true\nunattended = false\naccepts = [\"k1\", \"k2\"]\n\
                 [package.b]\nupdate = false\nremove = false\n";
    let file = File::parse(mixed).unwrap();
    assert!(file.base_update().allow() && !file.base_update().unattended());
    assert_eq!(file.base_update().accepts(), ["k1", "k2"]);
    assert_eq!(file.sections().len(), 2);
    assert!(!file.entry("b").unwrap().update);
}

fn why(text: &str) -> (usize, Why) {
    let e = File::parse(text).unwrap_err();
    (e.at, e.why)
}

/// **The section is held to its grammar**, each refusal naming its line.
#[test]
fn the_section_refuses_what_it_does_not_know() {
    let head = "approval = \"f\"\n";
    assert_eq!(why("[base-update]\n"), (1, Why::NoHeader));
    assert_eq!(
        why(&format!("{head}[base-update]\n[base-update]\n")),
        (3, Why::DuplicateKey)
    );
    assert_eq!(
        why(&format!(
            "{head}[base-update]\nallow = true\nallow = false\n"
        )),
        (4, Why::DuplicateKey)
    );
    assert_eq!(
        why(&format!("{head}[base-update]\nallow = yes\n")),
        (3, Why::BadValue)
    );
    assert_eq!(
        why(&format!("{head}[base-update]\nunattended = 1\n")),
        (3, Why::BadValue)
    );
    assert_eq!(
        why(&format!("{head}[base-update]\nkey = \"k\"\n")),
        (3, Why::UnknownKey("key".into()))
    );
    assert_eq!(
        why(&format!("{head}[base-update]\naccepts = [k]\n")),
        (3, Why::BadValue)
    );
    assert_eq!(
        why(&format!("{head}[base-update]\naccepts = [\"a b\"]\n")),
        (3, Why::BadName)
    );
    assert_eq!(
        why(&format!("{head}[base-update]\naccepts = [\"k\", \"k\"]\n")),
        (3, Why::DuplicateKey)
    );
    // A package key after a package section keeps its own grammar.
    assert_eq!(
        why(&format!(
            "{head}[base-update]\nallow = true\n[package.a]\nallow = true\n"
        )),
        (5, Why::UnknownKey("allow".into()))
    );
}

/// **A base version is three integers**, and nothing else is one.
#[test]
fn a_version_is_three_integers() {
    let v = version("10.0.7");
    assert_eq!((v.major, v.minor, v.patch), (10, 0, 7));
    assert_eq!(format!("{v}"), "10.0.7");
    for bad in [
        "",
        "1",
        "1.2",
        "1.2.3.4",
        "1.2.",
        ".1.2",
        "1..2",
        "1.2.3-rc1",
        "1.2.3+b",
        "a.b.c",
        "+1.2.3",
        "-1.2.3",
        " 1.2.3",
        "1.2.3 ",
        "01.2.3",
        "1.02.3",
        "1.2.03",
        "18446744073709551616.0.0",
    ] {
        assert_eq!(Version::parse(bad), None, "{bad:?}");
    }
    assert!(Version::parse("0.0.0").is_some());
    assert!(Version::parse("18446744073709551615.0.0").is_some());
}

/// **Versions compare left to right as integers**, so 1.10.0 is newer than 1.9.9.
#[test]
fn versions_compare_as_integers() {
    assert!(version("1.10.0") > version("1.9.9"));
    assert!(version("2.0.0") > version("1.99.99"));
    assert!(version("1.2.4") > version("1.2.3"));
    assert!(version("1.3.0") > version("1.2.9"));
    assert!(version("1.2.3") == version("1.2.3"));
}

/// **The full request passes**, started by a person or by nobody.
#[test]
fn the_full_request_passes() {
    assert_eq!(refusal(ALL_ON, &good()), Ok(()));
    let person = Request {
        starter: Starter::Person,
        ..good()
    };
    assert_eq!(refusal(ALL_ON, &person), Ok(()));
}

/// **`allow` is the first switch**, for a person and for nobody.
#[test]
fn allow_is_needed() {
    let off = "approval = \"f\"\n[base-update]\nunattended = true\naccepts = [\"release-1\"]\n";
    assert_eq!(refusal(off, &good()), Err(Refusal::NotAllowed));
    let person = Request {
        starter: Starter::Person,
        ..good()
    };
    assert_eq!(refusal(off, &person), Err(Refusal::NotAllowed));
}

/// **A person's update needs only `allow`, a newer version, and nothing of the unattended bounds.**
#[test]
fn a_person_needs_only_allow_and_a_newer_version() {
    let allow = "approval = \"f\"\n[base-update]\nallow = true\n";
    let person = Request {
        starter: Starter::Person,
        writes_spare_slot_only: false,
        signed_by: None,
        slot_on_trial: true,
        ..good()
    };
    assert_eq!(refusal(allow, &person), Ok(()));
    let same = Request {
        candidate: person.running,
        ..person
    };
    assert_eq!(refusal(allow, &same), Err(Refusal::NotNewer));
}

/// **Unattended needs its own switch.**
#[test]
fn unattended_needs_its_switch() {
    let off = "approval = \"f\"\n[base-update]\nallow = true\naccepts = [\"release-1\"]\n";
    assert_eq!(refusal(off, &good()), Err(Refusal::NotUnattended));
}

/// **An unattended update writes only the spare slot.**
#[test]
fn unattended_writes_only_the_spare_slot() {
    let r = Request {
        writes_spare_slot_only: false,
        ..good()
    };
    assert_eq!(refusal(ALL_ON, &r), Err(Refusal::NotSpareSlotOnly));
}

/// **An unsigned manifest is refused**, and so is a key the file does not accept or the trust
/// root does not hold; the file never adds a key.
#[test]
fn the_signing_key_is_accepted_by_both() {
    let unsigned = Request {
        signed_by: None,
        ..good()
    };
    assert_eq!(refusal(ALL_ON, &unsigned), Err(Refusal::Unsigned));
    // Held by the trust root, not accepted by the owner.
    let other = Request {
        signed_by: Some("release-2"),
        ..good()
    };
    assert_eq!(refusal(ALL_ON, &other), Err(Refusal::KeyNotAccepted));
    // Accepted by the owner, absent from the trust root: the file adds no key.
    let absent = Request {
        trust_root_keys: &["release-2"],
        ..good()
    };
    assert_eq!(refusal(ALL_ON, &absent), Err(Refusal::KeyNotAccepted));
    // No key accepted at all.
    let none = "approval = \"f\"\n[base-update]\nallow = true\nunattended = true\n";
    assert_eq!(refusal(none, &good()), Err(Refusal::KeyNotAccepted));
}

/// **The version must be strictly greater**: equal and older are refused, for the unattended too.
#[test]
fn the_version_must_be_strictly_greater() {
    let equal = Request {
        candidate: version("1.2.3"),
        ..good()
    };
    assert_eq!(refusal(ALL_ON, &equal), Err(Refusal::NotNewer));
    let older = Request {
        candidate: version("1.2.2"),
        ..good()
    };
    assert_eq!(refusal(ALL_ON, &older), Err(Refusal::NotNewer));
}

/// **No slot may be on trial.**
#[test]
fn no_slot_may_be_on_trial() {
    let r = Request {
        slot_on_trial: true,
        ..good()
    };
    assert_eq!(refusal(ALL_ON, &r), Err(Refusal::SlotOnTrial));
}

/// **Each refusal names its condition** in the words a person reads.
#[test]
fn each_refusal_names_its_condition() {
    let all = [
        (Refusal::NotAllowed, "allow"),
        (Refusal::NotUnattended, "unattended"),
        (Refusal::NotSpareSlotOnly, "spare slot"),
        (Refusal::Unsigned, "signed"),
        (Refusal::KeyNotAccepted, "trust root"),
        (Refusal::NotNewer, "strictly greater"),
        (Refusal::SlotOnTrial, "on trial"),
    ];
    for (refusal, word) in all {
        let text = format!("{refusal}");
        assert!(
            text.starts_with("refused: ") && text.contains(word),
            "{text}"
        );
    }
}
