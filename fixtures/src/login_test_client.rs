//! **The login service's test client**: one binary and one endowment, run as `_start(behaviour,
//! identity, secret)`, the last two indices into [`credential_contract::fixture::PEOPLE`]. So
//! `(ALICE, WRONG)` is the honest program presenting a secret that is nobody's, and a refusal is
//! tested on the honest path. Each behaviour constant ([`LOGIN`] through [`EMPTY_SCHEDULE`])
//! documents its own flow. Session teardown and freeing the terminal are independent acts
//! ([`HOLD_TERMINAL`]). See notes/login.md.

//! # What every behaviour shares
//!
//! - After any refusal ([`login_contract::DENIED`], [`login_contract::NO_TERMINAL`]) nothing
//!   follows, so a run never blocks in `RECEIVE_CAP`. An identity with no subtree is refused as a
//!   wrong secret is (§117 (a principal's subtree is named by its identity string)).
//! - A run that succeeds uses what it received (a caretaker directory read, the file service's
//!   frame, a page retyped from the budget), so a capability that arrived broken fails.
//! - Two identities' logins yield distinct caretaker endpoints (§109 (attribution is a channel
//!   property)); [`WRITE_MARKER`] and [`READ_MARKER`] prove two identities land in isolated
//!   subtrees and one identity's two sessions in the same.

//! # Capability contract
//!
//! - slot 0: the front-door request endpoint, `WRITE`, carrying only
//!   [`login_contract::connect_word`] and, for [`FREE_TERMINAL`], [`login_contract::logout_word`].
//! - slot 1: the front-door result endpoint, `READ`: [`login_contract::CONNECTED`], then a private
//!   request endpoint, a private result endpoint and the staging page the login happens on.
//! - slot 2: a report endpoint, `WRITE`; slot 3: a small `MemoryRegion`, `WRITE`, for the staging
//!   page's page tables (`kernel::user::login_service::CLIENT_SCRATCH_UT_PAGES`).
//! - `a2`, the secret, is the person's own index, [`credential_contract::fixture::WRONG`] for
//!   nobody's, or [`credential_contract::fixture::NONE`] for none.

//! # BUGS
//!
//! - The credential travels as indices because nothing can hand a `no_std` program a string it was
//!   not compiled against. That suits a fixture; a real program needs a capability to whatever
//!   holds the secret (§41 (the endpoint is the broker)), as `login` itself has.
//! - A spawn argument is visible wherever a spawn is recorded; nothing prints these today.
//!
//! Name: ratified 2026-09-14 (calef), on the `<service>_test_client` pattern, which keeps
//! `login_client` free for a real client.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{
    call, destroy_memory_region, exit, map_page_frame, receive, receive_cap,
    retype_into_page_frame, send, send_capability, split_memory_region, yield_now,
};

/// The login service's front-door request endpoint (slot 0), `WRITE`.
const SERVICE: u64 = 0;
/// The login service's front-door result endpoint (slot 1), `READ`.
const RESULT: u64 = 1;
/// The report endpoint (slot 2), `WRITE`.
const REPORT: u64 = 2;
/// This program's own scratch `MemoryRegion` (slot 3), `WRITE`. Used once, for `map_page_frame`'s
/// own page-table cost when this program self-maps the page [`login_contract::CONNECTED`] delegates.
const SCRATCH: u64 = 3;

/// Where this program maps the private staging page [`login_contract::CONNECTED`] delegates, at
/// runtime, once `CONNECT` hands back the capability naming it (milestone 49's channel-per-client
/// update; see `_start`'s own `map_page_frame` call). Not a `MappedWindow` (round 6's usual
/// collapse for a statically pre-mapped page): nothing is mapped here before this process runs.
const PAGE_VA: u64 = address_space_map::pair_page(0x0000_0000_00e2_0000);
/// Where this process maps the delegated file-service frame, once it has one. Distinct from
/// [`PAGE_VA`]: they are two different pages (this process's login request, and the filesystem
/// contract's shared page), and a client of both must not confuse them the way `credentialer.rs`'s
/// own two-frame rule exists to prevent for its own pair.
const FS_VA: u64 = address_space_map::pair_page(0x0000_0000_00f0_0000);

// SAFETY: constructing the window touches no memory; only `put_page`/`get_page` do, and every
// caller of those runs behind `if mapped { .. }` (this process's own `map_page_frame(fs_page_frame, FS_VA,
// ..)` having already returned true), the same condition the hand-rolled comment this replaces
// relied on (milestone 139 round 2; see `user_mode_runtime::mapped_window`).
const FS_WINDOW: MappedWindow =
    unsafe { MappedWindow::new(FS_VA, filesystem_contract::PAGE as u64) };

/// **Present the credential and prove what came back.** The whole of what every credential case
/// needs: two identities' correct credentials, a wrong secret, an identity with no home subtree, and
/// a real credential presented while [`HOLD_TERMINAL`]'s loan is outstanding are one behaviour with
/// four different arguments, not four behaviours. See the module docs.
pub const LOGIN: u64 = 0;
/// Log in, write **the identity this run was handed** into the marker file, and check the old fixed
/// subtree's own name is absent (DECISIONS §117). See the module docs.
pub const WRITE_MARKER: u64 = 1;
/// Log in in an independent channel and read the marker [`WRITE_MARKER`] wrote, packed into the
/// report the same way `login`'s own audit trail packs an identity
/// ([`login_contract::identity_hint`]). See the module docs.
pub const READ_MARKER: u64 = 2;
/// Log in, prove the directory and budget work exactly like [`LOGIN`], then use the fourth delegated
/// capability (the logout ticket; [`login_contract::OK`]'s doc) to tear the session down and
/// confirm it actually came down: a further `READDIR` through the now-`DESTROY`ed directory
/// capability must fail. See the module docs.
pub const LOGOUT: u64 = 3;
/// Log in, prove the fifth delegated capability (the terminal) is real by sending [`TERM_MAGIC`]
/// through it, then tear the session down like [`LOGOUT`] **without** freeing the terminal. See the
/// module docs.
pub const HOLD_TERMINAL: u64 = 4;
/// Send [`login_contract::logout_word`] on the front door directly; no identity involved, and the one
/// behaviour that takes [`credential_contract::fixture::NONE`] for both halves of its credential. See
/// the module docs.
pub const FREE_TERMINAL: u64 = 5;
/// Log in, and prove the sixth delegated capability (the run-unvouched one, DECISIONS §219 gate D2)
/// names the endpoint `login` was given, by sending [`RUN_UNVOUCHED_MAGIC`] on it for the kernel
/// test to receive, then tears its session down as [`HOLD_TERMINAL`] does.
pub const PRESENT_RUN_UNVOUCHED: u64 = 6;
/// Log in, split a pending-job child off the budget, and prove the logout is refused while it lives
/// and goes through once it is gone. Milestone 152; see the module docs. Provisional name.
pub const PENDING_WORK: u64 = 7;
/// Log in with `login_contract::SCHEDULE`, replace the new timetable's empty document with
/// [`SCHEDULED`], then detach: tear the directory down and leave the budget, which must refuse its
/// own `DESTROY` because `user_timetable_keeper` lives on it. Milestone 152. Provisional name.
pub const OPEN_SCHEDULE: u64 = 8;
/// Log in plainly, expect to be handed the same durable session back (its page still carries
/// [`OPEN_SCHEDULE`]'s reply), replace the document with an empty one, and wait for the timetable's
/// exit word. Milestone 152. Provisional name.
pub const EMPTY_SCHEDULE: u64 = 9;

/// [`OPEN_SCHEDULE`]'s document: one entry the suite's installed fixture backs
/// (`login_tests`'s `install_scheduled_program`, under its provisional name).
const SCHEDULED: &[u8] = b"every 1s scheduled_demo 7\n";
/// Where this program maps the registration page.
const REGISTRATION_VA: u64 = 0x0000_0000_0300_0000;

/// **[`PRESENT_RUN_UNVOUCHED`]'s proof of life**, [`TERM_MAGIC`]'s twin for the sixth capability.
const RUN_UNVOUCHED_MAGIC: u64 = 0x_7e12_0000_0000_0002;

/// **[`PENDING_WORK`]'s stand-in for a scheduled job's authority**, split off the delegated budget.
/// One page: nothing is retyped from it. What matters is that it is a live child, which is all
/// DECISIONS §16's refusal looks at.
const PENDING_JOB_PAGES: u64 = 1;

/// The one-shot marker file [`WRITE_MARKER`] writes and [`READ_MARKER`] reads, inside the identity's own
/// granted subtree. Chosen to collide with nothing else this tree's fixtures use.
const MARKER_NAME: &str = "whoami";

/// **[`HOLD_TERMINAL`]'s proof of life for the delegated terminal.** Sent through the fifth
/// delegated capability once it is in hand; the kernel test's own `sched::ipc_receive` on the stand-in
/// `Wiring::term_ep` is what confirms the delegated copy names the real object rather than merely
/// having arrived. The exact value carries no meaning beyond being recognisable in a test assertion.
const TERM_MAGIC: u64 = 0x_7e12_0000_0000_0001;

/// The report's first word: what the service answered.
pub const RPT_OK: u64 = login_contract::OK;
pub const RPT_DENIED: u64 = login_contract::DENIED;
pub const RPT_MALFORMED: u64 = login_contract::MALFORMED;
/// Milestone 49's terminal update: the terminal was already on loan to another session.
pub const RPT_NO_TERMINAL: u64 = login_contract::NO_TERMINAL;
/// [`FREE_TERMINAL`]'s own answer: the terminal is free again.
pub const RPT_LOGGED_OUT: u64 = login_contract::LOGGED_OUT;

/// Bits of the report's second word, set only when [`RPT_OK`] is the first: which of the delegated
/// capabilities this process proved actually work, rather than merely that they arrived.
pub const F_DIR_WORKS: u64 = 1 << 0;
pub const F_BUDGET_WORKS: u64 = 1 << 1;
/// **Set when [`filesystem_fixture::tree::INNER`] is confirmed absent** from the granted directory:
/// that name exists only in the old, shared fixture subtree every identity used to be attenuated to.
/// Its absence is this client's own proof (not merely an assertion) that the identity this run
/// authenticated as is
/// not looking at that subtree. Set only by [`WRITE_MARKER`].
pub const F_NOT_SHARED_SUBTREE: u64 = 1 << 2;
/// **Set when the marker file was created and written successfully.** Set only by
/// [`WRITE_MARKER`]; its absence there means the isolation proof below did
/// not get to run at all, which the kernel test must treat as its own failure rather than silence.
pub const F_MARKER_WRITTEN: u64 = 1 << 3;
/// **Set when the fourth capability's `MemoryRegion::DESTROY` returned success.** Set only by
/// [`LOGOUT`]; retried on refusal until the region comes down or [`DESTROY_WAIT_SECS`] runs out
/// ([`login_contract::OK`]'s doc, on the fourth capability, name the transient window this
/// covers, and `destroy_with_retry` says why the bound is a clock).
pub const F_TEARDOWN_OK: u64 = 1 << 4;
/// **Set when a `READDIR` through the directory capability failed *after* teardown.** Set only by
/// [`LOGOUT`]; this is the proof that the capability, not merely the syscall, came down: a
/// `DESTROY` that returned success but left the directory answering requests would pass every check
/// up to this one and fail only this one.
pub const F_DEAD_AFTER_TEARDOWN: u64 = 1 << 5;
/// **Set when `MemoryRegion::DESTROY` on the *budget* (the third delegated capability) also returned
/// success.** Set only by [`LOGOUT`], and proves the other half of `login.rs`'s BUGS: a full
/// logout needs no capability beyond what every run already receives, because `budget` was always
/// delegated with `WRITE`, the one right `DESTROY` needs. See the module docs.
pub const F_BUDGET_TEARDOWN_OK: u64 = 1 << 6;
/// **Set when a further `RETYPE` on the budget failed *after* its own teardown.** The budget's half
/// of [`F_DEAD_AFTER_TEARDOWN`].
pub const F_BUDGET_DEAD_AFTER_TEARDOWN: u64 = 1 << 7;
/// **Set when the fifth delegated capability (the terminal) delivered [`TERM_MAGIC`] to a real
/// receiver.** Set only by [`HOLD_TERMINAL`]. A capability that merely arrived (`RECEIVE_CAP`
/// succeeded) would pass every earlier check and never set this one: `send` only returns once a
/// receiver is actually matched.
pub const F_TERM_WORKS: u64 = 1 << 8;
/// **Set when a sixth capability arrived and could not be passed on**: `SEND_CAP` of it was refused
/// `NotPermitted`, which the kernel answers before it matches any receiver, for a capability that
/// lacks `GRANT`. That refusal is DECISIONS §219 limitation 2 holding: a session holds the right to
/// run unvouched bytes and cannot lend it by delegation. Set on every successful login that was
/// announced a sixth capability.
pub const F_RUN_UNVOUCHED_NOT_GRANTABLE: u64 = 1 << 9;
/// **Set when [`RUN_UNVOUCHED_MAGIC`] was taken on the sixth capability.** Set only by
/// [`PRESENT_RUN_UNVOUCHED`]; like [`F_TERM_WORKS`], `send` returns only once a receiver matched.
pub const F_RUN_UNVOUCHED_WORKS: u64 = 1 << 10;
/// **Set when `OK` announced a sixth capability at all** ([`login_contract::RUN_UNVOUCHED_FOLLOWS`]),
/// for every behaviour. Its absence is how the kernel test sees an identity that is not on the
/// owner's list (DECISIONS §221 (the boot prompt is the owner's console), ruling 2) without waiting
/// on an endpoint nothing will send to.
pub const F_RUN_UNVOUCHED_ANNOUNCED: u64 = 1 << 11;
/// **Set when the budget's `DESTROY` was refused `NotPermitted` while a pending-job child lived.**
/// Set only by [`PENDING_WORK`]. Any other answer, success included, leaves it clear: a logout that
/// succeeded here would have torn down a session with scheduled work still hanging off it.
pub const F_LOGOUT_REFUSED_WHILE_PENDING: u64 = 1 << 12;
/// **Set when the budget still retyped a page after that refusal.** Set only by [`PENDING_WORK`].
/// A refused `DESTROY` that half-tore the session down would set the flag above and fail this one.
pub const F_SESSION_SURVIVED_REFUSAL: u64 = 1 << 13;
/// **Set when the pending-job child itself was destroyed.** Set only by [`PENDING_WORK`], and it is
/// what makes the budget destroyable again.
pub const F_PENDING_WORK_DESTROYED: u64 = 1 << 14;
/// **Set when the reply announced the registration page** (`login_contract::SCHEDULE_FOLLOWS`),
/// by any behaviour. Milestone 152.
pub const F_SCHEDULE_ANNOUNCED: u64 = 1 << 15;
/// **Set when a replace was in force**: [`OPEN_SCHEDULE`]'s document replaced with its one entry
/// planned to fire, or [`EMPTY_SCHEDULE`]'s empty one answered `STATUS_EMPTIED`.
pub const F_REPLACED: u64 = 1 << 16;
/// **Set when the page already carried the first session's reply.** Set only by
/// [`EMPTY_SCHEDULE`]: the proof that the login reattached rather than minting a second session.
pub const F_REATTACHED: u64 = 1 << 17;
/// **Set when the timetable's exit word appeared** after the empty replace. Set only by
/// [`EMPTY_SCHEDULE`].
pub const F_TIMETABLE_EXITED: u64 = 1 << 18;
/// **Set when the document was written to the identity's stored schedule first.** Set only by
/// [`OPEN_SCHEDULE`] and [`EMPTY_SCHEDULE`]. Milestone 152.
pub const F_STORED: u64 = 1 << 19;
/// **Set when the plan in the page grants the job no report endpoint.** Set only by
/// [`OPEN_SCHEDULE`], and only when the plan lists what the job holds at all: Fork 6 C of milestone
/// 152, ruled 2026-09-27 on #1377, says a durable job holds none.
pub const F_NO_REPORT: u64 = 1 << 20;

user_mode_runtime::program_entry!(start);

/// `a0` is the behaviour, `a1` the identity and `a2` the secret; see the module docs. Three
/// registers because that is what a process is born with (`kernel::user::Spawn`), and the two
/// credential halves are indices because a register cannot carry a byte string.
fn start(behaviour: u64, identity: u64, secret: u64) -> ! {
    if behaviour == FREE_TERMINAL {
        // **Never calls `CONNECT` at all.** `login_contract::logout_word` travels on the shared front
        // door directly (`login_contract`'s own module docs): there is no secret to protect, so there
        // is nothing a private channel would buy here.
        send(SERVICE, login_contract::logout_word(), 0, 0);
        let (verdict, _, _) = receive(RESULT);
        done(verdict, 0, 0);
    }

    // **A credential this program cannot resolve is malformed, not a default.** Every other
    // behaviour authenticates, so an out-of-range index (or `NONE`, which is what
    // [`FREE_TERMINAL`] is handed and nothing else should be) must not fall back to whoever sits at
    // index zero: a run that silently logged in as `alice` would pass every assertion that follows
    // and prove nothing about the identity the caller meant.
    let (Some(identity), Some(secret)) = (
        credential_contract::fixture::identity(identity),
        credential_contract::fixture::secret(secret),
    ) else {
        done(RPT_MALFORMED, 0, 0);
    };

    // **Milestone 49's channel-per-client update: connect first.** The front door's only legal
    // word; see `login_contract`'s own module docs for the two-phase exchange. Nothing is staged for
    // this step, so there is no page to write before sending it.
    send(SERVICE, login_contract::connect_word(), 0, 0);
    let (connect_verdict, _, _) = receive(RESULT);
    if connect_verdict != login_contract::CONNECTED {
        // The front door answered something other than CONNECTED (MALFORMED or DENIED): nothing
        // follows, the same promise the private channel's own OK/DENIED gives.
        done(connect_verdict, 0, 0);
    }
    let (_, priv_request, _) = receive_cap(RESULT);
    let (_, priv_result, _) = receive_cap(RESULT);
    let (_, priv_page, _) = receive_cap(RESULT);

    // Map the delegated staging page using this program's own small scratch region: unlike the
    // post-auth `budget` the rest of this function uses, nothing else has been received yet at this
    // point.
    if !map_page_frame(priv_page, PAGE_VA, true, SCRATCH) {
        done(RPT_MALFORMED, 0, 0);
    }

    // SAFETY: `priv_page` was just mapped read/write at PAGE_VA, private to this process and to the
    // login service's own copy; no other client holds a capability to it. Not `PAGE_WINDOW`
    // (round 6's collapse): that type's contract assumes the wiring maps this page before the
    // process runs, which milestone 49's channel-per-client update made false here specifically:
    // the page is now mapped dynamically, per connection, from a capability CONNECT hands back at
    // runtime, so there is nothing for a compile-time-constant window to be a window onto yet.
    let page = unsafe { core::slice::from_raw_parts_mut(PAGE_VA as *mut u8, login_contract::PAGE) };
    let operation = if behaviour == OPEN_SCHEDULE {
        login_contract::SCHEDULE
    } else {
        login_contract::LOGIN
    };
    let Some(w0) = login_contract::place(page, identity, secret, operation) else {
        done(RPT_MALFORMED, 0, 0);
    };
    send(priv_request, w0, 0, 0);
    let (verdict, extra, _) = receive(priv_result);

    if verdict != login_contract::OK {
        // Nothing follows a refusal (`login_contract`). Reporting rather than attempting
        // `RECEIVE_CAP` is the check: a stray fourth message would desync the next login's first
        // `RECEIVE_CAP`, and a client that tried to receive here would hide that.
        done(verdict, 0, 0);
    }

    // Five capabilities, in login_contract's fixed order, on the private channel `priv_result` names.
    let (_, dir_ep, _) = receive_cap(priv_result);
    let (_, fs_page_frame, _) = receive_cap(priv_result);
    let (_, budget, _) = receive_cap(priv_result);
    let (_, region, _) = receive_cap(priv_result);
    let (_, term_ep, _) = receive_cap(priv_result);
    // The sixth, only as announced, and taken here with the other five: `login` is blocked sending
    // it, and anything this process sends first (the terminal's proof below) waits on a kernel test
    // that is itself waiting on `login`.
    let run_unvouched =
        (extra & login_contract::RUN_UNVOUCHED_FOLLOWS != 0).then(|| receive_cap(priv_result).1);
    // The registration page, last, and only as announced (milestone 152).
    let registration =
        (extra & login_contract::SCHEDULE_FOLLOWS != 0).then(|| receive_cap(priv_result).1);

    let mut flags = 0u64;
    if run_unvouched.is_some() {
        flags |= F_RUN_UNVOUCHED_ANNOUNCED;
    }
    if registration.is_some() {
        flags |= F_SCHEDULE_ANNOUNCED;
    }
    // **Prove the sixth is real (for one behaviour), then that it cannot be passed on.** In that
    // order, because the delegation is attempted on the report endpoint, which the kernel test
    // reads only after it has taken the proof of life: a capability wrongly carrying `GRANT` is then
    // sent there and garbles the report, failing the test rather than hanging it.
    if let Some(ru) = run_unvouched {
        flags |= prove_run_unvouched(behaviour, ru);
    }

    // Prove the terminal before anything else touches `budget`/`region`. `send` on a plain
    // rendezvous returns only once a receiver matched, so this blocks until the kernel test's
    // `sched::ipc_receive(w.term_ep)` takes it, which a dead or wrong object would never do.
    if behaviour == HOLD_TERMINAL {
        send(term_ep, TERM_MAGIC, 0, 0);
        flags |= F_TERM_WORKS;
    }

    // Prove the directory capability works: map the delegated frame (which needs `budget` alive, to
    // supply page-table pages for the mapping: `user_mode_runtime::map_page_frame`'s own contract), then read the
    // granted subtree's listing through the delegated caretaker endpoint. A capability that merely
    // arrived would pass every check up to this line and fail only this one.
    let mapped = map_page_frame(fs_page_frame, FS_VA, true, budget);

    // Prove the budget works: retype one page from it. `RETYPE`'s reply is the new frame's slot
    // (>= 0) or a negative error. Done here, right after the one use of `budget` that needs it
    // alive (`map_page_frame` above), and before anything below tears it down.
    if retype_into_page_frame(budget) >= 0 {
        flags |= F_BUDGET_WORKS;
    }

    // **Milestone 152's schedule, opened and later emptied through its page.**
    if let Some(reg) = registration
        && (behaviour == OPEN_SCHEDULE || behaviour == EMPTY_SCHEDULE)
        && mapped
        && map_page_frame(reg, REGISTRATION_VA, true, budget)
    {
        flags |= schedule(behaviour, budget, dir_ep);
    }

    // **Milestone 152's durable-session property, on a real login session.** A scheduled job's
    // authority would be split off the session's budget, so a live child here stands for pending
    // work. The logout's first step must be refused for exactly as long as that child lives: one
    // attempt, not `destroy_with_retry`, because this refusal is not a kill waiting for a tick. It is
    // permanent until the child goes, and retrying would only spend the watchdog.
    if behaviour == PENDING_WORK {
        flags |= hold_pending_work(budget);
    }

    // `budget` is destroyed before `region`: `mint()` (`components/src/login.rs`) splits `region`
    // then `budget` off `CONSTRUCTION_UT`, so `budget` sits on top. `crates/memory_regions`'
    // `return_to_parent` reclaims a hole only once nothing above it is held (`coalesced_watermark`,
    // §16 (object revocation)), and only the plain LIFO un-bump while a sibling is mid-teardown.
    // Freed first, `region` would be a hole under the live `budget`, not reused while it lives, and
    // kept past the logout if a sibling is mid-teardown when `budget` returns; later tests then starve into
    // the anti-oracle failure `login_contract::DENIED` guards against (a real password answered as
    // wrong). See `caretaker_teardown_reclaims_a_full_session_worth_of_memory` and
    // `login_contract`'s docs.
    if (behaviour == LOGOUT
        || behaviour == HOLD_TERMINAL
        || behaviour == PRESENT_RUN_UNVOUCHED
        || behaviour == PENDING_WORK)
        && destroy_with_retry(budget)
    {
        flags |= F_BUDGET_TEARDOWN_OK;
        if retype_into_page_frame(budget) < 0 {
            flags |= F_BUDGET_DEAD_AFTER_TEARDOWN;
        }
    }

    let (directory_flags, hint) = if mapped {
        prove_the_directory(behaviour, identity, dir_ep, region)
    } else {
        (0, 0)
    };
    flags |= directory_flags;

    // `LOGOUT` must not also free the terminal: `login` is blocked in `send(AUDIT, ...)` until the
    // caller drains it, which the caller does only after this report, so waiting here for a logout
    // answer would deadlock. The caller frees it after draining `AUDIT`
    // (`kernel::user::login_tests::free_terminal`).
    done(RPT_OK, flags, hint);
}

/// Prove the directory capability works, mapped at [`FS_VA`]: list the granted subtree, then do
/// what `behavior` asks of it. Returns the flags it earned and the third report word.
fn prove_the_directory(behavior: u64, identity: &[u8], dir_ep: u64, region: u64) -> (u64, u64) {
    let mut flags = 0u64;
    let mut hint = 0u64;
    let (r0, _) = call(
        dir_ep,
        filesystem_contract::fs::req(
            filesystem_contract::fs::READDIR,
            filesystem_contract::fs::ROOT,
            0,
        ),
        0,
    );
    // `call` returns the reply word as a `u64`; a negative errno reads as a huge one
    // (`entropy_contract`'s convention, followed here and by `smb_server.rs`'s own `fs_readdir`).
    // A `READDIR` answers the byte count written (>= 0) or a negative errno. Any non-negative
    // answer is the capability working; this test does not pin the fixture's exact contents,
    // which is a fact about the image and not about this capability.
    if (r0 as i64) >= 0 {
        flags |= F_DIR_WORKS;

        // DECISIONS §117's own proof: which subtree did this login actually land in? Only the
        // `WRITE_MARKER`/`READ_MARKER` do this; `LOGIN` stops at proving what it received.
        match behavior {
            WRITE_MARKER => {
                if write_marker(dir_ep, identity) {
                    flags |= F_MARKER_WRITTEN;
                }
                if is_absent(dir_ep, filesystem_fixture::tree::INNER) {
                    flags |= F_NOT_SHARED_SUBTREE;
                }
            }
            READ_MARKER => {
                if let Some(h) = read_marker(dir_ep) {
                    hint = h;
                }
            }
            // `budget` is already gone by construction (above); `region` is now the top of
            // `CONSTRUCTION_UT`'s watermark, so this `DESTROY` un-bumps it too, and this
            // login's whole 128-page contribution comes home.
            LOGOUT => {
                flags |= teardown_directory(dir_ep, region);
                // The third report word, for this behavior only: how many microseconds the
                // caretaker region's `DESTROY` waited (the budget's own wait is not reported;
                // nothing lives in that region, so it has never needed one). The kernel test
                // quotes it in the failure message.
                hint = waited_micros();
            }
            // `HOLD_TERMINAL` shares the teardown but not the wait report: it keeps the identity
            // hint every other behavior puts in the third word.
            HOLD_TERMINAL => flags |= teardown_directory(dir_ep, region),
            // Torn down as `HOLD_TERMINAL` is, and for a measured reason: left standing, its
            // caretaker lived for the rest of the suite and a later, unrelated test
            // (`timetable_tests`) hung on riscv64 and aarch64 for want of what it held.
            PRESENT_RUN_UNVOUCHED => flags |= teardown_directory(dir_ep, region),
            // Its whole point is that the session comes home once the work is gone.
            PENDING_WORK => flags |= teardown_directory(dir_ep, region),
            // Detach: the directory goes, the durable budget stays with its session.
            OPEN_SCHEDULE | EMPTY_SCHEDULE => flags |= teardown_directory(dir_ep, region),
            _ => {}
        }
    }
    (flags, hint)
}

/// The sixth capability, `ru`: used once (for [`PRESENT_RUN_UNVOUCHED`] only), then offered on to
/// the report endpoint, which must refuse it. Answers the flags it earned.
fn prove_run_unvouched(behavior: u64, ru: u64) -> u64 {
    let mut flags = 0u64;
    if behavior == PRESENT_RUN_UNVOUCHED {
        send(ru, RUN_UNVOUCHED_MAGIC, 0, 0);
        flags |= F_RUN_UNVOUCHED_WORKS;
    }
    if send_capability(REPORT, ru, abi::rights::WRITE, 0) == abi::Error::NotPermitted as i64 {
        flags |= F_RUN_UNVOUCHED_NOT_GRANTABLE;
    }
    flags
}

/// [`PENDING_WORK`]'s logout attempt with a pending-job child split off `budget`: refused while the
/// child lives, the session intact after the refusal, and the child itself destroyable. Answers the
/// flags it earned.
fn hold_pending_work(budget: u64) -> u64 {
    let mut flags = 0u64;
    let job = split_memory_region(budget, PENDING_JOB_PAGES);
    if job >= 0 {
        if destroy_memory_region(budget) == abi::Error::NotPermitted as i64 {
            flags |= F_LOGOUT_REFUSED_WHILE_PENDING;
        }
        if retype_into_page_frame(budget) >= 0 {
            flags |= F_SESSION_SURVIVED_REFUSAL;
        }
        if destroy_memory_region(job as u64) == 0 {
            flags |= F_PENDING_WORK_DESTROYED;
        }
    }
    flags
}

/// **Drive the registration page for [`OPEN_SCHEDULE`] or [`EMPTY_SCHEDULE`]**, mapped at
/// [`REGISTRATION_VA`], and answer the flags it earned.
fn schedule(behaviour: u64, budget: u64, dir: u64) -> u64 {
    use core::sync::atomic::{AtomicU64, Ordering};

    use timetable::registration as reg;
    let word = |off: usize| {
        // SAFETY: the page is mapped read/write at REGISTRATION_VA, and every offset used here is
        // an aligned word inside it (`timetable::registration`'s layout).
        unsafe { &*((REGISTRATION_VA + off as u64) as *const AtomicU64) }
    };
    let mut flags = 0u64;
    let (doc, seq): (&[u8], u64) = if behaviour == OPEN_SCHEDULE {
        (SCHEDULED, 1)
    } else {
        // The first session's reply is still in the page: this is the same session.
        if word(reg::REPLY).load(Ordering::Acquire) == 1 {
            flags |= F_REATTACHED;
        }
        (b"", 2)
    };
    // **The session writes the store, then stages the replace** (§222 (who holds a user's
    // schedule), sub-ruling 5): the document goes to `schedule` in this identity's own subtree,
    // where `login` reads it back at the next login.
    if store_schedule(dir, doc) {
        flags |= F_STORED;
    }
    // SAFETY: as above; the body is ours to write until the request word is published.
    let body = unsafe {
        core::slice::from_raw_parts_mut((REGISTRATION_VA + reg::BODY as u64) as *mut u8, doc.len())
    };
    body.copy_from_slice(doc);
    word(reg::LEN).store(doc.len() as u64, Ordering::Relaxed);
    word(reg::REQUEST).store(reg::request(reg::REPLACE, seq), Ordering::Release);
    if !wait(|| word(reg::REPLY).load(Ordering::Acquire) == seq) {
        return flags;
    }
    let status = word(reg::STATUS).load(Ordering::Acquire);
    let verdicts = word(reg::VERDICTS).load(Ordering::Acquire);
    let replaced = if behaviour == OPEN_SCHEDULE {
        status == reg::STATUS_REPLACED && reg::verdict_of(verdicts, 0) & 0x3 == reg::KIND_FIRES
    } else {
        status == reg::STATUS_EMPTIED
    };
    if replaced {
        flags |= F_REPLACED;
    }
    if behaviour == OPEN_SCHEDULE && replaced {
        let len = (word(reg::PLAN_LEN).load(Ordering::Acquire) as usize).min(reg::BODY_MAX);
        // SAFETY: the reply is published (the sequence above), so the plan it wrote into the body
        // is complete and nothing writes the page until this client's next request.
        let plan = unsafe {
            core::slice::from_raw_parts((REGISTRATION_VA + reg::BODY as u64) as *const u8, len)
        };
        let has = |needle: &[u8]| plan.windows(needle.len()).any(|w| w == needle);
        if has(b"exactly:") && !has(b"endpoint") {
            flags |= F_NO_REPORT;
        }
    }
    if behaviour == EMPTY_SCHEDULE
        && replaced
        && wait(|| word(reg::EXIT).load(Ordering::Acquire) & reg::EXITED != 0)
    {
        flags |= F_TIMETABLE_EXITED;
    }
    // Both detach with the session still standing, so the budget must refuse to come down.
    if destroy_memory_region(budget) == abi::Error::NotPermitted as i64 {
        flags |= F_LOGOUT_REFUSED_WHILE_PENDING;
    }
    flags
}

/// **Write `doc` as this identity's stored schedule** (`timetable_store::SCHEDULE_FILE_NAME`, in
/// the root of the delegated directory, which is the identity's own subtree), replacing whatever
/// was there. `true` when every byte landed.
fn store_schedule(dir: u64, doc: &[u8]) -> bool {
    use filesystem_contract::fs;
    let name = timetable_store::SCHEDULE_FILE_NAME.as_bytes();
    put_page(name);
    let (mut h, _) = call(dir, fs::req(fs::CREATE, fs::ROOT, name.len() as u64), 0);
    if (h as i64) < 0 {
        put_page(name);
        h = call(dir, fs::req(fs::OPEN, fs::ROOT, name.len() as u64), 0).0;
        if (h as i64) < 0 || (call(dir, fs::req(fs::TRUNCATE, h, 0), 0).0 as i64) < 0 {
            return false;
        }
    }
    put_page(doc);
    let (w, _) = call(dir, fs::req(fs::WRITE, h, doc.len() as u64), 0);
    let _ = call(dir, fs::req(fs::CLOSE, h, 0), 0);
    w as i64 == doc.len() as i64
}

/// Poll `done` until it holds or [`DESTROY_WAIT_SECS`] of counter time pass, yielding between.
fn wait(done: impl Fn() -> bool) -> bool {
    let ceiling = user_mode_runtime::cntfrq().saturating_mul(DESTROY_WAIT_SECS);
    let started = user_mode_runtime::now();
    while user_mode_runtime::now().wrapping_sub(started) < ceiling {
        if done() {
            return true;
        }
        yield_now();
    }
    false
}

/// Copy `bytes` into the shared filesystem page (a name to open/create, or data to write).
fn put_page(bytes: &[u8]) {
    for (i, &b) in bytes.iter().enumerate() {
        FS_WINDOW.w8(i as u64, b);
    }
}

/// Read `n` bytes out of the shared filesystem page into `out`.
fn get_page(n: usize, out: &mut [u8]) {
    for (i, b) in out.iter_mut().take(n).enumerate() {
        *b = FS_WINDOW.r8(i as u64);
    }
}

/// `CREATE` [`MARKER_NAME`] under `dir` and `WRITE` `content` into it, then `CLOSE` the handle.
/// `true` only if every step succeeded. Create is create, not create-or-open
/// (`filesystem_contract::fs::CREATE`'s own contract), so this fails loudly rather than overwriting a marker a
/// previous run left behind, which would silently defeat the isolation proof this behaviour exists
/// for.
fn write_marker(dir: u64, content: &[u8]) -> bool {
    put_page(MARKER_NAME.as_bytes());
    let (h, _) = call(
        dir,
        filesystem_contract::fs::req(filesystem_contract::fs::CREATE, 0, MARKER_NAME.len() as u64),
        0,
    );
    if (h as i64) < 0 {
        return false;
    }
    put_page(content);
    let (w, _) = call(
        dir,
        filesystem_contract::fs::req(filesystem_contract::fs::WRITE, h, content.len() as u64),
        0,
    );
    let ok = w as i64 == content.len() as i64;
    let _ = call(
        dir,
        filesystem_contract::fs::req(filesystem_contract::fs::CLOSE, h, 0),
        0,
    );
    ok
}

/// `true` if `OPEN`ing `name` under `dir` is refused. The expected answer for a name that lives only
/// in the old, shared fixture subtree ([`filesystem_fixture::tree::INNER`]) when `dir` is a genuinely
/// different, identity-scoped one.
fn is_absent(dir: u64, name: &str) -> bool {
    put_page(name.as_bytes());
    let (r0, _) = call(
        dir,
        filesystem_contract::fs::req(filesystem_contract::fs::OPEN, 0, name.len() as u64),
        0,
    );
    (r0 as i64) < 0
}

/// `OPEN` and `READ` [`MARKER_NAME`] under `dir`, packed the same way `login`'s own audit trail
/// packs an identity ([`login_contract::identity_hint`]), so the kernel test can compare what this run
/// read against what [`write_marker`]'s caller wrote without a second encoding to keep in sync.
/// `None` if the marker could not be opened or read.
fn read_marker(dir: u64) -> Option<u64> {
    put_page(MARKER_NAME.as_bytes());
    let (h, _) = call(
        dir,
        filesystem_contract::fs::req(filesystem_contract::fs::OPEN, 0, MARKER_NAME.len() as u64),
        0,
    );
    if (h as i64) < 0 {
        return None;
    }
    let (n, _) = call(
        dir,
        filesystem_contract::fs::req(filesystem_contract::fs::READ, h, 16),
        0,
    );
    let _ = call(
        dir,
        filesystem_contract::fs::req(filesystem_contract::fs::CLOSE, h, 0),
        0,
    );
    if (n as i64) < 0 {
        return None;
    }
    let mut buf = [0u8; 16];
    get_page((n as usize).min(buf.len()), &mut buf);
    Some(login_contract::identity_hint(
        &buf[..(n as usize).min(buf.len())],
    ))
}

/// **The fourth capability, `login_contract`'s own logout ticket: `MemoryRegion::DESTROY` reclaims the
/// caretaker `dir` names.** Sets [`F_TEARDOWN_OK`] on success and, only then, re-checks `dir` with a
/// `READDIR`: [`F_DEAD_AFTER_TEARDOWN`] if it now fails, which is the proof that the capability, not
/// merely the syscall, came down.
fn teardown_directory(dir: u64, region: u64) -> u64 {
    let Some(mut flags) = destroy_with_retry(region).then_some(F_TEARDOWN_OK) else {
        return 0;
    };
    let (r0, _) = call(
        dir,
        filesystem_contract::fs::req(
            filesystem_contract::fs::READDIR,
            filesystem_contract::fs::ROOT,
            0,
        ),
        0,
    );
    if (r0 as i64) < 0 {
        flags |= F_DEAD_AFTER_TEARDOWN;
    }
    flags
}

/// **How long [`destroy_with_retry`] waits for a refusal to clear, in seconds of counter time.**
///
/// A watchdog, not a rate: the loop returns the moment the region comes down, so only a failing run
/// pays it, and since the driving test stops at its first bad logout, at most twice (the budget and
/// the region) against the harness's 90 s per-test ceiling. Measured on 2026-08-27 (an eight-core
/// Mac under TCG, ten logouts per run, 100 waits over ten full-suite runs), the worst single wait
/// was 12.8 ms on aarch64 with no induced load (load average 7 to 15), 44.3 ms on aarch64 at 13 to
/// 29, and 126.4 ms on riscv64 at 27 to 47. Five seconds is 40x the worst
/// (notes/load-sensitive-assertions/caretaker-teardown-wait.md).
const DESTROY_WAIT_SECS: u64 = 5;

/// **`MemoryRegion::DESTROY` on `ut`, retried until the region comes down**; `true` if it did.
///
/// For the caretaker's region (`login_contract::OK` names its one transient refusal) and this
/// client's budget, where nothing else runs, so its first `DESTROY` should succeed. The bound is a
/// clock because the refusal lasts until §16 (object revocation)'s armed kill lands at a tick.
///
/// # BUGS
///
/// The bound is wall clock, which milestone 62 (tests that assert on time) argues against, because
/// a process cannot read delivered ticks; [`DESTROY_WAIT_SECS`]'s 40x margin is what makes it safe.
fn destroy_with_retry(ut: u64) -> bool {
    let ceiling = user_mode_runtime::cntfrq().saturating_mul(DESTROY_WAIT_SECS);
    let started = user_mode_runtime::now();
    loop {
        if destroy_memory_region(ut) == 0 {
            record(user_mode_runtime::now().wrapping_sub(started));
            return true;
        }
        if user_mode_runtime::now().wrapping_sub(started) >= ceiling {
            record(user_mode_runtime::now().wrapping_sub(started));
            return false;
        }
        yield_now();
    }
}

/// How long the last [`destroy_with_retry`] waited, in counter ticks. [`LOGOUT`] reports it as
/// microseconds in the third report word (see [`_start`]), so a red run carries the observation the
/// wait actually decided on instead of sending the reader back to re-run under load.
static mut WAITED: u64 = 0;

/// Store [`WAITED`]. Every run of this program is single-threaded and this is its only writer.
fn record(ticks: u64) {
    // SAFETY: one thread, one writer, and the only reader runs after it in the same thread.
    unsafe { WAITED = ticks }
}

/// [`WAITED`] in microseconds, for the report.
fn waited_micros() -> u64 {
    // SAFETY: as `record`.
    let ticks = unsafe { WAITED };
    ticks.saturating_mul(1_000_000) / user_mode_runtime::cntfrq().max(1)
}

fn done(tag: u64, w1: u64, w2: u64) -> ! {
    send(REPORT, tag, w1, w2);
    exit()
}

user_mode_runtime::panic_handler!();
