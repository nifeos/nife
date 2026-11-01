//! **The tests of the file grammar and its compile**: the example file, the refusals that name
//! their lines, the check basalt's composition calls, the compiled records, and the base-update grant.
//!
//! Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.

use alloc::{format, vec};

use installed_programs::{Digest, Granted};

use super::*;
use crate::base_update::{Refusal, Request, Starter, Version};

/// The file the block's appendix shows, with the keys its keys need.
const LAB_RUNNER: &str = "approval = \"lab-runner\"\n\
     \n\
     [package.monit]\n\
     zones   = []\n\
     service = true\n\
     dirs    = { \"monit/state\" = \"rw\" }\n\
     update  = true\n\
     remove  = false\n\
     \n\
     [package.vaultwarden]\n\
     zones   = [\"example.org\"]\n\
     service = true\n\
     dirs    = { \"vault\" = \"rw\" }\n\
     update  = true\n\
     remove  = false\n";

/// **The example file parses whole**, each entry holding exactly its keys' grants, in the
/// compiled records' order, and the check names what an entry does not allow.
#[test]
fn the_example_file_parses_whole() {
    let file = File::parse(LAB_RUNNER).unwrap();
    assert_eq!(file.name(), "lab-runner");
    assert_eq!(file.sections().len(), 2);
    let monit = file.entry("monit").unwrap();
    assert_eq!(
        monit.allows,
        [
            Granted::Directory("monit/state", DirectoryRights::RW),
            Granted::Service,
        ]
    );
    assert!(monit.update && !monit.remove);
    let vaultwarden = file.entry("vaultwarden").unwrap();
    assert_eq!(
        vaultwarden.allows,
        [
            Granted::Directory("vault", DirectoryRights::RW),
            Granted::Service,
            Granted::Zone("example.org"),
        ]
    );
    // The check names the extra grant, and a package the file does not bound receives nothing.
    assert_eq!(
        file.check("monit", &[Granted::Zone("example.net"), Granted::Network])
            .unwrap(),
        [Granted::Zone("example.net"), Granted::Network]
    );
    assert_eq!(file.check("monit", &[Granted::Service]).unwrap(), []);
    assert_eq!(file.entry("dialer"), None);
    assert_eq!(
        file.check("dialer", &[Granted::Network]).unwrap(),
        [Granted::Network]
    );
}

/// **Every key a line may carry reaches its grant**: the network and the installer, which no
/// example file asks for, and two directories in one table, each kept by its own name.
#[test]
fn the_network_and_the_installer_and_a_second_directory_are_kept() {
    let text = "approval = \"r\"\n\
         [package.m]\n\
         network   = true\n\
         installer = true\n\
         dirs      = { \"a\" = \"r\", \"b\" = \"rw\" }\n\
         update    = true\n\
         remove    = false\n";
    let file = File::parse(text).unwrap();
    let m = file.entry("m").unwrap();
    assert_eq!(
        m.allows,
        [
            Granted::Directory("a", DirectoryRights::R),
            Granted::Directory("b", DirectoryRights::RW),
            Granted::Installer,
            Granted::Network,
        ]
    );
}

/// **The grammar is held to exactly**: an unknown key, a bad value, a zone the resolver would
/// not spell, a rights word other than `r` or `rw`, a missing policy answer, a duplicate, a
/// section out of order, a name too long, and no header are each refused, naming the line.
#[test]
fn the_grammar_refuses_what_it_does_not_know() {
    let refused = |text: &str, why: &Why, at: usize| {
        let e = File::parse(text).unwrap_err();
        assert_eq!((e.at, &e.why), (at, why), "{text:?} -> {e:?}");
    };
    // A header alone bounds nothing: valid, and it grants nothing.
    let empty = File::parse("approval = \"r\"\n").unwrap();
    assert_eq!((empty.name(), empty.sections().len()), ("r", 0));
    refused(
        "[package.m]\nupdate = true\nremove = false\n",
        &Why::NoHeader,
        1,
    );
    refused(
        "approval = \"r\"\n[package.m]\nbogus = true\nupdate = true\nremove = false\n",
        &Why::UnknownKey("bogus".into()),
        3,
    );
    refused(
        "approval = \"r\"\n[package.m]\nservice = yes\nupdate = true\nremove = false\n",
        &Why::BadValue,
        3,
    );
    refused(
        "approval = \"r\"\n[package.m]\nzones = [\"Nife.test\"]\nupdate = true\nremove = false\n",
        &Why::ZoneNotCanonical,
        3,
    );
    refused(
        "approval = \"r\"\n[package.m]\nzones = [\"a..b\"]\nupdate = true\nremove = false\n",
        &Why::ZoneNotCanonical,
        3,
    );
    refused(
        "approval = \"r\"\n[package.m]\ndirs = { \"s\" = \"rwx\" }\nupdate = true\nremove = false\n",
        &Why::BadValue,
        3,
    );
    // Two directories of one table are each kept, and the second by the same name is a duplicate.
    refused(
        "approval = \"r\"\n[package.m]\ndirs = { \"a\" = \"r\", \"a\" = \"rw\" }\nupdate = true\nremove = false\n",
        &Why::DuplicateKey,
        3,
    );
    // This reader holds a basic string to no escapes, so a backslash is refused wherever it
    // stands, with or without a comment after the value.
    refused("approval = \"a\\b\" # note\n", &Why::BadName, 1);
    refused("approval = \"a\\b\"\n", &Why::BadName, 1);
    refused(
        "approval = \"r\"\n[package.m]\nservice = true\nremove = false\n",
        &Why::MissingPolicy("update"),
        4,
    );
    refused(
        "approval = \"r\"\n[package.m]\nupdate = true\nremove = false\n[package.m]\nupdate = true\nremove = false\n",
        &Why::SectionsOutOfOrder,
        5,
    );
    refused(
        "approval = \"r\"\n[package.m]\nupdate = true\nremove = false\n[package.a]\nupdate = true\nremove = false\n",
        &Why::SectionsOutOfOrder,
        5,
    );
    refused(
        &format!(
            "approval = \"r\"\n[package.{}]\nupdate = true\nremove = false\n",
            "x".repeat(33)
        ),
        &Why::BadName,
        2,
    );
    refused("approval r\n", &Why::NotTOML, 1);
    refused(
        "approval = \"r\"\n[package.m]\nzones = [\"nife.test\", \"nife.test\"]\nupdate = true\nremove = false\n",
        &Why::DuplicateKey,
        3,
    );
    refused(
        "approval = \"r\"\n[package.m]\n[package.m\nupdate = true\n",
        &Why::NotTOML,
        3,
    );
    refused(
        "approval = \"r\"\n[package.m.b]\nupdate = true\nremove = false\n",
        &Why::BadName,
        2,
    );
    // A duplicate header, and a duplicate key in one section.
    refused(
        "approval = \"r\"\napproval = \"s\"\n",
        &Why::DuplicateKey,
        2,
    );
    refused(
        "approval = \"r\"\n[package.m]\nupdate = true\nupdate = false\nremove = false\n",
        &Why::DuplicateKey,
        4,
    );
}

/// **The file compiles to the records the progenitor stores**, binding the file's digest in the
/// header, and the compiled document reads back through the store's own reader.
#[test]
fn the_file_compiles_to_the_stored_records() {
    let file = File::parse(LAB_RUNNER).unwrap();
    let digest = Digest::from([0x5au8; 32]);
    let mut out = vec![0u8; 4096];
    let n = compile(&file, digest, &mut out).unwrap();
    let text = core::str::from_utf8(&out[..n]).unwrap();
    let doc = installed_programs::approval::Document::parse(text).unwrap();
    assert_eq!(doc.name(), "lab-runner");
    assert_eq!(doc.digest(), digest);
    let monit = doc.entry("monit").unwrap().unwrap();
    assert_eq!(
        monit.allows().map(|a| a.unwrap()).collect::<Vec<_>>(),
        [
            Granted::Directory("monit/state", DirectoryRights::RW),
            Granted::Service,
        ]
    );
    assert!(monit.update && !monit.remove);
    // The zone array's order does not decide the compiled order; the records are sorted.
    let unsorted = "approval = \"r\"\n[package.m]\nzones = [\"b.test\", \"a.test\"]\nupdate = true\nremove = false\n";
    let file = File::parse(unsorted).unwrap();
    let mut out = vec![0u8; 4096];
    let n = compile(&file, digest, &mut out).unwrap();
    let text = core::str::from_utf8(&out[..n]).unwrap();
    assert!(
        text.contains("allow zone a.test\nallow zone b.test"),
        "{text}"
    );
}

/// **Names are held to one word a line and a component can hold**: the same bound the compiled
/// records' reader holds, with the dot refused here that a TOML table name cannot carry.
#[test]
fn names_are_held_to_the_compiled_bounds() {
    assert!(name_fits("lab-runner"));
    assert!(name_fits("vaultwarden"));
    assert!(!name_fits(""));
    assert!(!name_fits("a b"));
    assert!(!name_fits("#a"));
    assert!(!name_fits("a.b"));
    assert!(!name_fits(&"x".repeat(NAME_MAXIMUM + 1)));
    assert!(name_fits(&"x".repeat(NAME_MAXIMUM)));
}

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
    assert!(!grant.is_allowed() && !grant.is_unattended() && grant.accepts().is_empty());
    assert_eq!(grant.check(&good()), Err(Refusal::NotAllowed));
    let empty = File::parse("approval = \"f\"\n[base-update]\n").unwrap();
    assert!(!empty.base_update().is_allowed() && !empty.base_update().is_unattended());
}

/// **The section parses its three keys**, before or after the package sections.
#[test]
fn the_section_parses() {
    let file = File::parse(ALL_ON).unwrap();
    let grant = file.base_update();
    assert!(grant.is_allowed() && grant.is_unattended());
    assert_eq!(grant.accepts(), ["release-1"]);
    let mixed = "approval = \"f\"\n[package.a]\nupdate = false\nremove = false\n\
                 [base-update]\nallow = true\nunattended = false\naccepts = [\"k1\", \"k2\"]\n\
                 [package.b]\nupdate = false\nremove = false\n";
    let file = File::parse(mixed).unwrap();
    assert!(file.base_update().is_allowed() && !file.base_update().is_unattended());
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
