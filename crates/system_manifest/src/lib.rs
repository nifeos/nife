//! The system manifest: the file basalt writes beside the packages, listing each package of a base
//! by its digest. Milestone 976 (an OS update is a set of packages that lands through a slot), part
//! 6, which executes the ruling of 2026-11-01 (UTC).
//!
//! The file is plain text. The first line is [`VERSION_LINE`]. The second is the progenitor line,
//! `progenitor <stem>`, naming the package that carries the progenitor; there is exactly one, and
//! its stem is one of the package lines. Every later line is `<stem> sha256:<64 lowercase hex>`,
//! one package per line, sorted by stem, with no blank line and one newline ending the last, so one
//! content has exactly one encoding. A later column goes in behind a new version line. [`render`] is the writer (basalt's side), [`SystemManifest::parse`]
//! the reader (the device updater's and the progenitor's), and [`SystemManifest::check`] the
//! refusal both run against the packages they hold: it names the package.
//!
//! This is not the program manifest of milestone 597 (a program carries its manifest in an ELF
//! note), and it is not the kernel's `program_measurements` table, which is per program.
//!
//! # EXAMPLES
//!
//! ```
//! use system_manifest::{Refusal, SystemManifest, render};
//!
//! let digest = measured_boot::sha256(b"swish");
//! let text = render("swish-0.1.0-x86_64", &[("swish-0.1.0-x86_64", digest)]).unwrap();
//! let manifest = SystemManifest::parse(&text).unwrap();
//! assert_eq!(manifest.progenitor(), "swish-0.1.0-x86_64");
//! assert_eq!(manifest.digest_of("swish-0.1.0-x86_64"), Some(digest));
//! let other = measured_boot::sha256(b"not swish");
//! assert_eq!(
//!     manifest.check(&[("swish-0.1.0-x86_64", other)]),
//!     Err(Refusal::Mismatch { stem: "swish-0.1.0-x86_64" }),
//! );
//! ```
//!
//! # BUGS
//!
//! - **The progenitor line's spelling is provisional.** The ruling of 2026-11-01 (UTC) gives the
//!   manifest a progenitor line of its own; `progenitor <stem>` as line 2 is this lane's spelling,
//!   chosen to sit beside `version 1` and `<stem> sha256:<hex>`. The word `progenitor` is reserved:
//!   no package stem is the bare word. The format version stays 1, since no manifest has shipped.
//! - **A stem is held only to a character set and a length**, not to the `name-version-architecture`
//!   shape `package_index` checks. The manifest is read beside the packages, so a stem that no
//!   package carries is refused by [`SystemManifest::check`] and not by the parse.
//!
//! Name: provisional 2026-11-01 (UTC), milestone 976 part 6's lane, with every public item here.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use measured_boot::{DIGEST_TEXT_LEN, Digest, digest_text, parse_digest};
use package_archive::STEM_LEN;

/// The word that opens the progenitor line, and so the one stem no package may have.
const PROGENITOR_WORD: &str = "progenitor";

/// The first line of a version 1 file.
///
/// Name: provisional 2026-11-01 (UTC), milestone 976 part 6's lane.
pub const VERSION_LINE: &str = "version 1";

/// **A system manifest read from text**, every line already checked. Borrowed, so a reader holds the
/// file and no copy.
///
/// Name: provisional 2026-11-01 (UTC), milestone 976 part 6's lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SystemManifest<'a> {
    progenitor: &'a str,
    /// The package lines only.
    text: &'a str,
}

/// **One package the manifest lists**: its stem and the digest its whole `.nifepkg` file must hash to.
///
/// Name: provisional 2026-11-01 (UTC), milestone 976 part 6's lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry<'a> {
    /// `name-version-architecture`, the file name without `.nifepkg`.
    pub stem: &'a str,
    /// SHA-256 of the package file.
    pub digest: Digest,
}

/// Why a text is not a version 1 system manifest. `line` counts from 1; the version line is line 1.
///
/// Name: provisional 2026-11-01 (UTC), milestone 976 part 6's lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// The first line is not [`VERSION_LINE`], or the text is empty. A newer version is refused
    /// here, not read as though it were this one.
    Version,
    /// The text does not end with a newline, or has a blank line (an empty line is a line).
    Layout {
        /// The line, counted from 1.
        line: usize,
    },
    /// A line that is not exactly `<stem> <digest>`: a missing or extra field, or a carriage return.
    Shape {
        /// The line, counted from 1.
        line: usize,
    },
    /// The stem is empty, longer than a package can have, or has a character outside lowercase
    /// letters, digits, `.`, `-` and `_`.
    Stem {
        /// The line, counted from 1.
        line: usize,
    },
    /// The digest is not `sha256:` and 64 lowercase hex digits.
    Digest {
        /// The line, counted from 1.
        line: usize,
    },
    /// The stem does not sort strictly after the one before it: out of order, or listed twice.
    Order {
        /// The line, counted from 1.
        line: usize,
    },
    /// The file lists no package. An update of nothing is not an update.
    Empty,
    /// The file has no progenitor line. A manifest names its progenitor exactly once.
    NoProgenitor,
    /// A second progenitor line. A manifest names its progenitor exactly once.
    ManyProgenitors {
        /// The line of the second one, counted from 1.
        line: usize,
    },
    /// The one progenitor line is not line 2, straight after the version line.
    ProgenitorPlace {
        /// The line, counted from 1.
        line: usize,
    },
    /// The progenitor line names a stem that no package line carries.
    ProgenitorUnlisted,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Version => write!(f, "the first line is not `{VERSION_LINE}`"),
            ParseError::Layout { line } => {
                write!(f, "line {line}: a blank line, or no newline at the end")
            }
            ParseError::Shape { line } => write!(f, "line {line}: not `<stem> sha256:<hex>`"),
            ParseError::Stem { line } => write!(f, "line {line}: not a package stem"),
            ParseError::Digest { line } => {
                write!(
                    f,
                    "line {line}: the digest is not `sha256:` and 64 lowercase hex digits"
                )
            }
            ParseError::Order { line } => {
                write!(f, "line {line}: the stem is out of order or listed twice")
            }
            ParseError::Empty => write!(f, "the manifest lists no package"),
            ParseError::NoProgenitor => write!(f, "the manifest has no progenitor line"),
            ParseError::ManyProgenitors { line } => {
                write!(
                    f,
                    "line {line}: a second progenitor line; there is exactly one"
                )
            }
            ParseError::ProgenitorPlace { line } => {
                write!(f, "line {line}: the progenitor line is line 2")
            }
            ParseError::ProgenitorUnlisted => {
                write!(f, "the progenitor line names a package no line lists")
            }
        }
    }
}

/// Why a package set does not match a manifest. Each names the package, so a person reading the
/// refusal knows which file to look at.
///
/// Name: provisional 2026-11-01 (UTC), milestone 976 part 6's lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal<'a> {
    /// The package is held, and its digest is not the one the manifest lists.
    Mismatch {
        /// The package.
        stem: &'a str,
    },
    /// The manifest lists the package and none was held.
    Missing {
        /// The package.
        stem: &'a str,
    },
    /// A package was held that the manifest does not list. A slot is filled from the manifest
    /// alone, so an unlisted file is refused rather than carried along.
    Unlisted {
        /// The package.
        stem: &'a str,
    },
}

impl fmt::Display for Refusal<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Mismatch { stem } => {
                write!(
                    f,
                    "{stem}: its digest is not the one the system manifest lists"
                )
            }
            Refusal::Missing { stem } => {
                write!(f, "{stem}: the system manifest lists it and it is not here")
            }
            Refusal::Unlisted { stem } => {
                write!(f, "{stem}: the system manifest does not list it")
            }
        }
    }
}

/// Why [`render`] refused its input.
///
/// Name: provisional 2026-11-01 (UTC), milestone 976 part 6's lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    /// No packages.
    Empty,
    /// The stem is not one [`SystemManifest::parse`] would read back.
    Stem(String),
    /// The stem appears twice, even with the same digest: one package is one line.
    Duplicate(String),
    /// The progenitor's stem is not one of the packages.
    ProgenitorUnlisted(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::Empty => write!(f, "a system manifest lists at least one package"),
            RenderError::Stem(stem) => write!(f, "`{stem}` is not a package stem"),
            RenderError::Duplicate(stem) => write!(f, "`{stem}` is listed twice"),
            RenderError::ProgenitorUnlisted(stem) => {
                write!(f, "the progenitor `{stem}` is not among the packages")
            }
        }
    }
}

fn stem_ok(stem: &str) -> bool {
    !stem.is_empty()
        && stem != PROGENITOR_WORD
        && stem.len() <= STEM_LEN
        && stem.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'_')
        })
}

/// One package line, or why it is refused (`line` filled in by the caller).
fn read_line(text: &str, line: usize) -> Result<Entry<'_>, ParseError> {
    let (stem, digest) = text.split_once(' ').ok_or(ParseError::Shape { line })?;
    if digest.contains(' ') || digest.contains('\r') {
        return Err(ParseError::Shape { line });
    }
    if !stem_ok(stem) {
        return Err(ParseError::Stem { line });
    }
    // `parse_digest` also reads uppercase hex, which would give one content two encodings.
    let lowercase = digest.bytes().all(|b| !b.is_ascii_uppercase());
    match parse_digest(digest) {
        Ok(digest) if lowercase => Ok(Entry { stem, digest }),
        _ => Err(ParseError::Digest { line }),
    }
}

impl<'a> SystemManifest<'a> {
    /// **Read `text`, refusing it whole at the first thing wrong**, so a file that parses has
    /// exactly the encoding [`render`] writes.
    ///
    /// Name: provisional 2026-11-01 (UTC), milestone 976 part 6's lane.
    pub fn parse(text: &'a str) -> Result<SystemManifest<'a>, ParseError> {
        let body = text
            .strip_prefix(VERSION_LINE)
            .and_then(|rest| rest.strip_prefix('\n'))
            .ok_or(ParseError::Version)?;
        let mut previous: Option<&str> = None;
        let mut progenitor: Option<&str> = None;
        let mut packages_at: Option<&str> = None;
        let mut count = 0usize;
        let mut packages = 0usize;
        let mut rest = body;
        while !rest.is_empty() {
            let line = count + 2;
            let Some((one, after)) = rest.split_once('\n') else {
                return Err(ParseError::Layout { line });
            };
            if one.is_empty() {
                return Err(ParseError::Layout { line });
            }
            if let Some(named) = one
                .strip_prefix(PROGENITOR_WORD)
                .and_then(|named| named.strip_prefix(' '))
            {
                if progenitor.is_some() {
                    return Err(ParseError::ManyProgenitors { line });
                }
                if count != 0 {
                    return Err(ParseError::ProgenitorPlace { line });
                }
                if !stem_ok(named) {
                    return Err(ParseError::Stem { line });
                }
                progenitor = Some(named);
                packages_at = Some(after);
                count += 1;
                rest = after;
                continue;
            }
            let entry = read_line(one, line)?;
            if previous.is_some_and(|before| before >= entry.stem) {
                return Err(ParseError::Order { line });
            }
            previous = Some(entry.stem);
            count += 1;
            packages += 1;
            rest = after;
        }
        if packages == 0 {
            return Err(ParseError::Empty);
        }
        let (Some(progenitor), Some(text)) = (progenitor, packages_at) else {
            return Err(ParseError::NoProgenitor);
        };
        let manifest = SystemManifest { progenitor, text };
        if manifest.digest_of(progenitor).is_none() {
            return Err(ParseError::ProgenitorUnlisted);
        }
        Ok(manifest)
    }

    /// The stem of the package that carries the progenitor, one of [`SystemManifest::entries`].
    ///
    /// Name: provisional 2026-11-01 (UTC), milestone 976 part 6's lane.
    pub fn progenitor(&self) -> &'a str {
        self.progenitor
    }

    /// Every package, in file order, which is sorted by stem.
    ///
    /// Name: provisional 2026-11-01 (UTC), milestone 976 part 6's lane.
    pub fn entries(&self) -> impl Iterator<Item = Entry<'a>> + 'a {
        // `parse` read every line already, so none is refused here; the line number is unused.
        self.text.lines().filter_map(|line| read_line(line, 0).ok())
    }

    /// The digest the manifest lists for `stem`, if it lists it.
    ///
    /// Name: provisional 2026-11-01 (UTC), milestone 976 part 6's lane.
    pub fn digest_of(&self, stem: &str) -> Option<Digest> {
        self.entries().find(|e| e.stem == stem).map(|e| e.digest)
    }

    /// **Hold a set of packages to the manifest**: every listed package present at its listed
    /// digest, and nothing else. The progenitor is one of the listed packages, so a set without it
    /// is refused as missing it. The first failure in manifest order names its package; a held
    /// package the manifest omits is named after every listed one is satisfied.
    ///
    /// Name: provisional 2026-11-01 (UTC), milestone 976 part 6's lane.
    pub fn check<'b>(&self, held: &[(&'b str, Digest)]) -> Result<(), Refusal<'b>>
    where
        'a: 'b,
    {
        for entry in self.entries() {
            match held.iter().find(|(stem, _)| *stem == entry.stem) {
                None => return Err(Refusal::Missing { stem: entry.stem }),
                Some((stem, digest)) if *digest != entry.digest => {
                    return Err(Refusal::Mismatch { stem });
                }
                Some(_) => {}
            }
        }
        match held.iter().find(|(stem, _)| self.digest_of(stem).is_none()) {
            Some((stem, _)) => Err(Refusal::Unlisted { stem }),
            None => Ok(()),
        }
    }
}

/// **The file for a set of packages and the stem of the one that carries the progenitor**, sorted by
/// stem whatever order they come in, so the same set is always the same bytes.
///
/// Name: provisional 2026-11-01 (UTC), milestone 976 part 6's lane.
pub fn render(progenitor: &str, packages: &[(&str, Digest)]) -> Result<String, RenderError> {
    if packages.is_empty() {
        return Err(RenderError::Empty);
    }
    let mut sorted: Vec<&(&str, Digest)> = packages.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    let mut text = String::with_capacity(
        VERSION_LINE.len() + 1 + sorted.len() * (STEM_LEN + 1 + DIGEST_TEXT_LEN + 1),
    );
    if !stem_ok(progenitor) {
        return Err(RenderError::Stem(progenitor.into()));
    }
    if !sorted.iter().any(|(stem, _)| *stem == progenitor) {
        return Err(RenderError::ProgenitorUnlisted(progenitor.into()));
    }
    text.push_str(VERSION_LINE);
    text.push('\n');
    text.push_str(PROGENITOR_WORD);
    text.push(' ');
    text.push_str(progenitor);
    text.push('\n');
    let mut previous: Option<&str> = None;
    for (stem, digest) in sorted {
        if !stem_ok(stem) {
            return Err(RenderError::Stem((*stem).into()));
        }
        if previous == Some(*stem) {
            return Err(RenderError::Duplicate((*stem).into()));
        }
        previous = Some(stem);
        text.push_str(stem);
        text.push(' ');
        // ASCII by construction: the label, a colon and lowercase hex.
        text.push_str(core::str::from_utf8(&digest_text(digest)).unwrap_or_default());
        text.push('\n');
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use measured_boot::sha256;

    use super::*;

    const A: &str = "init-0.1.0-x86_64";
    const B: &str = "shell-0.1.0-x86_64";

    fn d(seed: &[u8]) -> Digest {
        sha256(seed)
    }

    fn hex(seed: &[u8]) -> String {
        String::from_utf8(digest_text(&d(seed)).to_vec()).unwrap()
    }

    fn file(lines: &[(&str, &[u8])]) -> String {
        let mut text = format!("version 1\nprogenitor {A}\n");
        for (stem, seed) in lines {
            text.push_str(&format!("{stem} {}\n", hex(seed)));
        }
        text
    }

    #[test]
    fn a_manifest_renders_sorted_and_reads_back() {
        let text = render(A, &[(B, d(b"b")), (A, d(b"a"))]).unwrap();
        assert_eq!(text, file(&[(A, b"a"), (B, b"b")]));
        let manifest = SystemManifest::parse(&text).unwrap();
        let entries: Vec<_> = manifest.entries().collect();
        assert_eq!(
            entries,
            [
                Entry {
                    stem: A,
                    digest: d(b"a")
                },
                Entry {
                    stem: B,
                    digest: d(b"b")
                }
            ]
        );
        assert_eq!(manifest.digest_of(B), Some(d(b"b")));
        assert_eq!(manifest.digest_of("nothing-0.1.0-x86_64"), None);
    }

    #[test]
    fn the_same_set_in_any_order_is_the_same_bytes() {
        let one = render(A, &[(A, d(b"a")), (B, d(b"b"))]).unwrap();
        let two = render(A, &[(B, d(b"b")), (A, d(b"a"))]).unwrap();
        assert_eq!(one, two);
    }

    #[test]
    fn the_writer_refuses_what_the_reader_would() {
        assert_eq!(render(A, &[]), Err(RenderError::Empty));
        assert_eq!(
            render("Init", &[("Init", d(b"a"))]),
            Err(RenderError::Stem("Init".into()))
        );
        assert_eq!(
            render("", &[("", d(b"a"))]),
            Err(RenderError::Stem("".into()))
        );
        assert_eq!(
            render(A, &[(A, d(b"a")), (A, d(b"a"))]),
            Err(RenderError::Duplicate(A.into()))
        );
    }

    #[test]
    fn the_first_line_must_be_version_one() {
        let good = file(&[(A, b"a")]);
        assert!(SystemManifest::parse(&good).is_ok());
        for bad in [
            "",
            "version 2\n",
            "version 1",
            "Version 1\n",
            " version 1\n",
            "version 10\n",
            "version 1 \n",
        ] {
            let text = format!("{bad}{}", &good["version 1\n".len()..]);
            assert_eq!(
                SystemManifest::parse(&text),
                Err(ParseError::Version),
                "{bad:?}"
            );
        }
        assert_eq!(SystemManifest::parse(""), Err(ParseError::Version));
    }

    #[test]
    fn a_manifest_lists_at_least_one_package() {
        assert_eq!(SystemManifest::parse("version 1\n"), Err(ParseError::Empty));
    }

    #[test]
    fn layout_faults_name_their_line() {
        let a = hex(b"a");
        let b = hex(b"b");
        assert_eq!(
            SystemManifest::parse(&format!("version 1\nprogenitor {A}\n{A} {a}")),
            Err(ParseError::Layout { line: 3 })
        );
        assert_eq!(
            SystemManifest::parse(&format!("version 1\nprogenitor {A}\n{A} {a}\n\n{B} {b}\n")),
            Err(ParseError::Layout { line: 4 })
        );
        assert_eq!(
            SystemManifest::parse(&format!("version 1\n\n{A} {a}\n")),
            Err(ParseError::Layout { line: 2 })
        );
        assert_eq!(
            SystemManifest::parse(&format!("version 1\nprogenitor {A}\n{A} {a}\n\n")),
            Err(ParseError::Layout { line: 4 })
        );
    }

    #[test]
    fn shape_faults_name_their_line() {
        let a = hex(b"a");
        let cases = [
            format!("{A}\n"),
            format!("{A} {a} extra\n"),
            format!("{A}  {a}\n"),
            format!("{A} {a}\r\n"),
        ];
        for line in cases {
            assert_eq!(
                SystemManifest::parse(&format!("version 1\nprogenitor {A}\n{line}")),
                Err(ParseError::Shape { line: 3 }),
                "{line:?}"
            );
        }
        assert_eq!(
            SystemManifest::parse(&format!("version 1\nprogenitor {A}\n{A} {a}\n{B}\n")),
            Err(ParseError::Shape { line: 4 })
        );
    }

    #[test]
    fn a_stem_is_held_to_its_character_set_and_length() {
        let a = hex(b"a");
        for stem in [
            "Init-0.1.0-x86_64",
            "init 0",
            "in\u{e9}-0.1.0-x",
            "init/0.1.0-x",
        ] {
            let text = format!("version 1\nprogenitor {A}\n{stem} {a}\n");
            let got = SystemManifest::parse(&text);
            assert!(
                matches!(
                    got,
                    Err(ParseError::Stem { line: 3 } | ParseError::Shape { line: 3 })
                ),
                "{stem:?} {got:?}"
            );
        }
        let long = "a".repeat(STEM_LEN);
        assert!(
            SystemManifest::parse(&format!("version 1\nprogenitor {long}\n{long} {a}\n")).is_ok()
        );
        let too_long = "a".repeat(STEM_LEN + 1);
        assert_eq!(
            SystemManifest::parse(&format!("version 1\nprogenitor {A}\n{too_long} {a}\n")),
            Err(ParseError::Stem { line: 3 })
        );
        assert_eq!(
            SystemManifest::parse(&format!("version 1\nprogenitor {A}\n {a}\n")),
            Err(ParseError::Stem { line: 3 })
        );
        assert!(
            SystemManifest::parse(&format!("version 1\nprogenitor ab.c-d_e0\nab.c-d_e0 {a}\n"))
                .is_ok()
        );
    }

    #[test]
    fn a_digest_is_sha256_and_lowercase() {
        let a = hex(b"a");
        for bad in [
            a.to_uppercase(),
            a.replace("sha256:", "SHA256:"),
            a.replace("sha256:", "sha512:"),
            a.replace("sha256:", ""),
            a[..a.len() - 1].to_string(),
            format!("{a}0"),
            "sha256:".to_string(),
        ] {
            let text = format!("version 1\nprogenitor {A}\n{A} {bad}\n");
            let got = SystemManifest::parse(&text);
            assert_eq!(got, Err(ParseError::Digest { line: 3 }), "{bad:?}");
        }
        // Mixed case: only the hex digits are upper.
        let mixed = format!("sha256:{}", a["sha256:".len()..].to_uppercase());
        assert_eq!(
            SystemManifest::parse(&format!("version 1\nprogenitor {A}\n{A} {mixed}\n")),
            Err(ParseError::Digest { line: 3 })
        );
    }

    #[test]
    fn stems_must_ascend_strictly() {
        let a = hex(b"a");
        let b = hex(b"b");
        assert_eq!(
            SystemManifest::parse(&format!("version 1\nprogenitor {A}\n{B} {b}\n{A} {a}\n")),
            Err(ParseError::Order { line: 4 })
        );
        assert_eq!(
            SystemManifest::parse(&format!("version 1\nprogenitor {A}\n{A} {a}\n{A} {a}\n")),
            Err(ParseError::Order { line: 4 })
        );
        assert_eq!(
            SystemManifest::parse(&format!("version 1\nprogenitor {A}\n{A} {a}\n{A} {b}\n")),
            Err(ParseError::Order { line: 4 })
        );
        assert!(
            SystemManifest::parse(&format!("version 1\nprogenitor {A}\n{A} {a}\n{B} {b}\n"))
                .is_ok()
        );
    }

    #[test]
    fn a_mismatch_names_the_package() {
        let text = file(&[(A, b"a"), (B, b"b")]);
        let manifest = SystemManifest::parse(&text).unwrap();
        assert_eq!(manifest.check(&[(A, d(b"a")), (B, d(b"b"))]), Ok(()));
        assert_eq!(manifest.check(&[(B, d(b"b")), (A, d(b"a"))]), Ok(()));
        let refusal = manifest
            .check(&[(A, d(b"a")), (B, d(b"other"))])
            .unwrap_err();
        assert_eq!(refusal, Refusal::Mismatch { stem: B });
        assert!(refusal.to_string().starts_with(B), "{refusal}");
        let refusal = manifest.check(&[(A, d(b"x")), (B, d(b"b"))]).unwrap_err();
        assert_eq!(refusal, Refusal::Mismatch { stem: A });
    }

    #[test]
    fn a_missing_or_unlisted_package_is_named() {
        let text = file(&[(A, b"a"), (B, b"b")]);
        let manifest = SystemManifest::parse(&text).unwrap();
        let refusal = manifest.check(&[(A, d(b"a"))]).unwrap_err();
        assert_eq!(refusal, Refusal::Missing { stem: B });
        assert!(refusal.to_string().starts_with(B));
        let refusal = manifest
            .check(&[(A, d(b"a")), (B, d(b"b")), ("extra-0.1.0-x86_64", d(b"e"))])
            .unwrap_err();
        assert_eq!(
            refusal,
            Refusal::Unlisted {
                stem: "extra-0.1.0-x86_64"
            }
        );
        assert!(refusal.to_string().starts_with("extra-0.1.0-x86_64"));
        assert!(manifest.check(&[]).is_err());
    }

    #[test]
    fn every_refusal_reads_as_a_sentence() {
        let errors = [
            ParseError::Version,
            ParseError::Layout { line: 4 },
            ParseError::Shape { line: 5 },
            ParseError::Stem { line: 6 },
            ParseError::Digest { line: 7 },
            ParseError::Order { line: 8 },
            ParseError::Empty,
            ParseError::NoProgenitor,
            ParseError::ManyProgenitors { line: 9 },
            ParseError::ProgenitorPlace { line: 10 },
            ParseError::ProgenitorUnlisted,
        ];
        let words: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        for (error, word) in errors.iter().zip(&words) {
            assert!(!word.is_empty());
            if let Some(n) = match error {
                ParseError::Layout { line }
                | ParseError::Shape { line }
                | ParseError::Stem { line }
                | ParseError::Digest { line }
                | ParseError::Order { line }
                | ParseError::ManyProgenitors { line }
                | ParseError::ProgenitorPlace { line } => Some(line),
                _ => None,
            } {
                assert!(word.contains(&format!("line {n}")), "{word}");
            }
        }
        for i in 0..words.len() {
            for j in 0..i {
                assert_ne!(words[i], words[j]);
            }
        }
        assert_ne!(
            RenderError::Empty.to_string(),
            RenderError::Duplicate("x".into()).to_string()
        );
        assert!(RenderError::Stem("zz".into()).to_string().contains("zz"));
        assert!(
            RenderError::Duplicate("yy".into())
                .to_string()
                .contains("yy")
        );
    }

    fn with_progenitor(lines: &[&str], rest: &[(&str, &[u8])]) -> String {
        let mut text = String::from("version 1\n");
        for line in lines {
            text.push_str(&format!("progenitor {line}\n"));
        }
        for (stem, seed) in rest {
            text.push_str(&format!("{stem} {}\n", hex(seed)));
        }
        text
    }

    #[test]
    fn a_manifest_with_no_progenitor_line_is_refused_naming_the_cause() {
        let text = file(&[(A, b"a"), (B, b"b")]).replacen(&format!("progenitor {A}\n"), "", 1);
        let got = SystemManifest::parse(&text).unwrap_err();
        assert_eq!(got, ParseError::NoProgenitor);
        assert!(got.to_string().contains("progenitor"), "{got}");
    }

    #[test]
    fn a_manifest_with_two_progenitor_lines_is_refused_naming_the_cause() {
        let text = with_progenitor(&[A, B], &[(A, b"a"), (B, b"b")]);
        let got = SystemManifest::parse(&text).unwrap_err();
        assert!(got.to_string().contains("progenitor"), "{got}");
        let twice = with_progenitor(&[A, A], &[(A, b"a")]);
        let got = SystemManifest::parse(&twice).unwrap_err();
        assert!(got.to_string().contains("progenitor"), "{got}");
    }

    #[test]
    fn a_progenitor_stem_no_package_line_carries_is_refused_naming_the_cause() {
        let text = with_progenitor(&["absent-0.1.0-x86_64"], &[(A, b"a"), (B, b"b")]);
        let got = SystemManifest::parse(&text).unwrap_err();
        let said = got.to_string();
        assert!(said.contains("progenitor"), "{said}");
    }

    #[test]
    fn the_progenitor_line_names_its_cause_by_variant() {
        let two = with_progenitor(&[A, B], &[(A, b"a"), (B, b"b")]);
        assert_eq!(
            SystemManifest::parse(&two),
            Err(ParseError::ManyProgenitors { line: 3 })
        );
        let unknown = with_progenitor(&["absent-0.1.0-x86_64"], &[(A, b"a")]);
        assert_eq!(
            SystemManifest::parse(&unknown),
            Err(ParseError::ProgenitorUnlisted)
        );
        // One progenitor line, but after a package line.
        let late = format!(
            "{}progenitor {A}\n",
            file(&[(A, b"a")]).replace("progenitor init-0.1.0-x86_64\n", "")
        );
        assert_eq!(
            SystemManifest::parse(&late),
            Err(ParseError::ProgenitorPlace { line: 3 })
        );
        // The word is reserved, so a package cannot be called it.
        let reserved = format!(
            "version 1\nprogenitor progenitor\nprogenitor {}\n",
            hex(b"a")
        );
        assert!(SystemManifest::parse(&reserved).is_err());
    }

    #[test]
    fn the_writer_names_the_progenitor_and_refuses_an_unlisted_one() {
        let text = render(B, &[(A, d(b"a")), (B, d(b"b"))]).unwrap();
        assert_eq!(SystemManifest::parse(&text).unwrap().progenitor(), B);
        assert_eq!(
            render("absent-0.1.0-x86_64", &[(A, d(b"a"))]),
            Err(RenderError::ProgenitorUnlisted(
                "absent-0.1.0-x86_64".into()
            ))
        );
        assert!(
            RenderError::ProgenitorUnlisted("zz".into())
                .to_string()
                .contains("progenitor")
        );
    }

    #[test]
    fn a_manifest_with_one_listed_progenitor_line_is_read() {
        let text = with_progenitor(&[B], &[(A, b"a"), (B, b"b")]);
        let manifest = SystemManifest::parse(&text).unwrap();
        assert_eq!(manifest.digest_of(B), Some(d(b"b")));
        assert_eq!(manifest.entries().count(), 2);
        assert_eq!(manifest.progenitor(), B);
    }
}
