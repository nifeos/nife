//! Tab completion: what the word under the cursor is, which names could finish it, and what to do
//! about them (milestone 47 (navigation and naming), DECISIONS §227 (how Tab reaches the shell)
//! option D).
//!
//! In command position it offers builtins, image programs and installed bare names; anywhere else,
//! entries of the directory the word's lead names, which costs `ENUMERATE` as `echo *` does.
//! [`Matches`] holds no names: the program feeds it candidates, twice when it must list them. See
//! notes/shell/the-swish-crate.md, "Completion".

//! # EXAMPLES
//!
//! ```
//! use swish::complete::{self, Answer, Completing, Matches};
//!
//! // `ca` at the start of a line completes among command names.
//! let line = b"ca";
//! let Completing::Command { prefix } = complete::completing(line, 2) else { panic!() };
//! let mut m = Matches::new(prefix);
//! for name in [b"caps".as_slice(), b"cat", b"cd"] {
//!     m.offer(name, false);
//! }
//! // Two candidates share "ca", which is already typed, so the answer is to list them.
//! assert!(matches!(m.answer(), Answer::List));
//!
//! // `cat rep` completes among the names in the directory the shell stands in.
//! let Completing::Name { dir, prefix } = complete::completing(b"cat rep", 7) else { panic!() };
//! assert_eq!(dir, b"");
//! let mut m = Matches::new(prefix);
//! m.offer(b"report.txt", false);
//! m.offer(b"notes.txt", false);
//! let Answer::Insert { text, finished } = m.answer() else { panic!() };
//! assert_eq!((text, finished), (b"ort.txt".as_slice(), Some(b' ')));
//! ```
//!
//! # BUGS
//!
//! - A quoted word is not completed, and a name with a space in it is inserted unquoted.
//! - Completing an argument does not ask the program's manifest what the argument is.
//! - A name that is both the image's and installed is offered once and refused when run.
//!
//! Name: provisional (milestone 47, 2026-09-26).

/// The longest name [`Matches`] tracks a common prefix over. Longer names still count as
/// candidates; their common prefix is cut at this length, which only shortens what is inserted.
pub const NAME_MAX: usize = 255;

/// What the word under the cursor is.
#[derive(Debug, PartialEq, Eq)]
pub enum Completing<'a> {
    /// The first word of a command: a builtin or a program.
    Command {
        /// What has been typed of it.
        prefix: &'a [u8],
    },
    /// A name in a directory: `dir` is the word up to and including its last `/` (empty for the
    /// directory the shell stands in), `prefix` what follows.
    Name {
        /// The directory part, as typed, with its trailing `/`.
        dir: &'a [u8],
        /// What has been typed of the name.
        prefix: &'a [u8],
    },
    /// Nothing to complete: inside a quote, or a word this shell cannot read as a name.
    Nothing,
}

/// Bytes that end a word, besides whitespace: the operators split a line into commands and
/// redirections.
fn is_break(b: u8) -> bool {
    b.is_ascii_whitespace() || matches!(b, b'|' | b';' | b'&' | b'<' | b'>')
}

/// The prefix words whose operand is itself a command line, so the word after one is in command
/// position again.
const PREFIX_WORDS: [&[u8]; 3] = [b"caps", b"time", b"xargs"];

/// **What the word ending at `cur` is.** Only the part before the cursor counts, as in bash: Tab in
/// the middle of a word completes what is to its left.
pub fn completing(line: &[u8], cur: usize) -> Completing<'_> {
    let cur = cur.min(line.len());
    let mut start = cur;
    while start > 0 && !is_break(line[start - 1]) {
        start -= 1;
    }
    let word = &line[start..cur];
    if word.contains(&b'\'') {
        return Completing::Nothing;
    }
    if let Some(slash) = word.iter().rposition(|&b| b == b'/') {
        return Completing::Name {
            dir: &word[..=slash],
            prefix: &word[slash + 1..],
        };
    }
    if command_position(&line[..start]) {
        Completing::Command { prefix: word }
    } else {
        Completing::Name {
            dir: b"",
            prefix: word,
        }
    }
}

/// Whether a word starting after `before` is the first word of a command.
fn command_position(before: &[u8]) -> bool {
    let mut end = before.len();
    while end > 0 && before[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    if end == 0 {
        return true;
    }
    match before[end - 1] {
        // After `|`, `;`, `&&` or `||` a new command starts. After `<` or `>` a file does.
        b'|' | b';' | b'&' => true,
        b'<' | b'>' => false,
        _ => {
            let mut s = end;
            while s > 0 && !is_break(before[s - 1]) {
                s -= 1;
            }
            PREFIX_WORDS.contains(&&before[s..end]) && command_position(&before[..s])
        }
    }
}

/// **The candidates for one Tab**, fed one at a time: how many match, their longest common prefix,
/// and whether the only match is a directory.
pub struct Matches<'p> {
    prefix: &'p [u8],
    common: [u8; NAME_MAX],
    common_len: usize,
    total: usize,
    only_is_dir: bool,
}

/// What a Tab should do.
#[derive(Debug, PartialEq, Eq)]
pub enum Answer<'a> {
    /// Nothing matches: ring the bell.
    NoMatch,
    /// Insert `text` at the cursor. `finished` is the byte that ends the word when exactly one
    /// name matched: `/` for a directory, a space for anything else. `None` when the text is only
    /// the part every candidate shares.
    Insert {
        /// The bytes to insert.
        text: &'a [u8],
        /// How the word ends, when it is finished.
        finished: Option<u8>,
    },
    /// Several candidates and nothing more they all share: list them and repaint.
    List,
}

impl<'p> Matches<'p> {
    /// No candidates yet, completing `prefix`.
    pub const fn new(prefix: &'p [u8]) -> Self {
        Matches {
            prefix,
            common: [0; NAME_MAX],
            common_len: 0,
            total: 0,
            only_is_dir: false,
        }
    }

    /// Whether `name` would be offered. Dot-names only when the prefix asks for them, as in bash:
    /// `.` and `..` are never a useful completion, and neither is a hidden file you did not start.
    pub fn wants(&self, name: &[u8]) -> bool {
        name.starts_with(self.prefix)
            && !(name.first() == Some(&b'.') && self.prefix.first() != Some(&b'.'))
    }

    /// Consider one candidate.
    pub fn offer(&mut self, name: &[u8], is_dir: bool) {
        if !self.wants(name) {
            return;
        }
        if self.total == 0 {
            let n = name.len().min(NAME_MAX);
            self.common[..n].copy_from_slice(&name[..n]);
            self.common_len = n;
        } else {
            let same = self.common[..self.common_len]
                .iter()
                .zip(name)
                .take_while(|(a, b)| a == b)
                .count();
            self.common_len = same;
        }
        self.total += 1;
        self.only_is_dir = is_dir;
    }

    /// How many candidates matched.
    pub fn total(&self) -> usize {
        self.total
    }

    /// What to do about them.
    pub fn answer(&self) -> Answer<'_> {
        let more = &self.common[self.prefix.len().min(self.common_len)..self.common_len];
        match self.total {
            0 => Answer::NoMatch,
            1 => Answer::Insert {
                text: more,
                finished: Some(if self.only_is_dir { b'/' } else { b' ' }),
            },
            _ if !more.is_empty() => Answer::Insert {
                text: more,
                finished: None,
            },
            _ => Answer::List,
        }
    }
}

/// The column a listing wraps at. The engine assumes one row of a terminal it cannot measure
/// (`line_editor`'s module doc), and this is the same assumption, stated.
pub const LIST_WIDTH: usize = 78;

/// **Prints a listing of candidates**, several to a row, a directory marked with `/` as `ls` marks
/// it. Starts on a fresh row and ends with one, so the caller can repaint the prompt after it.
pub struct Lister {
    col: usize,
    shown: usize,
}

impl Default for Lister {
    fn default() -> Self {
        Self::new()
    }
}

impl Lister {
    /// A listing with nothing printed yet.
    pub const fn new() -> Self {
        Lister { col: 0, shown: 0 }
    }

    /// Print one candidate. The caller filters with [`Matches::wants`], so this prints what it is
    /// given.
    pub fn name(&mut self, name: &[u8], is_dir: bool, out: &mut dyn FnMut(&[u8])) {
        let width = name.len() + usize::from(is_dir);
        if self.shown == 0 {
            out(b"\n");
        } else if self.col + 2 + width > LIST_WIDTH {
            out(b"\n");
            self.col = 0;
        } else {
            out(b"  ");
            self.col += 2;
        }
        out(name);
        if is_dir {
            out(b"/");
        }
        self.col += width;
        self.shown += 1;
    }

    /// End the listing on a fresh row.
    pub fn finish(&mut self, out: &mut dyn FnMut(&[u8])) {
        if self.shown > 0 {
            out(b"\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::vec::Vec;

    fn answer_for(prefix: &[u8], names: &[(&[u8], bool)]) -> (usize, Vec<u8>, Option<u8>, bool) {
        let mut m = Matches::new(prefix);
        for &(n, d) in names {
            m.offer(n, d);
        }
        match m.answer() {
            Answer::NoMatch => (m.total(), Vec::new(), None, false),
            Answer::Insert { text, finished } => (m.total(), text.to_vec(), finished, false),
            Answer::List => (m.total(), Vec::new(), None, true),
        }
    }

    /// Where a word starts and what it completes against, for each place a word can stand.
    #[test]
    fn the_word_under_the_cursor_is_read_the_way_the_parser_reads_it() {
        use Completing::*;
        assert_eq!(completing(b"", 0), Command { prefix: b"" });
        assert_eq!(completing(b"ec", 2), Command { prefix: b"ec" });
        assert_eq!(
            completing(b"cat re", 6),
            Name {
                dir: b"",
                prefix: b"re"
            }
        );
        assert_eq!(
            completing(b"cat docs/re", 11),
            Name {
                dir: b"docs/",
                prefix: b"re"
            }
        );
        assert_eq!(
            completing(b"/lo", 3),
            Name {
                dir: b"/",
                prefix: b"lo"
            }
        );
        // After an operator that starts a command, a command; after a redirection, a file.
        assert_eq!(completing(b"date | w", 8), Command { prefix: b"w" });
        assert_eq!(completing(b"a; b && c", 9), Command { prefix: b"c" });
        assert_eq!(
            completing(b"date >out", 9),
            Name {
                dir: b"",
                prefix: b"out"
            }
        );
        // A prefix word keeps command position for the word after it, and only in command position.
        assert_eq!(completing(b"caps r", 6), Command { prefix: b"r" });
        assert_eq!(completing(b"time xargs r", 12), Command { prefix: b"r" });
        assert_eq!(
            completing(b"echo time r", 11),
            Name {
                dir: b"",
                prefix: b"r"
            }
        );
        // Only the part left of the cursor counts.
        assert_eq!(
            completing(b"cat report", 5),
            Name {
                dir: b"",
                prefix: b"r"
            }
        );
        // Inside a quote there is nothing to complete.
        assert_eq!(completing(b"echo 'no", 8), Nothing);
    }

    /// One match finishes the word; several insert what they share; nothing shared left lists.
    #[test]
    fn the_answer_is_the_longest_thing_every_candidate_agrees_on() {
        assert_eq!(
            answer_for(b"re", &[(b"report.txt", false)]),
            (1, b"port.txt".to_vec(), Some(b' '), false)
        );
        assert_eq!(
            answer_for(b"do", &[(b"docs", true)]),
            (1, b"cs".to_vec(), Some(b'/'), false)
        );
        assert_eq!(
            answer_for(
                b"n",
                &[(b"notes-a", false), (b"notes-b", false), (b"x", false)]
            ),
            (2, b"otes-".to_vec(), None, false)
        );
        assert_eq!(
            answer_for(b"notes-", &[(b"notes-a", false), (b"notes-b", false)]),
            (2, Vec::new(), None, true)
        );
        assert_eq!(
            answer_for(b"z", &[(b"a", false)]),
            (0, Vec::new(), None, false)
        );
        // A name that is exactly the prefix still finishes it.
        assert_eq!(
            answer_for(b"ls", &[(b"ls", false)]),
            (1, Vec::new(), Some(b' '), false)
        );
    }

    /// Hidden names are offered only to a prefix that starts with a dot, so `.` and `..` never are.
    #[test]
    fn dot_names_wait_for_a_dot() {
        assert_eq!(
            answer_for(b"", &[(b".", true), (b"..", true), (b"a", false)]).0,
            1
        );
        assert_eq!(
            answer_for(b".", &[(b".", true), (b"..", true), (b".profile", false)]).0,
            3
        );
    }

    /// A listing wraps before the column limit and ends on a fresh row.
    #[test]
    fn a_listing_wraps_and_marks_directories() {
        let mut out = Vec::new();
        let mut l = Lister::new();
        let long = [b'x'; 40];
        l.name(b"docs", true, &mut |b| out.extend_from_slice(b));
        l.name(&long, false, &mut |b| out.extend_from_slice(b));
        l.name(&long, false, &mut |b| out.extend_from_slice(b));
        l.finish(&mut |b| out.extend_from_slice(b));
        let text = std::string::String::from_utf8(out).unwrap();
        let rows: Vec<&str> = text.split('\n').collect();
        assert_eq!(rows[0], "");
        assert!(rows[1].starts_with("docs/  xxx"), "{text}");
        assert!(rows.iter().all(|r| r.len() <= LIST_WIDTH), "{text}");
        assert_eq!(rows.len(), 4, "{text}");
        assert!(text.ends_with('\n'));
    }

    /// The rows a listing of `(length, is_dir)` names prints, so the wrap column can be probed to
    /// the byte.
    fn listing_rows(names: &[(usize, bool)]) -> Vec<std::string::String> {
        let mut out = Vec::new();
        let mut l = Lister::new();
        for &(len, dir) in names {
            let name = std::vec![b'x'; len];
            l.name(&name, dir, &mut |b| out.extend_from_slice(b));
        }
        l.finish(&mut |b| out.extend_from_slice(b));
        let text = std::string::String::from_utf8(out).unwrap();
        text.lines()
            .skip(1)
            .map(std::string::String::from)
            .collect()
    }

    /// A row of exactly `LIST_WIDTH` columns stays one row, and one column more wraps. Three names
    /// so the column carried after the second is tested too: it is the sum of both names and both
    /// separators.
    #[test]
    fn a_row_may_fill_the_list_width_exactly_and_no_more() {
        assert_eq!(LIST_WIDTH, 78);
        let fits = listing_rows(&[(30, false), (30, false), (14, false)]);
        assert_eq!(fits.len(), 1, "{fits:?}");
        assert_eq!(fits[0].len(), 78);
        let wraps = listing_rows(&[(30, false), (30, false), (15, false)]);
        assert_eq!(wraps.len(), 2, "{wraps:?}");
        assert_eq!(wraps[1].len(), 15);
    }

    /// The `/` a directory is marked with takes a column, so it counts against the width.
    #[test]
    fn the_slash_marking_a_directory_counts_toward_the_row_width() {
        let fits = listing_rows(&[(39, true), (36, false)]);
        assert_eq!(fits.len(), 1, "{fits:?}");
        assert_eq!(fits[0].len(), 78);
        let wraps = listing_rows(&[(38, true), (38, false)]);
        assert_eq!(wraps.len(), 2, "{wraps:?}");
    }

    /// Nothing listed, nothing printed: `finish` adds a row only to end one.
    #[test]
    fn an_empty_listing_prints_nothing() {
        let mut out = Vec::new();
        Lister::new().finish(&mut |b| out.extend_from_slice(b));
        assert!(out.is_empty());
    }
}
