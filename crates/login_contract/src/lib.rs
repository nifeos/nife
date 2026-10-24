#![no_std]
//! **The wire contract between a client and the login service**: on success it
//! delegates capabilities back rather than changing anything ambient. See notes/login.md.
//!
//! Not a `CALL`: a reply carries two words, and only `abi::rendezvous::SEND_CAP` hands back
//! capabilities. `REQUEST`/`RESULT` are a front door shared by every client, carrying only a bare
//! [`CONNECT`] (answered with a private [`CONNECTED`] channel), [`LOGOUT`] and the other bare
//! words. Neither carries a page or a secret, so the identity and secret travel only on the
//! private channel. [`OK`]'s doc gives what follows it; on [`DENIED`], [`MALFORMED`] or
//! [`NO_TERMINAL`] nothing does. The request page is [`credential_contract`]'s, reused because
//! login relays it unchanged.

//! ```text
//!   client --send(REQUEST, connect_word(), 0, 0)-------------------------> login
//!   client <--- receive(RESULT) -> CONNECTED, then RECEIVE_CAP x 3: ------- login
//!                priv_request, priv_result, page
//!   client --place(), send(priv_request, w0, 0, 0)-----------------------> login
//!   client <--- receive(priv_result) -> OK, DENIED or NO_TERMINAL; -------- login
//!                RECEIVE_CAP(priv_result) x 5, only after OK
//!   client --send(REQUEST, logout_word(), 0, 0)--------------------------> login
//!   client <--- receive(RESULT) -> LOGGED_OUT ----------------------------- login
//! ```

//! # BUGS
//!
//! - [`RUN_UNVOUCHED_LIST`] (§221 ruling 2) is read through the file service's one shared page,
//!   sound only while one session is live; delisting holds from the next login, since nothing
//!   revokes a delegated capability.
//! - [`LOGOUT`] authenticates nothing: any holder of `REQUEST` can free the terminal from under
//!   its user. Accepted for one boot, one terminal and no untrusted co-tenant; a hostile
//!   deployment needs `LOGOUT` to carry a capability only the session holds.
//! - A terminal holder that exits without logging out wedges it until `login` restarts.
//!
//! Name: ratified 2026-08-23 (calef); see `design/naming/vocabulary-rulings.md`.

pub use credential_contract::{MAX_IDENTITY, MAX_SECRET, PAGE, operation, place, read, wipe};

/// **The front door's only legal request** (milestone 49's channel-per-client update): "give me my
/// own private channel." Carries no lengths and touches no page; build the word with
/// [`connect_word`] rather than [`place`] (there is nothing to stage).
pub const CONNECT: u64 = 2;

/// The one opcode a private, per-client channel accepts. There is only one verb (authenticate), so
/// unlike `credential_contract` (verify, provision) there is nothing to distinguish it from; it exists so a
/// reader of a captured request word can tell this contract's traffic from any other sharing the
/// encoding. Build a request word with `place(page, identity, secret, LOGIN)`, which returns it
/// already carrying this opcode and the two lengths. Sent on the private `priv_request` endpoint
/// [`CONNECTED`] delegates, never on the front door.
pub const LOGIN: u64 = 1;

/// **Free the terminal** (milestone 49's terminal update): "I am done; the next login may have it."
/// A bare word on the shared front door, like [`CONNECT`], and for the same reason [`CONNECT`]
/// carries no page: there is nothing here a caller did not already know. Build it with
/// [`logout_word`]. See this contract's own BUGS for what `LOGOUT` does not authenticate.
pub const LOGOUT: u64 = 3;
/// **Authenticate, and open this identity's schedule** (milestone 152 (durable delegation),
/// §222 (who holds a user's schedule), calef's L2 ruling of 2026-09-26). Sent on the private
/// channel in place of [`LOGIN`], staged with [`place`] as [`LOGIN`] is. On success `login` builds
/// the `user_timetable_keeper` holding the identity's timetable, and [`OK`] carries
/// [`SCHEDULE_FOLLOWS`]; a [`LOGIN`] that reattaches a durable session carries it too. The ruling
/// said "a new request word after `OK`"; it is the request itself because `login` blocks on one
/// endpoint at a time. Name and value: provisional.
pub const SCHEDULE: u64 = 7;
/// **The owner's suspended list changed: apply it now** (milestone 152 (durable delegation),
/// calef's §108 (disabling credentials kills the durable session) ruling of 2026-09-26). A bare word on the front door, like [`LOGOUT`], built with
/// [`suspend_word`]. `login` rereads [`SUSPENDED_LIST`] and ends every durable session it keeps for
/// an identity on it, then answers [`APPLIED`] with how many it ended. Anyone holding the front
/// door can send it, which is harmless for the reason [`LOGOUT`] is: it acts only on a list a
/// session cannot write. Name and value: provisional, a wire item.
pub const SUSPEND: u64 = 8;

/// **Report why the start-up pass skipped what it skipped** (milestone 152): a skip that leaves no
/// reason anywhere is the defect, not the skip. A bare word on the front door, like [`SUSPEND`],
/// built with [`rederive_skips_word`]. Answers [`SKIP_COUNTS`] with [`durable::pack_skip_counts`]'s
/// word on `RESULT`'s second word; the third is always 0, reserved. Unauthenticated, like
/// [`SUSPEND`] and [`LOGOUT`]: it names no identity, only counts. The counters cost a saturating
/// add each and are kept in production too. Name and value: provisional.
pub const REDERIVE_SKIPS: u64 = 9;

/// `send(REQUEST, connect_word(), 0, 0)`. The bare word [`CONNECT`] travels as; a client never calls
/// [`place`] for this step, because there is no identity or secret to stage.
pub fn connect_word() -> u64 {
    CONNECT << credential_contract::OPERATION_SHIFT
}

/// `send(REQUEST, suspend_word(), 0, 0)`. The bare word [`SUSPEND`] travels as.
pub fn suspend_word() -> u64 {
    SUSPEND << credential_contract::OPERATION_SHIFT
}

/// `send(REQUEST, rederive_skips_word(), 0, 0)`. The bare word [`REDERIVE_SKIPS`] travels as.
pub fn rederive_skips_word() -> u64 {
    REDERIVE_SKIPS << credential_contract::OPERATION_SHIFT
}

/// `send(REQUEST, logout_word(), 0, 0)`. The bare word [`LOGOUT`] travels as, on the *front door*
/// (unlike [`connect_word`]'s answer, this needs no private channel): see [`LOGOUT`]'s own doc.
pub fn logout_word() -> u64 {
    LOGOUT << credential_contract::OPERATION_SHIFT
}

/// **A private channel is ready.** Answered on the front door's `RESULT` endpoint, followed by
/// exactly three delegated capabilities, in this order: the private `priv_request` endpoint
/// (`WRITE`), the private `priv_result` endpoint (`READ`), and a page frame (`READ | WRITE`) to stage
/// the actual login on. See the module docs for the two-phase exchange.
pub const CONNECTED: u64 = 4;

/// **Authenticated.** Five capabilities follow on the private result endpoint, or six when the
/// reply's second word carries [`RUN_UNVOUCHED_FOLLOWS`], in this order:
///
/// 1. the directory: a freshly built `fs_subtree_caretaker`'s endpoint, `WRITE`;
/// 2. the file service's shared page, `READ | WRITE`, mapped where the client chooses, for both
///    hops, which is sound because every request on both is a blocking `CALL`;
/// 3. the budget: a freshly split `MemoryRegion`, `WRITE | GRANT`, the client's own;
/// 4. the logout ticket: the caretaker's construction region, `WRITE`, whose one use is
///    `abi::memory_region::DESTROY`; retry a refused `DESTROY` a bounded few times, since the
///    caretaker can be mid-request to the file service but never blocks permanently;
/// 5. the terminal, `WRITE`; always present, since login answers [`NO_TERMINAL`] before
///    authenticating while another session holds it;
/// 6. only with [`RUN_UNVOUCHED_FOLLOWS`], for identities on [`RUN_UNVOUCHED_LIST`]:
///    run-unvouched, `WRITE` without `GRANT` (§219 gate D2), so it cannot be passed on.
///
/// A full logout destroys 3 before 4, because LIFO reclaim strands 4's pages otherwise
/// (notes/login/contract.md). Logging out is optional.
pub const OK: u64 = 1;

/// **A bit of [`OK`]'s second word: the sixth capability, the run-unvouched one, follows**
/// (DECISIONS §219 (how the shell names an installed program to the spawner) gate D2). Carried on
/// the reply because a client's sixth `RECEIVE_CAP` must match a sixth `SEND_CAP` exactly: a
/// `login` spawned without the capability sends five, and a client waiting for six would block
/// forever. Name: provisional.
pub const RUN_UNVOUCHED_FOLLOWS: u64 = 1;
/// **A bit of [`OK`]'s second word: the registration page follows** (milestone 152). After the
/// run-unvouched capability when that is announced too, one more `RECEIVE_CAP` delivers a page frame
/// (`WRITE`): the identity's timetable's registration page, `timetable::registration`'s whole
/// protocol. Announced for the reason [`RUN_UNVOUCHED_FOLLOWS`] is. Name: provisional.
pub const SCHEDULE_FOLLOWS: u64 = 2;

/// **The owner's list of identities whose sessions may run unvouched bytes** (DECISIONS §221 (the
/// boot prompt is the owner's console), ruling 2): a file at the root of the file service, beside
/// the identities' own subtrees (DECISIONS §117 (a principal's subtree is named by its identity
/// string)). A session cannot name it; the boot prompt can (`echo alice >> may-run-unvouched`).
/// **Empty by default**: no file, an unreadable file, or one larger than a page lists nobody, so
/// the failure is always toward the narrower grant. The format is [`lists`]'s.
///
/// Name: provisional (milestone 198 (a package manager), lane `milestone/198-owner-console`,
/// 2026-09-26).
pub const RUN_UNVOUCHED_LIST: &str = "may-run-unvouched";

/// **Whether `list` names `identity`** ([`RUN_UNVOUCHED_LIST`]'s format): one identity per line,
/// surrounding spaces, tabs and a carriage return ignored, blank lines and lines starting with `#`
/// skipped, and the match exact. `alice` does not list `ali` or `alice2`, and an empty identity
/// is never listed.
///
/// # EXAMPLES
///
/// ```
/// let list = b"# who may run new native code\nalice\n  bob \r\n";
/// assert!(login_contract::lists(list, b"alice"));
/// assert!(login_contract::lists(list, b"bob"));
/// assert!(!login_contract::lists(list, b"ali"));
/// assert!(!login_contract::lists(b"", b"alice"));
/// ```
pub fn lists(list: &[u8], identity: &[u8]) -> bool {
    !identity.is_empty()
        && list
            .split(|&b| b == b'\n')
            .map(|line| line.trim_ascii())
            .filter(|line| !line.is_empty() && !line.starts_with(b"#"))
            .any(|line| line == identity)
}

/// **`list` with `identity` added**, into `out`: `list` unchanged when it already names it, else
/// `list`, a newline if it did not end in one, and `identity` on a line of its own. The byte count,
/// or `None` when `out` is too small or `identity` is not a name a list can hold (empty, or with
/// whitespace, or starting with `#`). What `user suspend <name>` writes (milestone 152 (durable
/// delegation)).
///
/// # EXAMPLES
///
/// ```
/// let mut out = [0u8; 64];
/// let n = login_contract::with_listed(b"# suspended\nalice", b"bob", &mut out).unwrap();
/// assert_eq!(&out[..n], b"# suspended\nalice\nbob\n");
/// let n = login_contract::with_listed(b"alice\n", b"alice", &mut out).unwrap();
/// assert_eq!(&out[..n], b"alice\n");
/// assert!(login_contract::with_listed(b"", b"two words", &mut out).is_none());
/// ```
pub fn with_listed(list: &[u8], identity: &[u8], out: &mut [u8]) -> Option<usize> {
    if !listable(identity) {
        return None;
    }
    if lists(list, identity) {
        out.get_mut(..list.len())?.copy_from_slice(list);
        return Some(list.len());
    }
    let sep = usize::from(!list.is_empty() && !list.ends_with(b"\n"));
    let n = list.len() + sep + identity.len() + 1;
    let dst = out.get_mut(..n)?;
    dst[..list.len()].copy_from_slice(list);
    if sep == 1 {
        dst[list.len()] = b'\n';
    }
    dst[list.len() + sep..n - 1].copy_from_slice(identity);
    dst[n - 1] = b'\n';
    Some(n)
}

/// **`list` with every line naming `identity` removed**, into `out`, other lines and comments kept
/// as they were. What `user resume <name>` writes (milestone 152). `None` when `out` is too small.
///
/// # EXAMPLES
///
/// ```
/// let mut out = [0u8; 64];
/// let n = login_contract::without_listed(b"# suspended\nalice\n bob\n", b"bob", &mut out).unwrap();
/// assert_eq!(&out[..n], b"# suspended\nalice\n");
/// ```
pub fn without_listed(list: &[u8], identity: &[u8], out: &mut [u8]) -> Option<usize> {
    let mut n = 0;
    for line in list.split_inclusive(|&b| b == b'\n') {
        let body = line.strip_suffix(b"\n").unwrap_or(line).trim_ascii();
        if !identity.is_empty() && body == identity {
            continue;
        }
        out.get_mut(n..n + line.len())?.copy_from_slice(line);
        n += line.len();
    }
    Some(n)
}

/// Whether `identity` can be a line of a list: non-empty, no whitespace, not a comment.
fn listable(identity: &[u8]) -> bool {
    !identity.is_empty()
        && !identity.starts_with(b"#")
        && !identity.iter().any(u8::is_ascii_whitespace)
}

/// **Refused.** The identity is unknown, the secret is wrong, the service could not mint a
/// capability set for an otherwise-authenticated principal, or (on the front door) the service could
/// not mint a private channel at all (see this service's BUGS on the second and third cases: both
/// are folded into the same code on purpose, for [`credential_contract`]'s reason: a caller must not be able
/// to distinguish "wrong password" from "the service is out of memory" by trying the same identity
/// twice and comparing outcomes).
pub const DENIED: u64 = 2;

/// The request word's lengths are out of range, or the front door was sent something other than
/// [`CONNECT`] or [`LOGOUT`]. Not an authentication outcome, and a client that gets this has learned
/// nothing about whether the identity exists.
pub const MALFORMED: u64 = 3;

/// **The terminal is already held by another session** (milestone 49's terminal update, the
/// "deny cleanly" shape). Sent on the private `priv_result` endpoint, *before* the presented
/// identity and secret are even relayed to the credential service: this is global, caller-
/// independent state (there is exactly one physical terminal), not a fact about any identity, so
/// checking it first costs nothing an attacker could turn into a timing oracle and saves a round
/// trip to the credential service on every refusal. A client that gets this has learned nothing
/// about whether its identity or secret were correct.
pub const NO_TERMINAL: u64 = 5;

/// **The terminal is free again.** Sent on the front door's `RESULT` endpoint in answer to
/// [`LOGOUT`]. Idempotent: sent whether or not anything was actually held, since a logout that
/// arrives when nobody holds the terminal is harmless rather than an error.
pub const LOGGED_OUT: u64 = 6;

/// **Authenticated, and suspended** (milestone 152, the §108 ruling of 2026-09-26). The identity
/// and secret were right and the identity is on [`SUSPENDED_LIST`]. Sent only after a successful
/// authentication, so only someone who already holds the secret learns the account is suspended,
/// and learns it plainly rather than as a wrong password. Nothing follows. Name and value:
/// provisional, a wire item.
pub const SUSPENDED: u64 = 7;

/// **[`SUSPEND`]'s answer**, on the front door's `RESULT`; the second word is how many durable
/// sessions were ended. Name and value: provisional.
pub const APPLIED: u64 = 8;

/// **[`REDERIVE_SKIPS`]'s answer**, on the front door's `RESULT`; the second word is
/// [`durable::pack_skip_counts`]'s word. Name and value: provisional.
pub const SKIP_COUNTS: u64 = 9;

/// **The owner's list of suspended identities** (milestone 152, calef's §108 ruling of
/// 2026-09-26), a file at the root of the file service in [`RUN_UNVOUCHED_LIST`]'s place and
/// format, read with [`lists`]. `user suspend <name>` at the owner's console adds a name and
/// `user resume <name>` removes it. `login` refuses a listed identity with [`SUSPENDED`] and ends
/// its durable session; the boot-time re-deriver skips it.
///
/// **Empty by default, and the failure direction is the opposite of the run-unvouched list's**, which
/// is worth saying because it is not a free choice: no file, or one that cannot be read, suspends
/// nobody. Failing toward "suspended" would lock every user out when the file service is slow.
///
/// Name: provisional (milestone 152, 2026-09-26).
pub const SUSPENDED_LIST: &str = "suspended";

/// **One attribution record**, sent once per successful login on the service's own audit endpoint
/// (`components/src/login.rs`'s `AUDIT` slot), so the property DECISIONS §109 names ("a server ... logs
/// which channel a request arrived on") is checkable rather than merely claimed. `w0` is
/// [`ATTRIBUTED`], `w1` is the channel's sequence number (the order this service established
/// channels in, starting at 0), `w2` is [`identity_hint`] of the identity that established it.
///
/// This is **login's own record of what it just established**, not a downstream server logging a
/// later request against that channel; see this service's BUGS for the natural place the second
/// half would live and why nothing in this tree needs it yet.
pub const ATTRIBUTED: u64 = 1;

/// Pack up to the first 8 bytes of `identity` into one `u64`, big-endian, zero-padded. A debugging
/// aid for the audit record, not a general identity encoding: two identities that share an 8-byte
/// prefix are indistinguishable in it, which is fine for this contract (the audit record exists to
/// let a test, or an operator, confirm *which* login produced *which* channel among the identities
/// actually in use, not to serve as a second store of who exists).
pub fn identity_hint(identity: &[u8]) -> u64 {
    let mut buf = [0u8; 8];
    let n = identity.len().min(8);
    buf[..n].copy_from_slice(&identity[..n]);
    u64::from_be_bytes(buf)
}

// ===========================================================================================
// **How the login service is started** (milestone 233 (`login` dies on every boot)): a contract
// between whoever spawns it and the program, here because three binaries agree on it (rule 7).
// notes/login/contract.md has why a client contract's crate holds it.
// ===========================================================================================

/// **Where `fs_subtree_caretaker`'s ELF bytes are mapped, read-only, before `login`'s `_start`
/// runs**, with the length in `x0`/`a0`/`rdi` (milestone 233).
///
/// `login` gets this one program's bytes, not the boot archive: the spawner can map only pages it
/// holds a `PageFrame` for, and a service that can read every boot file to answer a password holds
/// authority it never exercises. The blob costs the spawner no capability-table slots
/// (notes/login/contract.md). Zero length means no vouched-for caretaker: `login` still starts and
/// answers every login [`DENIED`].
pub const CARETAKER_ELF_VA: u64 = 0x0000_0000_0100_0000;

/// **Where `measured_boot::PROGRAM_MEASUREMENTS`' bytes are mapped, read-only, before `login`'s
/// `_start` runs**, with the length in `x1`/`a1`/`rsi` (milestone 233).
///
/// Four megabytes above [`CARETAKER_ELF_VA`] so a caretaker image would have to grow forty-fold
/// before the two could meet. Both sit well below
/// `user_mode_runtime::initial_ramdisk::INITIAL_RAMDISK_VA` and well above the per-channel scratch
/// VAs `login` bump-allocates, which is the only other thing in that address space that grows.
pub const PROGRAM_MEASUREMENTS_VA: u64 = 0x0000_0000_0140_0000;

/// **Where `user_timetable_keeper`'s image is mapped, read-only, before `login`'s `_start` runs** (milestone 152),
/// with `timetable`'s at [`TIMETABLE_ELF_VA`] and both lengths in the third argument register
/// ([`schedule_lengths`]). They are the two programs a durable session is built from, and each is
/// checked against the table at [`PROGRAM_MEASUREMENTS_VA`] before anything is built from it. No
/// program a job runs travels with them: a job runs what the live activation generation names
/// (Fork 8 ruled D by calef on 2026-09-27, on #1377). Zero lengths mean no schedule can be opened
/// on this boot: [`SCHEDULE`] is then answered as [`LOGIN`] is, without [`SCHEDULE_FOLLOWS`].
///
/// Name: provisional, milestone 152's lane, 2026-09-27; it replaces a schedule archive that also
/// carried the jobs.
pub const USER_TIMETABLE_KEEPER_ELF_VA: u64 = 0x0000_0000_0180_0000;

/// **Where `timetable`'s image is mapped**, beside [`USER_TIMETABLE_KEEPER_ELF_VA`], four megabytes above it.
/// Provisional.
pub const TIMETABLE_ELF_VA: u64 = 0x0000_0000_01c0_0000;

/// **The third start argument: `user_timetable_keeper`'s length in the low half, `timetable`'s in the high.**
/// Two lengths in one register, because the first two carry the caretaker's and the table's. An
/// image of 4 GiB or more cannot be mapped at these addresses anyway. Provisional.
///
/// ```
/// use login_contract::{schedule_lengths, split_schedule_lengths};
/// assert_eq!(split_schedule_lengths(schedule_lengths(40_960, 409_600)), (40_960, 409_600));
/// assert_eq!(split_schedule_lengths(0), (0, 0));
/// ```
pub const fn schedule_lengths(keeper: u64, timetable: u64) -> u64 {
    (keeper & 0xffff_ffff) | (timetable << 32)
}

/// [`schedule_lengths`]' inverse: `(session, timetable)`.
pub const fn split_schedule_lengths(word: u64) -> (u64, u64) {
    (word & 0xffff_ffff, word >> 32)
}

/// **`login`'s slot for its durable sessions' file-service channel**: one page of the file
/// service's window [`DURABLE_WINDOW`], `WRITE | GRANT`, placed by the spawner. A durable session's
/// timetable reads the store through it while its user may be using the store through window 0 at
/// the prompt, and two clients staging bytes in one page overwrite each other (milestone 599 (a
/// frame per filesystem client channel)). `login` badges the file service's endpoint with the
/// window's number itself. Provisional.
pub const DURABLE_WINDOW_SLOT: u64 = 8;

/// **The file service's window the spawner reserves for `login`'s durable sessions**: the last. The
/// progenitor hands jobs behind a directory grant the others, and the kernel harness claims this
/// one for `login`. One window, so one durable session at a time reads the store
/// ([`durable::sessions_held`] is bounded by it too). Provisional.
pub const DURABLE_WINDOW: u64 = filesystem_contract::fs::CLIENT_WINDOWS as u64 - 1;

/// **How `login` starts a `user_timetable_keeper`** (milestone 152, S1 of 2026-09-26), in this crate for
/// the reason the two constants above are: `components/src/login.rs` and
/// `components/src/user_timetable_keeper.rs` both read it. Every name here is provisional.
pub mod user_timetable_keeper {
    /// Slot 0: the endpoint `user_timetable_keeper` reports readiness on, once, `WRITE`.
    pub const READY_SLOT: u64 = 0;
    /// Slot 1: the region the timetable and every job it fires are built from, `WRITE | GRANT`.
    pub const BUDGET_SLOT: u64 = 1;
    /// Slot 2: the registration page, `WRITE`, which the session maps into its timetable.
    pub const PAGE_SLOT: u64 = 2;
    /// Slot 3: an endpoint to a caretaker serving the store's `activation/` read-only, `WRITE`,
    /// which the session hands its timetable (Fork 8 D).
    pub const ACTIVATION_SLOT: u64 = 3;
    /// Slot 4: likewise for `packages/`.
    pub const PACKAGES_SLOT: u64 = 4;
    /// Slot 5: the page of the durable window the two caretakers stage through, `WRITE`, which the
    /// session maps into its timetable at `timetable::contract::STORE_PAGE_VA`.
    pub const STORE_PAGE_SLOT: u64 = 5;
    /// Where `timetable`'s image is copied into `user_timetable_keeper`; its length is `a0`.
    pub const TIMETABLE_VA: u64 = 0x0000_0000_0200_0000;
    /// The readiness word: the timetable is built, started, and watching the page.
    pub const READY: u64 = 0x5e55_0000_0000_0001;
    /// The last word, on the same endpoint: the timetable is gone and everything built from the
    /// `user_timetable_keeper`'s budget is given back. `login` takes it before reclaiming the process, so
    /// the reclaim never lands mid-teardown and strands a region under the user's budget.
    pub const STOPPED: u64 = 0x5e55_0000_0000_0002;
    /// The failure word; the low byte says which step.
    pub const FAILED: u64 = 0x5e55_0000_0000_0f00;
}

/// **How many durable sessions `login` can keep, and which ones it re-derives at start-up**
/// (milestone 152 (durable delegation), Fork 7 ruled A by calef on 2026-09-27: `login` re-derives
/// every durable session that is not suspended, before it serves its front door). Pure logic, in
/// this crate so a host test reaches it; `components/src/login.rs` is the one caller.
///
/// The slot counts are from `login.rs`'s code, counted in
/// `notes/durable-delegation/boot-rederivation-in-login.md` (question 5). They are counts, not a
/// measurement: no gauge reports one process's table.
///
/// Name: provisional, minted 2026-09-27 (UTC) by milestone 152's lane, for this module and
/// everything in it.
pub mod durable {
    /// Slots `login` holds at rest: ten endowed (`REQUEST`, `RESULT`, `VERIFY`, `FS_EP`,
    /// `FS_PAGE_FRAME`, `CONSTRUCTION_UT`, `AUDIT`, `TERM_EP`, `RUN_UNVOUCHED` and
    /// [`super::DURABLE_WINDOW_SLOT`]) and the three budgets `_start` splits (`own_ut`,
    /// `durable_ut`, `channel_ut`).
    pub const SLOTS_AT_REST: u64 = 13;
    /// Slots an ordinary login adds at its peak, inside `mint`'s `build_child`: the channel's
    /// `result` and `region`, then `region`, `narrow_ep`, `ready`, and the child's address space
    /// and one frame or its thread.
    pub const SLOTS_LOGIN_PEAK: u64 = 7;
    /// Slots each kept durable session holds: the user's budget, `user_timetable_keeper`'s region, the
    /// registration page and the readiness endpoint.
    pub const SLOTS_PER_SESSION: u64 = 4;

    /// **A signed-in client's own spending budget**, in pages, split for every login and delegated
    /// to it. In this crate because `login`, the kernel harness and the progenitor all size budgets
    /// from it and [`BUDGET_PAGES`]. Provisional.
    pub const CLIENT_BUDGET_PAGES: u64 = 64;
    /// **A durable `user_timetable_keeper`'s region**: the process, its copy of `timetable`'s image, and
    /// the two store caretakers `login` builds in it (Fork 8 D), 64 pages each as `login` sizes a
    /// caretaker's region. Measured for the process on aarch64's debug build on 2026-09-26 at 192
    /// with room to spare. Provisional.
    pub const USER_TIMETABLE_KEEPER_REGION_PAGES: u64 = 192 + 2 * 64;
    /// **What a durable `user_timetable_keeper` builds its timetable from**: the timetable's region (272,
    /// `user_timetable_keeper.rs`'s `TIMETABLE_REGION_PAGES`) and its jobs' budget (128), with room for two
    /// endpoints. 400 -> 416 on 2026-10-02 (UTC), for the timetable's staging buffer growing to
    /// 128 KiB. Provisional.
    pub const USER_TIMETABLE_KEEPER_BUDGET_PAGES: u64 = 416;
    /// **One durable session's whole budget**: the client's own, `user_timetable_keeper`'s region, and
    /// what it builds from. `login` splits this many pages per durable session, and its spawner
    /// sizes `login`'s construction budget with it. Provisional.
    pub const BUDGET_PAGES: u64 = CLIENT_BUDGET_PAGES
        + USER_TIMETABLE_KEEPER_REGION_PAGES
        + USER_TIMETABLE_KEEPER_BUDGET_PAGES;

    /// **How many durable sessions fit a `table_slots`-slot capability table** with room left for
    /// one ordinary login beside them: the largest `n` with `SLOTS_AT_REST + SLOTS_PER_SESSION * n
    /// + SLOTS_LOGIN_PEAK <= table_slots`. Opening the last at a login peaks no higher: 16 held at
    /// the open, 23 building the second store caretaker (Fork 8 D), 24 building
    /// `user_timetable_keeper`; lower at start-up. Counted from the code on 2026-09-27, not
    /// measured.
    ///
    /// # EXAMPLES
    ///
    /// ```
    /// use login_contract::durable::sessions_held;
    /// assert_eq!(sessions_held(24), 1); // today's table: one session, 24 of 24 at a login's peak
    /// assert_eq!(sessions_held(32), 3); // the table PR #1360 proposes
    /// assert_eq!(sessions_held(16), 0); // too small for any
    /// ```
    pub const fn sessions_held(table_slots: u64) -> usize {
        let fixed = SLOTS_AT_REST + SLOTS_LOGIN_PEAK;
        if table_slots < fixed {
            return 0;
        }
        ((table_slots - fixed) / SLOTS_PER_SESSION) as usize
    }

    /// **The identities a start-up pass re-derives, in manifest order**: every entry of the
    /// durable-session manifest (DECISIONS §125 (which identities have pending work)) that the
    /// owner's suspended list (`super::lists`'s format, [`super::SUSPENDED_LIST`]) does not name.
    /// The caller stops at its own capacity and skips an identity whose stored schedule is missing
    /// or empty, which is a stale manifest line rather than a failure (§125).
    ///
    /// # EXAMPLES
    ///
    /// ```
    /// let manifest: [&[u8]; 3] = [b"alice", b"bob", b"carol"];
    /// let mut left = login_contract::durable::to_rederive(&manifest, b"# suspended\nbob\n");
    /// assert_eq!(left.next(), Some(&b"alice"[..]));
    /// assert_eq!(left.next(), Some(&b"carol"[..]));
    /// assert_eq!(left.next(), None);
    /// ```
    pub fn to_rederive<'a>(
        manifest: &'a [&'a [u8]],
        suspended: &'a [u8],
    ) -> impl Iterator<Item = &'a [u8]> + 'a {
        manifest
            .iter()
            .copied()
            .filter(move |identity| !super::lists(suspended, identity))
    }

    /// **Why the start-up pass skipped a manifest entry** (milestone 152, added 2026-09-27 against
    /// CI's own falsification of `rederive`'s "skipped, not failed" contract with no reason
    /// anywhere to read). One variant per `continue`/`break` site in `components/src/login.rs`'s
    /// `rederive`. Provisional: this lane's, not an architect's.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    #[repr(u8)]
    pub enum RederiveSkip {
        /// No free slot: as many sessions are already kept as this process can hold.
        TableFull = 0,
        /// The name does not fit `filesystem_contract::grant::MAX_NAME`, or this identity already
        /// has a session kept.
        Identity = 1,
        /// The stored schedule is missing or empty: a stale manifest line.
        NoStoredSchedule = 2,
        /// Splitting the session's budget off `durable_ut` failed: out of pages.
        BudgetOutOfPages = 3,
        /// Opening `user_timetable_keeper` (its capability table, its timetable, or the wait for its
        /// readiness word) failed.
        SessionBuildFailed = 4,
    }

    /// How many [`RederiveSkip`] variants there are, and the length [`pack_skip_counts`] and
    /// [`unpack_skip_counts`] agree on.
    pub const REDERIVE_SKIP_REASONS: usize = 5;

    /// **Pack one count per [`RederiveSkip`] reason into one word**, one byte each, saturating at
    /// 255 (ample: `components/src/login.rs`'s `Durables` holds at most a handful of sessions and a boot's
    /// manifest is not expected to grow past that). `counts[RederiveSkip::X as usize]` lands in
    /// byte `X`. The whole word travels as `REDERIVE_SKIPS`'s `RESULT` second word.
    pub const fn pack_skip_counts(counts: &[u32; REDERIVE_SKIP_REASONS]) -> u64 {
        let mut word = 0u64;
        let mut i = 0;
        while i < REDERIVE_SKIP_REASONS {
            let byte = if counts[i] > 255 { 255 } else { counts[i] };
            word |= (byte as u64) << (i * 8);
            i += 1;
        }
        word
    }

    /// The reverse of [`pack_skip_counts`], for a test to read the reply back.
    pub const fn unpack_skip_counts(word: u64) -> [u32; REDERIVE_SKIP_REASONS] {
        let mut counts = [0u32; REDERIVE_SKIP_REASONS];
        let mut i = 0;
        while i < REDERIVE_SKIP_REASONS {
            counts[i] = ((word >> (i * 8)) & 0xff) as u32;
            i += 1;
        }
        counts
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// **The capacity is the note's arithmetic, at both table sizes in play.** 24 slots hold
        /// one session and 28 would be needed for two, since the durable window's page is held at
        /// rest (Fork 8 D); 32 (PR #1360) holds three. Falsified by dropping `SLOTS_LOGIN_PEAK`
        /// from the sum, which answers two for 24.
        #[test]
        fn the_capacity_leaves_room_for_one_login_beside_the_sessions() {
            assert_eq!(sessions_held(24), 1);
            assert_eq!(sessions_held(27), 1);
            assert_eq!(sessions_held(28), 2);
            assert_eq!(sessions_held(32), 3);
            assert_eq!(sessions_held(19), 0);
            assert_eq!(sessions_held(0), 0);
            for slots in 0..64u64 {
                let n = sessions_held(slots) as u64;
                if n > 0 {
                    assert!(
                        SLOTS_AT_REST + SLOTS_PER_SESSION * n + SLOTS_LOGIN_PEAK <= slots,
                        "{n} sessions overflow a {slots}-slot table at a login's peak"
                    );
                }
                assert!(
                    SLOTS_AT_REST + SLOTS_PER_SESSION * (n + 1) + SLOTS_LOGIN_PEAK > slots,
                    "a {slots}-slot table could hold {} sessions, not {n}",
                    n + 1
                );
            }
        }

        /// **A suspended identity is never re-derived, and nothing else is dropped.** The proof
        /// `session_reviver`'s skip carried, now on the function `login`'s start-up pass calls.
        #[test]
        fn a_suspended_identity_is_skipped_and_the_rest_keep_their_order() {
            let manifest: [&[u8]; 3] = [b"alice", b"bob", b"carol"];
            let all: [&[u8]; 3] = manifest;
            assert!(to_rederive(&manifest, b"").eq(all.iter().copied()));
            assert!(to_rederive(&manifest, b"alice\ncarol\n").eq([&b"bob"[..]]));
            // A prefix is not a name, and a comment suspends nobody.
            assert!(to_rederive(&manifest, b"ali\n#bob\n").eq(all.iter().copied()));
            assert_eq!(to_rederive(&[], b"alice\n").count(), 0);
        }

        /// **Skip counts round-trip through one word, and a wild count saturates rather than
        /// overflows into its neighbour's byte.**
        #[test]
        fn skip_counts_round_trip_and_saturate() {
            let counts = [1, 0, 3, 0, 2];
            assert_eq!(unpack_skip_counts(pack_skip_counts(&counts)), counts);
            let wild = [300, 0, 0, 0, 0];
            let word = pack_skip_counts(&wild);
            assert_eq!(unpack_skip_counts(word), [255, 0, 0, 0, 0]);
            // A saturated first byte does not bleed into the second.
            assert_eq!(word & 0xff00, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The list is exact, and every doubtful case lists nobody** (DECISIONS §221 ruling 2).
    /// Falsified once: matching by prefix (`line.starts_with(identity)`) listed `ali` here.
    #[test]
    fn the_run_unvouched_list_names_whole_identities_only() {
        let list = b"# owner-written\nalice\n\n\tbob\r\n#carol\n";
        assert!(lists(list, b"alice"));
        assert!(lists(list, b"bob"));
        for not in [&b"ali"[..], b"alice2", b"carol", b"#carol", b"", b" "] {
            assert!(
                !lists(list, not),
                "{:?} was listed",
                core::str::from_utf8(not)
            );
        }
        assert!(!lists(b"", b"alice"), "an empty list named somebody");
    }

    /// **What `user suspend` and `user resume` write is what `login` reads** (milestone 152
    /// (durable delegation)): a name added is listed, added twice is listed once, removed is not
    /// listed, and a name the list cannot hold is refused rather than written.
    #[test]
    fn the_suspended_list_round_trips_through_its_two_edits() {
        let mut out = [0u8; 128];
        let n = with_listed(b"# owner's list\nalice", b"bob", &mut out).unwrap();
        let list = out;
        assert!(lists(&list[..n], b"bob") && lists(&list[..n], b"alice"));
        let again = with_listed(&list[..n], b"bob", &mut out).unwrap();
        assert_eq!(again, n, "adding a listed name changed the list");
        let n2 = without_listed(&list[..n], b"bob", &mut out).unwrap();
        assert!(!lists(&out[..n2], b"bob") && lists(&out[..n2], b"alice"));
        assert!(
            out[..n2].starts_with(b"# owner's list\n"),
            "the comment was lost"
        );
        for bad in [&b""[..], b"two words", b"#alice"] {
            assert!(with_listed(b"", bad, &mut out).is_none(), "{bad:?}");
        }
        assert!(with_listed(b"alice\n", b"bob", &mut [0u8; 4]).is_none());
        assert!(without_listed(b"alice\n", b"bob", &mut [0u8; 2]).is_none());
        assert_eq!(with_listed(b"", b"alice", &mut out), Some(6));
        assert_eq!(operation(suspend_word()), SUSPEND);
        assert_eq!(operation(logout_word()), LOGOUT);
        assert_eq!(operation(connect_word()), CONNECT);
    }

    #[test]
    fn identity_hint_round_trips_short_names() {
        assert_eq!(identity_hint(b"alice"), identity_hint(b"alice"));
        assert_ne!(identity_hint(b"alice"), identity_hint(b"bob"));
    }

    #[test]
    fn req_and_place_agree_with_cred_proto_on_the_shape() {
        let mut page = [0u8; PAGE];
        let w0 = place(&mut page, b"alice", b"secret", LOGIN).expect("fits");
        assert_eq!(operation(w0), LOGIN);
        let (id, secret) = read(&page, w0).expect("well-formed");
        assert_eq!(id, b"alice");
        assert_eq!(secret, b"secret");
    }

    #[test]
    fn connect_word_carries_no_lengths_and_reads_back_as_connect() {
        let w0 = connect_word();
        assert_eq!(operation(w0), CONNECT);
        // Unlike a LOGIN word, there is nothing else packed into it: the low 32 bits (where `place`
        // packs the two lengths) are zero.
        assert_eq!(w0 & 0xffff_ffff, 0);
        // And it is distinguishable from LOGIN's own opcode, so a front door that only expects
        // CONNECT can refuse a stray LOGIN word rather than mistake it for one.
        assert_ne!(CONNECT, LOGIN);
    }

    #[test]
    fn logout_word_carries_no_lengths_and_reads_back_as_logout() {
        let w0 = logout_word();
        assert_eq!(operation(w0), LOGOUT);
        assert_eq!(w0 & 0xffff_ffff, 0);
        // Distinguishable from both existing front-door/private-channel opcodes, so a front door
        // that dispatches on `operation(w0)` can never confuse the three.
        assert_ne!(LOGOUT, CONNECT);
        assert_ne!(LOGOUT, LOGIN);
    }

    #[test]
    fn every_wire_code_this_contract_defines_is_distinct() {
        // The two namespaces (a request's opcode, and a private channel's OK/DENIED/... verdict)
        // are read from different fields by different code and are allowed to share numbers; this
        // checks each namespace is internally distinct, which is the property a dispatch `match`
        // actually relies on.
        let request_operations = [LOGIN, CONNECT, LOGOUT];
        for (i, a) in request_operations.iter().enumerate() {
            for b in &request_operations[i + 1..] {
                assert_ne!(a, b, "two request opcodes collide");
            }
        }
        let verdicts = [OK, DENIED, MALFORMED, CONNECTED, NO_TERMINAL, LOGGED_OUT];
        for (i, a) in verdicts.iter().enumerate() {
            for b in &verdicts[i + 1..] {
                assert_ne!(a, b, "two verdict codes collide");
            }
        }
    }

    /// The bare words that travel on the front door are an opcode in the top byte and nothing else.
    #[test]
    fn each_bare_word_is_its_opcode_in_the_top_byte_alone() {
        for (word, operation) in [
            (suspend_word(), SUSPEND),
            (rederive_skips_word(), REDERIVE_SKIPS),
            (logout_word(), LOGOUT),
        ] {
            assert_eq!(word >> credential_contract::OPERATION_SHIFT, operation);
            assert_eq!(word << (64 - credential_contract::OPERATION_SHIFT), 0);
        }
    }

    /// Two lengths in one word, each from its own half.
    #[test]
    fn the_two_schedule_lengths_do_not_share_bits() {
        let w = schedule_lengths(0xdead_beef, 0x1234_5678);
        assert_eq!(w, 0x1234_5678_dead_beef);
        assert_eq!(split_schedule_lengths(w), (0xdead_beef, 0x1234_5678));
    }

    /// The durable window is the file service's last, and the budgets a spawner sizes from are the
    /// documented sums. A change to any of these is a decision about what `login` may spend, so it
    /// is written down twice.
    #[test]
    fn the_durable_window_is_the_last_and_the_budgets_are_the_documented_sums() {
        assert_eq!(
            DURABLE_WINDOW as usize + 1,
            filesystem_contract::fs::CLIENT_WINDOWS
        );
        assert_eq!(durable::USER_TIMETABLE_KEEPER_REGION_PAGES, 320);
        assert_eq!(durable::BUDGET_PAGES, 64 + 320 + 416);
        assert_eq!(durable::BUDGET_PAGES, 800);
    }

    /// Skip counts pack one byte each in reason order and saturate at 255, not wrap.
    #[test]
    fn skip_counts_pack_one_byte_each_and_saturate() {
        let packed = durable::pack_skip_counts(&[1, 2, 255, 256, 100_000]);
        assert_eq!(packed, u64::from_le_bytes([1, 2, 255, 255, 255, 0, 0, 0]));
        assert_eq!(durable::unpack_skip_counts(packed), [1, 2, 255, 255, 255]);
    }
}
