//! **The tests of a generation's reads and edits**: the store the protocol in the crate
//! documentation uses, the version and pointer rules, the grant record and the `answered` line.
//!
//! Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.

extern crate std;
use std::collections::BTreeMap;
use std::string::{String, ToString};
use std::{format, vec};

use super::*;
use crate::answered::{answered, record, records_agree};
use crate::grants::{DirectoryRights, covers, extras};
use crate::network_granted;

/// The store as the protocol in the crate documentation uses it: generation files that are
/// written once, and one `current` line. A `BTreeMap` stands in for the directory.
struct Store {
    files: BTreeMap<String, String>,
}

impl Store {
    fn new() -> Self {
        let mut files = BTreeMap::new();
        files.insert("generation-0".to_string(), String::new());
        files.insert("current".to_string(), "0\n".to_string());
        Store { files }
    }
    fn current(&self) -> u32 {
        parse_current(&self.files["current"]).unwrap()
    }
    fn table(&self) -> &str {
        &self.files[&format!("generation-{}", self.current())]
    }
    fn newest(&self) -> u32 {
        self.files
            .keys()
            .filter_map(|k| k.strip_prefix("generation-"))
            .map(|n| n.parse::<u32>().unwrap())
            .max()
            .unwrap()
    }
    /// Write the next generation (never an existing file), then flip `current`: the commit.
    fn commit(&mut self, next: &[u8]) {
        let number = self.newest() + 1;
        let name = format!("generation-{number}");
        assert!(
            !self.files.contains_key(&name),
            "a generation was rewritten"
        );
        self.files
            .insert(name, String::from_utf8(next.to_vec()).unwrap());
        self.select(number);
    }
    fn select(&mut self, number: u32) {
        let mut out = [0u8; 16];
        let n = format_current(number, &mut out).unwrap();
        self.files.insert(
            "current".to_string(),
            String::from_utf8(out[..n].to_vec()).unwrap(),
        );
    }
    fn install(&mut self, entry: Entry<'_>) {
        let mut out = vec![0u8; 4096];
        let n = with_entry(self.table(), &entry, false, &mut out).unwrap();
        self.commit(&out[..n]);
    }
    fn remove(&mut self, package: &str) -> Result<(), Error> {
        let mut out = vec![0u8; 4096];
        let n = without_package(self.table(), package, None, &Answered::Console, &mut out)?;
        self.commit(&out[..n]);
        Ok(())
    }
    fn remove_version(&mut self, package: &str, version: &str) -> Result<(), Error> {
        let mut out = vec![0u8; 4096];
        let n = without_package(
            self.table(),
            package,
            Some(version),
            &Answered::Console,
            &mut out,
        )?;
        self.commit(&out[..n]);
        Ok(())
    }
    /// The version the bare word for `program` runs, or `None`.
    fn bare_version_of(&self, program: &str) -> Option<String> {
        lookup(self.table(), program)
            .unwrap()
            .map(|e| e.version.to_string())
    }
    fn live_versions_of(&self, program: &str) -> std::vec::Vec<String> {
        versions_of(self.table(), program)
            .map(|v| v.unwrap().to_string())
            .collect()
    }
}

/// A package row. One digest per version, so tests can name digests by version.
fn row<'a>(program: &'a str, package: &'a str, version: &'a str, seed: u8) -> Entry<'a> {
    Entry {
        program,
        version,
        package,
        digest: [seed; 32],
        manager: "jig",
    }
}

fn vouch<'a>(program: &'a str, seed: u8) -> Entry<'a> {
    Entry {
        program,
        version: NO_VERSION,
        package: OWNER,
        digest: [seed; 32],
        manager: OWNER,
    }
}

/// **The property §208 asked for by name**: a rollback restores the whole set, not one package.
/// **An owner's vouch is found by digest and claims no name** (DECISIONS §221 (the boot prompt
/// is the owner's console); §229 (how a bare name at the prompt reaches an installed program),
/// B2): it reads back, writes no pointer, and an install of the same program name sits beside
/// it rather than replacing it, and holds the pointer the vouch never did.
#[test]
fn an_owner_vouch_is_an_ordinary_row() {
    let built = [9u8; 32];
    let mut g = [0u8; 512];
    let n = with_entry("", &vouch("a.out", 9), false, &mut g).unwrap();
    let table = core::str::from_utf8(&g[..n]).unwrap();
    assert_eq!(
        lookup_digest(table, &built).unwrap().unwrap().package,
        OWNER
    );
    assert!(
        lookup(table, "a.out").unwrap().is_none(),
        "a vouch claims no name"
    );
    assert_eq!(defaults(table).count(), 0, "a vouch writes no pointer");
    let upgrade = Entry {
        program: "a.out",
        version: "0.1.0",
        package: "a.out",
        digest: [1; 32],
        manager: "jig",
    };
    let mut h = [0u8; 512];
    let n = with_entry(table, &upgrade, false, &mut h).unwrap();
    let next = core::str::from_utf8(&h[..n]).unwrap();
    assert_eq!(lookup_digest(next, &built).unwrap().unwrap().package, OWNER);
    assert_eq!(lookup(next, "a.out").unwrap().unwrap().digest, [1; 32]);
}

/// **§229 B2, the install half, as milestone 614 reshaped it**: installing a second version
/// appends a second row and moves the pointer; another package cannot take the name; and a
/// vouch of the same name neither takes the bare name nor is taken by an install. Removing the
/// package leaves the vouch and takes the pointer.
#[test]
fn a_name_belongs_to_one_package_and_never_to_a_vouch() {
    let mut g = [0u8; 512];
    let n = with_entry("", &row("uptime", "uptime", "0.1.0", 1), false, &mut g).unwrap();
    let t1 = core::str::from_utf8(&g[..n]).unwrap().to_string();

    let mut h = [0u8; 512];
    let n = with_entry(&t1, &row("uptime", "uptime", "0.2.0", 2), false, &mut h).unwrap();
    let t2 = core::str::from_utf8(&h[..n]).unwrap().to_string();
    assert_eq!(lookup(&t2, "uptime").unwrap().unwrap().digest, [2; 32]);
    assert_eq!(entries(&t2).count(), 2, "both versions stay live");
    assert!(lookup_digest(&t2, &[1; 32]).unwrap().is_some());

    let mut k = [0u8; 512];
    assert_eq!(
        with_entry(&t2, &row("uptime", "procps", "4.0.0", 3), false, &mut k),
        Err(Error::Taken)
    );

    let n = with_entry(&t2, &vouch("uptime", 4), false, &mut k).unwrap();
    let t3 = core::str::from_utf8(&k[..n]).unwrap().to_string();
    assert_eq!(lookup(&t3, "uptime").unwrap().unwrap().digest, [2; 32]);
    assert_eq!(
        lookup_digest(&t3, &[4; 32]).unwrap().unwrap().package,
        OWNER
    );

    let mut m = [0u8; 512];
    let n = without_package(&t3, "uptime", None, &Answered::Console, &mut m).unwrap();
    let t4 = core::str::from_utf8(&m[..n]).unwrap();
    assert!(lookup(t4, "uptime").unwrap().is_none());
    assert_eq!(lookup_digest(t4, &[4; 32]).unwrap().unwrap().package, OWNER);
    assert_eq!(defaults(t4).count(), 0, "the pointer went with the program");
}

/// **§229, calef's ruling of 2026-09-27: a package cannot take a name the image carries.**
/// Refused before `Taken` and before a byte is written, whether or not the table already has
/// the name, and at any version. An owner's vouch of the same name claims no name and is kept.
#[test]
fn a_package_cannot_take_an_image_programs_name() {
    let mut out = [0u8; 512];
    let base = row("uptime", "uptime", "0.1.0", 1);
    assert_eq!(with_entry("", &base, true, &mut out), Err(Error::ImageName));

    // Already installed (a later base added the name, which §229's prompt refusal covers):
    // an install of that package at a new version is refused too, and so is another package's.
    let n = with_entry("", &base, false, &mut out).unwrap();
    let t = core::str::from_utf8(&out[..n]).unwrap().to_string();
    let mut next = [0u8; 512];
    let upgrade = row("uptime", "uptime", "0.2.0", 2);
    assert_eq!(
        with_entry(&t, &upgrade, true, &mut next),
        Err(Error::ImageName)
    );
    let other = row("uptime", "procps", "4.0.0", 3);
    assert_eq!(
        with_entry(&t, &other, true, &mut next),
        Err(Error::ImageName)
    );

    let n = with_entry(&t, &vouch("uptime", 4), true, &mut next).unwrap();
    let t2 = core::str::from_utf8(&next[..n]).unwrap();
    assert_eq!(lookup_digest(t2, &[4; 32]).unwrap().unwrap().package, OWNER);
}

/// **Milestone 614's first Done-means test**: installing 0.2.0 over 0.1.0 leaves both digests
/// live, the bare name resolves to the version the ruled default names (ruling 3: the newest
/// install), and a rebuild claiming a live version string is a second row (ruling 2).
#[test]
fn two_versions_of_one_program_stay_live_and_the_bare_word_means_the_newest() {
    let mut store = Store::new();
    store.install(row("uptime", "uptime", "0.1.0", 1));
    assert_eq!(store.bare_version_of("uptime").unwrap(), "0.1.0");
    store.install(row("uptime", "uptime", "0.2.0", 2));

    let table = store.table();
    assert!(lookup_digest(table, &[1; 32]).unwrap().is_some());
    assert!(lookup_digest(table, &[2; 32]).unwrap().is_some());
    assert_eq!(
        store.bare_version_of("uptime").unwrap(),
        "0.2.0",
        "ruling 3: the bare word means the newest install"
    );
    assert_eq!(store.live_versions_of("uptime"), ["0.1.0", "0.2.0"]);

    // **A rebuild claiming a live version string is a second row, never a replacement** (the
    // bytes differ, and the digest is the key). The pointer moves to it, because it is the
    // newest install.
    store.install(row("uptime", "uptime", "0.1.0", 5));
    let table = store.table();
    assert_eq!(entries(table).count(), 3, "three rows, one per digest");
    assert!(lookup_version(table, "uptime", "0.1.0").unwrap().is_some());
    assert_eq!(store.bare_version_of("uptime").unwrap(), "0.1.0");

    // Reinstalling the *same* bytes is a relabel, not a new row: the rebuild's row changes
    // version string under its digest, and the original 0.1.0 row still says what it said.
    store.install(row("uptime", "uptime", "0.1.0-rc1", 5));
    let table = store.table();
    assert_eq!(entries(table).count(), 3, "same digest, same row");
    assert_eq!(
        lookup_version(table, "uptime", "0.1.0")
            .unwrap()
            .unwrap()
            .digest,
        [1; 32],
        "the original row is untouched"
    );
    assert!(
        lookup_version(table, "uptime", "0.1.0-rc1")
            .unwrap()
            .is_some()
    );
}

/// **The version-qualified removal** (`jig remove <package>@<version>`): removing one version
/// leaves the other; removing the default's version moves the pointer to the sole survivor; and with several remaining the
/// removal refuses and [`versions_of`] names the candidates. Removing a version that does not
/// hold the pointer leaves the pointer alone.
#[test]
fn a_version_qualified_remove_honours_the_pointer_rule() {
    let mut store = Store::new();
    store.install(row("uptime", "uptime", "0.1.0", 1));
    store.install(row("uptime", "uptime", "0.2.0", 2));

    // A version that does not hold the pointer: the row goes, the pointer stays.
    store.remove_version("uptime", "0.1.0").unwrap();
    assert_eq!(store.bare_version_of("uptime").unwrap(), "0.2.0");
    assert!(
        lookup_version(store.table(), "uptime", "0.1.0")
            .unwrap()
            .is_none()
    );
    store.install(row("uptime", "uptime", "0.1.0", 1));

    // The default's version, one survivor: the pointer moves to it.
    store.remove_version("uptime", "0.2.0").unwrap();
    assert_eq!(store.bare_version_of("uptime").unwrap(), "0.1.0");

    // The default's version, several survivors: refused, candidates named, nothing written.
    store.install(row("uptime", "uptime", "0.3.0", 3));
    store.install(row("uptime", "uptime", "0.4.0", 4));
    assert_eq!(store.bare_version_of("uptime").unwrap(), "0.4.0");
    let newest = store.newest();
    let mut out = [0u8; 4096];
    assert_eq!(
        without_package(
            store.table(),
            "uptime",
            Some("0.4.0"),
            &Answered::Console,
            &mut out
        ),
        Err(Error::SeveralVersions)
    );
    assert_eq!(store.newest(), newest, "a refusal writes no generation");
    assert_eq!(
        store.live_versions_of("uptime"),
        ["0.1.0", "0.3.0", "0.4.0"],
        "the candidates the refusal names"
    );

    // A version nobody installed is a report, not a silence.
    assert_eq!(
        store.remove_version("uptime", "9.9.9"),
        Err(Error::NotInstalled)
    );
}

/// **Bare removal at two versions live, milestone 614's second Done-means test**:
/// `jig remove <package>` takes every live version's row and the pointer, and a rollback
/// brings both rows back, because bytes are never deleted.
#[test]
fn bare_removal_takes_every_version_and_a_rollback_brings_them_back() {
    let mut store = Store::new();
    store.install(row("uptime", "uptime", "0.1.0", 1));
    store.install(row("uptime", "uptime", "0.2.0", 2));
    let before = store.current();

    store.remove("uptime").unwrap();
    assert_eq!(
        store.live_versions_of("uptime"),
        std::vec::Vec::<String>::new()
    );
    assert_eq!(store.bare_version_of("uptime"), None);
    assert_eq!(defaults(store.table()).count(), 0);

    store.select(before);
    assert_eq!(store.live_versions_of("uptime"), ["0.1.0", "0.2.0"]);
    assert_eq!(store.bare_version_of("uptime").unwrap(), "0.2.0");
}

/// Two rows at one version string (a rebuild) are one *version*, so a qualified removal that
/// leaves only them moves the pointer to the first of them rather than refusing: the refusal's
/// candidates are versions, and there is one.
#[test]
fn a_rebuild_is_one_survivor_for_the_pointer_rule() {
    let mut store = Store::new();
    store.install(row("uptime", "uptime", "0.1.0", 1));
    // A rebuild of 0.1.0: a second row at the same version string (ruling 2).
    store.install(row("uptime", "uptime", "0.1.0", 5));
    store.install(row("uptime", "uptime", "0.2.0", 2));
    assert_eq!(store.bare_version_of("uptime").unwrap(), "0.2.0");

    store.remove_version("uptime", "0.2.0").unwrap();
    // The pointer's version went; two rows remain and they are one version, so the pointer
    // moves to the first of them instead of refusing.
    assert_eq!(store.live_versions_of("uptime"), ["0.1.0", "0.1.0"]);
    assert_eq!(
        store.bare_version_of("uptime").unwrap(),
        "0.1.0",
        "the sole remaining version"
    );
    assert_eq!(
        lookup(store.table(), "uptime").unwrap().unwrap().digest,
        [1; 32],
        "the first row of the surviving version"
    );
}

/// Two programs installed, one upgraded beside itself, one removed; selecting the generation
/// before both changes brings back the old version of the first and the presence of the second
/// together. The pointer rides along: a generation is one snapshot of rows and pointer
/// (milestone 614 (two installed versions of one program, each runnable)).
#[test]
fn a_rollback_restores_the_whole_set() {
    let mut store = Store::new();
    store.install(row("uptime", "uptime", "0.1.0", 1));
    store.install(row("date", "date", "1.0.0", 2));
    let before = store.current();

    store.install(row("uptime", "uptime", "0.2.0", 3));
    store.remove("date").unwrap();
    assert_eq!(store.bare_version_of("uptime").unwrap(), "0.2.0");
    assert_eq!(store.bare_version_of("date"), None);

    store.select(before);
    assert_eq!(store.bare_version_of("uptime").unwrap(), "0.1.0");
    assert_eq!(store.bare_version_of("date").unwrap(), "1.0.0");
    // And rolling forward again is the same act.
    let newest = store.newest();
    store.select(newest);
    assert_eq!(store.bare_version_of("date"), None);
}

/// An install appends; the pointer names the newest. (Until milestone 614 this test asserted
/// the opposite, that an upgrade replaced in place, which is what one entry per name could do.)
#[test]
fn an_install_appends_and_the_pointer_follows_the_newest() {
    let mut store = Store::new();
    store.install(row("a", "a", "1", 1));
    store.install(row("b", "b", "1", 2));
    store.install(row("a", "a", "2", 3));
    let programs: std::vec::Vec<_> = entries(store.table()).map(|e| e.unwrap().program).collect();
    assert_eq!(programs, ["a", "b", "a"]);
    assert_eq!(lookup(store.table(), "a").unwrap().unwrap().digest, [3; 32]);
    assert_eq!(
        lookup_version(store.table(), "a", "1")
            .unwrap()
            .unwrap()
            .digest,
        [1; 32]
    );
}

#[test]
fn removing_what_is_not_installed_is_reported_and_writes_nothing() {
    let mut store = Store::new();
    store.install(row("a", "a", "1", 1));
    let newest = store.newest();
    assert_eq!(store.remove("b"), Err(Error::NotInstalled));
    assert_eq!(store.newest(), newest);
}

#[test]
fn a_malformed_line_anywhere_makes_the_table_vouch_for_nothing() {
    let good = format!("sha256:{} a 1 a\n", "11".repeat(32));
    let pointer = format!("{good}default a sha256:{}\n", "11".repeat(32));
    // The bare name needs its pointer: rows without one answer `None`, never a guess.
    assert!(lookup(&good, "a").unwrap().is_none());
    assert!(lookup(&pointer, "a").unwrap().is_some());
    for bad in [
        // A bad row, after a good one.
        format!("{good}b 2 b nothex\n"),
        format!("{good}b 2 b\n"),
        format!("{good}b 2 b {} extra\n", "22".repeat(32)),
        format!("{good}b 2 b {}\n", "2".repeat(63)),
        // A digest with no label, as every row was written before 2026-10-07, and one under a
        // label this reader does not know (DECISIONS §197's digest ruling).
        format!("{good}{} b 2 b\n", "22".repeat(32)),
        format!("{good}merkle-sha256-8k:{} b 2 b\n", "22".repeat(32)),
        format!("{good}default a {}\n", "11".repeat(32)),
        // A bad pointer line.
        format!("{good}default a\n"),
        format!("{good}default a nothex\n"),
        format!("{good}default a {} extra\n", "22".repeat(32)),
        // A pointer naming a digest no row of the program carries.
        format!("{good}default a {}\n", "33".repeat(32)),
    ] {
        assert_eq!(lookup(&bad, "a"), Err(Error::Malformed), "{bad:?}");
    }
    // Comments and blank lines are not rows, and the pointer survives them.
    let commented = format!("# installed\n\n{pointer}");
    assert!(lookup(&commented, "a").unwrap().is_some());
    // Two pointers for one program is no single answer.
    let twice = format!("{pointer}default a {}\n", "33".repeat(32));
    assert_eq!(lookup(&twice, "a"), Err(Error::Malformed));
}

#[test]
fn a_name_that_would_not_read_back_is_refused() {
    let mut out = [0u8; 512];
    for (program, version, package) in [
        ("", "1", "p"),
        ("a b", "1", "p"),
        ("a", "", "p"),
        ("a", "1 2", "p"),
        ("a", "1", "p\nq"),
        ("#a", "1", "p"),
    ] {
        let e = Entry {
            program,
            version,
            package,
            digest: [0; 32],
            manager: "jig",
        };
        assert_eq!(
            with_entry("", &e, false, &mut out),
            Err(Error::BadName),
            "{program:?} {version:?}"
        );
    }
}

/// **A row records its manager, and a row from before the column reads as unrecorded**
/// (milestone 809, calef's 2026-10-06 ruling). The four-word row is what every table on a disk
/// written before this change holds, so refusing it would make each one unreadable.
#[test]
fn a_row_records_its_manager_and_an_older_row_reads_as_unrecorded() {
    let mut out = [0u8; 512];
    let n = with_entry(
        "",
        &row("greeting", "greeting", "0.1.0", 3),
        false,
        &mut out,
    )
    .unwrap();
    let table = core::str::from_utf8(&out[..n]).unwrap();
    assert!(table.lines().next().unwrap().ends_with(" greeting jig"));
    assert_eq!(lookup(table, "greeting").unwrap().unwrap().manager, "jig");
    let older = format!("sha256:{} a 1 a\n", "11".repeat(32));
    assert_eq!(entries(&older).next().unwrap().unwrap().manager, NO_MANAGER);
    let six = format!("sha256:{} a 1 a jig extra\n", "11".repeat(32));
    assert_eq!(entries(&six).next().unwrap(), Err(Error::Malformed));
}

/// **A manager may not undo another manager's rows** (calef, 2026-10-06), and the owner may
/// undo anything. `foreign_change` is what a rollback asks of the two generations it moves
/// between, and `can_edit` what a removal asks of the row.
#[test]
fn a_manager_may_not_undo_another_managers_rows() {
    assert!(can_edit("jig", "jig"));
    assert!(can_edit(OWNER, "jig"));
    assert!(can_edit(NO_MANAGER, "jig"));
    assert!(!can_edit("other", "jig"));
    assert!(can_edit("other", OWNER));
    let mut a = [0u8; 512];
    let n = with_entry("", &row("greeting", "greeting", "0.1.0", 3), false, &mut a).unwrap();
    let older = core::str::from_utf8(&a[..n]).unwrap();
    let theirs = Entry {
        manager: "other",
        ..row("hello", "hello", "1", 4)
    };
    let mut b = [0u8; 512];
    let n = with_entry(older, &theirs, false, &mut b).unwrap();
    let newer = core::str::from_utf8(&b[..n]).unwrap();
    assert_eq!(
        foreign_change(older, newer, "jig")
            .unwrap()
            .unwrap()
            .program,
        "hello"
    );
    assert_eq!(foreign_change(older, newer, OWNER).unwrap(), None);
    assert_eq!(foreign_change(older, newer, "other").unwrap(), None);
    // The same generation twice changes nothing.
    assert_eq!(foreign_change(newer, newer, "jig").unwrap(), None);
}

/// **A row differs when its program and digest are not both in the other table.** Another
/// manager's program upgraded to new bytes keeps its name but not its digest, and a rollback
/// across that upgrade undoes their row, so it is foreign to `jig` although the name matches.
#[test]
fn a_new_digest_under_the_same_name_is_a_change() {
    let older = format!("sha256:{} hello 1 hello other\n", "11".repeat(32));
    let newer = format!("sha256:{} hello 2 hello other\n", "22".repeat(32));
    let found = foreign_change(&older, &newer, "jig").unwrap().unwrap();
    assert_eq!((found.program, found.manager), ("hello", "other"));
}

/// A manager column is a name like any other column: one starting `#` would read as a comment
/// when the table is shown, so the line is malformed rather than a row with an odd manager.
#[test]
fn a_manager_that_is_not_a_name_makes_the_line_malformed() {
    let bad = format!("sha256:{} a 1 a #jig\n", "11".repeat(32));
    assert_eq!(entries(&bad).next().unwrap(), Err(Error::Malformed));
}

#[test]
fn a_buffer_too_small_is_refused_rather_than_truncated() {
    let mut out = [0u8; 40];
    let e = row("uptime", "uptime", "0.1.0", 1);
    assert_eq!(with_entry("", &e, false, &mut out), Err(Error::TooSmall));
    assert_eq!(format_current(1234, &mut [0u8; 4]), Err(Error::TooSmall));
}

/// **A digest finds its row, and a malformed line elsewhere still vouches for nothing**
/// (§219 D). The miss is a real `None` rather than an error, because "not installed" is the
/// ordinary answer for a binary somebody just built.
#[test]
fn a_digest_finds_its_row_and_a_miss_is_none() {
    let mut out = [0u8; 512];
    let n = with_entry("", &row("uptime", "uptime", "0.1.0", 3), false, &mut out).unwrap();
    let mut two = [0u8; 512];
    let text = core::str::from_utf8(&out[..n]).unwrap();
    let n = with_entry(text, &row("date", "date", "0.1.0", 4), false, &mut two).unwrap();
    let table = core::str::from_utf8(&two[..n]).unwrap();

    let hit = lookup_digest(table, &[4u8; 32]).unwrap().unwrap();
    assert_eq!(hit.program, "date");
    assert!(lookup_digest(table, &[9u8; 32]).unwrap().is_none());

    let mut broken = String::from(table);
    broken.push_str("not a line\n");
    assert_eq!(lookup_digest(&broken, &[4u8; 32]), Err(Error::Malformed));
}

/// The row a version set resolves to: first in file order, never a vouch, never another
/// program's.
#[test]
fn a_version_finds_its_row() {
    let mut out = [0u8; 512];
    let n = with_entry("", &row("uptime", "uptime", "0.1.0", 1), false, &mut out).unwrap();
    let text = core::str::from_utf8(&out[..n]).unwrap().to_string();
    let mut two = [0u8; 512];
    let n = with_entry(&text, &row("uptime", "uptime", "0.2.0", 2), false, &mut two).unwrap();
    let text = core::str::from_utf8(&two[..n]).unwrap().to_string();
    let mut three = [0u8; 512];
    let n = with_entry(&text, &vouch("uptime", 7), false, &mut three).unwrap();
    let table = core::str::from_utf8(&three[..n]).unwrap();
    assert_eq!(
        lookup_version(table, "uptime", "0.1.0")
            .unwrap()
            .unwrap()
            .digest,
        [1; 32]
    );
    assert_eq!(
        lookup_version(table, "uptime", "0.2.0")
            .unwrap()
            .unwrap()
            .digest,
        [2; 32]
    );
    assert_eq!(lookup_version(table, "uptime", "0.3.0").unwrap(), None);
    // Neither is the vouch's own column, which is not a version anyone asked for.
    assert_eq!(lookup_version(table, "uptime", NO_VERSION).unwrap(), None);
    assert_eq!(lookup_version(table, "date", "0.1.0").unwrap(), None);
    assert_eq!(versions_of(table, "uptime").count(), 2);
    assert_eq!(versions_of(table, "date").count(), 0);
}

#[test]
fn a_generation_is_named_by_its_number() {
    let mut buf = [0u8; 10];
    assert_eq!(generation_name(0, &mut buf), "0");
    assert_eq!(generation_name(17, &mut buf), "17");
    assert_eq!(generation_name(u32::MAX, &mut buf), "4294967295");
}

#[test]
fn the_current_line_reads_back() {
    for n in [0, 7, 10, 4_294_967_295] {
        let mut out = [0u8; 16];
        let len = format_current(n, &mut out).unwrap();
        assert_eq!(
            parse_current(core::str::from_utf8(&out[..len]).unwrap()),
            Some(n)
        );
    }
    for bad in ["", "\n", "-1", "1 ", "4294967296", "0x1"] {
        assert_eq!(parse_current(bad), None, "{bad:?}");
    }
}

/// A row is refused for any one bad name, not only when all three are bad: each of program,
/// version and package is checked on its own.
#[test]
fn a_row_with_one_bad_name_is_malformed() {
    let hex = format!("sha256:{}", "22".repeat(32));
    for bad in [
        format!("{hex} #p 1 pkg\n"),
        format!("{hex} p #1 pkg\n"),
        format!("{hex} p 1 #pkg\n"),
    ] {
        assert_eq!(lookup(&bad, "p"), Err(Error::Malformed), "{bad:?}");
    }
    assert!(
        lookup_digest(&format!("{hex} p 1 pkg\n"), &[0x22; 32])
            .unwrap()
            .is_some()
    );
}

/// `parse_current` takes decimal digits only: `u32::from_str` would also take a leading `+`.
#[test]
fn a_current_line_with_a_plus_sign_is_not_a_generation() {
    assert_eq!(parse_current("+5"), None);
    assert_eq!(parse_current("+5\n"), None);
    assert_eq!(parse_current("5\n"), Some(5));
}

/// Two default pointers for one program are no single answer, and a removal that needs the
/// pointer's digest says so rather than reading the last one.
#[test]
fn removing_a_version_from_a_table_with_two_pointers_is_malformed() {
    let (a, b) = ("11".repeat(32), "22".repeat(32));
    let table = format!("{a} p 1 pkg\n{b} p 2 pkg\ndefault p {a}\ndefault p {b}\n");
    let mut out = [0u8; 512];
    assert_eq!(
        without_package(&table, "pkg", Some("1"), &Answered::Console, &mut out),
        Err(Error::Malformed)
    );
}

/// Removing one version of a package looks only at that package's rows: another program, and
/// the owner's vouch for this one, are not survivors, so they cannot make the removal
/// ambiguous or take the pointer.
#[test]
fn other_rows_are_not_survivors_of_a_qualified_removal() {
    let mut store = Store::new();
    store.install(row("date", "date", "1.0.0", 9));
    store.install(vouch("uptime", 7));
    store.install(row("uptime", "uptime", "0.1.0", 1));
    store.install(row("uptime", "uptime", "0.2.0", 2));
    store.remove_version("uptime", "0.2.0").unwrap();
    assert_eq!(store.bare_version_of("uptime").unwrap(), "0.1.0");
    assert_eq!(store.bare_version_of("date").unwrap(), "1.0.0");
    assert!(
        lookup_digest(store.table(), &[7; 32]).unwrap().is_some(),
        "the vouch stays"
    );
}

/// Removing a version that does not hold the pointer leaves the pointer exactly where it was,
/// even when the first survivor is some other version; and another program's pointer is never
/// the removal's to move.
#[test]
fn a_removal_moves_only_its_own_programs_pointer_and_only_when_it_held_it() {
    let mut store = Store::new();
    store.install(row("date", "date", "1.0.0", 9));
    store.install(row("uptime", "uptime", "0.1.0", 1));
    store.install(row("uptime", "uptime", "0.2.0", 2));
    store.install(row("uptime", "uptime", "0.3.0", 3));
    store.remove_version("uptime", "0.1.0").unwrap();
    assert_eq!(store.bare_version_of("uptime").unwrap(), "0.3.0");
    assert_eq!(store.bare_version_of("date").unwrap(), "1.0.0");

    // The pointer's own version goes, and a second program's pointer sits after it in the file.
    store.install(row("zed", "zed", "1", 8));
    store.remove_version("uptime", "0.3.0").unwrap();
    assert_eq!(store.bare_version_of("uptime").unwrap(), "0.2.0");
    assert_eq!(store.bare_version_of("zed").unwrap(), "1");
    assert_eq!(store.bare_version_of("date").unwrap(), "1.0.0");
}

/// A qualified removal takes its version's rows only. Two rows sharing a digest (a
/// hand-written table; an install relabels instead) are not both taken because they agree on
/// it, and the pointer stays on the digest the survivor still carries.
#[test]
fn a_qualified_removal_takes_one_row_even_when_a_digest_repeats() {
    let a = format!("sha256:{}", "11".repeat(32));
    let table = format!("{a} p 1 pkg\n{a} p 2 pkg\ndefault p {a}\n");
    let mut out = [0u8; 512];
    let n = without_package(&table, "pkg", Some("2"), &Answered::Console, &mut out).unwrap();
    let after = core::str::from_utf8(&out[..n]).unwrap();
    assert_eq!(versions_of(after, "p").count(), 1);
    assert!(lookup_version(after, "p", "1").unwrap().is_some());
}

/// Why [`install_package`] refused: the error, the program and the holder, owned.
type Why = (Error, Option<String>, Option<String>);

/// Install every row of one package as one generation, or say why not.
fn install_package(store: &mut Store, rows: &[Entry<'_>]) -> Result<(), Why> {
    let mut out = vec![0u8; 4096];
    let table = store.table().to_string();
    let n =
        with_entries(&table, || rows.iter().copied(), |p| p == "ls", &mut out).map_err(|r| {
            let owned = |n: Option<&str>| n.map(String::from);
            (r.error, owned(r.program), owned(r.holder))
        })?;
    store.commit(&out[..n]);
    Ok(())
}

/// **A package's programs are one install** (milestone 919 (a package carries several
/// programs)): two programs named unlike their package are two rows and two pointers in one
/// generation, each bare word finding its own row, and a rollback of one step undoes both.
#[test]
fn a_package_with_two_programs_is_one_generation_with_a_row_and_pointer_each() {
    let mut store = Store::new();
    store.install(row("date", "date", "1.0.0", 9));
    let before = store.current();
    install_package(
        &mut store,
        &[
            row("alto", "duet", "0.1.0", 1),
            row("tenor", "duet", "0.1.0", 2),
        ],
    )
    .unwrap();
    assert_eq!(
        store.current(),
        before + 1,
        "one generation for the package"
    );
    let table = store.table();
    assert_eq!(lookup(table, "alto").unwrap().unwrap().digest, [1; 32]);
    assert_eq!(lookup(table, "tenor").unwrap().unwrap().digest, [2; 32]);
    assert!(
        lookup(table, "duet").unwrap().is_none(),
        "the package is not a program"
    );
    assert_eq!(defaults(table).count(), 3);
    store.select(before);
    assert_eq!(store.bare_version_of("alto"), None);
    assert_eq!(store.bare_version_of("tenor"), None);
}

/// **calef, 2026-10-14 (UTC): "If two packages offer the same program name, refuse the second
/// package. Need to remove the first one before installing the second one."** One held name
/// refuses the whole second package, naming the program and the package that holds it, and
/// writes nothing; once the holder is removed, the second package installs. A program whose
/// name the image carries refuses its whole package the same way.
#[test]
fn a_held_name_refuses_the_whole_second_package_until_the_holder_is_removed() {
    let mut store = Store::new();
    install_package(
        &mut store,
        &[
            row("alto", "duet", "0.1.0", 1),
            row("tenor", "duet", "0.1.0", 2),
        ],
    )
    .unwrap();
    let newest = store.newest();
    let solo = [row("bass", "solo", "1", 3), row("alto", "solo", "1", 4)];
    assert_eq!(
        install_package(&mut store, &solo),
        Err((Error::Taken, Some("alto".into()), Some("duet".into())))
    );
    assert_eq!(store.newest(), newest, "a refusal writes no generation");
    assert_eq!(store.bare_version_of("bass"), None, "not installed in part");

    let image = [row("bass", "solo", "1", 3), row("ls", "solo", "1", 5)];
    assert_eq!(
        install_package(&mut store, &image),
        Err((Error::ImageName, Some("ls".into()), None))
    );

    store.remove("duet").unwrap();
    install_package(&mut store, &solo).unwrap();
    assert_eq!(
        lookup(store.table(), "alto").unwrap().unwrap().package,
        "solo"
    );
}

/// **calef, 2026-10-14 (UTC): "jig remove should work on packages, not programs."** Removing
/// a package takes every program it installed and their pointers, and nothing of another
/// package's; a program's name is not a package and removes nothing.
#[test]
fn removing_a_package_takes_every_program_it_installed() {
    let mut store = Store::new();
    store.install(row("date", "date", "1.0.0", 9));
    install_package(
        &mut store,
        &[
            row("alto", "duet", "0.1.0", 1),
            row("tenor", "duet", "0.1.0", 2),
        ],
    )
    .unwrap();
    assert_eq!(store.remove("alto"), Err(Error::NotInstalled));
    store.remove("duet").unwrap();
    assert_eq!(store.bare_version_of("alto"), None);
    assert_eq!(store.bare_version_of("tenor"), None);
    assert_eq!(store.bare_version_of("date").unwrap(), "1.0.0");
    assert_eq!(defaults(store.table()).count(), 1);
}

/// **A grant line is the owner's grant, read strictly** (calef's 2026-10-19 (UTC) ruling on
/// #2405): the network and zones read back for their digest and no other, the root zone as
/// `.`; a grant for no row, an unsorted or repeated grant, a zone that is not canonical, or a
/// word after one makes the whole table unreadable. An install carries the grants of the rows
/// it keeps, and a removal drops its rows' grants.
#[test]
fn grant_lines_are_read_strictly_and_follow_their_rows() {
    let a = [1u8; 32];
    let b = [2u8; 32];
    let text = |d: &Digest| String::from_utf8(measured_boot::digest_text(d).to_vec()).unwrap();
    let (ta, tb) = (text(&a), text(&b));
    // A comment and a blank line are skipped, as in every generation; `alto` is granted a zone
    // and not the network, so the network is never read off another grant of its digest.
    let table = format!(
        "# generation 1\n{ta} dialer 0.1.0 dialer jig\n{tb} alto 0.1.0 duet jig\n\n\
         default dialer {ta}\ndefault alto {tb}\ngrant {ta} network\ngrant {ta} zone .\n\
         grant {ta} zone nife.test\ngrant {tb} zone nife.test\n"
    );
    assert_eq!(network_granted(&table, &a), Ok(true));
    assert_eq!(network_granted(&table, &b), Ok(false));
    let zones: vec::Vec<_> = grants(&table)
        .map(|g| g.unwrap())
        .filter(|g| g.digest == a)
        .filter_map(|g| match g.what {
            Granted::Zone(z) => Some(z),
            Granted::Network | Granted::Service | Granted::Installer | Granted::Directory(..) => {
                None
            }
        })
        .collect();
    assert_eq!(zones, ["", "nife.test"]);
    assert!(
        lookup_digest(&table, &a).unwrap().is_some(),
        "rows still read"
    );

    let c = text(&[3u8; 32]);
    for bad in [
        format!("grant {c} network\n"),
        format!("grant {ta} zone nife.test\ngrant {ta} network\n"),
        format!("grant {ta} network\ngrant {ta} network\n"),
        format!("grant {ta} zone Nife.test\n"),
        format!("grant {ta} network now\n"),
        format!("grant {ta} clock\n"),
    ] {
        let t = format!("{ta} dialer 0.1.0 dialer jig\n{bad}");
        assert_eq!(network_granted(&t, &a), Err(Error::Malformed), "{bad}");
    }

    let mut out = vec![0u8; 4096];
    let duet2 = Entry {
        program: "tenor",
        version: "0.1.0",
        package: "duet",
        digest: [4u8; 32],
        manager: "jig",
    };
    let n = with_entry(&table, &duet2, false, &mut out).unwrap();
    let next = core::str::from_utf8(&out[..n]).unwrap();
    assert_eq!(
        network_granted(next, &a),
        Ok(true),
        "an install keeps others' grants"
    );
    let next = next.to_string();
    let n = without_package(&next, "dialer", None, &Answered::Console, &mut out).unwrap();
    let after = core::str::from_utf8(&out[..n]).unwrap();
    assert!(
        !after.contains(&format!("grant {ta}")),
        "a removal takes its rows' grants: {after}"
    );
    assert_eq!(network_granted(after, &a), Ok(false));
    assert!(
        after.contains(&format!("grant {tb} zone nife.test")),
        "and only its rows'"
    );
}

/// **An install writes the owner's grants beside its rows, sorted and once** (calef's ruling
/// "a" on #2405, the grant record): the new grants merge with the kept ones in line order, a
/// grant for no new row or with a misspelled zone refuses the install whole, and installing the
/// same digest again changes nothing.
#[test]
fn an_install_writes_its_grants_in_order() {
    let (a, b) = ([0xaau8; 32], [0x11u8; 32]);
    let text = |d: &Digest| String::from_utf8(measured_boot::digest_text(d).to_vec()).unwrap();
    let entry = |program, digest| Entry {
        program,
        version: "0.1.0",
        package: program,
        digest,
        manager: "jig",
    };
    let reach = [
        Grant {
            digest: a,
            what: Granted::Zone("nife.test"),
        },
        Grant {
            digest: a,
            what: Granted::Network,
        },
    ];
    let mut out = vec![0u8; 4096];
    let n = with_entries_granting(
        "",
        || core::iter::once(entry("reach", a)),
        |_| false,
        || reach.into_iter(),
        None,
        &mut out,
    )
    .unwrap();
    let first = core::str::from_utf8(&out[..n]).unwrap().to_string();
    let (ta, tb) = (text(&a), text(&b));
    assert!(
        first.ends_with(&format!("grant {ta} network\ngrant {ta} zone nife.test\n")),
        "network sorts before zone: {first}"
    );
    assert_eq!(network_granted(&first, &a), Ok(true));

    // A second package's grant sorts between by digest, and the first's lines are kept.
    let other = [Grant {
        digest: b,
        what: Granted::Zone(""),
    }];
    let n = with_entries_granting(
        &first,
        || core::iter::once(entry("other", b)),
        |_| false,
        || other.into_iter(),
        None,
        &mut out,
    )
    .unwrap();
    let second = core::str::from_utf8(&out[..n]).unwrap().to_string();
    let lines: vec::Vec<_> = second.lines().filter(|l| l.starts_with("grant ")).collect();
    assert_eq!(
        lines,
        [
            format!("grant {tb} zone ."),
            format!("grant {ta} network"),
            format!("grant {ta} zone nife.test"),
        ]
    );
    // The same bytes again: the same table.
    let n = with_entries_granting(
        &second,
        || core::iter::once(entry("reach", a)),
        |_| false,
        || reach.into_iter(),
        None,
        &mut out,
    )
    .unwrap();
    assert_eq!(core::str::from_utf8(&out[..n]).unwrap(), second);
}

/// **A grant the table has no row for is refused, and so is a zone name the policy's spelling
/// refuses**: the table keeps only what a live program's row bounds, under the name the policy
/// compares lowercased.
#[test]
fn a_grant_for_no_new_row_or_a_misspelled_zone_is_refused() {
    let (a, b) = ([0xaau8; 32], [0x11u8; 32]);
    let entry = |program, digest| Entry {
        program,
        version: "0.1.0",
        package: program,
        digest,
        manager: "jig",
    };
    let reach = [
        Grant {
            digest: a,
            what: Granted::Zone("nife.test"),
        },
        Grant {
            digest: a,
            what: Granted::Network,
        },
    ];
    let mut out = vec![0u8; 4096];
    let n = with_entries_granting(
        "",
        || core::iter::once(entry("reach", a)),
        |_| false,
        || reach.into_iter(),
        None,
        &mut out,
    )
    .unwrap();
    let first = core::str::from_utf8(&out[..n]).unwrap().to_string();

    let stray = [Grant {
        digest: b,
        what: Granted::Network,
    }];
    assert_eq!(
        with_entries_granting(
            &first,
            || core::iter::once(entry("reach", a)),
            |_| false,
            || stray.into_iter(),
            None,
            &mut out,
        )
        .map(|_| ()),
        Err(Error::Malformed.into()),
        "a grant for no new row"
    );
    let misspelled = [Grant {
        digest: a,
        what: Granted::Zone("Nife.test"),
    }];
    assert_eq!(
        with_entries_granting(
            "",
            || core::iter::once(entry("reach", a)),
            |_| false,
            || misspelled.into_iter(),
            None,
            &mut out,
        )
        .map(|_| ()),
        Err(Error::Malformed.into())
    );
}

/// **Two default pointers for one program are no single answer** (milestone 614 (two installed
/// versions of one program, each runnable)'s default pointer): `lookup` refuses the table as
/// malformed rather than pick one, while one pointer answers.
#[test]
fn two_default_pointers_for_one_program_make_the_table_malformed() {
    let (a, b) = ([1u8; 32], [2u8; 32]);
    let text = |d: &Digest| String::from_utf8(measured_boot::digest_text(d).to_vec()).unwrap();
    let (ta, tb) = (text(&a), text(&b));
    let rows = format!("{ta} uptime 0.1.0 uptime jig\n{tb} uptime 0.2.0 uptime jig\n");
    let one = format!("{rows}default uptime {tb}\n");
    assert_eq!(lookup(&one, "uptime").unwrap().unwrap().version, "0.2.0");
    let two = format!("{one}default uptime {ta}\n");
    assert_eq!(lookup(&two, "uptime"), Err(Error::Malformed));
}

/// **The grant record is byte-identical across the paths, and how the answer was given is a
/// line beside it, never inside it** (milestone 911 (scripted installs grant only what the
/// owner's approval file allows), Done-when 4): the same grants written for the console's yes
/// and for an approval's answer make the same `grant` lines and different `answered` lines,
/// and each line reads back.
#[test]
fn two_paths_write_one_record_and_their_answers_differ() {
    let a = [0x5au8; 32];
    let text = |d: &Digest| String::from_utf8(measured_boot::digest_text(d).to_vec()).unwrap();
    let ta = text(&a);
    let asks = [
        Grant {
            digest: a,
            what: Granted::Network,
        },
        Grant {
            digest: a,
            what: Granted::Zone("nife.test"),
        },
    ];
    let entry = || Entry {
        program: "reach",
        version: "0.1.0",
        package: "reach",
        digest: a,
        manager: "jig",
    };
    let mut out = vec![0u8; 4096];
    let n = with_entries_granting(
        "",
        || core::iter::once(entry()),
        |_| false,
        || asks.into_iter(),
        Some(&Answered::Console),
        &mut out,
    )
    .unwrap();
    let console = core::str::from_utf8(&out[..n]).unwrap().to_string();
    let n = with_entries_granting(
        "",
        || core::iter::once(entry()),
        |_| false,
        || asks.into_iter(),
        Some(&Answered::Approval("lab-runner", [0x77u8; 32])),
        &mut out,
    )
    .unwrap();
    let scripted = core::str::from_utf8(&out[..n]).unwrap().to_string();
    assert!(
        records_agree(&console, &scripted),
        "the records agree: {console} against {scripted}"
    );
    assert_ne!(console, scripted, "the answers do not");
    assert_eq!(
        answered(&console).unwrap(),
        Some(Answered::Console),
        "the console's line"
    );
    assert_eq!(
        answered(&scripted).unwrap(),
        Some(Answered::Approval("lab-runner", [0x77u8; 32])),
        "the approval's line, name and file digest"
    );
    // The `answered` object hands back the approval and its file digest, and the console's
    // answer hands back nothing.
    assert_eq!(Answered::Console.approval(), None);
    match answered(&scripted).unwrap() {
        Some(answer) => assert_eq!(answer.approval(), Some(("lab-runner", [0x77u8; 32]))),
        other => panic!("{other:?}"),
    }
    assert!(
        !records_agree(&scripted, ""),
        "a table with no grant lines is not the same record"
    );
    // The record itself, as text: the grant lines, sorted, once.
    let lines: vec::Vec<_> = record(&scripted).collect();
    assert_eq!(
        lines,
        [
            format!("grant {ta} network"),
            format!("grant {ta} zone nife.test")
        ]
    );
    // A vouch, which grants nothing, records no answer: the line is beside the record.
    let mut vouched = [0u8; 512];
    let n = with_entry("", &vouch("a.out", 3), false, &mut vouched).unwrap();
    assert_eq!(
        answered(core::str::from_utf8(&vouched[..n]).unwrap()),
        Ok(None)
    );
}

/// **The `answered` line is one per generation and held to its grammar** (milestone 911):
/// a second one is no single answer, and a line this reader does not know makes the table
/// unreadable, as every line kind's reader does.
#[test]
fn an_answered_line_is_read_strictly() {
    let mut out = [0u8; 512];
    let n = with_entry("", &row("uptime", "uptime", "0.1.0", 1), false, &mut out).unwrap();
    let rows = core::str::from_utf8(&out[..n]).unwrap().to_string();
    assert_eq!(answered(&rows).unwrap(), None, "no answer without one");
    let with_line = format!("{rows}answered console\n");
    assert_eq!(answered(&with_line).unwrap(), Some(Answered::Console));
    let twice = format!("{with_line}answered approval runner {}\n", "1".repeat(71));
    assert_eq!(answered(&twice), Err(Error::Malformed));
    // A blank line and a comment are not rows: the table still reads, answer and all.
    let spaced = format!("# a note\n\n{with_line}\n");
    assert_eq!(answered(&spaced).unwrap(), Some(Answered::Console));
    for bad in [
        "answered\n",
        "answered maybe\n",
        "answered console now\n",
        "answered approval runner\n",
        "answered approval console\n",
        "answered approval runner sha256:nope\n",
        "answered approval runner sha256:3 extra\n",
        "answered approval  sha256:3333333333333333333333333333333333333333333333333333333333333333\n",
    ] {
        assert_eq!(
            answered(&format!("{rows}{bad}")),
            Err(Error::Malformed),
            "{bad}"
        );
    }
    // The other readers skip a well-formed answered line and refuse a malformed one.
    assert!(lookup(&with_line, "uptime").unwrap().is_some());
    assert!(grants(&with_line).flatten().next().is_none());
    assert_eq!(
        lookup(&format!("{rows}answered bogus\n"), "uptime"),
        Err(Error::Malformed)
    );
}

/// **The grants a package may hold beyond the network and zones** (milestone 911): a service,
/// a directory with its rights and the installer each spell one line, read back exactly, and
/// sort into the record by their words as the table's reader holds them.
#[test]
fn service_dir_and_installer_grants_round_trip() {
    let a = [0x33u8; 32];
    let text = |d: &Digest| String::from_utf8(measured_boot::digest_text(d).to_vec()).unwrap();
    let ta = text(&a);
    let asks = [
        Grant {
            digest: a,
            what: Granted::Directory("monit/state", DirectoryRights::RW),
        },
        Grant {
            digest: a,
            what: Granted::Installer,
        },
        Grant {
            digest: a,
            what: Granted::Service,
        },
    ];
    let mut out = vec![0u8; 4096];
    let n = with_entries_granting(
        "",
        || {
            core::iter::once(Entry {
                program: "monit",
                version: "5.36",
                package: "monit",
                digest: a,
                manager: "jig",
            })
        },
        |_| false,
        || asks.into_iter(),
        Some(&Answered::Console),
        &mut out,
    )
    .unwrap();
    let table = core::str::from_utf8(&out[..n]).unwrap();
    let lines: vec::Vec<_> = record(table).collect();
    assert_eq!(
        lines,
        [
            format!("grant {ta} dir monit/state rw"),
            format!("grant {ta} installer"),
            format!("grant {ta} service"),
        ]
    );
    assert_eq!(answered(table).unwrap(), Some(Answered::Console));
    // `r` reads back as `r`, and a rights word this reader does not know is malformed.
    let mut narrow = [0u8; 4096];
    let n = with_entries_granting(
        "",
        || {
            core::iter::once(Entry {
                program: "monit",
                version: "5.36",
                package: "monit",
                digest: a,
                manager: "jig",
            })
        },
        |_| false,
        || {
            core::iter::once(Grant {
                digest: a,
                what: Granted::Directory("monit/state", DirectoryRights::R),
            })
        },
        None,
        &mut narrow,
    )
    .unwrap();
    let table = core::str::from_utf8(&narrow[..n]).unwrap();
    assert!(
        record(table).eq(core::iter::once(format!("grant {ta} dir monit/state r"))),
        "the rights spell as the file does: {table}"
    );
    // And `r` is a rights word, not merely a spelling this reader passes over.
    assert_eq!(
        grants(table).flatten().collect::<vec::Vec<_>>(),
        [Grant {
            digest: a,
            what: Granted::Directory("monit/state", DirectoryRights::R),
        }]
    );
    for bad in [
        format!("grant {ta} dir monit/state rwx\n"),
        format!("grant {ta} dir monit/state\n"),
        format!("grant {ta} dir\n"),
        format!("grant {ta} dir a.b r\n"),
        format!("grant {ta} service now\n"),
    ] {
        assert_eq!(
            network_granted(&format!("{table}{bad}"), &a),
            Err(Error::Malformed),
            "{bad}"
        );
    }
}

/// **The subset test never grants the intersection and names every extra grant** (milestone
/// 911, the rule the approval file holds installs to): an ask the allowance does not cover is
/// named, in order, and an `r` ask lies within a `rw` allowance but not the other way. The
/// root zone is spelled, never implied.
#[test]
fn the_subset_test_names_every_extra_grant_and_never_grants_the_intersection() {
    let allowed = [
        Granted::Network,
        Granted::Zone("nife.test"),
        Granted::Directory("state", DirectoryRights::RW),
    ];
    assert_eq!(extras(&allowed, &[]).count(), 0, "no asks, no extras");
    let asks = [
        Granted::Network,
        Granted::Zone("example.net"),
        Granted::Service,
    ];
    let named: vec::Vec<_> = extras(&allowed, &asks).copied().collect();
    assert_eq!(
        named,
        [Granted::Zone("example.net"), Granted::Service],
        "every extra grant, in the asks' order"
    );
    // The root zone is spelled out: one zone never covers another, and no zone covers the root.
    assert!(!covers(Granted::Zone("nife.test"), Granted::Zone("")));
    assert!(!covers(Granted::Zone(""), Granted::Zone("nife.test")));
    assert!(covers(Granted::Zone(""), Granted::Zone("")));
    // Rights subset, both ways.
    assert!(covers(
        Granted::Directory("state", DirectoryRights::RW),
        Granted::Directory("state", DirectoryRights::R)
    ));
    assert!(!covers(
        Granted::Directory("state", DirectoryRights::R),
        Granted::Directory("state", DirectoryRights::RW)
    ));
    assert!(!covers(
        Granted::Directory("other", DirectoryRights::RW),
        Granted::Directory("state", DirectoryRights::R)
    ));
}

/// **A directory grant's name is held to the row columns' rule** (milestone 911): the guard is
/// not decoration, so a name with a `#`, a control byte or nothing is malformed, and one the
/// columns can hold is a grant.
#[test]
fn a_directory_grants_name_is_held_to_the_columns_rule() {
    let digest = format!("sha256:{}", "11".repeat(32));
    assert!(matches!(
        classify(&format!("grant {digest} dir state rw")),
        Line::Grant(Grant {
            what: Granted::Directory("state", DirectoryRights::RW),
            ..
        })
    ));
    for bad in ["", "#x", "a\u{7f}b", "state\u{0}"] {
        assert!(
            matches!(
                classify(&format!("grant {digest} dir {bad} rw")),
                Line::Malformed
            ),
            "{bad:?} must be malformed"
        );
    }
}
