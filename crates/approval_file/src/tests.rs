//! **The tests of the file grammar and its compile**: the example file, the refusals that name
//! their lines, the check basalt's composition calls, and the compiled records.
//!
//! Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.

use alloc::format;
use alloc::vec;
use installed_programs::{Digest, Granted};

use super::*;

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
