//! Planning tests: the operators, `2>`, and the draining lane.

use crate::expand::{Expansion, Name};
use crate::test_support::{WITH_DIRECTORY, plan_line};
use crate::*;

// ---- milestone 50: the operators, checked against the manifests that ship ----

/// **The headline, at the level this crate can prove it: `>` and `|` are one field.**
///
/// The same `date`, planned three ways, differs in exactly one thing. Nothing about the
/// program's own endowment (its argument, its memory, its files, its options) moves, because
/// redirection is not a grant to the program at all: it is a different capability in a slot
/// that was always going to hold one.
#[test]
fn redirecting_and_piping_change_one_field_and_nothing_else() {
    let (plain, _) = plan_line(b"date", WITH_DIRECTORY).unwrap();
    let (piped, n) = plan_line(b"date | wc", WITH_DIRECTORY).unwrap();
    let (filed, _) = plan_line(b"date > report.txt", WITH_DIRECTORY).unwrap();
    assert_eq!(n, 2);
    let (a, b, c) = (plain[0].unwrap(), piped[0].unwrap(), filed[0].unwrap());
    assert_eq!(a.sink, line::Sink::Report);
    assert_eq!(b.sink, line::Sink::Pipe);
    assert!(matches!(c.sink, line::Sink::File(g, _) if g.name.as_bytes() == b"report.txt"));
    // And everything else is identical, which is the claim. Compared field by field rather than
    // by clearing `sink` and comparing wholes, so a field added later has to be thought about.
    for other in [b, c] {
        assert_eq!(a.program, other.program);
        assert_eq!(a.program_argument, other.program_argument);
        assert_eq!(a.memory_grant_pages, other.memory_grant_pages);
        assert_eq!(a.file, other.file);
        assert_eq!(a.directory, other.directory);
        assert_eq!(a.short_options, other.short_options);
        assert_eq!(a.source, other.source);
        assert_eq!(a.reports, other.reports);
        assert_eq!(a.interruptible, other.interruptible);
    }
}

/// **`>>` changes nothing about the endowment, which is what makes it a shell-side open mode.**
///
/// The stronger version of the test above: `date > f` and `date >> f` plan to endowments that
/// are equal in every field including the `FileGrant` the sink names, and differ only in a
/// [`line::Mode`] the child has no way to observe. Append is not a second capability, and
/// nothing about what `date` holds records which operator was typed.
#[test]
fn append_and_truncate_plan_the_same_endowment() {
    let (trunc, _) = plan_line(b"date > report.txt", WITH_DIRECTORY).unwrap();
    let (app, _) = plan_line(b"date >> report.txt", WITH_DIRECTORY).unwrap();
    let (t, a) = (trunc[0].unwrap(), app[0].unwrap());
    let (line::Sink::File(tg, tm), line::Sink::File(ag, am)) = (t.sink, a.sink) else {
        panic!("a redirection did not plan to a file sink");
    };
    assert_eq!(tg, ag, "the two spellings designate the same file");
    assert_eq!((tm, am), (line::Mode::Truncate, line::Mode::Append));
    // Everything else, compared through the whole value with the one differing field made
    // equal. Cheaper than the field-by-field comparison above and sound here because the only
    // thing that may differ is inside `sink`.
    let mut a_flat = a;
    a_flat.sink = t.sink;
    assert_eq!(t, a_flat, "`>>` changed something other than the open mode");
}

/// The joints of a pipeline are pipes, its head reads and its tail writes. Three stages, so the
/// middle one is a real middle and not an end wearing a disguise.
#[test]
fn a_pipeline_wires_its_joints_and_only_its_ends_touch_a_file() {
    let (p, n) = plan_line(b"date | wc | wc > out.txt", WITH_DIRECTORY).unwrap();
    assert_eq!(n, 3);
    assert_eq!(p[0].unwrap().source, line::Source::None);
    assert_eq!(p[0].unwrap().sink, line::Sink::Pipe);
    assert_eq!(p[1].unwrap().source, line::Source::Pipe);
    assert_eq!(p[1].unwrap().sink, line::Sink::Pipe);
    assert_eq!(p[2].unwrap().source, line::Source::Pipe);
    assert!(
        matches!(p[2].unwrap().sink, line::Sink::File(g, _) if g.name.as_bytes() == b"out.txt")
    );
}

/// **The refusal Unix cannot produce.** `wc` declares that it reads a stream, so a `wc` with
/// nothing feeding it is caught at the prompt rather than becoming a process blocked on a
/// receive that will never complete. On Unix the same command is a shell that appears to hang.
#[test]
fn a_reader_with_nothing_to_read_is_refused_at_the_prompt() {
    assert_eq!(
        plan_line(b"wc", WITH_DIRECTORY),
        Err((0, Refusal::InputRequired))
    );
    // And the three ways to give it one all satisfy it.
    assert!(plan_line(b"wc report.txt", WITH_DIRECTORY).is_ok());
    assert!(plan_line(b"wc < report.txt", WITH_DIRECTORY).is_ok());
    assert!(plan_line(b"date | wc", WITH_DIRECTORY).is_ok());
    let msg = Refusal::InputRequired.message();
    assert!(msg.contains("name a file"), "{msg}");
}

/// **`wc report.txt` is `wc < report.txt` with the operator left out**, and typing the name is
/// what moves the authority: the same command with no name is refused at the prompt, above.
///
/// `wc` declares a stream and no file or directory, so a bare name can only be what feeds it. The
/// assertion puts the name in the *source*, not in `Endowment::file`: `wc` gets an endpoint
/// carrying the bytes, narrower than a file capability. See
/// notes/grant-expression/per-file-grants.md.
#[test]
fn naming_a_file_to_a_reader_is_the_operator_left_out() {
    let e = plan_line(b"wc report.txt", WITH_DIRECTORY).unwrap().0[0].unwrap();
    let line::Source::File(g) = e.source else {
        panic!("the name did not become the input: {:?}", e.source)
    };
    assert_eq!(g.name.as_bytes(), b"report.txt");
    assert!(!g.writable, "a reader reads");
    assert!(
        e.file.is_none(),
        "this is a stream, not the per-file capability FileGrantDeclaration describes",
    );
    // The same line against a shell that holds no directory is refused: a statement about its
    // capability table.
    assert_eq!(
        plan_line(b"wc report.txt", Holdings::default()),
        Err((0, Refusal::NoSuchCapability(CapabilityKind::File))),
    );
}

/// **An explicit `<` wins, and a line that names its input twice is refused**, because the
/// second name then has nowhere to go. Nothing is silently dropped: an operand the planner
/// cannot place is the same refusal it always was.
#[test]
fn an_input_named_twice_is_refused_rather_than_one_of_them_ignored() {
    let e = plan_line(b"wc < a.txt", WITH_DIRECTORY).unwrap().0[0].unwrap();
    let line::Source::File(g) = e.source else {
        panic!()
    };
    assert_eq!(g.name.as_bytes(), b"a.txt");
    assert!(plan_line(b"wc b.txt < a.txt", WITH_DIRECTORY).is_err());
    // And a stage fed by a pipe has no room for an operand either.
    assert!(plan_line(b"date | wc b.txt", WITH_DIRECTORY).is_err());
}

/// The mirror: a program that reads nothing, handed an input. Authority that moved for no
/// reason, which this shell refuses in every other position too.
#[test]
fn a_program_that_reads_nothing_is_refused_an_input() {
    assert_eq!(
        plan_line(b"date < report.txt", WITH_DIRECTORY),
        Err((0, Refusal::InputForbidden)),
    );
    assert_eq!(
        plan_line(b"date | date", WITH_DIRECTORY),
        Err((1, Refusal::InputForbidden)),
    );
}

/// **A program whose answer is a register cannot be redirected**, and the manifest is what says
/// so. `least_authority_demo 9 > out.txt` would otherwise write a `u64` into a byte sink, which is a file
/// with nothing legible in it and no error anywhere.
#[test]
fn only_a_byte_stream_can_be_redirected() {
    assert_eq!(
        plan_line(b"least_authority_demo 9 > out.txt", WITH_DIRECTORY),
        Err((0, Refusal::NotAByteStream)),
    );
    assert_eq!(
        plan_line(b"least_authority_demo 9 | wc", WITH_DIRECTORY),
        Err((0, Refusal::NotAByteStream)),
    );
    // The interrupt demonstrators hold no output capability at all, which is the third case and
    // reaches the same refusal from the other side.
    assert_eq!(
        plan_line(b"interrupt_heeder | wc", WITH_DIRECTORY),
        Err((0, Refusal::NotAByteStream)),
    );
    // A `Bytes` program in the same position is fine, so the refusal is about the declaration
    // and not about the operator.
    assert!(plan_line(b"date > out.txt", WITH_DIRECTORY).is_ok());
}

// ---- `2>`: the second stream is a declaration, not a number (DECISIONS §67) ----

/// **The declaration is what `2>` binds to, and a program that declares none is refused.**
///
/// This is the whole of §67 at the level this crate can prove it. `date` declares a second
/// output, so the operator has something to name; `wc` writes one stream and its diagnostics
/// ride it, so the same operator names nothing and the line does not run. Nothing here consults
/// a number: the refusal comes from a manifest that is silent about a second stream.
#[test]
fn a_diagnostic_redirection_binds_to_a_declaration_and_refuses_without_one() {
    let e = plan_line(b"date 2> err.txt", WITH_DIRECTORY).unwrap().0[0].unwrap();
    let line::Diagnostics::File(g, mode) = e.diagnostics else {
        panic!("2> did not plan to a file: {:?}", e.diagnostics)
    };
    assert_eq!(g.name.as_bytes(), b"err.txt");
    assert_eq!(mode, line::Mode::Truncate);
    assert!(g.writable, "the shell writes that file");
    // The output is untouched. `2>` says where the *second* stream goes and nothing else.
    assert_eq!(e.sink, line::Sink::Report);

    // And the refusal, which is the truthful one rather than a permission.
    assert_eq!(
        plan_line(b"wc gate.txt 2> err.txt", WITH_DIRECTORY),
        Err((0, Refusal::NoDiagnosticStream)),
    );
    let msg = Refusal::NoDiagnosticStream.message();
    assert!(msg.contains("declares no second output"), "{msg}");
    // A program with no byte stream at all reaches it too, from the other side.
    assert_eq!(
        plan_line(b"least_authority_demo 9 2> err.txt", WITH_DIRECTORY),
        Err((0, Refusal::NoDiagnosticStream)),
    );
}

/// **A declared second stream exists whether or not the line mentions it**, which is the half of
/// §67 that fixes the motivating loss.
///
/// `date > when.txt` with no `2>` on it still plans a second destination, and that destination
/// is *not* the file: the shell prints those bytes. A `date` whose complaint went where its
/// answer went is exactly what the decision was taken to end, so the assertion is that the two
/// destinations differ on a line that names only one of them.
#[test]
fn a_declarer_gets_a_second_destination_with_no_operator_on_the_line() {
    let e = plan_line(b"date", WITH_DIRECTORY).unwrap().0[0].unwrap();
    assert_eq!(e.diagnostics, line::Diagnostics::Printed);

    let e = plan_line(b"date > when.txt", WITH_DIRECTORY).unwrap().0[0].unwrap();
    assert!(matches!(e.sink, line::Sink::File(g, _) if g.name.as_bytes() == b"when.txt"));
    assert_eq!(
        e.diagnostics,
        line::Diagnostics::Printed,
        "a redirected `date` must not write its complaint into the file",
    );

    // And a program that declares none has none to place, on any line.
    for text in [
        &b"wc gate.txt"[..],
        b"wc gate.txt > out.txt",
        b"least_authority_demo 9",
    ] {
        let e = plan_line(text, WITH_DIRECTORY).unwrap().0[0].unwrap();
        assert_eq!(
            e.diagnostics,
            line::Diagnostics::None,
            "{}",
            core::str::from_utf8(text).unwrap(),
        );
    }
}

/// **`2>` changes one field**, which is the same claim `>` and `>>` already make and is what
/// says the second stream is a destination rather than a different program.
#[test]
fn a_diagnostic_redirection_changes_one_field_and_nothing_else() {
    let plain = plan_line(b"date > out.txt", WITH_DIRECTORY).unwrap().0[0].unwrap();
    let redirected = plan_line(b"date > out.txt 2>> err.txt", WITH_DIRECTORY)
        .unwrap()
        .0[0]
        .unwrap();
    assert_eq!(redirected.diagnostics, {
        let (g, _) = match redirected.diagnostics {
            line::Diagnostics::File(g, m) => (g, m),
            other => panic!("2>> did not plan to a file: {other:?}"),
        };
        line::Diagnostics::File(g, line::Mode::Append)
    });
    let mut flat = redirected;
    flat.diagnostics = plain.diagnostics;
    assert_eq!(
        plain, flat,
        "`2>` changed something other than the second stream"
    );
}

/// The slot the declaration names is high and out of the way, because how many low slots a child
/// gets depends on the wiring. A declaration that landed among the ordinary grants would move
/// under the program, and the program probes one number.
#[test]
fn the_declared_slot_is_out_of_reach_of_the_ordinary_grants() {
    let slot = Program::Date
        .manifest()
        .output
        .diagnostics_slot()
        .expect("date declares a second stream");
    assert_eq!(slot, DIAGNOSTICS_SLOT);
    // Four is the most ordinary grants any manifest here asks for (output, clock, input,
    // budget); fifteen keeps it well under `abi::fault::FAULT_EP_SLOT`, the table's last slot.
    assert!(slot > 4 && slot < 15, "slot {slot} is not out of the way");
    // And every other program declares none, so nothing else needs the slot at all.
    for p in [
        Program::LeastAuthorityDemo,
        Program::MemoryGrantDepleter,
        Program::InterruptHeeder,
        Program::InterruptIgnorer,
        Program::Rm,
        Program::Wc,
    ] {
        assert_eq!(p.manifest().output.diagnostics_slot(), None, "{p:?}");
    }
}

/// A declared second stream does not change what `>` and `|` may substitute: the first stream is
/// still the sink contract. If it did, declaring diagnostics would silently make a program
/// unpipeable, which would be a cost §67 does not pay.
#[test]
fn declaring_a_second_stream_leaves_the_first_one_redirectable() {
    assert!(Program::Date.manifest().output.is_byte_stream());
    assert!(plan_line(b"date | wc", WITH_DIRECTORY).is_ok());
    assert!(plan_line(b"date > out.txt", WITH_DIRECTORY).is_ok());
    // And `2>` rides alongside a pipe, at the tail where it is written.
    let (p, n) = plan_line(b"date | wc", WITH_DIRECTORY).unwrap();
    assert_eq!(n, 2);
    assert_eq!(p[0].unwrap().diagnostics, line::Diagnostics::Printed);
    assert_eq!(p[1].unwrap().diagnostics, line::Diagnostics::None);
}

/// The grant refusal wins over the stream refusal when a line is wrong about both, because what
/// a program is endowed is a bigger fact about the line than where its bytes were going.
#[test]
fn a_missing_argument_is_reported_before_a_missing_byte_stream() {
    // `least_authority_demo` needs an integer AND has no bytes to redirect. Both are true; one is printed.
    assert_eq!(
        plan_line(b"least_authority_demo > out.txt", WITH_DIRECTORY),
        Err((0, Refusal::ProgramArgumentRequired)),
    );
}

/// A redirection is a designation like any other, so a shell that holds no directory cannot
/// back one. The same sentence an operand gets, for the same reason.
#[test]
fn a_redirection_needs_a_directory_to_resolve_against() {
    assert_eq!(
        plan_line(b"date > report.txt", Holdings::default()),
        Err((0, Refusal::NoSuchCapability(CapabilityKind::File))),
    );
    // A pipeline with no file on it needs no directory at all, which is what makes `date | wc`
    // runnable in a shell that holds nothing but a spawn channel.
    assert!(plan_line(b"date | wc", Holdings::default()).is_ok());
}

/// A redirection resolves its leading path exactly as an operand does, against where the shell
/// stands, and records the result as a value.
#[test]
fn a_redirection_resolves_its_leading_path() {
    let mut deeper = nav::Cwd::root();
    deeper.apply(nav::path(b"logs").unwrap().steps()).unwrap();
    let holds = Holdings {
        directory: true,
        mounted: None,
        cwd: deeper,
        binds: nav::Bindings::none(),
    };
    let (p, _) = plan_line(b"date > old/report.txt", holds).unwrap();
    let line::Sink::File(g, _) = p[0].unwrap().sink else {
        panic!("not a file sink")
    };
    assert_eq!(g.name.as_bytes(), b"report.txt");
    assert!(g.writable, "a `>` target is written");
    let mut buf = [0u8; nav::RENDER_MAX];
    let n = g.directory.render(&mut buf);
    assert_eq!(&buf[..n], b"/logs/old");
    // An absolute path resolves from this shell's root rather than from where it stands, so
    // the same line typed in two directories names one file.
    let line::Sink::File(g, _) = plan_line(b"date > /old/report.txt", holds).unwrap().0[0]
        .unwrap()
        .sink
    else {
        panic!("not a file sink")
    };
    let n = g.directory.render(&mut buf);
    assert_eq!(&buf[..n], b"/old");
}

/// A `<` target is readable and a `>` target is writable, and that is the one direction this
/// shell reads off the line rather than out of a manifest. It is not an exception: the program
/// is handed a sink or a source, neither of which is a file capability.
#[test]
fn the_operator_decides_the_direction() {
    let (p, _) = plan_line(b"wc < in.txt > out.txt", WITH_DIRECTORY).unwrap();
    let line::Source::File(i) = p[0].unwrap().source else {
        panic!()
    };
    let line::Sink::File(o, _) = p[0].unwrap().sink else {
        panic!()
    };
    assert!(!i.writable);
    assert!(o.writable);
}

// ---- milestone 50's draining lane: the input operand inside a pipeline, and the one wait
// point ----

/// **`wc report.txt | wc` names its input the way `wc report.txt` does**, and the planner is
/// the only thing that knows it: an input operand is a trailing positional at a program that
/// declares an input, not a token any grammar could classify.
///
/// This is the fact the shell was dropping. It planned the stage correctly and then wired the
/// pipeline off the **line**, which has no `<` on it, so the head was spawned with an empty
/// input slot. A `receive` there answers `NoSuchSlot` rather than blocking and every reader reads
/// that as end of document, so the stage reported an empty stream: a wrong answer, not a hang,
/// which is why nothing caught it.
#[test]
fn an_input_operand_survives_a_pipeline() {
    let (p, n) = plan_line(b"wc report.txt | wc", WITH_DIRECTORY).unwrap();
    assert_eq!(n, 2);
    let line::Source::File(g) = p[0].unwrap().source else {
        panic!("the head stage's operand did not become a source")
    };
    assert_eq!(g.name.as_bytes(), b"report.txt");
    assert!(!g.writable, "a stream is read, whatever the operator");
    assert_eq!(p[0].unwrap().sink, line::Sink::Pipe);
    // And the same name written out loud plans to the same thing, which is what makes the
    // operand a spelling rather than a second mechanism.
    let (explicit, _) = plan_line(b"wc < report.txt | wc", WITH_DIRECTORY).unwrap();
    assert_eq!(p[0], explicit[0]);
}

/// A filter that answers while it reads: the manifest shape no shipped program declares yet.
/// Planned through [`plan_against_with`] so [`check_chain`] is live, tested logic rather than a
/// branch nothing reaches, exactly as [`READS_A_FILE`] keeps `FileGrantDeclaration::Required` live.
const RENDERS_AS_IT_READS: Manifest = Manifest {
    program_argument: ProgramArgumentDeclaration::Forbidden,
    memory_grant: MemoryGrantDeclaration::Forbidden,
    file: FileGrantDeclaration::Forbidden,
    directory: DirectoryGrantDeclaration::Forbidden,
    short_options: NO_SHORT_OPTIONS,
    output: OutputDeclaration::Bytes,
    input: InputDeclaration::Required {
        writes_while_reading: true,
    },
    reports: true,
    interruptible: false,
    clock: false,
    domain: false,
    configuration: false,
    entropy: false,
    network: false,
    machine: false,
    share: false,
    reboot: false,
    sync: false,
    installer: false,
    catalog: false,
    indexes: IndexStoreDeclaration::Forbidden,
    activation_directory: false,
    console_input: false,
    startup: StartupContract::Native,
};

/// Plan one stage against an explicit manifest, with the operators' answer folded in.
fn plan_as(m: Manifest, text: &[u8], streams: Streams) -> Endowment {
    let Command::Run(r) = parse(text) else {
        panic!()
    };
    plan_against_with(
        &r,
        Program::Wc,
        m,
        WITH_DIRECTORY,
        Expansion::none(),
        streams,
    )
    .unwrap()
}

/// **The declaration reaches the endowment**, which is what lets a whole line be checked rather
/// than one stage. A program that answers while it reads says so; `wc` does not.
#[test]
fn writing_while_reading_is_a_declaration_the_plan_carries() {
    let streams = Streams {
        source: line::Source::File(FileGrant {
            which: nav::Which::A,
            directory: nav::Cwd::root(),
            name: Name::new(b"page.md").unwrap(),
            writable: false,
        }),
        ..Streams::default()
    };
    assert!(plan_as(RENDERS_AS_IT_READS, b"wc", streams).writes_while_reading);
    assert!(!plan_as(Program::Wc.manifest(), b"wc", streams).writes_while_reading);
}

/// **One wait point, stated as a rule about a line.**
///
/// The shell can be at one end of a chain or the other, never both, because a `SEND` blocks
/// until somebody receives and there is no select. So a line whose bytes all come from this
/// shell needs at least one stage that reads to the end before it writes; with one, the feed
/// completes before anything travels back, and with none, both processes stop.
#[test]
fn a_chain_this_shell_feeds_needs_one_stage_that_reads_to_the_end() {
    let src = Streams {
        source: line::Source::File(FileGrant {
            which: nav::Which::A,
            directory: nav::Cwd::root(),
            name: Name::new(b"page.md").unwrap(),
            writable: false,
        }),
        ..Streams::default()
    };
    let piped = Streams {
        sink: line::Sink::Pipe,
        ..src
    };
    let renderer = plan_as(RENDERS_AS_IT_READS, b"wc", src);
    let counter = plan_as(Program::Wc.manifest(), b"wc", src);
    let renderer_piped = plan_as(RENDERS_AS_IT_READS, b"wc", piped);

    // `mdr page.md`: this shell writes the file in, and DECISIONS §106 gives the render
    // somewhere to go that is not this shell (`terminal_sink_caretaker`), so there is no second
    // reader for the shell to wait behind.
    assert_eq!(check_chain(true, &[Some(renderer)]), Ok(()));
    // `mdr page.md | wc`: the `wc` absorbs the whole stream, so the feed finishes first.
    assert_eq!(
        check_chain(true, &[Some(renderer_piped), Some(counter)]),
        Ok(()),
    );
    // `wc page.md`: the barrier is the only stage.
    assert_eq!(check_chain(true, &[Some(counter)]), Ok(()));
    // `something | doc`: a chain this shell does not feed is somebody else's rendezvous.
    assert_eq!(check_chain(false, &[Some(renderer)]), Ok(()));
    // `ls > out.txt`: the shell is the producer and no stage was spawned at all, so there is no
    // second process for it to deadlock against.
    assert_eq!(check_chain(true, &[None, None]), Ok(()));
    // `mdr page.md > out.txt`: DECISIONS §106 (take the `terminal_sink_caretaker` narrowing) only
    // narrows the unredirected case. A `>` still comes back through this shell (DECISIONS §55 (the
    // file behind a `>` is the shell itself)).
    let to_file = Streams {
        sink: line::Sink::File(
            FileGrant {
                which: nav::Which::A,
                directory: nav::Cwd::root(),
                name: Name::new(b"out.txt").unwrap(),
                writable: true,
            },
            line::Mode::Truncate,
        ),
        ..src
    };
    let renderer_to_file = plan_as(RENDERS_AS_IT_READS, b"wc", to_file);
    assert_eq!(
        check_chain(true, &[Some(renderer_to_file)]),
        Err(Refusal::NoReaderButThisShell),
    );
}

/// The refusal names what a person can do about it, and it is about the **line**: the program
/// is fine, and so is the shell.
#[test]
fn the_both_ends_refusal_names_the_fix() {
    assert!(
        Refusal::NoReaderButThisShell
            .message()
            .contains("give it a reader that is not this shell"),
    );
}
