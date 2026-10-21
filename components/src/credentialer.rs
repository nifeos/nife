//! **The secrets service** (notes/credentials.md): hold the key, expose the operation, never the
//! key. The one process that holds secrets; everything else holds an endpoint meaning "you may ask
//! this store to compute with a secret", so a client cannot enumerate identities, obtain a salt or
//! a tag, or write a record. Today the one secret is an Argon2id tag and the one operation is
//! `VERIFY`.
//!
//! ```text
//!   the provisioner ──the provision endpoint──►┌────────────┐
//!   (once, at boot)      (PUT, then SEAL)       │  secrets   │──►the entropy service
//!                                               │  service   │   (salts; §44's endpoint)
//!   a client ────────the verify endpoint───────►└────────────┘
//!                  (VERIFY, forever)               the store lives here and nowhere else
//! ```
//!
//! A process blocks on one endpoint at a time, so writing the store is a phase that ends:
//! provision (each [`credential_contract::provision::PUT`] derives a record with an
//! entropy-service salt, [`credential_contract::provision::SEAL`] ends it), then both ends of the
//! provision endpoint are deleted, then serve `VERIFY` forever. A client is not refused write
//! permission; there is no object left through which a write could travel.
//!
//! It never invents a salt (§44 (entropy is a capability)): a `PUT` that cannot get one answers
//! [`credential_contract::NO_ENTROPY`], and a service that cannot draw its decoy record's salt and
//! tag at start-up refuses to start, since a weaker fallback would fail silently.
//!
//! Slots: 0, the provision endpoint (RECEIVE), deleted at the seal; 1, the verify endpoint
//! (RECEIVE); 2, the entropy service (WRITE); 3, an untyped budget for the memory-hard scratch;
//! 4, a readiness endpoint (WRITE). Mapped: the provision page and the verify page, **two frames,
//! never one**, because the provisioner writes plaintext secrets into its page. Owning this service
//! lets an attacker answer logins while they hold it, not carry the passwords away.
//!
//! # BUGS
//!
//! - One verify page means one client at a time (today `login`); a second needs a page per
//!   channel, as `filesystem_contract` has.
//! - Nothing survives a reboot. No rate limit or lockout: a guess costs one Argon2id derivation.
//! - Revocation is per holder, not per secret: the sealed store has no way to remove a record, so
//!   rotating one means reprovisioning.
//!
//! Name: ratified 2026-08-01 (calef), replacing `credential`. Refused `credential` (this service
//! never gives you one). Whether a secrets service still owes a rename is calef's question.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use credential_contract as proto;
use credentialer::{Block, Cost, Store, Verdict};
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{call, cap_delete, exit, receive_request, reply, send};

/// The provision endpoint (slot 0): RECEIVE, and only until the seal.
const PROV: u64 = 0;
/// The verify endpoint (slot 1): RECEIVE, forever.
const VERIFY: u64 = 1;
/// The entropy service's endpoint (slot 2): WRITE. Names no device (DECISIONS §44).
const ENTROPY: u64 = 2;
/// The untyped budget (slot 3): pays for the memory-hard scratch.
const BUDGET: u64 = 3;
/// The readiness endpoint (slot 4): WRITE, one message.
const READY: u64 = 4;

/// The provisioner's page. Plaintext secrets cross it; nothing but the provisioner maps it.
const PROV_VA: u64 = address_space_map::pair_page(0x0000_0000_00e0_0000);
// SAFETY: the wiring maps one page read/write at PROV_VA before this program runs (milestone 139
// round 6).
const PROV_WINDOW: MappedWindow = unsafe { MappedWindow::new(PROV_VA, proto::PAGE as u64) };
/// A client's page. Must match `fixtures/src/credentialer_test_client.rs`.
const VERIFY_VA: u64 = address_space_map::pair_page(0x0000_0000_00e1_0000);
// SAFETY: as PROV_WINDOW's.
const VERIFY_WINDOW: MappedWindow = unsafe { MappedWindow::new(VERIFY_VA, proto::PAGE as u64) };

/// How many secrets the store holds. **Six: three logins and three shares.**
///
/// Three, because calef's existing setup serves three family members with separate passwords
/// (design/roadmap/0056-secrets-and-entropy.md). Doubled by milestone 65, because each of those
/// family members also has a Time Machine share with its own account and password, and a secret
/// here is scoped to a *resource* rather than to a person. Three family members therefore means
/// at least three shares, which is why multi-share is the deliverable rather than a later
/// generalisation: a single-secret store would have been discovered as wrong at the worst moment.
///
/// A store sized to the requirement also makes "the seventh is refused" a thing the tests can
/// show rather than a branch nothing reaches.
const CAPACITY: usize = 6;

/// The heap's virtual cap. It holds one allocation, the Argon2 scratch, plus the slack an
/// allocator needs to place it; the untyped behind it is the real ceiling either way.
const HEAP_MAX: u64 = 6 * 1024 * 1024;

/// The startup report's first word, so a reader of the endpoint knows this is the credential
/// service speaking. ASCII-ish, so a hex dump of a report reads.
pub const RPT_READY: u64 = 0x_c2ed_0000_0000_0001;

/// A start-up failure, in the `0xDEAD_...` shape every driver here uses, with the low byte naming
/// the step so a failure is diagnosable from one word.
const E_ENTROPY: u64 = 0x01;
const E_SCRATCH: u64 = 0x02;

/// **The wire contract and the store must agree about their numbers**, and this is the only place
/// both are in scope to be compared. `cseam` is the cautionary tale rule 7 was written from: a
/// layout deliberately written twice with nothing checking that the two copies agree, whose drift
/// shows up as a component scribbling on the wrong bytes arbitrarily far from the edit. A page
/// offset that disagreed here would put a client's blob where the service reads a proof.
const _: () = assert!(proto::MAX_IDENTITY == credentialer::MAX_IDENTITY);
const _: () = assert!(proto::MAX_SECRET == credentialer::MAX_SECRET);
// There were six until 2026-08-30; four of them were the NTLM path's and went with the SMB
// implementation that was its only consumer (notes/smb.md).

#[global_allocator]
static HEAP: memory_region_heap::MemoryRegionHeap =
    memory_region_heap::MemoryRegionHeap::new();

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    HEAP.init(BUDGET, memory_region_heap::DEFAULT_BASE, HEAP_MAX);

    let cost = Cost::DEFAULT;

    // The decoy first, and the service dies without it. Its salt and tag are what a lookup miss
    // lands on, so that "no such identity" costs one full derivation instead of returning
    // instantly; a decoy of zeros would be a tag an attacker could aim a preimage at, and a
    // constant salt would let one precomputation cover every miss on every machine.
    let mut decoy_salt = [0u8; credentialer::SALT_LEN];
    let mut decoy_tag = [0u8; credentialer::TAG_LEN];
    if !fill(&mut decoy_salt) || !fill(&mut decoy_tag) {
        die(E_ENTROPY);
    }

    // One allocation, at start-up, reused by every derivation. Doing it here rather than per
    // request means a login cannot fail because the heap was fragmented, and it means the 4 MiB
    // this service costs is visible in its budget rather than appearing under load.
    let mut scratch: Vec<Block> = vec![Block::default(); cost.blocks()];
    if scratch.len() < cost.blocks() {
        die(E_SCRATCH);
    }

    let mut store = Store::<CAPACITY>::new(cost, decoy_salt, decoy_tag);
    provision(&mut store, &mut scratch);

    // Phase two begins here, and phase one can never resume: the receive end of the provision
    // endpoint is gone (see `provision`), so even this code could not go back to it.
    send(READY, RPT_READY, store.len() as u64, cost.m_kib() as u64);
    serve(&store, &mut scratch)
}

/// **Phase one.** Write the store, then destroy the ability to write the store.
fn provision(store: &mut Store<CAPACITY>, scratch: &mut [Block]) {
    loop {
        let req = receive_request(PROV);
        let (w0, w1) = (req.w0, req.w1);
        let Some(cap) = req.delivered.into_reply() else {
            // A plain SEND, or a SEND_CAP, on a CALL-only contract: nobody is waiting for an
            // answer, so there is nothing to reply into. Drop it (a delegation's slot is deleted,
            // milestone 706 (a `CALL` server can tell a Reply from a delegation)) rather than
            // replying into a slot that is not a Reply.
            continue;
        };
        match proto::operation(w0) {
            proto::provision::PUT => {
                let verdict = put(store, scratch, w0, w1);
                // Unconditionally, on every path including the malformed one: the page holds a
                // plaintext secret and the provisioner is still mapping it.
                wipe(PROV_WINDOW);
                reply(cap, verdict, proto::NO_DATA);
            }
            proto::provision::SEAL => {
                wipe(PROV_WINDOW);
                reply(cap, proto::OK, proto::NO_DATA);
                // **The seal.** After this the service holds no capability to this endpoint, so
                // there is no code path back into the loop above even if one were written. The
                // provisioner drops its send end for the same reason; between them, nothing in the
                // system can name the object any more.
                cap_delete(PROV);
                return;
            }
            _ => {
                reply(cap, proto::MALFORMED, proto::NO_DATA);
            }
        }
    }
}

/// One `PUT`, with its salt drawn fresh. Split out so the wipe above covers every exit.
///
/// It served `PUT_NTLM` too until 2026-08-30 (notes/smb.md), which is why it takes a second word it
/// no longer reads: `w1` carried the account name and domain lengths of the NTLM variant.
fn put(store: &mut Store<CAPACITY>, scratch: &mut [Block], w0: u64, _w1: u64) -> u64 {
    // SAFETY: forwarded from PROV_WINDOW's own contract.
    let page = unsafe { PROV_WINDOW.as_slice() };
    let Some((identity, secret)) = proto::read(page, w0) else {
        return proto::MALFORMED;
    };
    let mut salt = [0u8; credentialer::SALT_LEN];
    if !fill(&mut salt) {
        // No unpredictable bits, so no record. Answering with a weak salt would be the silent
        // degradation DECISIONS §42 forbids, and it would be invisible: every login would still
        // work, and the store would be one rainbow table wide.
        return proto::NO_ENTROPY;
    }
    match store.put(identity, secret, salt, scratch) {
        Ok(()) => proto::OK,
        Err(credentialer::Error::Full) => proto::FULL,
        Err(_) => proto::MALFORMED,
    }
}

/// **Phase two.** One endpoint, one question, forever.
fn serve(store: &Store<CAPACITY>, scratch: &mut [Block]) -> ! {
    loop {
        let req = receive_request(VERIFY);
        let w0 = req.w0;
        let Some(cap) = req.delivered.into_reply() else {
            continue;
        };
        let verdict = match proto::operation(w0) {
            proto::verify::VERIFY => answer(store, scratch, w0),
            // Every other opcode, including the provisioning ones. A client that tries `PUT` here
            // is not refused by a permission check; it is talking to a loop in which that opcode
            // has no meaning, because the object that gave it meaning no longer exists.
            _ => proto::MALFORMED,
        };
        // On every path: the client wrote a secret into this page, and leaving it there would make
        // the frame a place the secret persists after the answer.
        wipe(VERIFY_WINDOW);
        reply(cap, verdict, proto::NO_DATA);
    }
}

/// One `VERIFY`. The reply is a verdict and nothing else; see `credential_contract`'s module docs on why
/// the reply channel has no room for data.
fn answer(store: &Store<CAPACITY>, scratch: &mut [Block], w0: u64) -> u64 {
    // SAFETY: forwarded from VERIFY_WINDOW's own contract.
    let page = unsafe { VERIFY_WINDOW.as_slice() };
    let Some((identity, presented)) = proto::read(page, w0) else {
        return proto::MALFORMED;
    };
    match store.verify(identity, presented, scratch) {
        Ok(Verdict::Match) => proto::MATCH,
        Ok(Verdict::Mismatch) => proto::MISMATCH,
        // A malformed question is not an authentication outcome. Reporting it as MISMATCH would be
        // safe but would tell a buggy client it had the wrong password, which is the kind of wrong
        // diagnosis that costs somebody an afternoon.
        Err(_) => proto::MALFORMED,
    }
}

/// Zero the request area of the shared page named by `window` (`PROV_WINDOW` or `VERIFY_WINDOW`).
fn wipe(window: MappedWindow) {
    // SAFETY: forwarded from `window`'s own contract; this process is the only writer between a
    // request arriving and its reply going out.
    let page = unsafe { window.as_mut_slice() };
    proto::wipe(page);
}

/// Fill `out` with bytes from the entropy service, `entropy_contract::MAX_BYTES` at a time. `false`
/// when the service could not supply them, which every caller treats as fatal to the request.
///
/// A short reply is a refusal here, not something to pad out. The entropy contract is explicit
/// that a count below what was asked means the device went dry, and half a salt is not a salt.
fn fill(out: &mut [u8]) -> bool {
    let mut done = 0;
    while done < out.len() {
        let want = (out.len() - done).min(entropy_contract::MAX_BYTES as usize);
        let (r0, r1) = call(
            ENTROPY,
            entropy_contract::req(entropy_contract::GET, want as u64),
            0,
        );
        // `delivered` is what makes "no entropy capability" distinguishable from "no entropy"
        // without a probe: a count is 0..=8 and every kernel error is a huge u64.
        let Some(n) = entropy_contract::delivered(r0) else {
            return false;
        };
        if n < want {
            return false;
        }
        done += entropy_contract::take(n, r1, &mut out[done..]);
    }
    true
}

fn die(step: u64) -> ! {
    send(READY, 0xDEAD_0000_0000_0000 | step, 0, 0);
    exit()
}

user_mode_runtime::panic_handler!();
