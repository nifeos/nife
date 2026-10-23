use super::*;
use crate::cap::{Rights, memory_region_root_cap, page_frame_cap, rendezvous_cap};
use crate::sched::{self, RendezvousId};

/// Where the service maps its own request to the credential service. Must match `components/src/login.rs`.
const CRED_VA: u64 = address_space_map::pair_page(0x0000_0000_00e3_0000);

/// How many pages a `login_test_client` run's own `memory_region_cap` (slot 3) holds.
///
/// It pays `page_frame::MAP`'s page-table cost when the run maps the frame `login`'s `CONNECT`
/// step delegates, before the run holds any other budget, and one page for the run's report
/// endpoint. [`wait_client`] reclaims it once the run's thread is dead, since it pays for tables in
/// that run's address space. See notes/login.md, "The kernel's test harness".
const CLIENT_SCRATCH_UT_PAGES: u64 = 5;

/// Stack pages beyond the one page `run` maps. `login` parses an ELF and builds a child address
/// space (`supervision_protocol::build_child`), deeper than `root_supervisor`'s 8-page stack
/// covers; sixteen is the size `credentialer.rs`'s Argon2id loop needed.
const LOGIN_STACK_PAGES: u64 = 16;

/// The behaviours a `login_test_client` run is handed, separately from its credential. Must match
/// `fixtures/src/login_test_client.rs`, whose module docs argue the split.
///
/// The credential halves are `credential_contract::fixture`'s indices, not named again here.
pub const LOGIN: u64 = 0;
/// §117 (a principal's subtree is named by its identity string)'s proof, writing the identity it
/// was handed; see the same
/// file's module docs.
pub const WRITE_MARKER: u64 = 1;
/// Reads that marker back, in an independent channel.
pub const READ_MARKER: u64 = 2;
/// Logs in, then tears the session down with the fourth delegated capability and proves the
/// directory came down with it. See the same file's module docs.
pub const LOGOUT: u64 = 3;
/// Logs in, proves the fifth delegated capability (the terminal)
/// works, tears the session down without freeing the terminal. See the same file's module docs.
pub const HOLD_TERMINAL: u64 = 4;
/// Sends `login_contract::logout_word` on the front door directly, with no credential at all.
pub const FREE_TERMINAL: u64 = 5;
/// §219 (how the shell names an installed program to the spawner) gate D2: logs in and sends
/// [`RUN_UNVOUCHED_MAGIC`] on the sixth delegated capability. See the same file's module docs.
pub const PRESENT_RUN_UNVOUCHED: u64 = 6;
/// Logs in, holds a pending-job child off the budget, proves the logout is refused until the child
/// is gone. See the same file's module docs. Provisional name.
pub const PENDING_WORK: u64 = 7;
/// Logs in with `SCHEDULE`, replaces the document, detaches. Provisional name.
pub const OPEN_SCHEDULE: u64 = 8;
/// Logs in, reattaches, empties the schedule, waits for the timetable to stop.
/// Provisional name.
pub const EMPTY_SCHEDULE: u64 = 9;
/// [`PRESENT_RUN_UNVOUCHED`]'s proof-of-life word; must match the same file's
/// `RUN_UNVOUCHED_MAGIC`.
pub const RUN_UNVOUCHED_MAGIC: u64 = 0x_7e12_0000_0000_0002;

/// The report words `login_test_client` sends; must match the same file.
pub const RPT_OK: u64 = login_contract::OK;
pub const RPT_DENIED: u64 = login_contract::DENIED;
#[allow(dead_code)] // named for completeness with the pair above; nothing exercises it today
pub(crate) const RPT_MALFORMED: u64 = login_contract::MALFORMED;
/// The terminal was already on loan.
pub const RPT_NO_TERMINAL: u64 = login_contract::NO_TERMINAL;
/// [`FREE_TERMINAL`]'s own answer.
pub const RPT_LOGGED_OUT: u64 = login_contract::LOGGED_OUT;
/// Authenticated, and on the owner's suspended list.
pub const RPT_SUSPENDED: u64 = login_contract::SUSPENDED;

/// [`HOLD_TERMINAL`]'s proof-of-life word for the delegated terminal; must match the same file's
/// `TERM_MAGIC`.
pub const TERM_MAGIC: u64 = 0x_7e12_0000_0000_0001;

/// Bits of a successful report's second word; must match the same file.
pub const F_DIR_WORKS: u64 = 1 << 0;
pub const F_BUDGET_WORKS: u64 = 1 << 1;
pub const F_NOT_SHARED_SUBTREE: u64 = 1 << 2;
pub const F_MARKER_WRITTEN: u64 = 1 << 3;
pub const F_TEARDOWN_OK: u64 = 1 << 4;
pub const F_DEAD_AFTER_TEARDOWN: u64 = 1 << 5;
pub const F_BUDGET_TEARDOWN_OK: u64 = 1 << 6;
pub const F_BUDGET_DEAD_AFTER_TEARDOWN: u64 = 1 << 7;
/// The fifth delegated capability delivered [`TERM_MAGIC`] to a
/// real receiver. Set only by [`HOLD_TERMINAL`].
pub const F_TERM_WORKS: u64 = 1 << 8;
/// §219 limitation 2: the sixth capability arrived and `SEND_CAP` of it was refused.
pub const F_RUN_UNVOUCHED_NOT_GRANTABLE: u64 = 1 << 9;
/// The sixth capability delivered [`RUN_UNVOUCHED_MAGIC`]. Set only by [`PRESENT_RUN_UNVOUCHED`].
pub const F_RUN_UNVOUCHED_WORKS: u64 = 1 << 10;
/// `OK` announced a sixth capability. Set by every behaviour that was sent one.
pub const F_RUN_UNVOUCHED_ANNOUNCED: u64 = 1 << 11;
/// The budget's `DESTROY` was refused `NotPermitted` while a pending-job child lived.
pub const F_LOGOUT_REFUSED_WHILE_PENDING: u64 = 1 << 12;
/// The budget still retyped a page after that refusal.
pub const F_SESSION_SURVIVED_REFUSAL: u64 = 1 << 13;
/// The pending-job child was destroyed.
pub const F_PENDING_WORK_DESTROYED: u64 = 1 << 14;
/// The reply announced the registration page.
pub const F_SCHEDULE_ANNOUNCED: u64 = 1 << 15;
/// The replace was in force (planned to fire, or emptied).
pub const F_REPLACED: u64 = 1 << 16;
/// The page already carried the first session's reply.
pub const F_REATTACHED: u64 = 1 << 17;
/// The timetable's exit word appeared.
pub const F_TIMETABLE_EXITED: u64 = 1 << 18;
/// The document was written to the identity's stored schedule first.
pub const F_STORED: u64 = 1 << 19;
/// The plan in the page granted the job no report endpoint.
pub const F_NO_REPORT: u64 = 1 << 20;

/// The ceiling, in microseconds, on [`LOGOUT`]'s wait for §16 (object revocation)'s armed kill to
/// land on the caretaker region's `MemoryRegion::DESTROY`.
///
/// [`LOGOUT`]'s third report word is that wait (`waited_micros`), not an identity hint, so a failure
/// can say whether the host was slow or the caretaker never died. Must match
/// `fixtures/src/login_test_client.rs`'s `DESTROY_WAIT_SECS`.
pub const DESTROY_WAIT_MICROS: u64 = 5 * 1_000_000;

/// A running login service and the endpoints that reach it.
pub struct Wiring {
    /// A client's login request, `WRITE`.
    pub request: RendezvousId,
    /// The verdict and, on success, five delegated capabilities, `READ`.
    pub result: RendezvousId,
    /// One [`login_contract::ATTRIBUTED`] message per successful login, `READ`.
    pub audit: RendezvousId,
    /// The stand-in terminal, `READ`: a bare rendezvous, since the harness holds no real terminal.
    /// A test `ipc_receive`s here to prove a delegated `TERM_EP` copy names this object.
    pub term_ep: RendezvousId,
    /// The stand-in run-unvouched endpoint (§219 gate D2), `READ`: the progenitor's
    /// role, played by the harness. `login` holds it `WRITE | GRANT` at
    /// `grant_plan::spawnproto::RUN_UNVOUCHED_SLOT`, as the real boot places it, and a test
    /// `ipc_receive`s here to confirm the copy a session was handed names this object.
    pub run_unvouched: RendezvousId,
    /// `login`'s own thread: what `retype_fault::arm` names, and whose capability table a sweep
    /// reads before and after each run.
    pub tid: crate::thread::ThreadId,
}

/// Copy `bytes` into fresh read-only pages at consecutive VAs from `base`.
///
/// The kernel-side twin of `supervision_protocol`'s `blobs`, read-only so a child cannot rewrite
/// an image it hands on. An empty `bytes` maps nothing; `login` learns that from a zero length in
/// its argument register.
fn map_blob(space: &mut AddressSpace, base: u64, bytes: &[u8]) {
    let mut off = 0usize;
    while off < bytes.len() {
        let page = space
            .map_new(base + off as u64, Flags::user_rodata())
            .expect("could not map a login blob");
        let n = core::cmp::min(FRAME_SIZE as usize, bytes.len() - off);
        page[..n].copy_from_slice(&bytes[off..off + n]);
        off += n;
    }
}

/// Wire and spawn the login service, and return once its start-up pass is over.
///
/// `verify` is a sealed credential service's verify endpoint, and `verify_page_frame` the frame that
/// instance maps at its `VERIFY_VA` (`credential_service::Wiring::verify_page_frame`), passed rather
/// than looked up because a boot may wire more than one. `fs_ep` and `fs_page_frame` are the file
/// service's root directory and shared page (`fs_service::root_directory`). After
/// `construction_pages` is spent every login is answered [`login_contract::DENIED`]. `schedule`
/// maps the durable-session programs. See notes/login.md, "The kernel's test harness".
pub fn start(
    image: &'static [u8],
    verify: RendezvousId,
    verify_page_frame: u64,
    fs_ep: RendezvousId,
    fs_page_frame: u64,
    construction_pages: u64,
    schedule: bool,
) -> Wiring {
    let elf = Elf::parse(image).expect("login is not loadable");

    let blobs = read_login_blobs(schedule);
    let aspace = build_login_space(&elf, &blobs, verify_page_frame);

    let request = sched::create_rendezvous();
    let result = sched::create_rendezvous();
    let audit = sched::create_rendezvous();
    let term_ep = sched::create_rendezvous();
    let construction = crate::memory_region::create(construction_pages)
        .expect("no construction budget for the login service");

    let thread_control_block_region =
        crate::memory_region::create(2).expect("no tcb region for login");
    let tid =
        sched::create_thread_control_block(thread_control_block_region).expect("no tcb for login");

    grant_in_slot_order(
        tid,
        &LoginSlots {
            request,
            result,
            verify,
            fs_ep,
            fs_page_frame,
            construction,
            audit,
            term_ep,
        },
    );
    let run_unvouched = grant_named_slots(tid);

    sched::configure_thread_control_block(tid, elf.entry(), USER_STACK_TOP, aspace)
        .expect("configure");
    sched::start_thread_control_block(
        tid,
        [
            blobs.caretaker.len() as u64,
            blobs.measurements.len() as u64,
            login_contract::schedule_lengths(
                blobs.keeper.len() as u64,
                blobs.timetable.len() as u64,
            ),
        ],
    )
    .expect("start");

    // The start-up pass uses `fs_page_frame` as its own, so a caller that writes that page before
    // the pass ends races it. `REDERIVE_SKIPS` is the one front-door word with no side effect, and
    // `login` receives nothing on the front door until the pass has run.
    sched::ipc_send(request, [login_contract::rederive_skips_word(), 0, 0]);
    let r = sched::ipc_receive(result);
    assert_eq!(
        r[0],
        login_contract::SKIP_COUNTS,
        "login did not answer REDERIVE_SKIPS at the end of its start-up pass",
    );

    Wiring {
        request,
        result,
        audit,
        term_ep,
        run_unvouched,
        tid,
    }
}

/// The four blobs `login` is started against, read out of the initial ramdisk; an empty one maps
/// nothing.
struct LoginBlobs {
    caretaker: &'static [u8],
    measurements: &'static [u8],
    keeper: &'static [u8],
    timetable: &'static [u8],
}

/// Read the caretaker, the program measurements and, when `schedule` is set, the durable-session
/// programs (`user_timetable_keeper` and `timetable`) out of the initial ramdisk.
fn read_login_blobs(schedule: bool) -> LoginBlobs {
    // `login` gets these as blobs, never the archive itself, because that is how
    // `crates/system_initializer` starts it.
    let archive = super::initial_ramdisk().expect("no initial ramdisk");
    let fs = nifefs::Fs::parse(archive).expect("the initial ramdisk is not a nifefs archive");
    let caretaker = fs.read("fs_subtree_caretaker").unwrap_or(&[]);
    let measurements = fs.read(measured_boot::PROGRAM_MEASUREMENTS).unwrap_or(&[]);
    let (keeper, timetable) = if schedule {
        (
            fs.read("user_timetable_keeper")
                .expect("no user_timetable_keeper in the initial ramdisk"),
            fs.read("timetable").expect("no timetable in the archive"),
        )
    } else {
        (&[][..], &[][..])
    };
    LoginBlobs {
        caretaker,
        measurements,
        keeper,
        timetable,
    }
}

/// Build `login`'s address space (segments, stack, blobs and the credential-relay page) and
/// register it.
fn build_login_space(elf: &Elf<'_>, blobs: &LoginBlobs, verify_page_frame: u64) -> u64 {
    let &LoginBlobs {
        caretaker,
        measurements,
        keeper,
        timetable,
    } = blobs;
    let content: u64 = elf
        .segments()
        .map(|seg| {
            let (s, e) = seg.page_range(FRAME_SIZE);
            (e - s) / FRAME_SIZE
        })
        .sum::<u64>()
        + 1 // CRED_VA
        + caretaker.len().div_ceil(FRAME_SIZE as usize) as u64
        + measurements.len().div_ceil(FRAME_SIZE as usize) as u64
        + keeper.len().div_ceil(FRAME_SIZE as usize) as u64
        + timetable.len().div_ceil(FRAME_SIZE as usize) as u64
        + LOGIN_STACK_PAGES
        // The tables: two more than when one schedule archive sat at one address, since the two
        // images each reach a table of their own.
        + 10;
    let mut space = AddressSpace::new(content).expect("no memory for login");
    map_segments(&mut space, elf).expect("could not lay out login");
    for k in 0..LOGIN_STACK_PAGES {
        space
            .map_new(USER_STACK_VA - k * FRAME_SIZE, Flags::user_data())
            .expect("could not map login's stack");
    }
    #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
    map_timebase_page(&mut space).expect("could not map login's timebase page");
    map_blob(&mut space, login_contract::CARETAKER_ELF_VA, caretaker);
    map_blob(
        &mut space,
        login_contract::PROGRAM_MEASUREMENTS_VA,
        measurements,
    );
    // Empty means no schedule opens.
    map_blob(
        &mut space,
        login_contract::USER_TIMETABLE_KEEPER_ELF_VA,
        keeper,
    );
    map_blob(&mut space, login_contract::TIMETABLE_ELF_VA, timetable);
    // The one page wired statically: the credential service reads requests only from the frame
    // it maps at its `VERIFY_VA`. A login's identity and secret travel on a page `connect()` mints.
    let cred_page = verify_page_frame;
    space
        .map_physical(
            CRED_VA,
            cred_page,
            Flags::user_data(),
            crate::revoke::PageMapSource::NoCapability,
        )
        .expect("could not map login's credential-relay page");

    readopt_user_address_space(space).expect("register the login aspace")
}

/// The eight objects `login` holds in its first eight slots, in `components/src/login.rs`'s order.
struct LoginSlots {
    request: RendezvousId,
    result: RendezvousId,
    verify: RendezvousId,
    fs_ep: RendezvousId,
    fs_page_frame: u64,
    construction: u64,
    audit: RendezvousId,
    term_ep: RendezvousId,
}

/// Grant `login` its eight capabilities, each asserted into the slot `login.rs` expects.
fn grant_in_slot_order(tid: crate::thread::ThreadId, slots: &LoginSlots) {
    let &LoginSlots {
        request,
        result,
        verify,
        fs_ep,
        fs_page_frame,
        construction,
        audit,
        term_ep,
    } = slots;
    // Granted one at a time, not from an array: an unoptimised build materialises a `[(&str, Cap);
    // 8]` whole, and that took `start`'s frame past the 4096-byte guard page
    // `script/stack-frame-check` gates against. See notes/login.md, "The kernel's test harness".
    let mut next_slot = 0u64;
    let mut grant_in_order = |name: &str, cap: crate::cap::Cap| {
        let slot = sched::thread_control_block_insert_cap(tid, cap, None)
            .unwrap_or_else(|_| panic!("insert {name}"));
        assert_eq!(
            slot, next_slot,
            "login's {name} must land in slot {next_slot}"
        );
        next_slot += 1;
    };
    grant_in_order("request", rendezvous_cap(request, Rights::READ));
    grant_in_order(
        "result",
        rendezvous_cap(result, Rights::WRITE.union(Rights::GRANT)),
    );
    grant_in_order("verify", rendezvous_cap(verify, Rights::WRITE));
    grant_in_order(
        "fs_ep",
        rendezvous_cap(fs_ep, Rights::WRITE.union(Rights::GRANT)),
    );
    grant_in_order(
        "fs_page_frame",
        // GRANT too: `login` delegates this frame to every authenticated client (`SEND_CAP`).
        page_frame_cap(
            fs_page_frame,
            Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
        ),
    );
    grant_in_order("construction", memory_region_root_cap(construction));
    grant_in_order("audit", rendezvous_cap(audit, Rights::WRITE));
    grant_in_order(
        "term_ep",
        rendezvous_cap(term_ep, Rights::WRITE.union(Rights::GRANT)),
    );
    assert_eq!(next_slot, 8, "login must hold exactly eight capabilities");
}

/// Grant the run-unvouched endpoint (§219 gate D2) and the durable window (the page a durable
/// session's timetable reads the store through) at the slots the progenitor places them; returns
/// the run-unvouched endpoint.
fn grant_named_slots(tid: crate::thread::ThreadId) -> RendezvousId {
    let run_unvouched = sched::create_rendezvous();
    let landed = sched::thread_control_block_insert_cap(
        tid,
        rendezvous_cap(run_unvouched, Rights::WRITE.union(Rights::GRANT)),
        Some(grant_plan::spawnproto::RUN_UNVOUCHED_SLOT),
    )
    .expect("insert run_unvouched");
    assert_eq!(landed, grant_plan::spawnproto::RUN_UNVOUCHED_SLOT);
    let window = super::fs_service::durable_window().expect("the file service is wired");
    let landed = sched::thread_control_block_insert_cap(
        tid,
        page_frame_cap(
            window,
            Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
        ),
        Some(login_contract::DURABLE_WINDOW_SLOT),
    )
    .expect("insert the durable window");
    assert_eq!(landed, login_contract::DURABLE_WINDOW_SLOT);
    run_unvouched
}

/// Spawn one `login_test_client` run against `w`, wait for it, and return its report. For two
/// runs in flight at once, use [`spawn_client`] and [`wait_client`].
pub fn client(
    image: &'static [u8],
    w: &Wiring,
    behaviour: u64,
    identity: u64,
    secret: u64,
) -> [u64; 5] {
    wait_client(spawn_client(image, w, behaviour, identity, secret))
}

/// One `login_test_client` run in flight: where its report arrives, and what [`wait_client`]
/// gives back once the run is dead (its thread, and the scratch region its page tables came from).
pub struct ClientRun {
    report: RendezvousId,
    tid: crate::thread::ThreadId,
    scratch: u64,
}

/// Spawn one `login_test_client` run at `behaviour` and return without waiting for it; pair with
/// [`wait_client`]. Two runs spawned this way reach the front door concurrently.
///
/// `identity` and `secret` are `credential_contract::fixture` indices, separate so they compose:
/// the wrong-secret case is `(fixture::ALICE, fixture::WRONG)`.
pub fn spawn_client(
    image: &'static [u8],
    w: &Wiring,
    behaviour: u64,
    identity: u64,
    secret: u64,
) -> ClientRun {
    // One scratch per run, so two runs mapping at once cannot exhaust each other's page tables.
    let scratch =
        crate::memory_region::create(CLIENT_SCRATCH_UT_PAGES).expect("no scratch region for a run");
    // From the scratch, not `sched::create_rendezvous`, whose chunks are never given back: the
    // machine shares `sched::MAX_RENDEZVOUS` live endpoints, and the suite's runs would reach it.
    let report =
        sched::create_rendezvous_from(scratch).expect("no report endpoint in a run's scratch");
    let (request, result) = (w.request, w.result);
    let tid = sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: behaviour,
                arg1: identity,
                arg2: secret,
                grants: &[
                    rendezvous_cap(request, Rights::WRITE),
                    rendezvous_cap(result, Rights::READ),
                    rendezvous_cap(report, Rights::WRITE),
                    memory_region_root_cap(scratch),
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn a login_test_client");
    ClientRun {
        report,
        tid,
        scratch,
    }
}

/// Block for one run's report, the other half of [`spawn_client`], then hand the run's scratch
/// back once its thread is gone. The report is the last thing a run sends before it exits, so the
/// wait is for an exit already under way.
pub fn wait_client(run: ClientRun) -> [u64; 5] {
    let report = sched::ipc_receive(run.report);
    let mut held = super::holding::Holding::new();
    held.add_thread(run.tid);
    held.add_region_after_death(run.scratch);
    held.release_or_fail("a login_test_client run");
    report
}
