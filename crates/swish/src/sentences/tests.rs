//! Tests of the fixed sentences, `help`, and the outcome of a spawn.

use grant_plan::nav::Cwd;

use super::*;
use crate::test_support::{endowment, shown, spec_of};

/// **A vouch's answer names what is live afterwards** (DECISIONS §221), and a refusal says
/// nothing changed in words distinct from a success. `script/swish-check` asserts the first.
#[test]
fn a_vouch_answer_says_what_is_live() {
    use spawnproto::ActivationStatus as S;
    assert_eq!(
        shown(|o| write_vouch(S::Done, 3, o)),
        "  vouched; generation 3 is live\n"
    );
    assert_eq!(
        shown(|o| write_vouch(S::NotExecutable, 0, o)),
        "  refused: those bytes are not a program this machine runs; nothing is installed\n"
    );
    assert_eq!(
        shown(|o| write_vouch(S::StoreFailed, 2, o)),
        "  could not write the installed programs; generation 2 is live\n"
    );
}

#[test]
fn pwd_prints_the_position_relative_to_this_shells_own_root() {
    // The leading slash is this shell's root, not the system's. A shell holding one directory
    // capability has no name for anything above it, so `/` at depth zero is the honest answer
    // rather than a borrowed one, and `pwd` is the only place a user sees it.
    assert_eq!(shown(|o| write_pwd(&Holdings::default(), o)), "  /\n");

    let mut cwd = Cwd::root();
    assert!(cwd.descend(b"docs"));
    assert!(cwd.descend(b"drafts"));
    let holdings = Holdings {
        dir: true,
        cwd,
        ..Holdings::default()
    };
    assert_eq!(shown(|o| write_pwd(&holdings, o)), "  /docs/drafts\n");
}

#[test]
fn an_unresolvable_name_is_repeated_back_and_a_flag_only_line_is_not() {
    let named = shown(|o| write_refusal(&spec_of(b"cat 7"), Refusal::NoSuchProgram, o));
    assert!(named.starts_with("  cat: "), "{named}");

    // `--mem 16` names no program, and "  : the named program does not exist" would be worse
    // than the bare sentence.
    let flags_only = shown(|o| write_refusal(&spec_of(b"--mem 16"), Refusal::NoSuchProgram, o));
    assert!(!flags_only.contains(": "), "{flags_only}");
    assert!(flags_only.ends_with('\n'));
}

#[test]
fn a_refusal_about_a_real_program_uses_the_canonical_name() {
    // The name the manifest carries, not the bytes typed, because the manifest is what refused.
    let s = shown(|o| write_refusal(&spec_of(b"least_authority_demo"), Refusal::ArgRequired, o));
    assert!(s.starts_with("  least_authority_demo: "), "{s}");
    // Nothing to prefix when the program did not resolve, whatever the refusal.
    let s = shown(|o| write_refusal(&spec_of(b"nope x"), Refusal::FileForbidden, o));
    assert!(!s.contains("nope"), "{s}");
}

#[test]
fn a_spawn_that_failed_says_so_whatever_the_program_was() {
    // The sentinel is checked before the program is looked at, which matters: `least_authority_demo`'s arm
    // would otherwise report that a process computed `u64::MAX`.
    for prog in [
        Program::LeastAuthorityDemo,
        Program::MemoryGrantDepleter,
        Program::Date,
    ] {
        let s = shown(|o| write_outcome(&endowment(prog), spawnproto::SPAWN_FAILED, o));
        assert!(s.contains("could not spawn"), "{prog:?}: {s}");
    }
}

#[test]
fn the_two_programs_that_answer_with_a_number_report_it_in_their_own_terms() {
    let mut e = endowment(Program::LeastAuthorityDemo);
    e.arg = 7;
    assert_eq!(
        shown(|o| write_outcome(&e, 49, o)),
        "  a process at EL0 computed 7*7 = 49\n"
    );

    let mut e = endowment(Program::MemoryGrantDepleter);
    e.mem_pages = 16;
    let s = shown(|o| write_outcome(&e, 14, o));
    // Both numbers, because the gap between them is the page tables the grant paid for, and a
    // reader who sees only one of them cannot tell that the grant was honored.
    assert!(s.contains("mapped 14 pages"), "{s}");
    assert!(s.contains("16-page budget"), "{s}");
}

#[test]
fn a_program_that_reports_elsewhere_prints_nothing_here() {
    // Text-answering and supervised programs are drained by other readers. A line printed here
    // would be a second, empty report for the same run.
    for prog in [
        Program::Date,
        Program::Wc,
        Program::Rm,
        Program::InterruptHeeder,
        Program::InterruptIgnorer,
    ] {
        assert_eq!(shown(|o| write_outcome(&endowment(prog), 0, o)), "");
    }
}

/// **A program that answers in a register gets a sentence here, or the answer is lost**
/// (milestone 150). [`write_outcome`]'s match ends in a wildcard, because byte-stream programs
/// have nothing to render there; this is what keeps that wildcard from swallowing the one case
/// where it would be wrong. A new `OutputSpec::Words` program with no arm prints nothing at the
/// prompt, which is the silence this fails on.
#[test]
fn every_program_that_answers_in_words_renders_its_answer() {
    let mut checked = 0;
    for &p in Program::ALL {
        if p.manifest().output != OutputSpec::Words {
            continue;
        }
        let s = shown(|o| write_outcome(&endowment(p), 441, o));
        assert!(
            s.contains("441"),
            "{} answers in a register and write_outcome has no arm to print it:\n{s:?}",
            p.name()
        );
        checked += 1;
    }
    assert!(checked > 0, "a sweep over nothing proves nothing");
}

#[test]
fn help_names_every_verb_the_parser_accepts() {
    // The drift this catches is the cheap one to introduce and the expensive one to notice: a
    // builtin added to the parser and not to the help, which is then a feature nobody at the
    // prompt can find.
    let text = shown(write_help);
    for verb in [
        "help", "echo", "caps", "time", "xargs", "cd", "pwd", "ls", "mkdir", "touch", "bind",
    ] {
        assert!(text.contains(verb), "help does not mention {verb}");
        // And it really is a verb this shell answers, rather than a word in a sentence.
        assert!(
            !matches!(grant_plan::parse(verb.as_bytes()), Command::Run(_)),
            "{verb} is not a builtin any more; the help is stale"
        );
    }
    for op in ["a | b", "a > name", "a >> name", "a < name", "a 2> name"] {
        assert!(text.contains(op), "help does not mention {op}");
    }
}
