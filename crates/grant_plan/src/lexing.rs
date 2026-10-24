//! **The small text helpers every parser here shares**: whether a name can travel as a grant,
//! trimming, splitting off a word, and reading a number.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The longest file name the shell can name and grant. Duplicated from `filesystem_contract::grant::MAX_NAME`
/// rather than imported, because `grant_plan` is the shell's parser and must not depend on the filesystem
/// contract to check a command line; the pair is pinned by a test in each crate so a change to one
/// without the other fails on the host in milliseconds. 252, calef's ruling of 2026-10-24 (UTC) on milestone 946 (a granted file name can be longer than sixteen bytes)).
pub const MAX_FILE_NAME: usize = 252;

/// **The longest name two `START` argument words carry**: sixteen bytes, duplicated from
/// `filesystem_contract::grant::WORDS_NAME` for [`MAX_FILE_NAME`]'s reason and pinned the same way.
/// A name the shell sends that way is held to it at the prompt: the directory a grant descends into
/// ([`crate::nav::MAX_DIRECTORY_NAME`]), an identity `user` names, and the name `vouch` records.
///
/// Name: provisional (milestone 946), after the constant it mirrors.
pub const WORDS_NAME: usize = 16;

/// Whether a designated name can travel as a grant at all. A name is a single component, the same
/// rule the FS server enforces (DECISIONS §27): a path is not something this shell can express,
/// because there is no namespace here to walk.
pub fn file_name_fits(name: &[u8]) -> bool {
    !name.is_empty()
        && name.len() <= MAX_FILE_NAME
        && name != b"."
        && name != b".."
        && !name.contains(&b'/')
        && !name.contains(&b'\\')
}

/// Trim ASCII whitespace from both ends.
pub fn trim(s: &[u8]) -> &[u8] {
    let mut a = 0;
    let mut b = s.len();
    while a < b && s[a].is_ascii_whitespace() {
        a += 1;
    }
    while b > a && s[b - 1].is_ascii_whitespace() {
        b -= 1;
    }
    &s[a..b]
}

/// Split off the first whitespace-delimited word; return `(word, rest)` where `rest` keeps its
/// internal spacing (so `echo` can print it verbatim). The word keeps its quotes if it had any;
/// [`parse`] takes them off, because whether the word is a builtin is a question about what it
/// says rather than about how it was spelled.
pub(crate) fn split_first_word(s: &[u8]) -> (&[u8], &[u8]) {
    let mut i = word::span(s, 0, &|b| b.is_ascii_whitespace());
    let word = &s[..i];
    while i < s.len() && s[i].is_ascii_whitespace() {
        i += 1;
    }
    (word, &s[i..])
}

/// Parse a base-10 `u64`. `None` for empty or any non-digit byte, so `--mem twelve` is a missing
/// grant rather than a silent zero.
pub fn parse_u64(s: &[u8]) -> Option<u64> {
    if s.is_empty() {
        return None;
    }
    let mut v: u64 = 0;
    for &b in s {
        if !b.is_ascii_digit() {
            return None;
        }
        v = v.checked_mul(10)?.checked_add((b - b'0') as u64)?;
    }
    Some(v)
}
