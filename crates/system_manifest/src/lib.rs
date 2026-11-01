//! The system manifest: the file basalt writes beside the packages, listing each package of a base
//! by its digest. Milestone 976 (an OS update is a set of packages that lands through a slot), part
//! 6, which executes the ruling of 2026-11-01 (UTC).
//!
//! The file is plain text. The first line is [`VERSION_LINE`]. Every later line is
//! `<stem> sha256:<64 lowercase hex>`, one package per line, sorted by stem, with no blank line and
//! one newline ending the last, so one content has exactly one encoding. A later column goes in
//! behind a new version line. [`render`] is the writer (basalt's side), [`SystemManifest::parse`]
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
//! let text = render(&[("swish-0.1.0-x86_64", digest)]).unwrap();
//! let manifest = SystemManifest::parse(&text).unwrap();
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
//! - **The file does not name the progenitor.** The ruling's line shape has no place for it, and
//!   the step it comes from says the manifest "names the progenitor". Until that is settled, the
//!   progenitor is the package that carries it, listed like every other, and a reader asks
//!   [`SystemManifest::digest_of`] for that package's stem. Provisional; see milestone 976's block.
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

/// The first line of a version 1 file.
pub const VERSION_LINE: &str = "version 1";

/// **A system manifest read from text**, every line already checked. Borrowed, so a reader holds the
/// file and no copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SystemManifest<'a> {
    text: &'a str,
}

/// **One package the manifest lists**: its stem and the digest its whole `.nifepkg` file must hash to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry<'a> {
    /// `name-version-architecture`, the file name without `.nifepkg`.
    pub stem: &'a str,
    /// SHA-256 of the package file.
    pub digest: Digest,
}

/// Why a text is not a version 1 system manifest. `line` counts from 1; the version line is line 1.
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
        }
    }
}

/// Why a package set does not match a manifest. Each names the package, so a person reading the
/// refusal knows which file to look at.
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    /// No packages.
    Empty,
    /// The stem is not one [`SystemManifest::parse`] would read back.
    Stem(String),
    /// The stem appears twice, even with the same digest: one package is one line.
    Duplicate(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::Empty => write!(f, "a system manifest lists at least one package"),
            RenderError::Stem(stem) => write!(f, "`{stem}` is not a package stem"),
            RenderError::Duplicate(stem) => write!(f, "`{stem}` is listed twice"),
        }
    }
}

fn stem_ok(stem: &str) -> bool {
    !stem.is_empty()
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
    pub fn parse(text: &'a str) -> Result<SystemManifest<'a>, ParseError> {
        let body = text
            .strip_prefix(VERSION_LINE)
            .and_then(|rest| rest.strip_prefix('\n'))
            .ok_or(ParseError::Version)?;
        let mut previous: Option<&str> = None;
        let mut count = 0usize;
        let mut rest = body;
        while !rest.is_empty() {
            let line = count + 2;
            let Some((one, after)) = rest.split_once('\n') else {
                return Err(ParseError::Layout { line });
            };
            if one.is_empty() {
                return Err(ParseError::Layout { line });
            }
            let entry = read_line(one, line)?;
            if previous.is_some_and(|before| before >= entry.stem) {
                return Err(ParseError::Order { line });
            }
            previous = Some(entry.stem);
            count += 1;
            rest = after;
        }
        if count == 0 {
            return Err(ParseError::Empty);
        }
        Ok(SystemManifest { text: body })
    }

    /// Every package, in file order, which is sorted by stem.
    pub fn entries(&self) -> impl Iterator<Item = Entry<'a>> + 'a {
        self.text
            .lines()
            .enumerate()
            .filter_map(|(index, line)| read_line(line, index + 2).ok())
    }

    /// The digest the manifest lists for `stem`, if it lists it.
    pub fn digest_of(&self, stem: &str) -> Option<Digest> {
        self.entries().find(|e| e.stem == stem).map(|e| e.digest)
    }

    /// **Hold a set of packages to the manifest**: every listed package present at its listed
    /// digest, and nothing else. The first failure in manifest order names its package; a held
    /// package the manifest omits is named after every listed one is satisfied.
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

/// **The file for a set of packages**, sorted by stem whatever order they come in, so the same set
/// is always the same bytes.
pub fn render(packages: &[(&str, Digest)]) -> Result<String, RenderError> {
    if packages.is_empty() {
        return Err(RenderError::Empty);
    }
    let mut sorted: Vec<&(&str, Digest)> = packages.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    let mut text = String::with_capacity(
        VERSION_LINE.len() + 1 + sorted.len() * (STEM_LEN + 1 + DIGEST_TEXT_LEN + 1),
    );
    text.push_str(VERSION_LINE);
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
        let mut text = String::from("version 1\n");
        for (stem, seed) in lines {
            text.push_str(&format!("{stem} {}\n", hex(seed)));
        }
        text
    }

    #[test]
    fn a_manifest_renders_sorted_and_reads_back() {
        let text = render(&[(B, d(b"b")), (A, d(b"a"))]).unwrap();
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
        let one = render(&[(A, d(b"a")), (B, d(b"b"))]).unwrap();
        let two = render(&[(B, d(b"b")), (A, d(b"a"))]).unwrap();
        assert_eq!(one, two);
    }

    #[test]
    fn the_writer_refuses_what_the_reader_would() {
        assert_eq!(render(&[]), Err(RenderError::Empty));
        assert_eq!(
            render(&[("Init", d(b"a"))]),
            Err(RenderError::Stem("Init".into()))
        );
        assert_eq!(render(&[("", d(b"a"))]), Err(RenderError::Stem("".into())));
        assert_eq!(
            render(&[(A, d(b"a")), (A, d(b"a"))]),
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
            SystemManifest::parse(&format!("version 1\n{A} {a}")),
            Err(ParseError::Layout { line: 2 })
        );
        assert_eq!(
            SystemManifest::parse(&format!("version 1\n{A} {a}\n\n{B} {b}\n")),
            Err(ParseError::Layout { line: 3 })
        );
        assert_eq!(
            SystemManifest::parse(&format!("version 1\n\n{A} {a}\n")),
            Err(ParseError::Layout { line: 2 })
        );
        assert_eq!(
            SystemManifest::parse(&format!("version 1\n{A} {a}\n\n")),
            Err(ParseError::Layout { line: 3 })
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
                SystemManifest::parse(&format!("version 1\n{line}")),
                Err(ParseError::Shape { line: 2 }),
                "{line:?}"
            );
        }
        assert_eq!(
            SystemManifest::parse(&format!("version 1\n{A} {a}\n{B}\n")),
            Err(ParseError::Shape { line: 3 })
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
            let text = format!("version 1\n{stem} {a}\n");
            let got = SystemManifest::parse(&text);
            assert!(
                matches!(
                    got,
                    Err(ParseError::Stem { line: 2 } | ParseError::Shape { line: 2 })
                ),
                "{stem:?} {got:?}"
            );
        }
        let long = "a".repeat(STEM_LEN);
        assert!(SystemManifest::parse(&format!("version 1\n{long} {a}\n")).is_ok());
        let too_long = "a".repeat(STEM_LEN + 1);
        assert_eq!(
            SystemManifest::parse(&format!("version 1\n{too_long} {a}\n")),
            Err(ParseError::Stem { line: 2 })
        );
        assert_eq!(
            SystemManifest::parse(&format!("version 1\n {a}\n")),
            Err(ParseError::Stem { line: 2 })
        );
        assert!(SystemManifest::parse(&format!("version 1\nab.c-d_e0 {a}\n")).is_ok());
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
            let text = format!("version 1\n{A} {bad}\n");
            let got = SystemManifest::parse(&text);
            assert_eq!(got, Err(ParseError::Digest { line: 2 }), "{bad:?}");
        }
        // Mixed case: only the hex digits are upper.
        let mixed = format!("sha256:{}", a["sha256:".len()..].to_uppercase());
        assert_eq!(
            SystemManifest::parse(&format!("version 1\n{A} {mixed}\n")),
            Err(ParseError::Digest { line: 2 })
        );
    }

    #[test]
    fn stems_must_ascend_strictly() {
        let a = hex(b"a");
        let b = hex(b"b");
        assert_eq!(
            SystemManifest::parse(&format!("version 1\n{B} {b}\n{A} {a}\n")),
            Err(ParseError::Order { line: 3 })
        );
        assert_eq!(
            SystemManifest::parse(&format!("version 1\n{A} {a}\n{A} {a}\n")),
            Err(ParseError::Order { line: 3 })
        );
        assert_eq!(
            SystemManifest::parse(&format!("version 1\n{A} {a}\n{A} {b}\n")),
            Err(ParseError::Order { line: 3 })
        );
        assert!(SystemManifest::parse(&format!("version 1\n{A} {a}\n{B} {b}\n")).is_ok());
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
        ];
        let words: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        for (error, word) in errors.iter().zip(&words) {
            assert!(!word.is_empty());
            if let Some(n) = match error {
                ParseError::Layout { line }
                | ParseError::Shape { line }
                | ParseError::Stem { line }
                | ParseError::Digest { line }
                | ParseError::Order { line } => Some(line),
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
}
