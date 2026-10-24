//! **A client of the name resolver, and nothing more** (milestone 384 (in a capability system the
//! resolver is a grant)).
//!
//! It asks the resolver about every name in `domain_name_resolution_contract::fixture::CASES`, in
//! order, and reports each reply; the kernel's test judges them
//! (`system_tests/src/user/name_resolver_tests.rs`). Then, given a network, it connects to the
//! address the first name resolved to and checks an echo comes back. It has the shape milestone 801
//! (packages over the internet)'s package client will have: a badged capability to a resolver that
//! answers for one zone, a capability to the stack, and no way to look outside the zone.

//! # What it holds
//!
//! | slot | what | rights |
//! |---|---|---|
//! | 0 | its report endpoint | `WRITE` |
//! | 1 | the resolver's endpoint, badged by the spawner | `WRITE` |
//! | 2 | an untyped budget: its resolver page and its socket page | |
//! | 3 | `net_stack`'s `Stack` endpoint, when `arg0` is `fixture::WITH_NETWORK` | `WRITE` |

//! # What it reports
//!
//! One message per case, `(case index, the reply's outcome word, the first address)`, then
//! `(CASES.len(), echo verdict, 0)`. The address is packed big-endian, as
//! `domain_name_resolution_contract::endowment::server` packs one.
//!
//! Name: provisional 2026-10-06 (UTC), milestone 384's lane, on the `*_test_client` pattern
//! (`fs_test_client`, `login_test_client`).

#![no_std]
#![allow(missing_docs)]
#![no_main]

use abi::rights;
use domain_name_resolution_contract::fixture::{
    CASES, ECHO_FAILED, ECHO_NO_ADDRESS, ECHO_OK, ECHO_SKIPPED, WITH_NETWORK,
};
use domain_name_resolution_contract::{
    OFF_ADDRESSES, OFF_NAME, OPERATION_ATTACH_PAGE_FRAME, OPERATION_RESOLVE, Outcome, endowment,
    request, status,
};
use socket_contract as socket;
use user_mode_runtime::mapped_window::{MappedWindow, PAGE};
use user_mode_runtime::{
    call, call_receiving, cap_delete, exit, map_page_frame, retype_into_page_frame, send,
    send_capability,
};

const REPORT: u64 = 0;
const RESOLVER: u64 = 1;
const MEMORY_REGION: u64 = 2;
const STACK: u64 = 3;

const RESOLVER_VA: u64 = address_space_map::pair_page(0x0000_0000_00A0_0000);
const SOCKET_VA: u64 = address_space_map::pair_page(0x0000_0000_00A1_0000);

// SAFETY: `attach` maps each page read/write at its address before the first access through it, and
// the program stops before any access when a map fails.
const RESOLVER_PAGE: MappedWindow = unsafe { MappedWindow::new(RESOLVER_VA, PAGE) };
// SAFETY: as above.
const SOCKET_PAGE: MappedWindow = unsafe { MappedWindow::new(SOCKET_VA, PAGE) };

user_mode_runtime::program_entry!(start);

fn start(mode: u64, _a1: u64, _a2: u64) -> ! {
    // A page the resolver refuses (no grant for this badge) is deleted on its side; this client
    // cannot tell, and does not need to: every resolve then answers DENIED, which is the claim.
    if !attach(RESOLVER, RESOLVER_VA, request(OPERATION_ATTACH_PAGE_FRAME)) {
        send(REPORT, u64::MAX, 0, 0);
        exit();
    }
    let mut first = None;
    for (i, case) in CASES.iter().enumerate() {
        let (word, address) = resolve(case.name.as_bytes());
        if i == 0 && Outcome::from_word(word).is_some_and(|o| o.status == status::OK) {
            first = Some(address);
        }
        send(REPORT, i as u64, word, address);
    }
    let echo = match (mode == WITH_NETWORK, first) {
        (false, _) => ECHO_SKIPPED,
        (true, None) => ECHO_NO_ADDRESS,
        (true, Some(address)) => echo_at(endowment::server_octets(address)),
    };
    send(REPORT, CASES.len() as u64, echo, 0);
    exit();
}

/// One resolve: the reply word and the first address, packed.
fn resolve(name: &[u8]) -> (u64, u64) {
    for (i, &b) in name.iter().enumerate() {
        RESOLVER_PAGE.w8((OFF_NAME + i) as u64, b);
    }
    let (word, _ttl) = call(RESOLVER, request(OPERATION_RESOLVE), name.len() as u64);
    let mut a = [0u8; 4];
    for (j, b) in a.iter_mut().enumerate() {
        *b = RESOLVER_PAGE.r8((OFF_ADDRESSES + j) as u64);
    }
    (word, endowment::server(a))
}

/// Mint a page from the budget, map it at `va`, and delegate it on `slot` with `w0`.
fn attach(slot: u64, va: u64, w0: u64) -> bool {
    let frame = retype_into_page_frame(MEMORY_REGION);
    if frame < 0 || !map_page_frame(frame as u64, va, true, MEMORY_REGION) {
        return false;
    }
    send_capability(slot, frame as u64, rights::READ | rights::WRITE, w0) >= 0
}

/// Connect to `address` on the runners' echo peer port, send, and see the bytes come back.
fn echo_at(address: [u8; 4]) -> u64 {
    // A socket of our own (§255 (each socket is its own capability)): the resolver shares this
    // stack, and before milestone 649 (every client of a network stack shares its socket numbers)
    // the two had to agree by convention on which socket number each used.
    let (socket::REP_OK, _, Some(socket)) = call_receiving(STACK, socket::OPERATION_OPEN_TCP, 0)
    else {
        return ECHO_FAILED;
    };
    let verdict = echo_on(socket, address);
    let _ = call(socket, socket::OPERATION_CLOSE, 0);
    cap_delete(socket);
    verdict
}

fn echo_on(socket: u64, address: [u8; 4]) -> u64 {
    const MSG: &[u8] = b"nife-by-name";
    if !attach(socket, SOCKET_VA, socket::OPERATION_ATTACH_PAGE_FRAME) {
        return ECHO_FAILED;
    }
    for (i, &b) in address.iter().enumerate() {
        SOCKET_PAGE.w8(socket::OFF_DST_IP + i as u64, b);
    }
    SOCKET_PAGE.w16(socket::OFF_DST_PORT, socket::fixture::ECHO_PEER_PORT);
    if call(socket, socket::OPERATION_CONNECT, 0).0 != socket::CONNECT_ESTABLISHED {
        return ECHO_FAILED;
    }
    for (i, &b) in MSG.iter().enumerate() {
        SOCKET_PAGE.w8(socket::OFF_PAYLOAD + i as u64, b);
    }
    if call(socket, socket::OPERATION_SEND, MSG.len() as u64).0 == MSG.len() as u64
        && call(socket, socket::OPERATION_RECEIVE, 0).0 == MSG.len() as u64
        && MSG
            .iter()
            .enumerate()
            .all(|(i, &b)| SOCKET_PAGE.r8(socket::OFF_PAYLOAD + i as u64) == b)
    {
        ECHO_OK
    } else {
        ECHO_FAILED
    }
}

user_mode_runtime::panic_handler!();
