//! **The durable schedule store's shared names, and the one document `timetable` does not already
//! own** (milestone 152, DECISIONS §122 and §125).
//!
//! §122 (the on-disk, per-user schedule store) decided the schedule itself: one file per identity,
//! inside that identity's own subtree, in `timetable::parse`'s format. This crate answers what
//! tells boot-time re-derivation *which* identities have one, as §125 (what tells boot-time
//! re-derivation which identities have pending work) recorded: a **manifest** at a fixed,
//! well-known location, listing the identities with pending scheduled work, one name per line.
//!
//! # Why a manifest rather than a directory listing
//!
//! `READDIR` of the principal tree's root would be enumeration, which §123 (boot-time
//! re-derivation) refuses the re-deriver, and it would answer a different question: every
//! provisioned identity gets a subtree whether or not it ever registers a schedule. `OPEN`ing one
//! file at a name known at compile time ([`MANIFEST_FILE_NAME`]) is a targeted lookup, and what
//! the manifest's contents name is data, not a directory the reader walked.
//!
//! # What this crate is not
//!
//! It performs no IO and makes no syscalls. Both sides are `components/src/login.rs`: it writes an
//! identity's line when a schedule opens at a login and reads the manifest at start-up to
//! re-derive each session, and the parser and the renderer stay one function here. It does not
//! touch `timetable::Schedule` or `timetable::parse`, and depends on nothing.
//!

//! Name: ratified 2026-09-13 (calef), replacing `schedule_store`. A reader scanning `crates/` asks
//! *"What kind of schedule? Is this the thread scheduler?"*, and `crates/timetable` already avoids
//! `scheduler` (`kernel/src/sched.rs`). `schedule_store` is the plain noun §122 and §123 use in
//! running prose, but a crate name has no surrounding sentences. Refused `schedule_proto` (this is
//! names and a file format, not a request/reply vocabulary). Refused `schedule_file` (names one of
//! the two documents).

#![no_std]

/// **The name of one identity's own schedule file, inside that identity's own subtree**
/// (DECISIONS §117, §122). `<principal tree root>/<identity>/schedule`. The exact filename is a
/// detail §122 left to whoever built the write path; this lane picked it and says so here rather
/// than leaving it as a magic string duplicated at both call sites.
pub const SCHEDULE_FILE_NAME: &str = "schedule";

/// **The manifest's own name, directly under the principal tree's root**, a sibling of every
/// identity's own subtree rather than something nested inside one (DECISIONS §125): the manifest is
/// not any one identity's own record, so it does not belong inside any one identity's own subtree.
pub const MANIFEST_FILE_NAME: &str = "durable-sessions";

/// The most identities one manifest may list.
///
/// A ceiling rather than a limit anybody meets, matching `timetable::MAX_ENTRIES`'s own reasoning:
/// this crate allocates nothing, so a parsed manifest borrows the document's own bytes and the
/// table backing it is a fixed array. Eight matches `timetable::MAX_ENTRIES` because there is no
/// reason for the two ceilings to differ in this demonstration; a real deployment sizes both against
/// how many principals it actually has.
pub const MAX_IDENTITIES: usize = 8;

/// The longest one identity name this crate will carry.
///
/// Matches `login_contract::MAX_IDENTITY` (64 bytes) by convention rather than by a shared type: the
/// manifest names the same identities a login authenticates, so a name too long to ever log in with
/// is not a name this store needs to carry either. Not enforced by a shared dependency (this crate
/// takes none), only by this constant and the comment naming why it was chosen.
pub const MAX_IDENTITY_LEN: usize = 64;

/// A parsed manifest: the identity names it lists, in document order, each borrowing the document's
/// own bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Manifest<'a> {
    names: [&'a [u8]; MAX_IDENTITIES],
    n: usize,
}

impl<'a> Manifest<'a> {
    /// The identity names this manifest lists, in the order the document wrote them.
    pub fn entries(&self) -> &[&'a [u8]] {
        &self.names[..self.n]
    }
}

/// Why a manifest document does not parse. Every variant carries the **1-based line number** it
/// went wrong on, matching `timetable::Error`'s own convention and for the same reason: a
/// configuration error nobody can find in the file is one nobody will fix.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// A name longer than [`MAX_IDENTITY_LEN`].
    NameTooLong(usize),
    /// More identities than [`MAX_IDENTITIES`], reported at the line that overflowed rather than
    /// dropped, matching `timetable::Error::TooManyEntries`'s own posture.
    TooManyIdentities(usize),
}

impl Error {
    /// The 1-based line this error is about.
    pub fn line(self) -> usize {
        match self {
            Error::NameTooLong(l) | Error::TooManyIdentities(l) => l,
        }
    }

    /// The fixed half of the sentence a reader gets. Host-tested, so the wording cannot drift.
    pub fn message(self) -> &'static str {
        match self {
            Error::NameTooLong(_) => "an identity name longer than this store carries",
            Error::TooManyIdentities(_) => "more identities than this manifest holds",
        }
    }
}

/// Parse a manifest document: one identity name per line, blank lines and `#` comments ignored,
/// matching `timetable::parse`'s own dialect for the same reason (a reader who already knows one of
/// this tree's document formats should not have to learn a second one for a document this small).
///
/// Fails on the **first** problem, with its line, `timetable::parse`'s own posture.
///
/// # Examples
///
/// ```
/// let doc = timetable_store::parse_manifest("# who has pending work\nalice\nbob\n")
///     .expect("that document is well formed");
/// assert_eq!(doc.entries(), &[b"alice".as_slice(), b"bob".as_slice()]);
///
/// // Comments and blank lines are ignored, and an empty manifest is not an error: nobody has
/// // registered a schedule yet, which is a fact about the fleet, not a malformed document.
/// let empty = timetable_store::parse_manifest("# nobody yet\n\n").unwrap();
/// assert_eq!(empty.entries().len(), 0);
/// ```
pub fn parse_manifest(doc: &str) -> Result<Manifest<'_>, Error> {
    let mut names = [b"".as_slice(); MAX_IDENTITIES];
    let mut n = 0usize;

    for (i, raw) in doc.lines().enumerate() {
        let no = i + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let bytes = line.as_bytes();
        if bytes.len() > MAX_IDENTITY_LEN {
            return Err(Error::NameTooLong(no));
        }
        if n == MAX_IDENTITIES {
            return Err(Error::TooManyIdentities(no));
        }
        names[n] = bytes;
        n += 1;
    }
    Ok(Manifest { names, n })
}

/// Everything up to a `#`. No escape, matching `timetable::parse`'s own reasoning: an identity name
/// has no use for a literal `#`, and an escape nobody needs is a rule everybody has to know.
fn strip_comment(line: &str) -> &str {
    match line.split_once('#') {
        Some((before, _)) => before,
        None => line,
    }
}

/// **Render a manifest**, one identity name per line: the write-path half of this crate, used by
/// whoever records that an identity now has a durable session with pending work. `login` edits the
/// manifest a line at a time instead (`login_contract::with_listed`, the same format), so this is
/// the whole-document form, host-tested against [`parse_manifest`].
///
/// A fixed buffer rather than a `String`, because this crate is `no_std` with no `alloc`
/// (`timetable::write_plan`'s own reasoning, matching it exactly). `None` if `names` will not fit
/// [`MAX_IDENTITIES`], any one name is longer than [`MAX_IDENTITY_LEN`], or the rendered document is
/// wider than `buf`; otherwise `Some(bytes written)`.
///
/// # Examples
///
/// ```
/// let mut buf = [0u8; 64];
/// let n = timetable_store::render_manifest(&[b"alice", b"bob"], &mut buf)
///     .expect("two short names fit");
/// assert_eq!(&buf[..n], b"alice\nbob\n");
///
/// // Parsing what this just rendered gives back exactly what went in: the round trip the write
/// // path and the read path both depend on.
/// let doc = core::str::from_utf8(&buf[..n]).unwrap();
/// let parsed = timetable_store::parse_manifest(doc).unwrap();
/// assert_eq!(parsed.entries(), &[b"alice".as_slice(), b"bob".as_slice()]);
/// ```
pub fn render_manifest(names: &[&[u8]], buf: &mut [u8]) -> Option<usize> {
    if names.len() > MAX_IDENTITIES {
        return None;
    }
    let mut n = 0usize;
    for name in names {
        if name.is_empty() || name.len() > MAX_IDENTITY_LEN {
            return None;
        }
        if n + name.len() + 1 > buf.len() {
            return None;
        }
        buf[n..n + name.len()].copy_from_slice(name);
        n += name.len();
        buf[n] = b'\n';
        n += 1;
    }
    Some(n)
}

/// Fixture data the kernel's tests share, so the schedule document a reader meets in
/// `timetable_tests` and in `login_tests`' start-up test is the same one, matching
/// `filesystem_fixture`'s own convention.
pub mod fixture {
    /// One `at-boot` entry and one `every` entry, matching `timetable::parse`'s own document shape
    /// (`components/timetable.conf`'s own reference document is the model): enough to prove the format
    /// round-trips through a real read from the filesystem, not merely through `include_str!`.
    ///
    /// The commands name the suite's installed fixture program (provisional `scheduled_demo`,
    /// `login_tests`'s `install_scheduled_program`), because a durable session's timetable resolves
    /// them in the installed-programs table, where §229 (how a bare name at the prompt reaches an installed
    /// program) forbids a package from taking a name the image
    /// carries. `timetable_tests`'s registrar control copies this document's shape with an image
    /// program's name back in, because an archive-mode timetable plans against `grant_plan`'s
    /// static manifests and knows only image programs; see its `STORED_SCHEDULE_CONTROL`.
    pub const DEMO_SCHEDULE_DOC: &str = "at-boot scheduled_demo 3\nevery 30s scheduled_demo 7\n";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_manifest_lists_its_identities_in_order() {
        let doc = parse_manifest("# heading\nalice\n\nbob\n# trailing comment\n").unwrap();
        assert_eq!(doc.entries(), &[b"alice".as_slice(), b"bob".as_slice()]);
    }

    #[test]
    fn an_empty_manifest_is_not_an_error() {
        let doc = parse_manifest("# nobody yet\n\n").unwrap();
        assert_eq!(doc.entries().len(), 0);
    }

    #[test]
    fn a_name_past_the_bound_is_refused_at_its_own_line() {
        let mut doc = repeat_byte_string(b'a', MAX_IDENTITY_LEN + 1);
        doc.push('\n');
        assert_eq!(parse_manifest(&doc), Err(Error::NameTooLong(1)));
    }

    #[test]
    fn more_identities_than_the_table_holds_is_refused_rather_than_dropped() {
        let doc = heapless_repeat("alice\n", MAX_IDENTITIES + 1);
        assert_eq!(
            parse_manifest(&doc),
            Err(Error::TooManyIdentities(MAX_IDENTITIES + 1))
        );
    }

    #[test]
    fn render_then_parse_round_trips() {
        let mut buf = [0u8; 256];
        let n = render_manifest(&[b"alice", b"bob", b"durable_demo"], &mut buf).unwrap();
        let text = core::str::from_utf8(&buf[..n]).unwrap();
        let doc = parse_manifest(text).unwrap();
        assert_eq!(
            doc.entries(),
            &[
                b"alice".as_slice(),
                b"bob".as_slice(),
                b"durable_demo".as_slice()
            ]
        );
    }

    #[test]
    fn rendering_refuses_what_it_cannot_carry() {
        let mut buf = [0u8; 256];
        // Too many names.
        let many: [&[u8]; MAX_IDENTITIES + 1] = [b"x".as_slice(); MAX_IDENTITIES + 1];
        assert_eq!(render_manifest(&many, &mut buf), None);

        // A name past the bound.
        let long = repeat_byte_string(b'a', MAX_IDENTITY_LEN + 1);
        assert_eq!(render_manifest(&[long.as_bytes()], &mut buf), None);

        // A buffer too small for what would otherwise fit.
        let mut tiny = [0u8; 3];
        assert_eq!(render_manifest(&[b"alice"], &mut tiny), None);

        // An empty name.
        assert_eq!(render_manifest(&[b""], &mut buf), None);
    }

    /// **Every bound is tested at the bound, not one past it**, which is the gap
    /// milestone 326 (nobody has been assigned to turn a mutation score upward) found: six of this crate's eight survivors were a `>` that could become `>=`
    /// and nothing would notice, because every existing refusal test hands the code a value one
    /// *past* the limit and a value one past the limit is refused either way. A store that quietly
    /// lost the 64th byte of a name or the 8th identity would pass the whole suite above.
    #[test]
    fn the_last_thing_that_fits_still_fits() {
        // A name of exactly MAX_IDENTITY_LEN bytes, through both halves.
        let longest = repeat_byte_string(b'a', MAX_IDENTITY_LEN);
        let mut doc = longest.clone();
        doc.push('\n');
        assert_eq!(
            parse_manifest(&doc).unwrap().entries(),
            &[longest.as_bytes()],
            "a name exactly at the bound is a legal name"
        );
        let mut buf = [0u8; 256];
        assert!(
            render_manifest(&[longest.as_bytes()], &mut buf).is_some(),
            "a name exactly at the bound is renderable"
        );

        // Exactly MAX_IDENTITIES of them, through both halves.
        let full = heapless_repeat("alice\n", MAX_IDENTITIES);
        assert_eq!(
            parse_manifest(&full).unwrap().entries().len(),
            MAX_IDENTITIES
        );
        let names: [&[u8]; MAX_IDENTITIES] = [b"alice".as_slice(); MAX_IDENTITIES];
        assert!(
            render_manifest(&names, &mut buf).is_some(),
            "a manifest holding exactly its capacity is renderable"
        );
    }

    /// **A buffer the exact size of the document is enough, and one byte less is refused rather
    /// than overrun.**
    ///
    /// The two sit in one test because they are the two sides of the same comparison, and the
    /// interesting half is the second: the bound is `n + name.len() + 1`, where the `+ 1` is the
    /// newline that has not been written yet. Lose that term and the check passes on a buffer with
    /// room for the name but not its terminator, and the next line indexes one past the end. That
    /// is a panic in a `no_std` crate the kernel links, which is why the assertion is that it
    /// returns `None` rather than that it returns anything at all.
    #[test]
    fn the_buffer_bound_counts_the_newline_it_has_not_written_yet() {
        let names: [&[u8]; 2] = [b"alice", b"bob"];
        let exact = b"alice\nbob\n".len();

        let mut just_enough = [0u8; 10];
        assert_eq!(just_enough.len(), exact);
        assert_eq!(render_manifest(&names, &mut just_enough), Some(exact));
        assert_eq!(&just_enough[..], b"alice\nbob\n");

        let mut one_short = [0u8; 9];
        assert_eq!(
            render_manifest(&names, &mut one_short),
            None,
            "a buffer with room for the name but not its newline must be refused"
        );
    }

    /// **An error names the line it is about**, and [`Error::line`] is the accessor a caller reads
    /// it through. The refusal tests above compare whole variants, so nothing called this function
    /// at all: a `line` that returned a constant was invisible, and a configuration error pointing
    /// at the wrong line is the failure the 1-based convention exists to prevent.
    #[test]
    fn each_error_carries_the_line_it_is_about() {
        let mut doc = String::from("# heading\nalice\n");
        doc.push_str(&repeat_byte_string(b'a', MAX_IDENTITY_LEN + 1));
        doc.push('\n');
        assert_eq!(parse_manifest(&doc).unwrap_err().line(), 3);

        let crowded = heapless_repeat("alice\n", MAX_IDENTITIES + 1);
        assert_eq!(
            parse_manifest(&crowded).unwrap_err().line(),
            MAX_IDENTITIES + 1
        );
    }

    #[test]
    fn every_error_reads_differently() {
        assert_ne!(
            Error::NameTooLong(1).message(),
            Error::TooManyIdentities(1).message(),
        );
    }

    // This crate is `no_std` with no `alloc`; host tests link `std` (the usual `cfg(test)` shape
    // every crate in this tree uses), so a `String` here is fine even though the library code never
    // allocates one.
    extern crate std;
    use std::string::String;

    fn repeat_byte_string(byte: u8, len: usize) -> String {
        let mut s = String::with_capacity(len);
        for _ in 0..len {
            s.push(byte as char);
        }
        s
    }

    fn heapless_repeat(line: &str, times: usize) -> String {
        let mut s = String::new();
        for _ in 0..times {
            s.push_str(line);
        }
        s
    }
}
