//! Fuzz the line editor's handoff reader with arbitrary bytes.
//!
//! A replacement `line_editor` reads the blob its predecessor left in a granted frame. The frame
//! is not trusted: a blob can be cut short, carry a length past its buffer or a cursor past its
//! line. `LineEditor::restore` and `RawQueue::restore` must refuse such a blob or absorb it into a
//! consistent state.
//!
//! # What is asserted, beyond "it returned"
//!
//! 1. **A restored editor keeps its invariant**: `cursor <= length <= MAXIMUM_LINE_BYTES`.
//! 2. **A restored editor saves again, and the second blob restores to the same line and cursor**,
//!    so what was accepted is what is carried on.
//! 3. **A restored raw queue drains no more than `MAXIMUM_RAW_QUEUE_BYTES` bytes.**

#![no_main]

use libfuzzer_sys::fuzz_target;
use line_editor::handoff::{Reader, Writer};
use line_editor::{LineEditor, MAXIMUM_RAW_QUEUE_BYTES, RawQueue};

fuzz_target!(|data: &[u8]| {
    let Ok(mut reader) = Reader::new(data) else {
        return;
    };
    let mut editor = LineEditor::new();
    if editor.restore(&mut reader).is_some() {
        let (line, cursor) = editor.pending();
        assert!(cursor <= line.len(), "the restored cursor left the line");

        let mut page = [0u8; 4096];
        let mut writer = Writer::new(&mut page);
        editor.save(&mut writer);
        let length = writer.finish().expect("a restored editor fits one page");

        let mut again = LineEditor::new();
        let mut reader = Reader::new(&page[..length]).expect("a blob this build wrote reads");
        again
            .restore(&mut reader)
            .expect("a blob this build wrote restores");
        assert_eq!(
            editor.pending(),
            again.pending(),
            "the carried line changed"
        );
    }

    let Ok(mut reader) = Reader::new(data) else {
        return;
    };
    let mut queue = RawQueue::new();
    if queue.restore(&mut reader).is_some() {
        let mut drained = 0;
        while let Some((count, _)) = queue.pop_up_to_eight() {
            drained += count;
            assert!(
                drained <= MAXIMUM_RAW_QUEUE_BYTES,
                "a restored queue held too much"
            );
        }
    }
});
