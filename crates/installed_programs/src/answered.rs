//! **How the answer was given**: the `answered` line of a generation, written beside the grant
//! record, never inside it (milestone 911 (scripted installs grant only what the owner's approval
//! file allows)). The record says what the owner granted; this line says who answered: the owner's
//! console, or an approval file by name and digest. It is one line per generation, after the
//! grant lines, and it is the audit answer to "who allowed this?" that still lets two equal
//! grants look the same to anything that compares them.

use super::{Digest, Error};

/// **The `answered` line**: how the generation's answer was given.
///
/// ```text
/// answered console
/// answered approval lab-runner sha256:<64 hex>
/// ```
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answered<'a> {
    /// The owner said yes at the console (milestone 892's confirmation), or the console ran
    /// `jig install --approved` and the owner's own file replaced the prompt. The console holds
    /// the authority either way; the grants were answered, not delegated.
    Console,
    /// The answer came from the approval named, bound to the file's digest at `jig approve`
    /// (the ruling of 2026-10-12 on #2030, (a)): a console install with `--approved`, a
    /// delegated holder's install, or basalt's composition against the image's file.
    Approval(&'a str, Digest),
}

impl Answered<'_> {
    /// The line's bytes, written to `out`; [`Error::TooSmall`] when it does not fit. Absent for
    /// a generation that records no answer is the caller's choice: a vouch writes none, and
    /// neither does a table written before the line.
    pub fn write(&self, out: &mut [u8]) -> Result<usize, Error> {
        let mut at = 0;
        let mut push = |bytes: &[u8]| -> Result<(), Error> {
            out.get_mut(at..at + bytes.len())
                .ok_or(Error::TooSmall)?
                .copy_from_slice(bytes);
            at += bytes.len();
            Ok(())
        };
        push(b"answered ")?;
        match *self {
            Answered::Console => push(b"console")?,
            Answered::Approval(name, digest) => {
                push(b"approval ")?;
                push(name.as_bytes())?;
                push(b" ")?;
                let mut hex = [0u8; measured_boot::DIGEST_TEXT_LEN];
                hex.copy_from_slice(&measured_boot::digest_text(&digest));
                push(&hex)?;
            }
        }
        push(b"\n")?;
        Ok(at)
    }

    /// The name the answer came from, or `None` for the console.
    pub fn approval(&self) -> Option<(&str, Digest)> {
        match *self {
            Answered::Console => None,
            Answered::Approval(name, digest) => Some((name, digest)),
        }
    }
}

/// Parse one `answered` line's words (after its first), or [`Error::Malformed`]: anything this
/// reader does not know makes the table unreadable, as every line kind's reader does.
pub(super) fn parse<'a>(mut words: impl Iterator<Item = &'a str>) -> Result<Answered<'a>, Error> {
    let Some(kind) = words.next() else {
        return Err(Error::Malformed);
    };
    match kind {
        "console" if words.next().is_none() => Ok(Answered::Console),
        "approval" => {
            let (Some(name), Some(hex), None) = (words.next(), words.next(), words.next()) else {
                return Err(Error::Malformed);
            };
            // The name is one word a row's columns are held to (`good_name`), and it is not the
            // other kind's word, so the two spellings cannot be mistaken for each other.
            if !super::generation::good_name(name) || name == "console" {
                return Err(Error::Malformed);
            }
            match measured_boot::parse_digest(hex) {
                Ok(digest) => Ok(Answered::Approval(name, digest)),
                Err(_) => Err(Error::Malformed),
            }
        }
        _ => Err(Error::Malformed),
    }
}

/// **The grant record of `table`**: its grant lines' text, in the table's order. What milestone
/// 911's Done-when row 4 compares across the paths: for the same grants, this text is
/// byte-identical however the answer was given.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub fn record(table: &str) -> impl Iterator<Item = &str> {
    table.lines().filter(|line| line.starts_with("grant "))
}

/// **Whether two tables' records are byte-identical**: the property milestone 911 holds between
/// an interactive yes and a scripted install for the same grants.
pub fn same_record(a: &str, b: &str) -> bool {
    record(a).eq(record(b))
}

/// **The table's `answered` line**, or `None` for a generation that records no answer: what the
/// audit question ("who allowed this?") reads. Two of them are no single answer, as two default
/// pointers for one program are none, and either table is refused.
pub fn answered(table: &str) -> Result<Option<Answered<'_>>, Error> {
    use super::generation::{Line, classify};
    let mut found: Option<Answered<'_>> = None;
    for line in table
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        match classify(line) {
            Line::Answered(a) => {
                if found.is_some() {
                    return Err(Error::Malformed);
                }
                found = Some(a);
            }
            Line::Malformed => return Err(Error::Malformed),
            Line::Row { .. } | Line::Pointer { .. } | Line::Grant(_) => {}
        }
    }
    Ok(found)
}
