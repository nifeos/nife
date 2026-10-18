//! Tests of reading a line: tokens, quoting, builtins, `--mem`, and the words `argv` carries.

use crate::test_support::{WITH_DIR, plan};
use crate::*;

/// Every listed builtin is one `parse` answers as a builtin, and none is also a program, so
/// Tab never offers a word that would run something else.
#[test]
fn every_listed_builtin_is_a_builtin_and_no_program() {
    for &b in BUILTINS {
        assert!(
            !matches!(parse(b), Command::Run(_)),
            "{:?} is not a builtin",
            b
        );
        assert!(Prog::from_name(b).is_none(), "{:?} is also a program", b);
    }
    assert!(
        matches!(parse(b"wc"), Command::Run(_)),
        "the control: a program runs"
    );
}

#[test]
fn empty_and_blank_lines_are_empty() {
    assert_eq!(parse(b""), Command::Empty);
    assert_eq!(parse(b"   "), Command::Empty);
    assert_eq!(parse(b"\t \n"), Command::Empty);
}

/// The tokenizer's edges, exactly. Trailing whitespace must scan to `len` and stop (one byte
/// past is a fault) and must yield no token, empty or otherwise; a line with more tokens than
/// the buffer drops the rest rather than writing past it.
#[test]
fn tokenize_stops_at_its_buffer_and_at_trailing_whitespace() {
    let mut out = [&b""[..]; 8];
    assert_eq!(tokenize(b"  a  bb  ", &mut out), [&b"a"[..], &b"bb"[..]]);

    let mut two = [&b""[..]; 2];
    assert_eq!(tokenize(b"a b c", &mut two), [&b"a"[..], &b"b"[..]]);
}

/// **Whitespace inside quotes is not a separator** (milestone 67), which is what makes a name
/// with a space in it a single token and therefore a single designation.
#[test]
fn a_quoted_span_is_one_token_however_many_spaces_are_in_it() {
    let mut out = [&b""[..]; 8];
    assert_eq!(
        tokenize(b"wc \"my notes.txt\"", &mut out),
        [&b"wc"[..], &b"\"my notes.txt\""[..]],
    );
    // The quotes stay on here: what a token *means* is `word::read`'s question, and keeping the
    // two apart is what lets one place refuse `a"b"` for every scanner.
    assert_eq!(
        tokenize(b"echo 'a  b' c", &mut out),
        [&b"echo"[..], &b"'a  b'"[..], &b"c"[..]],
    );
}

/// The parser takes the quotes off, and what is left is the designation.
#[test]
fn a_quoted_operand_designates_the_bytes_between_the_quotes() {
    let r = parse_run(b"wc \"my notes.txt\"");
    assert_eq!(r.prog, b"wc");
    assert_eq!(r.positionals(), [&b"my notes.txt"[..]]);
    assert!(r.is_quoted(0), "the flag is what stops it being expanded");
    assert_eq!(r.misquoted, None);
    // A bare operand is unchanged, so nothing about an ordinary line moved.
    assert!(!parse_run(b"wc notes.txt").is_quoted(0));
}

/// **A quoted token is never an option**, which is the sharpest edge quoting has here: `-r` is
/// what widens `rm`'s grant from one name to a subtree walk, and a quoted `-r` is a file name.
#[test]
fn quoting_keeps_a_dash_from_widening_a_grant() {
    let r = parse_run(b"rm \"-r\"");
    assert_eq!(r.options(), b"");
    assert_eq!(r.positionals(), [&b"-r"[..]]);
    // And unquoted it is still the option it always was.
    assert_eq!(parse_run(b"rm -r x").options(), b"r");
    // Same rule for `--mem`, which is an option that moves memory.
    let r = parse_run(b"memory_grant_depleter \"--mem\" 16");
    assert_eq!(r.mem, None);
    assert_eq!(r.positionals(), [&b"--mem"[..], &b"16"[..]]);
    // ... while a quoted *value* is still the number it says.
    assert_eq!(
        parse_run(b"memory_grant_depleter --mem \"16\"").mem,
        Some(16)
    );
}

/// A builtin's name is a word like any other, so quoting it still names it. Nothing about
/// authority turns on this; what turns on it is that a reader can quote anything without
/// wondering which words are exempt.
#[test]
fn a_quoted_builtin_name_still_names_the_builtin() {
    assert!(matches!(parse(b"\"help\""), Command::Help));
    assert!(matches!(parse(b"'echo' hi"), Command::Echo(b"hi")));
}

/// **A line whose quotes do not make sense is refused before a program is named**, because
/// otherwise "no such program" would be a true sentence about a word nobody typed.
#[test]
fn a_misquoted_line_is_refused_ahead_of_everything_else() {
    let r = parse_run(b"wc 'my notes.txt");
    assert_eq!(r.misquoted, Some(Refusal::UnclosedQuote));
    assert_eq!(plan(&r, WITH_DIR), Err(Refusal::UnclosedQuote));
    // Even when the program does not exist, so the ordering is real rather than incidental.
    let r = parse_run(b"nosuchprog 'x");
    assert_eq!(plan(&r, WITH_DIR), Err(Refusal::UnclosedQuote));
    // And the joining case.
    assert_eq!(
        parse_run(b"wc a\"b\"").misquoted,
        Some(Refusal::PartlyQuoted)
    );
}

/// `--mem` at the very end of the line reads no value, and reads no memory past the token
/// array either; the missing value stays `None` for the plan to refuse.
#[test]
fn a_trailing_mem_flag_is_a_missing_value_not_a_read_past_the_line() {
    let r = parse_run(b"memory_grant_depleter --mem");
    assert_eq!(r.prog, b"memory_grant_depleter");
    assert_eq!(r.mem, None);
}

/// `--mem N` is exactly two tokens: the value is consumed with the flag and must not reappear
/// as an operand. The cursor step is the code under test, so the flag sits at index 1, where
/// stepping by the wrong arithmetic visibly lands somewhere else.
#[test]
fn a_mem_flag_consumes_its_value_and_nothing_after_it() {
    let r = parse_run(b"memory_grant_depleter --mem 4");
    assert_eq!(r.mem, Some(4));
    assert!(r.positionals().is_empty(), "the value became an operand");
    assert_eq!(r.unexpected, None);
}

/// One past either ceiling lands in `unexpected`; the arrays are exactly their declared sizes,
/// so an off-by-one here is a write past the end, not a smaller refusal.
#[test]
fn the_flag_and_positional_ceilings_refuse_rather_than_overflow() {
    // Nine letters in one cluster, and MAX_FLAGS is eight.
    let r = parse_run(b"rm -abcdefghi x");
    assert_eq!(r.options(), b"");
    assert_eq!(r.unexpected, Some(&b"-abcdefghi"[..]));

    // Five operands, and MAX_POSITIONALS is four.
    let r = parse_run(b"prog a b c d e");
    assert_eq!(r.positionals().len(), MAX_POSITIONALS);
    assert_eq!(r.unexpected, Some(&b"e"[..]));
}

#[test]
fn help_is_a_builtin_and_everything_else_is_a_program() {
    // The whole of milestone 47's `run` removal, in three lines: three reserved words, and any
    // other first token is a program name. Nobody has to know which class a command is in.
    assert_eq!(parse(b"help"), Command::Help);
    assert_eq!(parse(b"  help  "), Command::Help);
    let Command::Run(r) = parse(b"frobnicate") else {
        panic!("a bare word must be a program invocation")
    };
    assert_eq!(r.prog, b"frobnicate");

    // Milestone 47's navigation builtins. They parse to themselves, and, because builtins win
    // over programs, **the program namespace must never learn one of these names**: it would be
    // unreachable, and the reader would have no way to tell why.
    assert_eq!(parse(b"pwd"), Command::Pwd);
    assert_eq!(parse(b"cd deeper"), Command::Cd(b"deeper"));
    assert_eq!(parse(b"cd"), Command::Cd(b""), "bare `cd` is your root");
    assert_eq!(parse(b"ls"), Command::Ls(b""));
    assert_eq!(parse(b"ls  sub "), Command::Ls(b"sub"));
    assert_eq!(parse(b"mkdir logs"), Command::Mkdir(b"logs"));
    assert_eq!(
        parse(b"touch logs"),
        Command::Touch(TouchArgs {
            at: None,
            name: b"logs"
        })
    );
    assert_eq!(
        parse(b"touch"),
        Command::Touch(TouchArgs {
            at: None,
            name: b""
        }),
        "bare `touch` needs a name"
    );
    // DECISIONS §112's arbitrary-instant form: two words after the flag, the RFC 3339 text
    // (kept exactly as typed, unparsed) and the name.
    assert_eq!(
        parse(b"touch -t 2030-01-01T00:00:00Z logs"),
        Command::Touch(TouchArgs {
            at: Some(b"2030-01-01T00:00:00Z"),
            name: b"logs"
        })
    );
    // A name that merely starts with `-t` is not the flag: the guard requires whitespace or
    // end-of-input right after it.
    assert_eq!(
        parse(b"touch -template"),
        Command::Touch(TouchArgs {
            at: None,
            name: b"-template"
        }),
        "-template must not be misread as -t plus a stray name"
    );
    // `bind`: one word for the target, everything after it (trimmed) for the name.
    assert_eq!(
        parse(b"bind logs/2026 recent"),
        Command::Bind(b"logs/2026", b"recent"),
    );
    assert_eq!(
        parse(b"  bind   logs/2026   recent  "),
        Command::Bind(b"logs/2026", b"recent"),
    );
    assert_eq!(
        parse(b"bind"),
        Command::Bind(b"", b""),
        "bare `bind` needs both words"
    );
    assert_eq!(
        parse(b"bind logs/2026"),
        Command::Bind(b"logs/2026", b""),
        "one word is a target with no name to file it under",
    );
    assert_eq!(
        parse(b"apropos capability"),
        Command::Apropos(b"capability")
    );
    assert_eq!(
        parse(b"apropos  capability "),
        Command::Apropos(b"capability")
    );
    // With no operand it is still the builtin, so the shell answers "that verb needs a word"
    // rather than looking for a program nobody has.
    assert_eq!(parse(b"apropos"), Command::Apropos(b""));
}

/// **`vouch` takes one file and records it by its last component** (DECISIONS §221). A bare
/// name is not a file by the prompt's rule, and a name the installed-programs table could not read back
/// as itself is refused before anything is sent.
/// **`user` takes a verb and one name** (milestone 152's §108 ruling), and anything else is
/// [`UserVerb::Usage`], so a line the list could not hold never reaches it.
#[test]
fn user_takes_suspend_or_resume_and_one_name() {
    let Command::User(tail) = parse(b"user  suspend  alice ") else {
        panic!("`user` is not its own command");
    };
    assert_eq!(user_verb(tail), UserVerb::Suspend(b"alice"));
    assert_eq!(user_verb(b"resume bob"), UserVerb::Resume(b"bob"));
    for bad in [
        &b""[..],
        b"suspend",
        b"suspend two names",
        b"delete alice",
        b"suspend #alice",
        b"resume seventeen-bytes!!",
    ] {
        assert_eq!(
            user_verb(bad),
            UserVerb::Usage,
            "{:?}",
            core::str::from_utf8(bad)
        );
    }
}

#[test]
fn vouch_takes_one_path_and_names_it_by_its_last_component() {
    let Command::Vouch(tail) = parse(b"vouch  installed/unvouched ") else {
        panic!("`vouch` is not its own command");
    };
    assert_eq!(vouch_verb(tail), Some(&b"installed/unvouched"[..]));
    assert_eq!(
        vouched_name(b"installed/unvouched"),
        Some(&b"unvouched"[..])
    );
    assert_eq!(vouched_name(b"./a.out"), Some(&b"a.out"[..]));
    for bad in [
        &b""[..],
        b"a.out",
        b"./a b",
        b"dir/",
        b"./..",
        b"./x#y",
        b"./seventeen-bytes!!",
    ] {
        assert_eq!(vouch_verb(bad), None, "{:?}", core::str::from_utf8(bad));
    }
}

/// **`jig` is a program, and `package` is nothing** (milestone 809 (the package client
/// becomes a program)). The builtin and the program went in one change, so a line that still
/// says `package` reaches the planner as a program nobody has, and `jig` is a row of the
/// table, which builtins are matched before.
#[test]
fn jig_is_a_program_and_package_is_not_a_builtin() {
    let Command::Run(r) = parse(b"jig install greeting") else {
        panic!("`jig` must parse as a program invocation");
    };
    assert_eq!(Prog::from_name(r.prog), Some(Prog::Jig));
    let Command::Run(r) = parse(b"package install greeting") else {
        panic!("`package` is no longer a builtin");
    };
    assert_eq!(Prog::from_name(r.prog), None);
    // **And `rm` is a program**, which is the whole of milestone 47's rmdir lane at this level.
    // A builtin would have shadowed the name, so this line is also the check that it no longer
    // does; the manifest is what decides what the operand grants.
    let Command::Run(r) = parse(b"rm old.txt") else {
        panic!("`rm` must parse as a program invocation, not a builtin")
    };
    assert_eq!(r.prog, b"rm");
    assert_eq!(Prog::from_name(b"rm"), Some(Prog::Rm));
    for reserved in [
        &b"help"[..],
        b"echo",
        b"caps",
        b"time",
        b"cd",
        b"pwd",
        b"ls",
        b"mkdir",
        b"touch",
        b"bind",
    ] {
        assert!(
            Prog::from_name(reserved).is_none(),
            "`{}` is a builtin, so a program of that name could never be run",
            core::str::from_utf8(reserved).unwrap(),
        );
    }
}

#[test]
fn echo_keeps_internal_spacing() {
    assert_eq!(parse(b"echo hello world"), Command::Echo(b"hello world"));
    assert_eq!(parse(b"echo   two  spaces"), Command::Echo(b"two  spaces"));
    assert_eq!(parse(b"echo"), Command::Echo(b""));
}

#[test]
fn a_second_integer_is_unexpected_not_ignored() {
    let Command::Run(r) = parse(b"least_authority_demo 3 5") else {
        panic!()
    };
    assert_eq!(r.positionals(), [&b"3"[..], &b"5"[..]]);
    assert_eq!(plan(&r, Holdings::default()), Err(Refusal::Unexpected));
}

#[test]
fn mem_flag_must_have_a_numeric_value() {
    // `--mem twelve` is a missing grant, not a silent zero: parse_u64 rejects it, so memory_grant_depleter
    // sees "no --mem given" and refuses.
    let Command::Run(r) = parse(b"memory_grant_depleter --mem twelve") else {
        panic!()
    };
    assert_eq!(r.mem, None);
    assert_eq!(
        plan(&r, Holdings::default()),
        Err(Refusal::MemoryGrantRequired)
    );
}

#[test]
fn caps_previews_the_command_you_would_have_typed() {
    // The new spelling, and the reason for it: the tail is the line itself, so what you inspect
    // and what you run cannot drift apart. With `run` gone there is no verb left to echo.
    assert_eq!(parse(b"caps"), Command::Caps(b""));
    assert_eq!(
        parse(b"caps memory_grant_depleter --mem 16"),
        Command::Caps(b"memory_grant_depleter --mem 16")
    );
    let Command::Caps(tail) = parse(b"caps memory_grant_depleter --mem 16") else {
        panic!()
    };
    let (Command::Run(previewed), Command::Run(typed)) =
        (parse(tail), parse(b"memory_grant_depleter --mem 16"))
    else {
        panic!()
    };
    assert_eq!(
        plan(&previewed, Holdings::default()),
        plan(&typed, Holdings::default()),
        "the preview must plan the identical endowment to the command itself",
    );
}

/// **`time` runs the command you typed, so its tail has to parse back to that command**
/// (milestone 86).
///
/// The same property `caps` has, and asserted the same way, because it is the same claim: the
/// tail is the line with the prefix word taken off, so what is timed and what would have run
/// plan the identical endowment. A prefix word that rewrote its tail (dropping an option,
/// re-tokenizing, inserting a grant) would make "what you time is what you run" a slogan.
#[test]
fn time_runs_the_command_you_would_have_typed() {
    assert_eq!(parse(b"time"), Command::Time(b""));
    assert_eq!(
        parse(b"time memory_grant_depleter --mem 16"),
        Command::Time(b"memory_grant_depleter --mem 16")
    );
    let Command::Time(tail) = parse(b"time memory_grant_depleter --mem 16") else {
        panic!()
    };
    let (Command::Run(timed), Command::Run(typed)) =
        (parse(tail), parse(b"memory_grant_depleter --mem 16"))
    else {
        panic!()
    };
    assert_eq!(
        plan(&timed, Holdings::default()),
        plan(&typed, Holdings::default()),
        "timing a command must not change what it is granted",
    );

    // **The operators survive the prefix**, which is the reason the tail is a whole line rather
    // than a token list. `time date | wc` is a timed *pipeline*; a `time` that took the first
    // word would time `date` and silently drop the rest, which is the bug `caps` already has a
    // test for one layer up (`swish::route`).
    assert_eq!(parse(b"time date | wc"), Command::Time(b"date | wc"));
}

/// Assemble `line`'s argv and check it reads back as exactly `want`.
fn assert_words(line: &[u8], want: &[&[u8]]) {
    let mut page = [0u8; program_argument_contract::PAGE_BYTES];
    assert_eq!(argv(line, &mut page), Ok(want.len() as u32));
    let got = program_argument_contract::ArgPage::parse(&page);
    assert_eq!(got.len(), want.len());
    assert!(got.iter().eq(want.iter().copied()));
}

fn refused(line: &[u8]) -> Refusal {
    argv(line, &mut [0u8; program_argument_contract::PAGE_BYTES]).unwrap_err()
}

/// **Every word arrives as it was typed, and none is classified** (§170). An option the shell
/// never heard of, a `--long=value`, an integer, and a quoted phrase with the quotes off; the
/// name first, as `argv[0]`. Past `parse_run`'s sixteen tokens too, because nothing is placed.
#[test]
fn argv_carries_every_word_as_typed() {
    assert_words(
        b"rg  -i --color=never 'two words' 8 \"a*b\"",
        &[b"rg", b"-i", b"--color=never", b"two words", b"8", b"a*b"],
    );
    let mut many = [b' '; 80];
    for i in 0..40 {
        many[2 * i] = b'w';
    }
    assert_words(&many, &[&b"w"[..]; 40]);
}

/// **An unquoted pattern is refused, not expanded and not passed through** (§170: the words
/// carry no authority, so expanding one would hand over names and nothing they name). The same
/// bytes quoted are one word.
#[test]
fn argv_refuses_an_unquoted_pattern_and_passes_a_quoted_one() {
    assert_eq!(refused(b"rg foo *.rs"), Refusal::PatternInArguments);
    assert_words(b"rg foo '*.rs'", &[b"rg", b"foo", b"*.rs"]);
    assert_eq!(refused(b"rg 'foo"), Refusal::UnclosedQuote);
}

/// **A line that does not fit is refused whole**, and the page it was being written onto is
/// left reading as no arguments rather than as the words that fitted.
#[test]
fn argv_refuses_a_line_past_one_page_and_leaves_no_argv() {
    let mut line = [b'x'; program_argument_contract::CAPACITY + 3];
    line[..3].copy_from_slice(b"rg ");
    let mut page = [0u8; program_argument_contract::PAGE_BYTES];
    assert_eq!(argv(&line, &mut page), Err(Refusal::ArgumentsTooLong));
    assert!(program_argument_contract::ArgPage::parse(&page).is_empty());
}

// ---- survivor triage of the 2026-10-03 census ---------------------------------------------

#[test]
fn a_quoted_word_is_known_by_its_position_and_the_last_slot_is_readable() {
    let spec = parse_run(b"rm \"a\" b c d");
    assert!(spec.is_quoted(0));
    assert!(!spec.is_quoted(1));
    assert!(
        !spec.is_quoted(MAX_POSITIONALS),
        "past the end was not quoted"
    );
}

/// A line of words is taken while the page holds it. Each word costs its bytes and a four-byte
/// prefix, so the line that fills the page exactly is taken and one byte more is not.
#[test]
fn a_line_that_exactly_fills_the_argument_page_is_taken() {
    let cap = program_argument_contract::CAPACITY;
    let word = |n: usize| {
        let mut line = [b'a'; 8192];
        line[..n].fill(b'a');
        line
    };
    let exact = word(cap - 4);
    assert_eq!(check_words(&exact[..cap - 4]), Ok(()));
    assert_eq!(
        check_words(&exact[..cap - 3]),
        Err(Refusal::ArgumentsTooLong)
    );
    // The cost is bytes plus four, not four times the bytes: two 1,100-byte words fit.
    let mut two = [b'a'; 2201];
    two[1100] = b' ';
    assert_eq!(check_words(&two), Ok(()));
}
