//! **A confined process that fuzzes the syscall surface, conducted one call at a time** (milestone
//! 779 (fuzz the surface a confined process can reach), part (b), provisional; calef ruled yes on
//! 2026-10-04 UTC).
//!
//! `fixtures/src/confined_syscall_fuzzer.rs` is an EL0 program, spawned with `user::run` like any
//! confined program, that draws one syscall at a time from a seed and makes it through the real
//! trap entry: `svc`/`ecall`/`syscall`, register marshaling and all, which milestone 752 (a seeded syscall driver with a shadow
//! model)'s kernel threads deliberately skip. This module is the conductor. It spawns three of them, two endowed
//! (`ROLE_A`, `ROLE_B`) and a witness (`ROLE_WITNESS`) holding only the conductor's channel, steps
//! each one by sending `GO_STEP` on its slot 0, and checks between every two calls that nothing
//! escaped the endowment.
//!
//! **The oracle is the conductor's, and the generator never interprets an answer**, so a wrong
//! answer cannot steer it anywhere a right one would not. After every completed call:
//!
//! - **The table diff.** The conductor snapshots the fuzzer's 64 slots before and after. A slot may
//!   empty freely (deletes, revocation, one-shot Replies), but a fill is legal only for a call that
//!   can grant one: `RETYPE_OBJ` (the object's page must lie in the fuzzer's *own* region), `BADGE`
//!   (the same object, restamped with the badge the call named), or `RECEIVE_CAP` (the delivered
//!   object must be a delegation this conductor watched leave a sender, with no more rights and the
//!   same badge, or the one-shot `Reply` of a caller this conductor sees parked waiting for one). A
//!   fill by any other call, a plain `RECEIVE` included, is an escape.
//! - **The delegation ledger.** A `SEND_CAP` that parks stages its delegation; the conductor
//!   records it at the next quiet point, marks it dead when the object is revoked, and drops it
//!   when its sender comes back without a fuzzer having taken it. A `RECEIVE_CAP` fill naming a
//!   dead or unknown delegation is an escape (the #1525 shape: a staged capability surviving to a
//!   later delivery).
//! - **Register truth.** For a `RECEIVE_CAP`, `x1` must name the slot that actually filled (the
//!   table diff says which) or be `NO_CAP`, and `x4` must carry `REPLY_DELIVERED` exactly when the
//!   fill was a `Reply` (milestones 634 and 706). A plain `SEND` collected by a `RECEIVE_CAP`
//!   answers `NO_CAP` whichever side parked first, because the conductor is also a sender: it
//!   releases parked receivers with its own plain sends.
//! - **The badge.** The fuzzers' server endpoint is badged per fuzzer, so every message the
//!   conductor drains off it carries one of those badges or one a fuzzer minted at a `BADGE` call
//!   the conductor watched. Badge 0 is the #1494 shape.
//! - **The witness.** It holds only slot 0, which the generator never names, so every `SYS_INVOKE`
//!   it makes must come back an error and its table must stay empty for the whole run.
//! - **The page oracle.** Every mapping in every fuzzer's space lies in the pages it was spawned
//!   with (image, stack, report) or in the 32-page playground window, and a playground page's
//!   physical frame lies in one of the granted regions or is one of the two gift frames. A mapping
//!   of a frame nobody granted is an escape no register mentions.
//!
//! **Wedged calls are served, not timed out.** A random call may block (a `RECEIVE` on an empty
//! endpoint, a `SEND` nobody drains, a `CALL` waiting its reply, a `WAIT` on an unsignalled
//! notification). The conductor reads the parked thread's disposition and serves it: it drains
//! sends with `RECEIVE_CAP` (checking the badge, answering calls, deleting what lands in its own
//! table), releases receives with its own sends, answers `CALL`s with `ipc_reply`, and signals the
//! notification. Serving is keyed on observed park states, never on timing, so a seed replays
//! identically. Only a call that neither completes nor parks anywhere serviceable within the
//! deadline is a finding, and the report page names it, because the generator writes the call
//! before making it.
//!
//! **The seed is the reproducer** (752's discipline, one level down). `NIFE_CONFINED_FUZZ_SEEDS=
//! <first>:<count>` at build time replaces the committed list; a failure prints seed, step and the
//! call. Coverage-blind on purpose, as the proposal records.
//!
//! # BUGS
//!
//! - **Interrupt and death deliveries are unreachable, so milestone 714 (the sibling `RECEIVE_CAP`
//!   paths get a receiver-first test)'s two replayable
//!   falsifications cannot be run against this driver**: no fuzzer endpoint is bound to an
//!   interrupt and no fuzzer has a supervision channel, and the generator cannot make one (it
//!   holds no TCB capability, and `BIND` on the shared notification reaches only refusals). The
//!   x1-truth oracle would catch both shapes if a delivery existed; `receive_cap_attack_tests`
//!   keeps those two records.
//! - **Two more of the recorded falsifications cannot redden this driver, structurally** (swept
//!   to 400 seeds each, 2026-10-06 UTC): milestone 633 (an outside agent attacks the confinement
//!   claim)'s staged `outgoing_cap` and 706's missing
//!   `REPLY_DELIVERED` tag. Both need fuzzer-to-fuzzer IPC in one of two arrival orders (a plain
//!   `RECEIVE` collecting a parked `SEND_CAP`, a `CALL` reaching a parked `RECEIVE_CAP`), and
//!   this conductor's protocol closes both windows: at every quiet point each fuzzer is parked
//!   on its GO, only the stepped fuzzer makes calls, and the conductor is the sole counterparty
//!   for a mid-call park, always taking with `RECEIVE_CAP` and always answering `CALL`s. The
//!   orders those patches flip never arise; `receive_cap_attack_tests` keeps both records, and
//!   the fills the *other* four patches produce (seeds 0, 7, 16, 52) show the delivery paths
//!   that do arise are still judged.
//! - **`GO` (slot 0) is never fuzzed**, and holds `READ` alone, so `SEND` and `SEND_CAP` on it are
//!   never exercised: the fixture's own record, inherited.
//! - **No thread or timer is ever made**: the generator rewrites those retypes to a rendezvous, so
//!   the seed stays the reproducer (a second thread at a random entry breaks determinism). TCB and
//!   timer methods are therefore out of reach, as in 752.
//! - **The conductor cannot see shared-endpoint badges.** The fuzzers hold the shared endpoint
//!   unbadged and may mint badged copies the conductor does not know, so a badge check runs only
//!   on the server endpoint, where the conductor set the badges itself.
//! - **Stale names are admitted.** A capability whose object no longer resolves (its region was
//!   destroyed under it) is left alone by the object-page check rather than judged; every path
//!   that *fills* a slot is still judged.
//! - **A justified-but-late delegation is judged at the quiet point, not the step.** A `SEND_CAP`
//!   whose receiver was already parked completes the receiver's step before the sender's, so the
//!   fill can be judged only once the sender's report is in. One step of latency, never more,
//!   because a sender always completes when its delivery is taken.
//!
//! Name: provisional (milestone 779 (fuzz the surface a confined process can reach)'s lane), as
//! are the module's, the fixture's and the protocol
//! crate's.

use abi::Error;
use confined_fuzz_protocol as proto;
use confined_fuzz_protocol::report;

use super::*;
// The `cap` module and these constructors still carry their pre-sweep names
// (design/naming/capability-worklist.md renames them when the tree is quiet); aliased here so
// this file spells `capability` out everywhere a reader looks (calef, 2026-10-06).
use crate::cap::{
    Cap as Capability, Object, Rights, address_space_cap as address_space_capability,
    memory_region_cap_rights as memory_region_capability_rights,
    notification_cap as notification_capability, page_frame_cap as page_frame_capability,
    rendezvous_cap as rendezvous_capability, rendezvous_cap_badged as rendezvous_capability_badged,
};
use crate::sched::{
    self, RendezvousId, current_cap as current_capability,
    delete_current_cap as delete_current_capability,
};
use crate::thread::{State, ThreadId, Wait, WaitRole};

const SLOTS: usize = abi::CAPABILITY_TABLE_SLOTS as usize;
/// Fuzzer A, fuzzer B, and the witness.
const A: usize = 0;
const WITNESS: usize = 2;
const FUZZERS: usize = 3;
/// Calls per seed, across all three fuzzers.
const STEPS: usize = 96;
/// The committed suite seeds, then [`CORPUS`]. Measured 2026-10-06 (UTC) under QEMU: about 32
/// seeds/s on aarch64, 21 on riscv64, 36 on x86_64 (96 calls each), so this is about one second
/// of suite time on each.
const SUITE_SEEDS: u64 = 32;
/// **Seeds kept because they found something**, 752's corpus discipline: a seed is tied to this
/// generator, so a change to the fixture's `pick` re-rolls every seed and the corpus must be
/// re-found under each falsification patch. Seed 52 is the first a 400-seed sweep found red under
/// `a_call_to_a_plain_receive_is_answered_gone`'s patch (§246 (a plain `RECEIVE` never takes a
/// capability)'s caller half); the other patches
/// that redden this driver first do so inside the suite range itself (seed 0 under 634's,
/// seed 7 under revocation-in-flight, seed 16 under §246's receiver half).
const CORPUS: &[u64] = &[52];
/// The sweep's guest-time capability (752's), so a weekly run with a large count stops between seeds
/// inside the per-test budget.
const SWEEP_SECS: u64 = 60;
/// The fuzzers' badges on the server endpoint. Nonzero, distinct, and never mintable by a call.
const BADGE_A: u64 = 0x779A;
const BADGE_B: u64 = 0x779B;
/// Pages in each fuzzer's own region (its retypes and the page tables its maps need) and in the
/// conductor's (the endpoints, the notification, the gifts and the report pages).
const OWN_REGION_PAGES: u64 = 16;
const CONDUCTOR_PAGES: u64 = 16;

const PAGE: u64 = address_space_map::PAGE;

type Snap = [Option<(Object, u32)>; SLOTS];

/// One delegation the conductor watched leave a sender: parked in a `SEND_CAP` that has not been
/// taken, or spent by one that completed (its receiver's step may still be awaiting judgment).
#[derive(Clone, Copy, Debug)]
struct Delegation {
    sender: usize,
    obj: Object,
    rights: u32,
    /// The capability's badge: a rendezvous delegation keeps the sender's.
    badge: u64,
    /// The object was revoked while in flight; any delivery now is the revocation-in-flight
    /// escape.
    revoked: bool,
}

#[derive(Clone, Copy, Default)]
struct Ledger {
    parked: [Option<Delegation>; 8],
    spent: [Option<Delegation>; 8],
}

/// A `RECEIVE_CAP` fill whose sender's step had not been collected when the receiver's was: judged
/// at the next quiet point, when the ledger is whole (the module's last `BUGS` entry).
struct Pending {
    fuzzer: usize,
    slot: u64,
    obj: Object,
    rights: u32,
    x1: u64,
    x4: u64,
}

/// Everything one seed runs against. In a static, not the test thread's frame: the snapshots alone
/// are 6 KiB, and the boot stack's high-water is why 752 keeps its model in a static too.
struct World {
    conductor_region: u64,
    own: [u64; 2],
    server: RendezvousId,
    go: [RendezvousId; FUZZERS],
    gifts: [u64; 2],
    reports: [u64; FUZZERS],
    /// Badges a fuzzer minted at a `BADGE` call, so a later delivery may carry one.
    minted: [u64; 16],
    minted_n: usize,
    /// Every child region a fuzzer carved with `SPLIT`, in creation order, so teardown can free
    /// them before their parents (a parent with live children refuses to reclaim).
    children: [u64; 32],
    children_n: usize,
    ledger: Ledger,
}

struct Fuzzer {
    tid: ThreadId,
    /// The report page's `STEPS` word when last collected.
    steps: u64,
    table: Snap,
    /// The virtual addresses this fuzzer was spawned with (image, stack, report): the page
    /// oracle's baseline.
    baseline: [u64; 24],
    baseline_n: usize,
}

struct Conductor {
    world: World,
    fuzzers: [Fuzzer; FUZZERS],
    pending: Option<Pending>,
    /// Which fuzzers hold a *delivered* one-shot Reply and are parked awaiting it, so the reply
    /// service only ever answers a caller whose Reply capability exists. `ipc_reply` to a caller whose
    /// CALL was never collected would strand it on the endpoint's queue (the kernel's reply path
    /// assumes the caller was collected; a Reply capability is minted only then), and its stale queue
    /// entry would deliver its own CALL to itself later. An uncollected caller is drained like
    /// any queued sender instead.
    awaits_reply: [bool; FUZZERS],
    seed: u64,
    step: usize,
    /// A deterministic word for the conductor's releases, so a replay sends the same ones. Starts
    /// above the conductor protocol's own words (`GO_STEP` is 1, `GO_EXIT` is 2), so a release
    /// that somehow landed on a GO channel could never read as one of them.
    relay: u64,
}

static CONDUCTOR: spin::Mutex<Conductor> = spin::Mutex::new(Conductor {
    world: World {
        conductor_region: 0,
        own: [0; 2],
        server: 0,
        go: [0; FUZZERS],
        gifts: [0; 2],
        reports: [0; FUZZERS],
        minted: [0; 16],
        minted_n: 0,
        children: [0; 32],
        children_n: 0,
        ledger: Ledger {
            parked: [None; 8],
            spent: [None; 8],
        },
    },
    fuzzers: [const {
        Fuzzer {
            tid: 0,
            steps: 0,
            table: [None; SLOTS],
            baseline: [0; 24],
            baseline_n: 0,
        }
    }; FUZZERS],
    pending: None,
    awaits_reply: [false; FUZZERS],
    seed: 0,
    step: 0,
    relay: 3,
});

/// The module's one failure voice: seed, step, and the replay line, 752's `mismatch!`.
macro_rules! escape {
    ($c:expr, $($arg:tt)*) => {
        panic!(
            "confined fuzzer: seed {:#x} step {}: {} (replay: NIFE_CONFINED_FUZZ_SEEDS={}:1)",
            $c.seed, $c.step, format_args!($($arg)*), $c.seed
        )
    };
}

// -------------------------------------------------------------------------------------------
// Reading the fuzzers' state.

fn report_word(c: &Conductor, x: usize, i: usize) -> u64 {
    // SAFETY: the conductor mapped the report page itself at spawn; the fuzzer's stores are
    // volatile and ordered before its park on GO, which is the only time this is read.
    unsafe {
        core::ptr::read_volatile(
            (crate::arch::mmu::phys_to_virt(c.world.reports[x]) as *const u64).add(i),
        )
    }
}

fn snapshot(c: &Conductor, x: usize) -> Snap {
    let mut s: Snap = [None; SLOTS];
    sched::with_capability_table(c.fuzzers[x].tid, |t| {
        for (k, slot) in s.iter_mut().enumerate() {
            if let Ok(capability) = t.get(k as u64) {
                *slot = Some((capability.object, capability.rights.bits()));
            }
        }
    })
    .unwrap_or_else(|| panic!("fuzzer {x}'s capability table could not be read"));
    s
}
fn parked_on_go(c: &Conductor, x: usize) -> bool {
    sched::rendezvous_waiting_receivers(c.world.go[x]) == 1
}

fn disposition(c: &Conductor, x: usize) -> Option<(State, Option<Wait>)> {
    sched::thread_death_disposition(c.fuzzers[x].tid).map(|d| (d.state, d.wait_on))
}

fn live(c: &Conductor, x: usize) -> bool {
    sched::is_thread_present(c.fuzzers[x].tid)
}

/// A completed, uncollected step: parked back on GO having bumped its step count.
fn completed(c: &Conductor, x: usize) -> bool {
    parked_on_go(c, x) && report_word(c, x, report::STEPS) > c.fuzzers[x].steps
}

/// What the fuzzer's report page says about the call it just made.
#[derive(Clone, Copy)]
struct Call {
    number: u64,
    words: [u64; 6],
    rets: [u64; 6],
}

fn read_call(c: &Conductor, x: usize) -> Call {
    Call {
        number: report_word(c, x, report::NUMBER),
        words: core::array::from_fn(|i| report_word(c, x, report::ARGS + i)),
        rets: core::array::from_fn(|i| report_word(c, x, report::RETURNS + i)),
    }
}

fn badge_of(obj: &Object) -> u64 {
    match obj {
        Object::Rendezvous(_, badge) => *badge,
        _ => 0,
    }
}

/// A `BADGE` fill holds the same endpoint the fuzzer held; object equality is badge-sensitive on
/// rendezvous caps, so compare the stripped objects.
fn strip_badge(obj: Object) -> Object {
    match obj {
        Object::Rendezvous(ep, _) => Object::Rendezvous(ep, 0),
        other => other,
    }
}

// -------------------------------------------------------------------------------------------
// The conductor's services: a parked call is served, never timed out.

impl Conductor {
    /// Serve fuzzer `x`, which is blocked somewhere that is not its GO, with one
    /// [`sched::conductor_move`]: an operation that can never park the conductor, which is the
    /// whole discipline here. Three separate suite hangs came from check-then-act services around
    /// the blocking IPC primitives: on four cores, the party the check saw can be gone by the act,
    /// and a parked conductor is a dead suite (the kernel is fine, every fuzzer is parked on its
    /// GO, and nobody is left to wake the judge).
    ///
    /// **Only the stepped fuzzer is ever served**, which is also what makes the audit tractable:
    /// at every quiet point each fuzzer is parked on its GO and only the stepped fuzzer makes
    /// calls, so the one thread that can be blocked mid-call is the one being served.
    ///
    /// **Any rendezvous or notification, not only the conductor's.** A fuzzer retypes endpoints
    /// and notifications of its own out of its region and parks on them; each such park is
    /// legitimate kernel behavior, and the conductor releases every one the same way.
    fn serve(&mut self, x: usize) -> bool {
        let Some((State::Blocked, Some(wait))) = disposition(self, x) else {
            return false; // still running, or between states: yield and look again
        };
        match wait {
            Wait::Rendezvous(ep, WaitRole::Receiver) => {
                // But never the conductor's own channel: a fuzzer that has come back to its GO is
                // the completion check's business, not a service's.
                if ep == self.world.go[x] {
                    return false;
                }
                self.relay += 1;
                self.move_on(ep, self.relay, ep == self.world.server)
            }
            Wait::Rendezvous(ep, WaitRole::Sender) => self.move_on(ep, 0, ep == self.world.server),
            Wait::Rendezvous(ep, WaitRole::Reply) => {
                // A caller parked awaiting its reply. Only one whose Reply capability was delivered (so
                // the kernel collected its CALL) may be answered by tid: `ipc_reply` only ever
                // touches a Reply-parked thread, so it cannot park or corrupt anybody, but
                // answering a caller whose CALL was never collected strands it on the endpoint's
                // queue. An uncollected caller is still a queued sender: take its CALL instead.
                if self.awaits_reply[x] {
                    self.awaits_reply[x] = false;
                    sched::ipc_reply(self.fuzzers[x].tid, [0, 0]);
                    true
                } else {
                    self.move_on(ep, 0, false)
                }
            }
            Wait::Notification(n) => sched::notification_signal(n, 1).is_ok(),
            // A fuzzer that drew `AddressSpace::WAIT` on its own space with the one admitted form
            // and a matching word (milestone 812 (`std::thread::spawn` runs real threads in one
            // address space)) is parked on a futex, which a wake of its key releases.
            Wait::Futex(k) => sched::futex_wake(k.space, k.address, 1) == 1,
        }
    }

    /// One [`sched::conductor_move`] with the audit its `Took` answer owes: the badge check on
    /// the badged server endpoint, the answer a collected `CALL` is owed, and the ledger and
    /// table care for anything filed in the conductor's own table.
    fn move_on(&mut self, ep: RendezvousId, word: u64, badged: bool) -> bool {
        match sched::conductor_move(ep, word) {
            sched::ConductorMove::None => false,
            sched::ConductorMove::Sent => true,
            sched::ConductorMove::Took([_w0, x1, _w2, badge, _tag]) => {
                if badged && !self.known_badge(badge) {
                    escape!(
                        self,
                        "the server endpoint delivered badge {badge:#x}, which no fuzzer holds"
                    );
                }
                if x1 != proto::NO_CAPABILITY {
                    // A delivery into the conductor's own table: a CALL's one-shot Reply to
                    // answer, or a delegation to the conductor (legitimate, and not wanted). One
                    // revoked while staged is the revocation-in-flight escape either way.
                    // **Judged by object, not by `x4`:** the tag is a claim under test (milestone
                    // 706 (a `CALL` server can tell a Reply from a delegation)'s falsification
                    // removes it); the Reply in the conductor's own table is the thing itself.
                    if let Ok(capability) = current_capability(x1) {
                        match capability.object {
                            Object::Reply(caller) => sched::ipc_reply(caller, [0, 0]),
                            ref obj => {
                                if self.ledger_revoke_mark(obj) {
                                    escape!(
                                        self,
                                        "a revoked capability was delivered to the conductor as \
                                         {obj:?}"
                                    );
                                }
                                self.ledger_forget(obj);
                            }
                        }
                    }
                    let _ = delete_current_capability(x1);
                }
                true
            }
        }
    }

    fn known_badge(&self, badge: u64) -> bool {
        badge == BADGE_A
            || badge == BADGE_B
            || self.world.minted[..self.world.minted_n].contains(&badge)
    }

    /// Note a child region a fuzzer carved, for teardown. Refuses (and so fails the
    /// justification) only when the record is full, which is a finding about the seed's shape.
    fn record_child(&mut self, child: u64) -> bool {
        if self.world.children[..self.world.children_n].contains(&child) {
            return true;
        }
        if self.world.children_n == self.world.children.len() {
            return false;
        }
        self.world.children[self.world.children_n] = child;
        self.world.children_n += 1;
        true
    }

    /// Forget the ledger entries naming `obj`, without judging them: the conductor's own take is
    /// a legitimate delivery, so its entry is spent, not escaped.
    fn ledger_forget(&mut self, obj: &Object) {
        for e in self
            .world
            .ledger
            .parked
            .iter_mut()
            .chain(self.world.ledger.spent.iter_mut())
        {
            if let Some(d) = e
                && &d.obj == obj
            {
                *e = None;
            }
        }
    }
}

// -------------------------------------------------------------------------------------------
// The oracles, run once per collected step.

impl Conductor {
    /// Judge one completed step of fuzzer `x`: the table diff, the ledger, register truth, the
    /// witness's refusals, and the page oracle.
    fn collect(&mut self, x: usize) {
        let call = read_call(self, x);
        let now = snapshot(self, x);
        // However its CALL came back, it came back: it no longer awaits a reply.
        self.awaits_reply[x] = false;

        // The witness holds only GO, which the generator never names: every invoke must be
        // refused, and its table must never fill at all.
        if x == WITNESS {
            if call.number == abi::SYS_INVOKE && (call.rets[0] as i64) >= 0 {
                escape!(
                    self,
                    "the witness's invoke on slot {} method {} succeeded with {:#x}",
                    call.words[0],
                    call.words[1],
                    call.rets[0]
                );
            }
            if now
                .iter()
                .enumerate()
                .any(|(s, held)| held.is_some() && s != proto::GO as usize)
            {
                escape!(self, "the witness holds a capability it was never granted");
            }
        }

        let mut fills = [0usize; 2];
        let mut fills_n = 0usize;
        for s in 0..SLOTS {
            if self.fuzzers[x].table[s].is_none() && now[s].is_some() {
                if fills_n == fills.len() {
                    escape!(self, "one call filled more than {} slots", fills.len());
                }
                fills[fills_n] = s;
                fills_n += 1;
            }
        }
        let mut justified_slot: Option<u64> = None;
        let mut justified_reply = false;
        for &s in &fills[..fills_n] {
            let (obj, rights) = now[s].unwrap();
            if let Object::Reply(tid) = obj {
                // A one-shot Reply names its caller, who parks waiting for it at the delivery
                // itself, so this is judged now, not at the quiet point: by then the conductor's
                // own `ipc_reply` service may have answered the caller already.
                let parked = self
                    .fuzzers
                    .iter()
                    .position(|f| f.tid == tid)
                    .is_some_and(|y| {
                        disposition(self, y).is_some_and(|(st, w)| {
                            st == State::Blocked
                                && matches!(w, Some(Wait::Rendezvous(_, WaitRole::Reply)))
                        })
                    });
                if !parked {
                    escape!(
                        self,
                        "fuzzer {x}'s slot {s} holds a Reply nobody parked for"
                    );
                }
                if let Some(y) = self.fuzzers.iter().position(|f| f.tid == tid) {
                    self.awaits_reply[y] = true;
                }
                justified_reply = true;
                justified_slot = Some(s as u64);
                continue;
            }
            if self.was_receive_capability(x, &call) {
                // A delivery's justification can depend on the sender's step, which may not be
                // collected yet: defer to the quiet point (the module's last BUGS entry).
                if self.pending.is_some() {
                    escape!(self, "two deliveries await judgment at once");
                }
                self.pending = Some(Pending {
                    fuzzer: x,
                    slot: s as u64,
                    obj,
                    rights,
                    x1: call.rets[1],
                    x4: call.rets[4],
                });
                continue;
            }
            let ok = self.justify_fill(x, &call, obj, rights);
            if !ok {
                escape!(
                    self,
                    "fuzzer {x}'s slot {s} filled with {:?} (rights {rights:#x}) after syscall {} \
                     words {:?} on held {:?}, which grants nothing",
                    obj,
                    call.number,
                    call.words,
                    self.held(x, &call)
                );
            }
            justified_slot = Some(s as u64);
        }

        // Register truth for a `RECEIVE_CAP` (milestones 634 and 706): x1 names the slot that
        // actually filled (the table diff says which) or is NO_CAP, and x4 tags exactly a Reply.
        // A plain `RECEIVE` defines w0..w3 only and leaves x4 as the caller left it, so there is
        // nothing to check there. A deferred fill is checked at the quiet point, with its own x1
        // and x4.
        if call.number == abi::SYS_INVOKE
            && (call.rets[0] as i64) >= 0
            && self.was_receive_capability(x, &call)
            && (self.pending.is_none() || self.pending.as_ref().unwrap().fuzzer != x)
        {
            let want = justified_slot.unwrap_or(proto::NO_CAPABILITY);
            if call.rets[1] != want {
                escape!(
                    self,
                    "RECEIVE_CAP answered x1 = {got:#x} where the table says {want:#x}",
                    got = call.rets[1]
                );
            }
            let tag = u64::from(justified_reply) * abi::rendezvous::REPLY_DELIVERED;
            if call.rets[4] != tag {
                escape!(
                    self,
                    "RECEIVE_CAP answered x4 = {got:#x} where the delivery says {tag:#x}",
                    got = call.rets[4]
                );
            }
        }

        // A revoke kills the in-flight copy; a badge minted is a badge that may be seen.
        if call.number == abi::SYS_INVOKE {
            match call.words[1] {
                abi::page_frame::REVOKE => {
                    if let Some(Some((Object::PageFrame(phys, _), _))) =
                        self.fuzzers[x].table.get(call.words[0] as usize)
                    {
                        for e in self.world.ledger.parked.iter_mut().flatten() {
                            if matches!(e.obj, Object::PageFrame(p, _) if p == *phys) {
                                e.revoked = true;
                            }
                        }
                        for e in self.world.ledger.spent.iter_mut().flatten() {
                            if matches!(e.obj, Object::PageFrame(p, _) if p == *phys) {
                                e.revoked = true;
                            }
                        }
                    }
                }
                abi::rendezvous::BADGE
                    if matches!(
                        self.held(x, &call),
                        Some(Object::Rendezvous(ep, _)) if ep == self.world.server
                    ) =>
                {
                    // Only a badge minted on the server endpoint matters: that is the one the
                    // conductor's drains check.
                    let badge = call.words[2];
                    if badge != 0 && !self.known_badge(badge) {
                        if self.world.minted_n == self.world.minted.len() {
                            escape!(self, "the minted-badge record overflowed");
                        }
                        self.world.minted[self.world.minted_n] = badge;
                        self.world.minted_n += 1;
                    }
                }
                _ => {}
            }
        }

        // A SEND_CAP that completed spent its delegation: its receiver may be waiting on the
        // record to judge the fill. (Gated on the slot holding an endpoint: `SEND_CAP` and
        // `RETYPE_OBJ` share the number 2.)
        if call.number == abi::SYS_INVOKE
            && call.words[1] == proto::SEND_CAPABILITY
            && matches!(self.held(x, &call), Some(Object::Rendezvous(..)))
            && (call.rets[0] as i64) >= 0
            && let Some(Some((obj, rights))) =
                self.fuzzers[x].table.get(call.words[2] as usize).copied()
        {
            let narrowed = call.words[3] as u32 & Rights::ALL.bits();
            self.ledger_spend(Delegation {
                sender: x,
                obj,
                rights: narrowed & rights,
                badge: badge_of(&obj),
                revoked: false,
            });
        }

        self.check_objects(x, &now);
        self.check_pages(x);
        self.fuzzers[x].table = now;
        self.fuzzers[x].steps = report_word(self, x, report::STEPS);
    }

    /// May `call`, made by fuzzer `x`, have filled a slot with `obj` at these rights?
    /// The object the call's slot held when it was made (method numbers collide across object
    /// types: `RECEIVE_CAP` and `SPLIT` are both 3, `RECEIVE` and `RETYPE` both 1, so every
    /// method question is really an object-and-method question).
    fn held(&self, x: usize, call: &Call) -> Option<Object> {
        self.fuzzers[x]
            .table
            .get(call.words[0] as usize)
            .copied()
            .flatten()
            .map(|(o, _)| o)
    }

    /// Was this call a `RECEIVE_CAP` on an endpoint, the one call whose fills are deliveries?
    fn was_receive_capability(&self, x: usize, call: &Call) -> bool {
        call.number == abi::SYS_INVOKE
            && call.words[1] == proto::RECEIVE_CAPABILITY
            && matches!(self.held(x, call), Some(Object::Rendezvous(..)))
    }

    /// May `call`, made by fuzzer `x`, have filled a slot with `obj` at these rights? (A
    /// `RECEIVE_CAP`'s fills are deliveries, judged at the quiet point, never here.)
    fn justify_fill(&mut self, x: usize, call: &Call, obj: Object, rights: u32) -> bool {
        if call.number != abi::SYS_INVOKE {
            return false;
        }
        match self.held(x, call) {
            Some(region @ Object::MemoryRegion(_)) => {
                // Spending the untyped the invoked capability names: a frame, an object or a
                // child region, each out of pages that region already owned, and no other's.
                let spent_from = match region {
                    Object::MemoryRegion(r) => crate::memory_region::region_bounds(r),
                    _ => None,
                };
                let within = |page: u64, size: u64| {
                    spent_from
                        .is_some_and(|(base, span)| page >= base && page + size <= base + span)
                };
                let page = match &obj {
                    Object::PageFrame(phys, n) => Some((*phys, n.get() * PAGE)),
                    other => sched::object_page(other).map(|p| (p, PAGE)),
                };
                if let Some((p, span)) = page
                    && within(p, span)
                {
                    return true;
                }
                matches!(obj, Object::MemoryRegion(child)
                    if crate::memory_region::region_bounds(child)
                        .is_some_and(|(base, size)| within(base, size))
                        && self.record_child(child))
            }
            Some(Object::Rendezvous(..)) if call.words[1] == abi::rendezvous::BADGE => {
                // The same endpoint the fuzzer already held, restamped with the badge it named.
                let same = self.fuzzers[x].table.iter().flatten().any(|(o, r)| {
                    strip_badge(*o) == strip_badge(obj) && *r == rights && badge_of(o) == 0
                });
                same && badge_of(&obj) == call.words[2] && call.words[2] != 0
            }
            _ => false,
        }
    }

    /// A delivery's object is a live delegation with these rights. Consumes it.
    fn ledger_match(&mut self, obj: &Object, badge: u64, rights: u32) -> bool {
        let mut found = None;
        for e in self
            .world
            .ledger
            .parked
            .iter_mut()
            .chain(self.world.ledger.spent.iter_mut())
        {
            if let Some(d) = e
                && d.obj == *obj
                && d.badge == badge
            {
                found = Some((d.rights, d.revoked, d.sender));
                *e = None;
                break;
            }
        }
        match found {
            Some((granted, revoked, _)) => !revoked && rights & !granted == 0,
            None => false,
        }
    }

    /// Mark every entry for `obj` revoked (a `REVOKE` swept it), returning whether any was.
    fn ledger_revoke_mark(&mut self, obj: &Object) -> bool {
        let mut any = false;
        for e in self
            .world
            .ledger
            .parked
            .iter_mut()
            .chain(self.world.ledger.spent.iter_mut())
        {
            if let Some(d) = e
                && &d.obj == obj
            {
                d.revoked = true;
                any = true;
            }
        }
        any
    }

    fn ledger_spend(&mut self, d: Delegation) {
        for slot in self.world.ledger.spent.iter_mut() {
            if slot.is_none() {
                *slot = Some(d);
                return;
            }
        }
    }

    /// Is `(base, size)` wholly inside fuzzer `x`'s own region? A `SPLIT` child's pages are.
    fn extent_within_own(&self, x: usize, base: u64, size: u64) -> bool {
        crate::memory_region::region_bounds(self.world.own[x])
            .is_some_and(|(ob, os)| base >= ob && base + size <= ob + os)
    }

    fn in_any_bound(&self, page: u64) -> bool {
        [
            self.world.conductor_region,
            self.world.own[0],
            self.world.own[1],
        ]
        .iter()
        .any(|&r| {
            crate::memory_region::region_bounds(r)
                .is_some_and(|(base, size)| page >= base && page < base + size)
        })
    }

    /// Every capability the fuzzer holds names something inside the endowment's pages.
    fn check_objects(&self, x: usize, snap: &Snap) {
        for (obj, _rights) in snap.iter().flatten() {
            match obj {
                Object::Rendezvous(..)
                | Object::Notification(_)
                | Object::ThreadControlBlock(_) => {
                    if let Some(page) = sched::object_page(obj)
                        && !self.in_any_bound(page)
                    {
                        escape!(
                            self,
                            "fuzzer {x} holds {:?}, an object outside every grant",
                            obj
                        );
                    }
                }
                Object::PageFrame(phys, _) => {
                    if !self.in_any_bound(*phys) {
                        escape!(
                            self,
                            "fuzzer {x} holds a frame at {phys:#x} outside every grant"
                        );
                    }
                }
                Object::MemoryRegion(r) => {
                    // The conductor's own, a fuzzer's own, a `SPLIT` child of either (one fuzzer
                    // may delegate its child to the other, so both extents count), or a stale
                    // name whose region is already destroyed: a capability is a name, and it
                    // stops resolving without anyone reaching anything through it.
                    let granted = *r == self.world.conductor_region
                        || self.world.own.contains(r)
                        || crate::memory_region::region_bounds(*r).is_none_or(|(base, size)| {
                            (0..2).any(|x| self.extent_within_own(x, base, size))
                        });
                    if !granted {
                        escape!(self, "fuzzer {x} holds region {r:#x} it was never granted");
                    }
                }
                Object::Irq(_) | Object::DeviceFrame(_) => {
                    escape!(self, "fuzzer {x} holds {:?}, which no grant names", obj);
                }
                Object::Reply(tid) if !self.fuzzers.iter().any(|f| f.tid == *tid) => {
                    escape!(self, "fuzzer {x} holds a Reply to a stranger");
                }
                _ => {}
            }
        }
    }

    /// Every mapping lies in the user half, and every mapping the fuzzer added (anything not in
    /// its spawn-time baseline) sits on a frame inside a granted region or one of the two gifts.
    /// The *address* is placement, not authority: the kernel permits a user frame at any aligned
    /// user va (va 0 included), and no grant is crossed by where a process lays out its own
    /// memory. The two things that are escapes: a mapping in the kernel half, and a mapping onto
    /// a frame nobody granted. `0x8000_0000_0000_0000` is the lowest user top of the three ISAs
    /// (riscv64's sv39), so nothing at or above it is a user va anywhere.
    fn check_pages(&self, x: usize) {
        let Some(root) = sched::thread_space_root(self.fuzzers[x].tid) else {
            return; // a fuzzer whose space is gone is a mortality finding, not this oracle's
        };
        let mut cursor = 0;
        while let crate::revoke::Listing::Entry(next, va) =
            crate::revoke::list_mapping(root, cursor)
        {
            if va >= 0x8000_0000_0000_0000 {
                escape!(self, "fuzzer {x} maps {va:#x}, in the kernel half");
            }
            let known = self.fuzzers[x].baseline[..self.fuzzers[x].baseline_n].contains(&va);
            if !known
                && let Some((phys, _)) = crate::arch::mmu::translate_at(root, va)
                && !self.in_any_bound(phys)
                && !self.world.gifts.contains(&phys)
            {
                escape!(
                    self,
                    "fuzzer {x} maps {va:#x} onto frame {phys:#x} it was never granted"
                );
            }
            cursor = next;
        }
    }
}

// -------------------------------------------------------------------------------------------
// The step loop.

impl Conductor {
    /// Send `word` (a step or the exit) to fuzzer `x`'s GO, without ever being able to park on
    /// it: the fuzzer is parked there by the quiet point, so the move lands at once, and the
    /// loop only covers the impossible case.
    fn go_send(&mut self, x: usize, word: u64) {
        while !matches!(
            sched::conductor_move(self.world.go[x], word),
            sched::ConductorMove::Sent
        ) {
            sched::yield_now();
        }
    }

    /// Step fuzzer `x` once, then settle: serve every blocked call until all three fuzzers are
    /// parked back on their conductor channels, collecting each completed step as it lands. A
    /// call that neither completes nor parks anywhere serviceable within the deadline is a
    /// finding, named by the report page.
    fn step_and_settle(&mut self, x: usize) {
        self.go_send(x, proto::GO_STEP);
        let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
        loop {
            for y in 0..FUZZERS {
                if completed(self, y) {
                    self.collect(y);
                }
            }
            if !live(self, x) {
                let call = read_call(self, x);
                escape!(
                    self,
                    "fuzzer {x} died mid-call: syscall {} words {:?} rets {:?}",
                    call.number,
                    call.words,
                    call.rets
                );
            }
            if (0..FUZZERS).all(|y| parked_on_go(self, y))
                && (0..FUZZERS).all(|y| !completed(self, y))
            {
                // The second conjunct is the race: the stepped fuzzer can park *between* the
                // collect pass at the top of this loop and the all-parked check right here, in
                // which case the first conjunct alone would return with a completed, uncollected
                // step. Its effects (a SPLIT child, a delegation spent) would then be judged
                // against the fuzzer's *next* call, which was this suite's one-in-ten flake:
                // the conductor's own history ring caught the conductor skipping the judgment
                // (2026-10-06 UTC).
                return;
            }
            if !parked_on_go(self, x) {
                self.serve(x);
            }
            sched::yield_now();
            if crate::arch::timer::now() > deadline {
                let call = read_call(self, x);
                escape!(
                    self,
                    "fuzzer {x} never came back: syscall {} words {:?} rets {:?}, now {:?}",
                    call.number,
                    call.words,
                    call.rets,
                    disposition(self, x)
                );
            }
        }
    }

    /// The quiet point between steps: everyone is parked on GO, every delivery has landed and
    /// every sender's report is in, so the ledger is whole and anything still unjustified is an
    /// escape.
    fn quiet(&mut self) {
        // A deferred delivery is judged now, with the ledger whole. (A Reply never reaches here:
        // it is judged at the delivery, before the conductor's reply service can answer its
        // caller.)
        if let Some(p) = self.pending.take() {
            let ok = self.ledger_match(&p.obj, badge_of(&p.obj), p.rights);
            if !ok {
                escape!(
                    self,
                    "fuzzer {}'s slot {} filled with {:?} (rights {:#x}) from no live delegation",
                    p.fuzzer,
                    p.slot,
                    p.obj,
                    p.rights
                );
            }
            if p.x1 != p.slot {
                escape!(
                    self,
                    "RECEIVE_CAP answered x1 = {:#x} where the table says {:#x}",
                    p.x1,
                    p.slot
                );
            }
            // A deferred fill is a delegation, never a Reply (those are judged at the delivery),
            // so its tag is 0.
            if p.x4 != 0 {
                escape!(
                    self,
                    "RECEIVE_CAP answered x4 = {got:#x} where a delegation's tag is 0",
                    got = p.x4
                );
            }
        }
        // A ledger entry whose sender is back was either consumed by a fill (removed) or dropped
        // by a plain take or the conductor's drain: either way it is no longer deliverable, which
        // is what makes a later delivery of it an escape.
        for i in 0..self.world.ledger.parked.len() {
            if let Some(e) = self.world.ledger.parked[i] {
                let still = disposition(self, e.sender).is_some_and(|(st, w)| {
                    st == State::Blocked && matches!(w, Some(Wait::Rendezvous(_, WaitRole::Sender)))
                });
                if !still {
                    self.world.ledger.parked[i] = None;
                }
            }
        }
        // A parked SEND_CAP is a live staged delegation: record it from its in-flight report.
        for x in 0..FUZZERS {
            if let Some(e) = self.staged_of(x) {
                let known = self
                    .world
                    .ledger
                    .parked
                    .iter()
                    .flatten()
                    .any(|d| d.sender == e.sender && d.obj == e.obj);
                if !known {
                    for slot in self.world.ledger.parked.iter_mut() {
                        if slot.is_none() {
                            *slot = Some(e);
                            break;
                        }
                    }
                }
            }
        }
        // A spent entry no delivery claimed by now was dropped or went to the conductor: dead.
        self.world.ledger.spent = [None; 8];
    }

    /// Fuzzer `x` is parked sending a delegation: which one, from its report page and last table.
    /// Any endpoint counts: a `SEND_CAP` parks wherever it was sent, the conductor's or a
    /// fuzzer-retyped one alike.
    fn staged_of(&self, x: usize) -> Option<Delegation> {
        let (_, Some(Wait::Rendezvous(_, WaitRole::Sender))) = disposition(self, x)? else {
            return None;
        };
        if report_word(self, x, report::NUMBER) != abi::SYS_INVOKE {
            return None;
        }
        if report_word(self, x, report::ARGS + 1) != proto::SEND_CAPABILITY {
            return None;
        }
        let src = report_word(self, x, report::ARGS + 2) as usize;
        let narrowed = report_word(self, x, report::ARGS + 3) as u32 & Rights::ALL.bits();
        let (obj, rights) = self.fuzzers[x].table.get(src).copied().flatten()?;
        Some(Delegation {
            sender: x,
            obj,
            rights: narrowed & rights,
            badge: badge_of(&obj),
            revoked: false,
        })
    }
}

// -------------------------------------------------------------------------------------------
// The seed.

fn splitmix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The conductor's own rng, for choosing who steps. Not the fuzzers': theirs is the fixture's, and
/// the seed plus role decides every call they make.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        splitmix(self.0)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// The endowment, in the slot order `confined_fuzz_protocol` lays out, plus how many slots of it
/// this fuzzer gets: seven for a fuzzer, one for the witness (GO alone). The unused tail of the
/// witness's array holds its GO capability again, which `run` never sees, because the slice stops first:
/// there is no filler capability to invent.
struct Endowment {
    server: RendezvousId,
    client: RendezvousId,
    shared: RendezvousId,
    note: sched::NotificationId,
    gifts: [u64; 2],
}

fn grants_for(
    x: usize,
    go: RendezvousId,
    own: [u64; 2],
    e: &Endowment,
) -> ([Capability; 7], usize) {
    let go_capability = rendezvous_capability(go, Rights::READ);
    if x == WITNESS {
        return ([go_capability; 7], 1);
    }
    (
        [
            go_capability,
            memory_region_capability_rights(own[x], Rights::WRITE),
            rendezvous_capability_badged(
                e.server,
                Rights::WRITE.union(Rights::GRANT),
                if x == A { BADGE_A } else { BADGE_B },
            ),
            rendezvous_capability(e.client, Rights::READ),
            rendezvous_capability(e.shared, Rights::ALL),
            notification_capability(e.note, Rights::ALL),
            page_frame_capability(
                e.gifts[x],
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
        ],
        7,
    )
}

fn run_seed(image: &'static [u8], seed: u64) {
    let conductor_region =
        crate::memory_region::create(CONDUCTOR_PAGES).expect("no conductor region");
    let own = [
        crate::memory_region::create(OWN_REGION_PAGES).expect("no region for fuzzer A"),
        crate::memory_region::create(OWN_REGION_PAGES).expect("no region for fuzzer B"),
    ];
    let server = sched::create_rendezvous_from(conductor_region).expect("no server endpoint");
    let client = sched::create_rendezvous_from(conductor_region).expect("no client endpoint");
    let shared = sched::create_rendezvous_from(conductor_region).expect("no shared endpoint");
    let note = sched::create_notification_from(conductor_region).expect("no notification");
    let go = core::array::from_fn(|_| {
        sched::create_rendezvous_from(conductor_region).expect("no conductor channel")
    });
    let gifts = core::array::from_fn(|_| {
        crate::memory_region::retype_run(conductor_region, 1)
            .expect("no gift frame")
            .0
    });
    let reports = core::array::from_fn(|_| {
        crate::memory_region::retype_run(conductor_region, 1)
            .expect("no report frame")
            .0
    });

    // The report frames come from the allocator and may hold a earlier seed's words: a stale
    // STEPS would fake a completion. Fourteen words, zeroed by the one thread that reads them.
    for &phys in reports.iter() {
        // SAFETY: a frame this conductor just retyped, before any fuzzer is spawned.
        unsafe {
            let base = crate::arch::mmu::phys_to_virt(phys) as *mut u64;
            for i in 0..report::WORDS {
                base.add(i).write_volatile(0);
            }
        }
    }

    let roles = [proto::ROLE_A, proto::ROLE_B, proto::ROLE_WITNESS];
    let endowment = Endowment {
        server,
        client,
        shared,
        note,
        gifts,
    };
    let mut tids = [0u64; FUZZERS];
    for (x, tid) in tids.iter_mut().enumerate() {
        let (grants, granted) = grants_for(x, go[x], own, &endowment);
        let maps = [Mapping {
            va: proto::REPORT_VA,
            phys: reports[x],
            flags: Flags::user_data(),
        }];
        *tid = sched::spawn(move || {
            run(
                image,
                Spawn {
                    arg0: seed,
                    arg1: roles[x],
                    arg2: 0,
                    grants: &grants[..granted],
                    maps: &maps,
                },
            )
        })
        .expect("no fuzzer thread");
    }

    {
        let mut c = CONDUCTOR.lock();
        c.seed = seed;
        c.step = 0;
        c.relay = 3;
        c.pending = None;
        c.world = World {
            conductor_region,
            own,
            server,
            go,
            gifts,
            reports,
            minted: [0; 16],
            minted_n: 0,
            children: [0; 32],
            children_n: 0,
            ledger: Ledger::default(),
        };
        for x in 0..FUZZERS {
            c.fuzzers[x] = Fuzzer {
                tid: tids[x],
                steps: 0,
                table: [None; SLOTS],
                baseline: [0; 24],
                baseline_n: 0,
            };
        }

        // Everyone parks on its conductor channel before the first step; the baseline (image,
        // stack, report) is what the page oracle admits forever after.
        for x in 0..FUZZERS {
            let parked = wait_for(|| parked_on_go(&c, x));
            assert!(parked, "fuzzer {x} never parked on its conductor channel");
            let root =
                sched::thread_space_root(c.fuzzers[x].tid).expect("a live fuzzer has a space root");
            let mut cursor = 0;
            while let crate::revoke::Listing::Entry(next, va) =
                crate::revoke::list_mapping(root, cursor)
            {
                if c.fuzzers[x].baseline_n < c.fuzzers[x].baseline.len() {
                    let n = c.fuzzers[x].baseline_n;
                    c.fuzzers[x].baseline[n] = va;
                    c.fuzzers[x].baseline_n += 1;
                }
                cursor = next;
            }
            c.fuzzers[x].table = snapshot(&c, x);
            c.fuzzers[x].steps = report_word(&c, x, report::STEPS);
        }
    }

    let mut rng = Rng(splitmix(seed ^ 0x5EED_C0DE));
    for step in 0..STEPS {
        let mut c = CONDUCTOR.lock();
        c.step = step;
        c.quiet();
        let x = if rng.below(12) == 0 {
            WITNESS
        } else {
            rng.below(2) as usize
        };
        c.step_and_settle(x);
    }

    // Wind the seed down: tell every fuzzer to exit, wait for the threads, and give every region
    // back. `reclaim_region`, not `destroy`: the fuzzers pin their own regions with retyped
    // objects and the conductor's with its endpoints, and `destroy` refuses a pinned region
    // silently, which 33 seeds of would exhaust the machine's region table for every test after
    // this one (found by the full suite in CI, 2026-10-06 UTC; a filtered local run never sees
    // it).
    {
        let mut c = CONDUCTOR.lock();
        for x in 0..FUZZERS {
            c.go_send(x, proto::GO_EXIT);
        }
        for (x, f) in c.fuzzers.iter().enumerate() {
            let tid = f.tid;
            assert!(
                wait_for(|| !sched::is_thread_present(tid)),
                "fuzzer {x} did not exit"
            );
        }
    }
    let (children, children_n) = {
        let c = CONDUCTOR.lock();
        let mut kids = [0u64; 32];
        kids[..c.world.children_n].copy_from_slice(&c.world.children[..c.world.children_n]);
        (kids, c.world.children_n)
    };
    // Children before parents, newest first, so a grandchild frees before the child it was carved
    // from; a child the fuzzer already destroyed itself is skipped by its stale name.
    for &r in children[..children_n].iter().rev() {
        if crate::memory_region::region_bounds(r).is_some() {
            assert!(
                wait_for(|| sched::reclaim_region(r).is_ok()),
                "seed {seed:#x}: child region {r:#x} did not reclaim"
            );
        }
    }
    for r in [conductor_region, own[0], own[1]] {
        if crate::memory_region::region_bounds(r).is_some() {
            assert!(
                wait_for(|| sched::reclaim_region(r).is_ok()),
                "seed {seed:#x}: region {r:#x} did not reclaim"
            );
        }
    }
}

/// `NIFE_CONFINED_FUZZ_SEEDS=<first>:<count>` from the build environment, or `None` for the
/// committed list. 752's shape.
fn sweep() -> Option<(u64, u64)> {
    let spec = option_env!("NIFE_CONFINED_FUZZ_SEEDS")?;
    let parse = |s: &str| match s.trim().strip_prefix("0x") {
        Some(h) => u64::from_str_radix(h, 16).ok(),
        None => s.trim().parse().ok(),
    };
    let (first, count) = spec
        .split_once(':')
        .expect("NIFE_CONFINED_FUZZ_SEEDS is <first>:<count>");
    Some((
        parse(first).expect("NIFE_CONFINED_FUZZ_SEEDS: a bad first seed"),
        parse(count).expect("NIFE_CONFINED_FUZZ_SEEDS: a bad count"),
    ))
}

/// **A caller-chosen `LIST` cursor is refused unless this space's own log minted it** (found by
/// this milestone's fuzzer, 2026-10-06 UTC, and fixed the same day). `AddressSpace::LIST`'s
/// cursor is the caller's word, and before the fix the kernel followed it straight into the
/// revocation log's page chain: a confined process naming any kernel-mapped page had it walked as
/// a log page, a data abort at best and a read of the page's contents returned as mapping records
/// at worst. The seed that found it, 0x14, drew the cursor as a random word.
///
/// Falsification: replayable `system_tests/falsifications/user.confined_fuzzer_tests.a_caller_chosen_list_cursor_is_refused.patch`
#[test_case]
fn a_caller_chosen_list_cursor_is_refused() {
    use crate::arch::exceptions::TrapFrame;

    let region = crate::memory_region::create(8).expect("no region");
    let space = user_address_space_create(region).expect("no address space");
    let slot = sched::grant(address_space_capability(space, Rights::ALL))
        .expect("no free slot for the space capability");
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    // A cursor from nowhere: not this space's chain, and page-aligned onto a page the kernel
    // would have walked as a log. The refusal is the whole fix.
    let r = crate::syscall::invoke(
        &mut frame,
        slot,
        abi::address_space::LIST,
        0x1e1d_3b96_8000,
        0,
        0,
    );
    assert_eq!(
        r,
        Err(Error::BadPointer),
        "a LIST cursor this space never minted was followed rather than refused"
    );
    // The walk's own start cursor stays good: an empty space answers DONE.
    let r0 = crate::syscall::invoke(&mut frame, slot, abi::address_space::LIST, 0, 0, 0);
    assert_eq!(
        r0,
        Ok(abi::survey::DONE as i64),
        "the cursor the kernel itself mints (0, the walk's start) must still be taken"
    );
    let _ = delete_current_capability(slot);
    // `user_address_space_create` retyped the root out of this region, which pins it, and
    // `memory_region::destroy` silently refuses a pinned region (`memory_region.rs`, the same
    // trap `run_seed`'s teardown records): a destroy here would leak it on every suite run.
    // Reclaim tears the resident objects down with it, as `pmap_tests::tidy` does.
    sched::reclaim_region(region).expect("reclaim the cursor test's region");
}

/// **A confined fuzzer's random syscalls never move its reach outside its endowment** (milestone
/// 779, part (b), provisional). See the module header for the conductor, the oracles and the
/// services that keep a blocking call from ending the run.
///
/// Falsification: replayable `system_tests/falsifications/user.confined_fuzzer_tests.a_confined_fuzzer_reaches_nothing_it_was_not_granted.patch`
#[test_case]
fn a_confined_fuzzer_reaches_nothing_it_was_not_granted() {
    let Some(image) = program("confined_syscall_fuzzer") else {
        crate::testing::skip!("no confined_syscall_fuzzer program in the archive");
    };
    let hz = crate::arch::timer::frequency();
    let start = crate::arch::timer::now();
    let mut ran = 0u64;
    match sweep() {
        None => {
            for seed in (0..SUITE_SEEDS).chain(CORPUS.iter().copied()) {
                run_seed(image, seed);
                ran += 1;
            }
            crate::println!("    confined fuzzer: seeds 0..{SUITE_SEEDS} and corpus {CORPUS:?}");
        }
        Some((first, count)) => {
            let capability = start + SWEEP_SECS * hz;
            for seed in first..first.saturating_add(count) {
                if crate::arch::timer::now() > capability {
                    break;
                }
                run_seed(image, seed);
                ran += 1;
            }
            crate::println!(
                "    confined fuzzer: sweep of seeds {first}..{} ({ran} of {count} asked; a time \
                 capability of {SWEEP_SECS} s stops it early)",
                first + ran
            );
        }
    }
    let ms = (crate::arch::timer::now() - start) * 1000 / hz;
    crate::println!(
        "    confined fuzzer: {ran} seeds, {ms} ms ({} seeds/s)",
        (ran * 1000).checked_div(ms).unwrap_or(0)
    );
}
