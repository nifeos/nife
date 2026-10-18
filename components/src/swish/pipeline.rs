//! **The operators `>`, `<` and `|`** (milestone 50 (pipes and redirection),
//! notes/pipes.md), and the file behind a
//! `>` and a `<`, which this shell serves itself.
//!
//! Moved whole out of `components/src/swish.rs` by milestone 844 (the shell program is split
//! along its seams), as §273 (the shell program becomes a directory of modules) lays out.
//!
//! Name: provisional 2026-10-11 (UTC), milestone 844's lane. calef deferred every module name
//! in §273 to a name-ratification pass.

use super::*;

// ---- the operators: `>`, `<` and `|` (milestone 50, notes/pipes.md) ----

/// Pages the untyped region behind one pipeline holds. The endpoints themselves are page-resident
/// objects retyped out of it (one page each), and the region exists so the shell can **destroy** it
/// when the pipeline is over.
///
/// That is not tidiness. Deleting every capability naming an rendezvous does not destroy the rendezvous,
/// because the object lives in a page of this region; only reclaiming the region does. So a producer
/// still blocked in a `SEND` after its reader has gone would stay blocked forever, and `yes | head`
/// would hang instead of ending. Destroying the region is what turns that into `abi::Error::Gone`.
const PIPE_REGION_PAGES: u64 = 8;

/// **Run a line that has operators on it.**
///
/// The shape is: designate the redirection targets, plan every stage, and only then move anything.
/// Nothing is spawned until the whole line is known to be runnable, which matters more here than for
/// a single command: a pipeline that half-starts leaves a process blocked on a channel nobody will
/// ever write to, and this shell has no way to reach it.
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
    // `2>` names one destination for the **line**, because this shell mints one diagnostic rendezvous
    // and hands it to every stage that declares a second stream. See notes/pipes.md.
    let diag_file = match l.diagnostics {
        Some(t) => match grant_plan::redirect_target(t, holdings(nav), true) {
            Ok(g) => Some((g, l.diagnostics_mode())),
            Err(r) => return say(Say::Cannot(r)),
        },
        None => None,
    };

    // Plan every stage. `head` is the one shape a stage may have that is not a program: a builtin
    // that produces text, at the head of the pipeline, whose bytes **the shell writes into the pipe
    // itself**. That costs no process and no new mechanism, and it is what makes `ls | wc` a thing a
    // person can type in a shell that holds a directory.
    // **On the heap, not on the stack** (milestone 47, calef's allocator ruling). This array of
    // endowments, each carrying a whole name set, is what the four stack overflows in
    // notes/pipes/the-boot.md had in common, and this function's frame was the deepest in the
    // program. Sized to the line's own stages rather than to `MAX_STAGES`.
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
                if let swish::bare::Bare::Both(path) = bare(nav, spec.prog) {
                    refused();
                    return swish::bare::write_both(spec.prog, &path, &mut print);
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
            // `echo`, `ls` and `pwd` render text and grant nothing, so the shell can be their sink's
            // client. Only at the head, and only when nothing is feeding them: a builtin with an
            // input is a builtin that reads, and none of them do.
            Command::Echo(_) | Command::Ls(_) | Command::Pwd
                if i == 0 && matches!(l.source_for(0, inp), Source::None) =>
            {
                // A builtin has no manifest, so it declares no second stream, so a `2>` on it names
                // nothing. Same sentence a program that declares none gets, for the same reason.
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

    // **The head stage's input file comes off the plan, not off the line**, and not reading it here
    // was the bug milestone 40's viewer found: `doc page.md | wc` planned correctly, spawned the
    // head with an empty input slot, where a `receive` answers `NoSuchSlot` rather than blocking, so
    // the stage counted an empty stream and reported it. A wrong answer, not a hang.
    //
    // An input operand is a trailing positional at a program that declares an input
    // (`grant_plan::plan_against_with`), which is the same rule that makes `wc report.txt` mean
    // `wc < report.txt`. Only the planner knows it, because it needs the manifest; reading it off
    // `Line` would be the shell classifying tokens the planner has already classified. `plans[0]`
    // is `None` only for a producing builtin at the head, and a builtin has no operand.
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
/// **The shell is the process behind a `>` and a `<`**, and that is milestone 50's one wiring
/// decision that is not the obvious one. See notes/pipes.md: `filesystem_contract` shares a single page between
/// the FS server and its client, so two client processes racing to stage bytes in it is not sound,
/// and `ls > out.txt` is precisely a line where the shell must read the filesystem *while* the
/// redirection is being written. The shell already holds the directory capability, so it is the one
/// process that can back both ends without a second session.
///
/// What the child holds is unchanged by that, which is why it costs the milestone nothing: a
/// redirected program holds an rendezvous with `WRITE` and no way to ask what is behind it.
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

    // **This shell can be at one end of a chain, never both** (`grant_plan::check_chain`,
    // notes/pipes.md). It is the producer whenever the bytes going into the head come from here: a
    // `<`, an input operand, or a producing builtin. It is always the consumer, because the tail's
    // sink is this shell's result rendezvous or a file this shell backs. With one wait point per
    // process and no select, that only works if some stage reads to the end before it writes.
    //
    // **Before the files are opened**, which is the whole reason this is the first thing in the
    // function: a `>` truncates, and a line that is not going to run must not have emptied a file.
    if let Err(r) = grant_plan::check_chain(inp.is_some() || head_builtin, &plans[..n]) {
        say_chain_refused(&plans[..n], r);
        return;
    }

    // The files first, before any process exists. A line whose file cannot be opened must not
    // half-run, and `>` truncates here, which is where a person expects it: `date > f` empties `f`
    // even if `date` then says nothing. `>>` is the same call with the emptying left out.
    let Some((mut sink, mut source, mut diag_sink)) = open_line_files(nav, l, out, inp, diag_file)
    else {
        return;
    };
    // How many stages were handed the diagnostic rendezvous, which is what [`drain_diagnostics`]
    // counts end-of-stream messages against. It comes from the manifests rather than from the line:
    // a declared stream exists whether or not a `2>` named a destination for it.
    // Only the stages whose second stream this shell is backing, which is the ones a `2>` named a
    // file for. A `Diagnostics::Printed` stage writes to the terminal's sink adapter and this shell
    // never sees those bytes, which is the whole point of that component existing.
    let declarers = plans
        .iter()
        .take(n)
        .filter(|p| p.is_some_and(|e| matches!(e.diagnostics, line::Diagnostics::File(..))))
        .count();

    // **DECISIONS §106's narrowing rule, decided here before anything spawns.** An unredirected
    // tail (no `>` at the end of the line: `sink.is_none()`, this function's own `sink` being the
    // file this shell opened for one, not to be confused with `Endowment::sink`) whose program both
    // writes and reads is screen-narrowed: `terminal_sink_caretaker` takes its primary output
    // instead of this shell's own result rendezvous. `check_chain` above already used the identical
    // condition (the tail's `Sink` and its `writes_while_reading`) to let a chain with no absorbing
    // barrier through, so this has to agree with it exactly.
    //
    // **`check_chain` cannot itself confirm the rendezvous mints**, because it is host-testable, pure
    // logic with no capability to retype anything, so this is the one place that can. A line
    // `check_chain` allowed on the assumption of narrowing must not fall back to spawning the child
    // unnarrowed on a failed mint: that would reintroduce the exact deadlock `check_chain` exists to
    // refuse (the shell already mid-feed and the child's output routed back to `result_ep` with
    // nobody there to read it), so a mint failure is a loud refusal here, not a silent fallback.
    let tail_writes_while_reading = plans[n - 1].is_some_and(|e| e.writes_while_reading);
    let needs_screen = sink.is_none() && tail_writes_while_reading;
    let Ok(screen_ep) = screen_for_tail(needs_screen) else {
        return;
    };

    // **A builtin with a file on its output spawns nothing at all.** `ls > out.txt` is one process:
    // the shell reads the directory and the shell writes the file. There is no pipe to mint, no
    // stage to start, and no capability to move, because both ends are already this shell's.
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

    // **Every stage that hears words gets its argv written before any stage is spawned**
    // (milestone 205, DECISIONS §170), one window each, so running out of budget partway spawns
    // nothing. The regions are kept until the line is drained: the progenitor copies each frame
    // while it serves that stage's request, and a region given back sooner would unmap the page
    // under it.
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

    // **Diagnostics before anything else**, because in `date | wc` this shell is not the reader of
    // `date`'s output and draining the output first would wait on `wc`, which waits on `date`, which
    // is blocked in a rendezvous send on the diagnostic rendezvous. See [`drain_diagnostics`].
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

    // And the tail's answer. `>` is the shell putting those bytes somewhere else, not the child
    // being handed something different, so a redirected tail is unchanged: it still arrives on the
    // shell's own result rendezvous. **A screen-narrowed tail's bytes never arrive here at all**
    // (DECISIONS §106): they went straight to `terminal_sink_caretaker`, and what this shell waits
    // for instead is the child's exit.
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
        print(e.prog.name().as_bytes());
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
        // A `2>` on a line where nothing was spawned to write to it. Close the file rather than
        // leaving a handle open on a name the line created.
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

    // **The second stream's file, opened on the same terms as the output's** (DECISIONS §67). Held
    // as an `Option` beside the sink, because a line may name both and they are two files.
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
    // A `<` needs one more rendezvous than a bare pipeline does: the head stage reads a pipe like any
    // other stage, and the shell is what writes into it. That is the input side of "the same
    // contract, read from the other end" made concrete: nothing about the head stage knows.
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
) -> Result<(), (grant_plan::Prog, Option<&'static [u8]>)> {
    for i in 0..plans.len() {
        let Some(e) = plans[i].filter(|_| !(i == 0 && head_builtin)) else {
            continue;
        };
        let grant_plan::ProgramArgumentDeclaration::Words(grant) = e.prog.manifest().arg else {
            continue;
        };
        let assembled = assemble_argv(l.stages()[i], i)
            .map_err(|why| why.map(|r| r.message().as_bytes()))
            .and_then(|a| {
                argvs[i] = Some(a);
                match e.dir {
                    Some(g) => words_grant(&g, grant, i).map(|w| {
                        sets[i] = Some(w);
                    }),
                    None => Ok(()),
                }
            });
        if let Err(why) = assembled {
            return Err((e.prog, why));
        }
    }
    Ok(())
}

/// Say why [`write_stage_words`] could not write `prog`'s words: its sentence, or that the
/// budget ran out.
fn say_stage_words_failed(prog: grant_plan::Prog, why: Option<&[u8]>) {
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
        // Only the tail stage is ever eligible (`screen_ep` is `None` unless the tail's own sink is
        // unset), and `sink_for` never puts `Sink::Report` anywhere but the last stage, so this
        // could equally read `i == n - 1` without the `Some` check; both say the same thing.
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
/// Two implementations, and the whole of what `>` cost: an rendezvous (the next stage of a pipeline)
/// and a file. A builtin cannot tell them apart, exactly as a spawned program cannot tell what is in
/// its output slot, and for the same reason: it is handed a place to push bytes and nothing else.
pub(super) trait ByteOut {
    fn push(&mut self, bytes: &[u8]);
    /// Say the stream is over. On a pipe that is `OPERATION_EOF`; on a file it is the last write.
    fn finish(&mut self);
}

/// **Run a producing builtin with the given destination as its output**, one message at a time.
///
/// The builtins already write through a callback rather than through `print`, because the witness
/// roles have no terminal. That was done for testability and it pays off twice here: the same `echo`
/// and the same `ls` feed a pipe *and* a file with no branch in either of them.
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
    // End of stream whatever happened, including a refusal: the reader downstream is a process that
    // is blocked on a receive, and leaving it there because the shell had nothing to say would turn
    // a refused line into a hung prompt.
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
/// makes a request. Small on purpose: the shell runs on the four stack pages the progenitor maps it, and the
/// page itself is sixteen times this. A larger buffer would buy fewer FS round trips and cost the
/// one resource this program is actually short of.
const FILE_CHUNK: usize = 256;

/// **Walk to the directory a resolved grant records**, from the root of what this shell holds.
///
/// The grant carries a position rather than a token (`grant_plan::designate` resolved the name once, at
/// plan time), so this re-opens that position rather than re-reading the line. Nothing here consults
/// where the shell is standing *now*, which is what makes a `cd` between planning and running unable
/// to change what a redirection means.
///
/// The walk is from the root of the tree the grant names (`t`, milestone 154), so a grant planned
/// into a two-grant shell's second tree is opened there and nowhere else.
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
/// `CREATE` is create, not create-or-open (DECISIONS §27), so the fallback is explicit and is the
/// same one `fixtures/src/file_sink.rs` makes: on any refusal, open the existing name. What happens next is
/// the whole of `>` versus `>>`, and it happens **here, in the shell**, because the shell is what
/// backs the file (DECISIONS §55). The child is wired identically either way and has no message it
/// could send to find out which.
///
/// - `>` ([`line::Mode::Truncate`]) truncates, and does it **before the command runs**, which is
///   why `ls > out.txt` reports one more name than the `ls` before it did.
/// - `>>` ([`line::Mode::Append`]) asks the file how big it is and starts writing there. `FSTAT`
///   rather than a seek flag, because [`FileOut`] already carries its own running offset: every
///   write it makes names an absolute position, so "append" is an initial value and not a mode the
///   filesystem has to hold.
fn open_sink(nav: &mut Nav, g: grant_plan::FileGrant, mode: line::Mode) -> Result<FileOut, Say> {
    let t = nav.tree(g.which);
    let mut tmp = [0u64; nav::MAX_DEPTH];
    let (dir_handle, opened) = open_at(nav, t, g.dir, &mut tmp)?;
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
    let (dir_handle, opened) = open_at(nav, t, g.dir, &mut tmp)?;
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
/// It holds an open handle and a running offset, and buffers into [`FILE_CHUNK`] before each write,
/// because the FS contract's unit is a page and the sink contract's is sixteen bytes: writing every
/// message straight through would cost a filesystem round trip per sixteen bytes.
struct FileOut {
    /// The slot the directory capability lives in. A `u64` rather than a borrow of [`Nav`], so a
    /// builtin that borrows the shell's navigator can still write through this.
    dir: u64,
    handle: u64,
    off: u64,
    buf: [u8; FILE_CHUNK],
    n: usize,
    /// The errno of the first refused write, or 0. Kept rather than printed at once: a producer
    /// mid-listing has nothing to do with a failure, and reporting once at the end is what a person
    /// can read.
    failed: i32,
    /// A write the server accepted and did not finish. Its own field rather than an errno, because
    /// there is no errno for it: the request did not fail, it was answered with a smaller number
    /// than it asked for, and saying `EIO` would be inventing a fact.
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

    /// Say what happened, once, after the whole stream is through. Silence on success, because that
    /// is what `>` says everywhere else and because the bytes went into a file rather than onto the
    /// screen precisely so they would not be printed.
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
    /// **Stream the file over the sink contract**, which is the whole of `<`: the head stage holds
    /// the read end of an rendezvous and receives `OPERATION_BYTES` until `OPERATION_EOF`, exactly as it would from
    /// a program on the left of a `|`. Nothing about the file reaches it.
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
/// [`drain_text`] with a different destination, and deliberately not a parameterised version of it:
/// the printing one puts a two-space prefix on the answer and this one must put nothing at all in
/// the file that the program did not write.
fn drain_into(f: &mut FileOut) {
    for _ in 0..MAX_OUTPUT_CHUNKS {
        let (w0, w1, w2) = receive(RESULT);
        if w0 == spawnproto::SPAWN_FAILED {
            failed();
            print(b"  could not spawn (the progenitor is out of memory)\n");
            return;
        }
        // **The writer died mid-stream** (milestone 235). The file keeps what arrived before the
        // fault and is closed rather than left open, for `finish`'s own reason one arm down: a
        // half-written file a reader can open beats a handle nobody closed.
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
/// The two delegations are the entire difference between a piped stage and an ordinary spawn, and
/// they are why `>` and `|` are one mechanism: the progenitor receives an rendezvous and puts it where the
/// result rendezvous would have gone. Nothing here knows or can find out what is on the other end.
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
    let mem_slot = if e.mem_pages > 0 {
        match memory_region_split(e.mem_pages) {
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
        // **A stage is directory-granted only for words** (milestone 205): a program that hears
        // words is granted what they designated, here as on a plain line. `rm`'s grant is still
        // met only in `spawn`: the one program with `DirectoryGrantDeclaration::Required` writes a byte stream and
        // takes no input, so `rm x | wc` puts it at the head, and delivering it here would be a
        // second copy of `dir_grant`'s refusals. See notes/dir-capability.md's BUGS.
        dir: words.is_some(),
        dir2: false,
        screen: screen.is_some(),
        // A pipeline stage is always a program the image names (§219 D's first cut runs an image
        // only on a plain line; see `run_image`).
        image: false,
        run_unvouched: false,
        args: argv.is_some(),
        nameset: words.is_some_and(|w| w.set.is_some()),
        machine: machine_wiring(&e),
        // **A stage never promises devices** (milestone 632 (provisional)): `graphical_terminal`'s manifest
        // refuses every operator that builds a stage (`Words` output refuses `>` and a pipe's
        // left, `InputDeclaration::Forbidden` its right), so this is false by construction rather than
        // by decision, and says so for the reader checking the wire.
        graphics: false,
        keyboard: false,
    };
    let (w0, w1, w2) = spawnproto::request(e.prog.id(), e.arg, e.mem_pages, wiring);
    send(SPAWN, w0, w1, w2);
    // The words grant's two data messages first, as `spawn` sends a directory grant's.
    if let Some(DirWords { caretaker, child }) = words.map(|w| w.words) {
        send(SPAWN, caretaker.0, caretaker.1, caretaker.2);
        send(SPAWN, child.0, child.1, child.2);
    }
    // **The argv's frame, then the name set's, before every other capability**
    // (`spawnproto::ARGS_BIT`, `NAMESET_BIT`), as `spawn` sends them.
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
        // READ (not WRITE): the progenitor installs this as the *child's* fault target, and what this shell
        // needs back from its own copy is the right to `RECEIVE`/`REAP` on it, exactly
        // `job_undertaker`'s DEATHS. The progenitor narrows its own copy no further than that when it inserts
        // the child's (`abi::rights::READ`, `supervision_protocol::build_child_space`), so delegating
        // less than READ here would leave the progenitor unable to hand the child anything at all.
        delegate(slot, abi::rights::READ | abi::rights::GRANT);
    }
    if let Some(slot) = mem_slot {
        // Every right, for `running`'s reason (milestone 824 (a retype mints no right its budget
        // lacks)).
        delegate(slot, abi::rights::ALL);
        cap_delete(slot);
    }
    delegate_machine_page(wiring.machine);

    // A stage whose output was substituted owes this shell no answer, so the progenitor acks instead. Without
    // it a failed spawn would be invisible and the pipeline would wait on a producer that does not
    // exist. A screen-narrowed stage is the same shape: its completion signal is the fault rendezvous
    // above, not this one, so a build failure has to reach the shell here too (DECISIONS §106).
    //
    // **A fault word can arrive here too** (milestone 235), and it is never this stage's: it is an
    // earlier stage of the same line that died between its own ack and this one. Either way the
    // line is over, so both arms return false and only the sentence differs.
    if wiring.sink || wiring.screen {
        match receive(RESULT).0 {
            w if w == spawnproto::SPAWN_OK => {}
            w if w == spawnproto::JOB_FAULTED => {
                failed();
                print(swish::FAULTED_SENTENCE);
                return false;
            }
            _ => {
                failed();
                print(b"  could not spawn (the progenitor is out of memory)\n");
                return false;
            }
        }
    }
    true
}

/// Give the pipeline's memory back.
///
/// **The `DESTROY` is what makes a dead reader visible to a live writer.** An rendezvous is an object
/// in a page of this region, so deleting the capabilities that name it leaves it alive; reclaiming
/// the region is what turns a producer's next `SEND` into `abi::Error::Gone`. A pipeline that ended
/// with a stage still writing (the `yes | head` shape) ends here.
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
