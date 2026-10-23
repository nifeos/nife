//! Tests of the preview, which is the visibility surface: `caps <path>`, `caps` over a whole
//! line, and `2>`, the declared second stream (DECISIONS §67 (a program's second stream is a
//! declaration, not a number)).

use std::string::String;
use std::vec::Vec;

use grant_plan::nav::Cwd;
use grant_plan::{FileGrantDeclaration, InputDeclaration, MemoryGrantDeclaration};

use super::*;
use crate::test_support::{endowment, listing, shown};

/// **`caps <path>` tells the cases apart** (§219 (how the shell names an installed program to the
/// spawner) D and D2, and milestone 597 (a program carries its manifest in an ELF note)): vouched
/// runs with its own manifest and names its generation; unvouched runs with the two pages only on a
/// session that holds the capability, and prints what the note asked beside what it gets; a note
/// that asks the line for more than unvouched bytes may hold is refused; otherwise it says it would
/// be refused and grants nothing.
#[test]
fn caps_of_an_image_names_its_provenance_the_gate_and_what_the_note_asks() {
    use grant_plan::expand::Expansion;
    use grant_plan::{Command, Holdings, IMAGE_ROW, Manifest, NO_NOTE_MANIFEST};
    let say = |v: Option<super::Vouched>, holds: bool, declared: Option<Manifest>, line: &[u8]| {
        let Command::Run(spec) = grant_plan::parse(line) else {
            panic!("not an invocation")
        };
        let m = declared.unwrap_or(NO_NOTE_MANIFEST);
        let e =
            grant_plan::plan_against(&spec, IMAGE_ROW, m, Holdings::default(), Expansion::none())
                .unwrap();
        let mut said = Vec::new();
        super::write_image_capabilities(
            b"bin/x",
            b"00ff",
            v,
            holds,
            declared,
            &e,
            &Holdings::default(),
            &mut |b| {
                said.extend_from_slice(b);
            },
        );
        String::from_utf8(said).unwrap()
    };
    let by = |generation, by_owner| {
        Some(super::Vouched {
            generation,
            by_owner,
            network_granted: false,
        })
    };
    let vouched = say(by(3, false), false, None, b"bin/x");
    assert!(vouched.contains("vouched by activation generation 3 (digest 00ff)"));
    let owned = say(by(4, true), true, None, b"bin/x");
    assert!(owned.contains("vouched by the owner in activation generation 4 (digest 00ff)"));
    assert!(
        !owned.contains("clock"),
        "an owner's vouch runs as installed, not as D2"
    );
    assert!(vouched.contains("cap 0"));
    assert!(vouched.contains("carries no manifest note"));
    assert!(
        !vouched.contains("clock"),
        "a vouched image is endowed its manifest's, not D2's"
    );
    let asks_clock = Manifest {
        clock: true,
        ..NO_NOTE_MANIFEST
    };
    let vouched_clock = say(by(3, false), false, Some(asks_clock), b"bin/x");
    assert!(
        vouched_clock.contains("clock    read-only. it can read the time"),
        "a vouched note's clock is granted: {vouched_clock}"
    );
    assert!(vouched_clock.contains("its manifest note asks for: output bytes, the clock"));

    // A note's `machine` and `share` are read from the note, not from the placeholder row.
    let asks_machine_and_share = Manifest {
        machine: true,
        share: true,
        ..NO_NOTE_MANIFEST
    };
    let vouched_procps = say(by(3, false), false, Some(asks_machine_and_share), b"bin/x");
    assert!(
        vouched_procps.contains("cap 11 frame     machine"),
        "a vouched note's machine page is previewed: {vouched_procps}"
    );
    assert!(
        vouched_procps.contains("cap 12 region    share"),
        "a vouched note's share view is previewed: {vouched_procps}"
    );

    let asks_network = Manifest {
        network: true,
        ..NO_NOTE_MANIFEST
    };
    let held = say(None, true, Some(asks_network), b"bin/x");
    assert!(held.contains("provenance: unvouched (digest 00ff)"));
    assert!(held.contains("cap 1  page      clock") && held.contains("cap 2  page      config"));
    assert!(!held.contains("cap 10"), "an unvouched note grants nothing");
    assert!(held.contains("asks for: output bytes, the network"));
    assert!(held.contains("its note grants nothing"));
    let slot = std::format!("slot {}", grant_plan::spawnproto::RUN_UNVOUCHED_SLOT);
    assert!(held.contains(&slot), "{held}");

    let asks_arg = grant_plan::Program::LeastAuthorityDemo.manifest();
    let exceeds = say(None, true, Some(asks_arg), b"bin/x 5");
    assert!(exceeds.contains("would not run here"));
    assert!(exceeds.contains("more than unvouched bytes may hold"));
    assert!(!exceeds.contains("cap 0"));

    let not_held = say(None, false, None, b"bin/x");
    assert!(not_held.contains("would not run here"));
    assert!(
        !not_held.contains("cap 0"),
        "a refusal grants nothing, so it previews nothing"
    );
    assert!(not_held.contains("refused if run"));
}

#[test]
fn a_memory_grant_is_a_row_and_no_grant_is_no_row() {
    let mut e = endowment(Program::MemoryGrantDepleter);
    assert!(!shown(|o| write_preview(&e, &Holdings::default(), None, o)).contains("untyped"));
    e.memory_grant_pages = 16;
    assert!(
        shown(|o| write_preview(&e, &Holdings::default(), None, o))
            .contains("cap 1  untyped   16 pages")
    );
}

#[test]
fn append_and_truncate_differ_in_exactly_one_line_of_the_preview() {
    let grant = grant_plan::FileGrant {
        which: grant_plan::nav::Which::A,
        directory: Cwd::root(),
        name: grant_plan::expand::Name::new(b"out.txt").expect("a nameable file"),
        writable: true,
    };
    let mut e = endowment(Program::Date);
    e.sink = line::Sink::File(grant, line::Mode::Truncate);
    let truncate: Vec<String> = shown(|o| write_preview(&e, &Holdings::default(), None, o))
        .lines()
        .map(String::from)
        .collect();
    e.sink = line::Sink::File(grant, line::Mode::Append);
    let append: Vec<String> = shown(|o| write_preview(&e, &Holdings::default(), None, o))
        .lines()
        .map(String::from)
        .collect();

    assert_eq!(truncate.len(), append.len());
    let differing = truncate.iter().zip(&append).filter(|(a, b)| a != b).count();
    // `>` and `>>` hand the child the identical capability, so exactly one line may differ, and
    // it has to be about the shell rather than about the child.
    assert_eq!(differing, 1);
    assert!(
        append
            .iter()
            .any(|l| l.contains("keeps what is already in it"))
    );
    assert!(truncate.iter().any(|l| l.contains("empties it first")));
}

#[test]
fn a_directory_grant_prints_every_name_and_says_whether_r_was_typed() {
    let mut e = endowment(Program::Rm);
    e.directory = Some(grant_plan::DirectoryGrant {
        which: grant_plan::nav::Which::A,
        directory: Cwd::root(),
        names: listing(&[b"a.txt", b"b.txt"]),
        subtree: false,
    });
    let without = shown(|o| write_preview(&e, &Holdings::default(), None, o));
    assert!(without.contains("a.txt b.txt"), "{without}");
    assert!(
        without.contains("no -r, so it cannot even look"),
        "{without}"
    );

    if let Some(g) = e.directory.as_mut() {
        g.subtree = true;
    }
    let with = shown(|o| write_preview(&e, &Holdings::default(), None, o));
    assert!(with.contains("-r grants the walk"), "{with}");
}

#[test]
fn date_says_the_clock_is_not_on_the_line() {
    // No token designates `date`'s clock, and the child still holds it, read-only.
    let s = shown(|o| write_preview(&endowment(Program::Date), &Holdings::default(), None, o));
    assert!(s.contains("cap 1  frame     clock"), "{s}");
    assert!(s.contains("read the time and not set it"), "{s}");
    // And a program that declares no clock is not given a row that says it has one.
    assert!(
        !shown(|o| write_preview(&endowment(Program::Wc), &Holdings::default(), None, o))
            .contains("clock")
    );
}

#[test]
fn printenv_says_the_config_page_is_not_on_the_line() {
    // `clock`'s twin: no token designates `printenv`'s config page either.
    let s = shown(|o| write_preview(&endowment(Program::Printenv), &Holdings::default(), None, o));
    assert!(s.contains("cap 1  frame     config"), "{s}");
    assert!(s.contains("read-only"), "{s}");
    // A program that declares no config page is not given a row that says it has one.
    assert!(
        !shown(|o| write_preview(&endowment(Program::Wc), &Holdings::default(), None, o))
            .contains("config")
    );
}

/// A page assembled with values nobody boots with, so a test that passes cannot be reading the
/// boot's defaults by coincidence. `LANG` is left out, so the unset spelling is exercised too.
fn unusual_page() -> [u8; environment_contract::PAGE_BYTES] {
    environment_contract::PageBuilder::new()
        .tz("America/Los_Angeles")
        .unwrap()
        .term("xterm-256color")
        .unwrap()
        .build()
}

/// **§111's preview: the values, read from the page the child will be handed** (milestone 47).
/// What this proves that nothing else does: the row prints whatever the shell's view of the page
/// says, not a literal that happens to match the boot's defaults, and a key the page does not
/// carry reads the way `printenv` would print it.
#[test]
fn caps_printenv_prints_the_values_on_the_page_the_child_will_read() {
    let bytes = unusual_page();
    // SAFETY: `bytes` is a live buffer of exactly `PAGE_BYTES`, alive for this test.
    let page = unsafe { ConfigPage::new(bytes.as_ptr() as u64) };
    let s = shown(|o| {
        write_preview(
            &endowment(Program::Printenv),
            &Holdings::default(),
            Some(page),
            o,
        );
    });
    assert!(s.contains("the page this shell reads too"), "{s}");
    assert!(s.contains("TZ=America/Los_Angeles\n"), "{s}");
    assert!(s.contains("LANG (unset)\n"), "{s}");
    assert!(s.contains("TERM=xterm-256color\n"), "{s}");
    assert!(!s.contains("UTC"), "{s}");

    // A program that declares no configuration is shown none, whatever the shell can read.
    let wc = shown(|o| write_preview(&endowment(Program::Wc), &Holdings::default(), Some(page), o));
    assert!(!wc.contains("config") && !wc.contains("TZ="), "{wc}");

    // And a shell with no view says it cannot show them, rather than printing nothing and
    // letting a reader take the absence of values for the absence of configuration.
    let blind =
        shown(|o| write_preview(&endowment(Program::Printenv), &Holdings::default(), None, o));
    assert!(blind.contains("cannot show their values"), "{blind}");
    assert!(!blind.contains("TZ="), "{blind}");
}

/// The values reach a pipeline stage's preview too, which is the path `caps` takes for any line
/// with an operator on it: the page is threaded through [`write_capabilities`], not only
/// [`write_preview`].
#[test]
fn caps_on_a_pipeline_threads_the_page_to_the_stage_that_declares_it() {
    let bytes = unusual_page();
    // SAFETY: as above.
    let page = unsafe { ConfigPage::new(bytes.as_ptr() as u64) };
    let s = shown(|o| {
        write_capabilities(
            b"printenv | wc",
            128,
            Holdings::default(),
            None,
            Some(page),
            &mut |_| Ok(NameSet::empty()),
            o,
        );
    });
    assert!(s.contains("TZ=America/Los_Angeles"), "{s}");
    assert_eq!(
        s.matches("TZ=").count(),
        1,
        "only printenv declares it: {s}"
    );
}

/// The shell's own row: held `READ` without `GRANT` at the named slot, with the values, and no
/// row at all for a shell that was given no view.
#[test]
fn the_shells_config_view_is_one_row_and_it_cannot_pass_it_on() {
    let bytes = unusual_page();
    // SAFETY: as above.
    let page = unsafe { ConfigPage::new(bytes.as_ptr() as u64) };
    let with = shown(|o| write_holdings(128, Holdings::default(), None, Some(page), o));
    assert!(
        with.contains("cap 21 frame     config     READ only, NOT delegable"),
        "{with}"
    );
    assert!(with.contains("TZ=America/Los_Angeles"), "{with}");
    let without = shown(|o| write_holdings(128, Holdings::default(), None, None, o));
    assert!(!without.contains("config"), "{without}");
}

#[test]
fn uuid_says_the_entropy_endpoint_is_not_on_the_line() {
    // `clock`'s family again: entropy's use is invisible from outside, so only this row says it.
    let s = shown(|o| write_preview(&endowment(Program::Uuid), &Holdings::default(), None, o));
    assert!(s.contains("cap 9  endpoint  entropy"), "{s}");
    // `WRITE`: the right to `CALL`, and no wider right.
    assert!(s.contains("WRITE"), "{s}");
    // The half that carries the claim in English, for a reader who does not know the rights.
    assert!(s.contains("draws no randomness at all"), "{s}");
    // And a program that declares no entropy is not given a row that says it has one.
    assert!(
        !shown(|o| write_preview(&endowment(Program::Date), &Holdings::default(), None, o))
            .contains("entropy")
    );
    assert!(
        !shown(|o| write_preview(&endowment(Program::Wc), &Holdings::default(), None, o))
            .contains("entropy")
    );
}

/// **`caps reboot` names the authority, and nothing else's preview does** (milestone 805, exit
/// criterion 3): the reset is the one grant whose effect leaves no machine to inspect afterwards,
/// so the preview is the only place a person meets it.
#[test]
fn the_reboot_preview_names_the_reboot_object_and_nothing_else_has_it() {
    let s = shown(|o| write_preview(&endowment(Program::Reboot), &Holdings::default(), None, o));
    assert!(
        s.contains("cap 13 reboot    WRITE. restart the machine"),
        "{s}"
    );
    assert!(s.contains("cap 14 endpoint  sync   WRITE"), "{s}");
    for p in Program::ALL.iter().filter(|p| **p != Program::Reboot) {
        let s = shown(|o| write_preview(&endowment(*p), &Holdings::default(), None, o));
        assert!(
            !s.contains("cap 13") && !s.contains("cap 14"),
            "{}: {s}",
            p.name()
        );
    }
}

#[test]
fn exactly_three_programs_declare_entropy() {
    // The manifest table is the whole of who may draw random bytes here (§44 (entropy is a
    // capability)), so a new declaring program must change this count. `std_exerciser` checks
    // `SystemRng`; `jig`'s TLS handshake draws its key share (notes/shell/caps-preview.md).
    let mut declared = 0;
    for &p in Program::ALL {
        if p.manifest().entropy {
            assert!(
                matches!(p, Program::Uuid | Program::StdExerciser | Program::Jig),
                "{} declares entropy",
                p.name()
            );
            declared += 1;
        }
    }
    assert_eq!(declared, 3);
}

/// **`caps` names the slots the progenitor will use, and for a `std` program those are
/// `std_runtime_contract`'s** (milestone 595 (the shell runs a `std` program)). A preview
/// printing the native positions would put the output at slot 0, where a `std` child holds its
/// heap.
#[test]
fn a_std_program_previews_its_fixed_slots() {
    let s = shown(|o| {
        write_preview(
            &endowment(Program::StdExerciser),
            &Holdings::default(),
            None,
            o,
        );
    });
    assert!(
        s.contains("cap 0  untyped   heap. the 384-page region"),
        "{s}"
    );
    assert!(
        s.contains("cap 1  endpoint  result   stdout and stderr"),
        "{s}"
    );
    assert!(s.contains("cap 5  frame     clock"), "{s}");
    assert!(s.contains("cap 6  endpoint  entropy  WRITE"), "{s}");
    assert!(s.contains("cap 7  frame     config"), "{s}");
    // Its words (milestone 205, DECISIONS §170), worded as bytes rather than authority.
    assert!(s.contains("cap 8  frame     args     read-only"), "{s}");
    assert!(s.contains("grant none of them"), "{s}");
    assert!(!s.contains("cap 0  endpoint"), "{s}");
    // And a native program's rows did not move.
    let u = shown(|o| write_preview(&endowment(Program::Uuid), &Holdings::default(), None, o));
    assert!(
        u.contains("cap 0  endpoint  result   report its answer back"),
        "{u}"
    );
    assert!(u.contains("cap 9  endpoint  entropy  WRITE"), "{u}");
    assert!(!u.contains("args"), "{u}");
}

#[test]
fn exactly_two_programs_declare_the_network() {
    // `exactly_three_programs_declare_entropy`'s reason, one authority over: a new program that
    // reaches the network (milestone 590 (the booted system starts its network stack)) must
    // change this list.
    let declaring: Vec<Program> = Program::ALL
        .iter()
        .copied()
        .filter(|p| p.manifest().network)
        .collect();
    assert_eq!(declaring, [Program::NetworkEchoClient, Program::Jig]);
}

#[test]
fn the_preview_shows_the_network_only_where_it_is_declared() {
    // What `caps` prints is what the progenitor's spawn service reads (`Manifest::network`),
    // so the row appearing for the witness would be the preview admitting an over-grant.
    let shown_for =
        |p: Program| shown(|o| write_preview(&endowment(p), &Holdings::default(), None, o));
    assert!(shown_for(Program::NetworkEchoClient).contains("cap 10 endpoint  network  WRITE"));
    // A `std` program holds it at the layout's slot, with the region its sockets are made from.
    // `write_preview` is the unnarrowed manifest: what the verb table can choose from.
    let jig = shown_for(Program::Jig);
    assert!(jig.contains("cap 2  endpoint  network  WRITE"), "{jig}");
    assert!(jig.contains("cap 3  untyped   sockets"), "{jig}");
    assert!(jig.contains("cap 15 endpoint  installer  WRITE"), "{jig}");
    // Only `jig` resolves, and any name (milestone 892 (a std program at the prompt holds the
    // network and a resolver), calef's Q5 on #1884).
    assert!(
        jig.contains("cap 9  endpoint  resolver  WRITE. it may look up the addresses of any name"),
        "{jig}"
    );
    assert!(jig.contains("cannot vouch for bytes"), "{jig}");
    for &p in Program::ALL {
        if p != Program::NetworkEchoClient && p != Program::Jig {
            assert!(
                !shown_for(p).contains("network  WRITE"),
                "`caps {}` shows a network it does not declare",
                p.name()
            );
            assert!(
                !shown_for(p).contains("resolver  WRITE"),
                "`caps {}` shows a resolver it is not granted",
                p.name()
            );
        }
    }
}

#[test]
fn every_preview_names_where_the_output_goes() {
    // The output's destination is a capability, so `caps` can name it.
    for sink in [line::Sink::Report, line::Sink::Pipe] {
        let mut e = endowment(Program::Date);
        e.sink = sink;
        assert!(
            shown(|o| write_preview(&e, &Holdings::default(), None, o)).contains("    output   "),
            "{sink:?} left the destination unnamed"
        );
    }
}

#[test]
fn caps_with_no_tail_is_the_shells_own_endowment() {
    let tail = shown(|o| {
        write_capabilities(
            b"   ",
            128,
            Holdings::default(),
            None,
            None,
            &mut |_| Ok(NameSet::empty()),
            o,
        );
    });
    let direct = shown(|o| write_holdings(128, Holdings::default(), None, None, o));
    assert_eq!(tail, direct);
}

#[test]
fn caps_previews_every_stage_of_a_pipeline() {
    let s = shown(|o| {
        write_capabilities(
            b"date | wc",
            128,
            Holdings::default(),
            None,
            None,
            &mut |_| Ok(NameSet::empty()),
            o,
        );
    });
    // Two stages, two tables, and the pipe named on both sides: the first stage's output is an
    // endpoint into the second, and the second's input is the first's output. Reading one stage
    // at a time could not have shown that.
    assert!(s.contains("date would grant"), "{s}");
    assert!(s.contains("wc would grant"), "{s}");
    assert!(s.contains("the rendezvous IS the pipe"), "{s}");
    assert!(s.contains("the previous stage's output"), "{s}");
}

/// The preview's `arg` line must follow the manifest for every program, so it is a sweep over the
/// whole enum rather than a test naming the one program that takes an argument today. The line it
/// types supplies every operand the manifest asks for that the planner can resolve without a boot:
/// a `--mem` at the declared minimum, and a file name for a declared input or file.
#[test]
fn the_arg_line_follows_the_manifest_for_every_program() {
    let holdings = Holdings {
        directory: true,
        ..Holdings::default()
    };
    for &prog in Program::ALL {
        let m = prog.manifest();
        let takes_arg = m.program_argument == ProgramArgumentDeclaration::Required;
        let mut line = String::from(prog.name());
        if let MemoryGrantDeclaration::Required { min, .. } = m.memory_grant {
            line.push_str(&std::format!(" --mem {min}"));
        }
        line.push_str(" 21");
        let wants_a_name = matches!(m.input, InputDeclaration::Required { .. })
            || matches!(m.file, FileGrantDeclaration::Required { .. });
        if wants_a_name {
            line.push_str(" report.txt");
        }
        let s = shown(|o| {
            write_capabilities(
                line.as_bytes(),
                128,
                holdings,
                None,
                None,
                &mut |_| Ok(NameSet::empty()),
                o,
            );
        });
        // A program that refuses an argument is refused before any table is printed, so the
        // preview only has an `arg` line to get wrong when the manifest allows one.
        if takes_arg {
            assert!(
                s.contains("arg    21"),
                "{} takes an argument and the preview did not show it:\n{s}",
                prog.name(),
            );
        } else {
            assert!(
                !s.contains("arg    21"),
                "{} takes no argument and the preview showed one:\n{s}",
                prog.name(),
            );
        }
    }
}

#[test]
fn caps_refuses_a_pipeline_whose_stage_has_no_bytes_to_pipe() {
    // `least_authority_demo` answers with a number, not a stream, so there is nothing for `|` to carry. The
    // preview refuses the same line the prompt would, which is the property that makes `caps`
    // worth typing before a command rather than after it.
    let s = shown(|o| {
        write_capabilities(
            b"least_authority_demo 7 | wc",
            128,
            Holdings::default(),
            None,
            None,
            &mut |_| Ok(NameSet::empty()),
            o,
        );
    });
    assert!(s.starts_with("  least_authority_demo: "), "{s}");
    assert!(!s.contains("would grant"), "{s}");
}

#[test]
fn caps_refuses_the_line_it_would_have_refused_at_the_prompt() {
    // A shell holding no directory cannot back a name, and the preview says so rather than
    // printing a grant that would not happen.
    let s = shown(|o| {
        write_capabilities(
            b"wc report.txt",
            128,
            Holdings::default(),
            None,
            None,
            &mut |_| Ok(NameSet::empty()),
            o,
        );
    });
    assert!(s.contains("wc: "), "{s}");
    assert!(!s.contains("would grant"), "{s}");
}

#[test]
fn caps_says_so_when_the_tail_is_not_an_invocation() {
    let s = shown(|o| {
        write_capabilities(
            b"help",
            128,
            Holdings::default(),
            None,
            None,
            &mut |_| Ok(NameSet::empty()),
            o,
        );
    });
    assert!(s.contains("caps previews a command's grant"), "{s}");
}

/// **`caps time <command>` previews the command, unchanged** (milestone 86).
///
/// The assertion is equality with the preview of the same line untimed, not a substring, because
/// the claim is that `time` moves no authority at all. One extra line of prose says the prefix
/// was seen; every row of the table has to be identical, or the shell would be previewing one
/// endowment and spawning another.
#[test]
fn caps_of_a_timed_command_previews_the_command_itself() {
    let preview = |line: &'static [u8]| {
        shown(|o| {
            write_capabilities(
                line,
                128,
                Holdings::default(),
                Some(5),
                None,
                &mut |_| Ok(NameSet::empty()),
                o,
            );
        })
    };
    let timed = preview(b"time memory_grant_depleter --mem 16");
    let plain = preview(b"memory_grant_depleter --mem 16");
    assert_eq!(
        timed.strip_prefix("  time grants nothing; what it would run:\n"),
        Some(plain.as_str()),
        "timing a command must not change one row of what it would be granted",
    );

    // Nested prefixes collapse rather than recursing, and a prefix with nothing after it is the
    // same complaint the prompt makes.
    assert_eq!(preview(b"time time memory_grant_depleter --mem 16"), timed);
    assert!(preview(b"time").contains("name a command to time"));
}

/// **The preview names the second stream even when the line does not**, which is the row that
/// stops `caps date > when.txt` from being a half-truth: the answer goes into the file and the
/// complaint does not, and a reader can see both destinations before running anything.
#[test]
fn a_declarer_shows_where_its_second_stream_goes() {
    let mut e = endowment(Program::Date);
    e.sink = line::Sink::File(
        grant_plan::FileGrant {
            which: grant_plan::nav::Which::A,
            directory: Cwd::root(),
            name: grant_plan::expand::Name::new(b"when.txt").expect("a nameable file"),
            writable: true,
        },
        line::Mode::Truncate,
    );
    let s = shown(|o| write_preview(&e, &Holdings::default(), None, o));
    assert!(s.contains("    output   when.txt"), "{s}");
    assert!(
        // And the row says the destination is **not** this shell, which is what the terminal's
        // own sink adapter exists for: those bytes reach the screen without passing through
        // here, so nothing the shell does to the output can reach them.
        s.contains("    diags    the terminal's own sink"),
        "{s}"
    );

    // And a program that declares none has no row at all: there is no second stream to hide,
    // so inventing a line about one would be the preview claiming more than the manifest does.
    let s = shown(|o| write_preview(&endowment(Program::Wc), &Holdings::default(), None, o));
    assert!(!s.contains("diags"), "{s}");
}

/// `caps` refuses the `2>` the prompt would refuse, which is what makes previewing worth doing
/// before a command rather than after it.
#[test]
fn caps_refuses_a_second_stream_the_program_never_declared() {
    let s = shown(|o| {
        write_capabilities(
            b"least_authority_demo 7 2> err.txt",
            128,
            Holdings {
                directory: true,
                mounted: None,
                cwd: Cwd::root(),
                binds: nav::Bindings::none(),
            },
            None,
            None,
            &mut |_| Ok(NameSet::empty()),
            o,
        );
    });
    assert!(s.starts_with("  least_authority_demo: "), "{s}");
    assert!(s.contains("declares no second output"), "{s}");
    assert!(!s.contains("would grant"), "{s}");
}

/// A manifest note that asks for a word of output, or for none, says so, and a byte stream is
/// the third wording. Each of the three is a different claim about what a program can do.
#[test]
fn a_manifest_note_names_the_kind_of_output_it_asks_for() {
    let asks = |output| {
        shown(|o| {
            let m = grant_plan::Manifest {
                output,
                ..grant_plan::NO_NOTE_MANIFEST
            };
            write_note_asks(Some(m), o);
        })
    };
    assert!(asks(OutputDeclaration::Words).contains("asks for: a word of output"));
    assert!(asks(OutputDeclaration::Silent).contains("asks for: no output"));
    assert!(asks(OutputDeclaration::Bytes).contains("asks for: output bytes"));
}

/// **A slot number's column is three wide**, so the kind after it lines up: two spaces after a
/// one-digit slot and one after a two-digit one, 10 being the first.
#[test]
fn a_slot_column_pads_one_and_two_digit_slots_to_one_width() {
    assert_eq!(shown(|o| write_slot(9, o)), "    cap 9  ");
    assert_eq!(shown(|o| write_slot(10, o)), "    cap 10 ");
}

/// **The index store's rows say what the verb gets** (milestone 801 (packages over the
/// internet)): `jig update` the copy read and written, `jig list` read only, and both at the
/// directory slot with the sources page. A line that designates a directory of its own takes
/// that slot, so the store is not printed there.
#[test]
fn the_jig_preview_shows_the_index_store_the_verb_holds_and_yields_it_to_a_designated_directory() {
    let preview = |line: &[u8]| {
        shown(|o| {
            write_capabilities(
                line,
                128,
                Holdings::default(),
                None,
                None,
                &mut |_| Ok(NameSet::empty()),
                o,
            );
        })
    };
    let update = preview(b"jig update");
    assert!(
        update.contains("cap 4  endpoint  dir      /activation/indexes  read and write."),
        "{update}"
    );
    assert!(
        update.contains("cap 18 frame     sources  read-only."),
        "{update}"
    );
    let list = preview(b"jig list");
    assert!(
        list.contains("cap 4  endpoint  dir      /activation/indexes  read-only."),
        "{list}"
    );
    // `list -u` (milestone 809): `activation/` itself, read only, and no index store or sources.
    let outdated = preview(b"jig list -u");
    assert!(
        outdated.contains("cap 4  endpoint  dir      /activation  read-only."),
        "{outdated}"
    );
    assert!(!outdated.contains("/activation/indexes"), "{outdated}");
    assert!(!outdated.contains("sources"), "{outdated}");
    assert!(!list.contains("/activation  read-only"), "{list}");
    let mut designated = endowment(Program::Jig);
    designated.directory = Some(grant_plan::DirectoryGrant {
        which: grant_plan::nav::Which::A,
        directory: Cwd::root(),
        names: listing(&[b"x.nifepkg"]),
        subtree: false,
    });
    let own = shown(|o| write_preview(&designated, &Holdings::default(), None, o));
    assert!(!own.contains("/activation/indexes"), "{own}");
    assert!(!own.contains("sources"), "{own}");
}
