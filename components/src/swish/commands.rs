//! **Running a line**: the builtins, `time`, `xargs`, `user`, and the dispatch that sends
//! anything else to a program.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// **Run one line if it is a navigation builtin**, calling `each` for every entry a listing
/// produces. `None` means the line was not one of them.
///
/// Split out from [`dispatch`] because the navigating witness ([`navigate`]) runs the *same*
/// builtins from the same command lines with no terminal to print to. One implementation, two
/// callers, and the guest test therefore exercises what the prompt exercises rather than a
/// reimplementation of it.
pub(super) fn builtin(nav: &mut Nav, cmd: &[u8], each: &mut dyn FnMut(&[u8], bool)) -> Option<Say> {
    match grant_plan::parse(cmd) {
        // They spawn nothing and grant nothing: the shell is rebinding and reading a capability it
        // already holds, which is why these are builtins, and why the worry about an over-granted
        // listing *program* (which would hold the power to read everything it lists) does not arise.
        Command::Cd(path) => Some(nav.cd(path)),
        Command::Ls(path) => Some(nav.ls(path, each)),
        Command::Mkdir(path) => Some(nav.mkdir(path)),
        Command::Touch(args) => Some(nav.touch(args)),
        Command::Bind(target, name) => Some(nav.bind(target, name)),
        _ => None,
    }
}

/// **Read one typed line and run the commands on it** (milestone 67, notes/swish-language.md): the
/// outermost thing the prompt calls. It cuts the line on `;`, `&&` and `||` and hands each segment
/// to [`dispatch`], so a segment is a whole command line and nothing under here learned a grammar.
///
/// A connector carries one bit and no capability: each segment is planned from scratch, and the
/// line is cut here, above [`pipeline`](fn@pipeline), so each segment's pipeline region is its own
/// and is `DESTROY`ed with it, which is what turns a stalled writer's next `SEND` into `Gone`
/// (notes/pipes.md). [`LAST`] is written only by a segment that ran, bash's rule for `$?`.
pub(super) fn dispatch_line(nav: &mut Nav, cmd: &[u8]) {
    // **A line ending in a single `&`** (milestone 48 (job control)) is one command run in the
    // background, refused before the connectors are looked for, since a chain cannot be.
    if let Some(job) = sequence::background(cmd) {
        CURRENT.store(Status::Ran.code(), core::sync::atomic::Ordering::Relaxed);
        background(nav, job);
        LAST.store(
            CURRENT.load(core::sync::atomic::Ordering::Relaxed),
            core::sync::atomic::Ordering::Relaxed,
        );
        return;
    }
    let seq = match sequence::split(cmd) {
        Ok(s) => s,
        Err(r) => {
            // A line that cannot be cut runs nothing, so it is one refusal and one status.
            CURRENT.store(Status::Ran.code(), core::sync::atomic::Ordering::Relaxed);
            say(Say::Cannot(r));
            LAST.store(
                CURRENT.load(core::sync::atomic::Ordering::Relaxed),
                core::sync::atomic::Ordering::Relaxed,
            );
            return;
        }
    };
    for &(joint, segment) in seq.segments() {
        if !joint.runs_after(last()) {
            continue;
        }
        CURRENT.store(Status::Ran.code(), core::sync::atomic::Ordering::Relaxed);
        dispatch(nav, segment);
        LAST.store(
            CURRENT.load(core::sync::atomic::Ordering::Relaxed),
            core::sync::atomic::Ordering::Relaxed,
        );
    }
}

/// **Act on one command line**, routed by [`swish::route`] (host-tested there) to the capability each
/// answer reaches for. A line with no `>`, `<` or `|` takes the path it took before those existed, so
/// the operators cost an ordinary command nothing. `caps` is answered before the line is split, or
/// `caps date | wc` would lose the pipe.
pub(super) fn dispatch(nav: &mut Nav, cmd: &[u8]) {
    match swish::route(cmd) {
        Route::Caps(tail) => caps(nav, tail),
        Route::Time(tail) => time_command(nav, tail),
        Route::Xargs(tail) => xargs(nav, tail),
        Route::One(stage) => dispatch_one(nav, stage),
        Route::Pipeline(l) => pipeline(nav, l),
        Route::Cannot(r) => say(Say::Cannot(r)),
    }
}

/// **Run a command and say how long it took** (milestone 86, notes/time-command.md).
///
/// The tail goes back through [`dispatch`], so what you time is what you run: a pipeline as a
/// pipeline, a builtin as a builtin, a refusal as a refusal. The command is told nothing. The number
/// is wall clock taken around the dispatch, this shell's planning, spawning and draining included,
/// and not CPU time, which nothing in this kernel reports.
pub(super) fn time_command(nav: &mut Nav, tail: &[u8]) {
    // Collapse a nested prefix rather than recursing through [`dispatch`]. `time time date` is a line
    // a person can type, and each level of recursion here costs a `dispatch` frame on a stack that
    // has run out four times already (notes/pipes/the-boot.md). Timing something twice measures
    // nothing twice.
    let mut tail = grant_plan::trim(tail);
    while let Command::Time(inner) = grant_plan::parse(tail) {
        tail = grant_plan::trim(inner);
    }
    if tail.is_empty() {
        refused();
        return swish::write_untimed(Untimed::NothingToTime, &mut print);
    }
    // A duration needs no clock (§72): across a command the wall clock's offset cancels, and
    // `monotonic_nanos` is ambient (the kernel opened the counter to EL0) and cannot be stepped.
    // So `time` cannot be refused; a capability gates wall-clock identity, which is `date`'s.
    let start = monotonic_nanos();

    dispatch(nav, tail);

    // Saturating out of habit rather than need: the counter is monotonic, so `end` cannot precede
    // `start`, and a wrap would take centuries at either board's frequency.
    swish::write_timing(monotonic_nanos().saturating_sub(start), &mut print);
}

/// The most batches one sweep will run. A ceiling so a resume rule that failed to advance costs a
/// bounded number of spawns rather than a prompt that never comes back, in exactly the spirit of
/// [`ROUNDS`]. Eight names a batch, so this is 2048 names, and a directory that large is a machine's
/// problem before it is this shell's.
pub(super) const MAX_BATCHES: u64 = 256;

/// **Run a command once per batch of what its pattern matched** (milestone 109).
///
/// A prefix word and not a program: a batching program would have to hold the union of every
/// batch, more than [`grant_plan::expand::MAX_NAMES`], which nothing can carry, so the batching goes
/// where the directory authority already is. The iteration is the builtin; `rm` stays a program
/// with an attenuated grant. The sweep stops at the first batch that does not run, so what ran is a
/// prefix of the match, and each batch's set is printed before it runs, so the shell can name the
/// boundary. See notes/glob-grant/xargs.md.
pub(super) fn xargs(nav: &mut Nav, tail: &[u8]) {
    // Collapse a nested prefix rather than recursing, [`time_command`]'s reason: `xargs xargs rm
    // *.txt` is a line a person can type, batching a batch is still one sweep, and every level of
    // recursion costs a frame on a stack that has run out four times already.
    let mut tail = grant_plan::trim(tail);
    while let Command::Xargs(inner) = grant_plan::parse(tail) {
        tail = grant_plan::trim(inner);
    }
    if tail.is_empty() {
        refused();
        print(b"  xargs: name a command to batch\n");
        return;
    }
    let mut sweep = swish::Sweep::default();
    let mut resume = Resume::Start;
    for _ in 0..MAX_BATCHES {
        nav.batching = Batching {
            resume: Some(resume),
            index: sweep.batches + 1,
            next: None,
            names: 0,
        };
        // Each batch is its own command as far as the status is concerned, and the sweep stops at
        // the first one that did not run. That is the same reading `&&` takes (see [`Status`]): a
        // refusal and a failure are both a no, and which it was stays in the status the sweep
        // leaves behind for `$?`.
        CURRENT.store(Status::Ran.code(), core::sync::atomic::Ordering::Relaxed);
        dispatch(nav, tail);
        let batching = nav.batching;
        nav.batching = Batching::default();
        if !current().is_ok() {
            sweep.stop();
            break;
        }
        sweep.ran(batching.names);
        match batching.next {
            Some(r) => resume = r,
            None => break,
        }
    }
    swish::write_sweep(&sweep, &mut print);
}

/// Parse one line with `grant_plan` and act on it. All parsing and the manifest check are the host-tested
/// crate; this function is only IO and capability moves.
pub(super) fn dispatch_one(nav: &mut Nav, cmd: &[u8]) {
    if let Some(said) = builtin(nav, cmd, &mut |name, is_dir| {
        print(b"  ");
        print(name);
        if is_dir {
            print(b"/");
        }
        print(b"\n");
    }) {
        say(said);
        return;
    }
    match grant_plan::parse(cmd) {
        Command::Empty => {}
        Command::Help => help(),
        // `echo` expands, and that is half of the globbing demonstration: what it prints is
        // literally the authority the same words would transfer to a program. The newline goes out
        // only when the line itself did, so a refused expansion is one line and not two.
        Command::Echo(text) => match echo(nav, text, &mut print) {
            Say::Nothing => print(b"\n"),
            said => say(said),
        },
        // Unreachable through [`dispatch`], which answers the two prefix words before it splits the
        // line so they can see the operators. Kept so this function is complete on its own: it is
        // the whole of "what one command means", and a reader should not have to know about the
        // interception to find every arm.
        Command::Caps(tail) => caps(nav, tail),
        Command::Time(tail) => time_command(nav, tail),
        Command::Xargs(tail) => xargs(nav, tail),
        Command::Pwd => print_pwd(nav),
        // **Search, which grants nothing** (milestone 40 phase 2). It is here rather than in
        // [`builtin`] because its answer is columns of text rather than a listing, so the witness's
        // `each(name, is_dir)` callback is the wrong shape for it; what it shares with the
        // navigation builtins is the argument for being a builtin at all.
        Command::Apropos(term) => say(apropos(nav, term)),
        Command::Vouch(tail) => vouch(nav, grant_plan::vouch_verb(tail)),
        Command::User(tail) => user(nav, grant_plan::user_verb(tail)),
        Command::Jobs => list_jobs(&mut nav.jobs),
        Command::Wait(tail) => wait_jobs(&mut nav.jobs, tail),
        Command::Kill(tail) => kill_job(&mut nav.jobs, tail),
        Command::Fg(tail) => foreground(&mut nav.jobs, tail),
        Command::Bg(tail) => resume_in_background(&mut nav.jobs, tail),
        Command::Run(spec) => run(nav, cmd, spec),
        // Handled above, by the one implementation the witness also runs.
        Command::Cd(_)
        | Command::Ls(_)
        | Command::Mkdir(_)
        | Command::Touch(_)
        | Command::Bind(_, _) => {}
    }
}

/// **`user suspend <name>` and `user resume <name>`** (milestone 152 (durable delegation), §108
/// (disabling credentials kills the durable session)): edit `login_contract::SUSPENDED_LIST` at
/// this shell's root, which `login` and the boot-time re-deriver read. Owner-only by authority: only
/// the owner's console holds that root, and a confined shell edits a same-named file nothing reads.
///
/// # BUGS
///
/// It does not send `login_contract::SUSPEND`, holding no capability to `login`'s front door, so a
/// durable session ends at the identity's next login attempt (notes/shell/the-program.md).
pub(super) fn user(nav: &mut Nav, verb: grant_plan::UserVerb<'_>) {
    use grant_plan::UserVerb;
    let (name, suspend) = match verb {
        UserVerb::Suspend(n) => (n, true),
        UserVerb::Resume(n) => (n, false),
        UserVerb::Usage => {
            refused();
            print(swish::USER_USAGE);
            return;
        }
    };
    if nav.dir.is_none() {
        return say(Say::NoDirectory);
    }
    // The first tree, as `caps` reads the activation table from: the owner's lists are the
    // system's, whichever tree this shell stands in.
    let t = nav.first();
    let file = login_contract::SUSPENDED_LIST.as_bytes();
    let mut list = [0u8; USER_LIST_MAX];
    let had = read_named(nav, t, fs::ROOT, file, &mut list).unwrap_or(0);
    let mut edited = [0u8; USER_LIST_MAX];
    let n = if suspend {
        login_contract::with_listed(&list[..had], name, &mut edited)
    } else {
        login_contract::without_listed(&list[..had], name, &mut edited)
    };
    let Some(n) = n else {
        refused();
        print(b"  user: the suspended list is full\n");
        return;
    };
    let mut h = nav.name_call_in(t, fs::CREATE, fs::ROOT, file, 0);
    if h < 0 {
        h = nav.name_call_in(t, fs::OPEN, fs::ROOT, file, 0);
    }
    if h < 0 {
        return say(Say::Failed(-h as i32));
    }
    let h = h as u64;
    let truncated = call(t.slot, fs::req(fs::TRUNCATE, h, 0), 0).0 as i64;
    put_page(&edited[..n]);
    let wrote = call(t.slot, fs::req(fs::WRITE, h, n as u64), 0).0 as i64;
    nav.close_in(t, h);
    if truncated < 0 || wrote != n as i64 {
        return say(Say::Failed(
            if truncated < 0 { -truncated } else { 5 } as i32
        ));
    }
    print(if suspend {
        b"  suspended "
    } else {
        b"  resumed "
    });
    print(name);
    print(b"\n");
}

/// The longest suspended list `user` edits: two kilobytes, more than a hundred names.
pub(super) const USER_LIST_MAX: usize = 2048;

/// Print where we are, relative to our own root.
pub(super) fn print_pwd(nav: &Nav) {
    if nav.dir.is_none() {
        say(Say::NoDirectory);
        return;
    }
    swish::write_pwd(&holdings(nav), &mut print);
}
