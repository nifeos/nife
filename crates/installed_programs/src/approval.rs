//! **The compiled approval**: what `jig` compiles an approval file to on the owner's console and
//! sends to the progenitor as an `Approve` activation request, what the progenitor stores beside
//! the installed-programs table's generations, never as rows in them (the ruling of 2026-10-12 on
//! #2031, (a)), and what a delegated holder's every request is checked against. One document, one
//! grammar, read strictly: it is the authority a script acts within (milestone 911 (scripted
//! installs grant only what the owner's approval file allows)).
//!
//! ```text
//! approval lab-runner sha256:<64 hex of the file>
//! package monit
//! allow service
//! allow dir monit/state rw
//! update true
//! remove false
//! package vaultwarden
//! allow zone example.org
//! update true
//! remove false
//! ```
//!
//! The header binds the compiled records to the file they were compiled from: an approval is the
//! file's digest at `jig approve` (calef's ruling (a) of 2026-10-12, on #2030). A `package`
//! section holds the exact grants its package may receive, one `allow` line each, sorted and once
//! like the table's grant lines, and the two policy answers the file must carry: whether an
//! update may be installed, and whether the package may be removed. A package no section bounds
//! receives nothing: the subset test fails closed.
//!
//! Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.

use super::grants::{DirectoryRights, Granted, ROOT_ZONE};
use super::{Digest, Error};

/// **The directory the approval store lives in**, under [`super::DIRECTORY`], beside the
/// generations and never inside them. An approval file is `approvals/<name>`, and a rollback
/// rewrites only `current`, so it changes no approval.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub const STORE: &str = "approvals";

/// **The longest name an approval or a package section carries**: the file system's component
/// limit (`nifefs::NAME_LEN`, 32), which the store file's own name is held to. A longer name is
/// refused at compile time rather than truncated into another approval.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub const NAME_MAXIMUM: usize = 32;

/// **Why a document was refused.** Provisional names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub enum Refusal {
    /// A line is none this grammar knows, or a field is out of range. The whole document is
    /// refused, as a table's one bad line refuses the table.
    Malformed,
    /// The header is missing, so the document names no approval and binds to no file.
    NoHeader,
    /// A package section says `update` or `remove` twice, or not at all: the two answers the
    /// file must carry.
    PolicyNotOnce,
    /// Two sections bound the same package, or two allow lines grant the same thing: no single
    /// answer, as two default pointers for one program are no single answer.
    Duplicate,
}

/// **One package's compiled records**, as the writer takes them: what the approval allows, and
/// the two policy answers. The allows are sorted, each once, and the packages are sorted by name;
/// the writer refuses anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub struct Package<'a> {
    /// The package the section bounds, by name.
    pub name: &'a str,
    /// The grants it may receive, sorted, each once.
    pub allows: &'a [Granted<'a>],
    /// Whether an update of it may be installed.
    pub update: bool,
    /// Whether it may be removed.
    pub remove: bool,
}

/// **One package section as the reader finds it**, with its raw lines: [`Section::allows`]
/// re-reads them, so a caller that holds the section holds the whole of its grant set without a
/// buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub struct Section<'a> {
    /// The package the section bounds, by name.
    pub name: &'a str,
    /// Whether an update of it may be installed.
    pub update: bool,
    /// Whether it may be removed.
    pub remove: bool,
    /// The section's `allow` lines, as one text slice.
    lines: &'a str,
}

impl Section<'_> {
    /// **The section's allow lines**, in their sorted order, or [`Refusal::Malformed`] at the
    /// first line that is not one. The section was checked at [`Document::parse`], so the error
    /// is unreachable short of a caller that kept a section past its document.
    ///
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn allows(&self) -> impl Iterator<Item = Result<Granted<'_>, Refusal>> {
        self.lines.lines().map(allow_word)
    }

    /// **Whether the section allows one ask** (milestone 911 (scripted installs grant only what
    /// the owner's approval file allows)): the subset test's one-ask form, which the delegated
    /// check runs per request, where no list is kept. A section that no longer parses allows
    /// nothing, which is the refusal, not a guess.
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn covers(&self, ask: Granted<'_>) -> Result<bool, Refusal> {
        let mut any = false;
        for allowed in self.allows() {
            let allowed = allowed?;
            if super::grants::covers(allowed, ask) {
                any = true;
            }
        }
        Ok(any)
    }
}

/// **A compiled approval, read strictly.** The text is the document; every lookup re-reads it,
/// so a caller that holds one holds the whole of it, and a malformed line anywhere makes every
/// lookup refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub struct Document<'a> {
    text: &'a str,
}

impl<'a> Document<'a> {
    /// The document `text` spells, checked whole: the header once, then zero or more package
    /// sections, each with its allow lines sorted and once and its two policy answers exactly
    /// once, and the packages sorted by name, each once.
    ///
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn parse(text: &'a str) -> Result<Self, Refusal> {
        header(text).ok_or(Refusal::NoHeader)?;
        let mut previous: Option<&str> = None;
        for section in Sections::new(text) {
            let (name, lines) = section?;
            if !name_fits(name) {
                return Err(Refusal::Malformed);
            }
            if previous.is_some_and(|p| p >= name) {
                return Err(Refusal::Duplicate);
            }
            previous = Some(name);
            checked_section(lines)?;
        }
        Ok(Document { text })
    }

    /// The approval's name, from the header.
    ///
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn name(&self) -> &'a str {
        header(self.text).map_or("", |(name, _)| name)
    }

    /// The file's digest the header binds the records to.
    ///
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn digest(&self) -> Digest {
        header(self.text).map_or([0; 32], |(_, digest)| digest)
    }

    /// **Every package section, in the document's order.**
    ///
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn sections(&self) -> impl Iterator<Item = Result<Section<'a>, Refusal>> {
        Sections::new(self.text).map(|read| {
            let (name, lines) = read?;
            Ok(Section {
                name,
                update: policy_of(lines, "update")?,
                remove: policy_of(lines, "remove")?,
                lines: allow_lines(lines),
            })
        })
    }

    /// **The section for `package`**, or `None` when the approval does not bound it: every
    /// delegated request is checked against its package's section, and a package no section
    /// bounds receives nothing.
    ///
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn entry(&self, package: &str) -> Result<Option<Section<'a>>, Refusal> {
        for section in Sections::new(self.text) {
            let (name, lines) = section?;
            if name == package {
                return Ok(Some(Section {
                    name,
                    update: policy_of(lines, "update")?,
                    remove: policy_of(lines, "remove")?,
                    lines: allow_lines(lines),
                }));
            }
        }
        Ok(None)
    }
}

/// Whether `name` is one word a document's names are held to: the same words the table's
/// rows are ([`super::generation::good_name`]), at [`NAME_MAXIMUM`] and under.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub fn name_fits(name: &str) -> bool {
    super::generation::good_name(name) && name.len() <= NAME_MAXIMUM
}

/// The header's words: the approval's name and the file's digest it binds.
fn header(text: &str) -> Option<(&str, Digest)> {
    let mut words = text.lines().next()?.split(' ');
    match (words.next(), words.next(), words.next(), words.next()) {
        (Some("approval"), Some(name), Some(hex), None) => {
            let digest = measured_boot::parse_digest(hex).ok()?;
            (name_fits(name)).then_some((name, digest))
        }
        _ => None,
    }
}

/// One package section at a time: the header line skipped, then each `package` line with the
/// lines under it, to the next `package` line or the end. Any line that is neither is the
/// caller's [`Refusal::Malformed`].
struct Sections<'a> {
    rest: &'a str,
    header_ahead: bool,
}

impl<'a> Sections<'a> {
    fn new(text: &'a str) -> Self {
        Sections {
            rest: text,
            header_ahead: true,
        }
    }
}

impl<'a> Iterator for Sections<'a> {
    type Item = Result<(&'a str, &'a str), Refusal>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (line, tail) = next_line(self.rest)?;
            self.rest = tail;
            if self.header_ahead {
                self.header_ahead = false;
                continue;
            }
            let Some(name) = line.strip_prefix("package ") else {
                return Some(Err(Refusal::Malformed));
            };
            let lines = section_lines(self.rest);
            self.rest = &self.rest[lines.len()..];
            return Some(Ok((name, lines)));
        }
    }
}

/// The text's first line and the rest, or `None` at the end.
fn next_line(text: &str) -> Option<(&str, &str)> {
    if text.is_empty() {
        return None;
    }
    match text.split_once('\n') {
        Some((line, rest)) => Some((line, rest)),
        None => Some((text, "")),
    }
}

/// The lines from here to the next `package` line or the end, as one text slice: what a
/// section's checks and its reader both walk.
fn section_lines(rest: &str) -> &str {
    let mut taken = rest;
    loop {
        match next_line(taken) {
            Some((line, tail)) if !line.starts_with("package ") => {
                taken = tail;
                if tail.is_empty() {
                    return rest;
                }
            }
            Some(_) => return &rest[..rest.len() - taken.len()],
            None => return rest,
        }
    }
}

/// The section's `allow` lines only, as one slice: the canonical section puts them first, and
/// this is the prefix up to its first policy line. What [`Section::allows`] walks.
fn allow_lines(section: &str) -> &str {
    let mut taken = section;
    loop {
        match next_line(taken) {
            Some((line, tail)) if line.starts_with("allow ") => taken = tail,
            Some(_) => return &section[..section.len() - taken.len()],
            None => return section,
        }
    }
}

/// The section's one answer for `update` or `remove`, exactly once, or the refusal.
fn policy_of(section: &str, which: &str) -> Result<bool, Refusal> {
    let mut found = None;
    for line in section.lines() {
        let mut words = line.split(' ');
        match (words.next(), words.next(), words.next()) {
            (Some("allow"), ..) => {}
            (Some(w), Some(flag), None) if w == "update" || w == "remove" => {
                let value = match flag {
                    "true" => true,
                    "false" => false,
                    _ => return Err(Refusal::Malformed),
                };
                if w == which {
                    if found.is_some() {
                        return Err(Refusal::PolicyNotOnce);
                    }
                    found = Some(value);
                }
            }
            _ => return Err(Refusal::Malformed),
        }
    }
    found.ok_or(Refusal::PolicyNotOnce)
}

/// The section's lines, checked: `allow` lines sorted and once, then `update` and `remove` once
/// each and nothing after them, and no line this grammar does not know.
fn checked_section(section: &str) -> Result<(), Refusal> {
    let mut previous_allow: Option<&str> = None;
    let mut policy_seen = false;
    for line in section.lines() {
        if line.starts_with("allow ") {
            if policy_seen {
                return Err(Refusal::Malformed);
            }
            if previous_allow.is_some_and(|p| p >= line) {
                return Err(Refusal::Duplicate);
            }
            previous_allow = Some(line);
            allow_word(line)?;
        } else {
            policy_seen = true;
            let mut words = line.split(' ');
            match (words.next(), words.next(), words.next()) {
                (Some("update" | "remove"), Some("true" | "false"), None) => {}
                _ => return Err(Refusal::Malformed),
            }
        }
    }
    policy_of(section, "update")?;
    policy_of(section, "remove")?;
    Ok(())
}

/// The [`Granted`] one `allow` line spells, or [`Refusal::Malformed`]: the same words the table's
/// grant lines spell, one kind at a time.
fn allow_word(line: &str) -> Result<Granted<'_>, Refusal> {
    let mut words = line.split(' ');
    match (words.next(), words.next(), words.next(), words.next()) {
        (Some("allow"), Some("network"), None, _) => Ok(Granted::Network),
        (Some("allow"), Some("service"), None, _) => Ok(Granted::Service),
        (Some("allow"), Some("installer"), None, _) => Ok(Granted::Installer),
        (Some("allow"), Some("zone"), Some(zone), None) if zone == ROOT_ZONE => {
            Ok(Granted::Zone(""))
        }
        (Some("allow"), Some("zone"), Some(zone), None)
            if domain_name_resolution_contract::is_canonical_zone(zone.as_bytes()) =>
        {
            Ok(Granted::Zone(zone))
        }
        (Some("allow"), Some("dir"), Some(name), Some(rights))
            if words.next().is_none() && name_fits(name) =>
        {
            Ok(Granted::Directory(
                name,
                DirectoryRights::parse(rights).ok_or(Refusal::Malformed)?,
            ))
        }
        _ => Err(Refusal::Malformed),
    }
}

/// **The document's bytes**: the header, then each package's section. The caller sorts the
/// packages by name and each package's allows; a duplicate or an unsorted one is refused here
/// rather than written, so what the store holds is always canonical. `out` too small is
/// [`Error::TooSmall`], and a refusal writes nothing.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub fn write(
    name: &str,
    digest: Digest,
    packages: &[Package<'_>],
    out: &mut [u8],
) -> Result<usize, Error> {
    let mut at = 0;
    let mut push = |bytes: &[u8]| -> Result<(), Error> {
        out.get_mut(at..at + bytes.len())
            .ok_or(Error::TooSmall)?
            .copy_from_slice(bytes);
        at += bytes.len();
        Ok(())
    };
    if !name_fits(name) {
        return Err(Error::BadName);
    }
    push(b"approval ")?;
    push(name.as_bytes())?;
    push(b" ")?;
    let mut hex = [0u8; measured_boot::DIGEST_TEXT_LEN];
    hex.copy_from_slice(&measured_boot::digest_text(&digest));
    push(&hex)?;
    push(b"\n")?;
    let mut previous: Option<&str> = None;
    for package in packages {
        if !name_fits(package.name) {
            return Err(Error::BadName);
        }
        if previous.is_some_and(|p| p >= package.name) {
            return Err(Error::BadName);
        }
        previous = Some(package.name);
        push(b"package ")?;
        push(package.name.as_bytes())?;
        push(b"\n")?;
        let mut previous = [0u8; 320];
        let mut previous_len = 0usize;
        for allow in package.allows {
            let mut bytes = [0u8; 320];
            let n = allow_line(allow, &mut bytes)?;
            // The caller's allows are sorted; the writer refuses an unsorted one rather than
            // write a document its reader would refuse.
            if previous_len > 0 && previous[..previous_len] >= bytes[..n] {
                return Err(Error::BadName);
            }
            previous[..n].copy_from_slice(&bytes[..n]);
            previous_len = n;
            push(&bytes[..n])?;
        }
        push(if package.update {
            b"update true\n"
        } else {
            b"update false\n"
        })?;
        push(if package.remove {
            b"remove true\n"
        } else {
            b"remove false\n"
        })?;
    }
    Ok(at)
}

/// One `allow` line's bytes: the writer's half of [`allow_word`]'s grammar.
fn allow_line(what: &Granted<'_>, out: &mut [u8]) -> Result<usize, Error> {
    let mut at = 0;
    let mut push = |bytes: &[u8]| -> Result<(), Error> {
        out.get_mut(at..at + bytes.len())
            .ok_or(Error::TooSmall)?
            .copy_from_slice(bytes);
        at += bytes.len();
        Ok(())
    };
    push(b"allow ")?;
    match *what {
        Granted::Network => push(b"network")?,
        Granted::Service => push(b"service")?,
        Granted::Installer => push(b"installer")?,
        Granted::Zone(zone) => {
            push(b"zone ")?;
            push(if zone.is_empty() { ROOT_ZONE } else { zone }.as_bytes())?;
        }
        Granted::Directory(dir_name, rights) => {
            push(b"dir ")?;
            push(dir_name.as_bytes())?;
            push(b" ")?;
            push(rights.spell().as_bytes())?;
        }
    }
    push(b"\n")?;
    Ok(at)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::string::ToString;
    use std::vec::Vec;
    use std::{format, vec};

    use super::*;
    use crate::extras;

    /// The document the example file compiles to, as [`write`] spells it: the header binds the
    /// file's digest, each section bounds one package, and the two policy answers are there.
    fn example() -> (Vec<u8>, usize) {
        let monit = Package {
            name: "monit",
            allows: &[
                Granted::Directory("monit/state", DirectoryRights::RW),
                Granted::Service,
            ],
            update: true,
            remove: false,
        };
        let vaultwarden = Package {
            name: "vaultwarden",
            allows: &[Granted::Zone("example.org")],
            update: true,
            remove: false,
        };
        let mut out = vec![0u8; 4096];
        let n = write("lab-runner", [0x11u8; 32], &[monit, vaultwarden], &mut out).unwrap();
        (out, n)
    }

    /// **The document the writer spells, the reader reads back whole**: the name, the digest,
    /// each package's allows in order, and the two policy answers per package.
    #[test]
    fn a_document_round_trips_through_write_and_parse() {
        let (out, n) = example();
        let text = core::str::from_utf8(&out[..n]).unwrap();
        let doc = Document::parse(text).unwrap();
        assert_eq!(doc.name(), "lab-runner");
        assert_eq!(doc.digest(), [0x11u8; 32]);
        let sections: Vec<_> = doc.sections().map(|s| s.unwrap()).collect();
        assert_eq!(sections.len(), 2);
        assert_eq!(
            sections[0].allows().map(|a| a.unwrap()).collect::<Vec<_>>(),
            [
                Granted::Directory("monit/state", DirectoryRights::RW),
                Granted::Service
            ]
        );
        assert!(sections[0].update && !sections[0].remove);
        assert_eq!(sections[1].name, "vaultwarden");
        assert_eq!(
            sections[1].allows().map(|a| a.unwrap()).collect::<Vec<_>>(),
            [Granted::Zone("example.org")]
        );
        assert_eq!(doc.entry("monit").unwrap().unwrap().name, "monit");
        assert_eq!(doc.entry("no-such").unwrap(), None);
    }

    /// **The installer's, the network's and the root zone's allow lines read back** as the grants
    /// they spell: `allow installer` is the endpoint, `allow network` the stack, and
    /// `allow zone .` the root zone, spelled.
    #[test]
    fn the_installers_and_the_root_zones_allow_lines_read_back() {
        let digest = "1".repeat(64);
        let text = format!(
            "approval r sha256:{digest}\npackage m\nallow installer\nallow network\nallow zone .\nupdate true\nremove false\n"
        );
        let doc = Document::parse(&text).unwrap();
        let m = doc.entry("m").unwrap().unwrap();
        assert_eq!(
            m.allows().map(|a| a.unwrap()).collect::<Vec<_>>(),
            [Granted::Installer, Granted::Network, Granted::Zone("")]
        );
    }

    /// **One ask at a time is what a section covers**, the delegated check's per-request test: a
    /// directory ask within the section's rights is covered and one wider is not, and an ask the
    /// section does not name at all is not.
    #[test]
    fn a_section_covers_one_ask_at_a_time() {
        let (out, n) = example();
        let text = core::str::from_utf8(&out[..n]).unwrap();
        let doc = Document::parse(text).unwrap();
        let monit = doc.entry("monit").unwrap().unwrap();
        assert_eq!(
            monit.covers(Granted::Directory("monit/state", DirectoryRights::R)),
            Ok(true)
        );
        assert_eq!(monit.covers(Granted::Service), Ok(true));
        assert_eq!(
            monit.covers(Granted::Directory("monit/state", DirectoryRights::RW)),
            Ok(true)
        );
        assert_eq!(monit.covers(Granted::Network), Ok(false));
        assert_eq!(
            monit.covers(Granted::Directory("monit/elsewhere", DirectoryRights::R)),
            Ok(false)
        );
    }

    /// **The document is read strictly, the way a table is**: a line this grammar does not know,
    /// a second `update`, a missing `remove`, a duplicate package or allow, an unsorted allow,
    /// or a missing header refuses the document whole.
    #[test]
    fn a_document_is_read_strictly() {
        let (out, n) = example();
        let text = core::str::from_utf8(&out[..n]).unwrap().to_string();
        let digest = "sha256:".to_string() + &"1".repeat(64);
        for bad in [
            format!("approval lab-runner {digest}\nbogus\n"),
            // A policy answer twice, and a missing one.
            format!(
                "approval lab-runner {digest}\npackage m\nupdate true\nupdate true\nremove false\n"
            ),
            format!("approval lab-runner {digest}\npackage m\nallow service\nupdate true\n"),
            // An unknown policy spelling, and a package named like no row's column.
            format!(
                "approval lab-runner {digest}\npackage m\nallow service\nupdate yes\nremove false\n"
            ),
            format!("approval lab-runner {digest}\npackage a b\nupdate true\nremove false\n"),
            // Duplicates, and an unsorted allow.
            format!(
                "approval lab-runner {digest}\npackage m\nupdate true\nremove false\npackage m\nupdate true\nremove false\n"
            ),
            format!(
                "approval lab-runner {digest}\npackage m\nallow service\nallow service\nupdate true\nremove false\n"
            ),
            format!(
                "approval lab-runner {digest}\npackage m\nallow service\nallow network\nupdate true\nremove false\n"
            ),
            // A package line after the policy answers of the same section, and an unknown allow.
            format!(
                "approval lab-runner {digest}\npackage m\nallow network\nupdate true\nremove false\nallow service\n"
            ),
            format!(
                "approval lab-runner {digest}\npackage m\nallow zones a\nupdate true\nremove false\n"
            ),
            // A directory line with a word after the rights, a name the row's column cannot hold
            // (a leading `#`), a zone the resolver would not spell, and an unknown policy word
            // with a flag.
            format!(
                "approval lab-runner {digest}\npackage m\nallow dir monit/state r extra\nupdate true\nremove false\n"
            ),
            format!(
                "approval lab-runner {digest}\npackage m\nallow dir #a r\nupdate true\nremove false\n"
            ),
            format!(
                "approval lab-runner {digest}\npackage m\nallow zone Nife.test\nupdate true\nremove false\n"
            ),
            format!(
                "approval lab-runner {digest}\npackage m\nbogus true\nupdate true\nremove false\n"
            ),
        ] {
            let why = Document::parse(&bad).unwrap_err();
            assert!(
                matches!(
                    why,
                    Refusal::Malformed
                        | Refusal::NoHeader
                        | Refusal::PolicyNotOnce
                        | Refusal::Duplicate
                ),
                "{bad}"
            );
        }
        // The example still parses, and a header alone bounds nothing (valid, and grants nothing).
        let _ = Document::parse(&text).unwrap();
        assert_eq!(
            Document::parse(&format!("approval runner {digest}\n"))
                .unwrap()
                .name(),
            "runner"
        );
    }

    /// **The compiled approval is what a delegation is checked against, and the subset test is
    /// its check** (milestone 911, item 6: the host-side check basalt's composition calls): a
    /// package's asks within its section pass, an ask the section does not bound or allow is
    /// named, and a package with no section receives nothing.
    #[test]
    fn the_document_checks_the_asks_fail_closed() {
        let (out, n) = example();
        let text = core::str::from_utf8(&out[..n]).unwrap();
        let doc = Document::parse(text).unwrap();
        let monit = doc.entry("monit").unwrap().unwrap();
        let allowed: Vec<_> = monit.allows().map(|a| a.unwrap()).collect();
        assert_eq!(extras(&allowed, &[Granted::Service]).count(), 0);
        assert_eq!(
            extras(&allowed, &[Granted::Network])
                .copied()
                .collect::<Vec<_>>(),
            [Granted::Network],
            "the extra grant, named"
        );
        // No section for the package: nothing is allowed, so every ask is extra.
        assert!(doc.entry("dialer").unwrap().is_none());
        assert_eq!(extras(&[], &[Granted::Network]).count(), 1);
    }

    /// **The subset test never grants the intersection** even where the section exists: these
    /// are the shapes the delegated check's refusals name.
    #[test]
    fn the_writer_refuses_what_the_reader_would() {
        // An unsorted package list, an unsorted allow list, and a name over the component limit.
        let m = Package {
            name: "m",
            allows: &[],
            update: true,
            remove: false,
        };
        let a = Package {
            name: "a",
            allows: &[],
            update: true,
            remove: false,
        };
        let mut out = [0u8; 512];
        assert_eq!(write("r", [0; 32], &[m, a], &mut out), Err(Error::BadName));
        let unsorted = Package {
            name: "m",
            allows: &[Granted::Service, Granted::Network],
            update: true,
            remove: false,
        };
        assert_eq!(
            write("r", [0; 32], &[unsorted], &mut out),
            Err(Error::BadName)
        );
        let long = "x".repeat(NAME_MAXIMUM + 1);
        let long = Package {
            name: &long,
            allows: &[],
            update: true,
            remove: false,
        };
        assert_eq!(write("r", [0; 32], &[long], &mut out), Err(Error::BadName));
        // And a buffer too small is refused, not truncated.
        let mut out = [0u8; 8];
        assert_eq!(write("r", [0; 32], &[m], &mut out), Err(Error::TooSmall));
    }

    /// **The store file's name is one component the file system can hold** (`nifefs::NAME_LEN`),
    /// and [`NAME_MAXIMUM`] is that bound minus nothing: the store directory carries the prefix.
    #[test]
    fn the_name_bound_is_the_file_systems_component_limit() {
        assert_eq!(NAME_MAXIMUM, 32);
        let longest = "a".repeat(NAME_MAXIMUM);
        let mut out = [0u8; 512];
        let ok = Package {
            name: &longest,
            allows: &[],
            update: true,
            remove: false,
        };
        let n = write("r", [0; 32], &[ok], &mut out).unwrap();
        assert!(Document::parse(core::str::from_utf8(&out[..n]).unwrap()).is_ok());
    }

    /// **Only `update` and `remove` are policy words** (milestone 911): a line naming anything
    /// else is malformed, so the guard is what keeps a stranger's word from reading as a policy.
    /// A section with the same shape under another word is refused, not silently taken.
    #[test]
    fn a_policy_word_that_is_neither_update_nor_remove_is_malformed() {
        assert_eq!(policy_of("update true\nremove false", "update"), Ok(true));
        assert_eq!(policy_of("update true\nremove false", "remove"), Ok(false));
        // The same two lines under a word this grammar does not know: both are refused.
        assert_eq!(
            policy_of("update true\ndelete true", "update"),
            Err(Refusal::Malformed)
        );
        assert_eq!(
            policy_of("removed true", "remove"),
            Err(Refusal::Malformed)
        );
    }
}
