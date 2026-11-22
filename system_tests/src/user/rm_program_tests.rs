use file_system_contract::{directory, rm as rr};
use file_system_fixture::tree;
use grant_plan::rm_options;

use super::*;
use crate::sched;

/// The most messages one run may send before the harness stops reading. A `rm` that never ended
/// its report would otherwise hang the boot; this turns it into a failed assertion.
const MAXIMUM_MESSAGES: usize = 64;

/// What one run of the program reported.
struct Outcome {
    /// Its exit status: 0, or the errno of the first failure (`file_system_contract::rm`).
    status: u64,
    /// How many names it removed.
    removed: u64,
    /// **How many lines of text it printed before its report ended.** Zero is the interesting value:
    /// `rm(1)` is silent on success, and a `SEND` blocks until somebody receives it, so a line
    /// that was emitted cannot be missed by looking after the fact.
    printed: usize,
}

/// Run `rm` once inside a `fs_nameset_caretaker` holding `name` in [`tree::RMTREE`] with exactly
/// `rights`, with `flags`: the grant the shell plans for `rm <name>`.
///
/// `None` when no RedoxFS disk is attached (nothing to test; do not fail).
fn run_rm(rights: u64, name: &str, flags: u64) -> Option<Outcome> {
    let (report, verdict, held) = fs_service::start_granted_set(
        dir_capability_tests::blk_server_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("fs_nameset_caretaker")
            .expect("no fs_nameset_caretaker program in the initial ramdisk"),
        program("rm").expect("no rm program in the initial ramdisk"),
        fs_service::SetGrant {
            dir: tree::RMTREE,
            // The type is what a listing reports, and `rm` asks the filesystem what each name is
            // rather than trusting it, so a missing name can be listed as a file.
            names: &[(name.as_bytes(), false)],
            rights,
            // `rm` is started with a **grant's** three words rather than a role and a number:
            // the spec (the options where a caretaker's rights ride), then two name words, zero
            // because the operand is the set.
            role: file_system_contract::grant::specification(0, flags),
            arg: 0,
            arg2: 0,
            // Measured the way `spawn_fs_client` asks for: the recursion is real stack, each
            // level holding a listing buffer by value, and this program has no allocator.
            stack_pages: 6,
            verdict: true,
        },
    )?;
    let verdict = verdict.expect("a grant asked for a verdict endpoint hands one back");

    // `printed` is the enumeration index rather than a counter, and the two coincide exactly:
    // the end-of-stream arm returns *before* the increment, so on the iteration that sees it, the
    // index is the number of text frames that arrived first. That is the quantity being
    // reported, so this is not merely appeasing the lint.
    for (printed, _) in (0..MAXIMUM_MESSAGES).enumerate() {
        let [w0, w1, w2, _, _] = sched::ipc_receive(report);
        // `rm` speaks the sink contract it declares, so the report is decoded the way any sink
        // decodes it: an end of stream with a non-zero reserved word is malformed, not an end.
        match byte_sink_contract::unpack(
            byte_sink_contract::MessageWords::new(w0, w1, w2),
            &mut [0u8; byte_sink_contract::MAXIMUM_INLINE_BYTES],
        ) {
            byte_sink_contract::Message::Bytes(_) => {}
            byte_sink_contract::Message::EndOfStream => {
                // The verdict comes after the end, on its own endpoint (`file_system_contract::rm`).
                // Receiving it only now is what keeps it from overtaking the text.
                let [status, removed, zero, _, _] = sched::ipc_receive(verdict);
                assert_eq!(zero, 0, "rm's verdict has a third word, and it is zero");
                held.release_or_fail("rm's directory grant");
                return Some(Outcome {
                    status,
                    removed,
                    printed,
                });
            }
            byte_sink_contract::Message::Malformed => {
                panic!("rm sent a message the sink contract refuses: {w0:#x} {w1:#x} {w2:#x}")
            }
        }
    }
    panic!("rm sent {MAXIMUM_MESSAGES} messages and never ended its report");
}

/// `rm -r` removes the subtree it was granted, and a narrower capability cannot begin.
///
/// Three runs against one tree, in order, with the same binary and operand. `rm rm-doomed` with
/// the full grant is `EISDIR` and removes nothing: a directory is a refusal, never a silent
/// escalation to recursion, as in `rm(1)`. `rm -r rm-doomed` through a capability carrying only
/// `REMOVE` cannot look: `OPENDIR` is `ENOENT`, because a withheld naming right answers "in this
/// scope there is no such name" (§47 (a directory capability carries six rights)). Through
/// `ENUMERATE｜DESCEND｜REMOVE` it takes the whole tree, bottom-up, five names. The capability table
/// decides the outcome, not the program.
#[test_case]
fn rm_r_takes_the_subtree_it_was_granted_and_a_narrower_grant_cannot_begin() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(refused) = run_rm(directory::rights::REMOVE_TREE, tree::RM_DOOMED, 0) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_eq!(
        refused.status,
        rr::status(directory::EISDIR),
        "`rm` of a directory must refuse, not quietly recurse",
    );
    assert_eq!(refused.removed, 0);
    assert!(
        refused.printed > 0,
        "a failure is a diagnostic AND an exit status; this one printed nothing",
    );

    // The narrow grant: it may take names out of `rmtree` and may not walk into anything.
    let blind = run_rm(
        directory::rights::REMOVE,
        tree::RM_DOOMED,
        rm_options::RECURSIVE,
    )
    .expect("the service was wired by the first run");
    assert_eq!(
        blind.status,
        rr::status(2), // ENOENT
        "a capability that may not descend must not learn that the subtree is there",
    );
    assert_eq!(
        blind.removed, 0,
        "a `rm -r` that could not walk must not have removed anything on the way",
    );

    // And the same line through the grant that carries the walk.
    let done = run_rm(
        directory::rights::REMOVE_TREE,
        tree::RM_DOOMED,
        rm_options::RECURSIVE | rm_options::VERBOSE,
    )
    .expect("the service was wired by the first run");
    assert_eq!(done.status, rr::OK, "the removal reported a failure");
    assert_eq!(
        done.removed, 5,
        "two files, a leaf, the directory holding it, and the top: five names",
    );
    assert_eq!(
        done.printed, 5,
        "`-v` prints one line per name removed, and nothing else",
    );
}

/// `-f` is idempotency, and silence is the default, both checked against `rm(1)`'s man page.
///
/// `rm rm-nothing` on an absent name is a diagnostic and a non-zero status. `rm -f rm-nothing` is
/// neither ("if the file does not exist, do not display a diagnostic message or modify the exit
/// status"), which makes a script re-runnable. `rm rm-solo` on a present name removes it and prints
/// nothing at all; without that run the `-f` claim would hold of a program that never prints. Its
/// names are its own, so this run and the recursive one cannot depend on each other's order.
#[test_case]
fn rm_f_ignores_a_name_that_is_not_there_and_success_says_nothing() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(loud) = run_rm(directory::rights::REMOVE_TREE, tree::RM_MISSING, 0) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_eq!(loud.status, rr::status(2), "a missing name is ENOENT");
    assert!(loud.printed > 0, "and it says so");

    let quiet = run_rm(
        directory::rights::REMOVE_TREE,
        tree::RM_MISSING,
        rm_options::FORCE,
    )
    .expect("the service was wired by the first run");
    assert_eq!(
        quiet.status,
        rr::OK,
        "`-f` must not modify the exit status for a name that is not there",
    );
    assert_eq!(quiet.printed, 0, "`-f` suppresses the diagnostic too");
    assert_eq!(quiet.removed, 0, "there was nothing to remove");

    let did = run_rm(directory::rights::REMOVE_TREE, tree::RM_SOLO, 0)
        .expect("the service was wired by the first run");
    assert_eq!(did.status, rr::OK);
    assert_eq!(did.removed, 1);
    assert_eq!(
        did.printed, 0,
        "success prints nothing; `-v` is the option that changes that",
    );
}
