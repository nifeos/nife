//! The shared-frame witness (milestone 599 (a frame per filesystem client channel), provisional).
//!
//! Finding 1 of `notes/shared-page-audit.md` is one read-write staging frame shared by every FS
//! client, which kept clients apart by which was blocked, not by what each could map. Each client
//! now has its own channel window, keyed by the badge on its endpoint capability. The test runs two
//! live clients, one rewriting the frame the other staged its name in, and asserts isolation, after
//! a negative control on one shared window (the old wiring) where the substitution must land.
//! Nothing here is architecture-specific, so one module meets DECISIONS §19 (architectural parity
//! is a tenet) on every architecture `script/test` boots, as `dir_capability_tests` does.

//! # Why a handshake and not a race
//!
//! The property is whether a second holder of the frame can change the bytes the server resolves
//! for another client's call; the server reads the name from the page after the message arrives
//! and cannot tell whose bytes it read. A two-message handshake over a sync endpoint forces the
//! audit's interleaving (victim stages, attacker overwrites, victim calls) on every run. A runnable
//! second writer reaches it by chance on four-core boots; the handshake removes the flakiness, not
//! the mechanism.

use filesystem_fixture as fixture;

use super::*;
use crate::sched;

/// The two `fs_test_client` roles, mirrored from `fixtures/src/fs_test_client/role.rs`. The numbers
/// live on both sides because one program is started by role, the same shape as every other role
/// here.
const ROLE_SHARE_VICTIM: u64 = 13;
const ROLE_SHARE_ATTACKER: u64 = 14;

/// Run the witness once and return the victim's verdict, or `None` when there is no disk.
/// `shared` puts both clients on one window (the pre-599 wiring); see [`fs_service::start_shared_frame_witness`].
///
/// A helper returns `None` rather than calling `skip!`, because `skip!` returns from the function
/// it is written in and only the `#[test_case]` can honestly skip.
fn witness(shared: bool) -> Option<u64> {
    let (_readiness, report, held) = fs_service::start_shared_frame_witness(
        fs_service::blk_server_image(),
        program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk"),
        program("fs_test_client").expect("no fs_test_client program in the initial ramdisk"),
        ROLE_SHARE_VICTIM,
        ROLE_SHARE_ATTACKER,
        shared,
    )?;
    let [verdict, w1, ..] = sched::ipc_receive(report);
    held.release_or_fail("the shared-frame witness's two clients");
    assert_ne!(
        verdict,
        fixture::SHARED_UNEXPECTED,
        "the witness is wired wrong (shared = {shared}): the victim's open failed or returned \
         neither file's body (errno/aux {w1:#x})",
    );
    Some(verdict)
}

/// Two live clients on one file service; one rewrites the frame the other staged its name in.
///
/// The victim stages `fixture::SHARED_VICTIM_NAME`, the attacker writes
/// `fixture::SHARED_USURPER_NAME` into the frame it maps, then the victim calls and reports which
/// file it got. The control runs first, on one shared window: it must report `SHARED_SUBSTITUTED`,
/// or the attack or handshake is broken and the second run proves nothing. On separate windows the
/// victim's own name resolves: `SHARED_ISOLATED`.
#[test_case]
fn a_second_client_cannot_substitute_the_name_on_its_own_window() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(control) = witness(true) else {
        crate::testing::skip!("no RedoxFS disk attached");
    };
    assert_eq!(
        control,
        fixture::SHARED_SUBSTITUTED,
        "the negative control did not substitute ({control:#x}): with both clients on one window \
         the attacker's write must land in the victim's frame. If it does not, the handshake or \
         the attack is broken, and the isolated run below would pass for the wrong reason.",
    );
    let isolated = witness(false).expect("the service was wired by the control run");
    assert_eq!(
        isolated,
        fixture::SHARED_ISOLATED,
        "the victim read the attacker's file ({isolated:#x}) on separate windows, so a second \
         client still reached the frame the server resolved the victim's name from. Each client's \
         window is keyed by its endpoint's badge and must be its own.",
    );
}
