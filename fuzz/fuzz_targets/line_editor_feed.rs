//! Fuzz the line editor with an arbitrary stream of keystrokes and calls.
//!
//! `line_editor::LineEditor::feed` is a byte-at-a-time state machine over input from a serial
//! console, which is input nobody here chose: escape sequences cut anywhere, mouse reports, bytes
//! above ASCII. The editor keeps five fixed buffers and does index arithmetic on them, so the
//! property is its own invariant, not only "it returned".
//!
//! # Input layout
//!
//! Each byte is a keystroke, except the bytes `0xf0` to `0xf4`, which are calls. The byte after a
//! call is an argument length (taken modulo 80) and that many bytes follow as the argument:
//!
//! | Byte | Call |
//! |---|---|
//! | `0xf0` | `start_line` with the argument as the prompt |
//! | `0xf1` | `resume_line` with the argument as the prompt |
//! | `0xf2` | `repaint` |
//! | `0xf3` | `abandon` |
//! | `0xf4` | `insert_text` with the argument as the text |
//!
//! # What is asserted, beyond "it returned"
//!
//! 1. **After every byte, `cursor <= length <= MAXIMUM_LINE_BYTES`**, and the pending line is as long as
//!    the length.
//! 2. **A completed line is at most `MAXIMUM_LINE_BYTES` bytes.**
//! 3. **A sink put is never an empty write**, and no put carries more than a line, a prompt and
//!    the longest cursor-move sequence, so echo cannot grow without bound from one byte.

#![no_main]

use libfuzzer_sys::fuzz_target;
use line_editor::{Event, LineEditor, MAXIMUM_LINE_BYTES, MAXIMUM_PROMPT_BYTES, Sink};

/// The most one put may carry: the screen clear, a prompt, a whole line and a cursor move.
const MOST_PER_PUT: usize = 16 + MAXIMUM_PROMPT_BYTES + MAXIMUM_LINE_BYTES + 16;

struct Counting {
    puts: usize,
}

impl Sink for Counting {
    fn put(&mut self, bytes: &[u8]) {
        assert!(!bytes.is_empty(), "an empty put");
        assert!(bytes.len() <= MOST_PER_PUT, "a put longer than any echo");
        self.puts += 1;
    }
}

fn check(editor: &LineEditor) {
    let (line, cursor) = editor.pending();
    assert!(cursor <= line.len(), "the cursor left the line");
    assert!(
        line.len() <= MAXIMUM_LINE_BYTES,
        "the line outgrew its limit"
    );
}

fuzz_target!(|data: &[u8]| {
    let mut editor = LineEditor::new();
    let mut out = Counting { puts: 0 };
    let mut at = 0;
    while at < data.len() {
        let byte = data[at];
        at += 1;
        if (0xf0..=0xf4).contains(&byte) {
            let wanted = data.get(at).map_or(0, |n| usize::from(*n) % 80);
            at += 1;
            let argument = data
                .get(at..)
                .map_or(&[][..], |rest| &rest[..wanted.min(rest.len())]);
            at += argument.len();
            match byte {
                0xf0 => editor.start_line(argument, &mut out),
                0xf1 => editor.resume_line(argument),
                0xf2 => editor.repaint(&mut out),
                0xf3 => editor.abandon(),
                _ => editor.insert_text(argument, &mut out),
            }
        } else if editor.feed(byte, &mut out) == Event::Line {
            assert!(
                editor.line().len() <= MAXIMUM_LINE_BYTES,
                "a line past its limit"
            );
        }
        check(&editor);
    }
});
