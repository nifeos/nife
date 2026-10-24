//! The file caretaker: a directory capability attenuated to one file (milestone 31 phase 2,
//! DECISIONS §27, notes/grant-expression.md).
//!
//! It holds the FS service's directory capability ([`FS`]), opens the granted name once at startup,
//! and serves the same [`filesystem_contract::fs`] protocol on [`CLIENT`] with a namespace of
//! exactly one name and a direction it cannot widen. The confined program holds an endpoint to this
//! process and nothing that names the FS server, so the boundary is an address space, not a branch.
//! `start_granted` in `kernel/src/user/fs_service/file_grant.rs` wires its slots and passes the
//! name and direction in the three `START` words ([`filesystem_contract::grant`]). Why it is a
//! process and not a check in the FS server is in notes/grant-expression/per-file-grants.md.

//! Name: ratified 2026-08-01 (calef, milestone 61), replacing `fwarden`. Refused `fwarden` and the
//! `warden` family (a synonym invented for Mark Miller's caretaker, which DECISIONS §31 had cited
//! correctly since milestone 31 while the code said warden). The `fs_` prefix rather than `file_`
//! because `file` is already one of the qualifiers and `file_file_caretaker` is the reductio. A
//! reader should predict what the holder ends up able to do: a file, and it cannot list or create.
//!
//! # BUGS
//!
//! - No host fuzz target reaches this caretaker's decode (proposal #1592 part a, rank 4): it is
//!   inline in this EL0 binary with no library half, so a target needs it extracted first, the way
//!   `redoxfs_server::Server::handle` was. See `notes/fuzzing-the-services.md`.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use filesystem_contract::{fs, grant, operation, reply_err, reply_errno};
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{Reply, call, receive_request, send};

/// The FS-service endpoint (slot 0, `WRITE`): the directory capability this process attenuates,
/// and the whole of what it holds that its client does not.
const FS: u64 = 0;
/// The narrowed endpoint (slot 1, `READ`): where the confined program calls. Holding it is holding
/// the file.
const CLIENT: u64 = 1;
/// The report endpoint (slot 2, `WRITE`): readiness, sent once the granted name is open.
const REPORT: u64 = 2;

/// `EBADF`: a handle this capability never minted, or a verb this contract does not carry. One
/// number and one refusal site, so a forged handle and a forged opcode are refused alike.
const EBADF: i32 = filesystem_contract::verb::file_grant::EBADF;

/// The one page, shared with the FS server above and the client below. One frame for three parties
/// is sound because every request on both hops is a blocking `CALL`: the client is parked inside its
/// call for the whole time this process uses the page, so the two uses cannot interleave.
const PAGE_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);
/// Its size, the contract's transfer unit.
const PAGE: usize = filesystem_contract::PAGE;

// SAFETY: the wiring maps one page read/write at PAGE_VA before this program runs (milestone 139
// round 2; see `user_mode_runtime::mapped_window`, which is what collapsed the hand-rolled read_volatile/
// write_volatile loops below).
const WINDOW: MappedWindow = unsafe { MappedWindow::new(PAGE_VA, PAGE as u64) };

/// Copy `bytes` into the shared page.
fn put(bytes: &[u8]) {
    for (i, &b) in bytes.iter().take(PAGE).enumerate() {
        WINDOW.w8(i as u64, b);
    }
}

/// Read `out.len()` bytes out of the shared page.
fn get(out: &mut [u8]) {
    for (i, b) in out.iter_mut().enumerate() {
        *b = WINDOW.r8(i as u64);
    }
}

/// One request to the FS server, forwarded verbatim. The caretaker does not reinterpret a request
/// it has decided to allow: it passes the words through, so the file behaves exactly as it would
/// through the directory capability. Attenuation is about *which* requests pass, not about changing
/// what they mean.
fn forward(w0: u64, w1: u64) -> i64 {
    call(FS, w0, w1).0 as i64
}

/// Answer the blocked caller through the one-shot Reply the kernel minted.
fn reply(to: Reply, r0: i64) {
    user_mode_runtime::reply(to, r0 as u64, 0);
}

/// Serve [`CLIENT`] forever: the whole of the narrowing, in one place.
///
/// `handle` is the FS server's handle for the granted file, which the client never learns: it sends
/// [`grant::HANDLE`] and this substitutes, so a client guessing handle numbers guesses in a space
/// with one inhabitant. `name` is the granted name and `writable` the grant's direction. Each verb's
/// answer is its row in [`filesystem_contract::verb::file_grant::POLICY`], and a verb with no row
/// fails to compile. A mutating verb ([`filesystem_contract::verb::Verb::mutates`]) under a
/// read-only grant is [`grant::EROFS`] by one rule, and `CREATE` is [`grant::ENOTDIR`] because a
/// file capability is not a directory. See notes/grant-expression/per-file-grants.md.
fn serve(handle: u64, name: &[u8], writable: bool) -> ! {
    use filesystem_contract::verb::file_grant::Policy;
    use filesystem_contract::verb::{self};

    loop {
        // A CALL-only contract: a plain SEND or a client's SEND_CAP has nobody waiting, so it does
        // no work here, and a delegated capability is deleted rather than answered into (milestone
        // 706 (a CALL server can tell a Reply from a delegation)).
        let req = receive_request(CLIENT);
        let (w0, w1) = (req.w0, req.w1);
        let Some(reply_slot) = req.delivered.into_reply() else {
            continue;
        };
        let len = fs::req_len(w0).min(PAGE);
        let asked = fs::req_handle(w0);
        let code = operation(w0);

        // One refusal site: an opcode this contract does not carry gets `EBADF`, as a forged
        // handle does.
        let (Some(v), Some(policy)) = (verb::of(code), verb::file_grant::of(code)) else {
            reply(reply_slot, reply_err(EBADF));
            continue;
        };

        let r: i64 = match policy {
            // Any name but the granted one is ENOENT: in this scope there is no such name. The
            // buffer is `grant::WORDS_NAME` bytes, the granted name's own bound, because this
            // process runs on the one stack page `run` maps, so a length mismatch is refused
            // before any copy.
            Policy::Local if code == fs::OPEN => {
                let mut asked_name = [0u8; grant::WORDS_NAME];
                if len == name.len() {
                    get(&mut asked_name[..len]);
                }
                if len == name.len() && &asked_name[..len] == name {
                    grant::HANDLE as i64
                } else {
                    reply_err(2) // ENOENT
                }
            }
            // CLOSE is answered, not forwarded: this process owns the underlying handle for its
            // whole life, so a client's CLOSE must not make the next request fail.
            Policy::Local if asked == grant::HANDLE => 0,
            // A CLOSE of a handle this capability never minted.
            Policy::Local => reply_err(EBADF),
            // The client's handle is checked and then replaced by this process's, so a request
            // that gets through names the granted file whatever the client meant. Extended
            // attributes ride here too, and `mutates()` refuses changing them read-only.
            Policy::Forward if asked != grant::HANDLE => reply_err(EBADF),
            Policy::Forward if v.mutates() && !writable => reply_err(grant::EROFS),
            Policy::Forward => {
                let n = if v.carries_len() { len as u64 } else { 0 };
                let second = if v.carries_w1 { w1 } else { 0 };
                forward(fs::req(code, handle, n), second)
            }
            // Not offered: the row's errno says which "no", ENOTDIR for every directory verb.
            Policy::Refused(errno) => reply_err(errno),
        };
        reply(reply_slot, r);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(name_lo: u64, name_hi: u64, spec: u64) -> ! {
    let mut buf = [0u8; grant::WORDS_NAME];
    let n = grant::unpack_name(name_lo, name_hi, grant::spec_len(spec), &mut buf);
    let name = &buf[..n];

    // Open the granted name once. If it is not there, die rather than serve a hole, so the
    // failure lands at the grant rather than at the first read.
    put(name);
    let (r0, _) = call(FS, fs::req(fs::OPEN, 0, n as u64), 0);
    if reply_errno(r0 as i64).is_some() {
        panic!();
    }

    send(REPORT, filesystem_contract::READY, 0, 0);
    serve(r0, name, grant::is_writable(spec));
}

user_mode_runtime::panic_handler!();
