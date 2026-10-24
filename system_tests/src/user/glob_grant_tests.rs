use byte_sink_contract::eof;
use filesystem_contract::{dir, rm as rr};
use filesystem_fixture::{VERDICT, globscape as gb, tree};

use super::*;
use crate::sched;

/// The `swish` binary's globbing role (`components/src/swish.rs`).
const ROLE_GLOB: u64 = 2;

/// The most messages one `rm` run may send before the harness stops reading, as
/// [`rm_program_tests`] bounds it and for the same reason.
const MAX_MESSAGES: usize = 64;

/// The set `gl-*.txt` matches in the fixture, which `filesystem_contract`'s host test pins against the
/// matcher. Stated literally here because building a grant is the one thing the kernel can do
/// without an enumeration, and proved to *be* the expansion over there.
const MATCHED: [(&[u8], bool); 2] = [
    (tree::GLOB_ONE.as_bytes(), false),
    (tree::GLOB_TWO.as_bytes(), false),
];

/// What a shell shows for `gl-*.txt` is what a set grant takes away: two phases in one test.
///
/// The shell expands the pattern over the fixture as staged, then the second phase removes what
/// it matched; the other order would agree about the empty set. Two `#[test_case]`s would make
/// that order a property of the harness, the order-coupled fixture of DECISIONS §27.
#[test_case]
fn what_a_shell_shows_is_what_a_set_grant_takes_away() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(shown) = shell_expanded() else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_shell_agreed(shown);
    a_set_capability_cannot_name_a_stranger();
    the_set_grant_removes_exactly_the_match();
}

/// Phase one: a shell rooted in `globset` runs `echo gl-*.txt` and plans `rm gl-*.txt`, and
/// reports whether the two produced the same names.
///
/// The rights it is granted are what expansion actually costs: `ENUMERATE` to list the
/// directory, `DESCEND` because the shell walks a path with `OPENDIR`, and `READ` so a listing
/// is not the only thing it could ever do. There is no `REMOVE` in it at all, which is the point
/// of `echo` being the half that demonstrates this: **showing the authority costs none of it.**
fn shell_expanded() -> Option<u64> {
    let (report, held) = fs_service::start_granted_dir(
        dir_capability_tests::blk_server_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("fs_subtree_caretaker")
            .expect("no fs_subtree_caretaker program in the initial ramdisk"),
        program("swish").expect("no swish program in the initial ramdisk"),
        fs_service::DirGrant {
            name: tree::GLOBSET,
            rights: dir::ENUMERATE | dir::DESCEND | dir::READ,
            role: ROLE_GLOB,
            // The shell is told what its capability carries, for notes/shell-navigation.md's
            // reason: nothing on this wire reports what a handle holds.
            arg: filesystem_contract::grant::spec(0, dir::ENUMERATE | dir::DESCEND | dir::READ),
            arg2: 0,
            // A measurement: six pages ran within a few dozen bytes of the guard once `Holdings`
            // learned a second tree's mount point. See notes/two-trees.md, "The globbing
            // witness's stack, measured", and its BUGS for the smaller frame that should replace
            // the seventh page.
            stack_pages: 7,
        },
    )?;
    let [tag, verdict, ..] = sched::ipc_receive(report);
    held.release_or_fail("a granted shell");
    assert_eq!(tag, VERDICT, "the shell's report is not a verdict word");
    Some(verdict)
}

fn assert_shell_agreed(v: u64) {
    assert_eq!(
        v & gb::GLOB_FAILED,
        0,
        "the shell could not plan the grant at all, so nothing it reported means anything",
    );
    let want = gb::EXPANDED
        | gb::AGREED
        | gb::EXCLUDED_A_STRANGER
        | gb::NO_MATCH_REFUSED
        | gb::PATTERN_IN_PATH_REFUSED
        | gb::TEXT_UNTOUCHED;
    assert_eq!(
        v, want,
        "the globbing witness reported {v:#x}, wanted {want:#x}",
    );
}

/// Run `rm` once behind a `fs_nameset_caretaker` holding [`MATCHED`] inside `globset`.
///
/// `name` empty means [`filesystem_contract::grant::WHOLE_NAMESPACE`]: the operand is the set, and `rm`
/// learns it by enumerating the capability it was handed.
fn run_rm(name: &str, flags: u64) -> (u64, u64) {
    let (lo, hi) = filesystem_contract::grant::pack_name(name.as_bytes());
    let (report, held) = fs_service::start_granted_set(
        dir_capability_tests::blk_server_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("fs_nameset_caretaker")
            .expect("no fs_nameset_caretaker program in the initial ramdisk"),
        program("rm").expect("no rm program in the initial ramdisk"),
        fs_service::SetGrant {
            dir: tree::GLOBSET,
            names: &MATCHED,
            // `REMOVE` and nothing else: `rm *.txt` may not read, write, create beside or walk
            // under its names. Listing the set needs no right; the caretaker answers it from the
            // grant.
            rights: dir::REMOVE,
            role: filesystem_contract::grant::spec(name.len(), flags),
            arg: lo,
            arg2: hi,
            stack_pages: 6,
        },
    )
    .expect("the FS service was wired by the shell phase");

    for _ in 0..MAX_MESSAGES {
        let [w0, w1, w2, _, _] = sched::ipc_receive(report);
        // `rm` ends with the sink contract's end of stream, the verdict in the two words
        // `OPERATION_EOF` leaves free. The shell phase reports a `VERDICT` bitmap instead.
        if w0 == eof() {
            held.release_or_fail("rm's set grant");
            return (w1, w2);
        }
        assert!(w0 <= 16, "neither a verdict nor a text frame: {w0:#x}");
    }
    panic!("rm sent {MAX_MESSAGES} messages and never a verdict");
}

/// **The attacker, and it is `rm` itself.** Told to remove a name that exists, sits one
/// directory entry away from the two it was granted, and that the caretaker one hop up could
/// remove on any request it liked.
///
/// It gets `ENOENT`: in this scope there is no such name. Nothing consulted a permission, and
/// nothing in `rm` decided not to try, which is what makes this a fact about the capability.
fn a_set_capability_cannot_name_a_stranger() {
    let (status, removed) = run_rm(tree::GLOB_MISS, 0);
    assert_eq!(
        status,
        rr::status(2), // ENOENT
        "a name outside the set must not be nameable through a set capability",
    );
    assert_eq!(removed, 0);
}

/// **And the grant works**, which is what stops the refusal above being equally true of a
/// capability that reaches nothing. Two names in, two names removed, and the two the pattern did
/// not match are still on the disk, asserted from the host by
/// `xtask::redoxfs_glob_grant_took_exactly_the_match`.
fn the_set_grant_removes_exactly_the_match() {
    let (status, removed) = run_rm("", 0);
    assert_eq!(status, rr::OK, "the set grant reported a failure");
    assert_eq!(
        removed,
        MATCHED.len() as u64,
        "a set grant must remove every name in it, and only those",
    );
}
