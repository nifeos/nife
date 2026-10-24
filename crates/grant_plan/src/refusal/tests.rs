//! Tests of the refusal lines: the voice every one keeps, and the limits five of them quote.

use super::*;

/// Every variant, once. A variant added to [`Refusal`] and not here is untested, so this list sits
/// beside the enum it mirrors.
const ALL: &[Refusal] = &[
    Refusal::NoSuchProgram,
    Refusal::NoSuchCapability(CapabilityKind::File),
    Refusal::FileForbidden,
    Refusal::FileRequired,
    Refusal::PathRequired,
    Refusal::NoSuchOption,
    Refusal::FileNotNameable,
    Refusal::MemoryGrantForbidden,
    Refusal::MemoryGrantRequired,
    Refusal::MemoryGrantOutOfRange { min: 1, max: 2 },
    Refusal::ProgramArgumentRequired,
    Refusal::ProgramArgumentForbidden,
    Refusal::NoMatch,
    Refusal::TooManyNames,
    Refusal::MatchNotNameable,
    Refusal::PatternInPath,
    Refusal::Unexpanded,
    Refusal::AmbiguousFile,
    Refusal::NoRedirectTarget,
    Refusal::RedirectRepeated,
    Refusal::RedirectMidPipeline,
    Refusal::WordAfterRedirect,
    Refusal::EmptyStage,
    Refusal::TooManyStages,
    Refusal::PatternInRedirect,
    Refusal::NotAByteStream,
    Refusal::NoDiagnosticStream,
    Refusal::InputForbidden,
    Refusal::InputRequired,
    Refusal::NoReaderButThisShell,
    Refusal::NotAPipelineStage,
    Refusal::Unexpected,
    Refusal::UnclosedQuote,
    Refusal::PartlyQuoted,
    Refusal::EmptySegment,
    Refusal::TooManySegments,
    Refusal::PatternInArguments,
    Refusal::ArgumentsTooLong,
];

/// **No two refusals read alike, and none reads like a permission check.** The comments in
/// [`Refusal::message`] give the voice: a line says what the command designates, never what was
/// denied. Two variants sharing a line would leave the user unable to tell which rule fired.
#[test]
fn every_refusal_has_its_own_line_and_none_says_denied() {
    for (i, a) in ALL.iter().enumerate() {
        let m = a.message();
        assert!(!m.is_empty(), "{a:?} has an empty line");
        for word in ["denied", "permission"] {
            assert!(
                !m.contains(word),
                "{a:?} reads like a permission check: {m}"
            );
        }
        for b in &ALL[i + 1..] {
            assert_ne!(m, b.message(), "{a:?} and {b:?} print the same line");
        }
    }
}

/// `n` in decimal with a comma every three digits, as the lines spell it (`4,080`).
fn spelled(mut n: usize, buf: &mut [u8; 32]) -> &str {
    let mut at = buf.len();
    let mut digits = 0;
    loop {
        if digits > 0 && digits % 3 == 0 {
            at -= 1;
            buf[at] = b',';
        }
        at -= 1;
        buf[at] = b'0' + (n % 10) as u8;
        digits += 1;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    core::str::from_utf8(&buf[at..]).unwrap()
}

/// **A limit a line quotes is the limit the code enforces.** Each of these five strings states a
/// number as text, and nothing else ties that text to the constant, so a ceiling raised in one
/// place would leave the prompt quoting the old one. The segment ceiling `TooManySegments` quotes
/// lives in the shell (`swish::sequence::MAX_SEGMENTS`), which this crate cannot see.
#[test]
fn a_quoted_limit_is_the_constant_behind_it() {
    let cases: &[(Refusal, &str, usize, &str)] = &[
        (
            Refusal::FileNotNameable,
            "at most {} bytes a file",
            crate::MAX_FILE_NAME,
            "MAX_FILE_NAME",
        ),
        (
            Refusal::FileNotNameable,
            "{} a directory",
            crate::nav::MAX_DIRECTORY_NAME,
            "nav::MAX_DIRECTORY_NAME",
        ),
        (
            Refusal::FileNotNameable,
            "{} deep",
            crate::nav::MAX_DEPTH,
            "nav::MAX_DEPTH",
        ),
        (
            Refusal::TooManyNames,
            "at most {},",
            crate::expand::MAX_NAMES,
            "expand::MAX_NAMES",
        ),
        (
            Refusal::TooManyNames,
            ", {} bytes of name",
            crate::expand::NAME_BYTES,
            "expand::NAME_BYTES",
        ),
        (
            Refusal::MatchNotNameable,
            "at most {} bytes",
            crate::expand::MAX_NAME,
            "expand::MAX_NAME",
        ),
        (
            Refusal::TooManyStages,
            "at most {}",
            crate::line::MAX_STAGES,
            "line::MAX_STAGES",
        ),
        (
            Refusal::ArgumentsTooLong,
            "({} bytes",
            program_argument_contract::CAPACITY,
            "program_argument_contract::CAPACITY",
        ),
    ];
    for &(refusal, shape, limit, name) in cases {
        let mut buf = [0u8; 32];
        let number = spelled(limit, &mut buf);
        let (before, after) = shape.split_once("{}").unwrap();
        let m = refusal.message();
        let found = m
            .match_indices(number)
            .any(|(i, _)| m[..i].ends_with(before) && m[i + number.len()..].starts_with(after));
        assert!(
            found,
            "{refusal:?} should quote {name} ({number}) as \"{shape}\": {m}"
        );
    }
}
