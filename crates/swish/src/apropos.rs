//! **`apropos`: what a search of the documentation store says** (milestone 40 (documentation as
//! a system service), phase 2): [`write_found`] and [`write_apropos`].
//!
//! Name: provisional (milestone 884, 2026-10-12 UTC), for calef's naming pass.

use super::*;

/// How wide the count column is, so the numbers line up on their right edge. Four digits, because
/// the strongest term in the shipped store occurs 32 times and a page mentioning one nine thousand
/// times is a page with a different problem.
const APROPOS_COUNT: usize = 4;

/// **The column a title starts in**, counted from the start of the line.
///
/// Two spaces of indent, the count, two spaces, then the widest location the shipped store
/// produces (`doc/swish/line-discipline.md`, twenty-eight bytes) and two more. A longer location
/// pushes its title right rather than being truncated: losing the name a reader is meant to type
/// would defeat the whole line. See `BUGS` in notes/documentation.md for what that costs at eighty
/// columns.
const APROPOS_TITLE: usize = 2 + APROPOS_COUNT + 2 + 28 + 2;

/// **Render one search result**: how strongly it matched, the name a reader can type, and the
/// page's title.
///
/// The columns are the answer's argument. A person reads this to decide what to open, so the
/// **typeable name** is what has to be unmissable, and the count is what orders the list. The
/// origin ([`documentation::index::Found::origin`]) is deliberately not printed: it is provenance, it is
/// nearly the location again, and a second path on the line would compete with the one the reader
/// is meant to type.
pub fn write_found(f: &documentation::index::Found, out: &mut dyn FnMut(&[u8])) {
    let mut digits = 1;
    let mut v = f.count as u64 / 10;
    while v > 0 {
        digits += 1;
        v /= 10;
    }
    out(b"  ");
    pad(APROPOS_COUNT.saturating_sub(digits), out);
    write_num(f.count as u64, out);
    out(b"  ");
    out(f.location());
    let n = 2 + APROPOS_COUNT.max(digits) + 2 + f.location().len();
    // A location that ran past its column still gets one space, because a title jammed against a
    // path reads as one word.
    pad(APROPOS_TITLE.saturating_sub(n).max(1), out);
    out(f.title());
    out(b"\n");
}

/// **Render a whole search answer**, including the tail that keeps it honest.
///
/// Three things a reader needs and one of them is a refusal to overclaim: the results, the fact
/// that a store said nothing at all, and the fact that more pages matched than the table can hold.
/// [`documentation::index::Ranked`] counts what it dropped precisely so this can say so; printing the
/// results alone would imply the answer was complete.
pub fn write_apropos(term: &[u8], r: &documentation::index::Ranked, out: &mut dyn FnMut(&[u8])) {
    if r.offered() == 0 {
        out(b"  no page in the store says ");
        out(term);
        out(b"\n");
        return;
    }
    for f in r.results() {
        write_found(f, out);
    }
    if r.offered() > r.results().len() {
        out(b"  ");
        write_num(r.results().len() as u64, out);
        out(b" of ");
        write_num(r.offered() as u64, out);
        out(b" pages, strongest first\n");
    }
}

/// Write `n` spaces.
fn pad(n: usize, out: &mut dyn FnMut(&[u8])) {
    const SPACES: &[u8; 32] = &[b' '; 32];
    let mut left = n;
    while left > 0 {
        let take = left.min(SPACES.len());
        out(&SPACES[..take]);
        left -= take;
    }
}

#[cfg(test)]
mod tests;
