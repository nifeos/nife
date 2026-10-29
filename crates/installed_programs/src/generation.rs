//! **A generation, read and edited**: the rows and default pointers of an installed-programs table,
//! the lookups the progenitor and the shell make, and the edits that write the next generation.
//! The format is in notes/packages/installed-programs.md; this module is its code, split from the
//! crate root under §282 (when a file is split, the parent becomes an index).
//!
//! # Examples: install, remove, vouch
//!
//! ```
//! use installed_programs::{lookup, with_entry, without_package, Entry, NO_VERSION, OWNER};
//!
//! let digest = [7u8; 32];
//! let uptime = Entry { program: "uptime", version: "0.1.0", package: "uptime", digest, manager: "jig" };
//! let mut first = [0u8; 512];
//! let n = with_entry("", &uptime, false, &mut first).unwrap();
//! let generation_1 = core::str::from_utf8(&first[..n]).unwrap();
//! assert_eq!(lookup(generation_1, "uptime").unwrap().unwrap().version, "0.1.0");
//!
//! let mut second = [0u8; 512];
//! let n = without_package(
//!     generation_1,
//!     "uptime",
//!     None,
//!     &installed_programs::Answered::Console,
//!     &mut second,
//! )
//! .unwrap();
//! let generation_2 = core::str::from_utf8(&second[..n]).unwrap();
//! assert!(lookup(generation_2, "uptime").unwrap().is_none());
//! // Generation 1 is untouched, so rolling back is naming it again.
//! assert!(lookup(generation_1, "uptime").unwrap().is_some());
//!
//! // A vouch is a row with no version and no pointer: found by digest, never by name.
//! let vouch = Entry { program: "a.out", version: NO_VERSION, package: OWNER, digest, manager: OWNER };
//! let mut third = [0u8; 512];
//! let n = with_entry(generation_2, &vouch, false, &mut third).unwrap();
//! let generation_3 = core::str::from_utf8(&third[..n]).unwrap();
//! assert!(lookup(generation_3, "a.out").unwrap().is_none());
//! ```
//!
//! Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.

pub use measured_boot::Digest;

use super::answered::{Answered, parse as parse_answered};
use super::grants::{Grant, Granted, ROOT_ZONE, classify_grant, grant_before, grants};

/// Why a table or an edit was refused.
///
/// Name: provisional, minted 2026-09-24 (UTC) by milestone/198-rung-3a-consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// A line is neither a row nor a pointer line, so the table vouches for nothing.
    Malformed,
    /// A name is empty, or holds a space, a newline or a `#`, so it would not read back as itself.
    BadName,
    /// A removal was asked for a program, or a program at a version, the table does not have.
    NotInstalled,
    /// The output buffer is too small for the next generation.
    TooSmall,
    /// **Another package already provides a program of this name** (DECISIONS §229 (how a bare name
    /// at the prompt reaches an installed program), B2). Installing a second package's program
    /// under it would silently take the name from the first. The same package at another version
    /// is not this: both rows live side by side.
    Taken,
    /// **The image carries a program of this name** (DECISIONS §229, calef's ruling of 2026-09-27).
    /// A package cannot take a base program's name. Under §235 (the OS is built and updated from
    /// packages) a base program is updated by writing base packages into the inactive boot slot,
    /// never by `jig install`, so this refusal blocks no update. An owner's vouch claims no
    /// name and is never this.
    ///
    ///
    /// Name: provisional, milestone 47 (navigation and naming)'s bare-name lane, 2026-09-27.
    ImageName,
    /// [`without_package`] refused: the version asked for holds a program's default pointer and
    /// more than one other version of that program remains, so no ordering among live versions
    /// exists to pick a new default with. The caller names the candidates with [`versions_of`]. Nothing is written. The same
    /// word as `package_archive::CatalogMiss::SeveralVersions`, for the same condition.
    ///
    /// Refused `Ambiguous` (calef: "ambiguous"), `AmbiguousName` (the name is not ambiguous; the
    /// version is). Minted as `Ambiguous` by milestone 614's build lane, 2026-09-29.
    ///
    ///
    /// Name: ratified 2026-10-07 (calef, §258 (names for two installed versions of one program)).
    SeveralVersions,
}

/// One installed program: its name at the prompt, the version its upstream claims, the package it
/// came from, and its digest as it was verified at install. The **digest is the row's key** and the
/// other three are label columns (milestone 614, ruling 2): a rebuild that produces new bytes is a
/// second row even when the version string is one already live.
///
/// Name: provisional, minted 2026-09-24 (UTC) by milestone/198-rung-3a-consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry<'a> {
    /// The name a person types, and the name the spawner is asked for.
    pub program: &'a str,
    /// **The version the upstream developer claims**: a label, never an identity. An owner's vouch
    /// carries [`NO_VERSION`], because the owner vouches for bytes and claims no version.
    pub version: &'a str,
    /// The package it came from, by name. [`OWNER`] when the vouch is the package's.
    pub package: &'a str,
    /// **The program's own SHA-256**: the digest of the executable member, as the package's table
    /// of contents carries it (`package_archive::Package::member_digest`) and as installing
    /// verified it. Not the package file's digest, which §195 (a reviewed recipe vouches for a
    /// package)'s recipe vouches for and which installing checks first: the spawner is handed the
    /// executable's bytes, never the package's (DECISIONS §219 option D), so the digest it can
    /// compute is the member's. Changed 2026-09-26 by milestone 198 rung 3a's image lane, before
    /// anything wrote a table. Key of the row since milestone 614, ruling 2.
    pub digest: Digest,
    /// **Which package manager installed the row** (milestone 809, calef's 2026-10-06 ruling): the
    /// program that held the installer endpoint, or [`OWNER`] for the owner's console. A label like
    /// the version, never part of the key. `NO_MANAGER` for a row written before the column.
    pub manager: &'a str,
}

/// **A default pointer line**: the row the bare word for `program` runs ([`lookup`]). Read with
/// `defaults`, written by [`with_entries`] and [`without_package`]. The `default` line kind was
/// ratified 2026-10-07 by calef (§258 (names for two installed versions of one program)); this
/// struct's own name was not ruled on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pointer<'a> {
    /// The program whose bare word this pointer answers.
    pub program: &'a str,
    /// The digest of the row the pointer names. Naming a digest that is not a package row of the
    /// program makes the table unreadable: a pointer to nothing vouches for nothing.
    pub digest: Digest,
}

/// Every row in a generation, or [`Error::Malformed`] at the first line that is not one. Blank
/// lines, `#` comments and well-formed pointer lines are skipped; a malformed pointer line is a
/// malformed table. The pointer lines themselves are read with `defaults`.
///
/// Name: provisional, minted 2026-09-24 (UTC) by milestone/198-rung-3a-consumer.
pub fn entries(table: &str) -> impl Iterator<Item = Result<Entry<'_>, Error>> {
    table
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| match classify(line) {
            Line::Row {
                digest,
                program,
                version,
                package,
                manager,
            } => Some(Ok(Entry {
                program,
                version,
                package,
                digest,
                manager,
            })),
            // A well-formed pointer, grant or answered line is not a row; that it is well-formed
            // was the check.
            Line::Pointer { .. } | Line::Grant(_) | Line::Answered(_) => None,
            Line::Malformed => Some(Err(Error::Malformed)),
        })
}

/// Every default pointer line in a generation, or [`Error::Malformed`] at the first line that is
/// not a row, a comment, a blank, or a well-formed pointer line. The whole table is validated
/// either way, so a caller that reads only pointers still refuses a broken row.
pub(crate) fn defaults(table: &str) -> impl Iterator<Item = Result<Pointer<'_>, Error>> {
    table
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| match classify(line) {
            Line::Pointer { program, digest } => Some(Ok(Pointer { program, digest })),
            // A well-formed row, grant or answered line is not a pointer; that it is well-formed
            // was the check.
            Line::Row { .. } | Line::Grant(_) | Line::Answered(_) => None,
            Line::Malformed => Some(Err(Error::Malformed)),
        })
}

/// One parsed line of a generation. Internal; the two iterators above decide which kind they keep.
pub(super) enum Line<'a> {
    Row {
        digest: Digest,
        program: &'a str,
        version: &'a str,
        package: &'a str,
        manager: &'a str,
    },
    Pointer {
        program: &'a str,
        digest: Digest,
    },
    Grant(Grant<'a>),
    /// **How the generation's answer was given** (milestone 911): beside the grant record, one
    /// line per generation. The iterators skip it; the readers of the record do not.
    Answered(Answered<'a>),
    Malformed,
}

/// Classify one line. A row is `<digest> <program> <version> <package> <manager>`, or the four
/// words before the manager column existed (read as `NO_MANAGER`); a pointer line is
/// `default <program> <digest>`; a grant line is `grant ...`; an answered line is
/// `answered console`, or `answered approval <name> <digest>`; anything else is malformed. A
/// digest is its text form, `sha256:<64 hex>` (`measured_boot::parse_digest`), so an unlabeled or
/// unknown one makes the line malformed and the table unreadable. A row's first word is the key,
/// which is why `default`, `grant` and `answered` are words no digest can be: none has a label.
pub(super) fn classify(line: &str) -> Line<'_> {
    let mut words = line.split(' ');
    let Some(first) = words.next() else {
        return Line::Malformed;
    };
    if first == "grant" {
        return classify_grant(words);
    }
    if first == "answered" {
        return match parse_answered(words) {
            Ok(answered) => Line::Answered(answered),
            Err(_) => Line::Malformed,
        };
    }
    if first == "default" {
        let (Some(program), Some(hex), None) = (words.next(), words.next(), words.next()) else {
            return Line::Malformed;
        };
        let Some(digest) = measured_boot::parse_digest(hex)
            .ok()
            .filter(|_| good_name(program))
        else {
            return Line::Malformed;
        };
        return Line::Pointer { program, digest };
    }
    let (Some(program), Some(version), Some(package)) = (words.next(), words.next(), words.next())
    else {
        return Line::Malformed;
    };
    let manager = match (words.next(), words.next()) {
        (None, _) => NO_MANAGER,
        (Some(manager), None) if good_name(manager) => manager,
        _ => return Line::Malformed,
    };
    let Ok(digest) = measured_boot::parse_digest(first) else {
        return Line::Malformed;
    };
    if !good_name(program) || !good_name(version) || !good_name(package) {
        return Line::Malformed;
    }
    Line::Row {
        digest,
        program,
        version,
        package,
        manager,
    }
}

/// **The entry a bare name at the prompt runs** (DECISIONS §229 (how a bare name at the prompt
/// reaches an installed program), B2; milestone 614, rulings 2 and 3): the default pointer for
/// `program`, then the package row it names. An owner's vouch is never the answer, because it
/// claims no name and writes no pointer. Two pointer lines for one program, or a pointer naming a
/// digest that is not a package row of the program, make the table unreadable: the default is one
/// answer, and a pointer to nothing vouches for nothing.
///
/// The whole table is checked first, so a malformed line anywhere is a refusal even when the name
/// asked for is on a good line.
///
/// Name: provisional, minted 2026-09-24 (UTC) by milestone/198-rung-3a-consumer.
pub fn lookup<'a>(table: &'a str, program: &str) -> Result<Option<Entry<'a>>, Error> {
    let Some(digest) = default_of(table, program)? else {
        return Ok(None);
    };
    for entry in entries(table) {
        let entry = entry?;
        if entry.program == program && entry.package != OWNER && entry.digest == digest {
            return Ok(Some(entry));
        }
    }
    Err(Error::Malformed)
}

/// **The row these bytes belong to**, if the generation has one: what the progenitor asks when it
/// is handed an executable rather than a name (DECISIONS §219 option D). The whole table is
/// checked first, as in [`lookup`], so a malformed line anywhere vouches for nothing.
///
/// Two rows with one digest (one program installed under two names) answer with the **first**,
/// deterministically. That is harmless because the manifest an installed program is endowed with is
/// in its bytes (an ELF note, milestone 597 (a program carries its manifest in an ELF note)), not
/// in its row: one digest is one set of bytes and so one manifest, whichever name was installed.
///
/// Name: provisional, minted 2026-09-25 (UTC) by milestone/198-rung-3a-image.
pub fn lookup_digest<'a>(table: &'a str, digest: &Digest) -> Result<Option<Entry<'a>>, Error> {
    let mut found = None;
    for entry in entries(table) {
        let entry = entry?;
        if found.is_none() && entry.digest == *digest {
            found = Some(entry);
        }
    }
    Ok(found)
}

/// **The package row for this program at this version**, first in file order: what a version set's
/// `<program> <version>` line resolves to at activation. Owner's vouches are not versions and
/// never answer. A rebuild claiming a version already live is a second row, so the first is answered; the digest decides
/// what runs either way.
///
/// Name: provisional, minted 2026-09-29 (UTC) by milestone/614-every-version-live.
pub fn lookup_version<'a>(
    table: &'a str,
    program: &str,
    version: &str,
) -> Result<Option<Entry<'a>>, Error> {
    let mut found = None;
    for entry in entries(table) {
        let entry = entry?;
        if found.is_none()
            && entry.program == program
            && entry.version == version
            && entry.package != OWNER
        {
            found = Some(entry);
        }
    }
    Ok(found)
}

/// **The version labels live for `program`**, in file order, duplicates included: what a refusal
/// that may not pick among versions names instead. Owner's vouches claim no version and are not
/// listed.
///
/// Name: ratified 2026-10-07 (calef, §258 (names for two installed versions of one program)).
pub fn versions_of<'a>(
    table: &'a str,
    program: &str,
) -> impl Iterator<Item = Result<&'a str, Error>> {
    entries(table).filter_map(move |entry| match entry {
        Ok(e) if e.program == program && e.package != OWNER => Some(Ok(e.version)),
        Ok(_) => None,
        Err(e) => Some(Err(e)),
    })
}

/// The next generation: `table` with `entry` installed, its length written to `out`. **Upsert by
/// digest, then move the pointer**: the same program and digest replaces its row in place; the same
/// package at a new digest is a second row (two versions); another package's claim on the name is
/// [`Error::Taken`]; a vouch and a package's row sit side by side, since a vouch is found by digest
/// and never by name ([`lookup`]), and a later vouch of a name replaces the earlier one. A package
/// install also writes the program's default pointer (the bare word runs the newest install); a
/// vouch writes none. `image_carries` (the image has a program of that name, required so an
/// installer cannot forget to ask) makes a package's row [`Error::ImageName`] (§229), before
/// [`Error::Taken`]. Nothing is written on a refusal. [`with_entries`] takes several rows.
///
/// Name: provisional, minted 2026-09-24 (UTC) by milestone/198-rung-3a-consumer.
pub fn with_entry(
    table: &str,
    entry: &Entry<'_>,
    image_carries: bool,
    out: &mut [u8],
) -> Result<usize, Error> {
    with_entries(table, || core::iter::once(*entry), |_| image_carries, out).map_err(|r| r.error)
}

/// **Why [`with_entries`] refused, and over which row**: the error, the program it is about, and
/// for [`Error::Taken`] the package that already holds that program's name, so the refusal can
/// tell the owner what to remove first (calef, 2026-10-14 (UTC), milestone 919 (a package carries
/// several programs): "If two packages offer the same program name, refuse the second package.
/// Need to remove the first one before installing the second one.").
///
/// Name: provisional, milestone 919's lane, 2026-10-14 (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refused<'a> {
    /// What was wrong.
    pub error: Error,
    /// The program the refusal is about, when it is about one row.
    pub program: Option<&'a str>,
    /// For [`Error::Taken`], the package whose row holds the name.
    pub holder: Option<&'a str>,
}

impl From<Error> for Refused<'_> {
    fn from(error: Error) -> Self {
        Refused {
            error,
            program: None,
            holder: None,
        }
    }
}

/// [`with_entry`] for several rows at once: **every program of one package, in one generation**
/// (milestone 919 (a package carries several programs); §208 (installing a package is granting
/// it, and the installed programs are versioned): one install is one generation, and one rollback
/// undoes it). Every row is checked before a byte is written, so one refused program refuses the
/// whole package and `out` means nothing. `entries` is called once per pass and must yield the same
/// rows each time; `image_carries` answers, for one program's name, what [`with_entry`]'s flag
/// answers for its one.
///
/// Name: provisional, minted 2026-10-13 (UTC) by lane/multi-program-build.
pub fn with_entries<'a, I>(
    table: &'a str,
    entries: impl Fn() -> I,
    image_carries: impl Fn(&str) -> bool,
    out: &mut [u8],
) -> Result<usize, Refused<'a>>
where
    I: Iterator<Item = Entry<'a>>,
{
    // A plain install or vouch that records no grants records no answer either: the `answered`
    // line is beside the grant record, and there is no record to sit beside.
    with_entries_granting(table, entries, image_carries, core::iter::empty, None, out)
}

/// **[`with_entries`], and the owner's grants to the new rows** (milestone 892 (a std program at
/// the prompt holds the network and a resolver); calef's ruling "a" on #2405: the grant record).
/// `granted` yields each grant the owner approved, keyed on a new row's digest; the grants are
/// exactly a program's asks, all or nothing. Every existing grant is kept: a digest is one set of
/// bytes and so one note, so a digest installed again asks what it asked before, and its lines
/// merge with themselves. The lines come out sorted and each once, as [`grants`] reads them. A grant for no new row, or a zone
/// spelled other than the resolver spells it, is [`Error::Malformed`] and nothing is written.
///
/// `answered` is the line beside the record (milestone 911 (scripted installs grant only what the
/// owner's approval file allows)): how this answer was given, one line per generation, after the
/// grants.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub fn with_entries_granting<'a, I, G>(
    table: &'a str,
    entries: impl Fn() -> I,
    image_carries: impl Fn(&str) -> bool,
    granted: impl Fn() -> G,
    answered: Option<&Answered<'_>>,
    out: &mut [u8],
) -> Result<usize, Refused<'a>>
where
    I: Iterator<Item = Entry<'a>>,
    G: Iterator<Item = Grant<'a>>,
{
    for entry in entries() {
        refuse_entry(table, &entry, image_carries(entry.program))?;
    }
    for g in granted() {
        let named = entries().any(|e| e.digest == g.digest);
        let spelled = match g.what {
            Granted::Network | Granted::Service | Granted::Installer => true,
            Granted::Zone(z) => domain_name_resolution_contract::is_canonical_zone(z.as_bytes()),
            Granted::Directory(name, _) => good_name(name),
        };
        if !named || !spelled {
            return Err(Error::Malformed.into());
        }
    }
    let mut writer = Writer { out, at: 0 };
    write_rows(table, &entries, &mut writer)?;
    write_pointers(table, &entries, &mut writer)?;
    write_grants(table, |_| true, &granted, &mut writer)?;
    if let Some(answered) = answered {
        writer.answered(answered)?;
    }
    Ok(writer.at)
}

/// [`with_entries`]' rows: each existing row, or the entry that [`replaces`] it, then each entry
/// that replaced none. Out of line, as [`write_pointers`] is, so the passes' iterators do not share
/// one frame: inlined together they came to 4,624 bytes on riscv64, over the kernel's 4,096-byte
/// guard page (`script/stack-frame-check`).
#[inline(never)]
fn write_rows<'a, I>(
    table: &'a str,
    entries: &impl Fn() -> I,
    writer: &mut Writer<'_>,
) -> Result<(), Error>
where
    I: Iterator<Item = Entry<'a>>,
{
    for existing in self::entries(table) {
        let existing = existing?;
        match entries().find(|e| replaces(&existing, e)) {
            Some(entry) => writer.entry(&entry)?,
            None => writer.entry(&existing)?,
        }
    }
    for entry in entries() {
        if !self::entries(table).flatten().any(|e| replaces(&e, &entry)) {
            writer.entry(&entry)?;
        }
    }
    Ok(())
}

/// [`with_entries`]' pointers: each existing pointer, moved to a package entry of its program,
/// then one for each package entry whose program had none. A vouch writes no pointer.
#[inline(never)]
fn write_pointers<'a, I>(
    table: &'a str,
    entries: &impl Fn() -> I,
    writer: &mut Writer<'_>,
) -> Result<(), Error>
where
    I: Iterator<Item = Entry<'a>>,
{
    for existing in defaults(table) {
        let existing = existing?;
        match entries().find(|e| e.package != OWNER && e.program == existing.program) {
            Some(entry) => writer.pointer(entry.program, &entry.digest)?,
            None => writer.pointer(existing.program, &existing.digest)?,
        }
    }
    for entry in entries().filter(|e| e.package != OWNER) {
        if !defaults(table)
            .flatten()
            .any(|p| p.program == entry.program)
        {
            writer.pointer(entry.program, &entry.digest)?;
        }
    }
    Ok(())
}

/// The grant lines of `table` whose digest `keep` answers yes for, in their order, which is
/// already sorted ([`grants`] refuses a table that is not), merged with `granted`'s in one sorted
/// order. Each line is the least one greater than the last written, chosen by a scan of both, which
/// needs no buffer and no allocation; a table holds a few dozen grants at most, so the quadratic
/// scan costs nothing a person sees.
#[inline(never)]
fn write_grants<'a, G>(
    table: &'a str,
    keep: impl Fn(&Digest) -> bool,
    granted: &impl Fn() -> G,
    writer: &mut Writer<'_>,
) -> Result<(), Error>
where
    G: Iterator<Item = Grant<'a>>,
{
    for g in grants(table) {
        g?;
    }
    let candidates = || {
        grants(table)
            .flatten()
            .filter(|g| keep(&g.digest))
            .chain(granted())
    };
    let mut last: Option<Grant<'_>> = None;
    loop {
        let next = candidates()
            .filter(|c| last.is_none_or(|l| grant_before(&l, c)))
            .fold(None, |best: Option<Grant<'_>>, c| match best {
                Some(b) if !grant_before(&c, &b) => Some(b),
                _ => Some(c),
            });
        let Some(g) = next else {
            return Ok(());
        };
        writer.grant(&g)?;
        last = Some(g);
    }
}

/// Whether `entry` may join `table`: its names read back, a package's program is not the image's
/// (§229, calef's ruling of 2026-09-27), and no other package holds its name (§229 B2).
fn refuse_entry<'a>(
    table: &'a str,
    entry: &Entry<'a>,
    image_carries: bool,
) -> Result<(), Refused<'a>> {
    let about = |error, holder| Refused {
        error,
        program: Some(entry.program),
        holder,
    };
    let names = [entry.program, entry.version, entry.package, entry.manager];
    if !names.iter().all(|n| good_name(n)) {
        return Err(about(Error::BadName, None));
    }
    let vouch = entry.package == OWNER;
    if !vouch && image_carries {
        return Err(about(Error::ImageName, None));
    }
    for existing in entries(table) {
        let existing = existing?;
        if !vouch
            && existing.program == entry.program
            && existing.package != OWNER
            && existing.package != entry.package
        {
            return Err(about(Error::Taken, Some(existing.package)));
        }
    }
    Ok(())
}

/// Whether `entry` takes `existing`'s place rather than joining beside it: a package row of the
/// same program and digest (a relabel), or any earlier vouch of the same name.
fn replaces(existing: &Entry<'_>, entry: &Entry<'_>) -> bool {
    let vouch = entry.package == OWNER;
    (existing.package == OWNER) == vouch
        && existing.program == entry.program
        && (vouch || existing.digest == entry.digest)
}

/// The next generation: `table` without the package `package`, or without its version `version`.
/// **The verb's object is the package** (calef, 2026-10-14 (UTC), milestone 919 (a package
/// carries several programs): "jig remove should work on packages, not programs."): every row the
/// package wrote goes, whatever its programs are called, and the pointers with them, so no bare
/// word the package gave reaches anything.
///
/// With a version, only that version's rows go, and a pointer that named one moves to the
/// package's one live version; several live is [`Error::SeveralVersions`], since live versions
/// have no order to pick with ([`versions_of`] names them). A vouch is not a package and stays;
/// a rollback undoes a vouch, and brings a removed package back, since its bytes stay. Removing
/// what is not installed is [`Error::NotInstalled`]. Nothing is written on a refusal.
///
/// line beside the record (milestone 911 (scripted installs grant only what the owner's approval
/// file allows)): how this removal's answer was given, one line per generation, after the grants.
///
/// Name: provisional, milestone 919's lane, 2026-10-14 (UTC). `answered` is the removal's own
pub fn without_package(
    table: &str,
    package: &str,
    version: Option<&str>,
    answered: &Answered<'_>,
    out: &mut [u8],
) -> Result<usize, Error> {
    let gone = |e: &Entry<'_>| {
        e.package == package && e.package != OWNER && version.is_none_or(|v| e.version == v)
    };
    let mut removed = false;
    for existing in entries(table) {
        removed |= gone(&existing?);
    }
    if !removed {
        return Err(Error::NotInstalled);
    }
    // Refuse before writing a byte: every pointer that moves must have one place to go.
    // Two pointers for one program are no single answer, and are malformed here as in [`lookup`].
    for pointer in defaults(table) {
        let pointer = pointer?;
        default_of(table, pointer.program)?;
        moved_pointer(table, &pointer, &gone)?;
    }
    let mut writer = Writer { out, at: 0 };
    for existing in entries(table) {
        let existing = existing?;
        if !gone(&existing) {
            writer.entry(&existing)?;
        }
    }
    for pointer in defaults(table) {
        let pointer = pointer?;
        if let Some(digest) = moved_pointer(table, &pointer, &gone)? {
            writer.pointer(pointer.program, &digest)?;
        }
    }
    // A grant goes with the last row of its digest: the same bytes installed under a second name
    // keep the owner's grant.
    write_grants(
        table,
        |digest| {
            entries(table)
                .flatten()
                .any(|e| e.digest == *digest && !gone(&e))
        },
        &core::iter::empty,
        &mut writer,
    )?;
    writer.answered(answered)?;
    Ok(writer.at)
}

/// Where `pointer` points once the rows `gone` matches are removed: where it did while a
/// surviving package row of its program has that digest; else the first surviving row of the one
/// version left; `None` when no row of the program survives; [`Error::SeveralVersions`] when
/// several versions do. Survivors are counted by version, so two rows at one version string (a
/// rebuild) are one survivor.
fn moved_pointer(
    table: &str,
    pointer: &Pointer<'_>,
    gone: &impl Fn(&Entry<'_>) -> bool,
) -> Result<Option<Digest>, Error> {
    let mut survivor: Option<Entry<'_>> = None;
    let mut uniform = true;
    for existing in entries(table) {
        let existing = existing?;
        if existing.program != pointer.program || existing.package == OWNER || gone(&existing) {
            continue;
        }
        if existing.digest == pointer.digest {
            return Ok(Some(pointer.digest));
        }
        match survivor {
            Some(first) => uniform &= first.version == existing.version,
            None => survivor = Some(existing),
        }
    }
    if !uniform {
        return Err(Error::SeveralVersions);
    }
    Ok(survivor.map(|s| s.digest))
}

/// The program's default pointer's digest, as [`lookup`] reads it: `None` when the program has no
/// pointer, [`Error::Malformed`] on two. Internal because [`lookup`] is the ruled reader;
/// [`without_package`] checks it is one.
fn default_of(table: &str, program: &str) -> Result<Option<Digest>, Error> {
    let mut found = None;
    let mut count = 0usize;
    for pointer in defaults(table) {
        let pointer = pointer?;
        if pointer.program == program {
            found = Some(pointer.digest);
            count += 1;
        }
    }
    if count > 1 {
        return Err(Error::Malformed);
    }
    Ok(found)
}

/// The live generation's number, from the `current` file: decimal digits and an optional newline.
///
/// Name: provisional, minted 2026-09-24 (UTC) by milestone/198-rung-3a-consumer.
pub fn parse_current(text: &str) -> Option<u32> {
    let digits = text.strip_suffix('\n').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The directory the installed-programs table lives in, at the root of the file service. Provisional.
///
/// Name: provisional, minted 2026-09-25 (UTC) by milestone/198-rung-3a-image.
pub const DIRECTORY: &str = "activation";

/// The one-line file in [`DIRECTORY`] naming the live generation ([`parse_current`]).
///
/// Name: provisional, minted 2026-09-25 (UTC) by milestone/198-rung-3a-image.
pub const CURRENT: &str = "current";

/// **Where `current` is written before it is renamed over the real one** (milestone 198 rung 3a's
/// installer), so the flip is one `RENAME` and a reader meets the old line or the new one, never a
/// torn write. Provisional; a name no generation can have, because it is not all digits.
///
/// Name: provisional, minted 2026-09-25 (UTC) by milestone/198-installer.
pub const CURRENT_STAGED: &str = "current.next";

/// **The directory an installed program's bytes are placed under**, at the root of the file
/// service: `packages/<name>/<version>/<program>`, one directory per package version (milestone
/// 198 rung 3a's installer). A component per field rather than the stem, because a name at the
/// prompt is at most sixteen bytes and `uptime-0.1.0-aarch64` is twenty. The progenitor hashes
/// whatever bytes a person runs (DECISIONS §219 option D), so this is where they are kept, not
/// what vouches for them. Provisional.
///
/// Name: provisional, minted 2026-09-25 (UTC) by milestone/198-installer.
pub const PACKAGES: &str = "packages";

/// **What a row's package column says when the owner vouched for the bytes** (DECISIONS §221
/// (the boot prompt is the owner's console), ruling 1; §195 (a reviewed recipe vouches for a
/// package) clause 3). No package name can be this word by construction: the column is the
/// `name` field of a `name-version-architecture` stem, and the one package in this tree whose
/// name could say it does not exist. The row is found by digest like any other and undone by a
/// rollback. It never claims a bare name (§229, B2): [`lookup`] skips it, [`with_entry`] keeps it
/// beside a package's row of the same name rather than replacing either, and only a later vouch of
/// that name replaces it. Provisional, like the column.
///
/// Name: provisional, minted 2026-09-26 (UTC) by milestone/198-owner-console.
pub const OWNER: &str = "owner";

/// **What a row's manager column reads as when the row has none**: written before milestone 809
/// (the package client becomes a program) added the column (calef, 2026-10-06, quoted as he said it: "each
/// activation set row records which manager installed it"), by the shell's `package` builtin, which the same change
/// removed. Never written. A row's manager is otherwise `jig` or [`OWNER`]. Provisional.
pub(crate) const NO_MANAGER: &str = "-";

/// **Whether `requester` may remove or roll back a row `row_manager` installed** (milestone 809,
/// calef's ruling of 2026-10-06: "if the owner ever grants a second manager the endpoint, `jig`
/// refuses to remove or roll back a row it did not install"). A manager may edit its own rows, the
/// owner's ([`OWNER`]: the owner runs every manager it granted the endpoint, and a vouch is undone
/// by a rollback), and rows nothing recorded (`NO_MANAGER`). It may not edit another manager's.
/// The owner's console, which holds the spawn endpoint itself, may edit any row.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub fn can_edit(row_manager: &str, requester: &str) -> bool {
    requester == OWNER
        || row_manager == requester
        || row_manager == OWNER
        || row_manager == NO_MANAGER
}

/// **The first row that differs between two generations whose manager `requester` may not edit**
/// ([`can_edit`]), if any: what a rollback from `newer` to `older` would undo on another manager's
/// behalf. A row differs when no row with the same program and digest is in the other table. Both
/// tables are read whole first, so a malformed line in either is [`Error::Malformed`].
///
/// Name: provisional (milestone 809).
pub fn foreign_change<'a>(
    older: &'a str,
    newer: &'a str,
    requester: &str,
) -> Result<Option<Entry<'a>>, Error> {
    for table in [older, newer] {
        for entry in entries(table) {
            entry?;
        }
    }
    let present = |table: &str, e: &Entry<'_>| {
        entries(table)
            .flatten()
            .any(|o| o.program == e.program && o.digest == e.digest)
    };
    for (from, other) in [(older, newer), (newer, older)] {
        for entry in entries(from).flatten() {
            if !present(other, &entry) && !can_edit(entry.manager, requester) {
                return Ok(Some(entry));
            }
        }
    }
    Ok(None)
}

/// **What a vouch row's version column carries**: the owner vouches for bytes, and claims no
/// version string, so the column holds a word no version set selects and no install writes.
///
/// Name: ratified 2026-10-07 (calef, §258 (names for two installed versions of one program)).
pub const NO_VERSION: &str = "-";

/// The file name of generation `number` in [`DIRECTORY`]: its decimal digits, no padding.
///
/// Name: provisional, minted 2026-09-25 (UTC) by milestone/198-rung-3a-image.
pub fn generation_name(number: u32, out: &mut [u8; 10]) -> &str {
    let mut n = number;
    let mut at = out.len();
    loop {
        at -= 1;
        out[at] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    // Only ASCII digits were written, so this cannot fail; `unwrap_or` keeps the path panic-free.
    core::str::from_utf8(&out[at..]).unwrap_or("0")
}

/// Write the `current` file naming generation `number`. Returns the length written.
///
/// Name: provisional, minted 2026-09-24 (UTC) by milestone/198-rung-3a-consumer.
pub fn format_current(number: u32, out: &mut [u8]) -> Result<usize, Error> {
    let mut digits = [0u8; 10];
    let mut n = number;
    let mut len = 0;
    loop {
        digits[len] = b'0' + (n % 10) as u8;
        len += 1;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    let mut writer = Writer { out, at: 0 };
    for i in (0..len).rev() {
        writer.bytes(&[digits[i]])?;
    }
    writer.bytes(b"\n")?;
    Ok(writer.at)
}

/// Whether `name` is one word a line can hold: nonempty, no `#`, and nothing at or below a
/// space. The sibling modules' readers (`answered`'s approval name, the approval document's
/// package names) hold their names to the same words, so one line kind cannot be mistaken for
/// another.
pub(super) fn good_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('#') && name.bytes().all(|b| b > b' ' && b != 0x7f)
}

struct Writer<'o> {
    out: &'o mut [u8],
    at: usize,
}

impl Writer<'_> {
    fn bytes(&mut self, b: &[u8]) -> Result<(), Error> {
        let end = self.at + b.len();
        self.out
            .get_mut(self.at..end)
            .ok_or(Error::TooSmall)?
            .copy_from_slice(b);
        self.at = end;
        Ok(())
    }

    fn entry(&mut self, e: &Entry<'_>) -> Result<(), Error> {
        self.bytes(&measured_boot::digest_text(&e.digest))?;
        self.bytes(b" ")?;
        self.bytes(e.program.as_bytes())?;
        self.bytes(b" ")?;
        self.bytes(e.version.as_bytes())?;
        self.bytes(b" ")?;
        self.bytes(e.package.as_bytes())?;
        self.bytes(b" ")?;
        self.bytes(e.manager.as_bytes())?;
        self.bytes(b"\n")
    }

    fn grant(&mut self, g: &Grant<'_>) -> Result<(), Error> {
        self.bytes(b"grant ")?;
        self.bytes(&measured_boot::digest_text(&g.digest))?;
        match g.what {
            Granted::Network => self.bytes(b" network")?,
            Granted::Service => self.bytes(b" service")?,
            Granted::Installer => self.bytes(b" installer")?,
            Granted::Zone(zone) => {
                self.bytes(b" zone ")?;
                self.bytes(if zone.is_empty() { ROOT_ZONE } else { zone }.as_bytes())?;
            }
            Granted::Directory(name, rights) => {
                self.bytes(b" dir ")?;
                self.bytes(name.as_bytes())?;
                self.bytes(b" ")?;
                self.bytes(rights.spell().as_bytes())?;
            }
        }
        self.bytes(b"\n")
    }

    /// The generation's one `answered` line: how its answer was given (milestone 911), after the
    /// grants.
    fn answered(&mut self, a: &Answered<'_>) -> Result<(), Error> {
        let mut line = [0u8; 16 + 32 + measured_boot::DIGEST_TEXT_LEN];
        let n = a.write(&mut line)?;
        self.bytes(&line[..n])
    }

    fn pointer(&mut self, program: &str, digest: &Digest) -> Result<(), Error> {
        self.bytes(b"default ")?;
        self.bytes(program.as_bytes())?;
        self.bytes(b" ")?;
        self.bytes(&measured_boot::digest_text(digest))?;
        self.bytes(b"\n")
    }
}

#[cfg(test)]
#[path = "generation/tests.rs"]
mod tests;
