//! Tests of the scoped block's reading and its refusals.
//!
//! Name: provisional, minted 2026-10-27 (UTC) by milestone/52-subshells.

use super::*;

#[test]
fn a_block_is_one_braced_group_and_nothing_after_it() {
    assert_eq!(body(b"{ ls }"), Ok(&b"ls"[..]));
    assert_eq!(body(b"{ cd a; ls; }"), Ok(&b"cd a; ls;"[..]));
    // Braces inside quotes are text.
    assert_eq!(body(b"{ echo '}' }"), Ok(&b"echo '}'"[..]));
    // Nested blocks keep their own braces.
    assert_eq!(
        body(b"{ scope { ls } ; pwd }"),
        Ok(&b"scope { ls } ; pwd"[..])
    );
    for bad in [
        &b""[..],
        b"ls",
        b"{",
        b"{ ls",
        b"{ }",
        b"{ ls } extra",
        b"{ ls } | wc",
        b"{ scope { ls }",
        b"{ls}",
    ] {
        assert_eq!(body(bad), Err(Refusal::ScopeMalformed), "{bad:?}");
    }
}

#[test]
fn a_brace_is_syntax_only_as_a_word_of_its_own() {
    assert_eq!(body(b"{ echo {x} }"), Ok(&b"echo {x}"[..]));
    assert_eq!(body(b"{ echo a}b }"), Ok(&b"echo a}b"[..]));
}

#[test]
fn a_job_change_is_refused_before_anything_runs() {
    for bad in [
        &b"{ kill %1 }"[..],
        b"{ kill -9 %2 }",
        b"{ wait }",
        b"{ fg }",
        b"{ bg %1 }",
        b"{ interrupt_heeder & }",
        b"{ cd a; wait %1 }",
        b"{ time kill %1 }",
        b"{ scope { wait } }",
    ] {
        assert_eq!(check(bad), Err(Refusal::ScopeCannotRestore), "{bad:?}");
    }
    // Reading the jobs table changes nothing.
    assert!(check(b"{ jobs }").is_ok());
    assert!(check(b"{ cd a && ls && pwd }").is_ok());
}

#[test]
fn nesting_stops_at_the_ceiling() {
    assert!(check(b"{ scope { scope { ls } } }").is_ok());
    assert_eq!(
        check(b"{ scope { scope { scope { ls } } } }"),
        Err(Refusal::ScopeTooDeep)
    );
}

#[test]
fn the_limit_the_message_quotes_is_the_constant() {
    let digits = [b'0' + MAXIMUM_DEPTH as u8];
    let digit = core::str::from_utf8(&digits).unwrap();
    assert!(
        Refusal::ScopeTooDeep.message().contains(digit),
        "{}",
        Refusal::ScopeTooDeep.message()
    );
}

#[test]
fn parentheses_open_a_group_only_at_the_start_of_a_command() {
    assert!(has_parenthesized_command(b"(cd /tmp"));
    assert!(has_parenthesized_command(b"  ( ls )"));
    assert!(has_parenthesized_command(b"ls | (wc)"));
    assert!(!has_parenthesized_command(b"echo (hi)"));
    assert!(!has_parenthesized_command(b"echo '(hi'"));
    assert!(!has_parenthesized_command(b"a || b"));
    assert!(!has_parenthesized_command(b"ls"));
}

#[test]
fn a_leading_pipe_or_a_doubled_one_is_read_exactly() {
    // A single `|` starts a stage; the first byte of the segment may be one.
    assert!(has_parenthesized_command(b"| (x)"));
    // The two bytes of `||` start no stage, whichever of them the scan is on.
    assert!(!has_parenthesized_command(b"a || (b)"));
    assert!(!has_parenthesized_command(b"a ||(b)"));
    assert!(has_parenthesized_command(b"a |(b)"));
}

#[test]
fn a_block_opens_after_any_word_boundary_not_only_at_the_line_start() {
    // `opens_block` reads the word before the brace, which starts after a space or a connector.
    for line in [
        &b"echo a ; scope { ls }"[..],
        b"echo a;scope { ls }",
        b"echo a&&scope { ls }",
        b"a || scope { ls }",
        b"a | scope { ls }",
        b"  scope { ls }",
    ] {
        let brace = line.iter().position(|&b| b == b'{').unwrap();
        assert!(opens_block(line, brace), "{line:?}");
    }
    for line in [
        &b"xscope { ls }"[..],
        b"echo scoped { ls }",
        b"echo a ; cope { ls }",
    ] {
        let brace = line.iter().position(|&b| b == b'{').unwrap();
        assert!(!opens_block(line, brace), "{line:?}");
    }
}

#[test]
fn inside_a_block_only_a_brace_word_counts() {
    // A brace stuck to other bytes is text, whichever way it faces.
    let line = b"scope { echo {x} ; echo a}b ; ls }";
    let mut depth = 0usize;
    let mut seen = 0;
    for i in 0..line.len() {
        match brace_step(line, i, depth) {
            1 => depth += 1,
            -1 => depth -= 1,
            _ => {}
        }
        seen += 1;
    }
    assert_eq!(seen, line.len());
    assert_eq!(depth, 0);
    // And the step itself: neither brace-shaped byte inside a word moves the depth.
    assert_eq!(brace_step(b"scope { {x} }", 8, 1), 0);
    assert_eq!(brace_step(b"scope { a}b }", 9, 1), 0);
    assert_eq!(brace_step(b"scope { { }", 8, 1), 1);
    assert_eq!(brace_step(b"scope { a }", 10, 1), -1);
}

#[test]
fn only_a_brace_after_scope_opens_a_block() {
    // The byte at the position must itself be the brace.
    assert!(!opens_block(b"scope x", 5));
    assert!(!opens_block(b"scope {x", 6));
    assert!(!opens_block(b"scope", 4));
}
