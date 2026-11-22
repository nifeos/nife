//! Tests of the refusal lines: the voice every one keeps, and the limits five of them quote.

use super::*;

/// Every variant, once. A variant added to [`Refusal`] and not here is untested, so this list sits
/// beside the enum it mirrors.
const ALL: &[Refusal] = &[
    Refusal::NoSuchProgram,
    Refusal::NoSuchCapability(CapabilityKind::File),
    Refusal::FileForbidden,
    Refusal::FileRequired,
    Refusal::DirectoryRequired,
    Refusal::NoSuchOption,
    Refusal::FileNotNameable,
    Refusal::MemoryGrantForbidden,
    Refusal::MemoryGrantRequired,
    Refusal::MemoryGrantOutOfRange {
        minimum: 1,
        maximum: 2,
    },
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
    Refusal::NoReaderStage,
    Refusal::NotAPipelineStage,
    Refusal::Unexpected,
    Refusal::UnclosedQuote,
    Refusal::PartlyQuoted,
    Refusal::EmptySegment,
    Refusal::TooManySegments,
    Refusal::PatternInArguments,
    Refusal::ArgumentsTooLong,
    Refusal::ScopeMalformed,
    Refusal::ScopeTooDeep,
    Refusal::ScopeCannotRestore,
    Refusal::GroupingParentheses,
    Refusal::IsolationNotBuilt,
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
/// lives in the shell (`swish::sequence::MAXIMUM_SEGMENTS`), which this crate cannot see.
#[test]
fn a_quoted_limit_is_the_constant_behind_it() {
    let cases: &[(Refusal, &str, usize, &str)] = &[
        (
            Refusal::FileNotNameable,
            "at most {} bytes a name",
            crate::MAXIMUM_FILE_NAME_BYTES,
            "MAXIMUM_FILE_NAME_BYTES",
        ),
        (
            Refusal::FileNotNameable,
            "{} bytes a path",
            crate::navigation::MAXIMUM_PATH_BYTES,
            "navigation::MAXIMUM_PATH_BYTES",
        ),
        (
            Refusal::FileNotNameable,
            "{} deep",
            crate::navigation::MAXIMUM_DEPTH,
            "navigation::MAXIMUM_DEPTH",
        ),
        (
            Refusal::TooManyNames,
            "at most {},",
            crate::expansion::MAXIMUM_NAMES,
            "expansion::MAXIMUM_NAMES",
        ),
        (
            Refusal::TooManyNames,
            ", {} bytes of name",
            crate::expansion::MAXIMUM_NAME_SET_BYTES,
            "expansion::MAXIMUM_NAME_SET_BYTES",
        ),
        (
            Refusal::MatchNotNameable,
            "at most {} bytes",
            crate::expansion::MAXIMUM_NAME_BYTES,
            "expansion::MAXIMUM_NAME_BYTES",
        ),
        (
            Refusal::TooManyStages,
            "at most {}",
            crate::line::MAXIMUM_STAGES,
            "line::MAXIMUM_STAGES",
        ),
        (
            Refusal::ArgumentsTooLong,
            "({} bytes",
            program_argument_contract::MAXIMUM_RECORDS_BYTES,
            "program_argument_contract::MAXIMUM_RECORDS_BYTES",
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

/// **A sentence that names a number names the constant's current value.** The sentences are fixed
/// text because this crate cannot format at run time without `alloc`, and the other tests pin the
/// text exactly, which catches a drift of the sentence and not a drift of the constant. This
/// catches the constant: raise one and the prompt would otherwise keep telling the old number.
#[test]
fn every_number_a_refusal_names_is_its_constants_value() {
    extern crate std;
    use std::string::ToString;

    let says = |sentence: &str, number: usize| {
        let text = number.to_string();
        let grouped = if number >= 1000 {
            std::format!("{},{:03}", number / 1000, number % 1000)
        } else {
            text.clone()
        };
        assert!(
            sentence.contains(&text) || sentence.contains(&grouped),
            "`{sentence}` does not say {number}"
        );
    };

    let refused_file_name = Refusal::FileNotNameable.message();
    says(refused_file_name, crate::MAXIMUM_FILE_NAME_BYTES);
    says(refused_file_name, crate::navigation::MAXIMUM_PATH_BYTES);
    says(refused_file_name, crate::navigation::MAXIMUM_DEPTH);
    let too_many = Refusal::TooManyNames.message();
    says(too_many, crate::expansion::MAXIMUM_NAMES);
    says(too_many, crate::expansion::MAXIMUM_NAME_SET_BYTES);
    says(
        Refusal::MatchNotNameable.message(),
        crate::MAXIMUM_FILE_NAME_BYTES,
    );
    says(
        Refusal::TooManyStages.message(),
        crate::line::MAXIMUM_STAGES,
    );
    says(
        Refusal::ArgumentsTooLong.message(),
        program_argument_contract::MAXIMUM_RECORDS_BYTES,
    );
    says(
        crate::navigation::PathRefusal::NotAName.message(),
        crate::MAXIMUM_FILE_NAME_BYTES,
    );
    says(
        crate::navigation::PathRefusal::TooDeep.message(),
        crate::navigation::MAXIMUM_DEPTH,
    );
    says(
        crate::navigation::PathRefusal::PathTooLong.message(),
        crate::navigation::MAXIMUM_PATH_BYTES,
    );
    says(
        crate::navigation::BindRefusal::NotAName.message(),
        crate::navigation::MAXIMUM_BOUND_NAME_BYTES,
    );
    says(
        crate::navigation::BindRefusal::TooMany.message(),
        crate::navigation::MAXIMUM_BINDS,
    );
}
