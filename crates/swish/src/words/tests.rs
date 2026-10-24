//! Tests of pattern versus text, `echo`, quoting (milestone 67 (`swish` the language)) and
//! designation, with the designation survivors of the 2026-10-03 mutation census.

use std::string::String;
use std::vec::Vec;

use super::*;
use crate::test_support::{listing, shown, spec_of};

/// **No word splitting, ever** (calef's ruling, 2026-09-26, with the allocator; §141
/// (application is grant)). A substituted value with spaces, a tab and a pattern character in
/// it stays exactly one word, and the words around it are the ones that were typed. This fails
/// if substitution ever moves before splitting, or if a substituted value is ever read as
/// words or as a pattern again.
#[test]
fn a_substituted_value_is_never_split() {
    let value: &[u8] = b"two  words\tand *.txt";
    let sub = |w: &[u8]| (w == b"$X").then_some(value);
    let mut got = Vec::new();
    pieces(b"a $X 'b c' $X", &sub, &mut |p| {
        got.push(p);
        Ok(())
    })
    .unwrap();
    let words: Vec<Piece<'_>> = got
        .iter()
        .copied()
        .filter(|p| !matches!(p, Piece::Space(_)))
        .collect();
    assert_eq!(
        words,
        [
            Piece::Word(b"a"),
            Piece::Substituted(value),
            Piece::Quoted(b"b c"),
            Piece::Substituted(value),
        ]
    );
    // And through `echo`: `$?` is substituted, printed once, and a quoted `$?` is not.
    let mut out = Vec::new();
    echo(
        b"$? '$?'",
        Status::Refused,
        &mut |_| panic!("nothing here is a pattern"),
        &mut |b| out.extend_from_slice(b),
    );
    assert_eq!(out, b"2 $?");
}

#[test]
fn a_word_with_no_magic_is_text_even_when_it_is_not_a_path() {
    // `has_magic` first. `a:b` carries a byte a name cannot carry, so asking "is this a nameable
    // path" first would refuse an ordinary `echo` word.
    assert_eq!(is_pattern(b"a:b"), Ok(false));
    assert_eq!(is_pattern(b"report.txt"), Ok(false));
}

#[test]
fn a_pattern_off_the_last_component_is_refused() {
    assert_eq!(is_pattern(b"*.txt"), Ok(true));
    assert_eq!(is_pattern(b"docs/*.txt"), Ok(true));
    // Selecting directories to walk is an authority question, not a matching one.
    assert_eq!(is_pattern(b"*/report.txt"), Err(Refusal::PatternInPath));
}

#[test]
fn only_the_first_pattern_on_a_line_is_expanded() {
    let mut asked: Vec<Vec<u8>> = Vec::new();
    let e = expansion(&spec_of(b"rm a.txt *.log *.bak"), &mut |t| {
        asked.push(t.to_vec());
        Ok(listing(&[b"one.log"]))
    })
    .expect("the fixture matches");
    // The literal was skipped, the first pattern expanded, and the second never reached the
    // directory: it is already an unplaceable token and a refusal at plan time.
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0], b"*.log");
    assert!(e.for_positional(0).is_none());
    assert_eq!(e.for_positional(1).expect("expanded here").len(), 1);
}

#[test]
fn a_line_with_no_pattern_reads_no_directory() {
    let mut asked = 0;
    let e = expansion(&spec_of(b"wc report.txt"), &mut |_| {
        asked += 1;
        Ok(NameSet::empty())
    })
    .expect("nothing to expand");
    assert_eq!(asked, 0);
    assert!(e.for_positional(0).is_none());
}

#[test]
fn a_pattern_the_shell_cannot_read_stops_the_line() {
    let said = expansion(&spec_of(b"rm *.txt"), &mut |_| Err(Say::NoDirectory))
        .expect_err("the shell holds nothing to expand against");
    assert_eq!(said, Say::NoDirectory);
}

#[test]
fn echo_copies_ordinary_words_byte_for_byte() {
    let out = shown(|o| {
        echo(
            b"  two  spaces ",
            Status::Ran,
            &mut |_| Ok(NameSet::empty()),
            o,
        );
    });
    assert_eq!(out, "  two  spaces ");
}

#[test]
fn echo_prints_exactly_what_the_same_pattern_would_grant() {
    let set = listing(&[b"notes.txt", b"report.txt"]);
    let printed = shown(|o| {
        echo(b"*.txt", Status::Ran, &mut |_| Ok(set), o);
    });
    let granted = shown(|o| write_set(&set, o));
    // One renderer, so `echo *.txt` and `caps rm *.txt` cannot disagree about what the line
    // designates. That agreement is the whole demonstration.
    assert_eq!(printed, granted);
    assert_eq!(printed, "notes.txt report.txt");
}

#[test]
fn echo_writes_no_empty_runs() {
    // In the program `out` is the shell's `print`, and every call is one CALL on the terminal
    // endpoint. A word with nothing before it must not cost a round trip that carries no
    // bytes, which is what emitting the zero-length whitespace run ahead of it would be.
    let mut writes: Vec<Vec<u8>> = Vec::new();
    let said = echo(
        b"one  two",
        Status::Ran,
        &mut |_| Ok(NameSet::empty()),
        &mut |b| {
            writes.push(b.to_vec());
        },
    );
    assert_eq!(said, Say::Nothing);
    assert!(writes.iter().all(|w| !w.is_empty()), "{writes:?}");
    assert_eq!(writes.concat(), b"one  two");
}

#[test]
fn a_pattern_that_matched_nothing_stops_echo_rather_than_printing_itself() {
    // zsh's answer rather than bash's, and the model forces it: if `echo` printed the pattern
    // where `rm` refuses, the two would disagree about what the line means.
    let mut printed = Vec::new();
    let said = echo(
        b"before *.txt after",
        Status::Ran,
        &mut |_| Err(Say::Cannot(Refusal::NoMatch)),
        &mut |b| printed.extend_from_slice(b),
    );
    assert_eq!(said, Say::Cannot(Refusal::NoMatch));
    assert_eq!(printed, b"before ");
}

#[test]
fn echo_refuses_a_pattern_it_cannot_place_without_reading_a_directory() {
    let mut asked = 0;
    let said = shown_say(|o| {
        echo(
            b"*/report.txt",
            Status::Ran,
            &mut |_| {
                asked += 1;
                Ok(NameSet::empty())
            },
            o,
        )
    });
    assert_eq!(said, Say::Cannot(Refusal::PatternInPath));
    assert_eq!(asked, 0);
}

fn shown_say(f: impl FnOnce(&mut dyn FnMut(&[u8])) -> Say) -> Say {
    f(&mut |_| {})
}

/// **The demonstration the milestone owes, in two lines.** The same four characters are a name
/// when quoted and a set when not, and `echo` prints exactly what a grant would move either way.
#[test]
fn quoting_is_the_difference_between_naming_a_name_and_naming_a_set() {
    let set = listing(&[b"notes.txt", b"report.txt"]);
    let expanded = shown(|o| {
        echo(b"*.txt", Status::Ran, &mut |_| Ok(set), o);
    });
    assert_eq!(expanded, "notes.txt report.txt");

    let mut asked = 0;
    let literal = shown(|o| {
        echo(
            b"\"*.txt\"",
            Status::Ran,
            &mut |_| {
                asked += 1;
                Ok(set)
            },
            o,
        );
    });
    assert_eq!(literal, "*.txt");
    // And the directory was never read, so the quoting is a decision about the line rather than
    // a filter on what came back.
    assert_eq!(asked, 0);
}

#[test]
fn a_quoted_word_keeps_the_spaces_inside_it() {
    let out = shown(|o| {
        echo(
            b"a \"two  spaces\" b",
            Status::Ran,
            &mut |_| Ok(NameSet::empty()),
            o,
        );
    });
    assert_eq!(out, "a two  spaces b");
}

/// The planner never sees a quoted pattern as a pattern, which is the same claim one layer up
/// from [`quoting_is_the_difference_between_naming_a_name_and_naming_a_set`] and the one that
/// decides what actually moves.
#[test]
fn a_quoted_operand_is_not_expanded_before_it_is_planned() {
    let mut asked = 0;
    let e = expansion(&spec_of(b"rm \"*.txt\""), &mut |_| {
        asked += 1;
        Ok(listing(&[b"one.txt"]))
    })
    .expect("nothing to expand");
    assert_eq!(asked, 0, "a quoted word must not reach the directory");
    assert!(e.for_positional(0).is_none());
    // The same line unquoted does read the directory, so the pair is a control rather than a
    // test of a code path nothing takes.
    let mut asked = 0;
    expansion(&spec_of(b"rm *.txt"), &mut |_| {
        asked += 1;
        Ok(listing(&[b"one.txt"]))
    })
    .expect("the fixture matches");
    assert_eq!(asked, 1);
}

/// A misquoted word stops `echo` where a misplaced pattern stops it, and for the same reason:
/// the line does not designate what it looks like it designates.
#[test]
fn echo_refuses_a_word_it_cannot_read() {
    assert_eq!(
        shown_say(|o| echo(b"'unclosed", Status::Ran, &mut |_| Ok(NameSet::empty()), o)),
        Say::Cannot(Refusal::UnclosedQuote),
    );
    assert_eq!(
        shown_say(|o| echo(b"a\"b\"", Status::Ran, &mut |_| Ok(NameSet::empty()), o)),
        Say::Cannot(Refusal::PartlyQuoted),
    );
}

/// A directory holding `src/` and `notes.txt`, asked the way the shell asks: a literal word
/// matches itself or nothing.
fn here_holds(word: &[u8]) -> Result<NameSet, Say> {
    match word {
        b"src" => Ok(NameSet::one(b"src", true).unwrap()),
        b"notes.txt" => Ok(NameSet::one(b"notes.txt", false).unwrap()),
        _ => Err(Say::Cannot(Refusal::NoMatch)),
    }
}

fn designated(line: &[u8], g: grant_plan::WordGrant) -> grant_plan::expand::Designation {
    designation(line, g, &mut here_holds)
        .unwrap()
        .designation()
        .unwrap()
}

/// **A word that names something here is designated, and nothing else is** (milestone 205,
/// §170 clause 2). The pattern `needle` names nothing, the option is the program's, `src/lib.rs`
/// designates its first component, and a repeated name is one grant.
#[test]
fn a_word_that_names_something_here_is_designated() {
    let d = designated(
        b"rg -i needle src/lib.rs notes.txt ./src",
        grant_plan::WordGrant::ReadOnly,
    );
    let names: Vec<&[u8]> = d.names().iter().map(|(n, _)| n).collect();
    assert_eq!(names, [&b"src"[..], b"notes.txt"]);
    assert!(!d.is_here());
}

/// **A line that names nothing designates nothing** (calef's N1 ruling, 2026-09-27T06:27Z), and
/// `.` designates this directory itself. Absolute paths and `..` designate nothing.
#[test]
fn nothing_named_is_nothing_designated_and_dot_is_here() {
    let ro = grant_plan::WordGrant::ReadOnly;
    assert!(designated(b"rg needle", ro).is_empty());
    assert!(designated(b"rg needle /etc ../up", ro).is_empty());
    assert!(designated(b"rg needle .", ro).is_here());
    // A quoted glob is bytes, never looked up.
    assert!(designated(b"rg '*.txt'", ro).is_empty());
}

/// **Only a program that may create is granted a name that is not here yet** (§170 clause 3).
#[test]
fn a_name_not_here_is_designated_only_for_a_program_that_creates() {
    assert!(designated(b"cc -o main", grant_plan::WordGrant::ReadWrite).is_empty());
    let d = designated(b"cc -o main", grant_plan::WordGrant::Create);
    assert!(d.names().contains(b"main"));
}

/// The first word of a line is the program and is never looked up; an option is never looked up
/// either. A resolver that holds every name makes either mistake visible as a designation.
#[test]
fn the_program_and_its_options_are_never_designated() {
    let ro = grant_plan::WordGrant::ReadOnly;
    // `src` exists here, but as the program it is not a grant.
    assert!(designated(b"src", ro).is_empty());
    assert!(designated(b"notes.txt needle", ro).is_empty());
    let mut holds_all = |w: &[u8]| Ok(NameSet::one(w, false).unwrap());
    let d = designation(b"rg -i --color=never", ro, &mut holds_all)
        .unwrap()
        .designation()
        .unwrap();
    assert!(d.is_empty(), "an option is the program's, not a name");
}

/// `..`, an absolute path and a quoted glob designate nothing even where the directory would
/// answer for them: the guard is the shell's and does not depend on the directory refusing.
#[test]
fn dotdot_absolute_and_quoted_glob_words_are_refused_before_the_directory_is_asked() {
    let ro = grant_plan::WordGrant::ReadOnly;
    let mut asked = Vec::new();
    let mut holds_all = |w: &[u8]| {
        asked.push(w.to_vec());
        Ok(NameSet::one(w, false).unwrap())
    };
    for line in [&b"rg .."[..], b"rg ../up", b"rg /etc", b"rg '*.txt'"] {
        let d = designation(line, ro, &mut holds_all)
            .unwrap()
            .designation()
            .unwrap();
        assert!(d.is_empty(), "{}", String::from_utf8_lossy(line));
    }
    assert!(asked.is_empty(), "the directory was asked about {asked:?}");
}
