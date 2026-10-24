//! **The terminal model the tests share**: `Screen` and `feed_all`, kept outside any one test
//! module because the editor's tests and `handoff`'s both drive a [`LineDisc`] on it.
//!
//! Name: provisional (milestone 878's lane, 2026-10-12 UTC; §295 (the line editor's crate root
//! splits into module files)).

use crate::{Event, LineDisc, Sink};
extern crate std;
use std::string::String;
use std::vec::Vec;

/// A one-row terminal model: interprets the echo stream the way a VT does, so the tests
/// assert **what the user sees**, not which escape sequences we happened to emit. This is
/// what lets the engine change its redraw strategy without rewriting every test.
pub(super) struct Screen {
    row: Vec<u8>,
    pub(super) col: usize,
    pub(super) bells: usize,
    pub(super) cleared: bool,
}

impl Screen {
    pub(super) fn new() -> Self {
        Screen {
            row: Vec::new(),
            col: 0,
            bells: 0,
            cleared: false,
        }
    }

    /// The visible row as a string (trailing blanks trimmed).
    pub(super) fn text(&self) -> String {
        let end = self
            .row
            .iter()
            .rposition(|&b| b != b' ')
            .map_or(0, |p| p + 1);
        String::from_utf8_lossy(&self.row[..end]).into_owned()
    }

    fn interpret(&mut self, bytes: &[u8]) {
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            match b {
                0x07 => self.bells += 1,
                0x08 => self.col = self.col.saturating_sub(1),
                b'\r' => self.col = 0,
                b'\n' => { /* row model: a fresh line, content untouched */ }
                0x1b => {
                    // CSI only; that is all the engine emits.
                    assert_eq!(bytes.get(i + 1), Some(&b'['), "non-CSI escape in echo");
                    let mut j = i + 2;
                    let mut n: usize = 0;
                    let mut has_n = false;
                    while j < bytes.len() && bytes[j].is_ascii_digit() {
                        n = n * 10 + (bytes[j] - b'0') as usize;
                        has_n = true;
                        j += 1;
                    }
                    let count = if has_n { n } else { 1 };
                    match bytes[j] {
                        b'D' => self.col = self.col.saturating_sub(count),
                        b'C' => self.col += count,
                        b'K' => self.row.truncate(self.col),
                        b'J' => {
                            self.row.clear();
                            self.cleared = true;
                        }
                        b'H' => self.col = 0,
                        other => panic!("echo used unmodeled CSI final {other:?}"),
                    }
                    i = j;
                }
                _ => {
                    if self.col == self.row.len() {
                        self.row.push(b);
                    } else {
                        self.row[self.col] = b;
                    }
                    self.col += 1;
                }
            }
            i += 1;
        }
    }
}

impl Sink for Screen {
    fn put(&mut self, bytes: &[u8]) {
        self.interpret(bytes);
    }
}

pub(super) fn feed_all(d: &mut LineDisc, s: &mut Screen, bytes: &[u8]) -> Vec<Event> {
    bytes
        .iter()
        .map(|&b| d.feed(b, s))
        .filter(|e| *e != Event::None)
        .collect()
}
