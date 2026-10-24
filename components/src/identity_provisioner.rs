//! A provisioning tool: create an identity and its home subtree together (§117 (a principal's
//! subtree is named by its identity string)).
//!
//! Unix's `useradd`, one level down. Spawned once per identity by whoever holds [`PROV`] and
//! [`FS_EP`], it `MKDIR`s the home subtree named by the identity string, then `PUT`s the identity
//! and secret ([`credential_contract::provision::PUT`]); `a0` and `a1` are their lengths. It
//! authenticates no caller and needs not to: holding the two capabilities is the authority. The
//! subtree comes first, so a partial failure leaves an inert, empty subtree rather than a credential
//! with no home, and `MKDIR`'s [`EEXIST`] means proceed. It never seals the store
//! ([`credential_contract::provision::SEAL`]) and never deprovisions. See notes/login.md.

//! Name: ratified 2026-09-13 (calef). `identity` is the word the capability model manipulates, where
//! `user` is overloaded in this tree. Refused `useradd` (imports `user` and promises a typed
//! command), `provisioner` (spent and generic), `identity_creator` (omits the subtree) and
//! `account_provisioner` (a third word for identity).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use filesystem_contract::fs;
use user_mode_runtime::call;
use user_mode_runtime::mapped_window::MappedWindow;

/// The credential service's provision endpoint (slot 0), `WRITE`, before its seal. Never the verify
/// endpoint, so this process cannot authenticate anybody, even by mistake.
const PROV: u64 = 0;
/// The directory capability holding the principal tree (slot 1), `WRITE`, unnarrowed in this slice.
const FS_EP: u64 = 1;
/// The report endpoint (slot 2), `WRITE`: one report, then exit.
const REPORT: u64 = 2;

/// Where the caller stages the identity and secret, in [`credential_contract::place`]'s layout,
/// relayed and never chosen here. Numbered in
/// the `0x...00eN_0000` family `credentialer.rs` and `login.rs` already use, one past `login.rs`'s
/// highest (`CRED_VA`, `0xe3_0000`).
const REQ_VA: u64 = address_space_map::pair_page(0x0000_0000_00e4_0000);
// SAFETY: the wiring maps one page read/write at REQ_VA before this process runs (milestone 139
// round 6).
const REQ_WINDOW: MappedWindow =
    unsafe { MappedWindow::new(REQ_VA, credential_contract::PAGE as u64) };
/// The page shared with the credential service. Must match `components/src/credentialer.rs`'s `PROV_VA`.
const PROV_VA: u64 = address_space_map::pair_page(0x0000_0000_00e0_0000);
// SAFETY: the wiring maps one page read/write at PROV_VA before this process runs, the same
// physical frame `credentialer.rs` maps at its own PROV_VA.
const PROV_WINDOW: MappedWindow =
    unsafe { MappedWindow::new(PROV_VA, credential_contract::PAGE as u64) };
/// The page shared with the file service, for the one `MKDIR` request this process ever sends.
const FS_VA: u64 = address_space_map::pair_page(0x0000_0000_00e5_0000);
// SAFETY: the wiring maps one page read/write at FS_VA before this process runs, shared with the
// file service and with nothing else.
const FS_WINDOW: MappedWindow =
    unsafe { MappedWindow::new(FS_VA, filesystem_contract::PAGE as u64) };

/// **Success**: the subtree exists (freshly made or already there) and the credential was stored.
pub const RPT_OK: u64 = 1;
/// **The staged request was not one this contract allows**: an identity or secret length outside
/// [`credential_contract`]'s bounds. Nothing was sent to either server.
pub const RPT_MALFORMED: u64 = 2;
/// **`MKDIR` failed for a reason other than the name already being there.** Word 1 of the report is
/// the errno. Nothing was written to the credential store.
pub const RPT_SUBTREE_FAILED: u64 = 3;
/// **The subtree exists, but the credential `PUT` was refused.** Word 1 of the report is the raw
/// [`credential_contract`] verdict ([`credential_contract::FULL`], [`credential_contract::MALFORMED`] on a duplicate identity,
/// or [`credential_contract::NO_ENTROPY`]), or the raw kernel word if the provision endpoint could not be
/// reached at all. This is the accepted partial-failure state; see this program's module docs.
pub const RPT_CRED_FAILED: u64 = 4;

/// `EEXIST`: the standard value (matching `redoxfs`/`syscall`'s own, which `redoxfs_server` returns and
/// nothing in `filesystem_contract` re-exports under a name). The one `MKDIR` failure this process does not
/// treat as a refusal.
const EEXIST: i32 = 17;

#[unsafe(no_mangle)]
pub extern "C" fn _start(id_len: u64, secret_len: u64, _a2: u64) -> ! {
    // SAFETY: forwarded from REQ_WINDOW's own contract; staged by the caller with the identity and
    // secret this run provisions.
    let req_page = unsafe { REQ_WINDOW.as_slice() };
    let w0 = credential_contract::req(
        credential_contract::provision::PUT,
        id_len as usize,
        secret_len as usize,
    );
    let Some((identity, secret)) = credential_contract::read(req_page, w0) else {
        wipe_req();
        report(RPT_MALFORMED, 0);
    };

    // The subtree first, so a partial failure is inert rather than a live identity with no home.
    // `Some(EEXIST)` falls through like `None`: the subtree is already there, from this run or an
    // earlier attempt, which is recovery rather than a reason to refuse.
    if let Some(errno) = mkdir_home(identity)
        && errno != EEXIST
    {
        wipe_req();
        report(RPT_SUBTREE_FAILED, errno as i64 as u64);
    }

    // SAFETY: forwarded from PROV_WINDOW's own contract.
    let prov_page = unsafe { PROV_WINDOW.as_mut_slice() };
    let Some(cw0) = credential_contract::place(
        prov_page,
        identity,
        secret,
        credential_contract::provision::PUT,
    ) else {
        // Unreachable in practice: `identity` and `secret` already round-tripped `credential_contract::read`
        // with the same bounds `place` enforces. Refused rather than assumed, because a caller bug
        // here would otherwise be a silent no-op rather than a wrong-looking but honest report.
        wipe_req();
        report(RPT_MALFORMED, 0);
    };
    // `identity` and `secret` were read from REQ_VA and are done being read: `identity` was also
    // used by `mkdir_home` above, and both have now been copied to PROV_VA.
    wipe_req();
    let (cr0, _) = call(PROV, cw0, 0);
    credential_contract::wipe(prov_page);

    match credential_contract::code(cr0) {
        Some(credential_contract::OK) => report(RPT_OK, 0),
        Some(other) => report(RPT_CRED_FAILED, other),
        // A kernel error (no such capability, the endpoint is dead) reads as a huge u64; carried
        // through rather than collapsed, so the report is still diagnosable from one word.
        None => report(RPT_CRED_FAILED, cr0),
    }
}

/// `MKDIR` the identity's home subtree under [`FS_EP`]'s root, asking for full rights on it
/// ([`filesystem_contract::dir::ALL`]), the same rights `login.rs` requests when it descends into the fixed
/// fixture subtree today. `None` on success (the handle this process minted is closed immediately;
/// it has no further use for it), `Some(errno)` otherwise.
fn mkdir_home(identity: &[u8]) -> Option<i32> {
    // SAFETY: forwarded from FS_WINDOW's own contract.
    let page = unsafe { FS_WINDOW.as_mut_slice() };
    let n = identity.len().min(page.len());
    page[..n].copy_from_slice(&identity[..n]);
    let (r0, _) = call(
        FS_EP,
        fs::req(fs::MKDIR, fs::ROOT, identity.len() as u64),
        filesystem_contract::dir::ALL,
    );
    match filesystem_contract::reply_errno(r0 as i64) {
        Some(errno) => Some(errno),
        None => {
            // Best-effort: a failed close leaks a handle-table slot in the file service and
            // corrupts nothing a caller of this tool could observe.
            let _ = call(FS_EP, fs::req(fs::CLOSE, r0, 0), 0);
            None
        }
    }
}

/// Zero the staged request. The caller must not reuse [`REQ_VA`] after this returns; this process
/// is the only one that ever maps it.
fn wipe_req() {
    // SAFETY: forwarded from REQ_WINDOW's own contract, and this process is the only writer.
    let page = unsafe { REQ_WINDOW.as_mut_slice() };
    credential_contract::wipe(page);
}

fn report(code: u64, detail: u64) -> ! {
    user_mode_runtime::send(REPORT, code, detail, 0);
    user_mode_runtime::exit()
}

user_mode_runtime::panic_handler!();
