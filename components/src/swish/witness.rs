//! **The witness roles**: the six test roles `system_tests` spawns this shell in, and the
//! helpers only they use. [`_start`](crate::entry::_start) dispatches them by role number.
//!
//! **This is a temporary home** (calef, 2026-10-11 UTC, on §273's first fork: "Proceed with
//! both"). Milestone 885 (the shell's test roles become scripts the shipping shell runs) deletes
//! this file and the role numbers; until then the shipping binary carries them. Each script's
//! design is in `notes/swish-language/witness.md`.
//!
//! Name: provisional 2026-10-11 (UTC), milestone 844's lane. calef deferred every module name
//! in §273 to a name-ratification pass.

use super::*;

/// The constructors and the one request only the witnesses use, kept out of the
/// shipping `navigation` module (§273 (the shell program becomes a directory of modules)).
impl Nav {
    /// A shell rooted at the directory capability in [`DIR`], carrying `rights`.
    fn rooted(rights: u64) -> Self {
        Nav::rooted_at(DIR, rights)
    }

    /// **A shell holding two trees** (milestone 154): the first at `a`, the second at `b` mounted
    /// at `mount` in the first, standing at the root of the one tree it presents.
    fn rooted_twice(a: Tree, b: Tree, mount: grant_plan::MountedTree) -> Self {
        let mut nav = Nav::rooted_at(a.slot, a.rights);
        nav.holds.mounted = Some(mount);
        nav.second = Some(b);
        nav
    }

    /// One request that names something, in the tree this shell stands in: stage the name in the
    /// shared page and call. For a handle a walk produced, [`Nav::name_call_in`] with the walk's
    /// tree, since the walk may have been into the other one.
    fn name_call(&self, verb: u64, handle: u64, name: &[u8], w1: u64) -> i64 {
        self.name_call_in(self.cur(), verb, handle, name, w1)
    }
}

/// **A shell driven by a script instead of a keyboard, running real pipelines** (milestone 50).
///
/// Every line goes through [`dispatch`], the prompt's own function, and the transcript printed over
/// the terminal contract is the assertion. The answers check each other: `date | wc` has to count
/// `date`'s length. See `notes/swish-language/witness.md`.
pub(super) fn piping() -> ! {
    let mut nav = Nav::empty();
    for line in [
        // The shell as the producer: a builtin's bytes into a pipe, counted by a real process.
        &b"echo hello world | wc"[..],
        // Three stages, so a middle stage is a real middle: `wc`'s own output is a byte stream, so
        // it composes, which is what `OutputDeclaration::Bytes` on `wc` is for.
        b"echo hello world | wc | wc",
        // The same unmodified `date`, twice, to two different destinations.
        b"date",
        b"date | wc",
        // And the refusals, at the prompt, with nothing spawned.
        b"wc",
        b"least_authority_demo 9 | wc",
        b"date | date",
        b"date > report.txt",
    ] {
        print(b"$ ");
        print(line);
        print(b"\n");
        dispatch_line(&mut nav, line);
    }
    print(PIPELINE_DONE);
    exit();
}

/// The last thing [`piping`] prints, so a reader knows the transcript is complete rather than
/// having to guess from a quiet rendezvous. Must match `system_tests::user::pipeline_tests`.
const PIPELINE_DONE: &[u8] = b"== pipelines done\n";

/// **A scripted shell that holds a filesystem as well as a spawn channel** (milestone 50).
///
/// [`piping`]'s wiring plus one capability, the whole difference between a `>` that is refused and
/// a `>` that writes a file. Every line goes through [`dispatch`], and the answers check each other
/// rather than constants. See `notes/swish-language/witness.md`, "The redirecting script".
pub(super) fn redirecting(rights: u64) -> ! {
    let mut nav = Nav::rooted_at(DIR_TERMINAL, rights);
    for line in [
        // The shell as producer *and* as sink: no process is spawned by this line at all.
        &b"ls > out.txt"[..],
        b"ls",
        b"wc < out.txt",
        // The same designation with the operator left out (milestone 31's input operand). It has
        // to answer what the line above it answered, or one of the two opened something else.
        b"wc out.txt",
        // The same designation inside a pipeline: the head's input is wired off the plan, and the
        // answer has to be the length of the line above it.
        b"wc out.txt | wc",
        // The same unmodified `date`, twice, to two destinations.
        b"date",
        b"date > date.txt",
        b"wc < date.txt",
        // `>` twice keeps the second line only; `>>` keeps both. Same commands, same order, one
        // character different, and the two answers are each other's control.
        b"echo one > trunc.txt",
        b"echo two > trunc.txt",
        b"wc < trunc.txt",
        b"echo one > app.txt",
        b"echo two >> app.txt",
        b"wc < app.txt",
        // And `>>` creates a name that is not there, so it is not "open then seek": there is
        // nothing to open. Nothing between this and the count writes to it.
        b"echo one >> fresh.txt",
        b"wc < fresh.txt",
        // And the refusals a directory does not rescue.
        b"wc < nosuch.txt",
        b"least_authority_demo 9 > out.txt",
        // **`2>` binds to a declaration** (DECISIONS §67). `wc` writes one stream and its
        // diagnostics ride it, so the operator names nothing and the line does not run. The
        // sentence is the assertion; what it is *not* is a permission.
        b"wc out.txt 2> err.txt",
        // ---- milestone 67's grammar, on this same shell (notes/swish-language.md) ----
        //
        // On this shell, not a seventh: another live scripted shell puts `time_tests` over the
        // frame pool (`notes/swish-language/witness.md`, "One witness, not a seventh").
        //
        // A name with a space in it, written and read back.
        b"echo hello world > \"my notes.txt\"",
        b"wc < \"my notes.txt\"",
        // The same designation with the operator left out, which has to agree with the line above
        // it or one of the two opened something else.
        b"wc \"my notes.txt\"",
        // Quoted and unquoted, side by side: the same four characters are one name and a set.
        b"echo \"*.txt\"",
        b"echo *.txt",
        // A quoted operator is text, so this line has no redirection on it and writes no file.
        b"echo 'a > b'",
        // Sequencing. `least_authority_demo 3` succeeds and `least_authority_demo` alone is refused at the prompt, so these
        // lines cover every arm of the condition table with real commands.
        b"least_authority_demo 3 && echo yes",
        b"least_authority_demo && echo yes",
        b"least_authority_demo 3 || echo no",
        b"least_authority_demo || echo no",
        // `;` runs the second whatever the first did, which is the arm the four above cannot show.
        b"least_authority_demo && echo yes ; echo always",
        // The status, after a command that ran and after one this shell refused. The second is the
        // decision this milestone settled: a refusal is 2 rather than 1, because nothing ran.
        b"least_authority_demo 3",
        b"echo $?",
        b"least_authority_demo",
        b"echo $?",
        // And the refusals the grammar itself makes, which run nothing at all.
        b"echo 'unclosed",
        b"date &&",
    ] {
        print(b"$ ");
        print(line);
        print(b"\n");
        dispatch_line(&mut nav, line);
    }
    print(REDIRECT_DONE);
    exit();
}

/// [`PIPELINE_DONE`]'s twin for [`redirecting`]. Must match `system_tests::user::redirection_tests`.
const REDIRECT_DONE: &[u8] = b"== redirections done\n";

/// **A scripted shell that times what it runs** (milestone 86).
///
/// [`piping`]'s wiring plus at most one capability, the clock page. The same script runs against a
/// published page, a blank page and no page, so each refusal comes from the capability table and
/// not a test branch. The lines check each other. See `notes/swish-language/witness.md`, "The
/// timing script".
pub(super) fn timing() -> ! {
    let mut nav = Nav::empty();
    for line in [
        // The control: the untimed command, so the timed one below has something to agree with.
        &b"least_authority_demo 3"[..],
        // A timed spawn. `least_authority_demo` declares no clock and is handed none, which is the whole point of
        // the milestone: the thing being timed needs no authority to be timed.
        b"time least_authority_demo 3",
        // A timed builtin. No process, no spawn, no grant, and still a duration.
        b"time echo hello",
        // And the prefix with nothing after it.
        b"time",
    ] {
        print(b"$ ");
        print(line);
        print(b"\n");
        dispatch_line(&mut nav, line);
    }
    print(TIMING_DONE);
    exit();
}

/// [`PIPELINE_DONE`]'s twin for [`timing`]. Must match `system_tests::user::time_tests`.
const TIMING_DONE: &[u8] = b"== timings done\n";

// ---- the navigating witness (milestone 47) ----

/// Where the witness reports its bitmap (`fs_service::start_granted_dir` grants it at 1).
const REPORT: u64 = 1;

/// **A shell driven by a script instead of a keyboard**, so the builtins above can be gated on both
/// ISAs (DECISIONS §19) against a real subtree served by a real `fs_subtree_caretaker`.
///
/// `spec` carries the rights and a run index, nothing about which subtree it was rooted in. It
/// reports which of `sub`'s and `other`'s files it reached, so confinement is read off the pair of
/// runs. Lines go through [`builtin`], the prompt's path. See `notes/swish-language/witness.md`,
/// "The navigating script".
pub(super) fn navigate(spec: u64) -> ! {
    use filesystem_fixture::{VERDICT, navscape as nb, tree};
    let run = filesystem_contract::grant::spec_len(spec) as u64;
    let mut nav = Nav::rooted(filesystem_contract::grant::spec_rights(spec));
    let mut v = 0u64;

    probe_root_and_reach(&mut nav, &mut v);

    probe_listing_and_descent(&mut nav, &mut v);

    // 7. `mkdir`, which mints a capability and hands it straight back: making a directory is not
    //    going there.
    let dirname = run_name(tree::NAV_DIR, run);
    let mut cmd = [0u8; 32];
    if matches!(
        run_line(&mut nav, line(&mut cmd, b"mkdir ", &dirname)),
        Some(Say::Nothing)
    ) {
        v |= nb::MADE_DIR;
    }

    // 7b. `cd /` is `pwd`'s output typed back. The middle check stops a shell that never moved.
    //     The directory must be one this shell made: only one of the two subtrees ships a child.
    if matches!(
        run_line(&mut nav, line(&mut cmd, b"cd ", &dirname)),
        Some(Say::Nothing)
    ) && !pwd_is(&nav, b"/")
        && matches!(run_line(&mut nav, b"cd /"), Some(Say::Nothing))
        && pwd_is(&nav, b"/")
    {
        v |= nb::ABSOLUTE_IS_MY_ROOT;
    }

    probe_unlink_while_held(&mut nav, run, &mut v);

    probe_directory_removal(&mut nav, run, &dirname, &mut cmd, &mut v);

    // 11a. `touch`, the two-fold contract (see `touch_twice`).
    let touched = run_name(tree::NAV_TOUCH, run);
    touch_twice(&mut nav, &touched, &mut cmd, &mut v);

    // 11a-bis. `touch`'s mtime half, on the name `touch_twice` just made.
    touch_mtime_probes(&mut nav, &touched, &mut v);

    // 11a-ter. `WRITE` without `SETTIME`, over the real wire.
    let narrow_dir = run_name(tree::NAV_TOUCH_NO_SETTIME, run);
    touch_mtime_without_settime(&nav, name_of(&narrow_dir), &mut v);

    // 11. Neither removal reaches out of the root. Through the path resolver, not `removed`,
    //     because `..` is refused where it is parsed and nothing is sent.
    let reached_out = nav.act(b"../motd", |nav, t, handle, name| {
        if nav.name_call_in(t, fs::UNLINK, handle, name, 0) < 0 {
            Say::Failed(0)
        } else {
            Say::Nothing
        }
    });
    if matches!(reached_out, Say::Nothing) {
        v |= nb::WALKED_UP;
    }

    probe_bind(&mut nav, run, &dirname, &mut cmd, &mut v);

    // Nothing worked at all: say so, rather than letting a shell that reaches nothing pass as a
    // shell that is perfectly confined.
    if v & (nb::REACHED_INNER | nb::REACHED_SECRET | nb::LISTED) == 0 {
        v |= nb::NAVIGATION_FAILED;
    }
    send(REPORT, VERDICT, v, 0);
    exit();
}

/// [`navigate`]'s steps 1 to 4b: `pwd` and `..` at the root, then the two files reached plainly
/// and with a leading slash.
fn probe_root_and_reach(nav: &mut Nav, v: &mut u64) {
    use filesystem_fixture::{navscape as nb, tree};

    // 1. `pwd` at the start. The root of your namespace renders as `/` because it is the root of
    //    the only namespace you have.
    if pwd_is(nav, b"/") {
        *v |= nb::PWD_IS_ROOT;
    }

    // 2. **`..` at the root.** Nothing is sent: the shell holds a stack of the capabilities it
    //    descended through, and at the root there is nothing to pop. Both halves are checked, since
    //    a refusal that moved anyway would be the interesting failure.
    let said = run_line(nav, b"cd ..");
    if !pwd_is(nav, b"/") {
        *v |= nb::WALKED_UP;
    } else if matches!(said, Some(Say::Refused(Refused::AtYourRoot))) {
        *v |= nb::CLAMPED_AT_ROOT;
    }

    // 3. **An absolute path, rooted in this shell's own namespace** (milestone 47's namespace
    //    half). `/..` first, because it is the negative control: your root is the only root there
    //    is, so there is no level above it to name and the syntax meets the same wall `..` does.
    if matches!(
        run_line(nav, b"cd /.."),
        Some(Say::Refused(Refused::AtYourRoot))
    ) && pwd_is(nav, b"/")
    {
        *v |= nb::ABSOLUTE_CLAMPED_AT_ROOT;
    }

    // 4. The two files, one from each root. Exactly one of these must open, and which one is the
    //    whole point of running this twice.
    if let Some(h) = opened(nav, tree::INNER.as_bytes()) {
        *v |= nb::REACHED_INNER;
        nav.close(h);
    }
    if let Some(h) = opened(nav, tree::SECRET.as_bytes()) {
        *v |= nb::REACHED_SECRET;
        nav.close(h);
    }

    // 4b. **The same two probes, asked with a leading slash.** This is what "an absolute path
    //     grants nothing" is measured as rather than asserted: each shell reaches exactly the file
    //     its own root contains, and a `/` rooted in a global namespace would make both reach both.
    if let Some((t, h)) = opened_token(nav, tree::ABS_INNER.as_bytes()) {
        *v |= nb::ABSOLUTE_REACHED_INNER;
        nav.close_in(t, h);
    }
    if let Some((t, h)) = opened_token(nav, tree::ABS_SECRET.as_bytes()) {
        *v |= nb::ABSOLUTE_REACHED_SECRET;
        nav.close_in(t, h);
    }
}

/// [`navigate`]'s steps 5 and 6: what `ls` shows, then down one level and back.
fn probe_listing_and_descent(nav: &mut Nav, v: &mut u64) {
    use filesystem_fixture::{navscape as nb, tree};

    // 5. `ls`. A listing is a rendering of authority, so what is in it is checked and not merely
    //    counted: a name from the other shell's root appearing here would be an escape even though
    //    nothing was opened.
    let mut saw = 0u64;
    let said = nav.ls(b"", &mut |name, _| {
        if name == tree::INNER.as_bytes() {
            saw |= nb::SAW_INNER;
        }
        if name == tree::SECRET.as_bytes() {
            saw |= nb::SAW_SECRET;
        }
    });
    if matches!(said, Say::Nothing) {
        *v |= nb::LISTED;
    }
    *v |= saw;

    // 6. Down one level and back. `pwd` has to follow, or the shell's idea of where it is and the
    //    capability it is resolving against have come apart.
    if matches!(run_line(nav, b"cd deeper"), Some(Say::Nothing)) && pwd_is(nav, b"/deeper") {
        *v |= nb::DESCENDED;
        if matches!(run_line(nav, b"cd .."), Some(Say::Nothing)) && pwd_is(nav, b"/") {
            *v |= nb::RETURNED;
        }
    }
}

/// [`navigate`]'s steps 8 and 9: create two files, then `UNLINK` one while still holding it.
/// Reports and exits if either file cannot be created.
fn probe_unlink_while_held(nav: &mut Nav, run: u64, v: &mut u64) {
    use filesystem_fixture::{VERDICT, navscape as nb, tree};

    // 8. Two files: one that stays, so the other's absence afterwards is a fact about `rm` rather
    //    than about a shell that created nothing.
    let kept = run_name(tree::NAV_KEPT, run);
    let doomed = run_name(tree::NAV_GONE, run);
    let (Some(k), Some(h)) = (created(nav, name_of(&kept)), created(nav, name_of(&doomed))) else {
        // Nothing else below can mean anything without them.
        send(REPORT, VERDICT, *v | nb::NAVIGATION_FAILED, 0);
        exit();
    };
    *v |= nb::CREATED;
    nav.close(k); // the name is what this one is for; a handle nobody closes pins the node
    put_page(tree::NAV_BODY);
    let w = call(DIR, fs::req(fs::WRITE, h, tree::NAV_BODY.len() as u64), 0).0 as i64;
    if w != tree::NAV_BODY.len() as i64 {
        *v |= nb::NAVIGATION_FAILED;
    }

    // 9. `UNLINK` while still holding the file: the name goes, the object does not. Sent as the
    //    verb `rm` sends, because `rm` is a program and this witness holds no spawn channel.
    if removed(nav, fs::UNLINK, name_of(&doomed)) {
        *v |= nb::UNLINKED;
        let mut buf = [0u8; 64];
        let n = call(DIR, fs::req(fs::READ, h, tree::NAV_BODY.len() as u64), 0).0 as i64;
        if n == tree::NAV_BODY.len() as i64 {
            get_page(n as usize, &mut buf);
            if &buf[..n as usize] == tree::NAV_BODY {
                *v |= nb::HOLDER_KEPT_READING;
            }
        }
        // And the name really is gone, or the bit above is equally true of an unlink that did
        // nothing at all.
        if opened(nav, name_of(&doomed)).is_none() {
            *v |= nb::NAME_GONE_AFTER_UNLINK;
        }
    }
    nav.close(h);
}

/// [`navigate`]'s step 10: `UNLINK` refuses a directory and `RMDIR` refuses a non-empty one.
fn probe_directory_removal(
    nav: &mut Nav,
    run: u64,
    dirname: &([u8; 16], usize),
    cmd: &mut [u8; 32],
    v: &mut u64,
) {
    use filesystem_fixture::{navscape as nb, tree};

    // 10. `UNLINK` of a directory is refused, and so is `RMDIR` of a non-empty one: no single call
    //     on this contract takes a subtree away.
    if !removed(nav, fs::UNLINK, name_of(dirname)) {
        *v |= nb::UNLINK_REFUSED_A_DIRECTORY;
    }
    let empty = run_name(tree::NAV_EMPTY, run);
    if matches!(
        run_line(nav, line(cmd, b"mkdir ", &empty)),
        Some(Say::Nothing)
    ) {
        // A directory with exactly one name in it: enough to make `RMDIR` refuse, and cheap to
        // take back out so the same call can be shown to work.
        let inside = nav.name_call(
            fs::CREATE,
            open_dir(nav, name_of(&empty)).unwrap_or(fs::ROOT),
            tree::NAV_INSIDE.as_bytes(),
            0,
        );
        if let Some(d) = open_dir(nav, name_of(&empty)) {
            if inside >= 0 {
                nav.close(inside as u64);
                if !removed(nav, fs::RMDIR, name_of(&empty)) {
                    *v |= nb::RMDIR_REFUSED_NON_EMPTY;
                }
                // Empty it by hand, exactly as `rm -r` does: bottom-up, one safe step at a time.
                let gone = nav.name_call(fs::UNLINK, d, tree::NAV_INSIDE.as_bytes(), 0) == 0;
                nav.close(d);
                if gone && removed(nav, fs::RMDIR, name_of(&empty)) {
                    *v |= nb::RMDIR_REMOVED_EMPTY;
                }
            } else {
                nav.close(d);
            }
        }
    }
}

/// [`navigate`]'s step 12: `bind` a nested directory under a new word and reach it through that word.
fn probe_bind(
    nav: &mut Nav,
    run: u64,
    dirname: &([u8; 16], usize),
    cmd: &mut [u8; 32],
    v: &mut u64,
) {
    use filesystem_fixture::{navscape as nb, tree};

    // 12. `bind` a directory nested in step 7's, holding a marker, under a fresh top-level word,
    //     then reach it through the bound name three ways.
    let nested = run_name(tree::NAV_BIND_NESTED, run);
    let alias = run_name(tree::NAV_BIND_ALIAS, run);
    let marker_made = matches!(
        run_line(nav, line(cmd, b"cd ", dirname)),
        Some(Say::Nothing)
    ) && matches!(
        run_line(nav, line(cmd, b"mkdir ", &nested)),
        Some(Say::Nothing)
    ) && matches!(
        run_line(nav, line(cmd, b"cd ", &nested)),
        Some(Say::Nothing)
    ) && created(nav, tree::NAV_BIND_MARKER.as_bytes())
        .map(|h| nav.close(h))
        .is_some()
        && matches!(run_line(nav, b"cd /"), Some(Say::Nothing));
    if marker_made {
        let mut bindbuf = [0u8; 64];
        let bind_cmd = bind_line(&mut bindbuf, dirname, &nested, &alias);
        if matches!(run_line(nav, bind_cmd), Some(Say::Nothing)) {
            // `ls` of the bound name lists the real directory it points at: the marker, which
            // this shell never `mkdir`ed or `cd`ed into by that name.
            let mut cmd2 = [0u8; 32];
            let mut saw_marker = false;
            let listed = nav.ls(line(&mut cmd2, b"/", &alias), &mut |name, _| {
                if name == tree::NAV_BIND_MARKER.as_bytes() {
                    saw_marker = true;
                }
            });
            if matches!(listed, Say::Nothing) && saw_marker {
                *v |= nb::BIND_REACHED_TARGET;
            }

            // `/<dirname>`, the real parent's own path: built once, so the check below compares
            // against the *real* tree rather than the alias, which a position has no memory of
            // having been reached through.
            let mut want = [0u8; nav::RENDER_MAX];
            want[0] = b'/';
            want[1..1 + dirname.1].copy_from_slice(name_of(dirname));
            let want = &want[..1 + dirname.1];

            // `cd` through the bound name, then `..` three times: to the real parent (not a
            // boundary invented at the alias), to the real root, and refused there exactly where
            // a direct walk to that depth would refuse, never one step earlier or later.
            if matches!(
                run_line(nav, line(&mut cmd2, b"cd /", &alias)),
                Some(Say::Nothing)
            ) && matches!(run_line(nav, b"cd .."), Some(Say::Nothing))
                && pwd_is(nav, want)
            {
                *v |= nb::BIND_ASCEND_REACHES_REAL_PARENT;
                if matches!(run_line(nav, b"cd .."), Some(Say::Nothing))
                    && pwd_is(nav, b"/")
                    && matches!(
                        run_line(nav, b"cd .."),
                        Some(Say::Refused(Refused::AtYourRoot))
                    )
                    && pwd_is(nav, b"/")
                {
                    *v |= nb::BIND_STOPS_AT_TRUE_ROOT;
                }
            }
        }
    }
}

// ---- the two-tree witness (milestone 154) ----

/// **A shell holding two trees, driven by a script** (milestone 154 (a process that holds two
/// directory capabilities), one tree with the second mounted in it, as calef ruled on 2026-09-26).
///
/// Wired by `fs_service::start_granted_two_dirs`: `sub` at slot 0, `other` at slot 1, both with
/// `spec`'s rights, and a report rendezvous at slot 2. Lines go through [`builtin`] and
/// [`open_source`], the shell's own paths. Nothing here may print: this wiring maps no terminal
/// page, so [`print`] would fault. See `notes/swish-language/witness.md`, "The two-tree script".
pub(super) fn two_trees(spec: u64) -> ! {
    use filesystem_fixture::{VERDICT, tree, twotrees as tt};
    const A: u64 = 0;
    const B: u64 = 1;
    const REPORT: u64 = 2;
    let rights = filesystem_contract::grant::spec_rights(spec);
    let mount = nav::path(tt::MOUNT.as_bytes())
        .ok()
        .and_then(|p| grant_plan::Holdings::default().resolve_path(&p).ok())
        .and_then(grant_plan::MountedTree::mounted_at);
    let (Some(mount), true) = (mount, rights != 0) else {
        send(REPORT, VERDICT, tt::TWO_TREES_FAILED, 0);
        exit();
    };
    let mut nav = Nav::rooted_twice(Tree { slot: A, rights }, Tree { slot: B, rights }, mount);
    // Helpers rather than one body, each its own frame and none nested in another: a planned
    // endowment is large, and this wiring maps a few pages of stack for a debug build.
    let mut v = two_tree_moves(&mut nav);
    v |= two_tree_grants(&mut nav);
    v |= two_tree_rm(&mut nav);
    let (inner, secret) = (tree::INNER.as_bytes(), tree::SECRET.as_bytes());

    // 7. A bound name can point into the mount, and the mount point's own name cannot be bound.
    if matches!(run_line(&mut nav, b"bind /second bee"), Some(Say::Nothing))
        && matches!(
            run_line(&mut nav, b"bind / second"),
            Some(Say::CannotBind(_))
        )
        && listing_is(&mut nav, b"/bee", secret, inner)
    {
        v |= tt::BOUND_INTO_MOUNT;
    }

    // 8. Home is the one root.
    if matches!(run_line(&mut nav, b"cd /second"), Some(Say::Nothing))
        && matches!(run_line(&mut nav, b"cd"), Some(Say::Nothing))
        && pwd_is(&nav, b"/")
    {
        v |= tt::HOME_IS_ROOT;
    }

    if v & (tt::LISTED_ROOT | tt::LISTED_MOUNT | tt::OPENED_RELATIVE_IN_MOUNT) == 0 {
        v |= tt::TWO_TREES_FAILED;
    }
    send(REPORT, VERDICT, v, 0);
    exit();
}

/// [`two_trees`]' moves: where it starts, into the mount and out of it, and the refusals that
/// must not move it.
fn two_tree_moves(nav: &mut Nav) -> u64 {
    use filesystem_fixture::{tree, twotrees as tt};
    let mut v = 0u64;
    let inner = tree::INNER.as_bytes();
    let secret = tree::SECRET.as_bytes();

    // 1. One root, and the mount point listed in it.
    if pwd_is(nav, b"/") {
        v |= tt::PWD_STARTS_AT_ROOT;
    }
    if listing_is(nav, b"", inner, secret) && listing_is(nav, b"/", b"second", secret) {
        v |= tt::LISTED_ROOT;
    }

    // 2. Into the mount, with no new verb.
    if matches!(run_line(nav, b"cd /second"), Some(Say::Nothing)) && pwd_is(nav, b"/second") {
        v |= tt::MOVED_INTO_MOUNT;
    }
    if listing_is(nav, b"", secret, inner) {
        v |= tt::LISTED_MOUNT;
    }
    if let Some(h) = opened(nav, secret) {
        v |= tt::OPENED_RELATIVE_IN_MOUNT;
        nav.close(h);
    }
    if let Some(h) = opened(nav, inner) {
        v |= tt::REACHED_ACROSS;
        nav.close(h);
    }

    // 3. The first tree by an absolute path, without moving; and the two names that must not
    //    cross.
    if let Some((t, h)) = opened_token(nav, b"/inner") {
        v |= tt::OPENED_ROOT_FROM_MOUNT;
        nav.close_in(t, h);
    }
    for token in [&b"/secret"[..], b"/second/inner"] {
        if let Some((t, h)) = opened_token(nav, token) {
            v |= tt::REACHED_ACROSS;
            nav.close_in(t, h);
        }
    }

    // 4. `..` from the mount point is its parent, and back in by a relative name.
    if matches!(run_line(nav, b"cd .."), Some(Say::Nothing))
        && pwd_is(nav, b"/")
        && matches!(run_line(nav, b"cd second"), Some(Say::Nothing))
        && pwd_is(nav, b"/second")
    {
        v |= tt::UP_FROM_MOUNT_IS_PARENT;
    }

    // 5. `..` past the one root refuses and moves nothing, from inside the mount and from `/`.
    let mut clamped = 0;
    for (line, at) in [(&b"cd ../.."[..], &b"/second"[..]), (b"cd /..", b"/second")] {
        let said = run_line(nav, line);
        if !pwd_is(nav, at) {
            v |= tt::MOVED_ON_REFUSAL;
        } else if matches!(said, Some(Say::Refused(Refused::AtYourRoot))) {
            clamped += 1;
        } else {
            v |= tt::CLIMBED_OUT;
        }
    }
    if clamped == 2 {
        v |= tt::ROOT_CLAMPED;
    }
    v
}

/// [`two_trees`]' per-command grants, planned and opened under the mount. Its own frame because
/// each planned `Endowment` is a kilobyte or more in a debug build.
fn two_tree_grants(nav: &mut Nav) -> u64 {
    use filesystem_fixture::{tree, twotrees as tt};
    let mut v = 0u64;

    if let Ok(g) = grant_plan::redirect_target(b"/second/secret", holdings(nav), false)
        && g.which == nav::Which::B
        && let Ok(f) = open_source(nav, g)
    {
        let want = tree::SECRET_BODY;
        let got = call(f.dir, fs::req(fs::READ, f.handle, want.len() as u64), 0).0 as i64;
        let mut buf = [0u8; 64];
        let n = (got.max(0) as usize).min(buf.len());
        get_page(n, &mut buf);
        if &buf[..n] == want {
            v |= tt::REDIRECTED_FROM_MOUNT;
        }
        call(f.dir, fs::req(fs::CLOSE, f.handle, 0), 0);
    }
    // And the same name outside the mount opens nothing, because the first tree has no `secret`.
    if let Ok(g) = grant_plan::redirect_target(b"/secret", holdings(nav), false)
        && let Ok(f) = open_source(nav, g)
    {
        v |= tt::REACHED_ACROSS;
        call(f.dir, fs::req(fs::CLOSE, f.handle, 0), 0);
    }
    v
}

/// [`two_trees`]' `rm` step, alone in its frame and called from [`two_trees`] itself, because the
/// planner under it is the deepest chain this witness reaches and nothing else should sit under it.
fn two_tree_rm(nav: &mut Nav) -> u64 {
    use filesystem_fixture::twotrees as tt;
    if let Command::Run(rspec) = grant_plan::parse(b"rm /second/secret")
        && let Ok(e) =
            grant_plan::plan(&rspec, holdings(nav), grant_plan::expand::Expansion::none())
        && let Some(g) = e.directory
        && g.which == nav::Which::B
        && dir_grant(&g, e.short_options).is_err()
    {
        return tt::RM_IN_MOUNT_REFUSED;
    }
    0
}

/// Whether `ls <token>` succeeded and named `present` and not `absent`. A listing is a rendering of
/// authority, so the absent name is as much the claim as the present one.
fn listing_is(nav: &mut Nav, token: &[u8], present: &[u8], absent: &[u8]) -> bool {
    let (mut saw, mut stranger) = (false, false);
    let said = nav.ls(token, &mut |name, _| {
        saw |= name == present;
        stranger |= name == absent;
    });
    said == Say::Nothing && saw && !stranger
}

// ---- the globbing witness (milestone 47's globbing lane) ----

/// A small collector: what `echo` printed, or what a grant rendered to. Fixed size, and generous
/// enough that a truncation cannot make two renderings agree by both running out at one place.
struct Text {
    buf: [u8; 96],
    n: usize,
}

impl Text {
    fn new() -> Self {
        Text { buf: [0; 96], n: 0 }
    }

    fn push(&mut self, bytes: &[u8]) {
        for &b in bytes {
            if self.n < self.buf.len() {
                self.buf[self.n] = b;
                self.n += 1;
            }
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.n]
    }

    /// Whether a name appears in what was collected. Used only for the stranger check, where the
    /// names are distinctive enough that a substring is the right question.
    fn holds(&self, needle: &[u8]) -> bool {
        self.as_bytes().windows(needle.len()).any(|w| w == needle)
    }
}

/// **The demonstration: `echo *.txt` prints exactly the authority `rm *.txt` would transfer.**
///
/// Both halves share the expander and not the plumbing, over a real `READDIR`, so a planner that
/// narrowed, reordered or added to a set shows up as a disagreement. Three refusals keep the
/// agreement from being vacuous. See `notes/swish-language/witness.md`, "The globbing script".
pub(super) fn globbing(spec: u64) -> ! {
    use filesystem_fixture::{VERDICT, globscape as gb, tree};
    let mut nav = Nav::rooted(filesystem_contract::grant::spec_rights(spec));
    let mut v = 0u64;

    // 1. What `echo` shows.
    let mut shown = Text::new();
    let said = echo(&mut nav, tree::GLOB_PATTERN, &mut |b| shown.push(b));
    if matches!(said, Say::Nothing) && !shown.as_bytes().is_empty() {
        v |= gb::EXPANDED;
    }
    // And it is not simply everything in the directory. Both strangers are one entry away and the
    // caretaker a hop up could name either.
    if !shown.holds(tree::GLOB_MISS.as_bytes()) && !shown.holds(tree::GLOB_DIR.as_bytes()) {
        v |= gb::EXCLUDED_A_STRANGER;
    }

    // 2. What the same pattern would transfer, planned as `rm <pattern>` from the same text.
    let mut cmd = [0u8; 32];
    cmd[..3].copy_from_slice(b"rm ");
    cmd[3..3 + tree::GLOB_PATTERN.len()].copy_from_slice(tree::GLOB_PATTERN);
    let line = &cmd[..3 + tree::GLOB_PATTERN.len()];
    let mut granted = Text::new();
    if let Command::Run(rspec) = grant_plan::parse(line) {
        match expansion(&mut nav, &rspec) {
            Ok(e) => match grant_plan::plan(&rspec, holdings(&nav), e) {
                Ok(endow) => match endow.directory {
                    Some(g) => swish::write_set(&g.names, &mut |b| granted.push(b)),
                    None => v |= gb::GLOB_FAILED,
                },
                Err(_) => v |= gb::GLOB_FAILED,
            },
            Err(_) => v |= gb::GLOB_FAILED,
        }
    } else {
        v |= gb::GLOB_FAILED;
    }
    if !granted.as_bytes().is_empty() && granted.as_bytes() == shown.as_bytes() {
        v |= gb::AGREED;
    }

    // 3. A pattern that matches nothing. Refused, not printed as itself: the expansion is the grant,
    //    so an empty expansion is an empty grant.
    let mut nothing = Text::new();
    if matches!(
        echo(&mut nav, tree::GLOB_NOMATCH, &mut |b| nothing.push(b)),
        Say::Cannot(Refusal::NoMatch)
    ) && nothing.as_bytes().is_empty()
    {
        v |= gb::NO_MATCH_REFUSED;
    }

    // 4. A pattern in a leading component, refused where it is read.
    if let Command::Run(bad) = grant_plan::parse(b"rm */gone")
        && matches!(
            expansion(&mut nav, &bad),
            Err(Say::Cannot(Refusal::PatternInPath))
        )
    {
        v |= gb::PATTERN_IN_PATH_REFUSED;
    }

    // 5. And a line with no pattern in it is still text: byte for byte, spacing included.
    let mut plain = Text::new();
    if matches!(
        echo(&mut nav, tree::GLOB_PLAIN, &mut |b| plain.push(b)),
        Say::Nothing
    ) && plain.as_bytes() == tree::GLOB_PLAIN
    {
        v |= gb::TEXT_UNTOUCHED;
    }

    send(REPORT, VERDICT, v, 0);
    exit();
}

/// Run one command line through the builtins, discarding any listing.
fn run_line(nav: &mut Nav, cmd: &[u8]) -> Option<Say> {
    builtin(nav, cmd, &mut |_, _| {})
}

/// Whether `pwd` would print exactly this.
fn pwd_is(nav: &Nav, want: &[u8]) -> bool {
    let mut buf = [0u8; nav::RENDER_MAX];
    let n = nav.holds.cwd.render(&mut buf);
    &buf[..n] == want
}

/// `OPEN` a name where we stand; the handle, or `None` if it did not resolve.
fn opened(nav: &Nav, name: &[u8]) -> Option<u64> {
    let r = nav.name_call(fs::OPEN, nav.here(), name, 0);
    if r < 0 { None } else { Some(r as u64) }
}

/// `OPEN` what a **token** designates, resolved the way the prompt resolves one: from this shell's
/// root when the token starts with `/`, from where it stands otherwise.
///
/// It goes through [`Nav::plan_path`] and [`Nav::walk_steps`] rather than sending a name straight
/// at [`Nav::here`], because the claim being witnessed is about the shell's own resolver. A helper
/// that walked the path itself would prove that the *witness* can resolve a path.
fn opened_token(nav: &mut Nav, token: &[u8]) -> Option<(Tree, u64)> {
    let (p, _) = nav.plan_path(token).ok()?;
    let (lead, name) = p.split_last_component()?;
    let w = nav.walk_steps(p.is_from_root(), lead).ok()?;
    let r = nav.name_call_in(w.tree, fs::OPEN, w.handle, name, 0);
    nav.unwind(&w);
    if r < 0 {
        None
    } else {
        Some((w.tree, r as u64))
    }
}

/// `CREATE` a name where we stand, keeping the handle. `touch` (below) does the same call and
/// closes it straight back; this one is for the steps that need to write through what they made,
/// which `touch`'s contract has no reason to expose.
fn created(nav: &Nav, name: &[u8]) -> Option<u64> {
    let r = nav.name_call(fs::CREATE, nav.here(), name, 0);
    if r < 0 { None } else { Some(r as u64) }
}

/// `GETMTIME` a name where we stand (milestone 47's mtime lane, DECISIONS §112). `None` on any
/// error, which the mtime probes below treat as "prove nothing" rather than guess a value.
fn mtime_of(nav: &Nav, name: &[u8]) -> Option<u64> {
    let r = nav.name_call(fs::GETMTIME, nav.here(), name, 0);
    if r < 0 { None } else { Some(r as u64) }
}

/// `OPENDIR` a name where we stand, asking for exactly the rights this shell holds; the handle, or
/// `None`. Used to reach *into* a directory the witness made, which is one step of the walk
/// `components/src/rm.rs` does for a living.
fn open_dir(nav: &Nav, name: &[u8]) -> Option<u64> {
    let r = nav.name_call(fs::OPENDIR, nav.here(), name, nav.rights);
    if r < 0 { None } else { Some(r as u64) }
}

/// **`touch`'s two-fold contract, run and checked in one place.** First on a name that is not
/// there: the create half. Then, once it holds a body, a second `touch` on the same name: the
/// no-op half. A `touch` that truncated what it found would satisfy the first check and fail this
/// one, which is why the two are chained with early returns rather than nested as one question.
fn touch_twice(nav: &mut Nav, touched: &([u8; 16], usize), cmd: &mut [u8; 32], v: &mut u64) {
    use filesystem_fixture::{navscape as nb, tree};

    let Some(Say::Nothing) = run_line(nav, line(cmd, b"touch ", touched)) else {
        return;
    };
    let Some(h) = opened(nav, name_of(touched)) else {
        return;
    };
    *v |= nb::TOUCH_CREATED;
    put_page(tree::NAV_TOUCH_BODY);
    let w = call(
        DIR,
        fs::req(fs::WRITE, h, tree::NAV_TOUCH_BODY.len() as u64),
        0,
    )
    .0 as i64;
    nav.close(h);
    if w != tree::NAV_TOUCH_BODY.len() as i64 {
        return;
    }
    let Some(Say::Nothing) = run_line(nav, line(cmd, b"touch ", touched)) else {
        return;
    };
    let Some(h2) = opened(nav, name_of(touched)) else {
        return;
    };
    let n = call(
        DIR,
        fs::req(fs::READ, h2, tree::NAV_TOUCH_BODY.len() as u64),
        0,
    )
    .0 as i64;
    if n == tree::NAV_TOUCH_BODY.len() as i64 {
        let mut buf = [0u8; 64];
        get_page(n as usize, &mut buf);
        if &buf[..n as usize] == tree::NAV_TOUCH_BODY {
            *v |= nb::TOUCH_PRESERVED;
        }
    }
    nav.close(h2);
}

/// **`touch`'s mtime half, typed as real command lines and read back through the wire**
/// (the mtime lane of milestone 47 (navigation and naming), DECISIONS §112 (`touch`'s two
/// behaviors need two rights)). Runs after [`touch_twice`] and reuses `touched`, which by then
/// names a file with a body: a bare `touch`, then `touch -t`, each checked against a `GETMTIME`
/// read. Chained, because `TOUCH_AT_ROUND_TRIPPED` is checked against the mtime
/// `TOUCH_MTIME_ADVANCED` established.
fn touch_mtime_probes(nav: &mut Nav, touched: &([u8; 16], usize), v: &mut u64) {
    use filesystem_fixture::{navscape as nb, tree};

    let Some(before) = mtime_of(nav, name_of(touched)) else {
        return;
    };

    // Bare `touch`, real command line, real dispatch through `Nav::touch` to `fs::SETMTIME`.
    let mut cmd = [0u8; 32];
    let Some(Say::Nothing) = run_line(nav, line(&mut cmd, b"touch ", touched)) else {
        return;
    };
    let Some(after_bare) = mtime_of(nav, name_of(touched)) else {
        return;
    };
    if after_bare <= before {
        return;
    }
    *v |= nb::TOUCH_MTIME_ADVANCED;

    // `touch -t <RFC 3339>`, a real command line the parser has to split into three tokens
    // (`touch`, `-t`, the instant, the name) and dispatch to `fs::SETMTIME_AT` with the seconds
    // value `calendar` decoded from it. The buffer is sized for exactly this one line: "touch -t "
    // (9) + the RFC 3339 text (20) + " " (1) + the name (at most 16, `run_name`'s own bound).
    const AT_PREFIX: &[u8] = b"touch -t ";
    let mut at_cmd = [0u8; 9 + 20 + 1 + 16];
    let mut n = 0;
    at_cmd[n..n + AT_PREFIX.len()].copy_from_slice(AT_PREFIX);
    n += AT_PREFIX.len();
    let rfc = tree::NAV_TOUCH_AT_RFC3339.as_bytes();
    at_cmd[n..n + rfc.len()].copy_from_slice(rfc);
    n += rfc.len();
    at_cmd[n] = b' ';
    n += 1;
    at_cmd[n..n + touched.1].copy_from_slice(name_of(touched));
    n += touched.1;

    let Some(Say::Nothing) = run_line(nav, &at_cmd[..n]) else {
        return;
    };
    if let Some(after_at) = mtime_of(nav, name_of(touched))
        && after_at == tree::NAV_TOUCH_AT_UNIX
        && after_at != after_bare
    {
        *v |= nb::TOUCH_AT_ROUND_TRIPPED;
    }
}

/// **`WRITE` is enough for bare `touch`; it is not enough for `touch -t`, through the real wire**
/// (milestone 47's mtime lane, DECISIONS §112). Bypasses the `touch` builtin and the parser
/// entirely, unlike [`touch_mtime_probes`]: what this proves is that a `verb::TABLE` row or a
/// caretaker forwarding bug cannot smuggle the arbitrary-time authority past a handle that never
/// carried it, which a test that only ever grants `dir::ALL` (every other probe in this script)
/// cannot exercise. A raw `fs::MKDIR` mints a **fresh** directory attenuated to `WRITE` and not
/// `SETTIME`, one level below the shell's own root, and both mtime verbs are sent straight to it.
fn touch_mtime_without_settime(nav: &Nav, name: &[u8], v: &mut u64) {
    use filesystem_contract::dir;
    use filesystem_fixture::navscape as nb;

    let narrow_rights = dir::ALL & !dir::SETTIME;
    let r = nav.name_call(fs::MKDIR, nav.here(), name, narrow_rights);
    if r < 0 {
        return;
    }
    let dir_handle = r as u64;

    let inner = b"f";
    let created = nav.name_call(fs::CREATE, dir_handle, inner, 0);
    if created >= 0 {
        nav.close(created as u64);

        if nav.name_call(fs::SETMTIME, dir_handle, inner, 0) == 0 {
            *v |= nb::TOUCH_NOW_NEEDS_ONLY_WRITE;
        }
        if nav.name_call(
            fs::SETMTIME_AT,
            dir_handle,
            inner,
            filesystem_fixture::tree::NAV_TOUCH_AT_UNIX,
        ) < 0
        {
            *v |= nb::TOUCH_AT_REFUSED_WITHOUT_SETTIME;
        }
    }
    nav.close(dir_handle);
}

/// Send one removal (`UNLINK` or `RMDIR`) for a name where we stand, and say whether it worked.
///
/// The two verbs are one helper because the property they hold up is one property: `UNLINK` refuses
/// a directory, `RMDIR` refuses a non-empty one, and a witness that used a different path for each
/// could not compare them. What is *not* here is a loop: the recursion is the `rm` program's, in its
/// own address space, holding its own attenuated grant.
///
/// Name: provisional, flagged 2026-09-24 by the boolean-predicate pass
/// (design/naming/functions-that-answer-yes-or-no-worklist.md). It acts, so the Rust predicate rule calef
/// ratified 2026-09-24 exempts it, but it is a participle that reads as a question, on a function
/// that sends the unlink; recommended `remove`.
fn removed(nav: &Nav, verb: u64, name: &[u8]) -> bool {
    nav.name_call(verb, nav.here(), name, 0) == 0
}

/// A fixture name with the run index appended, so runs sharing one image do not collide on `EEXIST`
/// and read it as a refusal. Fixed-size, which is all it needs; written before the shell had a heap.
fn run_name(base: &str, run: u64) -> ([u8; 16], usize) {
    let mut out = [0u8; 16];
    let n = base.len().min(15);
    out[..n].copy_from_slice(&base.as_bytes()[..n]);
    out[n] = b'0' + (run % 10) as u8;
    (out, n + 1)
}

/// The bytes of a [`run_name`].
fn name_of(name: &([u8; 16], usize)) -> &[u8] {
    &name.0[..name.1]
}

/// Build one command line out of a verb and a generated name. The witness types real command lines,
/// so it has to be able to build them; the buffer is the caller's because this program has no
/// allocator and a static would be a needless piece of shared state.
fn line<'a>(buf: &'a mut [u8; 32], verb: &[u8], name: &([u8; 16], usize)) -> &'a [u8] {
    let n = verb.len();
    buf[..n].copy_from_slice(verb);
    buf[n..n + name.1].copy_from_slice(name_of(name));
    &buf[..n + name.1]
}

/// Build `bind <a>/<b> <c>` out of three generated names. `bind`'s own witness needs three
/// pieces where every other command here needs at most two ([`line()`]'s shape), so it gets its
/// own buffer rather than overloading that one.
fn bind_line<'a>(
    buf: &'a mut [u8; 64],
    a: &([u8; 16], usize),
    b: &([u8; 16], usize),
    c: &([u8; 16], usize),
) -> &'a [u8] {
    let mut n = 0;
    for part in [
        b"bind " as &[u8],
        name_of(a),
        b"/",
        name_of(b),
        b" ",
        name_of(c),
    ] {
        buf[n..n + part.len()].copy_from_slice(part);
        n += part.len();
    }
    &buf[..n]
}
