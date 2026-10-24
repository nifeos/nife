//! **The name resolver** (§248 (the name resolver is its own confined program)): host names to
//! IPv4 addresses for a client holding a grant to ask, and nobody else (notes/name-resolution.md).
//!
//! A client of `net_stack` and a server to its own clients: the contract is
//! `crates/domain_name_resolution_contract`, and `crates/domain_name_system`'s `Query::accept`
//! decides what is believed. **A name is judged against its badge's granted zone before anything is
//! sent**, so a client granted `nife.test` asking about `example.com` gets [`status::DENIED`] and
//! the network sees nothing. Only badge 0, the spawner's, can grant. **With no entropy it asks
//! nothing** ([`status::NO_ENTROPY`]): a guessable transaction id is a forger's gift. Its slots
//! and windows are the constants below.

//! # BUGS
//!
//! - One request at a time, and `net_stack` serves one too, so an unanswered UDP query stalls every
//!   client of both for three of `net_stack`'s 15-second receive bounds.
//! - No cache: the TTL goes back to the client, which knows how long it needs an address.
//! - IPv4 only, no DNSSEC, no EDNS(0), one name server.
//! - A page attached before its badge was granted is deleted, so clients are spawned after grants.
//! - The UDP path is proved only up to the bytes; the boot test (`name_resolver_tests.rs`) asks over
//!   TCP, because slirp forwards UDP to no process a test owns.
//!
//! Name: ratified 2026-10-17 (calef, a program-naming session), replacing `name_resolver`: he
//! proposed "`domain_name_resolver`" and confirmed it over `domain_name_system_resolver`. Refused
//! `name_resolver` ("name" is ambiguous: `bare::resolve` resolves program names, `grant_plan::nav`
//! paths), `domain_name_system_resolver`, `dns_resolver` (§154 (the acronym test)), `resolver`.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates: one lint
// policy)'s ratchet tracks (§107 (`missing_docs` moves to `workspace.lints.rust`)).
#![allow(missing_docs)]
#![no_main]

use abi::rights;
use domain_name_resolution_contract::{
    self as protocol, Grants, MAX_ADDRESSES, NAME_TEXT_MAX, OFF_ADDRESSES, OFF_NAME, Outcome,
    endowment, network_step, status,
};
use domain_name_system::{Answer, Name, Query, Reject, TcpReply};
use socket_contract as socket;
use user_mode_runtime::mapped_window::{MappedWindow, PAGE};
use user_mode_runtime::{
    Delivered, call, call_receiving, cap_delete, map_page_frame, receive_request, reply,
    retype_into_page_frame, send_capability,
};

/// Slot 0: its own endpoint, where grants and requests arrive (`READ`).
const SERVICE: u64 = 0;
/// Slot 1: `net_stack`'s `Stack`, the socket contract's endpoint and its whole network authority
/// (`WRITE`).
const STACK: u64 = 1;
/// Slot 2: the untyped budget, for the socket page and clients' page tables.
const MEMORY_REGION: u64 = 2;
/// Slot 3: the entropy service (`WRITE`), which draws each query's transaction id.
const ENTROPY: u64 = 3;
/// Slot 4: our own address space (`WRITE`), for `UNMAP` at a forget: `OPERATION_FORGET` unmaps
/// and deletes a client's page.
const OWN_SPACE: u64 = protocol::endowment::OWN_SPACE_SLOT;

/// Where the socket page is mapped. A private pair page: nobody else knows this address.
const SOCKET_VA: u64 = address_space_map::pair_page(0x0000_0000_00A0_0000);
/// Where client window 0 is mapped; window `i` is `i` pages above it. Each badge's page has its
/// own window, and a name is copied out of it before it is parsed.
const CLIENT_VA: u64 = address_space_map::pair_page(0x0000_0000_00B0_0000);
/// The last window, checked against the map at compile time like the first.
const _: u64 = address_space_map::pair_page(CLIENT_VA + (protocol::GRANTS_MAX as u64 - 1) * PAGE);

// SAFETY: `socket_page` maps one page read/write at SOCKET_VA before any accessor runs, and every
// accessor is reached only after it succeeded (`socket_ready`).
const SOCKET: MappedWindow = unsafe { MappedWindow::new(SOCKET_VA, PAGE) };

/// How many UDP queries one resolve sends before it gives up, a fresh id each. Three, the number
/// the network time client and the socket test client settled on for a lossy path. A truncated UDP
/// reply moves the query to TCP; the start words can ask for TCP only.
const ATTEMPTS: u32 = 3;
/// How many datagrams one attempt reads looking for its answer, so a flood of forgeries ends the
/// attempt rather than holding the resolver forever. A datagram from elsewhere, or one
/// `Query::accept` refuses, is dropped without ending the exchange.
const DATAGRAMS_PER_ATTEMPT: u32 = 8;

/// The largest reply over TCP this resolver reads. RFC 7766 allows 65,535 bytes; a host-name lookup
/// with eight addresses is a few hundred, and a reply past this is refused as malformed rather than
/// truncated, because over TCP there is nothing left to fall back to.
const TCP_REPLY_MAX: usize = 2048;

/// Everything too large for a one-page stack, in `.bss`. One thread, so one reference.
struct State {
    grants: Grants,
    /// The slot of the page frame mapped in each window, while one is: what a forget unmaps and
    /// deletes.
    attached: [Option<u64>; protocol::GRANTS_MAX],
    request: [u8; 2 + domain_name_system::UDP_MESSAGE_MAX],
    reply: [u8; TCP_REPLY_MAX],
    chunk: [u8; socket::DATA_MAX],
}

static mut STATE: State = State {
    grants: Grants::new(),
    attached: [None; protocol::GRANTS_MAX],
    request: [0; 2 + domain_name_system::UDP_MESSAGE_MAX],
    reply: [0; TCP_REPLY_MAX],
    chunk: [0; socket::DATA_MAX],
};

/// Where and how to ask, from the start words.
#[derive(Clone, Copy)]
struct Server {
    ip: [u8; 4],
    port: u16,
    tcp_only: bool,
    /// Our socket page's frame, which every socket this resolver opens is handed a copy of, or
    /// `None` if minting it failed.
    frame: Option<u64>,
}

/// Why an exchange produced no answer: a reply was refused, or something else is wrong.
enum Fail {
    Rejected(Reject),
    Status(Outcome),
}

/// The start words ([`domain_name_resolution_contract::endowment`]) name the server and the
/// transport, which the spawner reads from the DHCP lease.
#[unsafe(no_mangle)]
pub extern "C" fn _start(server: u64, port_and_transport: u64, _unused: u64) -> ! {
    // SAFETY: one thread, and `_start` is the only code that names `STATE`, once, here.
    let state = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
    let (port, transport) = endowment::port_and_transport_fields(port_and_transport);
    let server = Server {
        ip: endowment::server_octets(server),
        port,
        tcp_only: transport == endowment::TRANSPORT_TCP,
        frame: socket_page(),
    };
    // A failure here is answered on every request rather than by exiting: a resolver that vanished
    // would leave its clients blocked in `CALL` with nothing to say why.
    let socket_ready = server.frame.is_some();
    loop {
        serve(state, &server, socket_ready);
    }
}

fn serve(state: &mut State, server: &Server, socket_ready: bool) {
    let r = receive_request(SERVICE);
    if r.badge == 0 {
        // The spawner's unbadged capability, and nothing else, can say this. A control `CALL` gets
        // an answer so a spawner that called is not left blocked; a delegation is deleted.
        let control = state.grants.control(r.w0, r.w1);
        if let protocol::Control::Forgotten(window) = control {
            give_up_window(state, window);
        }
        if let Some(to) = r.delivered.into_reply() {
            let refused = matches!(control, protocol::Control::Refused(_));
            let _ = reply(to, refused as u64, 0);
        }
        return;
    }
    // A badge wider than a grant's 32 bits names no grant; refusing it here keeps a truncation from
    // aliasing it onto one that does.
    let badge = u32::try_from(r.badge).unwrap_or(0);
    match protocol::operation(r.w0) {
        protocol::OPERATION_ATTACH_PAGE_FRAME => match r.delivered {
            Delivered::Delegation(frame) => attach_client(state, badge, frame),
            other => {
                if let Some(to) = other.into_reply() {
                    let _ = reply(to, Outcome::of(status::BAD_REQUEST).word(), 0);
                }
            }
        },
        protocol::OPERATION_RESOLVE => {
            let Some(to) = r.delivered.into_reply() else {
                return;
            };
            let (outcome, ttl) = if socket_ready {
                resolve(state, server, badge, r.w1)
            } else {
                (network(network_step::NO_SOCKET_PAGE), 0)
            };
            let _ = reply(to, outcome.word(), ttl);
        }
        _ => {
            if let Some(to) = r.delivered.into_reply() {
                let _ = reply(to, Outcome::of(status::BAD_REQUEST).word(), 0);
            }
        }
    }
}

/// Map a client's page in its grant's window, or delete it: no grant, or a page already attached.
fn attach_client(state: &mut State, badge: u32, frame: u64) {
    let kept = match state.grants.granted(badge) {
        Some(i)
            if state.attached[i].is_none()
                && map_page_frame(frame, window_va(i), true, MEMORY_REGION) =>
        {
            state.attached[i] = Some(frame);
            true
        }
        _ => false,
    };
    if !kept {
        cap_delete(frame);
    }
}

/// **A forgotten client's page, given up**: unmapped from its window and its capability deleted, so
/// the next badge granted this window maps its own page here and this process keeps nothing of the
/// last client's. An `UNMAP` that answers `BadPointer` is a page already gone (the client's region
/// was reclaimed first, and revoking the frame took the mapping with it), which is the same end.
fn give_up_window(state: &mut State, window: usize) {
    if let Some(frame) = state.attached[window].take() {
        // SAFETY: `invoke` traps to the kernel, which checks the slot, the rights and the address.
        let _ = unsafe {
            user_mode_runtime::invoke(
                OWN_SPACE,
                abi::address_space::UNMAP,
                window_va(window),
                0,
                0,
            )
        };
        cap_delete(frame);
    }
}

fn window_va(i: usize) -> u64 {
    CLIENT_VA + i as u64 * PAGE
}

/// One resolve, start to finish: the outcome word and the TTL.
fn resolve(state: &mut State, server: &Server, badge: u32, len: u64) -> (Outcome, u64) {
    let Some(window) = state.grants.granted(badge) else {
        return (Outcome::of(status::DENIED), 0);
    };
    if state.attached[window].is_none() {
        return (Outcome::of(status::NOT_ATTACHED), 0);
    }
    // SAFETY: `attach_client` mapped a page read/write at this window's address before setting
    // `attached`, and `give_up_window` clears `attached` in the same step it unmaps. One thread,
    // so nothing runs between the two.
    let page = unsafe { MappedWindow::new(window_va(window), PAGE) };

    // Copied out before it is read, so what is judged and what is asked are the same bytes.
    let len = len as usize;
    if len == 0 || len > NAME_TEXT_MAX + 1 {
        return (Outcome::of(status::BAD_NAME), 0);
    }
    let mut text = [0u8; NAME_TEXT_MAX + 1];
    for (i, b) in text[..len].iter_mut().enumerate() {
        *b = page.r8((OFF_NAME + i) as u64);
    }
    let Some(name) = core::str::from_utf8(&text[..len])
        .ok()
        .and_then(|t| Name::from_dotted(t).ok())
    else {
        return (Outcome::of(status::BAD_NAME), 0);
    };

    // **The grant, before the network.** Nothing below this line runs for a name the badge may not
    // ask about.
    if state.grants.check(badge, &name).is_err() {
        return (Outcome::of(status::DENIED), 0);
    }

    let answer = match ask(state, server, name) {
        Ok(answer) => answer,
        Err(Fail::Status(outcome)) => return (outcome, 0),
        Err(Fail::Rejected(reject)) => return (refused(reject), 0),
    };
    let addresses = answer.addresses();
    for (k, a) in addresses.iter().enumerate().take(MAX_ADDRESSES) {
        for (j, &b) in a.iter().enumerate() {
            page.w8((OFF_ADDRESSES + 4 * k + j) as u64, b);
        }
    }
    (
        Outcome {
            status: status::OK,
            detail: 0,
            count: addresses.len() as u8,
        },
        answer.ttl() as u64,
    )
}

/// A refused reply as the client sees it: the two refusals a client acts on have their own status,
/// and the rest say which check refused.
fn refused(reject: Reject) -> Outcome {
    match reject {
        Reject::NoSuchName => Outcome::of(status::NO_SUCH_NAME),
        Reject::ServerError(rcode) => Outcome {
            status: status::SERVER_ERROR,
            detail: rcode,
            count: 0,
        },
        other => Outcome {
            status: status::REFUSED,
            detail: protocol::reject_code(other),
            count: 0,
        },
    }
}

fn network(step: u8) -> Outcome {
    Outcome {
        status: status::NETWORK,
        detail: step,
        count: 0,
    }
}

/// Ask the server about `name`: over TCP alone, or over UDP and then TCP when the UDP reply was
/// truncated. A fresh id for each query.
fn ask(state: &mut State, server: &Server, name: Name) -> Result<Answer, Fail> {
    let query = Query::for_name(transaction_id()?, name);
    if server.tcp_only {
        return over_tcp(state, server, &query);
    }
    match over_udp(state, server, &query) {
        Err(Fail::Rejected(Reject::Truncated)) => {
            let query = Query::for_name(transaction_id()?, name);
            over_tcp(state, server, &query)
        }
        other => other,
    }
}

/// Sixteen unguessable bits, or the refusal to ask at all.
fn transaction_id() -> Result<u16, Fail> {
    let no_entropy = Fail::Status(Outcome::of(status::NO_ENTROPY));
    let (r0, r1) = call(ENTROPY, entropy_contract::req(entropy_contract::GET, 2), 0);
    let n = entropy_contract::delivered(r0).ok_or(no_entropy)?;
    if n < 2 {
        return Err(Fail::Status(Outcome::of(status::NO_ENTROPY)));
    }
    let mut bytes = [0u8; 2];
    entropy_contract::take(n, r1, &mut bytes);
    Ok(u16::from_le_bytes(bytes))
}

fn over_udp(state: &mut State, server: &Server, query: &Query) -> Result<Answer, Fail> {
    let socket = open(server, socket::OPERATION_OPEN_UDP)?;
    let result = exchange_udp(state, server, socket, query);
    close(socket);
    result
}

/// A socket of our own, through the stack's front door, with our page attached (§255 (each socket
/// is its own capability)). Its capability, or the step that failed.
fn open(server: &Server, operation: u64) -> Result<u64, Fail> {
    let frame = server
        .frame
        .ok_or(Fail::Status(network(network_step::NO_SOCKET_PAGE)))?;
    let (socket::REP_OK, _, Some(socket)) = call_receiving(STACK, operation, 0) else {
        return Err(Fail::Status(network(network_step::OPEN)));
    };
    if send_capability(
        socket,
        frame,
        rights::READ | rights::WRITE,
        socket::OPERATION_ATTACH_PAGE_FRAME,
    ) < 0
    {
        close(socket);
        return Err(Fail::Status(network(network_step::NO_SOCKET_PAGE)));
    }
    Ok(socket)
}

/// End `socket` and drop our capability to it.
fn close(socket: u64) {
    let _ = call(socket, socket::OPERATION_CLOSE, 0);
    cap_delete(socket);
}

fn exchange_udp(
    state: &mut State,
    server: &Server,
    socket: u64,
    query: &Query,
) -> Result<Answer, Fail> {
    let n = query
        .request(&mut state.request)
        .map_err(|e| Fail::Rejected(Reject::Malformed(e)))?;
    // The last refusal of a reply that was not ours, reported only if nothing better arrives.
    let mut ignored = None;
    for _ in 0..ATTEMPTS {
        set_destination(server);
        write_payload(&state.request[..n]);
        if call(socket, socket::OPERATION_SENDTO, n as u64).0 != socket::REP_OK {
            return Err(Fail::Status(network(network_step::SEND)));
        }
        for _ in 0..DATAGRAMS_PER_ATTEMPT {
            let (got, _) = call(socket, socket::OPERATION_RECEIVE, 0);
            if got == socket::REP_ERR || got == 0 || got > socket::DATA_MAX as u64 {
                break; // nothing came: the next attempt
            }
            // A UDP RECEIVE reply carries the datagram's source in the destination fields
            // (socket_contract's layout). Anything not from the server is not an answer.
            if source() != (server.ip, server.port) {
                continue;
            }
            let got = got as usize;
            read_payload(&mut state.chunk[..got]);
            match query.accept(&state.chunk[..got]) {
                Ok(answer) => return Ok(answer),
                // Not a reply to this query: a forger's, or a late one to an earlier id. Keep
                // listening; the real answer may be behind it.
                Err(
                    reject @ (Reject::IdMismatch
                    | Reject::QuestionMismatch
                    | Reject::NotAResponse
                    | Reject::Malformed(_)),
                ) => ignored = Some(reject),
                Err(reject) => return Err(Fail::Rejected(reject)),
            }
        }
    }
    Err(match ignored {
        Some(reject) => Fail::Rejected(reject),
        None => Fail::Status(Outcome::of(status::NO_ANSWER)),
    })
}

fn over_tcp(state: &mut State, server: &Server, query: &Query) -> Result<Answer, Fail> {
    let socket = open(server, socket::OPERATION_OPEN_TCP)?;
    let result = exchange_tcp(state, server, socket, query);
    close(socket);
    result
}

fn exchange_tcp(
    state: &mut State,
    server: &Server,
    socket: u64,
    query: &Query,
) -> Result<Answer, Fail> {
    set_destination(server);
    if call(socket, socket::OPERATION_CONNECT, 0).0 != socket::CONNECT_ESTABLISHED {
        return Err(Fail::Status(network(network_step::CONNECT)));
    }
    let n = query
        .request_tcp(&mut state.request)
        .map_err(|e| Fail::Rejected(Reject::Malformed(e)))?;
    write_payload(&state.request[..n]);
    if call(socket, socket::OPERATION_SEND, n as u64).0 != n as u64 {
        return Err(Fail::Status(network(network_step::SEND)));
    }
    let mut framed = TcpReply::new(&mut state.reply);
    loop {
        let (got, _) = call(socket, socket::OPERATION_RECEIVE, 0);
        if got == 0 || got > socket::DATA_MAX as u64 {
            // The server closed, or the stack gave up, before the reply was whole.
            return Err(Fail::Status(Outcome::of(status::NO_ANSWER)));
        }
        let got = got as usize;
        read_payload(&mut state.chunk[..got]);
        match framed.feed(&state.chunk[..got]) {
            Ok(true) => break,
            Ok(false) => {}
            Err(e) => return Err(Fail::Rejected(Reject::Malformed(e))),
        }
    }
    let message = framed.message().ok_or(Fail::Rejected(Reject::Malformed(
        domain_name_system::Error::Truncated,
    )))?;
    query.accept(message).map_err(Fail::Rejected)
}

/// Mint the socket page from the budget and map it: its frame's slot, kept so each socket can be
/// handed a copy, or `None` at any step, and the resolver then answers every request with the step
/// that failed.
fn socket_page() -> Option<u64> {
    let frame = u64::try_from(retype_into_page_frame(MEMORY_REGION)).ok()?;
    map_page_frame(frame, SOCKET_VA, true, MEMORY_REGION).then_some(frame)
}

fn set_destination(server: &Server) {
    for (i, &b) in server.ip.iter().enumerate() {
        SOCKET.w8(socket::OFF_DST_IP + i as u64, b);
    }
    SOCKET.w16(socket::OFF_DST_PORT, server.port);
}

/// The source of the datagram a UDP `RECEIVE` just delivered.
fn source() -> ([u8; 4], u16) {
    let mut ip = [0u8; 4];
    for (i, b) in ip.iter_mut().enumerate() {
        *b = SOCKET.r8(socket::OFF_DST_IP + i as u64);
    }
    (ip, SOCKET.r16(socket::OFF_DST_PORT))
}

fn write_payload(bytes: &[u8]) {
    for (i, &b) in bytes.iter().enumerate() {
        SOCKET.w8(socket::OFF_PAYLOAD + i as u64, b);
    }
}

fn read_payload(out: &mut [u8]) {
    for (i, b) in out.iter_mut().enumerate() {
        *b = SOCKET.r8(socket::OFF_PAYLOAD + i as u64);
    }
}

user_mode_runtime::panic_handler!();
