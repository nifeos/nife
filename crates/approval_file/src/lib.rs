//! **The owner's approval file, parsed and compiled**: the TOML file that says which packages may
//! be installed and the exact grants each may receive (milestone 911 (scripted installs grant
//! only what the owner's approval file allows)). `jig` parses it on the owner's console and
//! compiles it to the canonical grant records ([`installed_programs::approval`]) that `jig
//! approve` sends and the progenitor stores; basalt's image composition checks its packages
//! against one the same way, host-side ([`File::check`]). [`File`] holds the grammar.
//!
//! # EXAMPLES
//!
//! ```
//! use approval_file::File;
//! use installed_programs::Granted;
//! let text = "approval = \"runner\"\n\n[package.servant]\nservice = true\nupdate = true\nremove = false\n";
//! let file = File::parse(text).unwrap();
//! assert_eq!(file.name(), "runner");
//! let servant = file.entry("servant").unwrap();
//! assert_eq!(servant.allows, [Granted::Service]);
//! assert!(servant.update && !servant.remove);
//! // A package no entry bounds may receive nothing.
//! assert_eq!(file.check("no-such", &[Granted::Network]).unwrap(), [Granted::Network]);
//! ```
//!
//! Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.

#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use installed_programs::{DirectoryRights, Granted, ROOT_ZONE};

/// The longest name the file's own grammar carries: the compiled record's bound
/// (`installed_programs::approval::NAME_MAXIMUM`), so a file that compiles past it is refused
/// here, at the line that says it.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub const NAME_MAXIMUM: usize = installed_programs::approval::NAME_MAXIMUM;

/// Why the file was refused, and where.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
#[derive(Debug, Clone, PartialEq, Eq)]
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub struct Error {
    /// The 1-based line the refusal is about.
    pub at: usize,
    /// What was wrong there.
    pub why: Why,
}

/// What was wrong at [`Error::at`].
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
#[derive(Debug, Clone, PartialEq, Eq)]
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub enum Why {
    /// A line is neither a header, a section, an entry, a blank nor a comment.
    NotTOML,
    /// A key this grammar does not know, or a top-level key other than `approval`.
    UnknownKey(String),
    /// A value of the wrong shape for its key, or a rights word other than `r` or `rw`.
    BadValue,
    /// A zone entry the resolver would not spell, the root zone spelled any other way than `.`
    /// included.
    ZoneNotCanonical,
    /// The same key twice in one section, or the same zone or directory twice in one entry.
    DuplicateKey,
    /// A package section says `update` or `remove` not at all.
    MissingPolicy(&'static str),
    /// A name is empty, holds a space, a `#` or a dot, or is longer than [`NAME_MAXIMUM`].
    BadName,
    /// No `approval = "..."` header before the first package section.
    NoHeader,
    /// The package sections are not in sorted order, the order the compiled records hold.
    SectionsOutOfOrder,
}

/// **One package's entry**: the grants the file allows it, and the two policy answers. The
/// allows are in the compiled records' order (directories by name, then the installer, the
/// network, the service, the zones by name), each once, whatever order the file's keys were in.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
#[derive(Debug, Clone, PartialEq, Eq)]
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub struct Section<'a> {
    /// The package the entry bounds, by name.
    pub name: &'a str,
    /// The exact grants it may receive.
    pub allows: Vec<Granted<'a>>,
    /// Whether an update of it may be installed.
    pub update: bool,
    /// Whether it may be removed.
    pub remove: bool,
}

/// **The parsed approval file.** [`File::parse`] reads it whole, so a refusal anywhere refuses
/// the file; every lookup after it is a slice lookup.
///
/// # The grammar
///
/// ```toml
/// approval = "lab-runner"
///
/// [package.monit]
/// zones   = ["nife.test"]        # each zone its own entry, `.` the root, spelled out
/// service = true
/// dirs    = { "monit/state" = "rw" }
/// network = false
/// installer = false
/// update  = true                 # whether an update may be installed; both answers required
/// remove  = false                # whether the package may be removed
/// ```
///
/// The reader holds the file to exactly this: a key this grammar does not know, a value of the
/// wrong shape, a zone the resolver would not spell, a rights word other than `r` or `rw`, a
/// package section twice, or a missing `update` or `remove` is a refusal naming the line. There
/// are no wildcards over packages, no "any zone", and no version ranges; which version installs
/// is §195 (a reviewed recipe vouches for a package)'s question, not this file's. Package
/// sections are listed in sorted order, the order the compiled records hold, and the section
/// name is one TOML table name, so no dot in it.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
#[derive(Debug, Clone, PartialEq, Eq)]
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub struct File<'a> {
    name: &'a str,
    sections: Vec<Section<'a>>,
}

impl<'a> File<'a> {
    /// The file `text` spells, or the refusal naming the line. Every section carries its two
    /// policy answers, its names are held to [`name_fits`], and its sections are sorted.
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn parse(text: &'a str) -> Result<Self, Error> {
        let mut name: Option<&'a str> = None;
        let mut sections: Vec<Section<'a>> = Vec::new();
        let mut building: Option<Building<'a>> = None;
        for (number, raw) in text.lines().enumerate() {
            let at = number + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(got) = section_name(line) {
                if name.is_none() {
                    return Err(Error {
                        at,
                        why: Why::NoHeader,
                    });
                }
                if !name_fits(got) {
                    return Err(Error {
                        at,
                        why: Why::BadName,
                    });
                }
                if let Some(built) = building.take() {
                    sections.push(built.finish(at)?);
                }
                if sections.last().is_some_and(|s| s.name >= got) {
                    return Err(Error {
                        at,
                        why: Why::SectionsOutOfOrder,
                    });
                }
                building = Some(Building::new(got));
                continue;
            }
            let Some((key, value)) = split_entry(line) else {
                return Err(Error {
                    at,
                    why: Why::NotTOML,
                });
            };
            let Some(built) = building.as_mut() else {
                // The one top-level entry is the header's, before any section.
                if key == "approval" {
                    if name.is_some() {
                        return Err(Error {
                            at,
                            why: Why::DuplicateKey,
                        });
                    }
                    name = Some(header_name(value).ok_or(Error {
                        at,
                        why: Why::BadName,
                    })?);
                    continue;
                }
                return Err(Error {
                    at,
                    why: Why::UnknownKey(key.to_string()),
                });
            };
            built.take(key, value, at)?;
        }
        if let Some(built) = building.take() {
            sections.push(built.finish(text.lines().count())?);
        }
        let name = name.ok_or(Error {
            at: 1,
            why: Why::NoHeader,
        })?;
        Ok(File { name, sections })
    }

    /// The approval's name, from the header.
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn name(&self) -> &'a str {
        self.name
    }

    /// **Every package entry, in the file's order** (sorted; [`File::parse`] held it to that).
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn sections(&self) -> &[Section<'a>] {
        &self.sections
    }

    /// **The entry for `package`**, or `None` when the file does not bound it: a package no
    /// entry bounds may receive nothing ([`installed_programs::extras`] on an empty allowance).
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn entry(&self, package: &str) -> Option<&Section<'a>> {
        self.sections.iter().find(|s| s.name == package)
    }

    /// **The check basalt's composition calls** (milestone 911, item 6): whether `asks` are
    /// within `package`'s entry, or every extra grant named. A package the file does not bound
    /// receives nothing, so every ask is extra; the subset test never grants the intersection.
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn check<'g>(
        &self,
        package: &str,
        asks: &'g [Granted<'g>],
    ) -> Result<Vec<Granted<'g>>, Error> {
        let allowed: &[Granted<'_>] = match self.entry(package) {
            Some(section) => &section.allows,
            None => &[],
        };
        Ok(installed_programs::extras(allowed, asks).copied().collect())
    }
}

/// **The file compiled to the canonical records** the progenitor stores and a delegated holder's
/// requests are checked against: the header binding the file's digest, one section per package.
/// The digest is the file's own bytes'; an approval binds to the file's digest at `jig approve`,
/// per the ruling of 2026-10-12 on #2030. `out` too small is
/// [`installed_programs::Error::TooSmall`], and a refusal writes nothing.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub fn compile(
    file: &File<'_>,
    file_digest: installed_programs::Digest,
    out: &mut [u8],
) -> Result<usize, installed_programs::Error> {
    let packages: Vec<installed_programs::approval::Package<'_>> = file
        .sections()
        .iter()
        .map(|s| installed_programs::approval::Package {
            name: s.name,
            allows: &s.allows,
            update: s.update,
            remove: s.remove,
        })
        .collect();
    installed_programs::approval::write(file.name(), file_digest, &packages, out)
}

/// One section under construction: its keys as they arrive, duplicates refused as they are
/// taken, the two policy answers checked at the section's end.
struct Building<'a> {
    name: &'a str,
    dirs: Vec<(&'a str, DirectoryRights)>,
    zones: Vec<&'a str>,
    service: bool,
    network: bool,
    installer: bool,
    update: Option<bool>,
    remove: Option<bool>,
    seen: Vec<&'a str>,
}

impl<'a> Building<'a> {
    fn new(name: &'a str) -> Self {
        Building {
            name,
            dirs: Vec::new(),
            zones: Vec::new(),
            service: false,
            network: false,
            installer: false,
            update: None,
            remove: None,
            seen: Vec::new(),
        }
    }

    /// Take one `key = value` line, or the refusal at `at`.
    fn take(&mut self, key: &'a str, value: &'a str, at: usize) -> Result<(), Error> {
        let err = |why| Error { at, why };
        if self.seen.contains(&key) {
            return Err(err(Why::DuplicateKey));
        }
        self.seen.push(key);
        match key {
            "zones" => {
                for zone in string_array(value).ok_or_else(|| err(Why::BadValue))? {
                    let zone = basic_string(zone).ok_or_else(|| err(Why::BadValue))?;
                    if !zone_is_canonical(zone) {
                        return Err(err(Why::ZoneNotCanonical));
                    }
                    let spelled = if zone == ROOT_ZONE { "" } else { zone };
                    if self.zones.contains(&spelled) {
                        return Err(err(Why::DuplicateKey));
                    }
                    self.zones.push(spelled);
                }
            }
            "service" => self.service = boolean(value).ok_or_else(|| err(Why::BadValue))?,
            "network" => self.network = boolean(value).ok_or_else(|| err(Why::BadValue))?,
            "installer" => self.installer = boolean(value).ok_or_else(|| err(Why::BadValue))?,
            "dirs" => {
                for (key, value) in inline_table(value).ok_or_else(|| err(Why::BadValue))? {
                    let name = basic_string(key).ok_or_else(|| err(Why::BadValue))?;
                    if !name_fits(name) {
                        return Err(err(Why::BadName));
                    }
                    let rights = DirectoryRights::parse(
                        basic_string(value).ok_or_else(|| err(Why::BadValue))?,
                    )
                    .ok_or_else(|| err(Why::BadValue))?;
                    if self.dirs.iter().any(|(n, _)| *n == name) {
                        return Err(err(Why::DuplicateKey));
                    }
                    self.dirs.push((name, rights));
                }
            }
            "update" => self.update = Some(boolean(value).ok_or_else(|| err(Why::BadValue))?),
            "remove" => self.remove = Some(boolean(value).ok_or_else(|| err(Why::BadValue))?),
            _ => return Err(err(Why::UnknownKey(key.to_string()))),
        }
        Ok(())
    }

    /// The section as its entry holds it, or the refusal for a missing policy answer.
    fn finish(mut self, at: usize) -> Result<Section<'a>, Error> {
        let err = |why| Error { at, why };
        // The allows in the compiled records' order: directories by name, then the installer,
        // the network, the service, the zones by name.
        self.dirs.sort_by(|(a, _), (b, _)| (*a).cmp(*b));
        self.zones.sort_unstable();
        let mut allows = Vec::new();
        for (name, rights) in self.dirs {
            allows.push(Granted::Directory(name, rights));
        }
        if self.installer {
            allows.push(Granted::Installer);
        }
        if self.network {
            allows.push(Granted::Network);
        }
        if self.service {
            allows.push(Granted::Service);
        }
        for zone in self.zones {
            allows.push(Granted::Zone(zone));
        }
        Ok(Section {
            name: self.name,
            allows,
            update: self
                .update
                .ok_or_else(|| err(Why::MissingPolicy("update")))?,
            remove: self
                .remove
                .ok_or_else(|| err(Why::MissingPolicy("remove")))?,
        })
    }
}

/// Whether `name` is one word a line and a file-system component can hold: the same test the
/// compiled records' reader holds its names to.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub fn name_fits(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('#')
        && !name.contains('.')
        && name.len() <= NAME_MAXIMUM
        && name.bytes().all(|b| b > b' ' && b != 0x7f)
}

/// The `[package.NAME]` header's name, or `None` for any other line.
fn section_name(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("[package.")?;
    let name = rest.strip_suffix(']')?.trim();
    (!name.is_empty()).then_some(name)
}

/// The header's `approval = "..."` value, unquoted.
fn header_name(value: &str) -> Option<&str> {
    let got = basic_string(value)?;
    name_fits(got).then_some(got)
}

/// The entry's key and value text, or `None` for a line with no `=`.
fn split_entry(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    let key = key.trim();
    (!key.is_empty()).then_some((key, value.trim()))
}

/// A `true` or `false`, with the line's remainder empty or a comment.
fn boolean(value: &str) -> Option<bool> {
    match value.strip_suffix_comment()? {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// A `"basic string"`, no escapes, with the line's remainder empty or a comment.
fn basic_string(value: &str) -> Option<&str> {
    let rest = value.strip_prefix('"')?;
    let end = rest.find('"')?;
    let got = &rest[..end];
    let after = rest[end + 1..].trim_start();
    (!got.contains('\\') && (after.is_empty() || after.starts_with('#'))).then_some(got)
}

/// A `["a", "b"]` array of basic strings, one line, or `None`.
fn string_array(value: &str) -> Option<Vec<&str>> {
    let rest = value.strip_prefix('[')?;
    let body = rest.strip_suffix(']')?.trim_end();
    let body = body.strip_suffix_comment()?;
    let mut items = Vec::new();
    if body.trim().is_empty() {
        return Some(items);
    }
    for item in body.split(',') {
        let item = item.trim();
        if item.is_empty() {
            return None;
        }
        items.push(item);
    }
    Some(items)
}

/// A `{ "k" = "v", ... }` inline table, one line, or `None`.
fn inline_table(value: &str) -> Option<Vec<(&str, &str)>> {
    let rest = value.strip_prefix('{')?;
    let body = rest.strip_suffix('}')?.trim_end();
    let body = body.strip_suffix_comment()?;
    let mut items = Vec::new();
    if body.trim().is_empty() {
        return Some(items);
    }
    for item in body.split(',') {
        let item = item.trim();
        let (key, value) = item.split_once('=')?;
        items.push((key.trim(), value.trim()));
    }
    Some(items)
}

/// The line's remainder after a value, empty or starting a comment.
trait TrimComment {
    fn strip_suffix_comment(&self) -> Option<&str>;
}

impl TrimComment for str {
    fn strip_suffix_comment(&self) -> Option<&str> {
        let trimmed = self.trim_end();
        // A trailing comment may follow the value; a `#` inside it cannot (basic strings do not
        // hold one, and booleans and brackets end before any).
        Some(match trimmed.find('#') {
            Some(at) => &trimmed[..at],
            None => trimmed,
        })
    }
}

/// The spelling a zone entry is held to: the resolver's canonical form, or the root zone as `.`.
fn zone_is_canonical(zone: &str) -> bool {
    zone == ROOT_ZONE || domain_name_resolution_contract::is_canonical_zone(zone.as_bytes())
}

#[cfg(test)]
mod tests;
