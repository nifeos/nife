//! **Whether a line can be granted**: `plan` and its variants, the stream checks,
//! `check_chain`, and the placing of a line's tokens into the slots a manifest declares.
//!
//! The crate root re-exports every public item (§272 (the grant-plan crate root splits into
//! private modules)).
//!
//! Name: provisional (milestone 842, 2026-10-11), as are its sibling modules' names.

use crate::expand::{Expansion, Name, NameSet};
use crate::line::{Sink, Source};
use crate::{
    CapabilityKind, DirectoryGrant, DirectoryGrantDeclaration, Endowment, FileGrant,
    FileGrantDeclaration, Holdings, InputDeclaration, Manifest, MemoryGrantDeclaration,
    ParsedInvocation, Program, ProgramArgumentDeclaration, Refusal, check_words, expand, line, nav,
    parse_u64,
};

/// Check a parsed invocation against the program's manifest and produce the exact [`Endowment`] to
/// grant, or the [`Refusal`] to print. The whole authority decision lives here: after this returns
/// `Ok`, the shell grants precisely what the `Endowment` names and the child can reach nothing
/// else.
///
/// The program is resolved first, because "there is no such program" is a fact about the system.
/// `expanded` is what the line's patterns matched, since the endowment is the set;
/// [`Expansion::none`] for a line with no pattern. See notes/glob-grant.md.
pub fn plan(
    run: &ParsedInvocation<'_>,
    holds: Holdings,
    expanded: Expansion,
) -> Result<Endowment, Refusal> {
    plan_stage(run, holds, expanded, Streams::default())
}

/// **What the operators decided for this stage**: which capability its output slot gets and which
/// its input slot gets. [`line::split`] and the shell between them produce it; [`plan_stage`] checks
/// it against the program's manifest.
///
/// It is a pair rather than two arguments because the two are one question asked in two directions,
/// and because a stage in the middle of a pipeline has both.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Streams {
    pub sink: Sink,
    pub source: Source,
    /// **The file a `2>` named, and what it does to what is in it**, or `None` for a line with no
    /// `2>` on it. It is the *question* the line asked; [`Endowment::diagnostics`] is the answer,
    /// because whether there is a second stream at all is the manifest's to say.
    pub diagnostics: Option<(FileGrant, line::Mode)>,
}

/// [`plan`], with the operators' answer folded in.
///
/// The stream check comes **after** the grant check and not before, deliberately. A line can be
/// wrong about both (`least_authority_demo > out.txt` names no integer *and* has no bytes to write), and the
/// grant refusal is the one that reads truer: what a program is endowed is a bigger fact about the
/// line than where its output was going to go.
pub fn plan_stage(
    run: &ParsedInvocation<'_>,
    holds: Holdings,
    expanded: Expansion,
    streams: Streams,
) -> Result<Endowment, Refusal> {
    // Ahead of resolving the program, for the reason [`ParsedInvocation::misquoted`] states: a line whose
    // words cannot be read has no reliable first word either.
    if let Some(r) = run.misquoted {
        return Err(r);
    }
    let prog = Program::from_name(run.program).ok_or(Refusal::NoSuchProgram)?;
    plan_against_with(run, prog, prog.manifest(), holds, expanded, streams)
}

/// [`plan`]'s second half, against an **explicit** manifest rather than the static table.
///
/// It exercises manifest shapes no shipped program declares, and it is the call a composer makes
/// with a component's own manifest (milestone 23 (a capability-routed component OS)).
///
/// **This is where the positional tokens acquire meaning**, in the order typed: the integer
/// argument first if the manifest declares one, then the file. A token past the last declared slot
/// is a refusal. Placement is by index, and the [`Expansion`] is keyed to it: a pattern with no
/// expansion is [`Refusal::Unexpanded`], and a token with no magic never consults the expansion.
/// See notes/glob-grant.md ("The shell expands before it plans").
pub fn plan_against(
    run: &ParsedInvocation<'_>,
    prog: Program,
    m: Manifest,
    holds: Holdings,
    expanded: Expansion,
) -> Result<Endowment, Refusal> {
    plan_against_with(run, prog, m, holds, expanded, Streams::default())
}

/// [`plan_against`], with the operators' answer. The one that does the work; the other two are the
/// spellings without a pipeline on the line.
pub fn plan_against_with(
    run: &ParsedInvocation<'_>,
    prog: Program,
    m: Manifest,
    holds: Holdings,
    expanded: Expansion,
    streams: Streams,
) -> Result<Endowment, Refusal> {
    // **A line whose quotes do not make sense is answered before anything else** (milestone 67),
    // because every other refusal below is a sentence about a word, and this is the one that says
    // the words could not be read. Ordering it after "no such program" would name a program the
    // user did not type.
    if let Some(r) = run.misquoted {
        return Err(r);
    }

    // A flag nothing knows, or a token past what any manifest could hold. Nothing about this
    // program's declaration can rescue it, so it is answered before the slots are filled.
    //
    // **Not for a program that hears words** (§170): its line is its argv, a flag-shaped token is
    // one of its words, and the shell has no business refusing an option it never had to know.
    let words = m.program_argument.hears_words();
    if run.unexpected.is_some() && !words {
        return Err(Refusal::Unexpected);
    }

    // The options, before any grant is placed: a letter this program does not declare is a refusal
    // at the prompt, never a letter passed along for the program to ignore. It comes first because
    // one of them may *widen the directory grant* below, so the line has to be known-good before
    // anything is decided about how much authority it moves.
    let short_options = plan_options(run, &m, words)?;

    let pos = run.positionals();
    let mut next = 0usize;

    // The argument takes the first positional, and it must be an integer: `least_authority_demo eight` is a
    // missing argument, not a file named "eight", because least_authority_demo's manifest declares no file.
    //
    // A pattern can never land here by accident: every magic byte (`*`, `?`, `[`, `\`) is a
    // non-digit, so `parse_u64` refuses a pattern before the expansion is ever consulted, and
    // `least_authority_demo *` is "needs an integer argument" rather than a grant of anything.
    let arg = plan_program_argument(run, &m, pos, &mut next)?;

    // The file grant takes the next positional. The name is the whole designation; the direction
    // comes from the manifest, which is why `wc report.txt` reads and `tee report.txt` writes
    // without the human typing a mode.
    let file = plan_file(&m, pos, &mut next, &holds, &expanded)?;

    // The directory grant takes the next positional, and it is resolved exactly as a file grant is:
    // the leading path is walked here, once, against where the shell is standing now, and the
    // result is recorded as a value. The child is handed a capability to the directory the names
    // are **in**, plus the names, because taking a name away is an operation on the directory that
    // holds it and no per-file capability can express it. See [`DirectoryGrantDeclaration`].
    let dir = plan_directory_grant(&m, short_options, pos, &mut next, &holds, &expanded)?;

    // **A program that hears words is granted what its words designate** (milestone 205, §170
    // (how a foreign program is told what to do) clauses 2 to 5): one directory grant at the
    // shell's current directory, filtered to the designated names, or the whole directory when a
    // word was `.`. A line that designated nothing grants nothing (calef's N1 ruling,
    // 2026-09-27T06:27Z). What the grant may do is the manifest's [`WordGrant`], which the shell
    // turns into rights and the progenitor clamps to the manifest it endows.
    let dir = words_directory_grant(&m, &holds, &expanded, dir)?;

    // The input operand: `wc report.txt` is `wc < report.txt` with the operator left out, which is
    // unambiguous because no manifest declares both a file and an input
    // (`no_program_declares_both_a_file_and_an_input`). An argument and an input compose by fixed
    // order. An explicit `<` wins, and the operand is then unplaceable below. See
    // notes/grant-expression/per-file-grants.md ("`wc report.txt`: the input operand").
    let mut streams = streams;
    plan_input_operand(&m, &mut streams, pos, &mut next, &holds, &expanded)?;

    // Anything left designates a slot this program does not have.
    if let Some(&extra) = pos.get(next) {
        return Err(unplaceable(extra, m));
    }

    // The memory grant. A flag, not a position, so it is checked last: it cannot be confused with
    // anything and it should not shadow a refusal about a resource.
    let mem_pages = plan_memory_grant(run, &m)?;

    let diagnostics = check_streams(m, streams)?;

    Ok(Endowment {
        program: prog,
        program_argument: arg,
        memory_grant_pages: mem_pages,
        file,
        directory: dir,
        short_options,
        sink: streams.sink,
        source: streams.source,
        diagnostics,
        reports: m.reports,
        interruptible: m.interruptible,
        writes_while_reading: matches!(
            m.input,
            InputDeclaration::Required {
                writes_while_reading: true
            }
        ),
    })
}

/// [`plan_against_with`]'s options: each letter on the line as its bit in `m`'s short options, or
/// [`Refusal::NoSuchOption`]. A program that hears words has none.
fn plan_options(run: &ParsedInvocation<'_>, m: &Manifest, words: bool) -> Result<u64, Refusal> {
    let mut short_options = 0u64;
    for &letter in run.options().iter().filter(|_| !words) {
        let bit = m
            .short_options
            .letters()
            .iter()
            .position(|&d| d == letter)
            .ok_or(Refusal::NoSuchOption)?;
        short_options |= 1 << bit;
    }
    Ok(short_options)
}

/// [`plan_against_with`]'s integer argument, taken from the positional at `next`.
fn plan_program_argument(
    run: &ParsedInvocation<'_>,
    m: &Manifest,
    pos: &[&[u8]],
    next: &mut usize,
) -> Result<u64, Refusal> {
    let arg = match m.program_argument {
        ProgramArgumentDeclaration::Required => {
            let first = pos.get(*next).ok_or(Refusal::ProgramArgumentRequired)?;
            let v = parse_u64(first).ok_or(Refusal::ProgramArgumentRequired)?;
            *next += 1;
            v
        }
        ProgramArgumentDeclaration::Forbidden => 0,
        // Every positional is a word of the argv the shell assembles at spawn ([`argv`]), so none
        // is left to place and none is unplaceable.
        ProgramArgumentDeclaration::Words(_) => {
            check_words(run.line())?;
            *next = pos.len();
            0
        }
    };
    Ok(arg)
}

/// [`plan_against_with`]'s file grant, designated by the positional at `next`.
fn plan_file(
    m: &Manifest,
    pos: &[&[u8]],
    next: &mut usize,
    holds: &Holdings,
    expanded: &Expansion,
) -> Result<Option<FileGrant>, Refusal> {
    let file = match m.file {
        FileGrantDeclaration::Forbidden => None,
        FileGrantDeclaration::Required { writable } => {
            let token = *pos.get(*next).ok_or(Refusal::FileRequired)?;
            let at = *next;
            *next += 1;
            // What the shell holds decides whether the designation can be backed at all, and it
            // wins over the name's shape: "there is nothing I hold that could grant this" is a
            // bigger fact than "and that name is too long".
            if !holds.directory {
                return Err(Refusal::NoSuchCapability(CapabilityKind::File));
            }
            let (which, dir, names) = designate(token, at, holds, expanded)?;
            // One file, so a pattern that matched more than one has designated something this
            // program cannot be handed. The count is the designation; nothing was denied.
            let name = names.only().ok_or(Refusal::AmbiguousFile)?;
            Some(FileGrant {
                which,
                directory: dir,
                name: Name::new(name).ok_or(Refusal::FileNotNameable)?,
                writable,
            })
        }
    };
    Ok(file)
}

/// [`plan_against_with`]'s directory grant, designated by the positional at `next`, widened to the
/// subtree when its recursion option is in `short`.
fn plan_directory_grant(
    m: &Manifest,
    short_options: u64,
    pos: &[&[u8]],
    next: &mut usize,
    holds: &Holdings,
    expanded: &Expansion,
) -> Result<Option<DirectoryGrant>, Refusal> {
    let dir = match m.directory {
        DirectoryGrantDeclaration::Forbidden => None,
        DirectoryGrantDeclaration::Required { subtree_option } => {
            let token = *pos.get(*next).ok_or(Refusal::PathRequired)?;
            let at = *next;
            *next += 1;
            if !holds.directory {
                return Err(Refusal::NoSuchCapability(CapabilityKind::File));
            }
            let (which, dir, names) = designate(token, at, holds, expanded)?;
            // Typing the recursion option is what widens the capability from "take names out of
            // this directory" to "walk what is under it". A program run without it holds no way to
            // descend, so its recursion is not disabled by a branch anybody has to get right.
            let subtree = match subtree_option {
                Some(letter) => match m.short_options.letters().iter().position(|&d| d == letter) {
                    Some(bit) => short_options & (1 << bit) != 0,
                    // A manifest naming an option it does not declare is a bug in the manifest, and
                    // the safe reading is the narrow one: no widening.
                    None => false,
                },
                None => false,
            };
            Some(DirectoryGrant {
                which,
                directory: dir,
                names,
                subtree,
            })
        }
    };
    Ok(dir)
}

/// The directory grant a program that hears words is given for what they designated, or `dir`
/// unchanged for any other program.
fn words_directory_grant(
    m: &Manifest,
    holds: &Holdings,
    expanded: &Expansion,
    dir: Option<DirectoryGrant>,
) -> Result<Option<DirectoryGrant>, Refusal> {
    let dir = match (m.program_argument, expanded.designation()) {
        (ProgramArgumentDeclaration::Words(_), Some(d)) if !d.is_empty() => {
            if !holds.directory {
                return Err(Refusal::NoSuchCapability(CapabilityKind::File));
            }
            // The tree the shell stands in, and its position there: under a mount point that is
            // the second tree, from its own root, as every other grant records it.
            let (which, directory) = holds.locate(&holds.cwd);
            Some(DirectoryGrant {
                which,
                directory,
                names: if d.is_here() {
                    NameSet::empty()
                } else {
                    *d.names()
                },
                subtree: true,
            })
        }
        _ => dir,
    };
    Ok(dir)
}

/// [`plan_against_with`]'s input operand: the positional at `next` as `streams`' source, for a
/// program that declares an input and a line with no `<`.
fn plan_input_operand(
    m: &Manifest,
    streams: &mut Streams,
    pos: &[&[u8]],
    next: &mut usize,
    holds: &Holdings,
    expanded: &Expansion,
) -> Result<(), Refusal> {
    if matches!(m.input, InputDeclaration::Required { .. })
        && matches!(streams.source, Source::None)
        && let Some(&token) = pos.get(*next)
    {
        let at = *next;
        *next += 1;
        // What the shell holds decides whether the designation can be backed at all, and it wins
        // over the name's shape, exactly as it does for a file grant.
        if !holds.directory {
            return Err(Refusal::NoSuchCapability(CapabilityKind::File));
        }
        let (which, dir, names) = designate(token, at, holds, expanded)?;
        let name = names.only().ok_or(Refusal::AmbiguousFile)?;
        streams.source = Source::File(FileGrant {
            which,
            directory: dir,
            name: Name::new(name).ok_or(Refusal::FileNotNameable)?,
            // A reader reads. The direction is the manifest's here as everywhere else, and
            // `InputDeclaration` has only the one.
            writable: false,
        });
    }
    Ok(())
}

/// [`plan_against_with`]'s memory grant: `--mem` checked against what `m` declares.
fn plan_memory_grant(run: &ParsedInvocation<'_>, m: &Manifest) -> Result<u64, Refusal> {
    let mem_pages = match (m.memory_grant, run.memory_grant) {
        (MemoryGrantDeclaration::Forbidden, None) => 0,
        (MemoryGrantDeclaration::Forbidden, Some(_)) => return Err(Refusal::MemoryGrantForbidden),
        (MemoryGrantDeclaration::Required { .. }, None) => {
            return Err(Refusal::MemoryGrantRequired);
        }
        (MemoryGrantDeclaration::Required { min, max }, Some(n)) => {
            if n < min || n > max {
                return Err(Refusal::MemoryGrantOutOfRange { min, max });
            }
            n
        }
    };
    Ok(mem_pages)
}

/// **Resolve a redirection's name into the grant that backs it** (milestone 50).
///
/// `> report.txt` and `< report.txt` designate a file the same way an operand does: the leading
/// path is walked against where the shell stands **now**, once, and the result is a value. So this
/// is `designate` with the pattern case taken out, because [`line::split`] has already refused a
/// pattern here.
///
/// `writable` comes from the operator, the one direction this shell reads off the line: the program
/// is handed a sink or a source, not a file, and the direction is which end the shell builds.
pub fn redirect_target(
    token: &[u8],
    holds: Holdings,
    writable: bool,
) -> Result<FileGrant, Refusal> {
    if !holds.directory {
        return Err(Refusal::NoSuchCapability(CapabilityKind::File));
    }
    let (which, dir, names) = designate(token, 0, &holds, &Expansion::none())?;
    let name = names.only().ok_or(Refusal::FileNotNameable)?;
    Ok(FileGrant {
        which,
        directory: dir,
        name: Name::new(name).ok_or(Refusal::FileNotNameable)?,
        writable,
    })
}

/// **Whether this program can be wired the way the line asks**: the operators' manifest check.
///
/// **Output**: a `Words` or `Silent` program has no bytes for `>` or `|` to substitute. **Input**:
/// a program that reads a stream and is handed none would block forever, and one that reads nothing
/// and is handed one would hold authority for no reason. A `2>` at a program declaring no second
/// stream is [`Refusal::NoDiagnosticStream`] (DECISIONS §67 (a program's second stream is a
/// declaration)). Returns where the second stream goes: none, the shell's printing, or a file.
fn check_streams(m: Manifest, streams: Streams) -> Result<line::Diagnostics, Refusal> {
    if !matches!(streams.sink, Sink::Report) && !m.output.is_byte_stream() {
        return Err(Refusal::NotAByteStream);
    }
    // A declared second stream exists whether or not the line mentions it; `2>` only decides where
    // it goes. That is the whole shape of the decision: the separation is the program's declaration
    // and the operator is a destination for something already separate.
    let diagnostics = match (m.output.diagnostics_slot(), streams.diagnostics) {
        (None, None) => line::Diagnostics::None,
        (None, Some(_)) => return Err(Refusal::NoDiagnosticStream),
        (Some(_), None) => line::Diagnostics::Printed,
        (Some(_), Some((g, mode))) => line::Diagnostics::File(g, mode),
    };
    match (m.input, streams.source) {
        (InputDeclaration::Required { .. }, Source::None) => Err(Refusal::InputRequired),
        (InputDeclaration::Forbidden, Source::None) => Ok(diagnostics),
        (InputDeclaration::Forbidden, _) => Err(Refusal::InputForbidden),
        (InputDeclaration::Required { .. }, _) => Ok(diagnostics),
    }
}

/// **Whether this shell can be at both ends of the chain it just planned.** `shell_feeds_head` is
/// true when the first stage's input comes from this process: a `<`, an input operand, or a
/// producing builtin at the head (`ls | wc`); when false, none of this applies.
///
/// A process has one wait point, so a shell feeding a chain cannot also receive from it. The line
/// runs if a stage declaring [`InputDeclaration::Required::writes_while_reading`] `false` absorbs
/// the stream first, or if its unredirected tail goes to `terminal_sink_caretaker` (DECISIONS §106
/// (take the `terminal_sink_caretaker` narrowing)); a `>` tail comes back here (DECISIONS §55 (the
/// file behind a `>` is the shell itself)) and is refused. See notes/pipes/one-wait-point.md.
pub fn check_chain(shell_feeds_head: bool, plans: &[Option<Endowment>]) -> Result<(), Refusal> {
    if !shell_feeds_head {
        return Ok(());
    }
    let mut tail: Option<Endowment> = None;
    for e in plans.iter().flatten() {
        if !e.writes_while_reading {
            return Ok(());
        }
        tail = Some(*e);
    }
    match tail {
        // Nothing was spawned: the shell is the sole producer (a head builtin backed by a file) and
        // there is no second process to deadlock against.
        None => Ok(()),
        // The tail's output goes to the terminal's sink adapter, not back to this shell. This
        // trusts that `writes_while_reading` rides only on a byte-stream output rather than
        // re-checking `e.program.manifest()`, because `e` may have been planned against a different
        // manifest ([`plan_against_with`]).
        Some(e) if matches!(e.sink, Sink::Report) => Ok(()),
        Some(_) => Err(Refusal::NoReaderButThisShell),
    }
}

/// **What one operand token designates**: the directory its leading path landed in, and the set of
/// names in that directory.
///
/// This is where "resolve at grant time" and "the expansion is the grant" meet, and both halves are
/// values rather than pointers. The leading path is walked here, once, against where the shell is
/// standing *now*; the final component is either the name typed or the set the shell expanded it
/// into. Nothing downstream re-resolves anything, so a `cd` after this line cannot change what an
/// already-planned grant means, and neither can a file appearing in the directory afterwards.
fn designate(
    token: &[u8],
    at: usize,
    holds: &Holdings,
    expanded: &Expansion,
) -> Result<(nav::Which, nav::Cwd, NameSet), Refusal> {
    // Refuses a pattern anywhere but the last component before anything else looks at the token,
    // because `a*/b` is not a smaller question than `a*`, it is a different one (notes/glob.md).
    let magic = expand::magic_component(token)?;
    let parsed = nav::path(token).map_err(nav_refusal)?;
    let (lead, last) = parsed
        .split_last_component()
        .ok_or(Refusal::FileNotNameable)?;
    // The lead resolves exactly as `cd` resolves it ([`Holdings::anchor`]), and
    // [`Holdings::locate`] says which tree serves it. The anchor and the apply are spelled out
    // rather than through `resolve_steps` because this runs on the shell's planning path on a few
    // pages of stack, and one frame fewer is the margin `glob_grant_tests` records.
    let mut place = nav::Cwd::root();
    let rest = holds.anchor(parsed.is_from_root(), lead, &mut place);
    place.apply(rest).map_err(nav_refusal)?;
    let (which, dir) = holds.locate(&place);

    let names = if magic {
        // The pattern's whole meaning is the set, so a planner with no set has nothing to grant and
        // must not fall back on the pattern's own text.
        let set = expanded.for_positional(at).ok_or(Refusal::Unexpanded)?;
        if set.is_empty() {
            return Err(Refusal::NoMatch);
        }
        set
    } else {
        // A name that was typed always designates itself. The expansion is not consulted, so a
        // caller cannot substitute a set for a literal and have the grant differ from the line.
        //
        // Its type bit reads as "not a directory" because nothing enumerated it; see the BUGS note
        // in notes/glob-grant.md. Only a set the shell expanded carries types it observed.
        NameSet::one(last, false).ok_or(Refusal::FileNotNameable)?
    };
    Ok((which, dir, names))
}

/// A navigation refusal, in the vocabulary the prompt prints for a *grant*.
///
/// Every navigation refusal now reads as a fact about the token's shape, which it did not before
/// 2026-08-18: `nav::Refused::Absolute` had its own line here because "there is no namespace to
/// root a path in" was a statement about the model rather than about the token. A leading `/` roots
/// in the holder's own namespace now (`nav::Path::is_from_root`), so the case is gone rather than
/// re-worded.
pub(crate) fn nav_refusal(_r: nav::Refused) -> Refusal {
    Refusal::FileNotNameable
}

/// Which refusal to print for a token no declared slot can hold.
///
/// The manifest decides that the token is unplaceable; the token's *shape* only decides which true
/// sentence reads closest to what the user meant. An integer at a program that takes no integer is
/// "takes no argument"; a name at a program that takes no file is "takes no file"; anything else is
/// the generic unexpected-argument line.
fn unplaceable(tok: &[u8], m: Manifest) -> Refusal {
    let numeric = parse_u64(tok).is_some();
    if numeric && matches!(m.program_argument, ProgramArgumentDeclaration::Forbidden) {
        Refusal::ProgramArgumentForbidden
    } else if !numeric
        && matches!(m.file, FileGrantDeclaration::Forbidden)
        && matches!(m.directory, DirectoryGrantDeclaration::Forbidden)
    {
        // Only when the program can be handed **neither** a file nor a directory does "takes no
        // file" read true. A second name at `rm` is a name it has nowhere to put, which is the
        // generic line: one operand is what a manifest granting one directory can express.
        Refusal::FileForbidden
    } else {
        Refusal::Unexpected
    }
}

#[cfg(test)]
mod tests;
