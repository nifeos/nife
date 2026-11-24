use byte_sink_contract::end_of_stream;
use file_system_contract::directory;
use file_system_fixture::{VERDICT, globbing, tree};
use grant_plan::rm_report as rr;

use super::*;
use crate::sched;

/// The `swish` binary's globbing role (`components/src/swish.rs`).
const ROLE_GLOB: u64 = 2;

/// The most messages one `rm` run may send before the harness stops reading, as
/// [`rm_program_tests`] bounds it and for the same reason.
const MAXIMUM_MESSAGES: usize = 64;

/// The set `gl-*.txt` matches in the fixture, which `file_system_contract`'s host test pins against the
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
/// that order a property of the harness, the order-coupled fixture of DECISIONS §27 (the
/// filesystem service).
#[test_case]
fn what_a_shell_shows_is_what_a_set_grant_takes_away() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(shown) = shell_expanded() else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_shell_agreed(shown);
    a_name_in_the_start_words_is_refused();
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
    let (report, _, held) = fs_service::start_granted_dir(
        dir_capability_tests::blk_server_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("fs_subtree_caretaker")
            .expect("no fs_subtree_caretaker program in the initial ramdisk"),
        program("swish").expect("no swish program in the initial ramdisk"),
        fs_service::DirGrant {
            name: tree::GLOBSET,
            rights: directory::rights::ENUMERATE
                | directory::rights::DESCEND
                | directory::rights::READ,
            role: ROLE_GLOB,
            // The shell is told what its capability carries, for partsfoundry/record:notes/shell-navigation.md's
            // reason: nothing on this wire reports what a handle holds.
            arg: file_system_contract::grant::specification(
                0,
                directory::rights::ENUMERATE | directory::rights::DESCEND | directory::rights::READ,
            ),
            arg2: 0,
            // In a debug build the deepest chain from `_start` through `globbing` and the planner
            // is about 44.3 KiB, the frame sizes summed; `globbing`'s own frame is 21 KiB, most
            // of it 512-byte directory paths held by value. Twelve pages leave about 4.8 KiB.
            // See partsfoundry/record:notes/two-trees.md, "The globbing witness's stack, measured", and its BUGS for
            // the smaller frame that should replace the extra pages.
            stack_pages: 11,
            verdict: false,
        },
    )?;
    let [tag, verdict, ..] = sched::ipc_receive(report);
    held.release_or_fail("a granted shell");
    assert_eq!(tag, VERDICT, "the shell's report is not a verdict word");
    Some(verdict)
}

fn assert_shell_agreed(v: u64) {
    assert_eq!(
        v & globbing::GLOB_FAILED,
        0,
        "the shell could not plan the grant at all, so nothing it reported means anything",
    );
    let want = globbing::EXPANDED
        | globbing::AGREED
        | globbing::EXCLUDED_A_STRANGER
        | globbing::NO_MATCH_REFUSED
        | globbing::PATTERN_IN_PATH_REFUSED
        | globbing::TEXT_UNTOUCHED;
    assert_eq!(
        v, want,
        "the globbing witness reported {v:#x}, wanted {want:#x}",
    );
}

/// Run `rm` once behind a `fs_nameset_caretaker` holding [`MATCHED`] inside `globset`, started with
/// `flags` and the two name words `name_lo` and `name_hi`.
///
/// Every wiring sends zero name words ([`file_system_contract::grant::WHOLE_NAMESPACE`]): the
/// operand is the set, and `rm` learns it by enumerating the capability it was handed.
fn run_rm(name_lo: u64, name_hi: u64, flags: u64) -> (u64, u64) {
    let (report, verdict, held) = fs_service::start_granted_set(
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
            rights: directory::rights::REMOVE,
            role: file_system_contract::grant::specification(0, flags),
            arg: name_lo,
            arg2: name_hi,
            stack_pages: 6,
            verdict: true,
        },
    )
    .expect("the FS service was wired by the shell phase");
    let verdict = verdict.expect("a grant asked for a verdict endpoint hands one back");

    for _ in 0..MAXIMUM_MESSAGES {
        let [w0, w1, w2, _, _] = sched::ipc_receive(report);
        // `rm` ends its report with the sink contract's end of stream, its reserved words zero,
        // and then sends the verdict on its own endpoint (`grant_plan::rm_report`). The shell
        // phase reports a `VERDICT` bitmap instead.
        if w0 == end_of_stream() {
            assert_eq!(
                (w1, w2),
                (0, 0),
                "an end of stream's reserved words are zero"
            );
            let [status, removed, _, _, _] = sched::ipc_receive(verdict);
            held.release_or_fail("rm's set grant");
            return (status, removed);
        }
        assert!(
            w0 <= 16,
            "neither an end of stream nor a text frame: {w0:#x}"
        );
    }
    panic!("rm sent {MAXIMUM_MESSAGES} messages and never ended its report");
}

/// **`rm` refuses START words that still carry a name**, restrictive first: its operands are the
/// set it was granted, and a wiring that also wrote a name in the words is wrong, so it is
/// `EINVAL` and nothing is removed. That a set capability cannot name a stranger is
/// `a_name_set_capability_reads_its_attributes_and_still_names_only_its_set`'s witness.
fn a_name_in_the_start_words_is_refused() {
    let (status, removed) = run_rm(u64::from_le_bytes(*b"gl-three"), 0, 0);
    assert_eq!(
        status,
        rr::status(22), // EINVAL
        "rm must refuse name words rather than read a second operand from them",
    );
    assert_eq!(removed, 0);
}

/// **And the grant works**, which is what stops the refusal above being equally true of a
/// capability that reaches nothing. Two names in, two names removed, and the two the pattern did
/// not match are still on the disk, asserted from the host by
/// `xtask::redoxfs_glob_grant_took_exactly_the_match`.
fn the_set_grant_removes_exactly_the_match() {
    let (status, removed) = run_rm(0, 0, 0);
    assert_eq!(status, rr::OK, "the set grant reported a failure");
    assert_eq!(
        removed,
        MATCHED.len() as u64,
        "a set grant must remove every name in it, and only those",
    );
}
