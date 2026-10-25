//! **The entry point and the prompt**: the roles this program is spawned in, `_start`'s dispatch
//! over them, and the interactive loop.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// **The navigating witness** (milestone 47): no terminal, a directory capability at slot 0, and a
/// report rendezvous at slot 1. It runs the same builtins this file's prompt runs and reports a
/// bitmap; see [`navigate`].
pub(super) const ROLE_NAVIGATE: u64 = 1;

/// **The globbing witness** (milestone 47's globbing lane): the same wiring as [`ROLE_NAVIGATE`],
/// pointed at the fixture's `globset`, running `echo` and the grant planner over one pattern and
/// reporting whether they agree. See [`globbing`].
pub(super) const ROLE_GLOB: u64 = 2;

/// **The pipeline witness** (milestone 50): the interactive wiring minus the keyboard. It holds a
/// terminal it writes to, a spawn channel, a result channel and a budget, and it types a fixed
/// script of command lines through the same [`dispatch`] the prompt uses. See [`piping`].
pub(super) const ROLE_PIPELINE: u64 = 3;

/// **The redirection witness** (milestone 50): [`ROLE_PIPELINE`]'s wiring plus a directory
/// capability at [`DIR_TERMINAL`], which is the endowment `>` and `<` need and the one the
/// interactive boot grants. It types a fixed script through the same [`dispatch`] the prompt uses.
/// See [`redirecting`].
pub(super) const ROLE_REDIRECT: u64 = 4;

/// **The timing witness** (milestone 86): [`ROLE_PIPELINE`]'s wiring plus, in the wiring that has
/// one, a clock page at the slot `x2` names. It types a fixed script through the same [`dispatch`]
/// the prompt uses, and the same script is run three times against three clock states, which is what
/// makes the refusals assertions rather than unreachable branches. See [`timing`].
pub(super) const ROLE_TIMING: u64 = 5;

/// **The two-tree witness** (milestone 154): no terminal, grant `a` at slot 0, grant `b` at slot 1
/// and a report rendezvous at slot 2, which is `fs_service::start_granted_two_dirs`' wiring. It
/// runs the prompt's own builtins across both trees and reports a bitmap; see [`two_trees`].
/// Provisional name.
pub(super) const ROLE_TWO_TREES: u64 = 6;

user_mode_runtime::program_entry!(start);

/// **What `arg1` carries into every role that holds a directory**: the `filesystem_contract::dir`
/// rights that capability was granted, 0 for none. Told rather than asked, because nothing on that
/// wire reports what a handle carries and `OPENDIR` refuses a request wider than the parent, so a
/// guess of `dir::ALL` could not `cd` (notes/shell-navigation.md). **`x2` carries the clock slot**
/// (milestone 86 (`time`: the shell times a command)), or [`NO_CLOCK`]: the slot moves with what
/// else the boot granted, and a probe of the wrong number would map some other object as a clock.
/// See [`CLOCK_SLOT`].
fn start(role: u64, arg: u64, clock: u64) -> ! {
    CLOCK_SLOT.store(clock, core::sync::atomic::Ordering::Relaxed);
    // Probed here and nowhere later: see [`HOLDS_RUN_UNVOUCHED`].
    HOLDS_RUN_UNVOUCHED.store(
        user_mode_runtime::is_granted(spawnproto::RUN_UNVOUCHED_SLOT),
        core::sync::atomic::Ordering::Relaxed,
    );
    HOLDS_CONFIG.store(
        user_mode_runtime::is_granted(grant_plan::SHELL_CONFIGURATION_SLOT),
        core::sync::atomic::Ordering::Relaxed,
    );
    HOLDS_MOUNTED_TREE.store(
        user_mode_runtime::is_granted(spawnproto::MOUNTED_TREE_SLOT),
        core::sync::atomic::Ordering::Relaxed,
    );
    // After the probes, which must see the table as its builder left it. The two witness roles
    // hold no budget at slot 3.
    if !matches!(role, ROLE_NAVIGATE | ROLE_GLOB) {
        heap_init();
    }
    // Probed here too: see [`HOLDS_MACHINE_PAGE`].
    HOLDS_MACHINE_PAGE.store(
        user_mode_runtime::is_granted(spawnproto::MACHINE_PAGE_SLOT),
        core::sync::atomic::Ordering::Relaxed,
    );
    match role {
        ROLE_NAVIGATE => navigate(arg),
        ROLE_GLOB => globbing(arg),
        ROLE_PIPELINE => piping(),
        ROLE_REDIRECT => redirecting(arg),
        ROLE_TIMING => timing(),
        ROLE_TWO_TREES => two_trees(arg),
        _ => interactive(arg),
    }
}

/// The interactive prompt, role 0: a terminal at slot 0, a spawn channel, a result channel and a
/// budget. `crates/system_initializer` starts this program with `(0, fs_rights, clock_slot)`, and
/// any unrecognized role falls through here, because this program's failure mode should be a
/// prompt. `rights` is the [`_start`] convention: the `filesystem_contract::dir` rights of the
/// directory capability at [`DIR_TERMINAL`], or 0 for a boot that wired no filesystem.
pub(super) fn interactive(rights: u64) -> ! {
    // **The line that says userspace is up**, and it is the top rung of milestone 268's boot
    // ladder: a default boot ends at this prompt rather than at a halt, so this is the sentence a
    // watcher reads to learn that the boot finished. Its head comes from `crates/boot_ladder`
    // rather than from a literal here, because `crates/serial_console` matches it and AGENTS.md
    // rule 7 says two binaries agreeing on a string means a crate.
    print(boot_ladder::PROMPT.as_bytes());
    print(b". naming a resource in a command IS granting it.\n");
    print(b"commands: help, echo <text>, caps [command], time <command>, xargs <command>,\n");
    print(b"          cd, pwd, ls, mkdir, touch, apropos <word>, rm, wc, mdr, <prog> [--mem N] [arg]\n");
    print(b"          and the operators  >  >>  <  |\n");
    print(b"          'quote a whole word'   and   ;  &&  ||   with  echo $?  for the status\n");

    // Whether the navigation builtins and the redirection operators have anything to name is
    // decided here, by one capability. A boot that wired no FS service starts this program with
    // `rights = 0` and every one of them answers "this shell holds no directory capability".
    let mut nav = if rights == 0 {
        Nav::empty()
    } else {
        Nav::rooted_at(DIR_TERMINAL, rights)
    };
    // **The second mounted tree, when the progenitor placed one** (milestone 834 (ioping on nife and Linux)): the disk tree,
    // at `/disk`. It shares the first tree's page, so it needs nothing mapped of its own.
    if rights != 0 && HOLDS_MOUNTED_TREE.load(core::sync::atomic::Ordering::Relaxed) {
        let mut mount = nav::Cwd::root();
        mount.descend(spawnproto::MOUNTED_TREE_NAME);
        nav.holds.mounted = grant_plan::MountedTree::mounted_at(mount);
        nav.second = Some(Tree {
            slot: spawnproto::MOUNTED_TREE_SLOT,
            rights: spawnproto::MOUNTED_TREE_RIGHTS,
        });
    }
    let mut line = [0u8; 128];
    loop {
        let (n, flags) = edit_line(&mut nav, b"$ ", &mut line);
        if flags & proto::FLAG_INTERRUPTED != 0 {
            // ^C at the prompt: the terminal discarded the line. Account for this interrupt so it
            // does not leak into the next job's watch, then come back for the next line.
            CONSUMED.store(intr_count(), core::sync::atomic::Ordering::Relaxed);
            continue;
        }
        if flags & proto::FLAG_EOF != 0 {
            // ^D on an empty line. This shell is the session; there is nowhere to exit to.
            print(b"  (end of input; this shell has nowhere to exit to)\n");
            continue;
        }
        dispatch_line(&mut nav, &line[..n]);
    }
}
