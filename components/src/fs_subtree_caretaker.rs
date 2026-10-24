//! The subtree caretaker: a directory capability attenuated to one subtree (milestone 47,
//! notes/dir-capability.md).
//!
//! It narrows a directory capability to the granted directory and everything beneath it, and it
//! performs no checks: one [`fs::OPENDIR`] at startup asks the FS server for a handle carrying the
//! granted rights, and afterwards it only maps the client's handle numbers (from
//! [`filesystem_contract::fs::ROOT`]) to the server's. The handle is the authority and the endpoint
//! is the boundary: the confined program holds nothing that names the FS server. `start_granted_dir`
//! (`kernel/src/user/fs_service/directory_grant.rs`) wires its slots and passes the name and rights
//! in the three `START` words ([`filesystem_contract::grant`]). See notes/dir-capability/caretaker.md.

//! # BUGS
//!
//! **No host fuzz target reaches this caretaker's decode.** It is inline in this EL0 binary with no
//! library half, so a target would need it extracted first (`notes/fuzzing-the-services.md`).
//!
//! Name: ratified 2026-08-01 (calef, milestone 61 (the caretakers)), replacing `dwarden`, named
//! for what it **holds** while both siblings are named for what they **serve**. Refused `dwarden`
//! and the `warden` family. Refused bare `subtree`, which already means the supervision tree, this
//! repository and git's own `subtree`.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use filesystem_contract::{fs, grant, operation, reply_err, reply_errno, verb};
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{Reply, call, exit, receive_request, send};

/// The FS-service endpoint (slot 0, `WRITE`): the directory capability this process attenuates.
const FS: u64 = 0;
/// The narrowed endpoint (slot 1, `READ`): where the confined program calls. Holding it is holding
/// the subtree.
const CLIENT: u64 = 1;
/// The report endpoint (slot 2, `WRITE`): [`filesystem_contract::READY`] once the descent has
/// succeeded, or [`filesystem_contract::DESCENT_REFUSED`] with the errno, after which this process
/// exits without serving.
const REPORT: u64 = 2;

/// The one page, shared with the FS server above and the client below. Sound because every request
/// on both hops is a blocking `CALL`, so the client is parked in its own call while this process
/// uses the page.
const PAGE_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);
/// Its size, the contract's transfer unit.
const PAGE: usize = filesystem_contract::PAGE;

// SAFETY: the wiring maps one page read/write at PAGE_VA before this program runs (milestone 139
// round 2; see `user_mode_runtime::mapped_window`, which is what collapsed the hand-rolled write_volatile
// loop below).
const WINDOW: MappedWindow = unsafe { MappedWindow::new(PAGE_VA, PAGE as u64) };

/// How many handles a confined program may hold open at once through this caretaker, including the
/// granted directory itself at index 0.
///
/// Fixed and small because this process has no heap and runs on the single stack page `run` maps: a
/// growable table would need an allocator and a 4 KiB local would overflow the stack on the first
/// request. Sixteen is well past what the attacker and any `cd`/`ls` sequence needs, and running out
/// is `EMFILE`, which is a real POSIX answer rather than a silently reused slot.
const SLOTS: usize = 16;

/// `EMFILE`: the client has as many handles open through this caretaker as it may.
const EMFILE: i32 = 24;
/// `EINVAL`, for the verbs this contract does not carry and for closing the root of a namespace.
const EINVAL: i32 = 22;
/// `EBADF`, for a handle this process never minted. One refusal site, so a forged handle and a
/// stale one are refused by the same check.
const EBADF: i32 = 9;

/// Copy `bytes` into the shared page.
fn put(bytes: &[u8]) {
    for (i, &b) in bytes.iter().take(PAGE).enumerate() {
        WINDOW.w8(i as u64, b);
    }
}

/// One request to the FS server, forwarded verbatim except for the handle this process substituted.
/// The bytes ride in the page all three parties share, so a forward copies nothing.
fn forward(w0: u64, w1: u64) -> i64 {
    call(FS, w0, w1).0 as i64
}

/// Answer the blocked caller through the one-shot Reply the kernel minted.
fn reply(to: Reply, r0: i64, r1: u64) {
    user_mode_runtime::reply(to, r0 as u64, r1);
}

/// The client's handle namespace: `table[i]` is the FS server's handle for the client's handle `i`,
/// or `None` for a slot the client does not hold. Slot 0 is the granted directory and is never
/// freed, which is what makes [`filesystem_contract::fs::ROOT`] mean "the root of *your* namespace" on this
/// endpoint exactly as it does on the FS server's.
struct Table([Option<u64>; SLOTS]);

impl Table {
    /// The FS server's handle for a client handle, or `None` if the client never got it.
    fn get(&self, client: u64) -> Option<u64> {
        self.0.get(client as usize).copied().flatten()
    }

    /// Record a handle the FS server just minted and return the client's number for it, or `None`
    /// if the client is already holding as many as it may.
    fn install(&mut self, server: u64) -> Option<u64> {
        // Never slot 0: that is the granted directory, and handing it out again would let a CLOSE
        // of the new number take the whole namespace away.
        let i = self.0.iter().skip(1).position(|s| s.is_none())? + 1;
        self.0[i] = Some(server);
        Some(i as u64)
    }
}

/// Serve [`CLIENT`] forever: a namespace translation and nothing else.
///
/// Each request maps the client's handle to the FS server's, forwards the request unchanged, and,
/// for a verb that mints a handle, maps the answer back. There is no rights check here: the rights
/// are on `dir`, the handle the FS server minted at startup, and everything the client reaches it
/// reaches through that handle. [`filesystem_contract::verb`] decides only whether the length field
/// and the second word travel, which is dispatch and cannot refuse anything, so a verb added to the
/// contract is forwarded from the day its row exists. The extended-attribute verbs take a handle, so
/// the server applies the grant's rights to them too. See notes/dir-capability/caretaker.md.
fn serve(dir: u64) -> ! {
    let mut table = Table([None; SLOTS]);
    table.0[0] = Some(dir);

    loop {
        // A CALL-only contract: a plain SEND or a client's SEND_CAP has nobody waiting, so it does
        // no work here, and a delegated capability is deleted rather than answered into (milestone
        // 706 (a CALL server can tell a Reply from a delegation)).
        let req = receive_request(CLIENT);
        let (w0, w1) = (req.w0, req.w1);
        let Some(reply_slot) = req.delivered.into_reply() else {
            continue;
        };
        let len = fs::req_len(w0).min(PAGE) as u64;
        let code = operation(w0);
        let Some(server_handle) = table.get(fs::req_handle(w0)) else {
            reply(reply_slot, reply_err(EBADF), 0);
            continue;
        };
        // An opcode the contract does not carry. One refusal site, and `EINVAL` because a word this
        // contract cannot resolve is a malformed request rather than a capability's refusal.
        let Some(v) = verb::of(code) else {
            reply(reply_slot, reply_err(EINVAL), 0);
            continue;
        };

        // `OPEN`'s second reply word is the file's size (milestone 606 (a directory walk costs
        // what it does on Linux), ruling B), passed through untouched; every other verb's is 0.
        let mut size = 0u64;
        let r: i64 = if code == fs::CLOSE {
            // Closing the granted directory is refused for the same reason the FS server refuses to
            // close its own root: it is not something the client opened, and a client that could
            // close it could make every later request in its own session fail.
            let client = fs::req_handle(w0);
            if client == fs::ROOT {
                reply_err(EINVAL)
            } else {
                table.0[client as usize] = None;
                forward(fs::req(fs::CLOSE, server_handle, 0), 0)
            }
        } else if v.operand == verb::Operand::Rename {
            // The one verb whose second word is not opaque: it names a second directory, so both
            // handles are translated, and the FS server decides whether their rights allow the move.
            match table.get(fs::dst_handle(w1)) {
                Some(dst) => forward(
                    fs::req(code, server_handle, len),
                    fs::rename_dst(dst, fs::dst_len(w1).min(PAGE) as u64),
                ),
                None => reply_err(EBADF),
            }
        } else {
            // `len` travels only for the verbs whose length field counts something and `w1` only
            // for the verbs that read it, which stops a client smuggling a length into a verb that
            // has none. The destructive verbs come through here too, under whatever `REMOVE` the
            // grant carried; nothing here loops, so no request removes more than the client named.
            let n = if v.carries_len() { len } else { 0 };
            let second = if v.carries_w1 { w1 } else { 0 };
            let (r0, r1) = call(FS, fs::req(code, server_handle, n), second);
            let r = r0 as i64;
            if code == fs::OPEN {
                size = r1;
            }
            // A minted handle gets a number of the client's own, so the client never learns the FS
            // server's numbering.
            if !v.mints_handle || r < 0 {
                r
            } else {
                match table.install(r as u64) {
                    Some(client) => client as i64,
                    None => {
                        // Give the handle straight back rather than leaking it in the server: this
                        // process holds the only reference to it and would otherwise pin a node for
                        // the rest of the boot.
                        forward(fs::req(fs::CLOSE, r as u64, 0), 0);
                        reply_err(EMFILE)
                    }
                }
            }
        };
        reply(reply_slot, r, size);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(name_lo: u64, name_hi: u64, spec: u64) -> ! {
    let mut buf = [0u8; grant::MAX_NAME];
    let n = grant::unpack_name(name_lo, name_hi, grant::spec_len(spec), &mut buf);

    // The whole grant, in one request. The FS server intersects the asked rights with its own and
    // refuses if the intersection is smaller, so a wiring that asked for more fails here, loudly.
    put(&buf[..n]);
    let (r0, _) = call(
        FS,
        fs::req(fs::OPENDIR, fs::ROOT, n as u64),
        grant::spec_rights(spec),
    );
    // A refused descent is answered, not trapped: the waiter is the progenitor, which serves every
    // command and has no second thread, so a caretaker that died silently would park the machine in
    // `RECEIVE`, and `rm nosuchdir/x` is ordinary. It then exits: with no narrowed handle, serving
    // would mean answering from `fs::ROOT`, the whole directory it was meant to attenuate.
    if let Some(errno) = reply_errno(r0 as i64) {
        send(
            REPORT,
            filesystem_contract::DESCENT_REFUSED,
            errno as i64 as u64,
            0,
        );
        exit();
    }

    send(REPORT, filesystem_contract::READY, 0, 0);
    serve(r0);
}

user_mode_runtime::panic_handler!();
