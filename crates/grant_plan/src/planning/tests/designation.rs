//! Planning tests: which slot a token fills, what a name designates, and what the shell must
//! hold to grant it.

use crate::expand::{Expansion, Name};
use crate::line::Source;
use crate::test_support::{READS_A_FILE, WITH_DIR, at, mounted, plan, plan_against, plan_line};
use crate::*;

#[test]
fn a_bare_program_name_spawns_it() {
    // What `run least_authority_demo 9` used to say. The name is the command; the manifest places the 9.
    let Command::Run(r) = parse(b"least_authority_demo 9") else {
        panic!("not an invocation")
    };
    assert_eq!(r.prog, b"least_authority_demo");
    assert_eq!(r.positionals(), [&b"9"[..]]);
    assert_eq!(r.mem, None);
    let e = plan(&r, Holdings::default()).unwrap();
    assert_eq!(e.prog, Prog::LeastAuthorityDemo);
    assert_eq!(e.arg, 9);
    assert_eq!(e.mem_pages, 0);
    assert!(e.reports);
}

#[test]
fn least_authority_demo_needs_an_integer_argument() {
    let Command::Run(r) = parse(b"least_authority_demo") else {
        panic!()
    };
    assert_eq!(plan(&r, Holdings::default()), Err(Refusal::ArgRequired));
    // And a token that cannot be an integer is a missing argument, not a file: least_authority_demo's
    // manifest declares no file, so there is no other slot the word could have meant.
    let Command::Run(r) = parse(b"least_authority_demo eight") else {
        panic!()
    };
    assert_eq!(plan(&r, Holdings::default()), Err(Refusal::ArgRequired));
}

#[test]
fn least_authority_demo_refuses_a_memory_grant_whichever_side_the_flag_is_typed() {
    // `--mem` survives the grammar change as an ordinary flag, and with the verb gone it reads
    // where a Unix user would type it: after the command name. Both spellings are the same
    // grant, so both must reach the same refusal.
    for line in [
        &b"least_authority_demo 3 --mem 8"[..],
        b"--mem 8 least_authority_demo 3",
    ] {
        let Command::Run(r) = parse(line) else {
            panic!()
        };
        assert_eq!(r.prog, b"least_authority_demo");
        assert_eq!(
            plan(&r, Holdings::default()),
            Err(Refusal::MemForbidden),
            "{}",
            core::str::from_utf8(line).unwrap(),
        );
    }
}

#[test]
fn memory_grant_depleter_needs_its_grant() {
    let Command::Run(r) = parse(b"memory_grant_depleter") else {
        panic!()
    };
    assert_eq!(plan(&r, Holdings::default()), Err(Refusal::MemRequired));
}

#[test]
fn memory_grant_depleter_with_mem_plans_the_grant() {
    let Command::Run(r) = parse(b"memory_grant_depleter --mem 16") else {
        panic!()
    };
    let e = plan(&r, Holdings::default()).unwrap();
    assert_eq!(e.prog, Prog::MemoryGrantDepleter);
    assert_eq!(e.mem_pages, 16);
    assert_eq!(e.arg, 0);
}

#[test]
fn memory_grant_depleter_mem_out_of_range() {
    let Command::Run(r) = parse(b"memory_grant_depleter --mem 999") else {
        panic!()
    };
    assert_eq!(
        plan(&r, Holdings::default()),
        Err(Refusal::MemOutOfRange { min: 1, max: 64 })
    );
    let Command::Run(r0) = parse(b"memory_grant_depleter --mem 0") else {
        panic!()
    };
    assert_eq!(
        plan(&r0, Holdings::default()),
        Err(Refusal::MemOutOfRange { min: 1, max: 64 })
    );
}

/// The declared range is inclusive at both ends: a grant of exactly `min` or exactly `max`
/// pages plans, and only one past either edge refuses.
#[test]
fn a_memory_grant_at_either_end_of_the_declared_range_plans() {
    for (line, pages) in [
        (&b"memory_grant_depleter --mem 1"[..], 1),
        (b"memory_grant_depleter --mem 64", 64),
    ] {
        let Command::Run(r) = parse(line) else {
            panic!()
        };
        assert_eq!(
            plan(&r, Holdings::default()).unwrap().mem_pages,
            pages,
            "{}",
            core::str::from_utf8(line).unwrap(),
        );
    }
}

#[test]
fn memory_grant_depleter_takes_no_argument() {
    let Command::Run(r) = parse(b"memory_grant_depleter --mem 8 5") else {
        panic!()
    };
    assert_eq!(plan(&r, Holdings::default()), Err(Refusal::ArgForbidden));
}

#[test]
fn unknown_program_is_refused_by_name() {
    let Command::Run(r) = parse(b"frobnicate 1") else {
        panic!()
    };
    assert_eq!(plan(&r, Holdings::default()), Err(Refusal::NoSuchProgram));
    // A mistyped builtin lands here too, now that the first word is either a builtin or a
    // program, so the line has to point at both halves of what the prompt understands.
    let Command::Run(r) = parse(b"hlep") else {
        panic!()
    };
    let msg = plan(&r, Holdings::default()).unwrap_err().message();
    assert!(msg.contains("no such program"), "{msg}");
    assert!(msg.contains("help"), "{msg}");
}

#[test]
fn a_bare_unplaceable_token_is_still_refused() {
    // **The safety property the `file:` prefix was credited with, proven without it.** The old
    // note argued the prefix stopped a stray word from becoming a capability transfer. It did
    // not: the manifest did. `least_authority_demo 5 extra` has no slot for `extra` because least_authority_demo declares
    // `FileSpec::Forbidden`, so nothing is granted and the prompt says why.
    let Command::Run(r) = parse(b"least_authority_demo 5 extra") else {
        panic!()
    };
    assert_eq!(r.positionals(), [&b"5"[..], &b"extra"[..]]);
    assert_eq!(plan(&r, Holdings::default()), Err(Refusal::FileForbidden));
    // In a shell that DOES hold a directory the answer is identical, which is the point: the
    // manifest decides, not the endowment.
    assert_eq!(plan(&r, WITH_DIR), Err(Refusal::FileForbidden));
}

#[test]
fn the_manifest_refuses_a_file_to_a_program_that_declares_none() {
    // The same rule from the other side: a name at a program with no file slot, in a shell that
    // could back one. Refused rather than granted-and-ignored, because a name the program has
    // no use for is authority the user thought they were moving.
    let Command::Run(r) = parse(b"interrupt_heeder report.txt") else {
        panic!()
    };
    assert_eq!(
        plan_against(
            &r,
            Prog::InterruptHeeder,
            Prog::InterruptHeeder.manifest(),
            WITH_DIR
        ),
        Err(Refusal::FileForbidden),
    );
    assert_eq!(
        Refusal::FileForbidden.message(),
        "takes no file; drop the name"
    );
}

#[test]
fn an_unknown_flag_never_reaches_the_file_position() {
    // The one thing a designator prefix genuinely bought: a token that is *shaped* like a flag
    // must not fall through into a grant. A shell that read `--secret` as a file name would
    // turn a typo into a capability transfer, which is the failure the prefix was defending
    // against, so the defence stays where it actually applies.
    let Command::Run(r) = parse(b"wc --secret report.txt") else {
        panic!()
    };
    assert_eq!(r.unexpected, Some(&b"--secret"[..]));
    assert_eq!(
        plan_against(&r, Prog::LeastAuthorityDemo, READS_A_FILE, WITH_DIR),
        Err(Refusal::Unexpected),
    );
}

/// The writable twin: a program that is endowed a file it may write.
const WRITES_A_FILE: Manifest = Manifest {
    file: FileSpec::Required { writable: true },
    ..READS_A_FILE
};

/// The shape milestone 47 says nothing ships yet and everything hinges on: a program that takes
/// **both** an integer and a file. It is the reason the prefix could be removed now (positional
/// resolution is unambiguous while at most one bare token is a file) and the reason `ArgSpec`
/// will have to grow position and arity later.
const STAMPS_A_FILE: Manifest = Manifest {
    arg: ArgSpec::Required,
    mem: MemSpec::Forbidden,
    file: FileSpec::Required { writable: true },
    dir: DirSpec::Forbidden,
    short_options: NO_SHORT_OPTIONS,
    output: OutputSpec::Words,
    input: InputSpec::Forbidden,
    reports: true,
    interruptible: false,
    clock: false,
    domain: false,
    config: false,
    entropy: false,
    network: false,
    machine: false,
    share: false,
    reboot: false,
    sync: false,
    installer: false,
    catalog: false,
    indexes: IndexStore::Forbidden,
    startup: StartupContract::Native,
};

// ---- `bind` (milestone 47 (navigation and naming): "blocked on a second grant") ----

/// The source a line planned for `wc`, which reads a stream and designates it by name: the
/// simplest per-command grant that carries a position.
fn wc_source(line: &[u8], holds: Holdings) -> Result<FileGrant, Refusal> {
    let Command::Run(r) = parse(line) else {
        panic!("not a run: {line:?}")
    };
    match plan_against(&r, Prog::Wc, Prog::Wc.manifest(), holds)?.source {
        Source::File(g) => Ok(g),
        other => panic!("wc's operand planned as {other:?}"),
    }
}

/// **A per-command grant says which tree it is in**: before this, `FileGrant` carried a bare
/// position, so a shell holding two trees would have planned a grant under the mount point
/// against the first tree.
#[test]
fn a_grant_under_a_mount_point_is_in_the_mounted_tree() {
    let g = wc_source(b"wc /media/usb/secret", mounted()).unwrap();
    assert_eq!(
        (g.which, g.dir, g.name.as_bytes()),
        (nav::Which::B, nav::Cwd::root(), &b"secret"[..])
    );
    let g = wc_source(b"wc /media/x", mounted()).unwrap();
    assert_eq!((g.which, g.dir), (nav::Which::A, at(&[b"media"])));

    // A relative name from inside the mount stays there, and `..` out of it leaves it.
    let mut holds = mounted();
    holds.cwd = at(&[b"media", b"usb", b"deep"]);
    let g = wc_source(b"wc x", holds).unwrap();
    assert_eq!((g.which, g.dir), (nav::Which::B, at(&[b"deep"])));
    let g = wc_source(b"wc ../../x", holds).unwrap();
    assert_eq!((g.which, g.dir), (nav::Which::A, at(&[b"media"])));

    // The redirection planner is the same resolver.
    let g = redirect_target(b"/media/usb/out", mounted(), true).unwrap();
    assert_eq!((g.which, g.dir), (nav::Which::B, nav::Cwd::root()));
}

/// **A bound name designates what it is bound to**, which it did not before milestone 154
/// unified the resolvers: `designate` rooted every absolute token literally, so `wc
/// /recent/x` asked for a directory named `recent` while `ls /recent` walked the bind.
#[test]
fn a_bound_name_designates_what_it_is_bound_to() {
    let mut holds = WITH_DIR;
    holds.bind(b"recent", at(&[b"logs", b"2026"])).unwrap();
    let g = wc_source(b"wc /recent/x", holds).unwrap();
    assert_eq!((g.which, g.dir), (nav::Which::A, at(&[b"logs", b"2026"])));
    let g = wc_source(b"wc /recent/../y", holds).unwrap();
    assert_eq!((g.which, g.dir), (nav::Which::A, at(&[b"logs"])));
}

/// [`Holdings::anchor`] hands back exactly the steps that are left, which is what the shell
/// opens handles for: a bound name is consumed, nothing else is.
#[test]
fn anchor_consumes_a_bound_name_and_nothing_else() {
    let mut base = nav::Cwd::root();
    let p = nav::path(b"/media/usb/y").unwrap();
    let rest = mounted().anchor(true, p.steps(), &mut base);
    assert_eq!((base, rest.len()), (nav::Cwd::root(), 3));

    let mut holds = WITH_DIR;
    holds.bind(b"here", at(&[b"x"])).unwrap();
    let p = nav::path(b"/here/y").unwrap();
    let rest = holds.anchor(true, p.steps(), &mut base);
    assert_eq!((base, rest.len()), (at(&[b"x"]), 1));

    holds.cwd = at(&[b"deep"]);
    let p = nav::path(b"x").unwrap();
    let rest = holds.anchor(false, p.steps(), &mut base);
    assert_eq!((base, rest.len()), (at(&[b"deep"]), 1));
}

/// The combination milestone 117's fifth stranger found no program declares: an integer
/// argument **and** an input stream. Unlike [`STAMPS_A_FILE`]'s pairing with `FileSpec`, this
/// one needs no "positional arity" widening, because `arg` consumes a fixed position (index 0,
/// numeric-shaped) before `input`'s bare-name fallback ever looks at what is left; see the
/// `positionals_fill_the_manifest_slots_in_the_order_typed` and
/// `an_argument_and_an_input_compose_by_the_same_fixed_order` tests below. Kept here rather than
/// promoted to a shipped manifest: whether the combination is *wanted* is an architect's call,
/// not
/// this test's (design/roadmap/proposals/a-program-that-takes-an-argument-and-an-input.md).
const TAKES_ARG_AND_READS: Manifest = Manifest {
    arg: ArgSpec::Required,
    file: FileSpec::Forbidden,
    input: InputSpec::Required {
        writes_while_reading: false,
    },
    ..READS_A_FILE
};

#[test]
fn a_bare_name_is_the_grant_and_the_manifest_is_the_direction() {
    // The headline: naming a file IS the grant, and the direction comes from the manifest, not
    // from anything typed. `wc report.txt` reads; the identical line against a writing program
    // writes. That opposite-authority-same-syntax pair is exactly why `file:` came out: it
    // decorated the half that was already visible.
    let Command::Run(r) = parse(b"wc report.txt") else {
        panic!()
    };
    assert_eq!(r.positionals(), [&b"report.txt"[..]]);
    let e = plan_against(&r, Prog::LeastAuthorityDemo, READS_A_FILE, WITH_DIR).unwrap();
    let g = e.file.expect("the name did not become a grant");
    assert_eq!(g.name.as_bytes(), b"report.txt");
    assert!(
        !g.writable,
        "the manifest declared a read, so the grant reads"
    );

    let e = plan_against(&r, Prog::LeastAuthorityDemo, WRITES_A_FILE, WITH_DIR).unwrap();
    assert!(
        e.file.unwrap().writable,
        "the same command line against a writing program grants a writable file",
    );
}

#[test]
fn a_file_whose_name_is_all_digits_is_still_a_file() {
    // Why the parser refuses to classify: a shape-based rule ("a number is the argument") would
    // read `wc 2026` as a missing file and an unexpected integer. The manifest knows this
    // program has no argument slot and one file slot, so there is only one thing the token can
    // be.
    let Command::Run(r) = parse(b"wc 2026") else {
        panic!()
    };
    let e = plan_against(&r, Prog::LeastAuthorityDemo, READS_A_FILE, WITH_DIR).unwrap();
    assert_eq!(e.file.unwrap().name.as_bytes(), b"2026");
}

#[test]
fn positionals_fill_the_manifest_slots_in_the_order_typed() {
    // The integer first, then the name, for a program that declares both. This is the shape
    // milestone 47 deferred rather than designed, so pin the behaviour it does have: the order
    // is the order, and swapping the tokens is a refusal rather than a surprise grant.
    let Command::Run(r) = parse(b"stamp 7 report.txt") else {
        panic!()
    };
    let e = plan_against(&r, Prog::LeastAuthorityDemo, STAMPS_A_FILE, WITH_DIR).unwrap();
    assert_eq!(e.arg, 7);
    assert_eq!(e.file.unwrap().name.as_bytes(), b"report.txt");
    assert!(e.file.unwrap().writable);

    let Command::Run(swapped) = parse(b"stamp report.txt 7") else {
        panic!()
    };
    assert_eq!(
        plan_against(&swapped, Prog::LeastAuthorityDemo, STAMPS_A_FILE, WITH_DIR),
        Err(Refusal::ArgRequired),
    );
}

/// **An argument and an input stream compose today, with no widening owed.** Milestone
/// 117's fifth stranger left `notes/adding-a-program.md`'s `BUGS` section unsure whether this
/// combination "needs `ArgSpec`'s widening" the way `FileSpec` + `InputSpec` genuinely does
/// (see the comment above the input operand in [`plan_against_with`]): two bare names cannot
/// both self-classify, but an integer argument and a bare-name input feed can, because `arg`
/// is numeric-shaped and consumes a fixed earlier position before `input`'s fallback ever
/// looks at what remains. This test is the check: the line composes exactly the way
/// [`positionals_fill_the_manifest_slots_in_the_order_typed`] shows `arg` and `file` composing,
/// and the one thing actually missing is that no shipped program declares the combination, so
/// `crates/swish`'s preview sweep never exercises it (see that crate's
/// `the_arg_line_follows_the_manifest_for_every_program`, which hard-codes a single-operand
/// line and only fails on this shape because it forgets the input operand, not because the
/// combination is refused).
#[test]
fn an_argument_and_an_input_stream_compose_by_the_same_fixed_order() {
    let Command::Run(r) = parse(b"nth 21 report.txt") else {
        panic!()
    };
    let e = plan_against(&r, Prog::LeastAuthorityDemo, TAKES_ARG_AND_READS, WITH_DIR).unwrap();
    assert_eq!(e.arg, 21);
    assert_eq!(
        e.source,
        Source::File(FileGrant {
            which: nav::Which::A,
            dir: nav::Cwd::root(),
            name: Name::new(b"report.txt").unwrap(),
            writable: false,
        }),
    );

    // The line the doc's own example used, with only the argument and no input operand: this
    // is the shape `crates/swish`'s sweep actually types, and it is refused for the ordinary
    // reason (the program declared an input and none was given), not because arg-plus-input is
    // a closed door.
    let Command::Run(missing_input) = parse(b"nth 21") else {
        panic!()
    };
    assert_eq!(
        plan_against(
            &missing_input,
            Prog::LeastAuthorityDemo,
            TAKES_ARG_AND_READS,
            WITH_DIR
        ),
        Err(Refusal::InputRequired),
    );
}

#[test]
fn a_program_endowed_a_file_is_refused_when_the_command_names_none() {
    // The manifest's whole job: catch the mismatch at the prompt instead of letting the program
    // fault on an empty slot somewhere deep inside itself.
    let Command::Run(r) = parse(b"wc") else {
        panic!()
    };
    assert_eq!(
        plan_against(&r, Prog::LeastAuthorityDemo, READS_A_FILE, WITH_DIR),
        Err(Refusal::FileRequired)
    );
    assert_eq!(
        Refusal::FileRequired.message(),
        "is granted one file; name it on the line"
    );
}

#[test]
fn a_name_that_cannot_travel_as_a_grant_is_refused_at_the_prompt() {
    // A grant's name rides in two argument words, so a component that does not fit them cannot
    // travel; `..` designates a directory rather than a file, so it is not a thing to grant.
    // Both are refused where they were typed rather than turning into an ENOENT from a server
    // that was asked something meaningless.
    for line in [&b"wc this-name-is-far-too-long.txt"[..], b"wc .."] {
        let Command::Run(r) = parse(line) else {
            panic!()
        };
        assert_eq!(
            plan_against(&r, Prog::LeastAuthorityDemo, READS_A_FILE, WITH_DIR),
            Err(Refusal::FileNotNameable),
            "{}",
            core::str::from_utf8(line).unwrap(),
        );
    }
    // **An absolute path is planned, and it is rooted in this shell's own namespace.** From
    // `/logs`, `/etc/passwd` designates `passwd` under `/etc` and not under `/logs/etc`, which
    // is the whole of what the leading slash means. It grants nothing extra: `/etc` is a
    // directory this shell could have reached by walking, and if it cannot walk there the
    // server says so at run time exactly as it would for `cd`.
    let mut deep = WITH_DIR;
    assert!(deep.cwd.apply(nav::path(b"logs").unwrap().steps()).is_ok());
    let Command::Run(r) = parse(b"wc /etc/passwd") else {
        panic!()
    };
    let g = plan_against(&r, Prog::LeastAuthorityDemo, READS_A_FILE, deep)
        .unwrap()
        .file
        .unwrap();
    assert_eq!(g.name.as_bytes(), b"passwd");
    let mut buf = [0u8; nav::RENDER_MAX];
    let n = g.dir.render(&mut buf);
    assert_eq!(&buf[..n], b"/etc");

    // And it stops at the root like everything else: `/..` names the level above the only root
    // there is, so there is nothing to express and nothing was denied.
    let Command::Run(r) = parse(b"wc /../passwd") else {
        panic!()
    };
    let refused = plan_against(&r, Prog::LeastAuthorityDemo, READS_A_FILE, deep);
    assert_eq!(refused, Err(Refusal::FileNotNameable));
    assert!(!refused.unwrap_err().message().contains("denied"));
}

/// **The cwd stops at the process boundary**, which is the rule most likely to be got wrong.
/// `wc report.txt` resolves the name against the shell's position *at the moment the grant is
/// made*, and the grant records where that was. A path operand is walked here, once, so the
/// child never sees one.
#[test]
fn a_name_is_resolved_against_the_shells_position_at_grant_time() {
    let mut holds = WITH_DIR;
    assert!(holds.cwd.apply(nav::path(b"logs").unwrap().steps()).is_ok());

    let Command::Run(r) = parse(b"wc report.txt") else {
        panic!()
    };
    let here = plan_against(&r, Prog::LeastAuthorityDemo, READS_A_FILE, holds)
        .unwrap()
        .file
        .unwrap();
    assert_eq!(here.name.as_bytes(), b"report.txt");
    assert_eq!(here.dir.depth(), 1, "resolved where the shell was standing");

    // A path operand is resolved to a directory plus one component, so the FS contract still
    // only ever sees a single name relative to a capability presented to it (§27 intact).
    let Command::Run(r) = parse(b"wc 2026/report.txt") else {
        panic!()
    };
    let deeper = plan_against(&r, Prog::LeastAuthorityDemo, READS_A_FILE, holds)
        .unwrap()
        .file
        .unwrap();
    assert_eq!(deeper.name.as_bytes(), b"report.txt");
    assert_eq!(deeper.dir.depth(), 2);
    assert_eq!(deeper.dir.component(1), b"2026");

    // **And the grant does not follow the shell.** Moving afterwards cannot change what an
    // already-planned grant means, because the grant carries a value and not a pointer to
    // wherever the shell happens to be next. This is the whole of "the child cannot re-resolve".
    holds.cwd.ascend();
    assert_eq!(here.dir.depth(), 1, "the grant moved when the shell did");

    // `..` in a grant's path is resolved at the prompt too, and clamps at the root exactly as
    // `cd` does: there is nothing above the root to name, so the grant cannot be for a file up
    // there whatever the token says.
    let Command::Run(r) = parse(b"wc ../../secret") else {
        panic!()
    };
    assert_eq!(
        plan_against(&r, Prog::LeastAuthorityDemo, READS_A_FILE, WITH_DIR),
        Err(Refusal::FileNotNameable),
        "a grant cannot name its way out of the shell's root",
    );
}

#[test]
fn the_no_such_capability_refusal_is_about_what_the_shell_holds() {
    // Phase 1 hardcoded this refusal, which was true then and would have quietly become a lie.
    // The same command line must read as "you hold nothing that could grant this" in a shell
    // that was granted no directory, and as a real grant in one that was.
    let Command::Run(r) = parse(b"wc report.txt") else {
        panic!()
    };
    assert_eq!(
        plan_against(&r, Prog::LeastAuthorityDemo, READS_A_FILE, WITH_DIR)
            .map(|e| e.file.map(|g| g.name.as_bytes().to_vec()))
            .unwrap(),
        Some(b"report.txt".to_vec()),
        "with a directory in hand, the same line is a grant",
    );
    let refused = plan_against(
        &r,
        Prog::LeastAuthorityDemo,
        READS_A_FILE,
        Holdings::default(),
    );
    assert_eq!(
        refused,
        Err(Refusal::NoSuchCapability(CapabilityKind::File)),
        "with no directory in hand, it is the milestone's headline refusal",
    );
    assert!(
        refused
            .unwrap_err()
            .message()
            .contains("no such capability")
    );
    // And it is reached only through a manifest that declares a file. Nothing in the static
    // table declares one, so the same *line* is refused for its name before a file is ever
    // considered. (`wc` used to be the name this line typed, and milestone 50 shipped a `wc`,
    // which is why it is spelled with a name nothing will ever claim.)
    let Command::Run(named) = parse(b"nosuchprog report.txt") else {
        panic!()
    };
    assert_eq!(plan(&named, WITH_DIR), Err(Refusal::NoSuchProgram));
}

/// A builtin is not a pipeline stage the planner spawns, because it spawns nothing. The shell
/// handles the producing ones (`echo`, `ls`, `pwd`) itself; everything else is refused here
/// rather than parsed into a program that does not exist.
#[test]
fn a_builtin_is_not_something_the_planner_spawns() {
    assert_eq!(
        plan_line(b"cd logs | wc", WITH_DIR),
        Err((0, Refusal::NotAPipelineStage)),
    );
}

#[test]
fn date_takes_nothing_from_the_command_line() {
    // `date` is the first program whose authority is entirely the progenitor's to give (a read-only
    // mapping of the clock page), so its grant expression is empty and both halves matter:
    // typing nothing works, and typing anything is refused rather than passed along.
    let Command::Run(r) = parse(b"date") else {
        panic!()
    };
    let e = plan(&r, Holdings::default()).unwrap();
    assert_eq!(e.prog, Prog::Date);
    assert_eq!((e.arg, e.mem_pages), (0, 0));
    assert!(e.file.is_none());
    assert!(e.reports, "it answers with a line of text");
    assert!(!e.interruptible, "it prints once and exits");

    let Command::Run(r) = parse(b"date 5") else {
        panic!()
    };
    assert_eq!(plan(&r, Holdings::default()), Err(Refusal::ArgForbidden));
    let Command::Run(r) = parse(b"date report.txt") else {
        panic!()
    };
    assert_eq!(plan(&r, WITH_DIR), Err(Refusal::FileForbidden));
    let Command::Run(r) = parse(b"date --mem 4") else {
        panic!()
    };
    assert_eq!(plan(&r, Holdings::default()), Err(Refusal::MemForbidden));
}

/// **What a line's words designate becomes one directory grant at the current directory**
/// (milestone 205, §170 clauses 2 and 5): the names for a line that named some, the whole
/// directory for `.`, and nothing for a line that named nothing (calef's N1 ruling,
/// 2026-09-27T06:27Z). A shell holding no directory cannot back a designation.
#[test]
fn a_designation_becomes_one_grant_at_the_current_directory() {
    use expand::Designation;
    let run = parse_run(b"std_exerciser needle src");
    let mut d = Designation::none();
    d.name(b"src", true).unwrap();
    let e = super::plan_against(
        &run,
        Prog::StdExerciser,
        Prog::StdExerciser.manifest(),
        WITH_DIR,
        Expansion::designated(d),
    )
    .unwrap();
    let g = e.dir.expect("src was designated");
    assert_eq!(g.dir, WITH_DIR.cwd);
    assert_eq!(g.names.only(), Some(&b"src"[..]));
    assert!(g.subtree);

    let none = super::plan_against(
        &parse_run(b"std_exerciser needle"),
        Prog::StdExerciser,
        Prog::StdExerciser.manifest(),
        WITH_DIR,
        Expansion::designated(Designation::none()),
    )
    .unwrap();
    assert_eq!(
        none.dir, None,
        "N1: a line that names no file grants nothing"
    );

    let mut here = Designation::none();
    here.here();
    let e = super::plan_against(
        &parse_run(b"std_exerciser needle ."),
        Prog::StdExerciser,
        Prog::StdExerciser.manifest(),
        WITH_DIR,
        Expansion::designated(here),
    )
    .unwrap();
    assert!(
        e.dir.expect("here").names.is_empty(),
        "empty names: the whole directory"
    );

    let holds_none = Holdings {
        dir: false,
        ..WITH_DIR
    };
    assert_eq!(
        super::plan_against(
            &run,
            Prog::StdExerciser,
            Prog::StdExerciser.manifest(),
            holds_none,
            Expansion::designated(d)
        ),
        Err(Refusal::NoSuchCapability(CapabilityKind::File))
    );
}
