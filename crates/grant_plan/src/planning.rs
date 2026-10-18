//! **Whether a line can be granted**: `plan` and its variants, the stream checks,
//! `check_chain`, and the placing of a line's tokens into the slots a manifest declares.
//!
//! Moved out of the crate's root unchanged by milestone 842 (the grant-plan crate root is split
//! along its seams), as §272 (the grant-plan crate root splits into private modules) lays out; the
//! root re-exports every public item at its old path.
//!
//! Name: provisional (milestone 842, 2026-10-11), as are its sibling modules' names.

use crate::expand::{Expansion, Name, NameSet};
use crate::line::{Sink, Source};
use crate::{
    ArgSpec, CapabilityKind, DirGrant, DirSpec, Endowment, FileGrant, FileSpec, Holdings,
    InputSpec, Manifest, MemSpec, Prog, Refusal, RunSpec, check_words, expand, line, nav,
    parse_u64,
};

/// Check a parsed invocation against the program's manifest and produce the exact [`Endowment`] to
/// grant, or the [`Refusal`] to print. The whole authority decision lives here: after this returns
/// `Ok`, the shell grants precisely what the `Endowment` names and the child can reach nothing
/// else.
///
/// The program is resolved first, because "there is no such program" is a fact about the system and
/// everything else is a fact about a program that exists.
/// **`expanded` is why the shell expands before it plans.** A pattern designates the names it
/// matched, so the endowment *is* the set, and the planner must see the set rather than the pattern.
/// [`Expansion::none`] is the answer for a line with no pattern on it, which is every line the shell
/// could plan before milestone 47's globbing lane.
pub fn plan(run: &RunSpec<'_>, holds: Holdings, expanded: Expansion) -> Result<Endowment, Refusal> {
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
    run: &RunSpec<'_>,
    holds: Holdings,
    expanded: Expansion,
    streams: Streams,
) -> Result<Endowment, Refusal> {
    // Ahead of resolving the program, for the reason [`RunSpec::misquoted`] states: a line whose
    // words cannot be read has no reliable first word either.
    if let Some(r) = run.misquoted {
        return Err(r);
    }
    let prog = Prog::from_name(run.prog).ok_or(Refusal::NoSuchProgram)?;
    plan_against_with(run, prog, prog.manifest(), holds, expanded, streams)
}

/// [`plan`]'s second half, against an **explicit** manifest rather than the static table.
///
/// Split out for two reasons, and the first is immediate: it is the only way to exercise a manifest
/// shape no shipped program declares yet (a program endowed a file), so `FileSpec::Required` is live,
/// tested logic instead of a branch nothing reaches. The second is milestone 23, where a manifest
/// travels *with* a component rather than living in this table, and the composer checks a program it
/// did not write. That is the same call with a different source of the manifest.
///
/// **This is where the positional tokens acquire meaning**, and the order is the order they are
/// typed in: the integer argument first if the manifest declares one, then the file if it declares
/// one. A token past the last declared slot is unplaceable, and unplaceable is a refusal.
///
/// # What globbing changed here, and why it had to
///
/// The slots used to be filled by splitting a slice, which was enough while a token was a name. A
/// token can now be a **pattern**, and a pattern designates the set the shell expanded it into, so
/// the placement is by **index** and the [`Expansion`] is keyed to that index. The alternative was
/// to let the planner infer which slot the expansion belonged to, which would have been the parser
/// classifying tokens again, one layer down and less visibly.
///
/// Two guards fall out of that, and both exist so authority cannot move silently:
///
/// - a pattern with no expansion behind it is [`Refusal::Unexpanded`], never a grant of a name with
///   a `*` in it;
/// - a token with **no** magic never consults the expansion at all, so a name that was typed always
///   designates itself.
pub fn plan_against(
    run: &RunSpec<'_>,
    prog: Prog,
    m: Manifest,
    holds: Holdings,
    expanded: Expansion,
) -> Result<Endowment, Refusal> {
    plan_against_with(run, prog, m, holds, expanded, Streams::default())
}

/// [`plan_against`], with the operators' answer. The one that does the work; the other two are the
/// spellings without a pipeline on the line.
pub fn plan_against_with(
    run: &RunSpec<'_>,
    prog: Prog,
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
    let flags = plan_options(run, &m, words)?;

    let pos = run.positionals();
    let mut next = 0usize;

    // The argument takes the first positional, and it must be an integer: `least_authority_demo eight` is a
    // missing argument, not a file named "eight", because least_authority_demo's manifest declares no file.
    //
    // A pattern can never land here by accident: every magic byte (`*`, `?`, `[`, `\`) is a
    // non-digit, so `parse_u64` refuses a pattern before the expansion is ever consulted, and
    // `least_authority_demo *` is "needs an integer argument" rather than a grant of anything.
    let arg = plan_arg(run, &m, pos, &mut next)?;

    // The file grant takes the next positional. The name is the whole designation; the direction
    // comes from the manifest, which is why `wc report.txt` reads and `tee report.txt` writes
    // without the human typing a mode.
    let file = plan_file(&m, pos, &mut next, &holds, expanded)?;

    // The directory grant takes the next positional, and it is resolved exactly as a file grant is:
    // the leading path is walked here, once, against where the shell is standing now, and the
    // result is recorded as a value. The child is handed a capability to the directory the names
    // are **in**, plus the names, because taking a name away is an operation on the directory that
    // holds it and no per-file capability can express it. See [`DirSpec`].
    let dir = plan_dir(&m, flags, pos, &mut next, &holds, expanded)?;

    // **A program that hears words is granted what its words designate** (milestone 205, §170
    // (how a foreign program is told what to do) clauses 2 to 5): one directory grant at the
    // shell's current directory, filtered to the designated names, or the whole directory when a
    // word was `.`. A line that designated nothing grants nothing (calef's N1 ruling,
    // 2026-09-27T06:27Z). What the grant may do is the manifest's [`WordGrant`], which the shell
    // turns into rights and the progenitor clamps to the manifest it endows.
    let dir = words_dir_grant(&m, &holds, expanded, dir)?;

    // **The input operand: `wc report.txt` is `wc < report.txt` with the operator left out.**
    //
    // This is milestone 47's move applied to the other direction. The `file:` prefix came out on the
    // finding that the manifest was doing all the work, and it is doing all of it here too: a
    // program that declares `InputSpec::Required` reads a stream and nothing else, so a bare name in
    // the position after its declared grants can only be the thing feeding it. There is nothing for
    // the parser to classify and no ambiguity to resolve, because a manifest declaring an input
    // never also declares a file (a program that wanted both would need positional arity, which is
    // the same widening `ArgSpec` is waiting on; `no_program_declares_both_a_file_and_an_input`
    // holds every manifest to it since milestone 150).
    //
    // **That widening is `FileSpec` + `InputSpec`'s problem, not `ArgSpec` + `InputSpec`'s.** An
    // argument is numeric-shaped and `arg` above already claims a fixed earlier position, so a
    // manifest declaring both `ArgSpec::Required` and `InputSpec::Required` has nothing left to
    // disambiguate: `arg` takes position 0, `input`'s fallback takes whatever bare name is left,
    // exactly as `arg` and `file` already compose for [`STAMPS_A_FILE`] below. No shipped program
    // has declared the combination, and whether one should is an open question for an architect
    // (design/roadmap/proposals/a-program-that-takes-an-argument-and-an-input.md), but the
    // absence is unclaimed headroom rather than a refusal here; see
    // `an_argument_and_an_input_stream_compose_by_the_same_fixed_order` in this module's tests.
    //
    // **What the child ends up holding is narrower than a file capability**, and that is worth being
    // exact about rather than claiming the headline. The shell opens the name and streams it, so the
    // program is handed an endpoint carrying the bytes of that one file: it cannot seek, re-read,
    // stat, or name a second thing, and it cannot tell a file from a pipe from a builtin. Typing the
    // name is still what moves the authority (`wc` alone is refused at the prompt, before anything
    // is spawned), which is designation-is-authorization; what it is *not* is the per-file capability
    // `FileSpec::Required` describes, because `wc` deliberately does not speak the filesystem
    // contract at all (milestone 50: "a `wc` that could open the file it counts would be a `wc` that
    // could open any file").
    //
    // An explicit `<` wins, because it is the same designation said out loud: a line carrying both
    // has named its input twice, and the operand then has nowhere to go and is unplaceable below.
    let mut streams = streams;
    plan_input_operand(&m, &mut streams, pos, &mut next, &holds, expanded)?;

    // Anything left designates a slot this program does not have.
    if let Some(&extra) = pos.get(next) {
        return Err(unplaceable(extra, m));
    }

    // The memory grant. A flag, not a position, so it is checked last: it cannot be confused with
    // anything and it should not shadow a refusal about a resource.
    let mem_pages = plan_mem(run, &m)?;

    let diagnostics = check_streams(m, streams)?;

    Ok(Endowment {
        prog,
        arg,
        mem_pages,
        file,
        dir,
        flags,
        sink: streams.sink,
        source: streams.source,
        diagnostics,
        reports: m.reports,
        interruptible: m.interruptible,
        writes_while_reading: matches!(
            m.input,
            InputSpec::Required {
                writes_while_reading: true
            }
        ),
    })
}

/// [`plan_against_with`]'s options: each letter on the line as its bit in `m`'s flags, or
/// [`Refusal::NoSuchOption`]. A program that hears words has none.
fn plan_options(run: &RunSpec<'_>, m: &Manifest, words: bool) -> Result<u64, Refusal> {
    let mut flags = 0u64;
    for &letter in run.options().iter().filter(|_| !words) {
        let bit = m
            .flags
            .letters()
            .iter()
            .position(|&d| d == letter)
            .ok_or(Refusal::NoSuchOption)?;
        flags |= 1 << bit;
    }
    Ok(flags)
}

/// [`plan_against_with`]'s integer argument, taken from the positional at `next`.
fn plan_arg(
    run: &RunSpec<'_>,
    m: &Manifest,
    pos: &[&[u8]],
    next: &mut usize,
) -> Result<u64, Refusal> {
    let arg = match m.program_argument {
        ArgSpec::Required => {
            let first = pos.get(*next).ok_or(Refusal::ProgramArgumentRequired)?;
            let v = parse_u64(first).ok_or(Refusal::ProgramArgumentRequired)?;
            *next += 1;
            v
        }
        ArgSpec::Forbidden => 0,
        // Every positional is a word of the argv the shell assembles at spawn ([`argv`]), so none
        // is left to place and none is unplaceable.
        ArgSpec::Words(_) => {
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
    expanded: Expansion,
) -> Result<Option<FileGrant>, Refusal> {
    let file = match m.file {
        FileSpec::Forbidden => None,
        FileSpec::Required { writable } => {
            let token = *pos.get(*next).ok_or(Refusal::FileRequired)?;
            let at = *next;
            *next += 1;
            // What the shell holds decides whether the designation can be backed at all, and it
            // wins over the name's shape: "there is nothing I hold that could grant this" is a
            // bigger fact than "and that name is too long".
            if !holds.dir {
                return Err(Refusal::NoSuchCapability(CapabilityKind::File));
            }
            let (which, dir, names) = designate(token, at, holds, expanded)?;
            // One file, so a pattern that matched more than one has designated something this
            // program cannot be handed. The count is the designation; nothing was denied.
            let name = names.only().ok_or(Refusal::AmbiguousFile)?;
            Some(FileGrant {
                which,
                dir,
                name: Name::new(name).ok_or(Refusal::FileNotNameable)?,
                writable,
            })
        }
    };
    Ok(file)
}

/// [`plan_against_with`]'s directory grant, designated by the positional at `next`, widened to the
/// subtree when its recursion option is in `flags`.
fn plan_dir(
    m: &Manifest,
    flags: u64,
    pos: &[&[u8]],
    next: &mut usize,
    holds: &Holdings,
    expanded: Expansion,
) -> Result<Option<DirGrant>, Refusal> {
    let dir = match m.directory {
        DirSpec::Forbidden => None,
        DirSpec::Required { subtree_flag } => {
            let token = *pos.get(*next).ok_or(Refusal::PathRequired)?;
            let at = *next;
            *next += 1;
            if !holds.dir {
                return Err(Refusal::NoSuchCapability(CapabilityKind::File));
            }
            let (which, dir, names) = designate(token, at, holds, expanded)?;
            // Typing the recursion option is what widens the capability from "take names out of
            // this directory" to "walk what is under it". A program run without it holds no way to
            // descend, so its recursion is not disabled by a branch anybody has to get right.
            let subtree = match subtree_flag {
                Some(letter) => match m.flags.letters().iter().position(|&d| d == letter) {
                    Some(bit) => flags & (1 << bit) != 0,
                    // A manifest naming an option it does not declare is a bug in the manifest, and
                    // the safe reading is the narrow one: no widening.
                    None => false,
                },
                None => false,
            };
            Some(DirGrant {
                which,
                dir,
                names,
                subtree,
            })
        }
    };
    Ok(dir)
}

/// The directory grant a program that hears words is given for what they designated, or `dir`
/// unchanged for any other program.
fn words_dir_grant(
    m: &Manifest,
    holds: &Holdings,
    expanded: Expansion,
    dir: Option<DirGrant>,
) -> Result<Option<DirGrant>, Refusal> {
    let dir = match (m.program_argument, expanded.designation()) {
        (ArgSpec::Words(_), Some(d)) if !d.is_empty() => {
            if !holds.dir {
                return Err(Refusal::NoSuchCapability(CapabilityKind::File));
            }
            Some(DirGrant {
                which: nav::Which::A,
                dir: holds.cwd,
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
    expanded: Expansion,
) -> Result<(), Refusal> {
    if matches!(m.input, InputSpec::Required { .. })
        && matches!(streams.source, Source::None)
        && let Some(&token) = pos.get(*next)
    {
        let at = *next;
        *next += 1;
        // What the shell holds decides whether the designation can be backed at all, and it wins
        // over the name's shape, exactly as it does for a file grant.
        if !holds.dir {
            return Err(Refusal::NoSuchCapability(CapabilityKind::File));
        }
        let (which, dir, names) = designate(token, at, holds, expanded)?;
        let name = names.only().ok_or(Refusal::AmbiguousFile)?;
        streams.source = Source::File(FileGrant {
            which,
            dir,
            name: Name::new(name).ok_or(Refusal::FileNotNameable)?,
            // A reader reads. The direction is the manifest's here as everywhere else, and
            // `InputSpec` has only the one.
            writable: false,
        });
    }
    Ok(())
}

/// [`plan_against_with`]'s memory grant: `--mem` checked against what `m` declares.
fn plan_mem(run: &RunSpec<'_>, m: &Manifest) -> Result<u64, Refusal> {
    let mem_pages = match (m.memory_grant, run.mem) {
        (MemSpec::Forbidden, None) => 0,
        (MemSpec::Forbidden, Some(_)) => return Err(Refusal::MemoryGrantForbidden),
        (MemSpec::Required { .. }, None) => return Err(Refusal::MemoryGrantRequired),
        (MemSpec::Required { min, max }, Some(n)) => {
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
/// `writable` comes from the operator rather than from a manifest, and that is the one place this
/// shell reads a direction off the line. It is not the exception it looks like: the program is not
/// being granted a file at all. It is being handed a **sink**, which appends and can do nothing
/// else, and the direction is a property of which end of the pipe the shell is building.
pub fn redirect_target(
    token: &[u8],
    holds: Holdings,
    writable: bool,
) -> Result<FileGrant, Refusal> {
    if !holds.dir {
        return Err(Refusal::NoSuchCapability(CapabilityKind::File));
    }
    let (which, dir, names) = designate(token, 0, &holds, Expansion::none())?;
    let name = names.only().ok_or(Refusal::FileNotNameable)?;
    Ok(FileGrant {
        which,
        dir,
        name: Name::new(name).ok_or(Refusal::FileNotNameable)?,
        writable,
    })
}

/// **Whether this program can be wired the way the line asks**, which is the whole of the operators'
/// manifest check.
///
/// Two questions, and they are not symmetric because the two directions are not. **Output**: has
/// this program got bytes to redirect at all? A `Words` program answers with a number in a register
/// and a `Silent` one holds no output capability, so in both cases there is nothing for `>` or `|`
/// to substitute and the honest answer is that the line asked for something that does not exist.
/// **Input**: a program that reads a stream and is handed none blocks forever, and a program that
/// reads nothing and is handed one has been given authority for no reason. Both are refusals, and
/// the first one is the one Unix cannot make.
///
/// **Output is now two questions**, and the second is `2>`'s (DECISIONS §67): the manifest says
/// whether this program has a second stream at all, so a `2>` aimed at one that does not is
/// [`Refusal::NoDiagnosticStream`] at the prompt with nothing spawned. What comes back is where the
/// second stream goes, which is `None` for a program that declares none, the shell's own printing
/// for one that does, and a file when the line named one.
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
        (InputSpec::Required { .. }, Source::None) => Err(Refusal::InputRequired),
        (InputSpec::Forbidden, Source::None) => Ok(diagnostics),
        (InputSpec::Forbidden, _) => Err(Refusal::InputForbidden),
        (InputSpec::Required { .. }, _) => Ok(diagnostics),
    }
}

/// **Whether this shell can be at both ends of the chain it just planned**, which is the one
/// question the operators cannot answer stage by stage.
///
/// `shell_feeds_head` is true when the bytes going into the first stage come from *this process*: a
/// `<`, an input operand (`wc report.txt`), or a producing builtin at the head (`ls | wc`). When it
/// is false there is a separate process on the far end of the head's input and none of this applies.
///
/// # The constraint, which is the kernel's and not the shell's
///
/// A process has **one wait point**. `SEND` blocks until a receiver takes the message, `RECEIVE` blocks
/// until one arrives, and there is no select and no poll (a timer and a bound notification can end a
/// receive since milestones 106 and 151, but not a `SEND`). So a shell that is feeding a chain cannot also be
/// receiving from it, and the two blocked processes have nothing that could wake either.
///
/// **No interleaving schedule fixes this**, which is worth stating because it is the first thing
/// anybody reaches for. Alternating one send with one receive deadlocks whenever the stage reads
/// twice before it writes; alternating the other way deadlocks whenever it writes twice before it
/// reads. The shell cannot know which, because the whole point of the sink contract is that neither
/// end knows anything about the other.
///
/// # What makes a line runnable anyway: one barrier
///
/// A stage that declares [`InputSpec::Required::writes_while_reading`] `false` absorbs the whole
/// stream before it answers. One of those anywhere in the chain is enough: everything upstream of it
/// can stream freely, the shell's feed completes, and only then does anything travel back. So
/// `mdr page.md | wc` runs and `mdr page.md` is refused, with `| wc` being the fix the message names.
///
/// A line whose head is a builtin and which spawns nothing at all (`ls > out.txt`) is not a chain
/// and is always `Ok`: there is no second process to deadlock against.
///
/// # DECISIONS §106's exception
///
/// A chain with no absorbing barrier used to be refused outright, because the tail's answer always
/// came back to this shell over its own result endpoint and this shell was already busy feeding the
/// head. That is no longer universally true: an unredirected tail (`Sink::Report`) whose program's
/// output is a byte stream is delegated to `terminal_sink_caretaker` by default instead, so the
/// shell never becomes this chain's reader at all. A tail the line redirected with `>`
/// (`Sink::File`) still comes back through this shell (DECISIONS §55: the file behind a `>` is the
/// shell itself), so that shape is still refused exactly as before.
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
        // **The tail's own output goes to the terminal's sink adapter, not back to this shell.**
        //
        // This trusts that `writes_while_reading` never rides on anything but a byte-stream output,
        // rather than re-checking `is_byte_stream` here off `e.prog.manifest()`: `e` may have been
        // planned against an explicit manifest that differs from `e.prog`'s real one
        // ([`plan_against_with`]'s whole reason to exist), so re-deriving from `e.prog` would answer
        // a question about the wrong manifest. No shipped program declares
        // `writes_while_reading: true` next to anything but `OutputSpec::Bytes` today, and a
        // filter's whole shape (writing before it has read to the end) presupposes the streaming
        // sink contract; a manifest that paired the two would be declaring a filter with no stream
        // to filter into.
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
    expanded: Expansion,
) -> Result<(nav::Which, nav::Cwd, NameSet), Refusal> {
    // Refuses a pattern anywhere but the last component before anything else looks at the token,
    // because `a*/b` is not a smaller question than `a*`, it is a different one (notes/glob.md).
    let magic = expand::magic_component(token)?;
    let parsed = nav::path(token).map_err(nav_refusal)?;
    let (lead, last) = parsed
        .split_last_component()
        .ok_or(Refusal::FileNotNameable)?;
    // **The lead resolves exactly as `cd` would resolve it** ([`Holdings::anchor`]): a
    // relative one from where the shell stands, an absolute one from a bound name or the root.
    // [`Holdings::locate`] then says which tree serves it. Everything downstream re-walks the
    // recorded `(which, dir)` from that tree's own root (`swish::open_at`), so the position is all
    // a grant carries.
    //
    // Before milestone 154 this rooted every absolute token at the sole root literally, so
    // `wc /recent/x` with `recent` bound walked a directory named `recent` while `ls /recent`
    // walked the bind. One resolver for both is what closed that.
    //
    // The anchor and the apply are spelled out here rather than through `resolve_steps` because
    // this runs on the shell's planning path, on a debug build's few pages of stack, and one frame
    // fewer is the margin `glob_grant_tests` records.
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
    if numeric && matches!(m.program_argument, ArgSpec::Forbidden) {
        Refusal::ProgramArgumentForbidden
    } else if !numeric
        && matches!(m.file, FileSpec::Forbidden)
        && matches!(m.directory, DirSpec::Forbidden)
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
