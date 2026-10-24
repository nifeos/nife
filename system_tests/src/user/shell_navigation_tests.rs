use filesystem_contract::dir;
use filesystem_fixture::{navscape as nb, tree};

use super::*;
use crate::sched;

/// The `swish` binary's navigating role (`components/src/swish.rs`).
pub(super) const ROLE_NAVIGATE: u64 = 1;

/// The bits every navigating shell must report whatever it was rooted in: `pwd` at its root,
/// `..` clamped, `/` naming its own root and stopping there, a listing, and the whole
/// `mkdir` / create / `rm` / `touch` sequence including the two halves of "unlink is not revoke"
/// and the two halves of `touch`'s own "create absent, leave present alone".
pub(super) const ALWAYS: u64 = nb::PWD_IS_ROOT
    | nb::CLAMPED_AT_ROOT
    // The namespace half (2026-08-18): `/` is the root of your own namespace, and it stops there.
    | nb::ABSOLUTE_IS_MY_ROOT
    | nb::ABSOLUTE_CLAMPED_AT_ROOT
    | nb::LISTED
    | nb::CREATED
    | nb::MADE_DIR
    | nb::UNLINKED
    | nb::HOLDER_KEPT_READING
    | nb::NAME_GONE_AFTER_UNLINK
    | nb::UNLINK_REFUSED_A_DIRECTORY
    // The two halves of "no single call takes a subtree away", added with `RMDIR`: a directory
    // with a name in it is refused, and the same call works once the name is out.
    | nb::RMDIR_REFUSED_NON_EMPTY
    | nb::RMDIR_REMOVED_EMPTY
    // `touch`'s two-fold contract: creates an absent name, and a second call on a name that now
    // holds a body leaves it exactly as it was.
    | nb::TOUCH_CREATED
    | nb::TOUCH_PRESERVED
    // `touch`'s mtime half (milestone 47's mtime lane, DECISIONS §112): a bare `touch` moves the
    // mtime forward, and `touch -t` lands on exactly the asserted instant rather than "now" again.
    | nb::TOUCH_MTIME_ADVANCED
    | nb::TOUCH_AT_ROUND_TRIPPED
    // The two-right split, proven against one freshly-minted handle rather than assumed from
    // `redoxfs_server`'s own host tests: `WRITE` alone is enough for bare `touch`, and it is not enough
    // for `touch -t`.
    | nb::TOUCH_NOW_NEEDS_ONLY_WRITE
    | nb::TOUCH_AT_REFUSED_WITHOUT_SETTIME
    // `bind` (milestone 47, "blocked on a second grant"; milestone 154 supplied it): a bound
    // name reaches the real position it was given (`ls` lists the marker it never `mkdir`ed or
    // `cd`ed into directly), `..` from inside it climbs the *real* tree rather than stopping at
    // a boundary invented at the alias, and that climb still refuses at this shell's own true
    // root, exactly where a direct walk to the same depth would.
    | nb::BIND_REACHED_TARGET
    | nb::BIND_ASCEND_REACHES_REAL_PARENT
    | nb::BIND_STOPS_AT_TRUE_ROOT;

/// Wire a `fs_subtree_caretaker` holding a capability to `root` and run the shell's navigation
/// script inside it. `run` keeps the names it creates distinct across runs sharing one image.
///
/// The run index and the rights ride in one word packed by `filesystem_contract::grant::spec`, the same
/// packing the caretaker's own grant uses: the shell is **told** what its capability carries
/// because nothing on this wire reports what a handle holds, and `OPENDIR` refuses a request
/// wider than the parent rather than narrowing it (notes/shell-navigation.md).
///
/// `None` when no RedoxFS disk is attached.
pub(super) fn navigate(root: &'static str, run: u64) -> Option<u64> {
    let (report, held) = fs_service::start_granted_dir(
        dir_capability_tests::blk_server_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("fs_subtree_caretaker")
            .expect("no fs_subtree_caretaker program in the initial ramdisk"),
        program("swish").expect("no swish program in the initial ramdisk"),
        fs_service::DirGrant {
            name: root,
            rights: dir::ALL,
            role: ROLE_NAVIGATE,
            arg: filesystem_contract::grant::spec(run as usize, dir::ALL),
            arg2: 0,
            // Measured, not guessed: see `spawn_fs_client`. A shell carries a path stack, a
            // parsed path and a listing buffer by value, and one page is 192 bytes short.
            stack_pages: 2,
        },
    )?;
    let [tag, verdict, ..] = sched::ipc_receive(report);
    held.release_or_fail("a granted shell");
    assert_eq!(
        tag,
        filesystem_fixture::VERDICT,
        "the shell's report is not a verdict word",
    );
    Some(verdict)
}

/// Compare a report against the exact set the configuration is specified to produce. Both
/// directions are failures and they are different ones: a bit set that should not be is a
/// shell reaching past its root, and a bit clear that should be set is a shell that could not
/// navigate at all, whose refusals therefore prove nothing.
fn assert_report(got: u64, want: u64, what: &str) {
    assert_eq!(
        got & !want,
        0,
        "the shell rooted at {what} did something it must not: {}",
        describe(got & !want),
    );
    assert_eq!(
        got & want,
        want,
        "the shell rooted at {what} could not do what its capability allows ({:#x} missing), \
         so every refusal it reported proves nothing",
        want & !got,
    );
}

/// Name a bit, so a failure reads as a sentence.
fn describe(v: u64) -> &'static str {
    if v & nb::WALKED_UP != 0 {
        "`..` climbed out of its root"
    } else if v & (nb::REACHED_SECRET | nb::REACHED_INNER) != 0 {
        "it opened a file that exists only in the OTHER shell's root"
    } else if v & (nb::ABSOLUTE_REACHED_SECRET | nb::ABSOLUTE_REACHED_INNER) != 0 {
        "an absolute path reached a file that exists only in the OTHER shell's root, so `/` is \
         rooted in something wider than what this shell holds"
    } else if v & (nb::SAW_SECRET | nb::SAW_INNER) != 0 {
        "its listing held a name from the other shell's root"
    } else if v & nb::NAVIGATION_FAILED != 0 {
        "the navigation itself failed, so nothing was proven"
    } else if v & nb::DESCENDED != 0 {
        "it descended into a directory that is not in its root"
    } else {
        "something no other bit describes"
    }
}

/// **A shell navigates the subtree it holds, and cannot climb out of it.**
///
/// `pwd` renders `/` at its root because that is the root of the only namespace it has; `..`
/// there is refused with nothing sent, because `..` is a pop of the stack of capabilities the
/// shell descended through and at the root there is nothing to pop; an absolute path is refused
/// as *unnameable*, since there is no namespace to root one in. Then the verbs that change the
/// tree, including the one this milestone insists on separating: `rm` removes the **name**, the
/// handle the shell still holds keeps reading the bytes, and the name really is gone.
#[test_case]
fn a_shell_navigates_its_own_subtree_and_clamps_at_its_root() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(v) = navigate(tree::SUB, 4) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_report(
        v,
        ALWAYS
            | nb::REACHED_INNER
            | nb::ABSOLUTE_REACHED_INNER
            | nb::SAW_INNER
            | nb::DESCENDED
            | nb::RETURNED,
        "sub",
    );
}

/// Every shell has its own root, and neither can name the other's files.
///
/// Two shells, two subtrees of one image, one script. Each tries to open `sub/inner` and
/// `other/secret` and is told nothing about which subtree it was rooted in, so the property is read
/// off the pair rather than claimed by either. Listings are checked the same way, since a name from
/// the other root appearing in one is an escape with nothing opened. The FS server and the
/// caretakers one hop up reach both directories; what stops each shell is that its table holds no
/// capability to the other subtree. They run in turn, sharing one page with the FS server.
///
/// Falsification: replayable `system_tests/falsifications/user.shell_navigation_tests.two_shells_with_different_roots_cannot_name_each_others_files.patch`
#[test_case]
fn two_shells_with_different_roots_cannot_name_each_others_files() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(a) = navigate(tree::SUB, 5) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    let b = navigate(tree::OTHER, 6).expect("the service was wired for the first shell");

    assert_report(
        a,
        ALWAYS
            | nb::REACHED_INNER
            | nb::ABSOLUTE_REACHED_INNER
            | nb::SAW_INNER
            | nb::DESCENDED
            | nb::RETURNED,
        "sub",
    );
    // The second holds a subtree with no child directory in it, so it cannot descend, and that
    // difference is the point: the same script against a different capability does different
    // things, and neither shell's world contains the other's.
    assert_report(
        b,
        ALWAYS | nb::REACHED_SECRET | nb::ABSOLUTE_REACHED_SECRET | nb::SAW_SECRET,
        "other",
    );

    // Stated once more as the crossing, because that is the sentence the milestone makes and an
    // exact-set assertion is easy to read as a list of unrelated facts.
    assert_eq!(
        (a & nb::REACHED_SECRET, b & nb::REACHED_INNER),
        (0, 0),
        "a shell named a file in the other shell's root",
    );
    assert_ne!(a & nb::REACHED_INNER, 0);
    assert_ne!(b & nb::REACHED_SECRET, 0);

    // **And once more with a leading slash**, which is the namespace half's whole claim: `/` is
    // the root of *your* namespace, so the same absolute token typed in two shells opens two
    // different files, and neither can name the other's. A `/` rooted in anything global would
    // set both of these bits in both reports.
    assert_eq!(
        (
            a & nb::ABSOLUTE_REACHED_SECRET,
            b & nb::ABSOLUTE_REACHED_INNER
        ),
        (0, 0),
        "an absolute path named a file in the other shell's root",
    );
    assert_ne!(a & nb::ABSOLUTE_REACHED_INNER, 0);
    assert_ne!(b & nb::ABSOLUTE_REACHED_SECRET, 0);
}
