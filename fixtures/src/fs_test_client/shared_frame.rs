//! **The shared-frame witness** (milestone 599 (a frame per filesystem client channel),
//! provisional): the victim and the attacker that hold one file frame between them.
//!
//! Cut out of `fs_test_client.rs` unchanged on 2026-10-12 (UTC) by lane
//! maintainer/files-over-1500, for §282 (when a file is split, the parent becomes an index). What
//! another role uses is `pub(crate)` here.
//!
//! Name: provisional (lane maintainer/files-over-1500, 2026-10-12 UTC), for the module and the
//! file.

use filesystem_contract::fs;
use filesystem_fixture as fixture;
use user_mode_runtime::{call, exit, receive, send};

use crate::channel::{FILE, REPORT, get_page, put_page};

/// The sync endpoint the two shared-frame witness roles hand off over (slot 2 by the wiring in
/// `fs_service::start_shared_frame_witness`). Both hold it `READ|WRITE`, so each can `SEND` and
/// `RECEIVE`; the roles strictly alternate, so the endpoint is never ambiguous about direction.
const SYNC: u64 = 2;
/// The victim's word to the attacker: "my name is staged in the page".
const SHARE_STAGED: u64 = 0x0599_57A6;
/// The attacker's word back to the victim: "I have overwritten the staged name; call now".
const SHARE_OVERWROTE: u64 = 0x0599_09E2;

/// The shared-frame witness's victim (milestone 599, provisional): stages its own name while the
/// attacker, holding the same frame, rewrites the page before the server reads it.
///
/// A two-message handshake forces that ordering. The first report word is
/// [`fixture::SHARED_SUBSTITUTED`] (the body read back is the attacker's file),
/// [`fixture::SHARED_ISOLATED`] (its own, what milestone 599's fix makes true) or
/// [`fixture::SHARED_UNEXPECTED`]. `window` (`a1`) is its channel window: it mints its own badged
/// endpoint with `abi::rendezvous::BADGE` and calls through it, while the attacker's comes
/// pre-badged from the wiring. See notes/fs-server/the-test-client.md.
pub(crate) fn share_victim(window: u64) -> ! {
    let victim = fixture::SHARED_VICTIM_NAME.as_bytes();
    // Mint a badged view of the FS endpoint naming our window, and call through it from here on.
    let r = user_mode_runtime::badge(FILE, window);
    if r < 0 {
        send(REPORT, fixture::SHARED_UNEXPECTED, (-r) as u64, 0);
        exit();
    }
    let ep = r as u64;
    // Stage our own name, then tell the attacker; the SEND blocks until the attacker receives it.
    put_page(victim);
    send(SYNC, SHARE_STAGED, 0, 0);
    // Block until the attacker has written its own window. Only then do we call. Before the fix the
    // attacker shared our frame and this is where the substitution landed; now it cannot reach it.
    let _ = receive(SYNC);
    // The length travels in the register (we choose our own name's length); the name bytes travel
    // in our window, which only we and the server map.
    let (r0, _) = call(ep, fs::req(fs::OPEN, 0, victim.len() as u64), 0);
    if (r0 as i64) < 0 {
        send(REPORT, fixture::SHARED_UNEXPECTED, (-(r0 as i64)) as u64, 0);
        exit();
    }
    let handle = r0;
    let (n, _) = call(ep, fs::req(fs::READ, handle, 8), 0);
    if (n as i64) < 0 {
        send(REPORT, fixture::SHARED_UNEXPECTED, (-(n as i64)) as u64, 0);
        exit();
    }
    let mut head = [0u8; 8];
    get_page(n as usize, &mut head);
    let got = &head[..];
    let verdict = if got == &fixture::SHARED_VICTIM_BODY[..8] {
        fixture::SHARED_ISOLATED
    } else if got == &fixture::SHARED_USURPER_BODY[..8] {
        fixture::SHARED_SUBSTITUTED
    } else {
        fixture::SHARED_UNEXPECTED
    };
    send(REPORT, verdict, 0, 0);
    exit();
}

/// **The shared-frame witness's attacker** (milestone 599, provisional).
///
/// It maps the same file frame the victim and the FS server share, and nothing else that names the
/// filesystem: it never calls the server. It waits for the victim's "staged" word, overwrites the
/// staged name with its own (same-length) name, and signals back. That is the whole of finding 1's
/// "a runnable third party": a second holder of the frame, writing it while the victim is parked in
/// its call.
pub(crate) fn share_attacker() -> ! {
    let _ = receive(SYNC);
    put_page(fixture::SHARED_USURPER_NAME.as_bytes());
    send(SYNC, SHARE_OVERWROTE, 0, 0);
    exit();
}
