use credential_contract::fixture::{ALICE, BOB, CAROL, NONE, WRONG};
use login_service as ls;

use super::*;
use crate::sched;

/// The login service's construction budget (`components/src/login.rs`'s `CONSTRUCTION_UT`), a
/// permanent charge against `kernel::testing::SUITE_PAGE_FRAME_BUDGET` from `ls::start` on.
///
/// The itemized minimum is 128 (`OWN_UT_PAGES`) + 32 (`CHANNEL_UT_PAGES`) + 16 * 64 (a caretaker
/// region per kept login, the four durable clients included) + 12 * 64 (ordinary client budgets) =
/// 1952; logouts, refusals and `connect`'s channels add nothing permanent. Out-of-order returns
/// strand slack, since a split needs contiguous pages. Measured: at 2688 the suite ran out near its
/// 23rd login (`DENIED` on a correct password); 3088 runs it green.
pub(super) const CONSTRUCTION_PAGES: u64 = 3088;

/// What `login` splits off its construction budget at start-up for durable sessions (milestone
/// 152): one durable session's budget, `login_contract::durable::BUDGET_PAGES`. Kept apart from
/// [`CONSTRUCTION_PAGES`] so the account above stays whole.
pub(super) const DURABLE_UT_PAGES: u64 = login_contract::durable::BUDGET_PAGES;

/// Install the one program the durable tests schedule, as an installer would (milestone 152, Fork 8
/// ruled D by calef on 2026-09-27, on #1377): the demo's bytes go to `packages/` and a generation
/// naming them to `activation/`, once per boot, before either scheduling `login` starts.
///
/// The bytes are `least_authority_demo`'s, but not its name: §229 (how a bare name at the prompt
/// reaches an installed program) refuses a package entry under a name the image carries, and
/// `with_entry`'s `ImageName` refusal would panic the suite. `scheduled_demo` is provisional.
fn install_scheduled_program() {
    use core::sync::atomic::{AtomicBool, Ordering};
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::Relaxed) {
        return;
    }
    let bytes = program("least_authority_demo").expect("no least_authority_demo in the archive");
    fs_service::install_for_test("scheduled_demo", "demo", "0.1.0", bytes);
}

/// `EEXIST`, matching `identity_provisioner.rs`'s own local constant: `fs_proto` does not re-export
/// it under a name (that file's own comment), so every direct caller of `fs::MKDIR` names it again.
const EEXIST: i32 = 17;

/// Ensure `name` exists as a subtree under `fs_ep`'s root, as `identity_provisioner.rs`'s
/// `mkdir_home` does: [`filesystem_contract::fs::MKDIR`] with [`filesystem_contract::dir::ALL`] and
/// the identity string as the name (DECISIONS §117). The FS root is the whole test binary's shared
/// fixture and another suite may have made the name already, so `EEXIST` is tolerated.
///
/// `login` never creates a subtree (§117: provision-time creation, never at login), so this is
/// what lets `wired`'s instance attenuate `alice` and `bob` at all.
fn ensure_home_subtree(fs_ep: sched::RendezvousId, fs_page_frame: u64, name: &[u8]) {
    // SAFETY: `fs_page_frame` is the file service's own shared page, already wired and idle (no client
    // exists yet at the point `wired` calls this, before the login service or any test client is
    // spawned), so writing into it directly and then calling through `fs_ep` is the same shape
    // `identity_provisioner_service.rs`'s own `provision` uses for its request pages.
    let page = unsafe {
        core::slice::from_raw_parts_mut(
            mmu::phys_to_virt(fs_page_frame) as *mut u8,
            filesystem_contract::PAGE,
        )
    };
    page[..name.len()].copy_from_slice(name);
    let r = sched::ipc_call(
        fs_ep,
        [
            filesystem_contract::fs::req(
                filesystem_contract::fs::MKDIR,
                filesystem_contract::fs::ROOT,
                name.len() as u64,
            ),
            filesystem_contract::dir::ALL,
        ],
    );
    match filesystem_contract::reply_errno(r[0] as i64) {
        Some(errno) => assert_eq!(
            errno,
            EEXIST,
            "MKDIR of {:?} failed for a reason other than already existing",
            core::str::from_utf8(name),
        ),
        // A fresh handle; close it best-effort, as `mkdir_home` does.
        None => {
            let _ = sched::ipc_call(
                fs_ep,
                [
                    filesystem_contract::fs::req(filesystem_contract::fs::CLOSE, r[0], 0),
                    0,
                ],
            );
        }
    }
}

/// Write the owner's run-unvouched list (`login_contract::RUN_UNVOUCHED_LIST`, DECISIONS §221 (the
/// boot prompt is the owner's console), ruling 2) at the shared file service's root; `None`
/// removes it (no list lists nobody). Called between logins, so nothing else is mid-request on the
/// file page. Every test that depends on the list sets it first: test order is not a contract.
fn set_run_unvouched_list(list: Option<&[u8]>) {
    fs_service::set_root_list(login_contract::RUN_UNVOUCHED_LIST, list);
}

/// Wire the whole system once: entropy, `credential_tests::provisioned`'s credential store
/// (`alice`, `bob` and `carol`), the file service and the login service. `alice` and `bob` get a
/// home subtree; `carol` deliberately does not, for
/// `login_denies_an_authenticated_identity_with_no_provisioned_subtree`.
///
/// Memoized, because the construction budget is never reclaimed (`components/src/login.rs`'s BUGS).
/// Returns `None` with no virtio-rng or no RedoxFS disk, both legitimate configurations, so a
/// caller skips rather than fails.
pub(super) fn wired() -> Option<ls::Wiring> {
    use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    static DONE: AtomicBool = AtomicBool::new(false);
    static OK: AtomicBool = AtomicBool::new(false);
    static SAVED: [AtomicU64; 6] = [
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
    ];

    if !DONE.load(Ordering::Acquire) {
        let result = (|| {
            let (cred_wiring, _, _) = credential_tests::provisioned()?;
            let (fs_ep, fs_page_frame) =
                fs_service::root_directory(fs_service::blk_server_image(), redoxfs_server_image())?;
            ensure_home_subtree(fs_ep, fs_page_frame, b"alice");
            ensure_home_subtree(fs_ep, fs_page_frame, b"bob");
            // A fresh machine's manifest, so no test here meets a durable session.
            fs_service::set_root_list(timetable_store::MANIFEST_FILE_NAME, None);
            install_scheduled_program();
            let login_image = program("login").expect("no login program in the initial ramdisk");
            let w = ls::start(
                login_image,
                cred_wiring.verify,
                cred_wiring.verify_page_frame,
                fs_ep,
                fs_page_frame,
                CONSTRUCTION_PAGES + DURABLE_UT_PAGES,
                true,
            );
            Some(w)
        })();

        if let Some(w) = &result {
            SAVED[0].store(w.request, Ordering::Relaxed);
            SAVED[1].store(w.result, Ordering::Relaxed);
            SAVED[2].store(w.audit, Ordering::Relaxed);
            SAVED[3].store(w.term_ep, Ordering::Relaxed);
            SAVED[4].store(w.run_unvouched, Ordering::Relaxed);
            SAVED[5].store(w.tid, Ordering::Relaxed);
            OK.store(true, Ordering::Release);
        }
        DONE.store(true, Ordering::Release);
    }
    if !OK.load(Ordering::Acquire) {
        return None;
    }
    Some(ls::Wiring {
        request: SAVED[0].load(Ordering::Relaxed),
        result: SAVED[1].load(Ordering::Relaxed),
        audit: SAVED[2].load(Ordering::Relaxed),
        term_ep: SAVED[3].load(Ordering::Relaxed),
        run_unvouched: SAVED[4].load(Ordering::Relaxed),
        tid: SAVED[5].load(Ordering::Relaxed),
    })
}

/// The name `redoxfs_server`'s image travels under, on the same terms `dir_capability_tests` reads it:
/// present only when `test` built the FS server, absent for a plain interactive run.
pub(super) fn redoxfs_server_image() -> &'static [u8] {
    program("redoxfs_server").expect("no redoxfs_server program in the initial ramdisk")
}

/// Free the terminal directly, without spawning a client.
///
/// Every successful login claims the single shared terminal, and `wired()` shares one `login`
/// across the file, so each test that logs in calls this first as an idempotent reset: `LOGOUT` is
/// answered `LOGGED_OUT` whether or not anything was held. A raw front-door exchange, because
/// freeing the terminal needs no identity and carries no secret.
pub(super) fn free_terminal(w: &ls::Wiring) {
    sched::ipc_send(w.request, [login_contract::logout_word(), 0, 0]);
    let r = sched::ipc_receive(w.result);
    assert_eq!(
        r[0],
        login_contract::LOGGED_OUT,
        "login_contract::logout_word on the front door was not answered LOGGED_OUT",
    );
}

/// A correct identity and secret produce a directory capability and a budget that work: a real
/// `READDIR` through the minted caretaker and a real page retyped from the budget. Neither is asked
/// to match the fixture's contents, a fact about the image (see `login_test_client.rs`).
#[test_case]
fn login_grants_a_working_capability_set_to_the_identity_it_verified() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    // No owner's list: `login` must give the run-unvouched capability to nobody (DECISIONS §221
    // ruling 2). Checked on this login, since every extra run costs later tests free frames.
    set_run_unvouched_list(None);
    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");
    let r = ls::client(cli, &w, ls::LOGIN, ALICE, ALICE);
    assert_eq!(
        r[0],
        ls::RPT_OK,
        "a correct identity and secret were not authenticated",
    );
    assert_eq!(
        r[1] & ls::F_RUN_UNVOUCHED_ANNOUNCED,
        0,
        "with no owner's list, alice was announced the run-unvouched capability",
    );
    assert_eq!(
        r[1] & ls::F_DIR_WORKS,
        ls::F_DIR_WORKS,
        "the delegated directory capability did not answer a real READDIR",
    );
    assert_eq!(
        r[1] & ls::F_BUDGET_WORKS,
        ls::F_BUDGET_WORKS,
        "the delegated budget could not retype a page",
    );
    // `login`'s `send(AUDIT, ...)` blocks until received, and the one shared service serves no
    // later login until it is, so every successful login must be drained.
    let a = sched::ipc_receive(w.audit);
    assert_eq!(
        a[0],
        login_contract::ATTRIBUTED,
        "no attribution record followed a successful login",
    );
}

/// A real identity with the wrong secret is denied, and nothing follows the refusal.
/// `login_test_client` never calls `RECEIVE_CAP` after a refusal, so a capability sent anyway
/// leaves the role short of its report and the test hangs: the honest failure for a promise broken
/// at the sender.
#[test_case]
fn login_denies_a_wrong_secret_and_sends_nothing_further() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");
    let r = ls::client(cli, &w, ls::LOGIN, ALICE, WRONG);
    assert_eq!(r[0], ls::RPT_DENIED, "a wrong secret was not refused",);
}

/// Two principals get two independently working channels, correctly attributed.
///
/// `alice` and `bob` each get a working capability set, and the audit trail names, in order, which
/// sequence number is whose: DECISIONS §109's property made checkable. It does not prove the
/// channels are distinct objects; that rests on retyping from a freshly split region always
/// yielding a new object, a primitive not re-derived here.
#[test_case]
fn two_different_identities_get_independently_working_channels_and_correct_attribution() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    // The owner's list names bob and not alice (DECISIONS §221 ruling 2), so one read of one file
    // must answer both ways.
    set_run_unvouched_list(Some(b"bob\n"));
    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");

    let r_alice = ls::client(cli, &w, ls::LOGIN, ALICE, ALICE);
    assert_eq!(r_alice[0], ls::RPT_OK, "alice was not authenticated");
    let a_alice = sched::ipc_receive(w.audit);
    assert_eq!(
        a_alice[0],
        login_contract::ATTRIBUTED,
        "no attribution record followed alice's login",
    );
    assert_eq!(
        a_alice[2],
        login_contract::identity_hint(b"alice"),
        "the attribution record named the wrong identity for the first channel",
    );
    // Free the terminal alice's login claimed.
    free_terminal(&w);

    let r_bob = ls::client(cli, &w, ls::LOGIN, BOB, BOB);
    assert_eq!(r_bob[0], ls::RPT_OK, "bob was not authenticated",);
    let a_bob = sched::ipc_receive(w.audit);
    assert_eq!(
        a_bob[0],
        login_contract::ATTRIBUTED,
        "no attribution record followed bob's login",
    );
    assert_eq!(
        a_bob[2],
        login_contract::identity_hint(b"bob"),
        "the attribution record named the wrong identity for the second channel",
    );

    assert!(
        a_bob[1] > a_alice[1],
        "the second channel's sequence number ({}) did not follow the first's ({})",
        a_bob[1],
        a_alice[1],
    );

    for (label, r) in [("alice", r_alice), ("bob", r_bob)] {
        assert_eq!(
            r[1] & ls::F_DIR_WORKS,
            ls::F_DIR_WORKS,
            "{label}'s directory capability did not work",
        );
        assert_eq!(
            r[1] & ls::F_BUDGET_WORKS,
            ls::F_BUDGET_WORKS,
            "{label}'s budget did not work",
        );
    }
    assert_eq!(
        r_alice[1] & ls::F_RUN_UNVOUCHED_ANNOUNCED,
        0,
        "alice is not on the owner's list and was announced the run-unvouched capability",
    );
    let listed = ls::F_RUN_UNVOUCHED_ANNOUNCED | ls::F_RUN_UNVOUCHED_NOT_GRANTABLE;
    assert_eq!(
        r_bob[1] & listed,
        listed,
        "bob is on the owner's list and was not given a run-unvouched capability he cannot \
         pass on",
    );
    set_run_unvouched_list(None);
}

/// Two clients reaching the front door together get independent channels, and neither observes the
/// other's secret (milestone 49's channel-per-client update, `components/src/login.rs`'s BUGS).
///
/// `ls::spawn_client` returns as soon as a run is spawned, so both roles race to `CONNECT`.
/// `login` serves one to completion before accepting the other: order, not shared state. The
/// terminal has one holder, so the loser gets `NO_TERMINAL` before its credential is relayed
/// (`serve_login` checks first, so the refusal is no oracle about an identity). The winner's
/// channel works and its one audit record names it, never the loser and never a garbled value.
#[test_case]
fn two_clients_connecting_together_get_independent_channels_and_neither_observes_the_others_secret()
{
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");

    // Both spawned before either is waited on: the scheduler picks the winner.
    let alice_report = ls::spawn_client(cli, &w, ls::LOGIN, ALICE, ALICE);
    let bob_report = ls::spawn_client(cli, &w, ls::LOGIN, BOB, BOB);

    // `login` accepts the loser's queued `CONNECT` only after this drain of the winner's blocking
    // `AUDIT` send. Exactly one record follows: the loser's refusal never reaches `mint`.
    let a = sched::ipc_receive(w.audit);
    assert_eq!(
        a[0],
        login_contract::ATTRIBUTED,
        "no attribution record followed the winning login",
    );

    let r_alice = ls::wait_client(alice_report);
    let r_bob = ls::wait_client(bob_report);

    let (winner_label, winner, loser_label, loser) = if r_alice[0] == ls::RPT_OK {
        ("alice", r_alice, "bob", r_bob)
    } else {
        ("bob", r_bob, "alice", r_alice)
    };
    assert_eq!(
        winner[0],
        ls::RPT_OK,
        "neither alice ({:#x}) nor bob ({:#x}) was authenticated",
        r_alice[0],
        r_bob[0],
    );
    assert_eq!(
        loser[0],
        ls::RPT_NO_TERMINAL,
        "{loser_label} was refused for a reason other than the terminal already being held \
         (alice: {:#x}, bob: {:#x})",
        r_alice[0],
        r_bob[0],
    );
    assert_eq!(
        winner[1] & ls::F_DIR_WORKS,
        ls::F_DIR_WORKS,
        "{winner_label}'s directory capability did not work",
    );
    assert_eq!(
        winner[1] & ls::F_BUDGET_WORKS,
        ls::F_BUDGET_WORKS,
        "{winner_label}'s budget did not work",
    );

    let expected_hint = login_contract::identity_hint(winner_label.as_bytes());
    assert_eq!(
        a[2], expected_hint,
        "the attribution record named the wrong identity for the winning channel \
         (winner was {winner_label})",
    );

    free_terminal(&w);
}

/// The login service serves past the old capability-table ceiling of eight successful logins.
///
/// `login`'s table has sixteen slots (`kernel::cap::CAPABILITY_TABLE_SLOTS`), eight spent at rest,
/// and `mint` must drop its copy of the caretaker's construction region, or the ninth correct login
/// is answered [`login_contract::DENIED`]. Six logins here, alternating `alice` and `bob`, plus the
/// earlier tests' three make nine, each proved by a real `READDIR` and retype. Six, not nine,
/// because every kept login is a permanent charge against `SUITE_PAGE_FRAME_BUDGET`.
#[test_case]
fn the_login_service_serves_past_the_old_capability_table_ceiling() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");

    const ATTEMPTS: usize = 6;
    for i in 0..ATTEMPTS {
        // Alternating identities, so a leak on one identity's path cannot hide.
        let who = if i % 2 == 0 { ALICE } else { BOB };
        let r = ls::client(cli, &w, ls::LOGIN, who, who);
        assert_eq!(
            r[0],
            ls::RPT_OK,
            "login {i} of {ATTEMPTS} in this test (the 4th through 9th against this shared \
             instance, past the old eight-login capability table ceiling) was not authenticated; a real \
             password was answered as though it were wrong",
        );
        assert_eq!(
            r[1] & ls::F_DIR_WORKS,
            ls::F_DIR_WORKS,
            "login {i}'s directory capability did not work",
        );
        assert_eq!(
            r[1] & ls::F_BUDGET_WORKS,
            ls::F_BUDGET_WORKS,
            "login {i}'s budget did not work",
        );
        // Drain the blocking `AUDIT` send so the service can serve the next login.
        let a = sched::ipc_receive(w.audit);
        assert_eq!(
            a[0],
            login_contract::ATTRIBUTED,
            "no attribution record followed login {i}",
        );
        // Free the terminal so login {i+1} is not refused NO_TERMINAL.
        free_terminal(&w);
    }
}

/// Each identity is attenuated to its own provisioned subtree (DECISIONS §117), not the shared
/// fixture and not another identity's.
///
/// `alice` and `bob` each `CREATE` a marker naming themselves ([`ls::WRITE_MARKER`]) and confirm
/// [`filesystem_fixture::tree::INNER`] is absent, so neither landed in the shared fixture tree.
/// `alice` then logs in again ([`ls::READ_MARKER`]) and must read her own marker: one identity's
/// sessions share a subtree, and it is not `bob`'s, whose write would otherwise have replaced it.
/// `identity_provisioning_tests` proves the tool makes the subtree; this proves `login` finds it.
#[test_case]
fn login_scopes_each_identity_to_its_own_provisioned_subtree() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");

    let r_alice = ls::client(cli, &w, ls::WRITE_MARKER, ALICE, ALICE);
    assert_eq!(r_alice[0], ls::RPT_OK, "alice was not authenticated");
    assert_eq!(
        r_alice[1] & ls::F_MARKER_WRITTEN,
        ls::F_MARKER_WRITTEN,
        "alice's marker was not created and written",
    );
    assert_eq!(
        r_alice[1] & ls::F_NOT_SHARED_SUBTREE,
        ls::F_NOT_SHARED_SUBTREE,
        "alice's granted directory carries the old shared fixture's own file \
         (filesystem_fixture::tree::INNER), so it is not a subtree of alice's own",
    );
    let a_alice = sched::ipc_receive(w.audit);
    assert_eq!(
        a_alice[0],
        login_contract::ATTRIBUTED,
        "no attribution record followed alice's marking login",
    );
    // Free the terminal alice's login claimed.
    free_terminal(&w);

    let r_bob = ls::client(cli, &w, ls::WRITE_MARKER, BOB, BOB);
    assert_eq!(r_bob[0], ls::RPT_OK, "bob was not authenticated");
    assert_eq!(
        r_bob[1] & ls::F_MARKER_WRITTEN,
        ls::F_MARKER_WRITTEN,
        "bob's marker was not created and written",
    );
    assert_eq!(
        r_bob[1] & ls::F_NOT_SHARED_SUBTREE,
        ls::F_NOT_SHARED_SUBTREE,
        "bob's granted directory carries the old shared fixture's own file \
         (filesystem_fixture::tree::INNER), so it is not a subtree of bob's own",
    );
    let a_bob = sched::ipc_receive(w.audit);
    assert_eq!(
        a_bob[0],
        login_contract::ATTRIBUTED,
        "no attribution record followed bob's marking login",
    );
    free_terminal(&w);

    let r_check = ls::client(cli, &w, ls::READ_MARKER, ALICE, ALICE);
    assert_eq!(
        r_check[0],
        ls::RPT_OK,
        "alice's second, independent login was not authenticated",
    );
    assert_eq!(
        r_check[1] & ls::F_DIR_WORKS,
        ls::F_DIR_WORKS,
        "alice's second login's directory capability did not work",
    );
    assert_eq!(
        r_check[2],
        login_contract::identity_hint(b"alice"),
        "alice's second, independent login did not read back alice's own marker: either it did \
         not land in alice's subtree, or bob's write clobbered it, which is exactly the \
         everyone-shares-one-subtree bug this milestone fixes",
    );
    let a_check = sched::ipc_receive(w.audit);
    assert_eq!(
        a_check[0],
        login_contract::ATTRIBUTED,
        "no attribution record followed alice's second login",
    );
    free_terminal(&w);
}

/// An authenticated identity with no provisioned subtree is refused, indistinguishably from a wrong
/// password.
///
/// `carol` has a real credential and no home subtree, as if `identity_provisioner` never ran for
/// her. `mint`'s caretaker construction meets an `OPENDIR` refusal, folded into
/// [`login_contract::DENIED`] so a caller cannot tell "no home" from "wrong password" across
/// attempts (`components/src/login.rs`'s BUGS). No audit record follows: `AUDIT` is success-only.
#[test_case]
fn login_denies_an_authenticated_identity_with_no_provisioned_subtree() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");
    // An honest login, refused for the subtree rather than the secret.
    let r = ls::client(cli, &w, ls::LOGIN, CAROL, CAROL);
    assert_eq!(
        r[0],
        ls::RPT_DENIED,
        "carol's real credential, with no provisioned subtree, was not refused",
    );
}

/// `login`'s caretaker measurement matches the real table, and a tampered copy would be refused.
///
/// `login`'s `_start` calls [`measured_boot::verify_in_manifest`] on `fs_subtree_caretaker` against
/// [`measured_boot::PROGRAM_MEASUREMENTS`] before building a caretaker (`components/src/login.rs`'s
/// BUGS, "Resolved, 2026-08-24"). The booted archive is shared and `cargo xtask` packs an agreeing
/// table, so a live refusal cannot be staged; this runs the same call on the real bytes, a tampered
/// copy and an unnamed program. `measured_boot_tests` covers only boot-time components, and this
/// caretaker is not one.
#[test_case]
fn logins_caretaker_measurement_matches_the_real_table_and_a_tampered_one_would_be_refused() {
    let table = program(measured_boot::PROGRAM_MEASUREMENTS)
        .and_then(|b| core::str::from_utf8(b).ok())
        .expect("the measurement table is missing or is not text");
    let bytes = program("fs_subtree_caretaker").expect("the archive has no 'fs_subtree_caretaker'");

    assert_eq!(
        measured_boot::verify_in_manifest(table, "fs_subtree_caretaker", bytes),
        Ok(()),
        "the caretaker every login builds a session from does not match the table login's own \
         _start now consults; login would refuse every login rather than serve one",
    );

    let mut h = measured_boot::Sha256::new();
    h.update(&[bytes[0] ^ 1]);
    h.update(&bytes[1..]);
    let tampered = h.finalize();
    assert_ne!(
        measured_boot::expected_in_manifest(table, "fs_subtree_caretaker"),
        Some(tampered),
        "a tampered fs_subtree_caretaker still satisfied the table",
    );
    assert_eq!(
        measured_boot::verify_in_manifest(table, "fs_subtree_caretaker", b"not the real caretaker"),
        Err(measured_boot::VerifyError::Mismatch),
        "login's own check would wave through a caretaker whose bytes do not match the table",
    );
    assert_eq!(
        measured_boot::verify_in_manifest(table, "no-such-caretaker", bytes),
        Err(measured_boot::VerifyError::Unmeasured),
        "login's own check would vouch for a name the table does not mention",
    );
}

/// Caretaker teardown reclaims a full session's worth of memory (`components/src/login.rs`'s BUGS,
/// "Resolved").
///
/// After the tests above, a few hundred pages of [`CONSTRUCTION_PAGES`] remain, and ten sessions
/// (2560 pages) can only pass if each logs fully out (both `MemoryRegion`s destroyed) first.
/// Each iteration asserts a real `READDIR` and `RETYPE`, both `DESTROY`s, and both capabilities
/// dead afterward. Ten in a row also catches a per-connect leak of `login`'s sixteen-slot table,
/// invisible to a service that never serves two logins (BUGS, "Resolved, 2026-08-26").
#[test_case]
fn caretaker_teardown_reclaims_a_full_session_worth_of_memory() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");

    const ATTEMPTS: usize = 10;
    for i in 0..ATTEMPTS {
        let r = ls::client(cli, &w, ls::LOGOUT, ALICE, ALICE);
        assert_eq!(
            r[0],
            ls::RPT_OK,
            "login {i} of {ATTEMPTS} (the teardown role) was not authenticated",
        );
        assert_eq!(
            r[1] & ls::F_DIR_WORKS,
            ls::F_DIR_WORKS,
            "login {i}'s directory capability did not work before teardown",
        );
        assert_eq!(
            r[1] & ls::F_BUDGET_WORKS,
            ls::F_BUDGET_WORKS,
            "login {i}'s budget did not work before teardown",
        );
        assert_eq!(
            r[1] & ls::F_TEARDOWN_OK,
            ls::F_TEARDOWN_OK,
            "login {i}'s logout ticket did not destroy the caretaker's construction region; the \
             client waited {} us for the refusal to clear, against its own ceiling of {} us. A \
             number near that ceiling means the caretaker never died, which is a kernel bug and \
             not this host being slow; a number far under it means the client stopped waiting \
             early, which is the defect that ceiling replaced. See \
             fixtures/src/login_test_client.rs's `destroy_with_retry`.",
            r[2],
            ls::DESTROY_WAIT_MICROS,
        );
        assert_eq!(
            r[1] & ls::F_DEAD_AFTER_TEARDOWN,
            ls::F_DEAD_AFTER_TEARDOWN,
            "login {i}'s directory capability still answered a READDIR after its region was destroyed",
        );
        assert_eq!(
            r[1] & ls::F_BUDGET_TEARDOWN_OK,
            ls::F_BUDGET_TEARDOWN_OK,
            "login {i}'s own budget did not destroy",
        );
        assert_eq!(
            r[1] & ls::F_BUDGET_DEAD_AFTER_TEARDOWN,
            ls::F_BUDGET_DEAD_AFTER_TEARDOWN,
            "login {i}'s budget still answered a RETYPE after its own destroy",
        );
        // Drain the blocking `AUDIT` send.
        let a = sched::ipc_receive(w.audit);
        assert_eq!(
            a[0],
            login_contract::ATTRIBUTED,
            "no attribution record followed login {i}",
        );
        // Free the terminal after draining `AUDIT`, not before; `LOGOUT` does not free it, since
        // that would deadlock against this drain (`login_test_client.rs`).
        free_terminal(&w);
    }
}

/// The terminal is handed out once, and a concurrent second login is denied until logout
/// (`design/roadmap/0049-users-and-attribution.md`'s BUGS).
///
/// 1. `alice` ([`ls::HOLD_TERMINAL`]) gets the terminal, proved by a word on `w.term_ep`, and ends
///    her session without freeing it. 2. `bob`'s correct credential gets
///    [`login_contract::NO_TERMINAL`], not `DENIED`. 3. [`ls::FREE_TERMINAL`] is answered
///    [`login_contract::LOGGED_OUT`]. 4. `alice` gets the terminal again: freeing it is what makes
///    it available to the next login. Nothing permanent is charged: each login logs out or is
///    refused before `mint`.
#[test_case]
fn login_hands_out_the_terminal_once_and_denies_a_concurrent_second_login_until_logout() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");

    // Step 1. `spawn_client`, not `client`: the role blocks in `send(term_ep, ...)` until this
    // test receives on `w.term_ep`, so waiting on the report first would deadlock.
    let report1 = ls::spawn_client(cli, &w, ls::HOLD_TERMINAL, ALICE, ALICE);
    let a1 = sched::ipc_receive(w.audit);
    assert_eq!(
        a1[0],
        login_contract::ATTRIBUTED,
        "no attribution record followed alice's first terminal login",
    );
    let term_msg = sched::ipc_receive(w.term_ep);
    assert_eq!(
        term_msg[0],
        ls::TERM_MAGIC,
        "the delegated terminal capability did not deliver a real message to a real receiver",
    );
    let r1 = ls::wait_client(report1);
    assert_eq!(
        r1[0],
        ls::RPT_OK,
        "alice's first terminal login was not authenticated"
    );
    for (bit, what) in [
        (ls::F_DIR_WORKS, "the directory capability"),
        (ls::F_BUDGET_WORKS, "the budget"),
        (ls::F_TERM_WORKS, "the terminal capability"),
        (ls::F_TEARDOWN_OK, "the logout ticket's own DESTROY"),
        (
            ls::F_DEAD_AFTER_TEARDOWN,
            "the directory's post-teardown refusal",
        ),
        (ls::F_BUDGET_TEARDOWN_OK, "the budget's own DESTROY"),
        (
            ls::F_BUDGET_DEAD_AFTER_TEARDOWN,
            "the budget's post-teardown refusal",
        ),
    ] {
        assert_eq!(
            r1[1] & bit,
            bit,
            "alice's first terminal login: {what} did not work"
        );
    }

    // Step 2: bob, a real credential, refused purely because the terminal is spoken for.
    let r2 = ls::client(cli, &w, ls::LOGIN, BOB, BOB);
    assert_eq!(
        r2[0],
        ls::RPT_NO_TERMINAL,
        "bob's real credential was not refused NO_TERMINAL while alice still held the terminal",
    );

    // Step 3: free it.
    let r3 = ls::client(cli, &w, ls::FREE_TERMINAL, NONE, NONE);
    assert_eq!(
        r3[0],
        ls::RPT_LOGGED_OUT,
        "login_contract::logout_word on the front door was not answered LOGGED_OUT",
    );

    // Step 4: alice again, a second, independent time, and the terminal is available once more.
    let report4 = ls::spawn_client(cli, &w, ls::HOLD_TERMINAL, ALICE, ALICE);
    let a4 = sched::ipc_receive(w.audit);
    assert_eq!(
        a4[0],
        login_contract::ATTRIBUTED,
        "no attribution record followed alice's second terminal login",
    );
    let term_msg2 = sched::ipc_receive(w.term_ep);
    assert_eq!(
        term_msg2[0],
        ls::TERM_MAGIC,
        "the terminal was not delegated again after being freed",
    );
    let r4 = ls::wait_client(report4);
    assert_eq!(
        r4[0],
        ls::RPT_OK,
        "alice's second terminal login (after logout freed it) was not authenticated",
    );
    assert_eq!(
        r4[1] & ls::F_TERM_WORKS,
        ls::F_TERM_WORKS,
        "the re-delegated terminal capability did not work",
    );
}

/// A listed identity's session gets the run-unvouched capability and cannot pass it on (DECISIONS
/// §219 (how the shell names an installed program to the spawner) gate D2, milestone 198 (a package
/// manager) rung 3a; DECISIONS §221 (the boot prompt is the owner's console), ruling 2).
///
/// The sixth capability names the endpoint the spawner gave `login` and lacks `GRANT`, so it is a
/// presentation, not a token. The only place the delivery runs; `script/swish-check` proves the
/// progenitor's half. Falsified by hand (aarch64, 2026-09-26): delegating it `WRITE | GRANT` turned
/// this red, the client's `SEND_CAP` arriving in place of the report.
#[test_case]
fn login_hands_a_listed_session_the_run_unvouched_capability_and_it_cannot_be_passed_on() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    set_run_unvouched_list(Some(b"# the owner's list\nalice\n"));
    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");
    // `spawn_client`/`wait_client` for the terminal test's reason: the role blocks in its `send`
    // on the sixth capability until this test takes the word.
    let report = ls::spawn_client(cli, &w, ls::PRESENT_RUN_UNVOUCHED, ALICE, ALICE);
    let a = sched::ipc_receive(w.audit);
    assert_eq!(
        a[0],
        login_contract::ATTRIBUTED,
        "no attribution record followed the login",
    );
    let presented = sched::ipc_receive(w.run_unvouched);
    assert_eq!(
        presented[0],
        ls::RUN_UNVOUCHED_MAGIC,
        "the session's run-unvouched capability did not reach the endpoint login was given",
    );
    let r = ls::wait_client(report);
    assert_eq!(
        r[0],
        ls::RPT_OK,
        "no OK report: alice was not authenticated, or the session's run-unvouched capability \
         carried GRANT and its delegation arrived in place of the report",
    );
    for (bit, what) in [
        (
            ls::F_RUN_UNVOUCHED_NOT_GRANTABLE,
            "the run-unvouched capability could be delegated onward",
        ),
        (
            ls::F_RUN_UNVOUCHED_WORKS,
            "the run-unvouched capability did not deliver a word",
        ),
        // The session comes home, so nothing this test built outlives it: left standing, its
        // caretaker starved `timetable_tests` into a hang later in the suite (riscv64, aarch64).
        (ls::F_TEARDOWN_OK, "the logout ticket's own DESTROY"),
        (ls::F_BUDGET_TEARDOWN_OK, "the budget's own DESTROY"),
    ] {
        assert_eq!(r[1] & bit, bit, "{what}");
    }
    free_terminal(&w);
    set_run_unvouched_list(None);
}
