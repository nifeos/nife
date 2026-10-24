//! Tests of `apropos`: the answer a search prints (milestone 40 (documentation as a system
//! service), phase 2).

use std::vec::Vec;

use super::*;
use crate::test_support::shown;

#[test]
fn a_search_answer_names_pages_a_reader_can_type() {
    let mut r = documentation::index::Ranked::new();
    // Same page length for both, so the ranking this test is not about (density) does not
    // move the order the formatting assertions below depend on.
    r.offer(
        b"swish",
        b"notes/pipes.md",
        b"Pipes and redirection",
        31,
        100,
    );
    r.offer(
        b"kernel",
        b"notes/ipc-naming.md",
        b"Who does IPC name?",
        11,
        100,
    );
    let s = shown(|o| write_apropos(b"capability", &r, o));

    // **The typeable name is the deliverable.** A result a person cannot act on is a list of
    // titles, and `doc doc/swish/pipes.md` is the line this answer exists to produce.
    assert!(s.contains("doc/swish/pipes.md"), "{s}");
    assert!(s.contains("doc/kernel/ipc-naming.md"), "{s}");
    assert!(s.contains("Pipes and redirection"), "{s}");
    // Strongest first, and the source path is not printed: it is provenance, and a second path
    // on the line would compete with the one the reader is meant to type.
    let pipes = s.find("pipes.md").expect("the first result");
    let ipc = s.find("ipc-naming.md").expect("the second result");
    assert!(pipes < ipc, "{s}");
    assert!(!s.contains("notes/pipes.md"), "{s}");
    // The titles line up, which is what makes a list of five scannable rather than ragged.
    let cols: Vec<usize> = s
        .lines()
        .map(|l| l.find("Pipes").or_else(|| l.find("Who")).unwrap_or(0))
        .filter(|&c| c > 0)
        .collect();
    // 38, not `APROPOS_TITLE`: comparing the rendering against the constant it is derived from
    // is an identity that a mutation of the constant moves with. The literal is what a reader at
    // eighty columns gets.
    assert_eq!(cols, [38, 38], "{s}");
    assert_eq!(APROPOS_TITLE, 38, "the column a title starts in");
    // Two results offered and two shown is not a truncation, so the honest tail stays off.
    assert!(!s.contains("strongest first"), "nothing was dropped: {s}");
}

/// **A count wide enough to need the loop that measures it.** `write_found` counts a match
/// count's digits so it can right-align it, and every fixture above uses a two-digit count,
/// where `count / 10` and `count % 10` happen to agree on the answer. Milestone 326's run
/// found the division alive under `%`: at three digits it measures one where there are three,
/// and the column the whole list lines up on moves under the widest row.
#[test]
fn a_three_digit_match_count_still_lines_its_title_up() {
    let mut r = documentation::index::Ranked::new();
    r.offer(
        b"swish",
        b"notes/pipes.md",
        b"Pipes and redirection",
        512,
        100,
    );
    let s = shown(|o| write_apropos(b"capability", &r, o));

    assert!(s.contains("512"), "{s}");
    let col = s.lines().next().unwrap().find("Pipes").expect("the title");
    assert_eq!(
        col, 38,
        "a wider count must not push the title off its column: {s}"
    );
}

#[test]
fn a_search_that_found_nothing_says_so_in_the_words_of_the_question() {
    let r = documentation::index::Ranked::new();
    let s = shown(|o| write_apropos(b"quantum", &r, o));
    assert_eq!(s, "  no page in the store says quantum\n");
}

#[test]
fn a_truncated_answer_says_how_much_it_dropped() {
    // The half that keeps the answer honest: printing sixteen results out of forty without
    // saying so implies the store holds sixteen.
    let mut r = documentation::index::Ranked::new();
    for i in 0..documentation::index::RESULTS_MAX + 4 {
        // Same page length for every offering, so the count this test is truncating stays the
        // thing that decides rank, and this is a test about the truncation count, not density.
        r.offer(b"kernel", b"notes/x.md", b"X", i as u16, 100);
    }
    let s = shown(|o| write_apropos(b"capability", &r, o));
    assert!(s.contains("16 of 20 pages, strongest first"), "{s}");
}
