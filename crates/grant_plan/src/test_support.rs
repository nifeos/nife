//! **What more than one test file leans on**: shims that plan a line with the expansion and
//! streams a test does not care about, the holdings most tests plan against, and the paths they
//! name. Imported by name, because `plan` and `plan_against` here shadow the crate's own. Split
//! out of the crate root's tests by milestone 842 (the grant-plan crate root is split along its
//! seams).
//!
//! Name: provisional (milestone 842, 2026-10-11).

use program_table::Program;

use crate::expansion::Expansion;
use crate::*;

/// **A line planned against nife's program table**, the way `program_table::plan_stage` plans it:
/// the two answers that come before a name, then the row the first word names, then this crate's
/// planner against that row's manifest. Spelled here rather than called there because
/// `program_table` links the library build of this crate, whose `Endowment` is not this test
/// build's; only the row's `Manifest`, which is `grant_contract`'s, crosses.
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
    let program = Program::from_name(run.program).ok_or(Refusal::NoSuchProgram)?;
    plan_against_with_streams(run, program.manifest(), shell_holdings, expanded, streams)
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
    crate::plan_against(run, m, shell_holdings, Expansion::none())
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
            Some(output_file_grant(t, shell_holdings).map_err(|r| (l.stage_count() - 1, r))?)
        }
        None => None,
    };
    let inp = match l.input {
        Some(t) => Some(input_file_grant(t, shell_holdings).map_err(|r| (0, r))?),
        None => None,
    };
    // A `2>` target is designated exactly as a `>` target is, and it is one destination for the
    // whole line: the shell mints one diagnostic endpoint and every declaring stage writes to
    // it, so every stage sees the same answer here.
    let diag = match l.diagnostics {
        Some(t) => Some((
            output_file_grant(t, shell_holdings).map_err(|r| (l.stage_count() - 1, r))?,
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
