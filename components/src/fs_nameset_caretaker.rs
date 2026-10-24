//! The nameset caretaker: a directory capability attenuated to the names a pattern matched
//! (milestone 47's globbing lane, notes/glob-grant.md).
//!
//! `fs_file_caretaker` narrows a directory capability to one file and `fs_subtree_caretaker` to
//! one subtree; this narrows it to a set of names in one directory, which is what `rm *.txt`
//! designates. A name not in the set does not exist here: every verb that names it is `ENOENT`, at
//! the granted directory and nowhere below it. `start_granted_set` in
//! `kernel/src/user/fs_service/set_grant.rs` wires its slots, passes the directory's name and rights
//! in the three `START` words, and maps the set at [`SET_VA`] ([`filesystem_contract::nameset`]).
//! Why it is a third caretaker is in notes/glob-grant/third-caretaker.md.

//! # BUGS
//!
//! **No host fuzz target reaches this caretaker's decode.** It is inline in this EL0 binary with no
//! library half, so a target would need it extracted first (`notes/fuzzing-the-services.md`).
//!
//! Name: ratified 2026-08-01 (calef, milestone 61), replacing `swarden`. Refused `swarden` and the
//! `warden` family. A reader should predict exactly these names, in one directory.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use filesystem_contract::{dirent, fs, grant, nameset, operation, reply_err, reply_errno, verb};
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{Reply, call, receive_request, send};

/// The FS-service endpoint (slot 0, `WRITE`): the directory capability this process attenuates.
const FS: u64 = 0;
/// The narrowed endpoint (slot 1, `READ`): where the confined program calls. Holding it is holding
/// the set.
const CLIENT: u64 = 1;
/// The report endpoint (slot 2, `WRITE`): readiness, sent once the descent has succeeded.
const REPORT: u64 = 2;

/// The one page, shared with the FS server above and the client below, for `fs_file_caretaker`'s
/// reason: every request on both hops is a blocking `CALL`.
const PAGE_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);
/// The read-only page the encoded name set arrives in. A second frame, and the only place in the
/// grant machinery that needs one: a set is the first thing a grant carries that registers cannot.
/// It is copied into a local at startup and never read again, so nothing that happens to this page
/// afterwards can widen the namespace.
const SET_VA: u64 = address_space_map::pair_page(0x0000_0000_0070_0000);
/// The shared page's size, the contract's transfer unit.
const PAGE: usize = filesystem_contract::PAGE;

// SAFETY: the wiring maps one page read/write at PAGE_VA before this program runs (milestone 139
// round 2; see `user_mode_runtime::mapped_window`, which is what collapsed the hand-rolled read_volatile/
// write_volatile loops below).
const WINDOW: MappedWindow = unsafe { MappedWindow::new(PAGE_VA, PAGE as u64) };
// SAFETY: the wiring maps SET_VA read-only, at least nameset::BYTES bytes, before this program runs.
const SET_WINDOW: MappedWindow = unsafe { MappedWindow::new(SET_VA, nameset::BYTES as u64) };

/// How many handles a confined program may hold open at once, including the granted directory at
/// index 0. `fs_subtree_caretaker`'s number, for `fs_subtree_caretaker`'s reason: no heap, one
/// stack page.
const SLOTS: usize = 16;

/// `EMFILE`: as many handles open through this caretaker as the client may have.
const EMFILE: i32 = 24;
/// `EINVAL`, for the verbs this contract does not carry and for closing the root of a namespace.
const EINVAL: i32 = 22;
/// `EBADF`, for a handle this process never minted.
const EBADF: i32 = 9;
/// `ENOENT`: **in this scope there is no such name.** The only refusal the set itself ever produces.
const ENOENT: i32 = 2;

/// Copy `bytes` into the shared page at `off`.
fn put_at(off: usize, bytes: &[u8]) {
    for (i, &b) in bytes.iter().enumerate() {
        if off + i >= PAGE {
            return;
        }
        WINDOW.w8((off + i) as u64, b);
    }
}

/// Read `out.len()` bytes out of the shared page starting at `off`.
fn get_at(off: usize, out: &mut [u8]) {
    for (i, b) in out.iter_mut().enumerate() {
        *b = WINDOW.r8((off + i) as u64);
    }
}

/// One request to the FS server, forwarded verbatim except for the handle this process substituted.
fn forward(w0: u64, w1: u64) -> i64 {
    call(FS, w0, w1).0 as i64
}

/// Answer the blocked caller through the one-shot Reply the kernel minted.
fn reply(to: Reply, r0: i64, r1: u64) {
    user_mode_runtime::reply(to, r0 as u64, r1);
}

/// The client's handle namespace, `fs_subtree_caretaker`'s table verbatim: `table[i]` is the FS
/// server's handle for the client's handle `i`. Slot 0 is the granted directory and is never freed.
struct Table([Option<u64>; SLOTS]);

impl Table {
    fn get(&self, client: u64) -> Option<u64> {
        self.0.get(client as usize).copied().flatten()
    }

    fn install(&mut self, server: u64) -> Option<u64> {
        let i = self.0.iter().skip(1).position(|s| s.is_none())? + 1;
        self.0[i] = Some(server);
        Some(i as u64)
    }
}

/// The length of the first component of the `len`-byte name at the start of the shared page: up to
/// its first `/`, or all of it. Scanning stops one byte past the longest name a grant carries,
/// because a first component longer than that is in no set and [`name_at`] refuses it anyway.
fn first_component(len: usize) -> usize {
    let mut b = [0u8; 1];
    for i in 0..len.min(grant::MAX_NAME + 1) {
        get_at(i, &mut b);
        if b[0] == b'/' {
            return i;
        }
    }
    len
}

/// Read a name of `len` bytes out of the shared page at `off`.
///
/// A name longer than a grant can carry is truncated to nothing rather than clamped, because the set
/// holds no such name and a clamped name is a *different* name: comparing a prefix against the set
/// is how a filter accidentally admits `gl-one.txtXXX`.
fn name_at(off: usize, len: usize, buf: &mut [u8; grant::MAX_NAME]) -> usize {
    if len == 0 || len > grant::MAX_NAME || off + len > PAGE {
        return 0;
    }
    get_at(off, &mut buf[..len]);
    len
}

/// Serve [`CLIENT`] forever: `fs_subtree_caretaker`'s dispatch, plus one question asked in one
/// place, whether a name in the granted directory `dir` is in `set`.
///
/// Which verbs are asked is [`verb::Verb::takes_name`], a row in `filesystem_contract`'s verb
/// table, so a name-taking verb added to the contract is filtered from the day its row exists. An
/// attribute name ([`verb::Operand::Payload`]) is a payload, not a name in the directory, so the
/// four extended-attribute verbs are never compared against the set. See notes/glob-grant.md.
fn serve(dir: u64, set: &[u8]) -> ! {
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
        let len = fs::req_len(w0).min(PAGE);
        let code = operation(w0);
        let asked = fs::req_handle(w0);
        let Some(server_handle) = table.get(asked) else {
            reply(reply_slot, reply_err(EBADF), 0);
            continue;
        };
        let Some(v) = verb::of(code) else {
            reply(reply_slot, reply_err(EINVAL), 0);
            continue;
        };
        // The filter is at the granted directory and nowhere else: what is under a directory the
        // pattern matched was never a question the pattern asked.
        let filtered = asked == fs::ROOT;

        // A name outside the set is refused before anything is forwarded, so the refusal cannot
        // be told apart from a name not on the disk. `RENAME` needs both halves, below.
        if filtered && v.takes_name() && v.operand == verb::Operand::Name {
            let mut buf = [0u8; grant::MAX_NAME];
            // `OPEN` and `OPENDIR` may carry a relative path (milestone 606 (a directory walk
            // costs what it does on Linux), ruling A). Only its first component is a name in this
            // directory, so only it is checked and written back.
            let first = if code == fs::OPEN || code == fs::OPENDIR {
                first_component(len)
            } else {
                len
            };
            let n = name_at(0, first, &mut buf);
            if !nameset::contains(set, &buf[..n]) {
                reply(reply_slot, reply_err(ENOENT), 0);
                continue;
            }
            // Write the checked name back before forwarding (milestone 43,
            // notes/shared-page-audit.md finding 2): the FS server reads the shared page again,
            // so re-staging makes the bytes it resolves the bytes the filter approved.
            put_at(0, &buf[..n]);
        }

        // `OPEN`'s second reply word is the file's size (milestone 606 (a directory walk costs
        // what it does on Linux), ruling B), passed through untouched; every other verb's is 0.
        let mut size = 0u64;
        let r: i64 = if code == fs::CLOSE {
            if asked == fs::ROOT {
                reply_err(EINVAL)
            } else {
                table.0[asked as usize] = None;
                forward(fs::req(fs::CLOSE, server_handle, 0), 0)
            }
        } else if code == fs::READDIR && filtered {
            // At the granted directory `READDIR` is answered from the set, which is the namespace.
            // It is not gated on the grant's `ENUMERATE`: a listing here reveals exactly the set,
            // which the command line already printed (notes/glob-grant.md).
            list(set, w1 as usize)
        } else if v.operand == verb::Operand::Rename {
            // `RENAME` needs both names in the set: renaming a matched name onto an unmatched one
            // destroys a name this capability was never granted. So a set capability cannot move a
            // name out of the set, and `mv *.txt` is not expressible (notes/glob-grant.md).
            let dst_len = fs::dst_len(w1).min(PAGE);
            let mut src = [0u8; grant::MAX_NAME];
            let mut dst = [0u8; grant::MAX_NAME];
            let sn = name_at(0, len, &mut src);
            let dn = name_at(len, dst_len, &mut dst);
            let dst_handle = table.get(fs::dst_handle(w1));
            let src_ok = !filtered || nameset::contains(set, &src[..sn]);
            let dst_ok = dst_handle != Some(dir) || nameset::contains(set, &dst[..dn]);
            // Re-stage both halves, for the shared-page reason above.
            if src_ok && dst_ok {
                put_at(0, &src[..sn]);
                put_at(len, &dst[..dn]);
            }
            match dst_handle {
                _ if !(src_ok && dst_ok) => reply_err(ENOENT),
                Some(d) => forward(
                    fs::req(code, server_handle, len as u64),
                    fs::rename_dst(d, dst_len as u64),
                ),
                None => reply_err(EBADF),
            }
        } else {
            // Everything else is `fs_subtree_caretaker`'s one forward, driven by the same table: the
            // length travels only for the verbs whose length field counts something, and the second
            // word only for the verbs that read it.
            let n = if v.carries_len() { len as u64 } else { 0 };
            let second = if v.carries_w1 { w1 } else { 0 };
            let (r0, r1) = call(FS, fs::req(code, server_handle, n), second);
            let r = r0 as i64;
            if code == fs::OPEN {
                size = r1;
            }
            if !v.mints_handle || r < 0 {
                r
            } else {
                match table.install(r as u64) {
                    Some(client) => client as i64,
                    None => {
                        forward(fs::req(fs::CLOSE, r as u64, 0), 0);
                        reply_err(EMFILE)
                    }
                }
            }
        };
        reply(reply_slot, r, size);
    }
}

/// Write the set into the shared page as directory entries, from `cursor`, and return the bytes.
///
/// The type each name carries is what the directory said **when the grant was planned**, which is
/// the same resolve-at-grant-time rule the rest of a grant follows. A file that became a directory
/// afterwards is listed as it was designated, and the verb that acts on it answers for what it is.
fn list(set: &[u8], cursor: usize) -> i64 {
    let mut off = 0usize;
    let mut rec = [0u8; dirent::record_len(grant::MAX_NAME)];
    for (name, is_dir) in nameset::iter(set).skip(cursor) {
        let Some(n) = dirent::encode(&mut rec, name, is_dir) else {
            break;
        };
        if off + n > PAGE {
            break;
        }
        put_at(off, &rec[..n]);
        off += n;
    }
    off as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(name_lo: u64, name_hi: u64, spec: u64) -> ! {
    // **Copy the set before anything else runs.** It arrives in a read-only page, and this process
    // then works from a local for the rest of its life, so the namespace it serves cannot be changed
    // by anything that touches that page later. It is 273 bytes on the one stack page `run` maps.
    let mut set = [0u8; nameset::BYTES];
    for (i, b) in set.iter_mut().enumerate() {
        *b = SET_WINDOW.r8(i as u64);
    }

    let mut buf = [0u8; grant::MAX_NAME];
    let n = grant::unpack_name(name_lo, name_hi, grant::spec_len(spec), &mut buf);

    // The descent, exactly `fs_subtree_caretaker`'s: ask for the granted directory with the granted
    // rights, and die rather than come up serving a hole. Everything the client reaches, it reaches
    // through the handle this one request minted. An empty name is a channel the progenitor already
    // bound to the directory, whose root is the granted directory and whose rights the server holds
    // it to, so there is nothing to descend into.
    let dir = if n == 0 {
        fs::ROOT
    } else {
        put_at(0, &buf[..n]);
        let (r0, _) = call(
            FS,
            fs::req(fs::OPENDIR, fs::ROOT, n as u64),
            grant::spec_rights(spec),
        );
        if reply_errno(r0 as i64).is_some() {
            panic!();
        }
        r0
    };
    // A set with nothing in it is not a capability anybody meant to hand out, and the shell refuses
    // to plan one (`grant_plan::Refusal::NoMatch`). If one arrives anyway the wiring is wrong, and dying
    // here is better than serving a namespace with no names in it.
    if nameset::count(&set) == 0 {
        panic!();
    }

    send(REPORT, filesystem_contract::READY, 0, 0);
    serve(dir, &set);
}

user_mode_runtime::panic_handler!();
