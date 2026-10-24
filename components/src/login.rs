//! **The login service: authentication produces capabilities** (milestone 49 (users, login, and
//! attribution), §109 (attribution is a channel property)). It authenticates an identity against
//! the credential service and, on success, hands back capabilities instead of mutating an identity
//! field: a fresh directory, a fresh budget, a logout ticket and, when free, the terminal. See
//! notes/login.md, and notes/login/budgets-and-reclamation.md for its budgets.

//! # What it holds to
//!
//! - Each login builds its own `fs_subtree_caretaker`, never a narrowed copy of a shared endpoint.
//! - The front door ([`REQUEST`]/[`RESULT`]) takes `CONNECT`, `LOGOUT`, `SUSPEND` and
//!   `REDERIVE_SKIPS`; [`connect`] mints a private channel per caller, the only place identity
//!   and secret travel. An identity with no provisioned subtree (§117) is refused `DENIED`.
//! - Run-unvouched ([`RUN_UNVOUCHED`]) goes `WRITE` without `GRANT`, to listed identities only.
//! - One session holds the terminal ([`TERM_EP`]): another login is refused `NO_TERMINAL` before
//!   its secret is relayed; `LOGOUT` releases it and authenticates nothing.
//!
//! Name: ratified 2026-09-15 (calef, the `login` family; `design/naming/vocabulary-rulings.md`).

//! # BUGS
//! - One durable session at a time ([`DURABLE_SESSIONS`]); a second identity gets an ordinary one.
//! - Start-up waits up to [`END_WAIT_SECS`] per re-derived session, unmeasured.
//! - A stopped durable session is retired only at its identity's next login.
//! - Ending a durable session waits for a running job (§222), and `SUSPEND` waits on the terminal.
//! - A `user_timetable_keeper` that fails half way leaves its splits in the user's budget.
//! - Identities over `grant::MAX_NAME` (16 bytes) get no subtree: [`mint`] refuses to truncate.
//! - A holder that never sends `LOGOUT` wedges the terminal until this process restarts.
//! - The measured-boot decision is spelled here rather than through `measured_boot::verdict`.
//! - The audit endpoint proves establishment, not per-request attribution.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use supervision_protocol::{
    ChildEndowment, Retention, build_child, memory_region_destroy, memory_region_split,
    retype_obj_from as retype_obj, retype_page_frame_from, start_child,
};
use timetable::registration;
use user_mode_runtime::{
    call, cap_delete, map_page_frame, receive, send, send_capability, yield_now,
};

/// The front door: a bare [`login_contract::CONNECT`], `RECEIVE` (milestone 49).
const REQUEST: u64 = 0;
/// [`login_contract::CONNECTED`], then three delegated capabilities, `WRITE | GRANT`. This process
/// keeps its copies, because it serves the login that channel was minted for.
const RESULT: u64 = 1;
/// The credential service's verify endpoint, `WRITE`. Its provision endpoint is
/// deleted at both ends before any client exists (`components/src/credentialer.rs`).
const VERIFY: u64 = 2;
/// The file service's root directory capability, `WRITE | GRANT`: what every minted caretaker
/// attenuates, and where [`listed`] reads the owner's list.
const FS_EP: u64 = 3;
/// The file service's shared page, a `PageFrame`, `READ | WRITE`, delegated on to each principal.
/// `WRITE` alone maps read+write for every holder ([`hand_over`]'s comment).
const FS_PAGE_FRAME: u64 = 4;
/// Everything a connecting client's own channel, a caretaker, and a client budget are all built
/// from, `WRITE | GRANT`. Never given away.
const CONSTRUCTION_UT: u64 = 5;
/// One [`login_contract::ATTRIBUTED`] message per successful login, `WRITE`. It must be drained,
/// or the first login blocks this process forever (`system_initializer::boot` wires
/// `login_audit_receiver`; the kernel harness drains via `Wiring::audit`).
const AUDIT: u64 = 6;
/// **The interactive terminal** (milestone 49's terminal update), `WRITE | GRANT`: the endpoint
/// `crates/system_initializer::boot` hands the shell. This process never reads a keystroke or
/// writes a line; it hands a narrowed `WRITE` copy to whichever session holds it (notes/login.md,
/// "The terminal: the design argument"), and `GRANT` is what lets [`delegate`] do that.
const TERM_EP: u64 = 7;
/// **The run-unvouched capability** (DECISIONS §219 gate D2), `WRITE | GRANT`, when the spawner
/// placed one: the endpoint the progenitor receives a presentation on. Delegated `WRITE` alone to
/// every session this process builds ([`serve_login`]), so a session can present it and cannot
/// pass it on. A named slot rather than the ninth, because a spawner that has none leaves it
/// empty and the other eight where they were; probed once at [`_start`] ([`HOLDS_RUN_UNVOUCHED`]).
const RUN_UNVOUCHED: u64 = grant_plan::spawnproto::RUN_UNVOUCHED_SLOT;
/// One page of the file service's durable window, `WRITE | GRANT` (milestone 152, Fork 8 D): the
/// channel a durable session's timetable reads the store through. [`FS_EP`] is badged with the
/// window's number for the two store caretakers ([`open_schedule`]). The window is shared by a
/// session's two caretakers and its timetable and nothing else.
const DURABLE_WINDOW: u64 = login_contract::DURABLE_WINDOW_SLOT;
// `grant_plan` states the slot without depending on `abi`; the relation is held by each reader.
const _: () = assert!(RUN_UNVOUCHED == abi::fault::FAULT_EP_SLOT - 1);

/// Whether [`RUN_UNVOUCHED`] holds a capability. Probed at [`_start`], before this process has
/// allocated anything, which is what makes the probe sound: later, a slot this process retyped
/// into could be the one being asked about.
static HOLDS_RUN_UNVOUCHED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// **Where this process maps the file service's shared page itself, to read the owner's
/// run-unvouched list** ([`listed`]). Below [`CRED_VA`] and the per-channel window above it, which
/// is the only thing in this space that grows.
const LIST_VA: u64 = address_space_map::pair_page(0x0000_0000_00e2_0000);

/// Whether [`LIST_VA`] holds the file page. Mapped once, at [`_start`], from [`OWN_UT_PAGES`]'
/// region, before any session exists, so its page tables sit at the bottom of that watermark and
/// never between a session's carves. `false` lists nobody.
static LIST_MAPPED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// The page shared with the credential service, for the relayed `VERIFY`.
const CRED_VA: u64 = address_space_map::pair_page(0x0000_0000_00e3_0000);
/// The base of the scratch VA range [`connect`] takes one fresh page from per channel it mints: a
/// VA is never reused, since `page_frame::MAP` refuses an already-mapped one and the page held a
/// secret. Numbered in the same family as `credentialer.rs`'s request pages, in another process.
const CONNECT_VA_BASE: u64 = address_space_map::pair_page(0x0000_0000_00e4_0000);
/// One channel's whole cost: two `RETYPE_OBJ`s (request, result), one `RETYPE` (the staging page),
/// and the page tables `page_frame::MAP` needs for that page's own mapping. Three pages minimum,
/// with margin over a tight count for the same reason [`CARETAKER_REGION_PAGES`] is (a region too
/// small fails as `Err(())`, which [`connect`] can only answer with `login_contract::DENIED`).
const CHANNEL_REGION_PAGES: u64 = 8;

/// **A budget of its own that only [`connect`] ever spends, so a served channel's pages actually
/// come home.** Four channels' worth, though only one is ever live at a time. A channel region
/// carved here is always this budget's only live child, so always the LIFO top and always fully
/// returned; nothing else may ever be split or retyped from it, or that stops holding. See
/// notes/login/budgets-and-reclamation.md, "Why a channel has a budget of its own".
const CHANNEL_UT_PAGES: u64 = 32;

/// Where a built caretaker and the file service's shared page meet. Must match
/// `components/src/fs_subtree_caretaker.rs`'s `PAGE_VA` (the same address every caretaker in this tree
/// uses, since the caretaker itself hardcodes it and this process copies its ELF, not its address).
const CARETAKER_FS_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);

/// This process's own scratch: page tables for [`build_child`]'s own temporary mappings (never a
/// child's), spent only by [`mint`]'s `build_child` calls. `supervision_protocol::fill_and_map`'s
/// scratch cursor wraps inside `SCRATCH_WINDOW`, so the most it can cost is `SCRATCH_TABLE_PAGES`
/// (32); a whole aarch64 suite run measured one page, so 128 is margin, not a tight count. See
/// notes/login/budgets-and-reclamation.md, "Four budgets, each with its own parent".
const OWN_UT_PAGES: u64 = 128;

/// One caretaker's whole construction: its address space, TCB, and stack.
/// `crates/system_initializer::DIR_JOB_REGION_PAGES` (96) covers a caretaker **and** the program
/// behind it; this process builds only the caretaker, so a smaller region should hold it, with
/// margin rather than a tight fit (a region too small fails as `Err(())` mid-login, which this
/// process can only answer with `login_contract::DENIED`).
const CARETAKER_REGION_PAGES: u64 = 64;

/// Stack pages beyond the one `build_child` maps, matching
/// `crates/system_initializer::CARETAKER_STACK_PAGES`: measured for that program rather than
/// guessed, and this is the same program.
const CARETAKER_STACK_PAGES: u64 = 4;

/// What this process hands each authenticated principal as its own budget. Arbitrary and modest,
/// for the demonstration this slice is; a real deployment sizes it against what a session actually
/// needs, which is not yet a question this program has enough callers to answer.
const CLIENT_BUDGET_PAGES: u64 = login_contract::durable::CLIENT_BUDGET_PAGES;

/// **The budget durable sessions are split from** (milestone 152 (durable delegation)), split once at start-up and only
/// when a schedule can be opened at all. Its own parent for the reason [`CHANNEL_UT_PAGES`] is:
/// a durable session outlives the logins around it, so carving it from [`CONSTRUCTION_UT`] would
/// leave a hole there each time one is reclaimed out of order. Room for exactly one, which is the
/// binding term of [`DURABLE_SESSIONS`] even once the table allows more (BUGS). `_start` splits
/// this, [`OWN_UT_PAGES`] and [`CHANNEL_UT_PAGES`] before serving anyone and stops at `fail(2)` if
/// `LOGIN_CONSTRUCTION_PAGES` is short; its owners derive that from
/// `login_contract::durable::BUDGET_PAGES`.
const DURABLE_UT_PAGES: u64 = DURABLE_BUDGET_PAGES;
/// **How many durable sessions this process keeps**: the fewer of what its capability table
/// holds beside one ordinary login (`login_contract::durable::sessions_held`, derived from
/// `abi::CAPABILITY_TABLE_SLOTS`, so a wider table raises it with no edit here) and what
/// [`DURABLE_UT_PAGES`] has memory for, and one, because the durable window is one channel and two
/// timetables staging in one page would overwrite each other (Fork 8 D). Raising it past one takes
/// a window and a budget per session from this process's spawners.
const DURABLE_SESSIONS: usize = {
    let slots = login_contract::durable::sessions_held(abi::CAPABILITY_TABLE_SLOTS);
    let memory = (DURABLE_UT_PAGES / DURABLE_BUDGET_PAGES) as usize;
    let fewer = if slots < memory { slots } else { memory };
    if fewer < 1 { fewer } else { 1 }
};
/// A durable session's budget: the client's own spending, plus `user_timetable_keeper` and its
/// timetable, both built from regions split off it (`login_contract::durable::BUDGET_PAGES`).
const DURABLE_BUDGET_PAGES: u64 = login_contract::durable::BUDGET_PAGES;
/// `user_timetable_keeper`'s construction: its segments, its stack, the timetable's image copied in,
/// its tables, the registration page, and the two store caretakers
/// (`login_contract::durable::USER_TIMETABLE_KEEPER_REGION_PAGES`).
const USER_TIMETABLE_KEEPER_REGION_PAGES: u64 =
    login_contract::durable::USER_TIMETABLE_KEEPER_REGION_PAGES;
/// `user_timetable_keeper`'s own budget: its timetable's region and the budget its jobs fire from
/// (`components/src/user_timetable_keeper.rs`), and the two endpoints.
const USER_TIMETABLE_KEEPER_BUDGET_PAGES: u64 =
    login_contract::durable::USER_TIMETABLE_KEEPER_BUDGET_PAGES;
/// Stack pages for `user_timetable_keeper`, beyond `build_child`'s default.
const USER_TIMETABLE_KEEPER_STACK_PAGES: u64 = 8;
/// Where this process maps the first durable session's registration page, to read the timetable's
/// exit word; the k-th kept session's is [`DURABLE_PAGE_STRIDE`] times k above it. Far above
/// `CONNECT_VA_BASE`, which grows a page per connect for ever.
const DURABLE_PAGE_VA: u64 = 0x0000_0000_0300_0000;
/// The distance between two kept sessions' registration pages: 64 KiB, a page on every
/// architecture this tree builds for.
const DURABLE_PAGE_STRIDE: u64 = 0x1_0000;
/// How long [`Durable::end`] waits for a timetable to stop, in seconds of counter time: a job
/// already running finishes first, and a scheduled job is short-lived by design.
const END_WAIT_SECS: u64 = 5;

#[unsafe(no_mangle)]
pub extern "C" fn _start(caretaker_len: u64, table_len: u64, schedule_len: u64) -> ! {
    // First, before anything is allocated: see [`HOLDS_RUN_UNVOUCHED`].
    HOLDS_RUN_UNVOUCHED.store(
        user_mode_runtime::is_granted(RUN_UNVOUCHED),
        core::sync::atomic::Ordering::Relaxed,
    );
    let (care_elf, table) = vouched_caretaker(caretaker_len, table_len);

    // **What a durable session is built from** (milestone 152): `user_timetable_keeper` and
    // `timetable`, vouched like the caretaker, which a session's two store caretakers also need.
    // Anything unvouched and no schedule opens on this boot: `SCHEDULE` is answered as `LOGIN` is.
    let schedule = care_elf
        .as_ref()
        .and_then(|care| schedule_images(schedule_len, care, table));

    let Ok(own_ut) = memory_region_split(CONSTRUCTION_UT, OWN_UT_PAGES) else {
        fail(1)
    };
    // Split once, and only when a schedule can be opened at all: see [`DURABLE_UT_PAGES`].
    let durable_ut = match schedule {
        Some(_) => memory_region_split(CONSTRUCTION_UT, DURABLE_UT_PAGES).ok(),
        None => None,
    };
    let mut durables = Durables::new();
    // Why the start-up pass below skipped what it skipped (milestone 152, 2026-09-27): one count
    // per `login_contract::durable::RederiveSkip` reason, read back over `REDERIVE_SKIPS`.
    let mut rederive_skips = [0u32; login_contract::durable::REDERIVE_SKIP_REASONS];
    // Split once, here, and never anywhere else: [`CHANNEL_UT_PAGES`]' own doc explains why a
    // channel's region must come from a budget nothing else spends.
    let Ok(channel_ut) = memory_region_split(CONSTRUCTION_UT, CHANNEL_UT_PAGES) else {
        fail(2)
    };
    // The owner's list is read through the file page ([`listed`]). A map that fails leaves every
    // session without the run-unvouched capability, which is the list's own default.
    LIST_MAPPED.store(
        map_page_frame(FS_PAGE_FRAME, LIST_VA, true, own_ut),
        core::sync::atomic::Ordering::Relaxed,
    );

    // **Boot re-derivation, before the front door opens** (milestone 152; DECISIONS §123 (the
    // boot-time re-derivation privilege) as amended): see [`rederive`]. No session is live yet, so
    // the file page is this process's alone.
    if let (Some(images), Some(ut)) = (schedule, durable_ut) {
        rederive(own_ut, images, ut, &mut durables, &mut rederive_skips);
    }

    let mut door = FrontDoor {
        own_ut,
        channel_ut,
        care: care_elf.as_ref(),
        schedule,
        durable_ut,
        durables,
        rederive_skips,
        seq: 0,
        connect_seq: 0,
        terminal_held: false,
    };
    loop {
        let (w0, _w1, _w2) = receive(REQUEST);
        door.serve(w0);
    }
}

/// The caretaker's image and the measurement table the spawner mapped, the image only if the table
/// vouches for it. A caretaker that is not vouched for is not a reason to refuse to start: every
/// login is answered `DENIED` instead, since `crates/system_initializer` treats this program as
/// optional.
fn vouched_caretaker(
    caretaker_len: u64,
    table_len: u64,
) -> (Option<elf::Elf<'static>>, &'static str) {
    // **Two blobs rather than the archive** (milestone 233 (`login` dies on every boot)): the
    // caretaker's ELF and the measurement table, their lengths in the first two argument registers
    // (notes/login/budgets-and-reclamation.md, "Two blobs rather than the archive").
    //
    // SAFETY: the spawner maps `caretaker_len` bytes read-only at `CARETAKER_ELF_VA`, for the
    // lifetime of this process, before `_start` runs (`login_contract::CARETAKER_ELF_VA`'s own
    // contract). Both spawners do: `crates/system_initializer` through `supervision_protocol`'s
    // `blobs`, and `kernel::user::login_service::start` through `map_new`. A zero length is a slice
    // of nothing rather than a read of nothing, which is the case below.
    let care_bytes = unsafe {
        core::slice::from_raw_parts(
            login_contract::CARETAKER_ELF_VA as *const u8,
            caretaker_len as usize,
        )
    };

    // Read as `crates/system_initializer` reads it (milestone 104 (the measurement continues past
    // init)): bytes that are not UTF-8 are the empty table, which vouches for nothing. Checked
    // once: both blobs are read-only for the life of this process. It is a consistency check on the
    // progenitor's hand-over, not an independent verification (notes/login.md, "Measured boot: the
    // posture").
    //
    // SAFETY: as above, for `table_len` bytes at `PROGRAM_MEASUREMENTS_VA`.
    let table = unsafe {
        core::slice::from_raw_parts(
            login_contract::PROGRAM_MEASUREMENTS_VA as *const u8,
            table_len as usize,
        )
    };
    let table = core::str::from_utf8(table).unwrap_or("");
    // An empty table, an unparseable image and a zero-length blob all end here as `None`, answered
    // by the `None` arm of `mint`.
    let care_elf = elf::Elf::parse(care_bytes).ok().filter(|_| {
        measured_boot::verify_in_manifest(table, "fs_subtree_caretaker", care_bytes).is_ok()
    });
    (care_elf, table)
}

/// The two durable-session images the spawner mapped, vouched for against `table` beside the
/// caretaker `care`; `None` when either is missing or unvouched.
fn schedule_images<'e>(
    schedule_len: u64,
    care: &'e elf::Elf<'static>,
    table: &str,
) -> Option<ScheduleImages<'e>> {
    let (keeper_len, timetable_len) = login_contract::split_schedule_lengths(schedule_len);
    // SAFETY: the spawner maps each image's bytes read-only at its address for the life of this
    // process before `_start` runs (`login_contract::USER_TIMETABLE_KEEPER_ELF_VA`'s contract).
    let (keeper_bytes, timetable_bytes) = unsafe {
        (
            core::slice::from_raw_parts(
                login_contract::USER_TIMETABLE_KEEPER_ELF_VA as *const u8,
                keeper_len as usize,
            ),
            core::slice::from_raw_parts(
                login_contract::TIMETABLE_ELF_VA as *const u8,
                timetable_len as usize,
            ),
        )
    };
    vouched_schedule(keeper_bytes, timetable_bytes, care, table)
}

/// **The front door's state**, carried across requests: the budgets this process split at start-up,
/// what a login is built from, the durable sessions it keeps, and the counters below.
struct FrontDoor<'e> {
    own_ut: u64,
    channel_ut: u64,
    care: Option<&'e elf::Elf<'static>>,
    schedule: Option<ScheduleImages<'e>>,
    durable_ut: Option<u64>,
    durables: Durables,
    /// The start-up pass's skip counts, one per reason; see `_start`.
    rederive_skips: [u32; login_contract::durable::REDERIVE_SKIP_REASONS],
    // How many logins this process has established, in order: the audit trail's sequence number,
    // not a capacity (`CONSTRUCTION_UT` bounds how many logins it can serve).
    seq: u64,
    // How many channels this process has minted, in order: [`connect`]'s own bump allocator for a
    // fresh scratch VA per channel (see `CONNECT_VA_BASE`'s own doc on why a VA is never reused).
    connect_seq: u64,
    // **Whether [`TERM_EP`] is on loan to a session** (milestone 49's terminal update), `false` at
    // start-up. notes/login.md, "The terminal: the design argument", has the design.
    terminal_held: bool,
}

impl FrontDoor<'_> {
    /// Answer one request on the front door.
    fn serve(&mut self, w0: u64) {
        let operation = login_contract::operation(w0);
        if operation == login_contract::LOGOUT {
            // On the front door itself: it carries no secret, so a shared endpoint exposes nothing
            // (`login_contract`'s BUGS says what it does not authenticate). Idempotent.
            self.terminal_held = false;
            send(RESULT, login_contract::LOGGED_OUT, 0, 0);
            return;
        }
        if operation == login_contract::SUSPEND {
            // **The §108 (disabling credentials kills the durable session) cascade** (milestone
            // 152): end the durable session of everyone on the owner's suspended list.
            // Unauthenticated, like `LOGOUT`, for the reason `login_contract::SUSPEND` gives. Not
            // while a session holds the terminal, whose caretaker shares the file page the list is
            // read through ([`listed`]); the cascade then runs at that identity's next login.
            self.suspend_listed();
            return;
        }
        if operation == login_contract::REDERIVE_SKIPS {
            // Unauthenticated, like `SUSPEND` and `LOGOUT`: see `login_contract::REDERIVE_SKIPS`'s
            // own doc for why this names no identity and costs nothing to keep in production.
            send(
                RESULT,
                login_contract::SKIP_COUNTS,
                login_contract::durable::pack_skip_counts(&self.rederive_skips),
                0,
            );
            return;
        }
        if operation != login_contract::CONNECT {
            // The front door's only other legal word; see `login_contract`'s own module docs. Not an
            // authentication outcome (no identity has been presented yet), so `MALFORMED` rather
            // than `DENIED`.
            send(RESULT, login_contract::MALFORMED, 0, 0);
            return;
        }
        self.serve_connect();
    }

    /// `SUSPEND`: end the durable session of everyone on the owner's suspended list, and say how
    /// many ended.
    fn suspend_listed(&mut self) {
        let mut ended = 0;
        for kept in &mut self.durables.held {
            if !self.terminal_held
                && kept
                    .as_ref()
                    .is_some_and(|d| on_list(login_contract::SUSPENDED_LIST, d.name()))
                && let Some(d) = kept.take()
            {
                match d.end() {
                    Ok(()) => ended += 1,
                    Err(d) => *kept = Some(d),
                }
            }
        }
        send(RESULT, login_contract::APPLIED, ended, 0);
    }

    /// `CONNECT`: mint a private channel, serve one login on it, and reclaim it.
    fn serve_connect(&mut self) {
        let Some(channel) = connect(self.channel_ut, self.connect_seq) else {
            // The channel budget is spent; folded into `DENIED` for
            // `login_contract::DENIED`'s own stated reason, even though no identity is in play yet:
            // this program has exactly one code for "authenticated or not, I could not serve you".
            send(RESULT, login_contract::DENIED, 0, 0);
            return;
        };
        self.connect_seq += 1;
        send(RESULT, login_contract::CONNECTED, 0, 0);
        // Delegate narrowed copies and **keep our own for the width of this one exchange**: unlike
        // `FS_PAGE_FRAME` (shared with every future client, forever), this channel is this process's
        // for exactly as long as `serve_login` is running and no longer, so its objects are reclaimed
        // the moment it returns (below).
        delegate(RESULT, channel.request, abi::rights::WRITE);
        delegate(RESULT, channel.result, abi::rights::READ);
        delegate(RESULT, channel.page, abi::rights::READ | abi::rights::WRITE);

        serve_login(
            &channel,
            self.own_ut,
            self.care,
            &mut self.seq,
            &mut self.terminal_held,
            &mut Schedules {
                archive: self.schedule,
                durable_ut: self.durable_ut,
                durables: &mut self.durables,
            },
        );
        // Destroy the channel's region rather than only deleting our capabilities to its pieces:
        // its rendezvous live in the kernel's global registry until the region goes, and `DESTROY`
        // leaves our `MemoryRegion` and `Rendezvous` capabilities in the table, so `result` is
        // deleted and the region discarded (notes/login/budgets-and-reclamation.md).
        // `channel.request` and `channel.page` are gone already ([`authenticate`]).
        cap_delete(channel.result);
        discard(channel.region);
    }
}

/// **Serve one login on its own private channel**: relay the presented identity and secret to the
/// credential service, and on success mint a fresh capability set. `seq` is the audit trail's
/// counter, shared across every channel (not the connect sequence `CONNECT_VA_BASE` counts).
/// [`authenticate`] drops `channel.request` and `channel.page` as soon as each stops being needed,
/// so [`mint`]'s peak use of the capability table comes with the channel down to one slot
/// (notes/login/budgets-and-reclamation.md, "The capability table, in order").
fn serve_login(
    channel: &Channel,
    own_ut: u64,
    care: Option<&elf::Elf>,
    seq: &mut u64,
    terminal_held: &mut bool,
    schedules: &mut Schedules,
) {
    let Some(Authenticated {
        wants_schedule,
        identity_buf,
        identity_len,
        hint,
    }) = authenticate(channel, *terminal_held)
    else {
        return;
    };
    // **Suspended** (milestone 152, the §108 ruling): checked after authentication, so only the
    // holder of the secret learns it. A durable session this identity still has ends here too, so
    // the cascade holds even if the owner's console never sent `SUSPEND`.
    if is_suspended(&identity_buf[..identity_len]) {
        if let Some(k) = schedules.durables.find(&identity_buf[..identity_len])
            && let Some(d) = schedules.durables.held[k].take()
            && let Err(d) = d.end()
        {
            schedules.durables.held[k] = Some(d);
        }
        send(channel.result, login_contract::SUSPENDED, 0, 0);
        return;
    }

    let identity = &identity_buf[..identity_len];

    let Some((budget, page, keep_budget)) =
        session_budget(own_ut, identity, wants_schedule, schedules, channel)
    else {
        return;
    };

    // **The client's own caretaker, minted after any durable session is opened**, and the order is
    // for the capability table: a durable session's two store caretakers (Fork 8 D) are built while
    // this process holds as little as it can, and one ordinary login's peak comes after, as it
    // does beside a kept session. Counted in `login_contract::durable`.
    let Some((dir_ep, region)) = mint(own_ut, care, identity) else {
        // Authenticated, and still not servable (budget spent, or the caretaker's descent refused):
        // answered as a wrong secret is, for `login_contract::DENIED`'s reason. A durable session
        // opened above stays kept, as one re-derived at start-up does, for its user's next login.
        if !keep_budget {
            discard(budget);
        }
        send(channel.result, login_contract::DENIED, 0, 0);
        return;
    };

    hand_over(channel, identity, dir_ep, region, budget, page);
    *terminal_held = true;
    cap_delete(dir_ep);
    cap_delete(region);
    // A durable session's budget stays named here, which is the whole of reattachment.
    if !keep_budget {
        cap_delete(budget);
    }
    send(AUDIT, login_contract::ATTRIBUTED, *seq, hint);
    *seq += 1;
}

/// What [`authenticate`] carries out of the request page, which is wiped by the time it returns.
struct Authenticated {
    /// `SCHEDULE` rather than `LOGIN`.
    wants_schedule: bool,
    /// An owned copy of the identity: its first `identity_len` bytes.
    identity_buf: [u8; login_contract::MAX_IDENTITY],
    identity_len: usize,
    /// [`login_contract::identity_hint`] of the identity, for the audit record.
    hint: u64,
}

/// **Receive the one request on `channel`, and relay its identity and secret to the credential
/// service.** `None` once the refusal has been sent (malformed, the terminal is held, or a wrong
/// secret); either way `channel.request` and `channel.page` are dropped and the page wiped.
fn authenticate(channel: &Channel, terminal_held: bool) -> Option<Authenticated> {
    let (w0, _w1, _w2) = receive(channel.request);
    cap_delete(channel.request);
    // `LOGIN`, or `SCHEDULE`: log in and open this identity's schedule (milestone 152).
    let wants_schedule = login_contract::operation(w0) == login_contract::SCHEDULE;
    // SAFETY: `connect` mapped one page read/write at `channel.va` before delegating `channel.page`
    // to the same client this request now arrives from.
    let page =
        unsafe { core::slice::from_raw_parts(channel.va as *const u8, login_contract::PAGE) };
    let Some((identity, secret)) = login_contract::read(page, w0) else {
        wipe_page(channel.va);
        cap_delete(channel.page);
        send(channel.result, login_contract::MALFORMED, 0, 0);
        return None;
    };
    // **The terminal check runs before anything about `identity` or `secret` is acted on**: it is
    // global state, the same for every caller, so checking it first gives no per-identity timing or
    // outcome oracle, and it saves a credential round trip while the terminal is spoken for.
    if terminal_held {
        wipe_page(channel.va);
        cap_delete(channel.page);
        send(channel.result, login_contract::NO_TERMINAL, 0, 0);
        return None;
    }
    // Computed before the page is wiped: `identity` borrows `channel.va` and must not be read after.
    let hint = login_contract::identity_hint(identity);
    // **Copy the identity out before it is gone**: the page is wiped right after the credential
    // relay, and `mint` needs the identity's bytes to name the subtree (DECISIONS §117) after that.
    // `login_contract::read` has already checked it fits `login_contract::MAX_IDENTITY`.
    let mut identity_buf = [0u8; login_contract::MAX_IDENTITY];
    let identity_len = identity.len();
    identity_buf[..identity_len].copy_from_slice(identity);

    // SAFETY: the wiring mapped one page read/write at CRED_VA before this process ran, shared
    // with the credential service and with nothing else.
    let cred_page =
        unsafe { core::slice::from_raw_parts_mut(CRED_VA as *mut u8, credential_contract::PAGE) };
    let placed = credential_contract::place(
        cred_page,
        identity,
        secret,
        credential_contract::verify::VERIFY,
    );
    // The presented secret has now been copied to CRED_VA (or the placement failed and never will
    // be); either way `channel.va`'s copy is done being read.
    wipe_page(channel.va);
    cap_delete(channel.page);
    let Some(cw0) = placed else {
        send(channel.result, login_contract::MALFORMED, 0, 0);
        return None;
    };
    let (cr0, _) = call(VERIFY, cw0, 0);
    credential_contract::wipe(cred_page);

    if !credential_contract::is_authenticated(cr0) {
        send(channel.result, login_contract::DENIED, 0, 0);
        return None;
    }
    Some(Authenticated {
        wants_schedule,
        identity_buf,
        identity_len,
        hint,
    })
}

/// **The budget this login's session spends**, reattaching or opening a durable session where one
/// is due: `(budget, registration page, whether the budget stays named here)`. `None` once
/// `DENIED` has been sent because no budget could be split.
fn session_budget(
    own_ut: u64,
    identity: &[u8],
    wants_schedule: bool,
    schedules: &mut Schedules,
    channel: &Channel,
) -> Option<(u64, Option<u64>, bool)> {
    // **An identity whose session is already durable gets that session back** (milestone 152,
    // reattachment). If its timetable has stopped, the session is retired first and this login is
    // an ordinary one; see [`Durable::exited`].
    if let Some(k) = schedules.durables.find(identity)
        && schedules.durables.held[k]
            .as_ref()
            .is_some_and(Durable::exited)
        && let Some(d) = schedules.durables.held[k].take()
    {
        // The timetable stopped because its user emptied it: nothing is pending, so the next
        // start-up has nothing to bring back for them either.
        d.retire(true);
    }
    let reattach = schedules
        .durables
        .find(identity)
        .and_then(|k| schedules.durables.held[k].as_ref())
        .map(|d| (d.budget, d.page));

    // The budget: the durable session's own on a reattach, else a fresh one, from
    // [`DURABLE_UT_PAGES`]'s budget when a session opens. **A stored schedule comes back at login**
    // (milestone 152; calef's §108 ruling): an identity with a non-empty `schedule` in its subtree
    // and no durable session gets one opened, as though it had asked, with that document in force.
    let room = schedules.durables.free();
    let can_open = reattach.is_none() && room.is_some() && schedules.archive.is_some();
    let mut stored = [0u8; login_contract::PAGE];
    let stored_len = if can_open {
        read_stored_schedule(identity, &mut stored).unwrap_or(0)
    } else {
        0
    };
    let opening = can_open && (wants_schedule || stored_len > 0);
    let (budget, page, keep_budget) = match reattach {
        Some((budget, page)) => (budget, Some(page), true),
        None => {
            let split = match (opening, schedules.durable_ut) {
                (true, Some(ut)) => memory_region_split(ut, DURABLE_BUDGET_PAGES),
                _ => memory_region_split(CONSTRUCTION_UT, CLIENT_BUDGET_PAGES),
            };
            let Ok(budget) = split else {
                send(channel.result, login_contract::DENIED, 0, 0);
                return None;
            };
            let opened = match (opening, schedules.archive, room) {
                (true, Some(images), Some(k)) => {
                    Durable::open(own_ut, images, budget, identity, k).map(|d| (k, d))
                }
                _ => None,
            };
            match opened {
                Some((k, d)) => {
                    if stored_len > 0 {
                        // The client reads the verdict in the page, so a refused document is its
                        // to see, and the session stays.
                        d.restore(&stored[..stored_len]);
                    }
                    record_in_manifest(identity, true);
                    let page = d.page;
                    schedules.durables.held[k] = Some(d);
                    (budget, Some(page), true)
                }
                None => (budget, None, false),
            }
        }
    };
    Some((budget, page, keep_budget))
}

/// **Answer `OK` and delegate the session's capabilities**, in the order the client reads them.
fn hand_over(
    channel: &Channel,
    identity: &[u8],
    dir_ep: u64,
    region: u64,
    budget: u64,
    page: Option<u64>,
) {
    // Read after the session is built, so a list read that fails costs the grant and nothing else
    // (DECISIONS §221 ruling 2: only a listed identity's session gets it).
    let run_unvouched =
        HOLDS_RUN_UNVOUCHED.load(core::sync::atomic::Ordering::Relaxed) && listed(identity);
    let mut flags = 0;
    if run_unvouched {
        flags |= login_contract::RUN_UNVOUCHED_FOLLOWS;
    }
    if page.is_some() {
        flags |= login_contract::SCHEDULE_FOLLOWS;
    }
    send(channel.result, login_contract::OK, flags, 0);
    delegate(channel.result, dir_ep, abi::rights::WRITE);
    // `WRITE` alone: the kernel's `page_frame_map` grants a read+write mapping on `WRITE`, and
    // `crates/system_initializer::boot` holds only `WRITE | GRANT` on this page, so a real boot
    // could never delegate `READ` here (`script/swish-check` refused exactly that `SEND_CAP`).
    delegate(channel.result, FS_PAGE_FRAME, abi::rights::WRITE);
    // Every right (milestone 824 (a retype mints no right its budget lacks)): the session builds
    // from it, and what it builds carries the budget's rights.
    delegate(channel.result, budget, abi::rights::ALL);
    // The logout ticket: `WRITE` is the one right `MemoryRegion::DESTROY` needs (notes/login.md,
    // "Reclaiming a session: the design argument"). Not `GRANT`: a client that could delegate its
    // logout ticket could hand another principal the means to end this one's session.
    delegate(channel.result, region, abi::rights::WRITE);
    // **The terminal, fifth** (milestone 49's terminal update). Reaching here already proved
    // `!*terminal_held`, so this is never refused. `WRITE` only: a login session gets to write the
    // terminal, never to hand the capability to read keystrokes on to anything it spawns.
    delegate(channel.result, TERM_EP, abi::rights::WRITE);
    // **The run-unvouched capability, sixth, and only as announced** (DECISIONS §219 gate D2;
    // `login_contract::OK`'s doc). `WRITE` alone: the session may present it to the progenitor
    // and may hand it to nothing.
    if run_unvouched {
        delegate(channel.result, RUN_UNVOUCHED, abi::rights::WRITE);
    }
    // **The registration page, last, and only as announced** (milestone 152). `WRITE` alone: the
    // client writes a document into it and reads the plan back, and has no reason to lend it.
    if let Some(page) = page {
        delegate(channel.result, page, abi::rights::WRITE);
    }
}

/// **The schedule side of this process's state** (milestone 152), passed to [`serve_login`] as one
/// argument rather than three.
struct Schedules<'a, 'e> {
    /// What a durable session is built from, vouched, or `None` when no schedule can be opened on
    /// this boot.
    archive: Option<ScheduleImages<'e>>,
    /// The budget durable sessions are split from: see [`DURABLE_UT_PAGES`].
    durable_ut: Option<u64>,
    /// The durable sessions this process keeps.
    durables: &'a mut Durables,
}

/// **The durable sessions this process keeps**, at most [`DURABLE_SESSIONS`] (milestone 152).
/// Slot `k` maps its registration page at `DURABLE_PAGE_VA + k * DURABLE_PAGE_STRIDE`, so a
/// session's page address is fixed by where it is kept.
struct Durables {
    held: [Option<Durable>; DURABLE_SESSIONS],
}

impl Durables {
    fn new() -> Self {
        Durables {
            held: [const { None }; DURABLE_SESSIONS],
        }
    }

    /// Where `identity`'s session is kept, if it has one.
    fn find(&self, identity: &[u8]) -> Option<usize> {
        self.held
            .iter()
            .position(|d| d.as_ref().is_some_and(|d| d.is(identity)))
    }

    /// A slot with nothing kept in it, if there is one.
    fn free(&self) -> Option<usize> {
        self.held.iter().position(Option::is_none)
    }
}

/// **Re-derive, at start-up, every durable session the manifest names** (milestone 152; Fork 7,
/// option A, calef's ruling of 2026-09-27; DECISIONS §123 as amended that day): for each identity
/// the manifest (§125) lists and the suspended list does not, in order, until [`DURABLE_SESSIONS`]
/// are kept, open its session ([`Durable::open`]) and put its stored schedule (§122) in force
/// ([`Durable::restore`]). No credential is needed: the session goes to nobody until its user logs
/// in and the credential service says yes, the ordinary reattachment path in [`serve_login`].
/// Skipped, and counted per reason in `skips` for `REDERIVE_SKIPS`: no stored schedule or an empty
/// one, a name too long to keep, one already kept, a budget or session that cannot be built. A
/// refused document ends its session and keeps the manifest line, so its user sees the refusal at
/// their next login, where the same document is restored with a client there to read the verdict.
fn rederive(
    own_ut: u64,
    images: ScheduleImages<'_>,
    durable_ut: u64,
    durables: &mut Durables,
    skips: &mut [u32; login_contract::durable::REDERIVE_SKIP_REASONS],
) {
    use login_contract::durable::RederiveSkip;
    let mut record = |reason: RederiveSkip| {
        let c = &mut skips[reason as usize];
        *c = c.saturating_add(1);
    };
    use filesystem_contract::fs;
    let mut manifest = [0u8; login_contract::PAGE];
    let Some(n) = read_file(
        fs::ROOT,
        timetable_store::MANIFEST_FILE_NAME.as_bytes(),
        &mut manifest,
    ) else {
        return;
    };
    let Ok(text) = core::str::from_utf8(&manifest[..n]) else {
        return;
    };
    let Ok(manifest) = timetable_store::parse_manifest(text) else {
        return;
    };
    // No list, or one that cannot be read, suspends nobody: `login_contract::SUSPENDED_LIST`'s
    // direction, the same one [`is_suspended`] takes at a login.
    let mut suspended = [0u8; login_contract::PAGE];
    let s = read_file(
        fs::ROOT,
        login_contract::SUSPENDED_LIST.as_bytes(),
        &mut suspended,
    )
    .unwrap_or(0);
    for identity in login_contract::durable::to_rederive(manifest.entries(), &suspended[..s]) {
        let Some(k) = durables.free() else {
            record(RederiveSkip::TableFull);
            break;
        };
        // `Durable` keeps a name of at most `grant::MAX_NAME` bytes, the bound `mint` holds a login
        // to; the manifest's own bound is wider.
        if !filesystem_contract::grant::fits(identity) || durables.find(identity).is_some() {
            record(RederiveSkip::Identity);
            continue;
        }
        let mut doc = [0u8; login_contract::PAGE];
        let len = read_stored_schedule(identity, &mut doc).unwrap_or(0);
        if len == 0 {
            record(RederiveSkip::NoStoredSchedule);
            continue;
        }
        let Ok(budget) = memory_region_split(durable_ut, DURABLE_BUDGET_PAGES) else {
            record(RederiveSkip::BudgetOutOfPages);
            break;
        };
        let Some(d) = Durable::open(own_ut, images, budget, identity, k) else {
            discard(budget);
            record(RederiveSkip::SessionBuildFailed);
            continue;
        };
        if d.restore(&doc[..len]) {
            durables.held[k] = Some(d);
        } else if let Err(d) = d.end() {
            // Still running past the wait: keep it, as `SUSPEND` does, so it can be ended later.
            durables.held[k] = Some(d);
        }
    }
}

/// **A durable session, as `login` keeps it** (milestone 152, S1 of 2026-09-26): the identity it
/// belongs to, and the four capabilities that let this process hand it back and, later, take it
/// down. Four slots each, which is what bounds [`DURABLE_SESSIONS`].
struct Durable {
    identity: [u8; filesystem_contract::grant::MAX_NAME],
    len: usize,
    /// The user's budget. `user_timetable_keeper` and its timetable are built from regions split off
    /// it, so it refuses `DESTROY` for as long as either lives (DECISIONS §16 (object revocation)).
    budget: u64,
    /// The region `user_timetable_keeper` was built from; the registration page was retyped from it.
    keeper: u64,
    /// The registration page, mapped at [`Durable::va`] so this process can read the timetable's
    /// exit word.
    page: u64,
    /// Where the registration page is mapped: [`DURABLE_PAGE_VA`] plus the slot's stride.
    va: u64,
    /// `user_timetable_keeper`'s readiness endpoint, which it also says `STOPPED` on once it has given
    /// its budget back. Retyped from [`Durable::keeper`].
    ready: u64,
}

impl Durable {
    /// **Open a durable session for `identity` on `budget`**, to be kept in slot `k` of
    /// [`Durables`]: build its `user_timetable_keeper` ([`open_schedule`]) with the registration page
    /// mapped at that slot's address. The one way a durable session comes to exist, at a login
    /// and at start-up ([`rederive`]) alike. `None` leaves `budget` childless and the caller's.
    fn open(
        own_ut: u64,
        images: ScheduleImages<'_>,
        budget: u64,
        identity: &[u8],
        k: usize,
    ) -> Option<Self> {
        let va = DURABLE_PAGE_VA + k as u64 * DURABLE_PAGE_STRIDE;
        let (keeper, page, ready) = open_schedule(own_ut, budget, images, va)?;
        let mut id = [0u8; filesystem_contract::grant::MAX_NAME];
        id[..identity.len()].copy_from_slice(identity);
        Some(Durable {
            identity: id,
            len: identity.len(),
            budget,
            keeper,
            page,
            va,
            ready,
        })
    }

    fn is(&self, identity: &[u8]) -> bool {
        self.name() == identity
    }

    fn name(&self) -> &[u8] {
        &self.identity[..self.len]
    }

    /// **End this session now** (the §108 cascade): replace its document with an empty one through
    /// the page, as its user could, wait for the timetable's exit word, then [`Durable::retire`].
    /// The timetable stops firing at once and lets a job already running finish (the fourth
    /// sub-ruling of §222 (who holds a user's schedule)), so "now" means within one job's run.
    /// `Err` hands the session back when the timetable has not stopped within [`END_WAIT_SECS`];
    /// see this program's BUGS.
    fn end(self) -> Result<(), Self> {
        use core::sync::atomic::{AtomicU64, Ordering};
        let word = |off: usize| {
            // SAFETY: `open_schedule` mapped the page read/write at `self.va` for as long as this
            // session is kept, and every offset here is an aligned word inside it
            // (`timetable::registration`'s layout).
            unsafe { &*((self.va + off as u64) as *const AtomicU64) }
        };
        let ceiling = user_mode_runtime::cntfrq().saturating_mul(END_WAIT_SECS);
        let started = user_mode_runtime::now();
        let mut asked = 0u64;
        while !self.exited() {
            if user_mode_runtime::now().wrapping_sub(started) >= ceiling {
                return Err(self);
            }
            // Ask again whenever a client's own request has overtaken ours.
            let last = registration::sequence(word(registration::REQUEST).load(Ordering::Acquire));
            if last != asked {
                asked = last + 1;
                word(registration::LEN).store(0, Ordering::Relaxed);
                word(registration::REQUEST).store(
                    registration::request(registration::REPLACE, asked),
                    Ordering::Release,
                );
            }
            yield_now();
        }
        // Suspended, not emptied: the stored schedule and the manifest line stay, so `user resume`
        // brings the schedule back at the next login and the start-up pass skips it only while the
        // mark is there.
        self.retire(false);
        Ok(())
    }

    /// **Put a stored document in force** before the page is handed out: the replace its user
    /// staged last time, staged again by this process as sequence 1. Waits for the answer, up to
    /// [`END_WAIT_SECS`]; a client reads the verdict and the plan in the page either way. `true`
    /// when the timetable answered and took the document ([`registration::STATUS_REPLACED`]).
    fn restore(&self, doc: &[u8]) -> bool {
        use core::sync::atomic::{AtomicU64, Ordering};
        let n = doc.len().min(registration::BODY_MAX);
        // SAFETY: as in `end`: the page is mapped read/write at `self.va`, and nothing else
        // writes it until it is handed out after this returns.
        let page = unsafe {
            core::slice::from_raw_parts_mut(self.va as *mut u8, registration::PAGE_BYTES)
        };
        page[registration::BODY..registration::BODY + n].copy_from_slice(&doc[..n]);
        let word = |off: usize| {
            // SAFETY: an aligned word inside the page above.
            unsafe { &*((self.va + off as u64) as *const AtomicU64) }
        };
        word(registration::LEN).store(n as u64, Ordering::Relaxed);
        word(registration::REQUEST).store(
            registration::request(registration::REPLACE, 1),
            Ordering::Release,
        );
        let ceiling = user_mode_runtime::cntfrq().saturating_mul(END_WAIT_SECS);
        let started = user_mode_runtime::now();
        while word(registration::REPLY).load(Ordering::Acquire) != 1 {
            if user_mode_runtime::now().wrapping_sub(started) >= ceiling {
                return false;
            }
            yield_now();
        }
        word(registration::STATUS).load(Ordering::Relaxed) == registration::STATUS_REPLACED
    }

    /// **Whether the timetable has stopped**, read from the exit word it writes into the page just
    /// before it exits (`timetable::registration::EXIT`). This is the liveness test reattachment
    /// needs, and it is why the page lives in [`Durable::keeper`] rather than in the region the
    /// `user_timetable_keeper` gives back: a `DESTROY` probe cannot tell a stale budget from a busy one,
    /// because the kernel answers both `NotPermitted` (`notes/durable-delegation.md`, question 1).
    fn exited(&self) -> bool {
        // SAFETY: `open_schedule` mapped this page at `self.va`, and it stays mapped until
        // `retire` reclaims the region it lives in. The timetable writes the word; a volatile read
        // sees its latest store.
        let word = unsafe {
            core::ptr::read_volatile((self.va + registration::EXIT as u64) as *const u64)
        };
        word & registration::EXITED != 0
    }

    /// **Take a stopped session down.** `user_timetable_keeper` destroyed its own timetable budget
    /// before exiting, so what is left is the region it was built from, then the user's budget,
    /// which is childless once that region is gone. Both come home to [`DURABLE_UT_PAGES`]'s budget.
    fn retire(self, unrecord: bool) {
        // Wait for `user_timetable_keeper` to finish its own teardown: reclaiming it earlier would kill
        // it between its two destroys and strand a region under the user's budget for good. It is
        // already blocked sending this word by the time anything calls `retire` after a clean stop.
        receive(self.ready);
        cap_delete(self.ready);
        discard(self.keeper);
        cap_delete(self.page);
        discard(self.budget);
        if unrecord {
            record_in_manifest(self.name(), false);
        }
    }
}

/// **What a durable session is built from**: `user_timetable_keeper`'s and `timetable`'s images, both vouched
/// for, and the caretaker image the store caretakers are built from (milestone 152, Fork 8 D).
#[derive(Clone, Copy)]
struct ScheduleImages<'e> {
    keeper: &'static [u8],
    timetable: &'static [u8],
    care: &'e elf::Elf<'static>,
}

/// **Check `user_timetable_keeper` and `timetable` against the measurement table**, and answer them back with
/// `care` only if both pass.
fn vouched_schedule<'e>(
    keeper: &'static [u8],
    timetable: &'static [u8],
    care: &'e elf::Elf<'static>,
    table: &str,
) -> Option<ScheduleImages<'e>> {
    if keeper.is_empty() || timetable.is_empty() {
        return None;
    }
    measured_boot::verify_in_manifest(table, "user_timetable_keeper", keeper).ok()?;
    measured_boot::verify_in_manifest(table, "timetable", timetable).ok()?;
    Some(ScheduleImages {
        keeper,
        timetable,
        care,
    })
}

/// **Build a user's `user_timetable_keeper`** out of `budget` (milestone 152, S1 and L2 of 2026-09-26):
/// split its construction region and its own budget off the user's, retype the registration page
/// from the first, start it, and wait for the one word it answers with. `Some((region, page))`
/// once it says its timetable is running, with the page mapped at `va`.
///
/// A failure leaves the user with an ordinary session, which is what `SCHEDULE` degrades to.
fn open_schedule(
    own_ut: u64,
    budget: u64,
    images: ScheduleImages<'_>,
    va: u64,
) -> Option<(u64, u64, u64)> {
    let elf = elf::Elf::parse(images.keeper).ok()?;
    let timetable = images.timetable;
    let keeper = memory_region_split(budget, USER_TIMETABLE_KEEPER_REGION_PAGES).ok()?;
    // **The store, read-only, for the timetable** (Fork 8 D): a caretaker each for `activation/`
    // and `packages/`, built in `user_timetable_keeper`'s own region so reclaiming it takes them too,
    // on the durable window's channel. First, before anything else is held, because building them
    // is where this function's use of the capability table peaks (`login_contract::durable`).
    let Some((activation, packages)) = store_caretakers(own_ut, keeper, images.care) else {
        discard(keeper);
        return None;
    };
    let fail = |held: &[u64]| {
        for &c in held {
            cap_delete(c);
        }
        discard(keeper);
    };
    let Ok(its_budget) = memory_region_split(budget, USER_TIMETABLE_KEEPER_BUDGET_PAGES) else {
        fail(&[activation, packages]);
        return None;
    };
    let (Ok(page), Ok(ready)) = (
        retype_page_frame_from(keeper),
        retype_obj(keeper, abi::objtype::RENDEZVOUS),
    ) else {
        discard(its_budget);
        fail(&[activation, packages]);
        return None;
    };
    let built = build_child(
        own_ut,
        keeper,
        &elf,
        &ChildEndowment {
            caps: &[
                (ready, abi::rights::WRITE),
                // `GRANT` too: a region split off this one inherits its rights, and the session
                // process hands its timetable a split of it, which `CAP_INSERT` refuses without
                // `GRANT`.
                (its_budget, abi::rights::ALL),
                (page, abi::rights::WRITE),
                // `GRANT` on the two endpoints `user_timetable_keeper` only hands on; the page it
                // only maps into its timetable, which `WRITE` alone allows.
                (activation, abi::rights::WRITE | abi::rights::GRANT),
                (packages, abi::rights::WRITE | abi::rights::GRANT),
                (DURABLE_WINDOW, abi::rights::WRITE),
            ],
            blobs: &[(
                login_contract::user_timetable_keeper::TIMETABLE_VA,
                timetable,
            )],
            stack_pages: USER_TIMETABLE_KEEPER_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    );
    cap_delete(activation);
    cap_delete(packages);
    let started = match built {
        Ok(child) => start_child(child, timetable.len() as u64, 0, 0),
        Err(()) => false,
    };
    let answer = if started { receive(ready).0 } else { 0 };
    cap_delete(its_budget);
    if answer != login_contract::user_timetable_keeper::READY
        || !map_page_frame(page, va, true, own_ut)
    {
        cap_delete(ready);
        // `user_timetable_keeper` has stopped (it reports a failure and exits) or never ran. What it
        // split off its budget before failing, if anything, stays until the user's budget is
        // reclaimed: see this program's BUGS. Discarding `keeper` also takes the two caretakers.
        cap_delete(page);
        discard(keeper);
        return None;
    }
    Some((keeper, page, ready))
}

/// **The two read-only store caretakers a durable session's timetable reads through** (milestone
/// 152, Fork 8 ruled D by calef on 2026-09-27, on #1377): `activation/` and `packages/`, each an
/// `fs_subtree_caretaker` holding the file service's endpoint badged with
/// [`login_contract::DURABLE_WINDOW`] and staging through [`DURABLE_WINDOW`]'s page, never window
/// 0, which this process and every signed-in user's caretaker share. Built in `region` from
/// `own_ut`'s scratch, the way [`mint`] builds one. The two narrowed endpoints, or `None` with
/// nothing of this process's own left behind (whatever started in `region` goes with it).
fn store_caretakers(own_ut: u64, region: u64, care: &elf::Elf) -> Option<(u64, u64)> {
    let badged = u64::try_from(user_mode_runtime::badge(
        FS_EP,
        login_contract::DURABLE_WINDOW,
    ))
    .ok()?;
    let activation = store_caretaker(own_ut, region, care, badged, installed_programs::DIRECTORY);
    let packages = activation
        .and_then(|_| store_caretaker(own_ut, region, care, badged, installed_programs::PACKAGES));
    cap_delete(badged);
    match (activation, packages) {
        (Some(a), Some(p)) => Some((a, p)),
        (Some(a), None) => {
            cap_delete(a);
            None
        }
        _ => None,
    }
}

/// One of [`store_caretakers`]: `name` at the store's root, `READ | DESCEND` and nothing more.
fn store_caretaker(
    own_ut: u64,
    region: u64,
    care: &elf::Elf,
    badged: u64,
    name: &str,
) -> Option<u64> {
    let name = name.as_bytes();
    let narrow_ep = retype_obj(region, abi::objtype::RENDEZVOUS).ok()?;
    let Ok(ready) = retype_obj(region, abi::objtype::RENDEZVOUS) else {
        cap_delete(narrow_ep);
        return None;
    };
    let (lo, hi) = filesystem_contract::grant::pack_name(name);
    let spec = filesystem_contract::grant::spec(
        name.len(),
        filesystem_contract::dir::READ | filesystem_contract::dir::DESCEND,
    );
    let started = build_child(
        own_ut,
        region,
        care,
        &ChildEndowment {
            caps: &[
                (badged, abi::rights::WRITE),
                (narrow_ep, abi::rights::READ),
                (ready, abi::rights::WRITE),
            ],
            maps: &[(CARETAKER_FS_VA, DURABLE_WINDOW, abi::address_space::MAP_RW)],
            stack_pages: CARETAKER_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    )
    .is_ok_and(|child| start_child(child, lo, hi, spec));
    let verdict = if started { receive(ready).0 } else { 0 };
    cap_delete(ready);
    if verdict != filesystem_contract::READY {
        cap_delete(narrow_ep);
        return None;
    }
    Some(narrow_ep)
}

/// One connecting client's private channel, as this process holds it: its own copies of the
/// request/result endpoints [`connect`] minted (narrowed copies went to the client), where the
/// staging page landed in this address space, and the region all of it was retyped from, never
/// delegated, which `FrontDoor::serve_connect` discards once [`serve_login`] returns.
struct Channel {
    /// `RECEIVE`, this process's own copy (the client's is `WRITE`).
    request: u64,
    /// `WRITE | GRANT`, this process's own copy (the client's is `READ`).
    result: u64,
    /// `READ | WRITE`, this process's own copy (the client's is also `READ | WRITE`: both ends
    /// stage into and read the same frame, the way `FS_PAGE_FRAME` already works for the caretaker
    /// hop).
    page: u64,
    /// Where `page` is mapped in this process's own address space.
    va: u64,
    /// The `MemoryRegion` `request`, `result` and `page` were all retyped from, and the page tables
    /// for `page`'s own mapping were drawn from too. This process's own, never delegated.
    region: u64,
}

/// **Mint one connecting client's own private channel**: a fresh region split from `channel_ut`
/// ([`CHANNEL_UT_PAGES`]), and a request/result rendezvous pair and a staging page retyped from it,
/// so the whole channel goes away in one `DESTROY`. `connect_seq` picks a scratch VA this process
/// has never mapped (`page_frame::MAP` refuses an already-mapped one). `None` on any failure, which
/// the caller answers with [`login_contract::DENIED`]. Why the channel has its own region and its
/// own budget is in notes/login/budgets-and-reclamation.md.
fn connect(channel_ut: u64, connect_seq: u64) -> Option<Channel> {
    let region = memory_region_split(channel_ut, CHANNEL_REGION_PAGES).ok()?;
    // Every abandoned step below gives back its capability slots as well as its memory
    // ([`discard`], and `cap_delete` for what was retyped before the step that failed).
    let Ok(request) = retype_obj(region, abi::objtype::RENDEZVOUS) else {
        discard(region);
        return None;
    };
    let Ok(result) = retype_obj(region, abi::objtype::RENDEZVOUS) else {
        cap_delete(request);
        discard(region);
        return None;
    };
    let Ok(page) = retype_page_frame_from(region) else {
        cap_delete(result);
        cap_delete(request);
        discard(region);
        return None;
    };
    let va = CONNECT_VA_BASE + connect_seq * login_contract::PAGE as u64;
    // Page tables for this new mapping come from `region` itself: the channel's whole cost, objects
    // and page tables alike, lives in one place and comes home in one `DESTROY`.
    if !map_page_frame(page, va, true, region) {
        // Nothing in the region has a live thread, so `discard` is expected to succeed first time.
        cap_delete(page);
        cap_delete(result);
        cap_delete(request);
        discard(region);
        return None;
    }
    // SAFETY: `va` was just mapped, read/write, by this process and by no one else yet (the frame
    // has not been delegated to a client at the point this runs).
    unsafe {
        core::ptr::write_bytes(va as *mut u8, 0, login_contract::PAGE);
    }
    Some(Channel {
        request,
        result,
        page,
        va,
        region,
    })
}

/// **Mint one principal's capability set**: a fresh `fs_subtree_caretaker` attenuated to
/// `identity`'s own home subtree (DECISIONS §117 (a principal's subtree is named by its identity
/// string), used directly) and the construction region itself, the caretaker's logout ticket
/// (notes/login.md, "Reclaiming a session: the design argument"), both held with full rights for
/// [`delegate`] to narrow. `None` on any failure, answered [`login_contract::DENIED`]: a spent
/// budget, an identity too long to grant, no provisioned subtree, or `care` being `None` because
/// `_start`'s measurement refused the caretaker. `identity_provisioner` creates the subtree; this
/// process never does (§117).
fn mint(own_ut: u64, care: Option<&elf::Elf>, identity: &[u8]) -> Option<(u64, u64)> {
    let care = care?;

    // The grant name travels in two `START` argument words, so it is capped at `grant::MAX_NAME`
    // (16 bytes), below `login_contract::MAX_IDENTITY` (64). `pack_name` silently stops copying at
    // the 16th byte, which would attenuate to a truncated name two identities could share; refuse
    // before anything is built.
    if !filesystem_contract::grant::fits(identity) {
        return None;
    }

    let region = memory_region_split(CONSTRUCTION_UT, CARETAKER_REGION_PAGES).ok()?;
    // As in [`connect`]: an abandoned step gives back its capability slots as well as its memory
    // ([`discard`]).
    let Ok(narrow_ep) = retype_obj(region, abi::objtype::RENDEZVOUS) else {
        discard(region);
        return None;
    };
    let Ok(ready) = retype_obj(region, abi::objtype::RENDEZVOUS) else {
        cap_delete(narrow_ep);
        discard(region);
        return None;
    };

    let (lo, hi) = filesystem_contract::grant::pack_name(identity);
    let spec = filesystem_contract::grant::spec(identity.len(), filesystem_contract::dir::ALL);

    // Its whole authority: the file service to attenuate, the endpoint it will serve, one place to
    // say it is ready, and the frame it shares with the file service. No untyped of its own, no
    // clock, nothing that could name another process. See `crates/system_initializer::build_caretaker`,
    // which this mirrors.
    let built = build_child(
        own_ut,
        region,
        care,
        &ChildEndowment {
            caps: &[
                (FS_EP, abi::rights::WRITE),
                (narrow_ep, abi::rights::READ),
                (ready, abi::rights::WRITE),
            ],
            maps: &[(CARETAKER_FS_VA, FS_PAGE_FRAME, abi::address_space::MAP_RW)],
            stack_pages: CARETAKER_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    );
    let Ok(child) = built else {
        cap_delete(ready);
        cap_delete(narrow_ep);
        // `build_child` gives back its own capability slots on failure (milestone 757 (a test
        // kernel fails a process on its Nth retype), provisional, whose sweep found that it did
        // not); what it built lives in `region`, which goes next.
        discard(region);
        return None;
    };
    let started = start_child(child, lo, hi, spec);
    if !started {
        cap_delete(ready);
        cap_delete(narrow_ep);
        // A caretaker that failed to start left nothing running in the region, so `DESTROY` has
        // nothing to wait on.
        discard(region);
        return None;
    }
    // The one bounded wait: the caretaker's descent against the file service, exactly the
    // handshake `crates/system_initializer::build_caretaker` performs. An identity with no
    // provisioned subtree is answered here with no special case: the caretaker's `OPENDIR` comes
    // back `ENOENT`, it reports [`filesystem_contract::DESCENT_REFUSED`], and the caller folds that
    // into `DENIED`, like a wrong password (notes/login.md says why that fold is meant).
    let (verdict, _, _) = receive(ready);
    cap_delete(ready);
    if verdict != filesystem_contract::READY {
        cap_delete(narrow_ep);
        // The caretaker's `OPENDIR` was refused, so it has already exited and nothing runs in
        // `region`: case (a) or a plain corpse in notes/hung-component.md's taxonomy, never (c), so
        // `DESTROY` cannot be refused for good. [`discard`], so the region's slot goes too.
        discard(region);
        return None;
    }

    // **`region` is not dropped here**: the caller delegates it to the client as the logout ticket
    // and only then deletes its own copy.
    Some((narrow_ep, region))
}

/// **Reclaim a construction region, retrying while something in it can still run**
/// (`crates/system_initializer::reclaim`'s idiom): a `DESTROY` of a region holding a live thread is
/// refused with §16's kill armed, and preemptions later the retry succeeds. The bound is
/// [`RECLAIM_ATTEMPTS`], not `system_initializer`'s 64, for [`Durable::retire`]'s store caretakers;
/// notes/login/budgets-and-reclamation.md, "The bounded retry", has the measurement.
fn reclaim(region: u64) {
    for _ in 0..RECLAIM_ATTEMPTS {
        if memory_region_destroy(region) {
            return;
        }
        yield_now();
    }
}

/// How many times [`reclaim`] retries. Not `system_initializer`'s 64; see [`reclaim`] for the
/// durable-retirement measurement that separates them.
const RECLAIM_ATTEMPTS: usize = 1024;

/// **Give a region back completely: the memory *and* this process's own capability-table slot.**
/// `MemoryRegion::DESTROY` ([`reclaim`]) leaves the `MemoryRegion` capability that named the region
/// in the caller's table, stale, so a long-lived server destroying a region per request runs out
/// of slots while its budget looks healthy. Every site here that stops wanting a region calls this.
fn discard(region: u64) {
    reclaim(region);
    cap_delete(region);
}

/// **Is `identity` on the owner's run-unvouched list?** (DECISIONS §221 (the boot prompt is the
/// owner's console), ruling 2.) Opens [`login_contract::RUN_UNVOUCHED_LIST`] at the root of the
/// file service and asks [`login_contract::lists`]. Every way of not reading it (no mapped page,
/// no file, a file larger than a page, a refused read) lists nobody.
///
/// Read on every login rather than once at start, so the owner's edit at the boot prompt holds
/// from the next login without a reboot. The page is the one every session and caretaker shares
/// with the server; this is sound for the reason those clients' use of it is, that only one session
/// is live at a time (the terminal rule, checked before this runs), and that session is still
/// being built.
fn listed(identity: &[u8]) -> bool {
    on_list(login_contract::RUN_UNVOUCHED_LIST, identity)
}

/// **Whether `identity` is suspended** (milestone 152, calef's §108 ruling of 2026-09-26): on the
/// owner's [`login_contract::SUSPENDED_LIST`]. The one place this program reads it. An absent or
/// unreadable list suspends nobody; `login_contract::SUSPENDED_LIST` says why that direction.
fn is_suspended(identity: &[u8]) -> bool {
    on_list(login_contract::SUSPENDED_LIST, identity)
}

/// **Whether the owner's list `file`, at the file service's root, names `identity`**, read fresh
/// on every call so an edit at the console holds without a restart. Every way of failing to read it
/// answers `false`.
fn on_list(file: &str, identity: &[u8]) -> bool {
    let mut list = [0u8; login_contract::PAGE];
    // A short read lists by what arrived, which can only be fewer names.
    read_file(filesystem_contract::fs::ROOT, file.as_bytes(), &mut list)
        .is_some_and(|n| login_contract::lists(&list[..n], identity))
}

/// **Read `name` under directory handle `at` into `out`**, through the file page at [`LIST_VA`]:
/// the byte count, or `None` for no mapped page, no such file, or one larger than a page. The one
/// way this program reads a file, for the owner's lists, the durable-session manifest and an
/// identity's stored schedule alike.
fn read_file(at: u64, name: &[u8], out: &mut [u8]) -> Option<usize> {
    use filesystem_contract::fs;
    let page = file_page()?;
    page[..name.len()].copy_from_slice(name);
    let opened = call(FS_EP, fs::req(fs::OPEN, at, name.len() as u64), 0).0 as i64;
    if opened < 0 {
        return None;
    }
    let handle = opened as u64;
    let size = call(FS_EP, fs::req(fs::FSTAT, handle, 0), 0).0 as i64;
    let read = if (0..=out.len().min(login_contract::PAGE) as i64).contains(&size) {
        call(FS_EP, fs::req(fs::READ, handle, size as u64), 0).0 as i64
    } else {
        -1
    };
    call(FS_EP, fs::req(fs::CLOSE, handle, 0), 0);
    let n = usize::try_from(read).ok()?.min(out.len());
    out[..n].copy_from_slice(&page[..n]);
    Some(n)
}

/// **Replace `name` at the file service's root with `bytes`**, creating it if absent. `true` when
/// every byte landed.
fn write_root_file(name: &[u8], bytes: &[u8]) -> bool {
    use filesystem_contract::fs;
    let Some(page) = file_page() else {
        return false;
    };
    page[..name.len()].copy_from_slice(name);
    let mut h = call(FS_EP, fs::req(fs::CREATE, fs::ROOT, name.len() as u64), 0).0 as i64;
    if h < 0 {
        page[..name.len()].copy_from_slice(name);
        h = call(FS_EP, fs::req(fs::OPEN, fs::ROOT, name.len() as u64), 0).0 as i64;
        if h < 0 || (call(FS_EP, fs::req(fs::TRUNCATE, h as u64, 0), 0).0 as i64) < 0 {
            return false;
        }
    }
    let h = h as u64;
    page[..bytes.len()].copy_from_slice(bytes);
    let wrote = call(FS_EP, fs::req(fs::WRITE, h, bytes.len() as u64), 0).0 as i64;
    call(FS_EP, fs::req(fs::CLOSE, h, 0), 0);
    wrote == bytes.len() as i64
}

/// **Read `identity`'s stored schedule** (`<root>/<identity>/schedule`, DECISIONS §122 (the on-disk
/// schedule store)) into `out`: the byte count, or `None` when there is none. Descends by name, as
/// [`rederive`] and the file service's other clients do, and never enumerates.
fn read_stored_schedule(identity: &[u8], out: &mut [u8]) -> Option<usize> {
    use filesystem_contract::{dir, fs};
    let page = file_page()?;
    page[..identity.len()].copy_from_slice(identity);
    let d = call(
        FS_EP,
        fs::req(fs::OPENDIR, fs::ROOT, identity.len() as u64),
        dir::READ | dir::DESCEND,
    )
    .0 as i64;
    if d < 0 {
        return None;
    }
    let n = read_file(
        d as u64,
        timetable_store::SCHEDULE_FILE_NAME.as_bytes(),
        out,
    );
    call(FS_EP, fs::req(fs::CLOSE, d as u64, 0), 0);
    n
}

/// **Add `identity` to, or take it off, the durable-session manifest** (DECISIONS §125 (which identities have
/// pending work)), which is what [`rederive`] reads at the next start-up to learn whose schedules to
/// bring back. A failure is not fatal to a login, and costs only that start-up pass.
fn record_in_manifest(identity: &[u8], present: bool) {
    let name = timetable_store::MANIFEST_FILE_NAME.as_bytes();
    let mut now = [0u8; login_contract::PAGE];
    let had = read_file(filesystem_contract::fs::ROOT, name, &mut now).unwrap_or(0);
    let mut next = [0u8; login_contract::PAGE];
    let n = if present {
        login_contract::with_listed(&now[..had], identity, &mut next)
    } else {
        login_contract::without_listed(&now[..had], identity, &mut next)
    };
    if let Some(n) = n
        && next[..n] != now[..had]
    {
        write_root_file(name, &next[..n]);
    }
}

/// The file page at [`LIST_VA`], when `_start` managed to map it.
fn file_page() -> Option<&'static mut [u8]> {
    if !LIST_MAPPED.load(core::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    // SAFETY: `_start` mapped the file page read/write at `LIST_VA`, one page, for this process's
    // life. One thread per address space, and every caller is done with the slice before the next
    // takes it.
    Some(unsafe { core::slice::from_raw_parts_mut(LIST_VA as *mut u8, login_contract::PAGE) })
}

/// Delegate our own copy of `slot`, narrowed to `rights`, over `ep`. `GRANT` must already be on our
/// own copy for the kernel to allow this at all (`abi::rendezvous::SEND_CAP`'s contract); every
/// capability this process delegates was retyped or split by this process, so it always is. `ep` is
/// [`RESULT`] for a [`login_contract::CONNECTED`] answer and a channel's own private `result` for a
/// [`login_contract::OK`] one; both are this process's own copy, always held with `GRANT`.
fn delegate(ep: u64, slot: u64, rights: u64) {
    send_capability(ep, slot, rights, 0);
}

/// Zero the identity/secret staged at `va`, on every path: a malformed request, a denial, and a
/// success all leave a presented secret sitting in a page two processes share until this runs.
fn wipe_page(va: u64) {
    // SAFETY: `connect` mapped one page read/write here, and this process is the only writer
    // between a request arriving on the channel it belongs to and that channel's reply going out.
    let page = unsafe { core::slice::from_raw_parts_mut(va as *mut u8, login_contract::PAGE) };
    login_contract::wipe(page);
}

fn fail(step: u64) -> ! {
    send(AUDIT, 0xDEAD_0000_0000_0000 | step, 0, 0);
    supervision_protocol::fail()
}

user_mode_runtime::panic_handler!();
