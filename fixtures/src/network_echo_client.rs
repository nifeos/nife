//! **Reach the network from a booted system's prompt** (milestone 590 (the booted system starts its
//! network stack)).
//!
//! Opens a TCP connection to the QEMU runners' echo peer (`socket_contract::fixture::ECHO_PEER_IP`,
//! port `ECHO_PEER_PORT`), sends [`MSG`], and prints what comes back. The real progenitor spawns it
//! through the real spawn service, holding exactly what its manifest
//! (`grant_plan::Program::NetworkEchoClient`) declares, against the `net_stack` built at boot, so a
//! green `script/swish-check` line for it is the claim about the system a person boots. A fixture
//! and not a tool, because its peer exists only inside a QEMU runner.

//! # EXAMPLES
//!
//! ```text
//! nife> network_echo_client --mem 4
//! echo peer 10.0.2.9:7777 answered: nife-net!
//! ```

//! # What this program holds
//!
//! - slot 0: its output, the sink contract (`crates/byte_sink_contract`).
//! - slot 1: the `--mem` budget, which pays for its socket page (the network grant has no memory).
//! - slot 10 ([`grant_plan::NETWORK_SLOT`]): `WRITE` on the stack's endpoint, placed because the
//!   manifest declares `network`; empty when the progenitor built no stack (`x86_64`, or `NIFE_NET`
//!   unset), and then this prints why and sends nothing.
//! - while it runs, the capability of the socket it opened (from `OPEN_TCP`), which reaches that
//!   socket and no other (§255 (each socket is its own capability)).
//!
//! Name: provisional. Introduced 2026-09-24 for milestone 590; says what it talks to, and expects
//! to be retired rather than renamed once a real network tool can stand in for it.

#![no_std]
#![allow(missing_docs)]
#![no_main]

use abi::rights;
use socket_contract::fixture::{ECHO_PEER_IP, ECHO_PEER_PORT};
use socket_contract::*;
use user_mode_runtime::mapped_window::{MappedWindow, PAGE};
use user_mode_runtime::{
    call, call_receiving, cap_delete, exit, is_granted, map_page_frame, retype_into_page_frame,
    send, send_capability,
};

/// The output slot: the sink contract.
const OUT: u64 = 0;
/// The `--mem` budget.
const MEMORY_REGION: u64 = 1;
/// The stack's endpoint, where the manifest's declaration puts it.
const STACK: u64 = grant_plan::NETWORK_SLOT;

/// Where this program maps the page it shares with the stack. Any address its own image does not
/// use; `socket_test_client`'s choice, because that one is known to be clear of a small program.
const PAGE_FRAME_VA: u64 = address_space_map::pair_page(0x0000_0000_00A0_0000);

// SAFETY: `PAGE_FRAME_VA` is mapped read/write for one page before any access through this window
// (`attach`), and nothing else in this program touches that range.
static WINDOW: MappedWindow = unsafe { MappedWindow::new(PAGE_FRAME_VA, PAGE) };

/// The line sent, and the line that must come back. `socket_test_client`'s own, so the peer sees
/// the same bytes from the prompt that it sees from the harness.
const MSG: &[u8] = b"nife-net!";

user_mode_runtime::program_entry!(start);

fn start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    if !is_granted(STACK) {
        finish(&[
            b"network_echo_client: no network capability was granted; ",
            b"this boot built no network stack\n",
        ]);
    }
    match exchange() {
        Ok(n) => {
            let mut got = [0u8; 64];
            let n = n.min(got.len());
            for (i, b) in got[..n].iter_mut().enumerate() {
                *b = WINDOW.r8(OFF_PAYLOAD + i as u64);
            }
            let mut addr = [0u8; 24];
            let a = render_peer(&mut addr);
            finish(&[b"echo peer ", &addr[..a], b" answered: ", &got[..n], b"\n"]);
        }
        Err(step) => finish(&[b"network_echo_client: the exchange failed at ", step, b"\n"]),
    }
}

/// Mint a page, open a socket, attach the page, connect, send, receive, close. `Ok` carries the received length; `Err`
/// names the step that failed, which is all a person at the prompt can act on.
fn exchange() -> Result<usize, &'static [u8]> {
    let frame = retype_into_page_frame(MEMORY_REGION);
    if frame < 0 {
        return Err(b"minting the shared page (was --mem given?)");
    }
    let frame = frame as u64;
    if !map_page_frame(frame, PAGE_FRAME_VA, true, MEMORY_REGION) {
        return Err(b"mapping the shared page");
    }
    let (REP_OK, _, Some(socket)) = call_receiving(STACK, OPERATION_OPEN_TCP, 0) else {
        return Err(b"opening a socket");
    };
    let result = converse(socket, frame);
    // Closed before the answer is judged, so a failed exchange still gives the socket back.
    let _ = call(socket, OPERATION_CLOSE, 0);
    cap_delete(socket);
    result
}

/// Attach the page to `socket`, connect, send, receive: the length received, or the step that
/// failed.
fn converse(socket: u64, frame: u64) -> Result<usize, &'static [u8]> {
    if send_capability(
        socket,
        frame,
        rights::READ | rights::WRITE,
        OPERATION_ATTACH_PAGE_FRAME,
    ) < 0
    {
        return Err(b"handing the stack the shared page");
    }
    for (i, &b) in ECHO_PEER_IP.iter().enumerate() {
        WINDOW.w8(OFF_DST_IP + i as u64, b);
    }
    WINDOW.w16(OFF_DST_PORT, ECHO_PEER_PORT);
    if call(socket, OPERATION_CONNECT, 0).0 != CONNECT_ESTABLISHED {
        return Err(b"connecting");
    }
    for (i, &b) in MSG.iter().enumerate() {
        WINDOW.w8(OFF_PAYLOAD + i as u64, b);
    }
    if call(socket, OPERATION_SEND, MSG.len() as u64).0 != MSG.len() as u64 {
        return Err(b"sending");
    }
    let (n, _) = call(socket, OPERATION_RECEIVE, 0);
    if n == REP_ERR || n == 0 {
        return Err(b"receiving");
    }
    Ok(n as usize)
}

/// `10.0.2.9:7777`, rendered from the constants rather than spelled, so the line printed is the
/// address that was dialled.
fn render_peer(out: &mut [u8; 24]) -> usize {
    let mut n = 0;
    for (i, &octet) in ECHO_PEER_IP.iter().enumerate() {
        if i > 0 {
            out[n] = b'.';
            n += 1;
        }
        n += decimal(octet as u64, &mut out[n..]);
    }
    out[n] = b':';
    n += 1;
    n + decimal(ECHO_PEER_PORT as u64, &mut out[n..])
}

/// Write `v` in decimal at the front of `out`; the digit count.
fn decimal(mut v: u64, out: &mut [u8]) -> usize {
    let mut digits = [0u8; 20];
    let mut k = 0;
    loop {
        digits[k] = b'0' + (v % 10) as u8;
        k += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    for i in 0..k {
        out[i] = digits[k - 1 - i];
    }
    k
}

/// Print the pieces as one stream and end it, then exit.
fn finish(pieces: &[&[u8]]) -> ! {
    for piece in pieces {
        let mut rest = *piece;
        while !rest.is_empty() {
            let (w0, w1, w2, n) = byte_sink_contract::pack(rest);
            send(OUT, w0, w1, w2);
            rest = &rest[n..];
        }
    }
    send(OUT, byte_sink_contract::eof(), 0, 0);
    exit();
}

user_mode_runtime::panic_handler!();
