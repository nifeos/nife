//! Planning tests: `rm` and the directory it is granted, globbing, and programs that hear
//! words.

use crate::expand::{Expander, Expansion, MAX_NAMES};
use crate::test_support::{READS_A_FILE, WITH_DIRECTORY, plan, plan_against};
use crate::*;

// ---- `rm` is a program, and its operand grants a directory (milestone 47's rmdir lane) ----

/// **The headline at this level: naming something to remove grants the directory that holds
/// it.** `rm old.txt` hands `rm` a capability to the directory `old.txt` is in, plus that one
/// name, because taking a name away is an operation on a directory and no per-file capability
/// can express it.
///
/// The grant is therefore **wider than the name typed**, which is stated here rather than
/// hidden: the same capability could remove anything else in that directory. Narrowing it to
/// the names on the line is the nameset caretaker globbing needs, and it is the same shape.
#[test]
fn naming_something_to_remove_grants_the_directory_that_holds_it() {
    let Command::Run(r) = parse(b"rm old.txt") else {
        panic!()
    };
    let e = plan(&r, WITH_DIRECTORY).unwrap();
    assert_eq!(e.program, Program::Rm);
    let g = e
        .directory
        .expect("the operand did not become a directory grant");
    assert_eq!(g.names.only(), Some(&b"old.txt"[..]));
    assert_eq!(
        g.directory.depth(),
        0,
        "resolved where the shell was standing"
    );
    assert!(
        !g.subtree,
        "without -r the capability must not reach below the directory it was given",
    );
    assert!(e.file.is_none(), "rm takes no file capability, ever");
    assert_eq!((e.program_argument, e.memory_grant_pages), (0, 0));

    // A path operand is walked at the prompt, so the grant is the directory it landed in and
    // the child still only ever sees one component (§27 intact).
    let Command::Run(r) = parse(b"rm logs/2026/old.txt") else {
        panic!()
    };
    let g = plan(&r, WITH_DIRECTORY).unwrap().directory.unwrap();
    assert_eq!(g.names.only(), Some(&b"old.txt"[..]));
    assert_eq!(g.directory.depth(), 2);
    assert_eq!(g.directory.component(0), b"logs");
    assert_eq!(g.directory.component(1), b"2026");
}

/// **Typing `-r` is what hands over more**, and that is the property worth having a test for:
/// the recursion is not a branch in the program that a bug could take anyway. Without the
/// option the capability cannot walk or list, so a `rm` that tried would be told there is no
/// such name.
#[test]
fn the_recursion_option_is_what_widens_the_grant() {
    let narrow = {
        let Command::Run(r) = parse(b"rm logs") else {
            panic!()
        };
        plan(&r, WITH_DIRECTORY).unwrap()
    };
    let wide = {
        let Command::Run(r) = parse(b"rm -r logs") else {
            panic!()
        };
        plan(&r, WITH_DIRECTORY).unwrap()
    };
    assert_eq!(
        narrow.directory.unwrap().names,
        wide.directory.unwrap().names
    );
    assert!(!narrow.directory.unwrap().subtree);
    assert!(wide.directory.unwrap().subtree, "-r must widen the grant");
    assert_eq!(narrow.short_options, 0);
    assert_eq!(wide.short_options, rmopt::RECURSIVE);
}

/// A manifest whose recursion letter is not its first option, which no shipped program has:
/// `rm` declares `r` first, so its bit is zero, where `1 << bit` and every wrong spelling of it
/// agree. Through [`plan_against`], the door an external manifest (milestone 23) comes through.
const SWEEPS_A_TREE: Manifest = Manifest {
    file: FileGrantDeclaration::Forbidden,
    directory: DirectoryGrantDeclaration::Required {
        subtree_option: Some(b'r'),
    },
    // `r` deliberately second, so the widening reads bit 1.
    short_options: ShortOptions::new(b"vr"),
    ..READS_A_FILE
};

/// The subtree check reads the recursion letter's **own** bit out of the short-options word, wherever
/// the manifest declares it, and widening must follow that bit rather than position zero.
#[test]
fn the_recursion_letter_widens_by_its_own_bit_wherever_it_is_declared() {
    let Command::Run(r) = parse(b"sweep -r logs") else {
        panic!()
    };
    let e = plan_against(&r, Program::Rm, SWEEPS_A_TREE, WITH_DIRECTORY).unwrap();
    assert_eq!(e.short_options, 1 << 1);
    assert!(
        e.directory.unwrap().subtree,
        "-r declared at bit 1 must still widen the grant",
    );
}

/// The option letters and the bits `rmopt` names are one thing spelled twice (the manifest has
/// the letters, the program has the bits), so a change to either without the other has to fail
/// here in milliseconds. A cluster is one token, as every Unix `rm -rf` is typed.
#[test]
fn rm_options_are_numbered_by_their_position_in_its_manifest() {
    let Command::Run(r) = parse(b"rm -rfv doomed") else {
        panic!()
    };
    assert_eq!(r.options(), b"rfv");
    let e = plan(&r, WITH_DIRECTORY).unwrap();
    assert_eq!(
        e.short_options,
        rmopt::RECURSIVE | rmopt::FORCE | rmopt::VERBOSE
    );
    // Separately typed, in any order, is the same grant: options are a set.
    let Command::Run(r) = parse(b"rm -v -f -r doomed") else {
        panic!()
    };
    assert_eq!(
        plan(&r, WITH_DIRECTORY).unwrap().short_options,
        e.short_options
    );
    // And each letter alone lands on exactly its own bit.
    for (line, want) in [
        (&b"rm -r x"[..], rmopt::RECURSIVE),
        (b"rm -f x", rmopt::FORCE),
        (b"rm -v x", rmopt::VERBOSE),
    ] {
        let Command::Run(r) = parse(line) else {
            panic!()
        };
        assert_eq!(
            plan(&r, WITH_DIRECTORY).unwrap().short_options,
            want,
            "{}",
            core::str::from_utf8(line).unwrap(),
        );
    }
}

/// An option nothing declares is refused **at the prompt**, not passed along unread. It matters
/// more here than it looks: an option can widen a grant, so a shell that silently dropped one
/// would be a shell whose printed preview and actual transfer could disagree.
#[test]
fn an_option_the_manifest_does_not_declare_is_refused() {
    let Command::Run(r) = parse(b"rm -q doomed") else {
        panic!()
    };
    assert_eq!(plan(&r, WITH_DIRECTORY), Err(Refusal::NoSuchOption));
    // A cluster is refused whole, on its first unknown letter: `-rq` must not half-apply.
    let Command::Run(r) = parse(b"rm -rq doomed") else {
        panic!()
    };
    assert_eq!(plan(&r, WITH_DIRECTORY), Err(Refusal::NoSuchOption));
    // And a program that declares no options at all refuses every one.
    let Command::Run(r) = parse(b"least_authority_demo -r 9") else {
        panic!()
    };
    assert_eq!(plan(&r, WITH_DIRECTORY), Err(Refusal::NoSuchOption));
    assert!(
        !Refusal::NoSuchOption.message().contains("denied"),
        "a refusal is a fact about the command, never a policy",
    );
}

/// `rm` with nothing to remove, and `rm` in a shell that holds no directory. The second is the
/// milestone's headline refusal reaching a **shipped** program for the first time: until now no
/// program in the static table declared a grant the interactive shell cannot back.
#[test]
fn rm_needs_an_operand_and_something_to_narrow() {
    let Command::Run(r) = parse(b"rm") else {
        panic!()
    };
    assert_eq!(plan(&r, WITH_DIRECTORY), Err(Refusal::PathRequired));
    let Command::Run(r) = parse(b"rm -rf") else {
        panic!()
    };
    assert_eq!(
        plan(&r, WITH_DIRECTORY),
        Err(Refusal::PathRequired),
        "options are not operands: `rm -rf` names nothing",
    );

    let Command::Run(r) = parse(b"rm doomed") else {
        panic!()
    };
    assert_eq!(
        plan(&r, Holdings::default()),
        Err(Refusal::NoSuchCapability(CapabilityKind::File)),
        "a shell granted no directory holds nothing that could back this",
    );
}

/// The same shape refusals a file grant gets, because they are the same resolver: a name that
/// cannot travel in a grant is refused where it was typed, and `..` clamps at the shell's root
/// so a grant cannot name its way out **whether or not it started at the root**.
#[test]
fn a_directory_grant_cannot_be_named_out_of_the_shells_root() {
    for (line, want) in [
        (&b"rm /.."[..], Refusal::FileNotNameable),
        (b"rm /../passwd", Refusal::FileNotNameable),
        (b"rm ..", Refusal::FileNotNameable),
        (b"rm ../motd", Refusal::FileNotNameable),
        (
            b"rm this-name-is-far-too-long.txt",
            Refusal::FileNotNameable,
        ),
    ] {
        let Command::Run(r) = parse(line) else {
            panic!()
        };
        assert_eq!(
            plan(&r, WITH_DIRECTORY),
            Err(want),
            "{}",
            core::str::from_utf8(line).unwrap(),
        );
    }
}

/// **One operand, and a second is refused rather than ignored.** `rm a b` is a real Unix line
/// and this shell does not run it: a manifest grants one directory, and two names in two
/// different directories are two grants. That is the same widening globbing needs (`rm *.txt`
/// is the same question), so it waits for the same answer instead of getting a private one.
#[test]
fn rm_takes_one_operand_because_a_manifest_grants_one_directory() {
    let Command::Run(r) = parse(b"rm a b") else {
        panic!()
    };
    assert_eq!(plan(&r, WITH_DIRECTORY), Err(Refusal::Unexpected));
    assert!(
        !Refusal::Unexpected.message().contains("no such"),
        "the second name is unplaceable, not unknown",
    );
}

/// The directory grant is resolved **at grant time** and recorded as a value, exactly as a file
/// grant is, so a `cd` afterwards cannot change what an already-planned `rm` would take away.
#[test]
fn a_directory_grant_is_fixed_where_it_was_planned() {
    let mut holds = WITH_DIRECTORY;
    assert!(holds.cwd.apply(nav::path(b"logs").unwrap().steps()).is_ok());
    let Command::Run(r) = parse(b"rm -r old") else {
        panic!()
    };
    let g = plan(&r, holds).unwrap().directory.unwrap();
    assert_eq!(g.directory.depth(), 1);
    assert_eq!(g.directory.component(0), b"logs");
    holds.cwd.ascend();
    assert_eq!(g.directory.depth(), 1, "the grant followed the shell");
}

// ---- globbing: the expansion is the grant (milestone 47's globbing lane) ----

/// A directory as the shell would enumerate it: two names the pattern takes, one it does not,
/// and a subdirectory it does not either. The last two are the controls, and they are what make
/// every claim below a statement about *which* names rather than about how many.
const LISTING: [(&[u8], bool); 4] = [
    (b"one.txt", false),
    (b"two.txt", false),
    (b"three.log", false),
    (b"logs", true),
];

/// Expand a pattern over [`LISTING`], the way both `echo` and the shell's grant path do.
fn expanded(pattern: &[u8]) -> Result<expand::NameSet, Refusal> {
    let mut e = Expander::new(pattern);
    for (name, is_dir) in LISTING {
        e.offer(name, is_dir);
    }
    e.finish()
}

/// **What `echo *.txt` shows is exactly what `rm *.txt` transfers.** The planner grants the set the
/// expander produced, unnarrowed, unreordered and with nothing added, and that set is not
/// everything in the directory. The guest half compares the two over a real `READDIR`
/// (`system_tests::user::glob_grant_tests`).
#[test]
fn what_echo_shows_is_what_rm_would_transfer() {
    let shown = expanded(b"*.txt").expect("the pattern matches two of these names");

    let Command::Run(r) = parse(b"rm *.txt") else {
        panic!("`rm *.txt` must parse as a program invocation")
    };
    let granted = super::plan(&r, WITH_DIRECTORY, Expansion::at(0, shown))
        .unwrap()
        .directory
        .expect("the pattern did not become a directory grant")
        .names;

    assert_eq!(granted, shown, "the grant is not what the expansion showed");
    assert_eq!(granted.len(), 2);
    assert!(granted.contains(b"one.txt") && granted.contains(b"two.txt"));
    assert!(
        !granted.contains(b"three.log") && !granted.contains(b"logs"),
        "the grant reached a name in the directory that the pattern did not match",
    );
    // And `caps rm *.txt` previews the identical endowment, because the preview re-parses the
    // command you would have typed and plans it against the same expansion.
    let Command::Caps(tail) = parse(b"caps rm *.txt") else {
        panic!()
    };
    let Command::Run(previewed) = parse(tail) else {
        panic!()
    };
    assert_eq!(
        super::plan(&previewed, WITH_DIRECTORY, Expansion::at(0, shown)),
        super::plan(&r, WITH_DIRECTORY, Expansion::at(0, shown)),
    );
}

/// **A literal operand is the set of one**, which is the whole of "this generalizes
/// `fs_file_caretaker` rather than replacing it": the same grant shape carries both, and a name
/// that was typed designates itself whatever anybody hands the planner alongside it.
#[test]
fn a_literal_operand_is_the_set_of_one_and_ignores_any_expansion() {
    let Command::Run(r) = parse(b"rm old.txt") else {
        panic!()
    };
    let names = plan(&r, WITH_DIRECTORY).unwrap().directory.unwrap().names;
    assert_eq!(names.len(), 1);
    assert_eq!(names.only(), Some(&b"old.txt"[..]));

    // A set offered for a token with no magic in it must not be able to replace what was typed.
    // Nothing should ever do this; the point is that it cannot change the grant if it does.
    let stray = expanded(b"*.txt").unwrap();
    assert_eq!(
        super::plan(&r, WITH_DIRECTORY, Expansion::at(0, stray))
            .unwrap()
            .directory
            .unwrap()
            .names,
        names,
        "a typed name must designate itself, whatever expansion is offered with it",
    );
}

/// **A pattern never becomes a name.** Without an expansion behind it the plan refuses, rather
/// than granting a capability for a name with a `*` in it: that name is well-formed here
/// (nothing refuses `*` in a component), so the grant would be silently useless today and live
/// the moment anything created a file called `*.txt`.
#[test]
fn a_pattern_with_nothing_to_expand_it_against_is_refused_not_taken_literally() {
    let Command::Run(r) = parse(b"rm *.txt") else {
        panic!()
    };
    assert_eq!(plan(&r, WITH_DIRECTORY), Err(Refusal::Unexpanded));
    assert!(
        file_name_fits(b"*.txt"),
        "the refusal above is the ONLY thing stopping a pattern becoming a grantable name",
    );
    // In a shell holding no directory the bigger fact wins: there is nothing to narrow, and
    // therefore nothing to expand against either.
    assert_eq!(
        plan(&r, Holdings::default()),
        Err(Refusal::NoSuchCapability(CapabilityKind::File)),
    );
}

/// The two costs the roadmap said to design rather than discover, at the prompt.
#[test]
fn an_empty_match_and_an_oversized_one_are_both_refused_before_anything_spawns() {
    let Command::Run(r) = parse(b"rm *.rs") else {
        panic!()
    };
    // Nothing matched. The expander refuses first, and the planner refuses an empty set handed
    // to it anyway, so there are two doors and neither leads to a spawn.
    assert_eq!(expanded(b"*.rs"), Err(Refusal::NoMatch));
    assert_eq!(
        super::plan(
            &r,
            WITH_DIRECTORY,
            Expansion::at(0, expand::NameSet::empty())
        ),
        Err(Refusal::NoMatch),
    );
    // Too many matched: the grant is refused whole. Sixteen of seventeen names would have been
    // a preview and a transfer that disagreed.
    let mut over = Expander::new(b"*");
    let mut names = [[b'n'; 4]; MAX_NAMES + 1];
    for (i, n) in names.iter_mut().enumerate() {
        n[2] = b'0' + (i / 10) as u8;
        n[3] = b'0' + (i % 10) as u8;
    }
    for n in &names {
        over.offer(&n[..], false);
    }
    assert_eq!(over.finish(), Err(Refusal::TooManyNames));
    assert!(
        Refusal::TooManyNames.message().contains("8"),
        "a bound the prompt will not name is a bound nobody can work around",
    );
}

/// A pattern lives in the last component or nowhere, and a program granted **one file** needs a
/// pattern that designates one name. Both are facts about what was designated; neither is a
/// permission.
#[test]
fn a_pattern_is_refused_where_it_cannot_mean_one_thing() {
    let Command::Run(r) = parse(b"rm */old.txt") else {
        panic!()
    };
    assert_eq!(plan(&r, WITH_DIRECTORY), Err(Refusal::PatternInPath));

    // A file grant takes one name, so a pattern that matched two has designated something this
    // program cannot be handed.
    let Command::Run(r) = parse(b"wc *.txt") else {
        panic!()
    };
    assert_eq!(
        super::plan_against(
            &r,
            Program::LeastAuthorityDemo,
            READS_A_FILE,
            WITH_DIRECTORY,
            Expansion::at(0, expanded(b"*.txt").unwrap()),
        ),
        Err(Refusal::AmbiguousFile),
    );
    // And one that matched exactly one name is a perfectly good way to name it.
    let Command::Run(r) = parse(b"wc *.log") else {
        panic!()
    };
    let g = super::plan_against(
        &r,
        Program::LeastAuthorityDemo,
        READS_A_FILE,
        WITH_DIRECTORY,
        Expansion::at(0, expanded(b"*.log").unwrap()),
    )
    .unwrap()
    .file
    .unwrap();
    assert_eq!(g.name.as_bytes(), b"three.log");
}

/// A pattern operand is resolved against the shell's position at **plan time**, exactly as a
/// typed name is, and the leading path is walked here rather than passed on.
#[test]
fn a_pattern_is_resolved_where_the_shell_was_standing() {
    let mut holds = WITH_DIRECTORY;
    assert!(holds.cwd.apply(nav::path(b"logs").unwrap().steps()).is_ok());
    let Command::Run(r) = parse(b"rm -r 2026/*.txt") else {
        panic!()
    };
    let g = super::plan(&r, holds, Expansion::at(0, expanded(b"*.txt").unwrap()))
        .unwrap()
        .directory
        .unwrap();
    assert_eq!(g.directory.depth(), 2);
    assert_eq!(g.directory.component(0), b"logs");
    assert_eq!(g.directory.component(1), b"2026");
    assert!(g.subtree, "-r still widens a set grant");
    holds.cwd.ascend();
    assert_eq!(g.directory.depth(), 2, "the grant followed the shell");
}

/// The set bound is one number spelled in two crates, for [`MAX_FILE_NAME`]'s reason: `grant_plan`
/// must be able to check a command line without linking the filesystem contract, so the pair is
/// pinned here rather than shared.
#[test]
fn the_set_bound_matches_the_contract_that_carries_the_set() {
    assert_eq!(MAX_NAMES, filesystem_contract::nameset::MAX_NAMES);
    assert_eq!(expand::MAX_NAME, filesystem_contract::grant::MAX_NAME);
    // And the widest set this crate can plan must fit the buffer that contract sizes for it.
    let widest: [(&[u8], bool); MAX_NAMES] = [(b"sixteen-bytes!!!", false); MAX_NAMES];
    let mut buf = [0u8; filesystem_contract::nameset::BYTES];
    assert!(filesystem_contract::nameset::encode(&widest, &mut buf).is_some());
}

/// **A program that hears words places no positional grant** (milestone 205, DECISIONS §170).
/// Its words carry no authority and all of them go to its argv, so a positional file,
/// directory or input slot would have nothing left to fill, and an option letter the shell
/// validated would be one the program never sees. Only `std` reads an argv.
#[test]
fn a_program_that_hears_words_places_no_positional_grant() {
    let mut hearing = 0;
    for &p in Program::ALL {
        let m = p.manifest();
        if !m.program_argument.hears_words() {
            continue;
        }
        hearing += 1;
        let name = p.name();
        assert_eq!(
            m.startup,
            StartupContract::Std,
            "{name}: only std reads an argv"
        );
        assert_eq!(m.file, FileGrantDeclaration::Forbidden, "{name}");
        assert_eq!(m.directory, DirectoryGrantDeclaration::Forbidden, "{name}");
        assert_eq!(m.input, InputDeclaration::Forbidden, "{name}");
        assert!(
            m.short_options.letters().is_empty(),
            "{name}: its options are its own"
        );
    }
    assert!(hearing > 0, "a sweep over nothing proves nothing");
}

/// **The planner lets a program that hears words have its whole line** (§170): a flag-shaped
/// token is not `Unexpected`, a letter is not `NoSuchOption`, and five words are not past
/// `MAX_POSITIONALS`. Nothing is placed, so the endowment carries no argument and no short options.
#[test]
fn a_program_that_hears_words_is_planned_whatever_its_words_are() {
    let e = plan(
        &parse_run(b"std_exerciser -xyz --long a b c d e f"),
        WITH_DIRECTORY,
    )
    .unwrap();
    assert_eq!(e.program, Program::StdExerciser);
    assert_eq!(
        (e.program_argument, e.short_options, e.file, e.directory),
        (0, 0, None, None)
    );
    // And a line `argv` would refuse is refused here, at the prompt, before anything is sent.
    assert_eq!(
        plan(&parse_run(b"std_exerciser *.rs"), WITH_DIRECTORY),
        Err(Refusal::PatternInArguments)
    );
}
