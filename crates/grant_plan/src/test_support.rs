//! **What more than one test file leans on**: shims that plan a line with the expansion and
//! streams a test does not care about, the holdings most tests plan against, and the paths they
//! name. Imported by name, because `plan` and `plan_against` here shadow the crate's own. Split
//! out of the crate root's tests by milestone 842 (the grant-plan crate root is split along its
//! seams).
//!
//! Name: provisional (milestone 842, 2026-10-11).

use crate::expansion::Expansion;
use crate::*;

/// **The programs the tests plan against**: a small catalog of the shapes a caller's manifests
/// take, one row per shape, each named by the word a line types to run it. A caller resolves a
/// name to a manifest its own way; this is the tests' way, so the crate tests with no catalog of
/// anyone else's.
pub(crate) mod rows {
    use crate::*;

    /// What a row starts from: no grant, no option, no output, nothing endowed.
    const NOTHING: Manifest = Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::Silent,
        input: InputDeclaration::Forbidden,
        reports: false,
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

    /// `count N`: one integer argument, answered in words.
    pub(crate) const COUNT: Manifest = Manifest {
        program_argument: ProgramArgumentDeclaration::Required,
        output: OutputDeclaration::Words,
        reports: true,
        ..NOTHING
    };

    /// `deplete --mem N`: a memory grant of one to sixty-four pages, answered in words.
    pub(crate) const DEPLETE: Manifest = Manifest {
        memory_grant: MemoryGrantDeclaration::Required {
            minimum: 1,
            maximum: 64,
        },
        output: OutputDeclaration::Words,
        reports: true,
        ..NOTHING
    };

    /// `heed`: silent and interruptible, so `^C` reaches it.
    pub(crate) const HEED: Manifest = Manifest {
        interruptible: true,
        ..NOTHING
    };

    /// `date`: bytes with a second stream at [`DIAGNOSTICS_SLOT`], and the clock.
    pub(crate) const DATE: Manifest = Manifest {
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        reports: true,
        clock: true,
        ..NOTHING
    };

    /// `rm [-rfv] NAME`: a directory grant that `-r` widens to the subtree, answered in bytes.
    pub(crate) const RM: Manifest = Manifest {
        directory: DirectoryGrantDeclaration::Required {
            subtree_option: Some(b'r'),
        },
        short_options: ShortOptions::new(b"rfv"),
        output: OutputDeclaration::Bytes,
        reports: true,
        ..NOTHING
    };

    /// `wc`: reads an input to its end before it writes, and writes bytes.
    pub(crate) const WC: Manifest = Manifest {
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Required {
            writes_while_reading: false,
        },
        reports: true,
        ..NOTHING
    };

    /// `search WORDS`: a `std` program that hears words, granted reading what they designate.
    pub(crate) const SEARCH: Manifest = Manifest {
        program_argument: ProgramArgumentDeclaration::Words(WordGrant::ReadOnly),
        output: OutputDeclaration::Bytes,
        reports: true,
        clock: true,
        configuration: true,
        entropy: true,
        startup: StartupContract::Std,
        ..NOTHING
    };

    /// Every row, with the word that runs it.
    pub(crate) const ALL: &[(&[u8], Manifest)] = &[
        (b"count", COUNT),
        (b"deplete", DEPLETE),
        (b"heed", HEED),
        (b"date", DATE),
        (b"rm", RM),
        (b"wc", WC),
        (b"search", SEARCH),
    ];

    /// The row a line's first word names, or `None` for a word no row answers to.
    pub(crate) fn named(name: &[u8]) -> Option<Manifest> {
        ALL.iter().find(|(n, _)| *n == name).map(|&(_, m)| m)
    }
}

/// **A line planned against the tests' catalog**, the way a shell plans one: the two answers that
/// come before a name, then the row the first word names, then this crate's planner against that
/// row's manifest.
pub(crate) fn plan_named_stage(
    run: &ParsedInvocation<'_>,
    shell_holdings: ShellHoldings,
    expanded: Expansion,
    streams: Streams,
) -> Result<Endowment, Refusal> {
    if let Some(r) = run.misquoted {
        return Err(r);
    }
    if run.program == b"swish" {
        return Err(Refusal::IsolationNotBuilt);
    }
    let manifest = rows::named(run.program).ok_or(Refusal::NoSuchProgram)?;
    plan_against_with_streams(run, manifest, &shell_holdings, &expanded, &streams)
}

/// [`plan_named_stage`] with no operators on the line.
pub(crate) fn plan_named(
    run: &ParsedInvocation<'_>,
    shell_holdings: ShellHoldings,
    expanded: Expansion,
) -> Result<Endowment, Refusal> {
    plan_named_stage(run, shell_holdings, expanded, Streams::default())
}

/// Plan a line with **nothing expanded**, which is every line that has no pattern on it. The two
/// shims shadow the real functions so the tests below read as they did before the globbing lane,
/// and the tests that *do* expand pass an [`Expansion`] explicitly and are grouped at the end.
pub(crate) fn plan(
    run: &ParsedInvocation<'_>,
    shell_holdings: ShellHoldings,
) -> Result<Endowment, Refusal> {
    plan_named(run, shell_holdings, Expansion::none())
}

pub(crate) fn plan_against(
    run: &ParsedInvocation<'_>,
    m: Manifest,
    shell_holdings: ShellHoldings,
) -> Result<Endowment, Refusal> {
    crate::plan_against(run, m, &shell_holdings, &Expansion::none())
}

/// A manifest no shipped program declares yet: one endowed a readable file. Checked through
/// [`plan_against`] so the `FileGrantDeclaration::Required` logic is live and tested rather than a branch
/// nothing reaches, which is the same door milestone 23 will come through.
pub(crate) const READS_A_FILE: Manifest = Manifest {
    program_argument: ProgramArgumentDeclaration::Forbidden,
    memory_grant: MemoryGrantDeclaration::Forbidden,
    file: FileGrantDeclaration::Required { writable: false },
    directory: DirectoryGrantDeclaration::Forbidden,
    short_options: NO_SHORT_OPTIONS,
    output: OutputDeclaration::Words,
    input: InputDeclaration::Forbidden,
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

/// A shell that WAS granted a directory to narrow, standing at its root.
pub(crate) const WITH_DIRECTORY: ShellHoldings = ShellHoldings {
    directory: true,
    mounted: None,
    working_directory: navigation::DirectoryPath::root(),
    binds: navigation::Bindings::none(),
};

// ---- milestone 154 (a process that holds two directory capabilities): one tree, a mount ----

pub(crate) fn at(components: &[&[u8]]) -> navigation::DirectoryPath {
    let mut c = navigation::DirectoryPath::root();
    for name in components {
        assert!(c.descend(name));
    }
    c
}

/// A shell with a second tree mounted at `/media/usb`, standing at the root.
pub(crate) fn mounted() -> ShellHoldings {
    ShellHoldings {
        mounted: MountedTree::mounted_at(at(&[b"media", b"usb"])),
        ..WITH_DIRECTORY
    }
}

// ---- milestone 50 (pipes and redirection): the operators, checked against the manifests that ship ----

/// Plan one stage of a line with the operators' answer folded in. The shim the operator tests
/// use, for the reason the other two shims exist: the lines below should read like command
/// lines and not like plumbing.
pub(crate) fn plan_streams(
    run: &ParsedInvocation<'_>,
    shell_holdings: ShellHoldings,
    streams: Streams,
) -> Result<Endowment, Refusal> {
    plan_named_stage(run, shell_holdings, Expansion::none(), streams)
}

/// Parse a whole line and plan every stage of it, which is what the shell does. Returns the
/// endowments in order, or the first refusal with the stage it came from.
///
/// It exists so the tests below can be written as the lines a person types. **The redirection
/// targets are designated here too**, which is the part worth reading: a `>` name becomes a
/// writable [`FileGrant`] and a `<` name a readable one, and both are resolved against the same
/// `ShellHoldings` an operand would be.
/// What [`plan_line`] answers: the planned stages and how many there are, or the stage index
/// that refused and why. Named because the tuple is long enough that clippy is right about it.
pub(crate) type Planned =
    Result<([Option<Endowment>; line::MAXIMUM_STAGES], usize), (usize, Refusal)>;

pub(crate) fn plan_line(text: &[u8], shell_holdings: ShellHoldings) -> Planned {
    let l = line::split(text).map_err(|r| (0, r))?;
    let out = match l.output {
        Some(t) => {
            Some(output_file_grant(t, &shell_holdings).map_err(|r| (l.stage_count() - 1, r))?)
        }
        None => None,
    };
    let inp = match l.input {
        Some(t) => Some(input_file_grant(t, &shell_holdings).map_err(|r| (0, r))?),
        None => None,
    };
    // A `2>` target is designated exactly as a `>` target is, and it is one destination for the
    // whole line: the shell mints one diagnostic endpoint and every declaring stage writes to
    // it, so every stage sees the same answer here.
    let diag = match l.diagnostics {
        Some(t) => Some((
            output_file_grant(t, &shell_holdings).map_err(|r| (l.stage_count() - 1, r))?,
            l.diagnostics_mode(),
        )),
        None => None,
    };
    let mut plans: [Option<Endowment>; line::MAXIMUM_STAGES] = [None; line::MAXIMUM_STAGES];
    for (i, stage) in l.stages().iter().enumerate() {
        let CommandLine::Run(run) = parse(stage) else {
            return Err((i, Refusal::NotAPipelineStage));
        };
        let streams = Streams {
            sink: l.stage_sink(i, out),
            source: l.stage_source(i, inp),
            diagnostics: diag,
        };
        plans[i] = Some(plan_streams(&run, shell_holdings, streams).map_err(|r| (i, r))?);
    }
    Ok((plans, l.stage_count()))
}

/// **A line naming a file one byte longer than a grant can carry**, `<prefix>xxx...x`, built in
/// `buf` because this crate has no allocator in tests either. Every test that proves the name
/// bound is refused uses it, so the bound is [`crate::MAXIMUM_FILE_NAME_BYTES`] and never a spelled-out
/// literal that a raised bound would silently make legal.
pub(crate) fn with_too_long_name<'a>(prefix: &[u8], buf: &'a mut [u8; 512]) -> &'a [u8] {
    let n = prefix.len() + crate::MAXIMUM_FILE_NAME_BYTES + 1;
    buf[..prefix.len()].copy_from_slice(prefix);
    buf[prefix.len()..n].fill(b'x');
    &buf[..n]
}
