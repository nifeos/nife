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
    assert_eq!(body(b"{ scope { ls } ; pwd }"), Ok(&b"scope { ls } ; pwd"[..]));
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
    assert!(has_parenthesised_command(b"(cd /tmp"));
    assert!(has_parenthesised_command(b"  ( ls )"));
    assert!(has_parenthesised_command(b"ls | (wc)"));
    assert!(!has_parenthesised_command(b"echo (hi)"));
    assert!(!has_parenthesised_command(b"echo '(hi'"));
    assert!(!has_parenthesised_command(b"a || b"));
    assert!(!has_parenthesised_command(b"ls"));
}
