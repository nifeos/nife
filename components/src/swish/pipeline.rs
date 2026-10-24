//! **The operators `>`, `<` and `|`** (milestone 50 (pipes and redirection),
//! notes/pipes.md), and the file behind a
//! `>` and a `<`, which this shell serves itself. One of the modules §273 (the shell program
//! becomes a directory of modules) lays out. See notes/pipes/the-shell-side.md.
//!
//! Name: provisional 2026-10-11 (UTC), milestone 844's lane. calef deferred every module name
//! in §273 to a name-ratification pass.

use super::*;

// ---- the operators: `>`, `<` and `|` (milestone 50, notes/pipes.md) ----

/// Pages the untyped region behind one pipeline holds. The endpoints themselves are page-resident
/// objects retyped out of it (one page each), and the region exists so the shell can **destroy** it
/// when the pipeline is over: only that frees the rendezvous, which turns a blocked producer's
/// `SEND` into `abi::Error::Gone` (notes/pipes.md, "SIGPIPE").
const PIPE_REGION_PAGES: u64 = 8;

/// **Run a line that has operators on it.**
///
/// Designates the redirection targets and plans every stage before anything moves, so a line that
/// cannot run spawns nothing. Refusals are said, not returned. See notes/pipes/the-shell-side.md.
pub(super) fn pipeline(nav: &mut Nav, l: Line<'_>) {
    // The names on the ends, resolved against where we stand now, exactly as an operand is.
    let out = match l.output {
        Some(t) => match grant_plan::redirect_target(t, holdings(nav), true) {
            Ok(g) => Some(g),
            Err(r) => return say(Say::Cannot(r)),
        },
        None => None,
    };
    let inp = match l.input {
        Some(t) => match grant_plan::redirect_target(t, holdings(nav), false) {
            Ok(g) => Some(g),
            Err(r) => return say(Say::Cannot(r)),
        },
        None => None,
    };
    // `2>` names one destination for the line: one diagnostic rendezvous serves every declarer.
    let diag_file = match l.diagnostics {
        Some(t) => match grant_plan::redirect_target(t, holdings(nav), true) {
            Ok(g) => Some((g, l.diagnostics_mode())),
            Err(r) => return say(Say::Cannot(r)),
        },
        None => None,
    };

    // On the heap, not the stack (milestone 47, calef's allocator ruling): this array is what
    // the four stack overflows in notes/pipes/the-boot.md had in common.
    let mut plans = match heap::filled::<Option<Endowment>>(l.stage_count(), None) {
        Ok(v) => v,
        Err(s) => return say(s),
    };
    let mut head_builtin = false;
    for (i, stage) in l.stages().iter().enumerate() {
        match grant_plan::parse(stage) {
            Command::Run(spec) => {
                // A name the image and an installed package both have is refused wherever it
                // stands (§229 (how a bare name at the prompt reaches an installed program), B2).
                if let swish::bare::Bare::Both(path) = bare(nav, spec.program) {
                    refused();
                    return swish::bare::write_both(spec.program, &path, &mut print);
                }
                let expanded = match expansion(nav, &spec) {
                    Ok(e) => e,
                    Err(Say::Cannot(r)) => return refuse(spec, r),
                    Err(said) => return say(said),
                };
                let streams = Streams {
                    sink: l.sink_for(i, out),
                    source: l.source_for(i, inp),
                    diagnostics: diag_file,
                };
                match grant_plan::plan_stage(&spec, holdings(nav), expanded, streams) {
                    Ok(e) => plans[i] = Some(e),
                    Err(r) => return refuse(spec, r),
                }
            }
            // A producing builtin, only at the head and only with nothing feeding it: none reads.
            Command::Echo(_) | Command::Ls(_) | Command::Pwd
                if i == 0 && matches!(l.source_for(0, inp), Source::None) =>
            {
                // No manifest, so no second stream for a `2>` to name.
                if diag_file.is_some() {
                    print(b"  ");
                    print(Refusal::NoDiagnosticStream.message().as_bytes());
                    print(b"\n");
                    return;
                }
                head_builtin = true;
            }
            _ => {
                print(b"  ");
                print(Refusal::NotAPipelineStage.message().as_bytes());
                print(b"\n");
                return;
            }
        }
    }

    // The head's input file comes off the plan, not the line: only the planner knows an input
    // operand. `plans[0]` is `None` only for a producing builtin, which has no operand.
    let inp = match plans[0] {
        Some(e) => match e.source {
            Source::File(g) => Some(g),
            _ => None,
        },
        None => inp,
    };

    run_pipeline(nav, l, &plans, head_builtin, out, inp, diag_file);
}

/// Mint the pipes, open the files, spawn the stages, feed the head if the shell is the producer, and
/// deliver what comes out of the tail. Everything above this decided; this moves capabilities.
///
/// The shell itself backs a `>` and a `<`, because `filesystem_contract`'s one shared page cannot
/// serve two clients (§55 (the file behind a `>` is the shell itself)); a redirected child still
/// holds only a rendezvous with `WRITE`. See notes/pipes.md and notes/pipes/the-shell-side.md.
#[allow(clippy::too_many_arguments)] // one parameter per operator, and there are four of them now
pub(super) fn run_pipeline(
    nav: &mut Nav,
    l: Line<'_>,
    plans: &[Option<Endowment>],
    head_builtin: bool,
    out: Option<grant_plan::FileGrant>,
    inp: Option<grant_plan::FileGrant>,
    diag_file: Option<(grant_plan::FileGrant, line::Mode)>,
) {
    let n = l.stage_count();

    // This shell can be at one end of a chain, never both. Checked before the files are opened:
    // a `>` truncates, and a line that will not run must not have emptied a file.
    if let Err(r) = grant_plan::check_chain(inp.is_some() || head_builtin, &plans[..n]) {
        say_chain_refused(&plans[..n], r);
        return;
    }

    // The files before any process exists, so a line whose file cannot open never half-runs.
    let Some((mut sink, mut source, mut diag_sink)) = open_line_files(nav, l, out, inp, diag_file)
    else {
        return;
    };
    // What [`drain_diagnostics`] counts end-of-stream messages against: only the stages whose
    // second stream this shell backs. A `Diagnostics::Printed` stage's bytes never reach it.
    let declarers = plans
        .iter()
        .take(n)
        .filter(|p| p.is_some_and(|e| matches!(e.diagnostics, line::Diagnostics::File(..))))
        .count();

    // §106 (the `terminal_sink_caretaker` narrowing), before anything spawns. `sink` is the `>`
    // file, not `Endowment::sink`. This condition must equal the one `check_chain` allowed the
    // line on, and a failed mint refuses rather than spawning unnarrowed, which would deadlock.
    let tail_writes_while_reading = plans[n - 1].is_some_and(|e| e.writes_while_reading);
    let needs_screen = sink.is_none() && tail_writes_while_reading;
    let Ok(screen_ep) = screen_for_tail(needs_screen) else {
        return;
    };

    // A builtin with a file on its output spawns nothing: both ends are this shell's.
    if head_builtin && n == 1 {
        if let Some(f) = &mut sink {
            feed(nav, l.stages()[0], f);
            f.report();
        }
        return;
    }

    let Some((region, pipes, minted)) = mint_pipes(n, source.is_some()) else {
        return;
    };
    let feed_pipe = source.is_some().then(|| pipes[n - 1]);

    // Every argv is written before any stage spawns (§170 (how a foreign program is told what to
    // do)), so running out of budget partway spawns nothing. The regions are kept until the line
    // is drained: the progenitor copies each frame while serving that stage's request.
    let mut argvs = [None::<Argv>; line::MAX_STAGES];
    // **And every stage's words grant, the same way** (milestone 205's designation half): a stage
    // that hears words is granted what they designated, on window `i`.
    let mut sets = [None::<WordsGrant>; line::MAX_STAGES];
    if let Err((prog, why)) = write_stage_words(l, &plans[..n], head_builtin, &mut argvs, &mut sets)
    {
        give_back_words(&argvs, &sets, true);
        release_pipeline(region, &pipes[..minted]);
        say_stage_words_failed(prog, why);
        return;
    }

    // Left to right, which is the order that cannot deadlock: a producer blocks in its first `SEND`
    // until its reader exists, and its reader is the next thing this loop spawns.
    let ends = StageEnds {
        pipes: &pipes,
        feed_pipe,
        screen_ep,
    };
    if !spawn_stages(&plans[..n], head_builtin, &ends, &argvs, &sets) {
        give_back_words(&argvs, &sets, false);
        release_pipeline(region, &pipes[..minted]);
        return;
    }

    // Diagnostics first: in `date | wc`, draining the output first waits on `wc`, which waits on
    // `date`, which is blocked sending on the diagnostic rendezvous.
    if let Some(f) = &mut diag_sink {
        finish_diagnostics(f, declarers);
    }

    // The shell as the producer, in either of its two shapes. Both run after every reader exists,
    // for the same reason the loop above goes left to right.
    if head_builtin {
        let mut w = SinkWriter::new(pipes[0]);
        feed(nav, l.stages()[0], &mut w);
    }
    if let (Some(f), Some(p)) = (&mut source, feed_pipe) {
        f.stream_into(p);
    }

    // A redirected tail still answers on the result rendezvous. A screen-narrowed tail's bytes
    // went to `terminal_sink_caretaker` (§106), so this waits for its exit instead.
    match &mut sink {
        Some(f) => {
            drain_into(f);
            f.report();
        }
        None => match screen_ep {
            Some(ep) => await_screen(ep),
            None => drain_text(),
        },
    }
    give_back_words(&argvs, &sets, false);
    release_pipeline(region, &pipes[..minted]);
}

/// The screen rendezvous for a screen-narrowed tail, minted only when `needs_screen`. `Err` when
/// it could not be minted, which it has already said.
fn screen_for_tail(needs_screen: bool) -> Result<Option<u64>, ()> {
    if needs_screen {
        match screen_rendezvous() {
            Some(ep) => Ok(Some(ep)),
            None => {
                failed();
                print(b"  this shell's memory budget is exhausted; nothing left to grant\n");
                Err(())
            }
        }
    } else {
        Ok(None)
    }
}

/// Say why `check_chain` refused the line, naming the first stage of `plans` that answers while it
/// reads.
fn say_chain_refused(plans: &[Option<Endowment>], r: Refusal) {
    refused();
    print(b"  ");
    // Named after the first stage that answers while it reads, which is the one a person would
    // reach for `| wc` about. The sentence itself is about the *line* and says so.
    if let Some(e) = plans.iter().flatten().find(|e| e.writes_while_reading) {
        print(e.program.name().as_bytes());
        print(b": ");
    }
    print(r.message().as_bytes());
    print(b"\n");
}

/// Drain the `2>` file from the `declarers` stages that write to it, or close it when none do.
fn finish_diagnostics(f: &mut FileOut, declarers: usize) {
    if declarers > 0 {
        drain_diagnostics(f, declarers);
        f.report();
    } else {
        // Nothing writes to it; close the handle on the name the line created.
        f.finish();
    }
}

/// [`run_pipeline`]'s `>`, `<` and `2>` files, opened in that order. On a failure the output file
/// (if it was opened) is finished, the reason is said, and the answer is `None`.
fn open_line_files(
    nav: &mut Nav,
    l: Line<'_>,
    out: Option<grant_plan::FileGrant>,
    inp: Option<grant_plan::FileGrant>,
    diag_file: Option<(grant_plan::FileGrant, line::Mode)>,
) -> Option<(Option<FileOut>, Option<FileIn>, Option<FileOut>)> {
    let mut sink = match out {
        Some(g) => match open_sink(nav, g, l.mode()) {
            Ok(f) => Some(f),
            Err(s) => {
                say(s);
                return None;
            }
        },
        None => None,
    };
    let source = match inp {
        Some(g) => match open_source(nav, g) {
            Ok(f) => Some(f),
            Err(s) => {
                if let Some(f) = &mut sink {
                    f.finish();
                }
                say(s);
                return None;
            }
        },
        None => None,
    };

    // The second stream's file (§67 (a program's second stream is a declaration)), on the
    // output's terms; a line may name both.
    let diag_sink = match diag_file {
        Some((g, mode)) => match open_sink(nav, g, mode) {
            Ok(f) => Some(f),
            Err(s) => {
                if let Some(f) = &mut sink {
                    f.finish();
                }
                say(s);
                return None;
            }
        },
        None => None,
    };
    Some((sink, source, diag_sink))
}

/// Split the pipeline's region and retype its pipe rendezvous out of it, one per joint and one more
/// when the shell feeds the head. On a failure it says so, gives back what it took, and answers `None`.
fn mint_pipes(n: usize, feeds_head: bool) -> Option<(u64, [u64; line::MAX_STAGES], usize)> {
    // One region for the whole pipeline, so a line costs one SPLIT and one DESTROY however many
    // stages it has. Each joint's rendezvous is a page retyped out of it.
    let Some(region) = memory_region_split(PIPE_REGION_PAGES) else {
        failed();
        failed();
        print(b"  this shell's memory budget is exhausted; nothing left to grant\n");
        return None;
    };
    let mut pipes = [0u64; line::MAX_STAGES];
    let mut minted = 0usize;
    // A `<` needs one more: the head reads a pipe like any stage, and the shell writes into it.
    let mut want = n - 1;
    if feeds_head {
        want += 1;
    }
    while minted < want {
        match retype_rendezvous(region) {
            Some(ep) => {
                pipes[minted] = ep;
                minted += 1;
            }
            None => {
                failed();
                print(b"  could not mint a pipe from this shell's budget\n");
                release_pipeline(region, &pipes[..minted]);
                return None;
            }
        }
    }
    Some((region, pipes, minted))
}

/// Give back every stage's argv and words grant.
///
/// Carved argv then set, stage by stage, so they go back in exactly the reverse order and each is
/// the top of this shell's budget when it goes (a region returns its pages only then).
fn give_back_words(argvs: &[Option<Argv>], sets: &[Option<WordsGrant>], unsent: bool) {
    for i in (0..argvs.len()).rev() {
        if let Some(w) = &sets[i] {
            release_words_grant(w, unsent);
        }
        if let Some(a) = argvs[i] {
            release_argv(a, unsent);
        }
    }
}

/// Write the argv and the words grant of every stage in `plans` that hears words. A failure
/// answers the stage's program and its sentence (`None` for an exhausted budget) and gives nothing back.
fn write_stage_words(
    l: Line<'_>,
    plans: &[Option<Endowment>],
    head_builtin: bool,
    argvs: &mut [Option<Argv>],
    sets: &mut [Option<WordsGrant>],
) -> Result<(), (grant_plan::Program, Option<&'static [u8]>)> {
    for i in 0..plans.len() {
        let Some(e) = plans[i].filter(|_| !(i == 0 && head_builtin)) else {
            continue;
        };
        let grant_plan::ProgramArgumentDeclaration::Words(grant) =
            e.program.manifest().program_argument
        else {
            continue;
        };
        let assembled = assemble_argv(l.stages()[i], i)
            .map_err(|why| why.map(|r| r.message().as_bytes()))
            .and_then(|a| {
                argvs[i] = Some(a);
                match e.directory {
                    Some(g) => words_grant(&g, grant, i).map(|w| {
                        sets[i] = Some(w);
                    }),
                    None => Ok(()),
                }
            });
        if let Err(why) = assembled {
            return Err((e.program, why));
        }
    }
    Ok(())
}

/// Say why [`write_stage_words`] could not write `prog`'s words: its sentence, or that the
/// budget ran out.
fn say_stage_words_failed(prog: grant_plan::Program, why: Option<&[u8]>) {
    match why {
        Some(sentence) => {
            refused();
            print(b"  ");
            print(prog.name().as_bytes());
            print(b": ");
            print(sentence.trim_ascii_start());
            if sentence.last() != Some(&b'\n') {
                print(b"\n");
            }
        }
        None => out_of_budget(),
    }
}

/// The pipe ends [`spawn_stages`] hands out: the joints, the head's feed and the tail's screen.
struct StageEnds<'a> {
    pipes: &'a [u64],
    feed_pipe: Option<u64>,
    screen_ep: Option<u64>,
}

/// Spawn every stage of `plans` left to right. On a failure it deletes the frames of the stages
/// after the one that failed and answers `false`; the caller gives back the rest.
fn spawn_stages(
    plans: &[Option<Endowment>],
    head_builtin: bool,
    ends: &StageEnds<'_>,
    argvs: &[Option<Argv>],
    sets: &[Option<WordsGrant>],
) -> bool {
    let n = plans.len();
    for i in 0..n {
        if i == 0 && head_builtin {
            continue;
        }
        let Some(e) = plans[i] else { continue };
        let stage_sink = if i + 1 < n { Some(ends.pipes[i]) } else { None };
        let stage_source = if i > 0 {
            Some(ends.pipes[i - 1])
        } else {
            ends.feed_pipe
        };
        let stage_diag = match e.diagnostics {
            line::Diagnostics::File(..) => diag_rendezvous(),
            _ => None,
        };
        // Only the tail is ever screen-narrowed.
        let stage_screen = ends.screen_ep.filter(|_| i + 1 == n);
        if !spawn_stage(
            e,
            stage_sink,
            stage_source,
            stage_diag,
            stage_screen,
            argvs[i],
            sets[i],
        ) {
            // Stages after this one never sent their frames, so their slots go too.
            for a in argvs[i + 1..].iter().flatten() {
                cap_delete(a.frame);
            }
            for w in sets[i + 1..].iter().flatten() {
                if let Some(p) = w.set {
                    cap_delete(p.frame);
                }
            }
            return false;
        }
    }
    true
}

/// **Somewhere this shell puts bytes it produced itself.**
///
/// Two implementations: a rendezvous (the next stage) and a file. A builtin cannot tell them apart,
/// as a spawned program cannot tell what is in its output slot.
pub(super) trait ByteOut {
    fn push(&mut self, bytes: &[u8]);
    /// Say the stream is over. On a pipe that is `OPERATION_EOF`; on a file it is the last write.
    fn finish(&mut self);
}

/// **Run a producing builtin with the given destination as its output**, one message at a time.
///
/// The builtins write through a callback, so the same `echo` and `ls` feed a pipe and a file.
fn feed(nav: &mut Nav, stage: &[u8], w: &mut dyn ByteOut) {
    let said = match grant_plan::parse(stage) {
        Command::Echo(text) => {
            let said = echo(nav, text, &mut |b| w.push(b));
            if matches!(said, Say::Nothing) {
                w.push(b"\n");
            }
            said
        }
        Command::Ls(path) => nav.ls(path, &mut |name, is_dir| {
            w.push(name);
            if is_dir {
                w.push(b"/");
            }
            w.push(b"\n");
        }),
        Command::Pwd => {
            if nav.dir.is_none() {
                Say::NoDirectory
            } else {
                let mut buf = [0u8; nav::RENDER_MAX];
                let k = nav.holds.cwd.render(&mut buf);
                w.push(&buf[..k]);
                w.push(b"\n");
                Say::Nothing
            }
        }
        _ => Say::Nothing,
    };
    // End of stream whatever happened, a refusal included: the reader downstream is blocked on a
    // receive, and leaving it there hangs the prompt.
    w.finish();
    if !matches!(said, Say::Nothing) {
        say(said);
    }
}

/// A byte-at-a-time writer onto a sink rendezvous, buffering into the contract's sixteen-byte
/// messages. The seam between a callback that hands over arbitrary slices and a wire that carries
/// two words at a time; `fixtures/src/file_source.rs` is the same loop from the other side.
struct SinkWriter {
    slot: u64,
    buf: [u8; byte_sink_contract::INLINE_MAX],
    n: usize,
    /// Set once a send has failed. Everything after is dropped rather than retried: `Gone` means the
    /// reader stopped caring, and there is nothing to report it to.
    stopped: bool,
}

impl SinkWriter {
    fn new(slot: u64) -> Self {
        SinkWriter {
            slot,
            buf: [0; byte_sink_contract::INLINE_MAX],
            n: 0,
            stopped: false,
        }
    }

    fn flush(&mut self) {
        if self.n == 0 || self.stopped {
            return;
        }
        let (w0, w1, w2, _) = byte_sink_contract::pack(&self.buf[..self.n]);
        self.n = 0;
        if !matches!(
            byte_sink_contract::classify(send(self.slot, w0, w1, w2)),
            byte_sink_contract::Sent::Ok
        ) {
            self.stopped = true;
        }
    }
}

impl ByteOut for SinkWriter {
    fn push(&mut self, bytes: &[u8]) {
        for &b in bytes {
            if self.stopped {
                return;
            }
            self.buf[self.n] = b;
            self.n += 1;
            if self.n == self.buf.len() {
                self.flush();
            }
        }
    }

    fn finish(&mut self) {
        self.flush();
        if !self.stopped {
            send(self.slot, byte_sink_contract::eof(), 0, 0);
        }
    }
}

// ---- the file behind a `>` and a `<`, which this shell serves itself (milestone 50) ----

/// How many bytes of a file this shell stages in the page it shares with the FS server before it
/// makes a request. Small because the shell runs on four stack pages; see
/// notes/pipes/the-shell-side.md.
const FILE_CHUNK: usize = 256;

/// **Walk to the directory a resolved grant records**, from the root of what this shell holds.
///
/// The walk starts at the root of tree `t` (milestone 154 (a process that holds two directory
/// capabilities)) and follows the position `at` that `grant_plan::designate` resolved at plan time,
/// never the current directory, so a `cd` cannot change what a redirection means.
///
/// Returns the handle plus the capabilities opened to reach it, which the caller closes.
fn open_at(
    nav: &Nav,
    t: Tree,
    at: Cwd,
    tmp: &mut [u64; nav::MAX_DEPTH],
) -> Result<(u64, usize), Say> {
    let mut handle = fs::ROOT;
    let mut n = 0usize;
    for level in 0..at.depth() {
        let r = nav.name_call_in(t, fs::OPENDIR, handle, at.component(level), t.rights);
        if r < 0 {
            for h in tmp.iter().take(n) {
                nav.close_in(t, *h);
            }
            return Err(Say::Failed(-r as i32));
        }
        tmp[n] = r as u64;
        n += 1;
        handle = r as u64;
    }
    Ok((handle, n))
}

/// **Open the file behind a `>` or a `>>`**, creating it if it is not there.
///
/// `CREATE` is create, not create-or-open (§27 (the filesystem service)), so on any refusal this
/// opens the existing name, as `fixtures/src/file_sink.rs` does. `mode` is decided here because the
/// shell backs the file (§55); the child is wired identically either way. [`line::Mode::Truncate`]
/// truncates before the command runs; [`line::Mode::Append`] starts the offset at `FSTAT`'s size.
/// See notes/pipes/the-file-end.md.
fn open_sink(nav: &mut Nav, g: grant_plan::FileGrant, mode: line::Mode) -> Result<FileOut, Say> {
    let t = nav.tree(g.which);
    let mut tmp = [0u64; nav::MAX_DEPTH];
    let (dir_handle, opened) = open_at(nav, t, g.directory, &mut tmp)?;
    let name = g.name.as_bytes();
    let dir = t.slot;
    let mut handle = nav.name_call_in(t, fs::CREATE, dir_handle, name, 0);
    // A file this shell just created is empty, so neither operator has anything to do to it. Only
    // the name that was already there needs a decision.
    let mut off = 0u64;
    if handle < 0 {
        handle = nav.name_call_in(t, fs::OPEN, dir_handle, name, 0);
        if handle >= 0 {
            let r = match mode {
                line::Mode::Truncate => {
                    call(dir, fs::req(fs::TRUNCATE, handle as u64, 0), 0).0 as i64
                }
                line::Mode::Append => {
                    let size = call(dir, fs::req(fs::FSTAT, handle as u64, 0), 0).0 as i64;
                    if size >= 0 {
                        off = size as u64;
                    }
                    size
                }
            };
            if r < 0 {
                nav.close_in(t, handle as u64);
                handle = r;
            }
        }
    }
    for h in tmp.iter().take(opened) {
        nav.close_in(t, *h);
    }
    if handle < 0 {
        return Err(Say::Failed(-handle as i32));
    }
    Ok(FileOut {
        dir,
        handle: handle as u64,
        off,
        buf: [0; FILE_CHUNK],
        n: 0,
        failed: 0,
        short: false,
    })
}

/// **Open the file behind a `<`.** No create and no truncate: reading a name that is not there is a
/// refusal, not an empty stream, because a `wc < typo.txt` that reported zero would be a lie a
/// person would believe.
pub(super) fn open_source(nav: &mut Nav, g: grant_plan::FileGrant) -> Result<FileIn, Say> {
    let t = nav.tree(g.which);
    let mut tmp = [0u64; nav::MAX_DEPTH];
    let (dir_handle, opened) = open_at(nav, t, g.directory, &mut tmp)?;
    let handle = nav.name_call_in(t, fs::OPEN, dir_handle, g.name.as_bytes(), 0);
    for h in tmp.iter().take(opened) {
        nav.close_in(t, *h);
    }
    if handle < 0 {
        return Err(Say::Failed(-handle as i32));
    }
    Ok(FileIn {
        dir: t.slot,
        handle: handle as u64,
    })
}

/// A file this shell is appending into, on behalf of a `>`.
///
/// It holds an open handle and a running offset, and buffers into [`FILE_CHUNK`] before each write
/// rather than paying a filesystem round trip per sixteen-byte sink message.
struct FileOut {
    /// The slot the directory capability lives in. A `u64` rather than a borrow of [`Nav`], so a
    /// builtin that borrows the shell's navigator can still write through this.
    dir: u64,
    handle: u64,
    off: u64,
    buf: [u8; FILE_CHUNK],
    n: usize,
    /// The errno of the first refused write, or 0, reported once at the end.
    failed: i32,
    /// A write the server answered with a smaller count. Not an errno: the request did not fail.
    short: bool,
}

impl FileOut {
    fn flush(&mut self) {
        if self.n == 0 || self.failed != 0 || self.short {
            self.n = 0;
            return;
        }
        put_page(&self.buf[..self.n]);
        let r = call(
            self.dir,
            fs::req(fs::WRITE, self.handle, self.n as u64),
            self.off,
        )
        .0 as i64;
        // A lost byte, either way, and a `>` that said nothing about it would be certifying a file
        // it did not write.
        if r < 0 {
            self.failed = -r as i32;
        } else if r != self.n as i64 {
            self.short = true;
        }
        self.off += self.n as u64;
        self.n = 0;
    }

    /// Say what happened, once, after the whole stream is through. Silence on success.
    fn report(&self) {
        if self.failed != 0 {
            say(Say::Failed(self.failed));
        } else if self.short {
            failed();
            print(b"  the filesystem took fewer bytes than were written; that file is short\n");
        }
    }
}

impl ByteOut for FileOut {
    fn push(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.buf[self.n] = b;
            self.n += 1;
            if self.n == self.buf.len() {
                self.flush();
            }
        }
    }

    fn finish(&mut self) {
        self.flush();
        call(self.dir, fs::req(fs::CLOSE, self.handle, 0), 0);
    }
}

/// A file this shell is reading out, on behalf of a `<`.
pub(super) struct FileIn {
    pub(super) dir: u64,
    pub(super) handle: u64,
}

impl FileIn {
    /// **Stream the file over the sink contract**, which is the whole of `<`: the head stage
    /// receives `OPERATION_BYTES` until `OPERATION_EOF` on a rendezvous, as from a `|`.
    fn stream_into(&mut self, pipe: u64) {
        let mut w = SinkWriter::new(pipe);
        let mut off = 0u64;
        let mut buf = [0u8; FILE_CHUNK];
        loop {
            let got = call(
                self.dir,
                fs::req(fs::READ, self.handle, FILE_CHUNK as u64),
                off,
            )
            .0 as i64;
            if got <= 0 {
                if got < 0 {
                    say(Say::Failed(-got as i32));
                }
                break;
            }
            let got = (got as usize).min(FILE_CHUNK);
            get_page(got, &mut buf);
            w.push(&buf[..got]);
            off += got as u64;
        }
        // End of stream whatever happened, for `feed`'s reason: the reader is a process blocked in a
        // receive, and a read error is not a reason to leave it there.
        w.finish();
        call(self.dir, fs::req(fs::CLOSE, self.handle, 0), 0);
    }
}

/// **Drain a byte stream off the result rendezvous into a file**, which is what `>` is.
///
/// Not a parameterised [`drain_text`]: that one prefixes two spaces, and the file gets only what
/// the program wrote.
fn drain_into(f: &mut FileOut) {
    for _ in 0..MAX_OUTPUT_CHUNKS {
        let (w0, w1, w2) = receive(RESULT);
        if w0 == spawnproto::SPAWN_FAILED || w0 == spawnproto::SPAWN_NOT_IN_IMAGE {
            failed();
            print(swish::spawn_failure_sentence(w0));
            return;
        }
        // The writer died mid-stream (milestone 235 (a command that faults hangs the prompt)):
        // the file keeps what arrived and is closed.
        if w0 == spawnproto::JOB_FAULTED {
            failed();
            f.finish();
            print(swish::FAULTED_SENTENCE);
            return;
        }
        let mut buf = [0u8; byte_sink_contract::INLINE_MAX];
        match byte_sink_contract::unpack(w0, w1, w2, &mut buf) {
            byte_sink_contract::Msg::Bytes(n) => f.push(&buf[..n]),
            byte_sink_contract::Msg::Eof => {
                f.finish();
                return;
            }
            byte_sink_contract::Msg::Malformed => {
                f.finish();
                print(b"  (that program sent something this shell cannot read as bytes)\n");
                return;
            }
        }
    }
    f.finish();
    print(b"  (output truncated: that program never said it was finished)\n");
}

/// **Direct the progenitor to build one stage**, delegating whatever the operators put in its slots.
///
/// The progenitor puts the delegated rendezvous where the result rendezvous would have gone, which
/// is why `>` and `|` are one mechanism. The wiring rules are in notes/pipes/the-shell-side.md.
///
/// Returns whether the stage started; a failure has already been printed.
fn spawn_stage(
    e: Endowment,
    sink: Option<u64>,
    source: Option<u64>,
    diagnostics: Option<u64>,
    screen: Option<u64>,
    argv: Option<Argv>,
    words: Option<WordsGrant>,
) -> bool {
    let mem_slot = if e.memory_grant_pages > 0 {
        match memory_region_split(e.memory_grant_pages) {
            Some(slot) => Some(slot),
            None => {
                // The argv frame was never sent, so its slot is still ours; its region is the
                // caller's to give back.
                if let Some(a) = argv {
                    cap_delete(a.frame);
                }
                if let Some(p) = words.and_then(|w| w.set) {
                    cap_delete(p.frame);
                }
                failed();
                print(b"  this shell's memory budget is exhausted; nothing left to grant\n");
                return false;
            }
        }
    } else {
        None
    };

    let wiring = spawnproto::Wiring {
        interruptible: false,
        sink: sink.is_some(),
        source: source.is_some(),
        diagnostics: diagnostics.is_some(),
        // Directory-granted only for words (milestone 205); `rm`'s required grant is met only in
        // `spawn`. See notes/dir-capability.md's BUGS.
        directory_grant: words.is_some(),
        second_directory_grant: false,
        screen: screen.is_some(),
        // A pipeline stage is always a program the image names (§219 D's first cut runs an image
        // only on a plain line; see `run_image`).
        image: false,
        run_unvouched: false,
        program_arguments: argv.is_some(),
        nameset: words.is_some_and(|w| w.set.is_some()),
        machine: machine_wiring(&e),
        // A stage never promises devices (milestone 632 (graphics on demand)):
        // `graphical_terminal`'s manifest refuses every operator that builds a stage.
        graphics: false,
        keyboard: false,
    };
    let (w0, w1, w2) = spawnproto::request(
        e.program.id(),
        e.program_argument,
        e.memory_grant_pages,
        wiring,
    );
    send(SPAWN, w0, w1, w2);
    // The words grant's two data messages first, as `spawn` sends a directory grant's.
    if let Some(DirWords { caretaker, child }) = words.map(|w| w.words) {
        send(SPAWN, caretaker.0, caretaker.1, caretaker.2);
        send(SPAWN, child.0, child.1, child.2);
    }
    // **The argv's frame, then the name set's, before every other capability**
    // (`spawnproto::PROGRAM_ARGUMENTS_BIT`, `NAMESET_BIT`), as `spawn` sends them.
    if let Some(a) = argv {
        user_mode_runtime::send_capability(SPAWN, a.frame, abi::rights::READ, 0);
        cap_delete(a.frame);
    }
    if let Some(w) = &words {
        send_set(w);
    }
    // In the protocol's order, which both sides read out of the same word. A `SEND_CAP` nobody
    // expects and a `RECEIVE_CAP` nobody answers both deadlock, so the order is the contract.
    if let Some(slot) = sink {
        delegate(slot, abi::rights::WRITE | abi::rights::GRANT);
    }
    if let Some(slot) = source {
        delegate(slot, abi::rights::READ | abi::rights::GRANT);
    }
    if let Some(slot) = diagnostics {
        delegate(slot, abi::rights::WRITE | abi::rights::GRANT);
    }
    if let Some(slot) = screen {
        // READ, not WRITE: this is the child's fault target, and this shell needs `RECEIVE` and
        // `REAP` on it. `supervision_protocol::build_child_space` inserts the child's copy at READ.
        delegate(slot, abi::rights::READ | abi::rights::GRANT);
    }
    if let Some(slot) = mem_slot {
        // Every right, for `running`'s reason (milestone 824 (a retype mints no right its budget
        // lacks)).
        delegate(slot, abi::rights::ALL);
        cap_delete(slot);
    }
    delegate_machine_page(wiring.machine);

    // A substituted or screen-narrowed stage owes no answer, so the progenitor acks; without it a
    // failed spawn is invisible. A fault word here is an earlier stage's (milestone 235).
    if wiring.sink || wiring.screen {
        match receive(RESULT).0 {
            w if w == spawnproto::SPAWN_OK => {}
            w if w == spawnproto::JOB_FAULTED => {
                failed();
                print(swish::FAULTED_SENTENCE);
                return false;
            }
            w => {
                failed();
                print(swish::spawn_failure_sentence(w));
                return false;
            }
        }
    }
    true
}

/// Give the pipeline's memory back.
///
/// The `DESTROY` is what turns a still-writing producer's next `SEND` into `abi::Error::Gone`
/// (the `yes | head` shape); see [`PIPE_REGION_PAGES`].
fn release_pipeline(region: u64, pipes: &[u64]) {
    for &p in pipes {
        cap_delete(p);
    }
    // Retried for `reclaim`'s reason: a child finishing its last instruction still counts as a live
    // thread, and DESTROY refuses while one is resident.
    reclaim(region);
    cap_delete(region);
}

/// RETYPE one page of `region` into an `Rendezvous` we hold with full rights, which is what lets us
/// delegate a narrowed view of it to each end of a pipe. Full because the shell's budget carries
/// every right, and since milestone 824 (a retype mints no right its budget lacks) a retype mints
/// its budget's rights.
pub(super) fn retype_rendezvous(region: u64) -> Option<u64> {
    let r = retype_into_kernel_object(region, abi::objtype::RENDEZVOUS);
    if r < 0 { None } else { Some(r as u64) }
}
