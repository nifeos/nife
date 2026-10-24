//! **What a bare word at the prompt runs** (milestone 47 (navigation and naming), DECISIONS §229
//! (how a bare name at the prompt reaches an installed program), B2).
//!
//! A word without a `/` is a builtin, a program the image carries, or the live installed-programs
//! table's default for that name ([`resolve`]); a name that is both the image's and installed is
//! refused, naming both. `program@version` asks for one row, and a version set
//! ([`crate::jig_versions`]) selects among the live versions, with a [`Notice`] when it cannot.
//! The table is read on every line that needs it. See notes/shell/the-swish-crate.md, "What a bare
//! word runs".

//! # EXAMPLES
//!
//! ```
//! use swish::bare::{self, Bare};
//! let table = "sha256:0000000000000000000000000000000000000000000000000000000000000001 \
//!     greeting 0.1.0 greeting\ndefault greeting \
//!     sha256:0000000000000000000000000000000000000000000000000000000000000001\n";
//! let Bare::Installed { path, notice: None } =
//!     bare::resolve(b"greeting", false, Some(table), None) else { panic!() };
//! assert_eq!(path.as_bytes(), b"/packages/greeting/0.1.0/greeting");
//! assert!(matches!(bare::resolve(b"uptime", true, Some(table), None), Bare::Image));
//! ```
//!
//! # BUGS
//!
//! - Only a plain line runs an installed program by its bare name (notes/packages.md's BUGS).
//! - A table that cannot be read resolves nothing, so the collision check is then skipped.
//! - **The both-names refusal is proven on the host only**; no boot gate reaches it.
//!
//! Name: provisional, milestone 47's bare-name lane, 2026-09-26.

use crate::jig_versions;

/// The longest installed path this builds: `/packages/` and three fields of at most
/// `package_archive::NAME_LEN` (32) bytes each, with their slashes. A path is walked a component at
/// a time, and a component longer than the shell can name fails there, loudly.
pub const PATH_MAX: usize = 10 + 3 * 32 + 2;

/// An installed program's path, from the shell's root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Path {
    bytes: [u8; PATH_MAX],
    len: usize,
}

impl Path {
    /// The path's bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    fn of(package: &str, version: &str, program: &str) -> Option<Path> {
        let mut p = Path {
            bytes: [0; PATH_MAX],
            len: 0,
        };
        for part in ["/packages/", package, "/", version, "/", program] {
            let end = p.len + part.len();
            p.bytes
                .get_mut(p.len..end)?
                .copy_from_slice(part.as_bytes());
            p.len = end;
        }
        Some(p)
    }
}

/// What a bare word names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bare {
    /// A program the image carries, and no installed program has the name.
    Image,
    /// An installed program, at this path: the default, or a version a set or an explicit ask
    /// selected. `notice` is set when a version set asked for a version that is not live and the
    /// default ran instead.
    Installed {
        /// What runs.
        path: Path,
        /// Why the spawn line says so, when what runs is not what was asked for.
        notice: Option<Notice>,
    },
    /// Both, which is refused.
    Both(Path),
    /// Neither: the planner refuses it as a program this shell cannot run.
    Unknown,
}

/// **A version label copied out of a read**, so the answer outlives the table and the set. Longest
/// is `package_archive::NAME_LEN` (32) bytes, which is the longest version an install can record;
/// a set line may say more, and the notice truncates there, as the progenitor's own sentences do.
/// Provisional, like the notice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label {
    bytes: [u8; 32],
    len: usize,
}

impl Label {
    fn of(text: &str) -> Label {
        let mut label = Label {
            bytes: [0; 32],
            len: 0,
        };
        let take = text.len().min(label.bytes.len());
        label.bytes[..take].copy_from_slice(&text.as_bytes()[..take]);
        label.len = take;
        label
    }

    /// The label's bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// **Why the spawn line says both**: what ran is not what a version set specified (milestone 614,
/// ruling 4). Provisional; the wording [`write_divergence`] prints was ratified, this struct's
/// name was not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Notice {
    /// The version label of the row that runs.
    pub ran: Label,
    /// The version the set specified.
    pub asked: Label,
}

/// **The spawn-line divergence notice**, `uptime 0.2.0 (.jig-versions asks for 0.1.0)`: what
/// ran, then the file that asked for something else and what it asked for, so the reader knows
/// which file to fix. The file's name is [`jig_versions::FILE`], printed from the constant so the
/// two cannot drift.
///
/// Wording ratified 2026-10-07 by calef (§258 (names for two installed versions of one program)).
/// Refused `(repo specifies <v>)`, ruling 4's first spelling ("repo" assumes a repository and does
/// not name the file to fix), and the file's full path (too long on every spawn; the shell finds
/// the file by walking up from the working directory).
pub fn write_divergence(word: &[u8], notice: &Notice, out: &mut dyn FnMut(&[u8])) {
    out(b"  ");
    out(word);
    out(b" ");
    out(notice.ran.as_bytes());
    out(b" (");
    out(jig_versions::FILE.as_bytes());
    out(b" asks for ");
    out(notice.asked.as_bytes());
    out(b")\n");
}

/// **Resolve a bare word.** `image` is whether the image carries a program of that name, `table`
/// the live generation when the shell could read one, and `set` the nearest version set's text
/// when the caller read one. The bare-word answer is the generation's default
/// (`installed_programs::lookup`), refined by the set when it names the program; `program@version` is
/// an explicit ask and answers only that row.
pub fn resolve(word: &[u8], image: bool, table: Option<&str>, set: Option<&str>) -> Bare {
    // The explicit ask first: `@` is reserved for it, so this word is not the contested bare name
    // and the image's claim does not apply. Not found is [`Bare::Unknown`].
    if let Ok(ask) = core::str::from_utf8(word)
        && let Some((program, version)) = ask.split_once('@')
        && !program.is_empty()
        && !version.is_empty()
    {
        let picked = table
            .and_then(|t| {
                installed_programs::lookup_version(t, program, version)
                    .ok()
                    .flatten()
            })
            .and_then(|row| Path::of(row.package, row.version, row.program));
        return match picked {
            Some(path) => Bare::Installed { path, notice: None },
            None => Bare::Unknown,
        };
    }
    let name = core::str::from_utf8(word).ok();
    let default = name
        .zip(table)
        .and_then(|(name, table)| installed_programs::lookup(table, name).ok().flatten());
    let Some(default) = default else {
        return if image { Bare::Image } else { Bare::Unknown };
    };
    let Some(path) = Path::of(default.package, default.version, default.program) else {
        return if image { Bare::Image } else { Bare::Unknown };
    };
    // The name is contested whatever the set says: both holders are refused, naming the installed
    // path.
    if image {
        return Bare::Both(path);
    }
    // A set selects only among live versions: a cloned repository can ask, but cannot run
    // uninstalled bytes, so a version that is not live leaves the default and the notice.
    if let Some(name) = name
        && let Some(set) = set
        && let Some(asked) = jig_versions::entry(set, name)
    {
        let selected = table
            .and_then(|t| {
                installed_programs::lookup_version(t, name, asked)
                    .ok()
                    .flatten()
            })
            .and_then(|row| Path::of(row.package, row.version, row.program));
        return match selected {
            Some(path) => Bare::Installed { path, notice: None },
            None => Bare::Installed {
                path,
                notice: Some(Notice {
                    ran: Label::of(default.version),
                    asked: Label::of(asked),
                }),
            },
        };
    }
    Bare::Installed { path, notice: None }
}

/// **The refusal for a name that is both**, naming both, and the way to run each.
pub fn write_both(word: &[u8], path: &Path, out: &mut dyn FnMut(&[u8])) {
    out(b"  refused: ");
    out(word);
    out(b" is both a program the image carries and an installed one, at ");
    out(path.as_bytes());
    out(b"\n  run the installed one by that path; the image's is not reachable by this name\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::vec::Vec;

    /// One installed program with its pointer. Rows without a pointer answer nothing, and a
    /// pointer without a row is malformed, so a resolvable table carries both.
    const TABLE: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000001 \
        greeting 0.1.0 greeting\ndefault greeting \
        sha256:0000000000000000000000000000000000000000000000000000000000000001\n";
    /// Two versions of `uptime` live; the default says 0.2.0, although 0.1.0's row is listed
    /// first. `a.out` is the owner's vouch.
    const TABLE_WITH_ROWS: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000001 \
        greeting 0.1.0 greeting\n\
        sha256:0000000000000000000000000000000000000000000000000000000000000004 \
        uptime 0.1.0 uptime\n\
        sha256:0000000000000000000000000000000000000000000000000000000000000002 \
        uptime 0.2.0 uptime\n\
        sha256:0000000000000000000000000000000000000000000000000000000000000003 \
        a.out - owner\ndefault uptime \
        sha256:0000000000000000000000000000000000000000000000000000000000000002\n";

    /// Each of the four answers, and the one that must never happen: a vouch or a non-default
    /// version claiming the name.
    #[test]
    fn a_bare_word_has_one_meaning_or_is_refused() {
        let Bare::Installed { path, notice: None } = resolve(b"greeting", false, Some(TABLE), None)
        else {
            panic!("an installed name resolves")
        };
        assert_eq!(path.as_bytes(), b"/packages/greeting/0.1.0/greeting");
        let Bare::Both(p) = resolve(b"uptime", true, Some(TABLE_WITH_ROWS), None) else {
            panic!("an image name that is also installed is both")
        };
        assert_eq!(
            p.as_bytes(),
            b"/packages/uptime/0.2.0/uptime",
            "the default's version"
        );
        assert_eq!(resolve(b"wc", true, Some(TABLE), None), Bare::Image);
        assert_eq!(resolve(b"nope", false, Some(TABLE), None), Bare::Unknown);
        // The owner's vouch is found by digest only: its name reaches nothing.
        assert_eq!(
            resolve(b"a.out", false, Some(TABLE_WITH_ROWS), None),
            Bare::Unknown
        );
        // A name whose only rows are not the default's reaches nothing either.
        let older_only = "sha256:0000000000000000000000000000000000000000000000000000000000000004 \
            uptime 0.1.0 uptime\n";
        assert_eq!(
            resolve(b"uptime", false, Some(older_only), None),
            Bare::Unknown
        );
        // No table, no installed names, and the image's run unchecked.
        assert_eq!(resolve(b"uptime", true, None, None), Bare::Image);
    }

    /// **The version set selects among live versions, and diverges when it cannot** (ruling 4):
    /// a live version runs silently; an uninstalled one leaves the default under the caller with
    /// the notice; a program the set does not name keeps the plain default.
    #[test]
    fn a_version_set_selects_live_versions_and_notices_the_rest() {
        let set = "uptime 0.1.0\n";
        let Bare::Installed { path, notice: None } =
            resolve(b"uptime", false, Some(TABLE_WITH_ROWS), Some(set))
        else {
            panic!("a set naming a live version selects it")
        };
        assert_eq!(path.as_bytes(), b"/packages/uptime/0.1.0/uptime");

        let set = "uptime 9.9.9\n";
        let Bare::Installed {
            path,
            notice: Some(notice),
        } = resolve(b"uptime", false, Some(TABLE_WITH_ROWS), Some(set))
        else {
            panic!("an uninstalled ask leaves the default and says so")
        };
        assert_eq!(path.as_bytes(), b"/packages/uptime/0.2.0/uptime");
        assert_eq!(notice.ran.as_bytes(), b"0.2.0");
        assert_eq!(notice.asked.as_bytes(), b"9.9.9");
        let mut said = Vec::new();
        write_divergence(b"uptime", &notice, &mut |b| said.extend_from_slice(b));
        let said = std::string::String::from_utf8(said).unwrap();
        assert_eq!(said, "  uptime 0.2.0 (.jig-versions asks for 9.9.9)\n");

        // A set that does not name the program changes nothing.
        let set = "date 1.0.0\n";
        let Bare::Installed { path, notice: None } =
            resolve(b"uptime", false, Some(TABLE_WITH_ROWS), Some(set))
        else {
            panic!()
        };
        assert_eq!(path.as_bytes(), b"/packages/uptime/0.2.0/uptime");
        // And the image's claim on the name is not weakened by the set's.
        assert!(matches!(
            resolve(b"uptime", true, Some(TABLE_WITH_ROWS), Some(set)),
            Bare::Both(_)
        ));
    }

    /// **The explicit ask**: `program@version` answers that row and nothing else, and the image's
    /// claim on the bare name does not reach a word that is not the bare name.
    #[test]
    fn a_qualified_ask_selects_one_version_or_nothing() {
        let Bare::Installed { path, notice: None } =
            resolve(b"uptime@0.1.0", false, Some(TABLE_WITH_ROWS), None)
        else {
            panic!("a qualified ask selects its row")
        };
        assert_eq!(path.as_bytes(), b"/packages/uptime/0.1.0/uptime");
        // Not a `Both`, although the image carries the name: this word is not the contested name.
        let Bare::Installed { notice: None, .. } =
            resolve(b"uptime@0.1.0", true, Some(TABLE_WITH_ROWS), None)
        else {
            panic!()
        };
        // An uninstalled version, an empty half, no table: all refused.
        assert_eq!(
            resolve(b"uptime@9.9.9", false, Some(TABLE_WITH_ROWS), None),
            Bare::Unknown
        );
        assert_eq!(
            resolve(b"uptime@", false, Some(TABLE_WITH_ROWS), None),
            Bare::Unknown
        );
        assert_eq!(resolve(b"uptime@0.1.0", false, None, None), Bare::Unknown);
    }

    /// The pointer, not the file order, decides: the table above lists 0.1.0 first and the default
    /// says 0.2.0.
    #[test]
    fn the_pointer_answers_and_not_the_first_row() {
        let Bare::Installed { path, notice: None } =
            resolve(b"uptime", false, Some(TABLE_WITH_ROWS), None)
        else {
            panic!()
        };
        assert_eq!(path.as_bytes(), b"/packages/uptime/0.2.0/uptime");
    }

    /// **The longest installed path is exactly `PATH_MAX`**: three 32-byte fields fill it to the
    /// last byte, so a bound computed too small refuses a legal install and one computed too large
    /// is noticed here.
    #[test]
    fn the_longest_installed_path_fills_path_max_exactly() {
        let f = "a".repeat(32);
        let p = Path::of(&f, &f, &f).expect("three 32-byte fields fit");
        assert_eq!(p.as_bytes().len(), PATH_MAX);
        assert_eq!(PATH_MAX, 108);
        let too_long = "a".repeat(33);
        assert!(Path::of(&too_long, &f, &f).is_none());
    }

    #[test]
    fn the_refusal_names_both() {
        let Bare::Both(p) = resolve(b"uptime", true, Some(TABLE_WITH_ROWS), None) else {
            panic!()
        };
        let mut out = Vec::new();
        write_both(b"uptime", &p, &mut |b| out.extend_from_slice(b));
        let text = std::string::String::from_utf8(out).unwrap();
        assert!(text.contains("uptime is both"), "{text}");
        assert!(text.contains("/packages/uptime/0.2.0/uptime"), "{text}");
    }
}
