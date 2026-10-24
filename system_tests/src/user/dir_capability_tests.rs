use filesystem_contract::dir;
use filesystem_fixture::dirscape as esc;

use super::*;
use crate::sched;

/// The binary carrying the block server's role. One `cfg`, in `fs_service`, because the boot
/// path needs the same answer this module's tests do.
pub(super) fn blk_server_image() -> &'static [u8] {
    fs_service::blk_server_image()
}

/// Wire a `fs_subtree_caretaker` holding a capability to the fixture's `sub` with exactly
/// `rights`, run the directory attacker against it, and return its verdict bitmap. `run` keeps
/// the names the attacker creates distinct, because all three runs share one image within a
/// boot and an `EEXIST` would otherwise read as a refusal.
///
/// `None` when no RedoxFS disk is attached (nothing to test; do not fail).
pub(super) fn attack_a_subtree(rights: u64, run: u64) -> Option<u64> {
    let Some((report, held)) = fs_service::start_granted_dir(
        blk_server_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("fs_subtree_caretaker")
            .expect("no fs_subtree_caretaker program in the initial ramdisk"),
        program("fs_test_client").expect("no fs_test_client program in the initial ramdisk"),
        fs_service::DirGrant {
            name: filesystem_fixture::tree::SUB,
            rights,
            role: 5, // ROLE_DIR_ATTACKER
            arg: run,
            arg2: 0,
            stack_pages: 0,
        },
    ) else {
        // No print, no skip!() here: this is a helper, and `skip!()` returns from the function it
        // is written in, which would leave the test running. `None` is the fixture's absence
        // travelling to the `#[test_case]`, which is the only place that can honestly skip.
        return None;
    };
    // Both handshakes happened inside `start_granted_dir`, before this attacker existed. That
    // ordering is the fix for the startup clobber `fs_service::wait_for_caretaker` records.
    let [tag, verdict, ..] = sched::ipc_receive(report);
    held.release_or_fail("a granted directory and its attacker");
    assert_eq!(
        tag,
        filesystem_fixture::VERDICT,
        "the attacker's report is not a verdict word",
    );
    Some(verdict)
}

/// Compare a verdict against the exact set the configuration is specified to produce, and say
/// what the difference means in words rather than in hex.
///
/// Both directions are failures and they are different failures, which is why this is not one
/// equality assertion: a bit set that should not be is an escape, and a bit clear that should be
/// set means the capability did not work, so every refusal it reported proves nothing.
pub(super) fn assert_verdict(got: u64, want: u64, what: &str) {
    let leaked = got & !want;
    let missing = want & !got;
    assert_eq!(
        leaked,
        0,
        "the {what} directory capability leaked: {}",
        describe_dirscape(leaked),
    );
    assert_eq!(
        missing, 0,
        "the {what} directory capability could not do what it was granted ({missing:#x}), so \
         its refusals prove nothing: a capability that reaches nothing is trivially confined",
    );
}

/// Name the bits a verdict set, so a failure reads as a sentence instead of a bitmap.
fn describe_dirscape(v: u64) -> &'static str {
    if v & esc::REACHED_PARENT != 0 {
        "it opened a file that is in the granted directory's PARENT"
    } else if v & esc::REACHED_SIBLING != 0 {
        "it reached the granted directory's SIBLING"
    } else if v & esc::WALKED_UP != 0 {
        "`..` resolved to something"
    } else if v & esc::WIDENED != 0 {
        "a child carried a right its parent did not have"
    } else if v & esc::ENUMERATED_A_STRANGER != 0 {
        "a listing held a name from outside the grant"
    } else if v & esc::FORGED_HANDLE != 0 {
        "it reached something with a handle it was never given"
    } else if v & esc::CREATED != 0 {
        "it created a name through a capability with no create right"
    } else if v & esc::MADE_A_DIR != 0 {
        "it made a directory through a capability that could not"
    } else if v & esc::RENAMED != 0 {
        "it renamed a name through a capability with no remove right"
    } else if v & esc::WROTE != 0 {
        "it wrote through a capability with no write right"
    } else if v & esc::DESCENDED != 0 {
        "it descended through a capability with no descend right"
    } else if v & esc::ENUMERATED != 0 {
        "it enumerated through a capability with no enumerate right"
    } else if v & esc::REACHED_AN_UNMATCHED_NAME != 0 {
        "it reached a name in the granted directory that the grant's set does not carry"
    } else if v & esc::SET_AN_ATTR != 0 {
        "it set an extended attribute through a capability with no write right"
    } else if v & esc::READ_ATTRS != 0 {
        "it read extended attributes through a capability that could not open the file"
    } else if v & esc::GRANTED_ACCESS_FAILED != 0 {
        "the granted access itself failed, so nothing was actually proven"
    } else {
        "nothing (an empty verdict should not have failed an assertion)"
    }
}

/// **A read-only explorer: it may descend, read and list, and it reaches nothing above itself.**
///
/// The keystone of milestone 47 (navigation and naming) under attack. Holding a capability to
/// `sub`, the attacker tries to name `motd` (in the parent), `other` and `other/secret` (a sibling)
/// and `..`. All three exist and the caretaker one hop up could reach each, so every refusal is a
/// fact about the capability. A child asked for no rights can do nothing, and a child asked for a
/// right this grant lacks is refused rather than given something smaller
/// (notes/dir-capability/tests-and-stack-usage.md).
///
/// Falsification: replayable `system_tests/falsifications/user.dir_capability_tests.a_read_only_directory_capability_reaches_its_subtree_and_nothing_above_it.patch`
#[test_case]
fn a_read_only_directory_capability_reaches_its_subtree_and_nothing_above_it() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(v) = attack_a_subtree(dir::DESCEND | dir::READ | dir::ENUMERATE, 1) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_verdict(
        v,
        esc::OPENED_ITS_OWN | esc::ENUMERATED | esc::DESCENDED | esc::READ_ATTRS,
        "read-only",
    );
}

/// **The same attacker against a capability carrying every right**, which is what makes the run
/// above mean anything.
///
/// Without it, a `fs_subtree_caretaker` that answered no to everything would pass the read-only
/// test perfectly, and so would a grant that reached nothing at all. Here the same requests
/// through the same code succeed: it writes, it creates, it makes a directory. And it *still*
/// reaches nothing above itself, which is the point: the widening is exactly the axes that were
/// widened.
#[test_case]
fn a_full_directory_capability_does_everything_inside_and_nothing_outside() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(v) = attack_a_subtree(dir::ALL, 2) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_verdict(
        v,
        esc::OPENED_ITS_OWN
            | esc::ENUMERATED
            | esc::DESCENDED
            | esc::CREATED
            | esc::WROTE
            | esc::RENAMED
            | esc::MADE_A_DIR
            | esc::READ_ATTRS
            | esc::SET_AN_ATTR,
        "full",
    );
}

/// **Milestone 47's motivating sentence, made a test**: a program handed a directory to write
/// into can add to it and write to what it added, and it cannot walk into a subdirectory or find
/// out what else is in there.
///
/// `DESCEND`, `ENUMERATE` and `REMOVE` are withheld. Without `DESCEND`, `CREATE` makes a file and
/// not a directory: `mkdir` needs both, and a directory it could not walk into would mint a
/// capability out of a withheld right. Without `REMOVE`, `CREATE` is "add to this, destroy
/// nothing" exactly, which `RENAME` makes falsifiable: the attacker creates a name and cannot move
/// it, through the same code the full run moves it with.
#[test_case]
fn an_append_only_directory_capability_adds_and_cannot_walk_or_list() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(v) = attack_a_subtree(dir::READ | dir::WRITE | dir::CREATE, 3) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_verdict(
        v,
        esc::OPENED_ITS_OWN | esc::CREATED | esc::WROTE | esc::READ_ATTRS | esc::SET_AN_ATTR,
        "append-only",
    );
}

/// **A name-set capability carries its files' attributes and still designates only its names**
/// (milestone 61, the third caretaker).
///
/// `fs_nameset_caretaker` inspects a name on every request, so its four attribute verbs could go
/// wrong either way. Filtering the attribute name against the set would hide what is attached to a
/// matched file (`READ_ATTRS` and `SET_AN_ATTR` catch it). Not filtering the directory names would
/// unmake the set (`REACHED_AN_UNMATCHED_NAME` catches it, through `dir::READ` verbs `rm` never
/// sends). The set is one name in `sub`, so `deeper` is one entry away and reachable one hop up.
#[test_case]
fn a_name_set_capability_reads_its_attributes_and_still_names_only_its_set() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some((report, held)) = fs_service::start_granted_set(
        blk_server_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("fs_nameset_caretaker")
            .expect("no fs_nameset_caretaker program in the initial ramdisk"),
        program("fs_test_client").expect("no fs_test_client program in the initial ramdisk"),
        fs_service::SetGrant {
            dir: filesystem_fixture::tree::SUB,
            // Exactly one name, and `deeper` deliberately left out of it.
            names: &[(filesystem_fixture::tree::INNER.as_bytes(), false)],
            rights: dir::READ | dir::WRITE | dir::DESCEND,
            role: 6, // ROLE_SET_ATTRS
            arg: 0,
            arg2: 0,
            stack_pages: 0,
        },
    ) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    let [tag, v, ..] = sched::ipc_receive(report);
    held.release_or_fail("a name-set grant and its witness");
    assert_eq!(
        tag,
        filesystem_fixture::VERDICT,
        "the name-set witness's report is not a verdict word",
    );
    assert_verdict(
        v,
        esc::OPENED_ITS_OWN | esc::READ_ATTRS | esc::SET_AN_ATTR,
        "name-set",
    );
}
