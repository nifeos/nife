//! **A login session's work outlives the login, and a suspended user's does not** (milestone 152
//! (durable delegation)): logout refused while work is pending, a user's schedule surviving their
//! logout and ending when they empty it, suspension ending it, and a durable session re-derived at
//! start-up.
//!
//! Cut out of `login_tests.rs` unchanged on 2026-10-12 (UTC) by lane maintainer/files-over-1500,
//! which was over the 1,500 lines the next split tier starts at. The wiring and the budget these
//! tests share stay in `login_tests`. Named to sort right after `login_tests`, so the suite runs
//! these next to it; within one module the order was alphabetical, so three of these used to run
//! before the rest of `login_tests` and now run after it.
//!
//! Name: provisional, lane maintainer/files-over-1500 (#2017), 2026-10-12 (UTC), for the module and
//! the file. Chosen to sort beside `login_tests`, as above; the pull request records no other name
//! considered.

use credential_contract::fixture::{ALICE, BOB, WRONG};
use login_service as ls;

use super::login_tests::{DURABLE_UT_PAGES, free_terminal, redoxfs_server_image, wired};
use super::*;
use crate::sched;

/// **A login session with pending work cannot be logged out until the work is gone**, for
/// milestone 152 (durable delegation), on DECISIONS §16 (object revocation).
///
/// Milestone 152's design keeps a user's session alive past a disconnect for exactly as long as it
/// has scheduled work, and it builds no new mechanism for that: a scheduled job's authority is a
/// child split off the session's budget, and §16 already refuses `MemoryRegion::DESTROY` on a
/// parent with a live child. This proves that on the object a login session actually is. The
/// session's own budget, delegated by `login`, carries a one-page child; the logout's first step is
/// refused `NotPermitted`; the budget still works afterwards; and once the child is destroyed the
/// same logout goes through in [`ls::LOGOUT`]'s order, with its proofs that both capabilities came
/// down.
///
/// The proof used to live on `smb_server`'s `DurableSession`, which went with the SMB code on
/// 2026-08-30. A session re-derived at start-up is built on a budget the same way
/// (`a_durable_session_is_re_derived_at_start_up_unless_suspended`), so this holds for it too.
///
/// **Costs nothing permanent against
/// [`CONSTRUCTION_PAGES`](super::login_tests::CONSTRUCTION_PAGES)**: the session logs fully back
/// out.
#[test_case]
fn a_login_session_with_pending_work_refuses_logout_until_the_work_is_gone() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");
    let r = ls::client(cli, &w, ls::PENDING_WORK, ALICE, ALICE);
    assert_eq!(r[0], ls::RPT_OK, "alice was not authenticated");
    for (bit, what) in [
        (
            ls::F_BUDGET_WORKS,
            "the budget did not work before the child was split",
        ),
        (
            ls::F_LOGOUT_REFUSED_WHILE_PENDING,
            "the budget's DESTROY was not refused NotPermitted while a pending-job child lived; a \
             session with scheduled work could be logged out from under it",
        ),
        (
            ls::F_SESSION_SURVIVED_REFUSAL,
            "the budget stopped working after its refused DESTROY",
        ),
        (
            ls::F_PENDING_WORK_DESTROYED,
            "the pending-job child did not destroy",
        ),
        (
            ls::F_BUDGET_TEARDOWN_OK,
            "the budget did not destroy once its child was gone",
        ),
        (
            ls::F_BUDGET_DEAD_AFTER_TEARDOWN,
            "the budget still answered a RETYPE after its own destroy",
        ),
        (ls::F_TEARDOWN_OK, "the logout ticket's own DESTROY"),
        (
            ls::F_DEAD_AFTER_TEARDOWN,
            "the directory still answered a READDIR after the session came down",
        ),
    ] {
        assert_eq!(r[1] & bit, bit, "{what}");
    }
    let a = sched::ipc_receive(w.audit);
    assert_eq!(
        a[0],
        login_contract::ATTRIBUTED,
        "no attribution record followed the login",
    );
    free_terminal(&w);
}

/// **A user's schedule outlives their login, comes back when they return, and ends when they empty
/// it** (milestone 152, durable delegation; S1 and L2 of 2026-09-26; §222 (who holds a user's
/// schedule)).
///
/// Three logins as `alice`, each by a separate client process, so nothing but `login` and the
/// durable session carries state from one to the next:
///
/// 1. [`ls::OPEN_SCHEDULE`] asks for its schedule. `login` builds `user_timetable_keeper` from a region
///    of the user's own budget, `user_timetable_keeper` builds a timetable, and the client replaces its
///    empty document with one entry the timetable plans to fire. It then detaches: its directory
///    goes, and its budget refuses `DESTROY`, because `user_timetable_keeper` lives on it (§16).
/// 2. [`ls::EMPTY_SCHEDULE`] logs in plainly and is handed the same session back: the page still
///    carries the first client's reply. It replaces the document with an empty one, and the
///    timetable answers `STATUS_EMPTIED` and writes its exit word.
/// 3. [`ls::LOGOUT`] logs in plainly and gets an ordinary session, with no page announced, and
///    tears it down completely. That is the proof nothing outlived its reason: `login` saw the exit
///    word, reclaimed `user_timetable_keeper` and the old budget, and minted fresh.
///
/// **Costs nothing permanent against
/// [`CONSTRUCTION_PAGES`](super::login_tests::CONSTRUCTION_PAGES)**: every session here comes home,
/// the durable one through `login`'s own `DURABLE_UT_PAGES` budget.
#[test_case]
fn a_users_schedule_outlives_their_login_and_ends_when_they_empty_it() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");
    let login = |behaviour: u64| {
        let r = ls::client(cli, &w, behaviour, ALICE, ALICE);
        let a = sched::ipc_receive(w.audit);
        assert_eq!(
            a[0],
            login_contract::ATTRIBUTED,
            "no attribution record followed a login"
        );
        free_terminal(&w);
        assert_eq!(r[0], ls::RPT_OK, "alice was not authenticated");
        r[1]
    };

    let first = login(ls::OPEN_SCHEDULE);
    for (bit, what) in [
        (
            ls::F_SCHEDULE_ANNOUNCED,
            "SCHEDULE was answered without a registration page",
        ),
        (
            ls::F_REPLACED,
            "the new timetable did not put the first document in force",
        ),
        (
            ls::F_NO_REPORT,
            "the plan granted a durable job a report endpoint, which Fork 6 C refuses",
        ),
        (
            ls::F_LOGOUT_REFUSED_WHILE_PENDING,
            "the budget came down while its `user_timetable_keeper` lived on it",
        ),
        (
            ls::F_TEARDOWN_OK,
            "the first session's directory did not come down",
        ),
    ] {
        assert_eq!(first & bit, bit, "{what}");
    }

    let second = login(ls::EMPTY_SCHEDULE);
    for (bit, what) in [
        (
            ls::F_SCHEDULE_ANNOUNCED,
            "a plain login for an identity with a durable session was not handed its page",
        ),
        (
            ls::F_REATTACHED,
            "the page handed back did not carry the first session's reply: a second session was \
             minted instead of the first reattached",
        ),
        (
            ls::F_REPLACED,
            "an empty document was not answered STATUS_EMPTIED",
        ),
        (
            ls::F_TIMETABLE_EXITED,
            "the timetable did not write its exit word",
        ),
    ] {
        assert_eq!(second & bit, bit, "{what}");
    }

    let third = login(ls::LOGOUT);
    assert_eq!(
        third & ls::F_SCHEDULE_ANNOUNCED,
        0,
        "a page was announced after the schedule was emptied; the session outlived its reason",
    );
    for (bit, what) in [
        (
            ls::F_BUDGET_TEARDOWN_OK,
            "the fresh session's budget did not come down",
        ),
        (
            ls::F_TEARDOWN_OK,
            "the fresh session's directory did not come down",
        ),
    ] {
        assert_eq!(third & bit, bit, "{what}");
    }
}

/// **Suspending a user ends their schedule now, refuses their login, and resuming lets them back**
/// (milestone 152 (durable delegation), calef's §108 (disabling credentials kills the durable session) ruling of 2026-09-26).
///
/// `alice` opens a schedule and detaches, so a `user_timetable_keeper` and a timetable are running on her
/// budget. The owner's console then does what `user suspend alice` does: it lists `alice` in
/// `login_contract::SUSPENDED_LIST` and sends `SUSPEND` on the front door. `login` answers that it
/// ended one durable session, and a correct password for `alice` is refused `SUSPENDED`, not
/// `DENIED`, while her stored schedule and her manifest line stay. `user resume alice` empties the
/// list, and the stored schedule comes back at her next login, as the ruling says; emptying it then
/// takes her off the manifest. A plain `DENIED` for the wrong secret while suspended shows the
/// refusal is still after authentication.
///
/// # BUGS
///
/// **Its first login was refused `DENIED` once on the HVF leg**, with the right password and before
/// any suspension: `alice was not authenticated`, left 2 (`login_contract::DENIED`), right 1. One of
/// ten full `script/test --hvf` runs on 2026-10-12 (UTC), at a 1-minute load of 3.4, found by
/// milestone 514 (the HVF std listener hangs in some full runs)'s verification runs and not
/// investigated there. The suspended list is emptied at the top of this test, so a list left by an
/// earlier test is not the obvious reading.
#[test_case]
fn suspending_a_user_ends_their_schedule_and_refuses_them_until_resumed() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    fs_service::set_root_list(login_contract::SUSPENDED_LIST, None);
    free_terminal(&w);
    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");
    let session = |behaviour: u64| {
        let r = ls::client(cli, &w, behaviour, ALICE, ALICE);
        if r[0] == ls::RPT_OK {
            let a = sched::ipc_receive(w.audit);
            assert_eq!(
                a[0],
                login_contract::ATTRIBUTED,
                "no attribution record followed a login"
            );
            free_terminal(&w);
        }
        r
    };

    let opened = session(ls::OPEN_SCHEDULE);
    assert_eq!(opened[0], ls::RPT_OK, "alice was not authenticated");
    assert_eq!(
        opened[1] & ls::F_REPLACED,
        ls::F_REPLACED,
        "the schedule did not open, so there is nothing for a suspension to end",
    );

    suspend_alice(&w);

    let refused = session(ls::LOGIN);
    assert_eq!(
        refused[0],
        ls::RPT_SUSPENDED,
        "a suspended identity with the right secret was not refused SUSPENDED",
    );
    let wrong = ls::client(cli, &w, ls::LOGIN, ALICE, WRONG);
    assert_eq!(
        wrong[0],
        ls::RPT_DENIED,
        "a wrong secret for a suspended identity must read as a wrong secret",
    );

    // The suspension kept the stored schedule and the manifest line: `user resume` is meant to
    // bring the schedule back, and the boot-time re-deriver skips alice only while she is listed.
    assert!(
        manifest_lists(b"alice"),
        "suspending alice took her off the durable-session manifest; resuming could not restore her",
    );

    // `user resume alice`. The stored schedule comes back at this login: `login` opens a fresh
    // session and stages the stored document as sequence 1, so the page carries a reply before the
    // client has asked anything. The client then empties it, writing the empty store first.
    fs_service::set_root_list(login_contract::SUSPENDED_LIST, None);
    let back = session(ls::EMPTY_SCHEDULE);
    assert_eq!(
        back[0],
        ls::RPT_OK,
        "a resumed identity was not let back in"
    );
    the_stored_schedule_came_back_and_was_emptied(back);

    // Emptied by its user, the schedule does not come back, and alice leaves the manifest.
    let last = session(ls::LOGOUT);
    assert_eq!(
        last[0],
        ls::RPT_OK,
        "alice was not let in after emptying her schedule"
    );
    assert_eq!(
        last[1] & ls::F_SCHEDULE_ANNOUNCED,
        0,
        "an emptied schedule came back at the next login",
    );
    assert_eq!(
        last[1] & ls::F_BUDGET_TEARDOWN_OK,
        ls::F_BUDGET_TEARDOWN_OK,
        "the last session did not tear down",
    );
    assert!(
        !manifest_lists(b"alice"),
        "alice is still on the durable-session manifest after emptying her schedule",
    );
}

/// List `alice` as suspended and send `SUSPEND` on the front door, which must end her one
/// durable session.
fn suspend_alice(w: &ls::Wiring) {
    // `user suspend alice`, as the owner's console does it.
    fs_service::set_root_list(login_contract::SUSPENDED_LIST, Some(b"alice\n"));
    sched::ipc_send(w.request, [login_contract::suspend_word(), 0, 0]);
    let applied = sched::ipc_receive(w.result);
    assert_eq!(
        applied[0],
        login_contract::APPLIED,
        "SUSPEND was not answered APPLIED"
    );
    assert_eq!(applied[1], 1, "SUSPEND did not end alice's durable session");
}

/// The flags of the first login after `user resume alice`, `back`.
fn the_stored_schedule_came_back_and_was_emptied(back: [u64; 5]) {
    for (bit, what) in [
        (
            ls::F_SCHEDULE_ANNOUNCED,
            "the stored schedule did not come back at the login after resume",
        ),
        (
            ls::F_REATTACHED,
            "the page carried no reply, so login did not put the stored document in force",
        ),
        (
            ls::F_STORED,
            "the empty document was not written to the store first",
        ),
        (
            ls::F_TIMETABLE_EXITED,
            "emptying the restored schedule did not stop its timetable",
        ),
    ] {
        assert_eq!(back[1] & bit, bit, "{what}");
    }
}

/// Whether the durable-session manifest at the file service's root names `identity`.
fn manifest_lists(identity: &[u8]) -> bool {
    let mut buf = [0u8; 512];
    fs_service::read_root_file(timetable_store::MANIFEST_FILE_NAME, &mut buf)
        .is_some_and(|n| login_contract::lists(&buf[..n], identity))
}

/// The construction budget the start-up test's own `login` is given: `login.rs`'s `OWN_UT_PAGES`
/// (128), [`DURABLE_UT_PAGES`] and `CHANNEL_UT_PAGES` (32), split at its start, with 32 pages of
/// margin. It serves no successful login, so nothing else is spent from it.
const START_UP_CONSTRUCTION_PAGES: u64 = 128 + DURABLE_UT_PAGES + 32 + 32;

/// **`login` re-derives a stored schedule at start-up, before anyone logs in, and skips a suspended
/// identity** (milestone 152 (durable delegation); Fork 7 ruled A by calef on 2026-09-27; DECISIONS
/// §123 (the boot-time re-derivation privilege) as amended). What `session_reviver_tests` proved
/// against the retired `session_reviver`, now against `login` itself.
///
/// The store is written as a reboot would find it: `alice` and `bob` each have
/// `timetable_store::fixture::DEMO_SCHEDULE_DOC` stored in their own subtree (§122 (the on-disk
/// schedule store)), the manifest (§125 (which identities have pending work)) names both, `alice`
/// first, and the owner's suspended list names `alice`. Then a second `login` starts against that
/// store, and three exchanges on its front door, none of them a successful login, show what its
/// start-up pass did:
///
/// 1. `SUSPEND` with `alice` listed ends nothing. He has a stored schedule and comes first in the
///    manifest, so only the skip explains it; with one session kept, a pass that ignored the list
///    would have spent it on her.
/// 2. A wrong secret for `bob` is `DENIED`. The session re-derived for him is handed out only
///    after the credential service says yes, as at any login.
/// 3. `SUSPEND` with `bob` listed ends one session. Nothing but the start-up pass could have
///    built it, and it only stays kept when the timetable accepted the stored document, so this
///    is also the proof that what was written through the file service is what `login` read and
///    the timetable took, the connective proof `session_reviver` made against `timetable::parse`.
///
/// Its own `login`, because the start-up pass runs once, at start: `wired()`'s is shared by the
/// whole suite and starts on an empty manifest. The store is put back afterwards.
///
/// **A permanent charge against `kernel::testing::SUITE_PAGE_FRAME_BUDGET`**: a second `login`
/// process and its [`START_UP_CONSTRUCTION_PAGES`], which no test tears down. The re-derived
/// session itself comes home to that budget when step 3 ends it.
#[test_case]
fn a_durable_session_is_re_derived_at_start_up_unless_suspended() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    // Brings up the file service and both home subtrees, and so orders this after them.
    if wired().is_none() {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    }
    let Some((cred, _, _)) = credential_tests::provisioned() else {
        crate::testing::skip!("no virtio-rng device");
    };
    let (fs_ep, fs_page_frame) =
        fs_service::root_directory(fs_service::blk_server_image(), redoxfs_server_image())
            .expect("wired() already brought the file service up");

    let doc = timetable_store::fixture::DEMO_SCHEDULE_DOC.as_bytes();
    let stored = timetable_store::SCHEDULE_FILE_NAME;
    fs_service::set_home_file(b"alice", stored, Some(doc));
    fs_service::set_home_file(b"bob", stored, Some(doc));
    fs_service::set_root_list(timetable_store::MANIFEST_FILE_NAME, Some(b"alice\nbob\n"));
    fs_service::set_root_list(login_contract::SUSPENDED_LIST, Some(b"alice\n"));

    let w = ls::start(
        program("login").expect("no login program in the initial ramdisk"),
        cred.verify,
        cred.verify_page_frame,
        fs_ep,
        fs_page_frame,
        START_UP_CONSTRUCTION_PAGES,
        true,
    );
    // The first word on the front door is received only once the start-up pass is over, so every
    // exchange below sees what it left.
    let suspend = || {
        sched::ipc_send(w.request, [login_contract::suspend_word(), 0, 0]);
        let r = sched::ipc_receive(w.result);
        assert_eq!(
            r[0],
            login_contract::APPLIED,
            "SUSPEND was not answered APPLIED"
        );
        r[1]
    };
    // What the start-up pass's own `rederive` skipped and why (`login_contract::REDERIVE_SKIPS`),
    // read back for a failing assertion below to print: the "skipped, not failed" contract used to
    // leave no reason anywhere a failure could show.
    let rederive_skips = || {
        sched::ipc_send(w.request, [login_contract::rederive_skips_word(), 0, 0]);
        let r = sched::ipc_receive(w.result);
        assert_eq!(
            r[0],
            login_contract::SKIP_COUNTS,
            "REDERIVE_SKIPS was not answered SKIP_COUNTS"
        );
        login_contract::durable::unpack_skip_counts(r[1])
    };

    assert_eq!(
        suspend(),
        0,
        "a suspended identity's stored schedule was re-derived at start-up",
    );

    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");
    let wrong = ls::client(cli, &w, ls::LOGIN, BOB, WRONG);
    assert_eq!(
        wrong[0],
        ls::RPT_DENIED,
        "a wrong secret for an identity with a re-derived session was not DENIED",
    );

    fs_service::set_root_list(login_contract::SUSPENDED_LIST, Some(b"bob\n"));
    let ended = suspend();
    // Read before the failure, not only on it: querying costs one exchange and the value must
    // exist by the time `assert_eq!` might format it.
    let skips = rederive_skips();

    fs_service::set_root_list(login_contract::SUSPENDED_LIST, None);
    fs_service::set_root_list(timetable_store::MANIFEST_FILE_NAME, None);
    fs_service::set_home_file(b"alice", stored, None);
    fs_service::set_home_file(b"bob", stored, None);
    assert_eq!(
        ended, 1,
        "no durable session was kept for bob at start-up: the manifest, his stored schedule \
         or the timetable's answer to it did not carry through `login`'s start-up pass. \
         rederive skip counts [table_full, identity, no_stored_schedule, budget_out_of_pages, \
         session_build_failed] = {skips:?}",
    );
}
