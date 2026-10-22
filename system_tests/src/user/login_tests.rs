use credential_contract::fixture::{ALICE, BOB, CAROL, NONE, WRONG};
use login_service as ls;

use super::*;
use crate::sched;

/// The login service's construction budget (`components/src/login.rs`'s `CONSTRUCTION_UT`).
/// `crate::memory_region::create` reserves this many frames from the boot's free pool when
/// `ls::start` runs, and it is a permanent charge against `kernel::testing::SUITE_PAGE_FRAME_BUDGET`
/// for the logins this suite never tears down.
///
/// The itemized minimum: 128 (`login`'s `OWN_UT_PAGES`) + 32 (`login::CHANNEL_UT_PAGES`) + 16 * 64
/// (a caretaker region for every permanently kept login, the four detaching durable clients
/// included) + 12 * 64 (the ordinary sessions' client budgets) = 1952. Tests that log back out
/// (`caretaker_teardown_reclaims_a_full_session_worth_of_memory` and the terminal test) and refusals
/// that never reach `mint` add nothing permanent, and `connect`'s channels are carved from
/// `CHANNEL_UT_PAGES`, so they always return as the LIFO top.
///
/// The flat sum is not enough: logout cycles and refused logins split and return **out of order**,
/// and a split needs contiguous pages, so LIFO holes strand some slack (as `DURABLE_UT_PAGES`'s
/// budget states). At 2688 the suite ran out at about its 23rd login, seen as `DENIED` on a correct
/// password (`mint`'s `None`, folded into `DENIED`); 3088 runs the whole suite green.
pub(super) const CONSTRUCTION_PAGES: u64 = 3088;

/// **What `login` splits off its construction budget at start-up for durable sessions** (milestone
/// 152): one durable session's budget, `login_contract::durable::BUDGET_PAGES`, the constant `login`
/// splits by. Kept apart from [`CONSTRUCTION_PAGES`] so the account above stays the account it was.
pub(super) const DURABLE_UT_PAGES: u64 = login_contract::durable::BUDGET_PAGES;

/// **Install the one program the durable tests schedule**, as an installer would (milestone 152,
/// Fork 8 ruled D by calef on 2026-09-27, on #1377): a scheduled job runs what the live activation
/// generation names, so the demo's bytes go to `packages/` and a generation naming them to
/// `activation/`. Written once per boot, before either `login` that opens a schedule starts.
///
/// The bytes are `least_authority_demo`'s, from the archive, because the image is the only source
/// this harness has; the **name is not the image's**, because §229 (how a bare name at the prompt
/// reaches an installed program) refuses a package entry under a
/// name the image carries, calef's ruling of 2026-09-27, and this image carries
/// `least_authority_demo`. Installing under the image's own name panicked the whole guest suite at
/// `with_entry`'s `ImageName` refusal the moment the rebase met that ruling, which is the inbound
/// check regression this fixture's rename fixes. `scheduled_demo` is provisional.
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

/// **Ensure `name` exists as a subtree under `fs_ep`'s root, tolerating `EEXIST`** exactly as
/// `identity_provisioner.rs`'s own `mkdir_home` does: the same opcode
/// ([`filesystem_contract::fs::MKDIR`]), the same rights ([`filesystem_contract::dir::ALL`]), and the identity string
/// itself as the name (DECISIONS §117). Issued directly from this test rather than through a
/// spawned `identity_provisioner`, because this suite's fs root is the tree-wide shared fixture
/// (`fs_service`'s own memoized `ensure`, one FS server for the whole kernel test binary): another
/// suite may have already created this name (`identity_provisioning_tests.rs` provisions `alice`
/// too, against its own, separate credential store, sharing only the filesystem), and `EEXIST` is
/// the correct, expected answer then, not a failure.
///
/// This is what makes `wired`'s login instance below able to attenuate `alice` and `bob` to a
/// subtree of their own at all: `login.rs` no longer creates one (DECISIONS §117: provision-time
/// creation, never auto-vivified at login), so something upstream of it must, exactly as a real
/// deployment's `identity_provisioner` would before anyone logs in.
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
        // A fresh directory handle this call minted; close it, best-effort, the same choice
        // `identity_provisioner.rs`'s own `mkdir_home` makes for the same reason (this test has no
        // further use for it, and a leaked handle-table slot in the FS server is not a correctness
        // bug a caller of this helper could observe).
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

/// **Write the owner's run-unvouched list** (`login_contract::RUN_UNVOUCHED_LIST`, DECISIONS §221
/// (the boot prompt is the owner's console), ruling 2) at the root of the shared test file service,
/// as the boot prompt would; `None` removes it, which is the default: no list lists nobody.
///
/// Issued the way [`ensure_home_subtree`] issues its `MKDIR`, directly on the file page, and for
/// its reason: called between logins, when no session is live, so nothing else is mid-request on
/// that page. Every test that depends on the list sets it first, because the file service is one
/// fixture for the whole suite and a test order is not a contract.
fn set_run_unvouched_list(list: Option<&[u8]>) {
    fs_service::set_root_list(login_contract::RUN_UNVOUCHED_LIST, list);
}

/// **Wire the whole system once**: entropy, the credential service (`credential_tests::provisioned`'s
/// own fixture, which already provisions `alice`, `bob` and `carol` among the three family
/// logins design/roadmap/0056-secrets-and-entropy.md names), the file service, and the login service
/// itself. Reusing that fixture rather than provisioning a second store is deliberate: a login-only
/// store would duplicate `credentialer_test_client.rs`'s `PEOPLE` fixture for no reason a reader
/// could point at.
///
/// **`alice` and `bob` are given a home subtree** ([`ensure_home_subtree`]) before the login
/// service starts; `carol` deliberately is not, so a credential that authenticates but has nowhere
/// to attenuate to is a fixture this suite already has on hand rather than one built to order. See
/// DECISIONS §117 and `login.rs`'s own BUGS on why an authenticated identity with no subtree is
/// refused rather than served, and `login_denies_an_authenticated_identity_with_no_provisioned_subtree`
/// below for the test that checks it.
///
/// Memoized on `credential_tests::provisioned`'s own terms and for the same reason: the login
/// service's construction budget is spent and never reclaimed by design in this slice (see
/// `components/src/login.rs`'s BUGS), so a re-wiring test-to-test would exhaust it for nothing.
///
/// `None` when a dependency this depends on is not attached to this run: no virtio-rng
/// (`credential_tests::provisioned`'s own condition) or no RedoxFS disk (`fs_service`'s). Both are
/// legitimate boot configurations this tree supports, so a caller skips rather than fails.
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
            // A fresh machine's manifest: nothing for `login`'s start-up pass to re-derive, so
            // every test here meets a `login` holding no durable session. The start-up pass has
            // its own test and its own `login`.
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

/// **Free the terminal directly, without spawning a client** (milestone 49's terminal update).
///
/// Every successful login now also claims the single, shared terminal
/// (`login_contract::NO_TERMINAL` denies any concurrent second one until it is freed), and `wired()`
/// memoizes **one** login instance across this entire file. Every test below that performs a
/// successful login and does not itself exercise the terminal property calls this first, as a
/// defensive, idempotent reset: `login_contract::LOGOUT` is answered `LOGGED_OUT` whether or not
/// anything was actually held (`login_contract::LOGGED_OUT`'s own doc), so this is safe to call
/// regardless of what a previous test (in whatever order this suite happens to run them) left
/// behind. Issued as a raw front-door exchange rather than through `login_test_client`, since
/// freeing the terminal needs no identity and carries no secret (`login_contract`'s own module docs on
/// why `LOGOUT` travels on the shared front door at all).
pub(super) fn free_terminal(w: &ls::Wiring) {
    sched::ipc_send(w.request, [login_contract::logout_word(), 0, 0]);
    let r = sched::ipc_receive(w.result);
    assert_eq!(
        r[0],
        login_contract::LOGGED_OUT,
        "login_contract::logout_word on the front door was not answered LOGGED_OUT",
    );
}

/// **The headline.** A correct identity and secret produce a directory capability and a budget
/// that actually work: a real `READDIR` through the minted caretaker, and a real page retyped from
/// the minted budget. Neither is asked to match the fixture's exact contents, which is a fact about
/// the image and not about this capability (see `login_test_client.rs`).
#[test_case]
fn login_grants_a_working_capability_set_to_the_identity_it_verified() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    // No owner's list, which is a fresh machine's state: `login` holds the run-unvouched
    // capability and must give it to nobody (DECISIONS §221 (the boot prompt is the owner's
    // console), ruling 2). Checked on this login rather than a login of its own, because every
    // extra run in this suite is paid for by a later test's free frames (`CLIENT_SCRATCH_UT_PAGES`).
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
    // `login`'s `send(AUDIT, ...)` is a blocking rendezvous: it does not return, and the service
    // cannot serve the *next* login, until something receives it. `wired()` memoizes one service
    // instance across this whole suite, so every test that causes a successful login must drain
    // this endpoint or leave the service permanently stuck here for every later test.
    let a = sched::ipc_receive(w.audit);
    assert_eq!(
        a[0],
        login_contract::ATTRIBUTED,
        "no attribution record followed a successful login",
    );
}

/// **The refusal, and the promise that nothing follows it.** A real identity with the wrong secret
/// is denied. `login_test_client` never calls `RECEIVE_CAP` after a refusal; if the service ever
/// sent a capability after a denial anyway, that role would simply never reach its report and this
/// test would hang rather than fail cleanly, which is the honest failure mode for a protocol
/// promise broken at the sender.
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
    // `alice`'s own identity with a secret that is nobody's: the same behaviour as the honest
    // login above, composed from a different credential rather than run as a different arm.
    let r = ls::client(cli, &w, ls::LOGIN, ALICE, WRONG);
    assert_eq!(r[0], ls::RPT_DENIED, "a wrong secret was not refused",);
}

/// **Two principals, two channels, correctly attributed.** `alice` and `bob` both log in and
/// both get a working capability set (this suite's headline test already proves one identity's
/// set works; this proves a second, different identity's does too, independently). The login
/// service's own audit trail then names, in order, which sequence number belongs to which
/// identity, which is DECISIONS §109's property made checkable: a server that establishes a
/// channel and can say who it belongs to.
///
/// **What this does not prove**, and it is named rather than assumed: that the two channels are
/// not secretly the same underlying object. That claim rests on the kernel's own guarantee that
/// retyping from a freshly split region always yields a new object, which is a capability-system
/// primitive this tree relies on everywhere and does not re-derive at this integration test's
/// level. What this test *does* prove is that both channels work independently and that the audit
/// record is honest about which principal established which.
#[test_case]
fn two_different_identities_get_independently_working_channels_and_correct_attribution() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    // **And the owner's list tells them apart** (DECISIONS §221 ruling 2): it names bob and not
    // alice, so one read of one file must answer both ways. Riding these two logins rather than
    // two of its own, for the headline test's reason.
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
    // Free the terminal before bob's login (milestone 49 (users, login, and attribution)'s
    // terminal update): unrelated to the channel-attribution property this test exists to prove,
    // but every successful login now also claims the single, shared terminal.
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

/// **Two clients reaching the front door together get independent channels, and neither observes
/// the other's secret.** Milestone 49's channel-per-client update (`components/src/login.rs`'s own BUGS,
/// "Resolved"): before it, `REQUEST`/`RESULT` and a single shared staging page carried every
/// client's identity and secret for the service's whole life, so two callers reaching the front door
/// close together could corrupt or observe each other's presented credential. `ls::spawn_client`
/// (unlike `ls::client`) returns the instant a run is spawned, without waiting for it to run at
/// all, so calling it twice before waiting on either puts both roles genuinely in flight together,
/// racing each other to `CONNECT` on the kernel's own schedule: this is the scenario the old design's
/// hazard needed, and the new one's whole point is that it no longer matters who wins the race.
///
/// `login` is still one thread and answers `CONNECT` one at a time, so whichever role is served
/// first is served to completion, private channel and all, before the second is even accepted; that
/// is service *order*, the module docs' own distinction, not shared state.
///
/// **Rewritten for milestone 49's terminal update.** Before it, both concurrent logins succeeded
/// and this test's own property was that the two attribution records named exactly one alice and
/// one bob, never crossed. Now only one of them *can* succeed: the single, shared terminal has
/// exactly one holder at a time, and the loser is refused `login_contract::NO_TERMINAL` before its
/// identity or secret is even relayed to the credential service (`login.rs`'s own `serve_login`
/// checks this first, deliberately, so the refusal cannot become a timing or outcome oracle about a
/// specific identity). The property this test can still prove, and does: whichever one wins, its
/// own channel works and its own attribution record names *it*, never the other identity and never
/// a garbled third value -- a shared, corrupted staging page (the pre-fix hazard this test was
/// originally written to catch) could have produced either.
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

    // Both spawned before either is waited on: this is the race. Which role's CONNECT actually
    // reaches `login`'s front door first is the kernel scheduler's call, not this test's, and the
    // property under test holds either way.
    let alice_report = ls::spawn_client(cli, &w, ls::LOGIN, ALICE, ALICE);
    let bob_report = ls::spawn_client(cli, &w, ls::LOGIN, BOB, BOB);

    // Whichever role wins the race, `login` cannot accept the *other*'s `CONNECT` until it finishes
    // fully serving the first: its own `send(AUDIT, ...)` is a blocking rendezvous (every other
    // successful-login test in this file drains it for the same reason), so this drain is what lets
    // the second role make any progress at all -- reach *its own* verdict, `NO_TERMINAL` now rather
    // than `OK` -- not merely bookkeeping. The loser's `CONNECT` sits queued on the front door until
    // this happens. Exactly one audit record follows, unlike before this update: the loser's
    // refusal never reaches `mint`, so it never reaches `AUDIT` either.
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

    // The property under test, stated positively: the one attribution record names exactly the
    // identity that won, never the other one and never a garbled third value.
    let expected_hint = login_contract::identity_hint(winner_label.as_bytes());
    assert_eq!(
        a[2], expected_hint,
        "the attribution record named the wrong identity for the winning channel \
         (winner was {winner_label})",
    );

    free_terminal(&w);
}

/// **Nothing else would have caught this**: `login`'s own capability table has sixteen slots
/// (`kernel::cap::CAPABILITY_TABLE_SLOTS`), eight are spent at rest (`REQUEST`..`AUDIT` plus its own scratch
/// untyped), and before `components/src/login.rs`'s `mint` learned to drop its own copy of the
/// caretaker's construction region, that region's capability was never freed on a successful
/// login. That left room for exactly eight successful logins ever; a ninth, correctly
/// authenticated, was silently answered [`login_contract::DENIED`], indistinguishable from a wrong
/// password, at a far tighter ceiling than the memory bound `components/src/login.rs`'s BUGS documents.
///
/// This performs **six** successful logins against one service instance, on top of the three the
/// headline test and the two-identity test above already performed against the same memoized
/// instance (`wired()`): nine in total, one past that old eight-login ceiling. It asserts every one
/// of its own six actually worked (not merely that the verdict was `OK`): a real `READDIR` through
/// the minted directory and a real page retyped from the minted budget, the same proof the headline
/// test above asks for. Alternates `alice` and `bob` rather than repeating one identity, so a
/// regression that only leaked on one particular identity's path would not hide from this test by
/// accident.
///
/// Six rather than nine performed here directly: a real login's `CARETAKER_REGION_PAGES` and
/// `CLIENT_BUDGET_PAGES` are permanently unreclaimable memory (see `components/src/login.rs`'s BUGS), so
/// every attempt this test adds is a permanent charge against `kernel::testing::SUITE_PAGE_FRAME_BUDGET`
/// and not merely against `CONSTRUCTION_PAGES`. Reusing the other two tests' three logins rather
/// than repeating them here is what keeps that charge to what actually proves the fix.
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
        // Alternating identities, which is what this loop always meant and can now say: one
        // behaviour, two credentials, rather than two roles that happened to run the same code.
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
        // Drain the attribution record so the service is free to serve the next login (the
        // headline test's own note: `send(AUDIT, ...)` is a blocking rendezvous).
        let a = sched::ipc_receive(w.audit);
        assert_eq!(
            a[0],
            login_contract::ATTRIBUTED,
            "no attribution record followed login {i}",
        );
        // Free the terminal so login {i+1} is authenticated rather than refused NO_TERMINAL
        // (milestone 49's terminal update: every successful login also claims the single, shared
        // terminal).
        free_terminal(&w);
    }
}

/// **DECISIONS §117: each identity is attenuated to its own provisioned subtree, not the old
/// shared fixture and not another identity's.**
///
/// `alice` and `bob` each log in and, through the directory capability `login` delegated,
/// `CREATE` a marker file naming themselves ([`ls::WRITE_MARKER`], run once per identity); both
/// also confirm
/// [`filesystem_fixture::tree::INNER`] is absent, which is this suite's own proof (not merely a
/// stated intent) that neither landed in the old shared subtree every identity used to be
/// attenuated to before this milestone. `alice` then logs in a **second, independent** time
/// ([`ls::READ_MARKER`]) and reads the marker back: it must read `alice`'s own,
/// which is the property under test stated positively: the *same* identity's two sessions land in
/// the *same* subtree, and it is not `bob`'s (had the old bug still been present, `bob`'s
/// later write would have overwritten `alice`'s marker in the one subtree they would have shared,
/// and this final read would come back `bob`'s instead).
///
/// `wired`'s own `ensure_home_subtree` issues the identical `filesystem_contract::fs::MKDIR` request
/// `identity_provisioner.rs`'s own `mkdir_home` does (same opcode, same
/// [`filesystem_contract::dir::ALL`] rights, the identity string as the name); that milestone's own suite
/// (`identity_provisioning_tests.rs`, `provisioning_creates_a_working_credential_and_a_real_subtree`)
/// already proves the *tool* produces a real, descendable subtree end to end, so this test's job is
/// the other half: that `login`, given one, finds and attenuates to the *right* one.
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
    // Free the terminal before bob's own login (milestone 49's terminal update): unrelated to
    // the per-identity subtree property this test exists to prove, but every successful login now
    // also claims the single, shared terminal.
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

/// **The considered fold this milestone adds: a real, authenticated identity with no provisioned
/// subtree is refused, indistinguishably from a wrong password.**
///
/// `carol` has a real credential in `wired`'s store (`credential_tests::provisioned`'s own family
/// fixture) and, deliberately, no home subtree (`wired` only calls `ensure_home_subtree` for `alice`
/// and `bob`): the case `identity_provisioner` never having run for a real identity, or its
/// `MKDIR` never reaching this file service's disk. `mint`'s caretaker construction reaches the
/// same `OPENDIR`-against-a-missing-name refusal, and `login`'s existing fold answers it with
/// [`login_contract::DENIED`], the same code [`login_denies_a_wrong_secret_and_sends_nothing_further`]
/// above already proves a wrong password gets. See `components/src/login.rs`'s own BUGS for the reasoning
/// (a caller must not be able to tell "your identity has no home" from "your password is wrong" by
/// comparing outcomes across attempts).
///
/// No audit record follows this login, by the same rule a wrong secret gets none: `login`'s `AUDIT`
/// send lives only on the success path.
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
    // `carol`: a real credential this suite provisions and a home subtree it deliberately never
    // creates. An honest login, refused for the subtree rather than for the secret.
    let r = ls::client(cli, &w, ls::LOGIN, CAROL, CAROL);
    assert_eq!(
        r[0],
        ls::RPT_DENIED,
        "carol's real credential, with no provisioned subtree, was not refused",
    );
}

/// **The measurement gap this milestone's BUGS used to name, resolved.** `login`'s `_start` now
/// reads [`measured_boot::PROGRAM_MEASUREMENTS`] out of the same archive it reads
/// `fs_subtree_caretaker` from and calls [`measured_boot::verify_in_manifest`] before it will ever
/// build a caretaker from those bytes, the identical check `crates/system_initializer::measured`
/// performs for its own six boot components (see `components/src/login.rs`'s own BUGS, "Resolved,
/// 2026-08-24", for the full reasoning, including why this is a boot-time check rather than a
/// per-login one, and why the fold into [`login_contract::DENIED`] it produces on refusal is not the
/// same anti-oracle reasoning a wrong password or a missing subtree gets).
///
/// **This suite cannot make the real, already-booted archive disagree with itself** to prove a live
/// refusal end to end: the archive is one physical region the whole kernel test binary shares, wired
/// once at boot, and `cargo xtask` always packs a measurement table that agrees with the bytes it
/// just packed alongside it. A second login instance spawned against a deliberately tampered archive
/// is not something this suite (or `ls::start`, which always takes the one real
/// `memory::initial_ramdisk_region()`) can construct. So this proves the two things a live end-to-end refusal
/// would have proven, at the exact level `login`'s own check operates on: the real
/// `fs_subtree_caretaker` bytes verify against the real table (which is *also* the reason `wired()`'s
/// own instance, and every other test in this file, can log anyone in at all, now that the check
/// exists), and the identical [`measured_boot::verify_in_manifest`] call `_start` makes refuses a
/// tampered copy of those same bytes and a name the table does not mention, mirroring
/// `measured_boot_tests::the_measurement_table_refuses_a_tampered_program_and_an_unnamed_one`, which
/// covers `console` and the boot-time components but not this one (`fs_subtree_caretaker` is not in
/// that test's own `BOOT_COMPONENTS`, since it is not a boot-time component; it is what a directory
/// grant is made of, per-invocation, and this milestone is what first gave it a measurement
/// consumer at all).
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

/// **Nothing else would have caught this**: the caretaker-teardown fix (this milestone; see
/// `components/src/login.rs`'s own BUGS, "Resolved"), proven by needing more memory than could possibly
/// fit if a session's pages did not actually come back. `wired()`'s shared instance holds
/// `CONSTRUCTION_PAGES` in total, and all but a few hundred pages of that is already permanently
/// spent by the tests above (this file's own accounting comment on that constant); what is left
/// would not survive a *second* leaked session at the old 128-pages-per-login rate, let alone the
/// ten this test performs (2560 pages, an order of magnitude past the headroom). Every one of the
/// ten logs fully back out
/// (both delegated `MemoryRegion`s destroyed, `login_test_client.rs`'s `LOGOUT`) before the next
/// begins, so this test could only pass by the memory genuinely coming home each time.
///
/// Each iteration asserts the whole chain, not merely that the final `DESTROY` returned success: a
/// real `READDIR` and a real `RETYPE` before teardown (the same proof every other successful-login
/// test in this file asks for), both `DESTROY` calls succeeding, and both capabilities answering
/// nothing afterward (`login_test_client.rs`'s own re-check, not merely the syscalls' return codes).
///
/// **It also caught a second bug, and this is the one worth knowing about**: for two days this test
/// failed reproducibly on its *second* iteration, refusing a correct password. Ten cycles in a row
/// is what made it visible at all, because the cause was a two-slot-per-connect leak of `login`'s
/// own sixteen-slot capability table, and a service that never serves two logins in a row cannot
/// show it. See `components/src/login.rs`'s BUGS ("Resolved, 2026-08-26") for the mechanism and for the
/// four memory hypotheses that were measured and ruled out before it was found.
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
        // Drain the attribution record, the same as every other successful-login test in this file
        // (the headline test's own note: `send(AUDIT, ...)` is a blocking rendezvous).
        let a = sched::ipc_receive(w.audit);
        assert_eq!(
            a[0],
            login_contract::ATTRIBUTED,
            "no attribution record followed login {i}",
        );
        // **Free the terminal, after draining `AUDIT`, not before.** `LOGOUT` deliberately
        // does not do this itself (`login_test_client.rs`'s own comment on why: it would deadlock
        // against this same drain). Every iteration after the first would otherwise be refused
        // `NO_TERMINAL` instead of authenticated, since milestone 49's terminal update means every
        // successful login also claims the single, shared terminal.
        free_terminal(&w);
    }
}

/// **Milestone 49's terminal update, end to end: single-session, deny cleanly.** The roadmap's own
/// recommendation (`design/roadmap/0049-users-and-attribution.md`'s BUGS) built and proven here in one
/// sequence against the one memoized service instance every other test in this file shares:
///
/// 1. `alice` logs in ([`ls::HOLD_TERMINAL`]) and receives a fifth delegated capability, the
///    terminal, which this test confirms is real (not merely present) by receiving the word the
///    role sends through it on `w.term_ep` -- the stand-in terminal this suite's own harness wires
///    in place of a real one (`ls::Wiring::term_ep`'s own doc). `alice` then tears the *session*
///    down (the same `MemoryRegion::DESTROY` pair [`ls::LOGOUT`] already proves) **without**
///    freeing the terminal: no `login_contract::logout_word` is ever sent.
/// 2. `bob` then presents a **real, correct** credential (a plain [`ls::LOGIN`], which is all
///    that case ever was) while the
///    terminal is still on loan. He is refused [`login_contract::NO_TERMINAL`], not `DENIED`: this is
///    provably not about his identity or secret, both of which are genuine.
/// 3. [`ls::FREE_TERMINAL`] frees the terminal, a bare word on the front door with no identity
///    involved at all, and is answered [`login_contract::LOGGED_OUT`].
/// 4. `alice` logs in again ([`ls::HOLD_TERMINAL`], a second, independent time) and receives the
///    terminal a second time, proven the same way as step 1 -- the property stated positively: a
///    session freeing the terminal is what makes it available to the *next* login, not merely what
///    stops it being refused.
///
/// **Costs nothing permanent against [`CONSTRUCTION_PAGES`].** Every login this test performs either
/// tears its own session down before returning (`HOLD_TERMINAL`, exactly like `LOGOUT`) or is
/// refused before `mint` ever runs (`bob`'s `NO_TERMINAL`, checked in `login.rs`'s
/// `serve_login` before authentication is even attempted), so this test's own two successful logins
/// are the same "logs back out" shape `caretaker_teardown_reclaims_a_full_session_worth_of_memory`
/// already established, not a third kind of permanent charge.
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

    // Step 1: alice takes the terminal. `spawn_client`/`wait_client`, not `client`, because the
    // role blocks inside `send(term_ep, ...)` until this test's own `ipc_receive(w.term_ep)` catches
    // it (a genuine rendezvous, not merely `RECEIVE_CAP` succeeding): waiting on the report first would
    // deadlock against the role's own forward progress.
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

/// **A listed identity's session is handed the run-unvouched capability, and cannot pass it on**
/// (DECISIONS §219 (how the shell names an installed program to the spawner) gate D2, milestone 198 (a package manager) rung 3a;
/// DECISIONS §221 (the boot prompt is the owner's console), ruling 2, for "listed").
///
/// What this proves that nothing else would: that the sixth capability a session receives names
/// the very endpoint the spawner gave `login` (a word sent on it arrives on that endpoint, which is
/// where the progenitor receives a presentation in the real boot), and that the session's copy
/// lacks `GRANT` (its `SEND_CAP` is refused), which is limitation 2's reason the capability is a
/// presentation rather than a token. `script/swish-check` proves the progenitor's half; no session
/// `login` builds holds a spawn endpoint, so this is the only place the delivery runs.
///
/// Falsified by hand before it was committed (aarch64, 2026-09-26): delegating the sixth capability
/// `WRITE | GRANT` in `components/src/login.rs` turned this test red. The client's `SEND_CAP` of it
/// then succeeded, onto the report endpoint, and arrived in place of the report (word 0 read 0).
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
