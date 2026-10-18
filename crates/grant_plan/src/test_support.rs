//! **What more than one test file leans on**: shims that plan a line with the expansion and
//! streams a test does not care about, the holdings most tests plan against, and the paths they
//! name. Imported by name, because `plan` and `plan_against` here shadow the crate's own. Split
//! out of the crate root's tests by milestone 842 (the grant-plan crate root is split along its
//! seams).
//!
//! Name: provisional (milestone 842, 2026-10-11).

use crate::expand::Expansion;
use crate::*;

/// Plan a line with **nothing expanded**, which is every line that has no pattern on it. The two
/// shims shadow the real functions so the tests below read as they did before the globbing lane,
/// and the tests that *do* expand pass an [`Expansion`] explicitly and are grouped at the end.
pub(crate) fn plan(run: &RunSpec<'_>, holds: Holdings) -> Result<Endowment, Refusal> {
    super::plan(run, holds, Expansion::none())
}

pub(crate) fn plan_against(
    run: &RunSpec<'_>,
    prog: Prog,
    m: Manifest,
    holds: Holdings,
) -> Result<Endowment, Refusal> {
    super::plan_against(run, prog, m, holds, Expansion::none())
}

/// A manifest no shipped program declares yet: one endowed a readable file. Checked through
/// [`plan_against`] so the `FileSpec::Required` logic is live and tested rather than a branch
/// nothing reaches, which is the same door milestone 23 will come through.
pub(crate) const READS_A_FILE: Manifest = Manifest {
    program_argument: ArgSpec::Forbidden,
    memory_grant: MemSpec::Forbidden,
    file: FileSpec::Required { writable: false },
    directory: DirSpec::Forbidden,
    flags: NO_FLAGS,
    output: OutputSpec::Words,
    input: InputSpec::Forbidden,
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
    indexes: IndexStore::Forbidden,
    runtime: Runtime::Native,
};

/// A shell that WAS granted a directory to narrow, standing at its root.
pub(crate) const WITH_DIR: Holdings = Holdings {
    dir: true,
    second: None,
    cwd: nav::Cwd::root(),
    binds: nav::Bindings::none(),
};

// ---- milestone 154 (a process that holds two directory capabilities): one tree, a mount ----

pub(crate) fn at(components: &[&[u8]]) -> nav::Cwd {
    let mut c = nav::Cwd::root();
    for name in components {
        assert!(c.descend(name));
    }
    c
}

/// A shell with a second tree mounted at `/media/usb`, standing at the root.
pub(crate) fn mounted() -> Holdings {
    Holdings {
        second: SecondDir::mounted_at(at(&[b"media", b"usb"])),
        ..WITH_DIR
    }
}

// ---- milestone 50 (pipes and redirection): the operators, checked against the manifests that ship ----

/// Plan one stage of a line with the operators' answer folded in. The shim the operator tests
/// use, for the reason the other two shims exist: the lines below should read like command
/// lines and not like plumbing.
pub(crate) fn plan_streams(
    run: &RunSpec<'_>,
    holds: Holdings,
    streams: Streams,
) -> Result<Endowment, Refusal> {
    super::plan_stage(run, holds, Expansion::none(), streams)
}

/// Parse a whole line and plan every stage of it, which is what the shell does. Returns the
/// endowments in order, or the first refusal with the stage it came from.
///
/// It exists so the tests below can be written as the lines a person types. **The redirection
/// targets are designated here too**, which is the part worth reading: a `>` name becomes a
/// writable [`FileGrant`] and a `<` name a readable one, and both are resolved against the same
/// `Holdings` an operand would be.
/// What [`plan_line`] answers: the planned stages and how many there are, or the stage index
/// that refused and why. Named because the tuple is long enough that clippy is right about it.
pub(crate) type Planned = Result<([Option<Endowment>; line::MAX_STAGES], usize), (usize, Refusal)>;

pub(crate) fn plan_line(text: &[u8], holds: Holdings) -> Planned {
    let l = line::split(text).map_err(|r| (0, r))?;
    let out = match l.output {
        Some(t) => Some(redirect_target(t, holds, true).map_err(|r| (l.stage_count() - 1, r))?),
        None => None,
    };
    let inp = match l.input {
        Some(t) => Some(redirect_target(t, holds, false).map_err(|r| (0, r))?),
        None => None,
    };
    // A `2>` target is designated exactly as a `>` target is, and it is one destination for the
    // whole line: the shell mints one diagnostic endpoint and every declaring stage writes to
    // it, so every stage sees the same answer here.
    let diag = match l.diagnostics {
        Some(t) => Some((
            redirect_target(t, holds, true).map_err(|r| (l.stage_count() - 1, r))?,
            l.diagnostics_mode(),
        )),
        None => None,
    };
    let mut plans: [Option<Endowment>; line::MAX_STAGES] = [None; line::MAX_STAGES];
    for (i, stage) in l.stages().iter().enumerate() {
        let Command::Run(run) = parse(stage) else {
            return Err((i, Refusal::NotAPipelineStage));
        };
        let streams = Streams {
            sink: l.sink_for(i, out),
            source: l.source_for(i, inp),
            diagnostics: diag,
        };
        plans[i] = Some(plan_streams(&run, holds, streams).map_err(|r| (i, r))?);
    }
    Ok((plans, l.stage_count()))
}
