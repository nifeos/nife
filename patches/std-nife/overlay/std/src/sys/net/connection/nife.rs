//! `std::net` for nife (milestone 27 phase two): `TcpStream`, `TcpListener` and outbound
//! `UdpSocket` bound to the net_stack socket contract (DECISIONS §25, `crates/socket_contract`).
//!
//! A program given the network holds a `Stack` endpoint and an untyped budget (`pal/nife/rt.rs`).
//! Opening a socket is a `CALL` on that endpoint, answered with **the socket's own capability**
//! (§255 (each socket is its own capability)). The PAL hands it a shared `PageFrame` minted from
//! the untyped (`SEND_CAP`, `OPERATION_ATTACH_PAGE_FRAME`), and every later operation is one
//! blocking `CALL` on the socket with the payload in the frame; bytes never cross a message. Socket
//! ids are local registry indices, and each open socket costs one of the process's 64 slots. See
//! notes/std/net.md.

//! ## `TcpListener` is a held authority
//!
//! `bind` is `OPERATION_LISTEN` and `accept` is `OPERATION_ACCEPT`. `net_stack` is spawned with a
//! **listen grant** (`netproto::listen_grant`, default `NO_LISTEN_GRANT`), an inclusive port range,
//! and refuses `LISTEN` outside it: `LISTEN_DENIED` is `PermissionDenied` (ask whoever spawned you
//! for the port; do not retry), and `LISTEN_IN_USE` is `AddrInUse` (pick another port). A listener
//! holds no frame; `accept` gets the connection as a second capability under a second id
//! (notes/net/the-inbound-half.md). The listener re-arms inside `ACCEPT`, so connections are served
//! one after another, and a second peer arriving while this program is busy gets a RST.

//! ## What is Unsupported, and why
//!
//! - **An accepted connection's peer address.** The `OPERATION_ACCEPT` reply carries none, so
//!   `accept` and `peer_addr()` report `0.0.0.0:0` (notes/net/std-tcp-listener.md).
//! - **Non-blocking mode and timeouts.** The contract is blocking-only with no poll verb:
//!   `set_nonblocking(true)` and `set_*_timeout(Some(..))` refuse; `false` and `None` succeed.
//! - **DNS without a resolver grant** (numeric addresses only; the end of this file), **IPv6**
//!   (net_stack is IPv4-only), and **`peek`, `duplicate`, multicast join/leave** (no verb).
//! - Advisory knobs (`set_nodelay`, `set_ttl`, the IPv6 hop limits, `set_linger`, broadcast,
//!   multicast options) accept and return plausible values and change nothing on the wire.

//! # BUGS
//!
//! - **One operation per socket at a time is assumed, not enforced.** The shared frame is not
//!   locked, a design from before std threads; two threads using one socket at once (a read and a
//!   write through a shared `&TcpStream`) can overwrite each other's bytes in its frame.
//! nothing on the wire.

#![allow(dead_code)]

use crate::cell::UnsafeCell;
use crate::fmt;
use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut};
use crate::net::{Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, SocketAddrV4, ToSocketAddrs};
use crate::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use crate::sys::pal::nife::netproto::*;
use crate::sys::pal::nife::{abi, rt};
use crate::sys::unsupported;
use crate::time::Duration;

/// The slots the loader owes a networked std program (see `pal/nife/rt.rs`).
const STACK: u64 = rt::STACK_SLOT;
const NET_MEMORY_REGION: u64 = rt::NET_MEMORY_REGION_SLOT;

/// Where each socket's shared frame maps in this process. One page per id, from the start of the
/// address-space map's runtime windows (`crates/address_space_map`), below std's three runtime pages
/// at 0x1100_0000, so 4,096 sockets before the two meet. net_stack maps
/// the same frame at its own address; the two are the one shared page the contract grants.
const PAGE_FRAME_BASE: u64 = 0x0000_0000_1000_0000;

fn page_frame_va(id: u64) -> u64 {
    PAGE_FRAME_BASE + id * 0x1000
}

// --- The socket registry ---------------------------------------------------------------------
//
// A small fixed table, one entry per possible socket id, guarded by a spinlock (the same
// discipline as the heap in `sys/alloc/nife`). It tracks which ids are in use, each open socket's capability, each id's page
// (minted once per id and kept, so open/close cycles reuse the page without re-mapping an
// already-mapped VA), a per-TCP-socket residual buffer (bytes net_stack delivered into the frame that a
// short `read` could not take), and each UDP socket's peer/last destination.

/// How many sockets this program holds at once. The PAL's own bound, not the wire's: ids are local.
/// Six, the number the wire allowed before §255, because each id keeps a page minted from the
/// program's frame budget, and the spawners size that budget for six (`system_tests`'
/// `std_service::NETWORK_FRAME_PAGES`).
const SOCKETS: usize = 6;

struct Slot {
    in_use: bool,
    /// This id's page, minted and mapped once and kept across open/close cycles, so a reused id
    /// hands its next socket a copy of the same page rather than mapping an already-mapped VA.
    /// `NO_FRAME` until the first socket at this id needs one.
    frame: u64,
    /// The open socket's capability, while `in_use`.
    socket: u64,
    // TCP: bytes received but not yet handed to the caller (a `read` whose buffer was smaller than
    // the segment net_stack delivered). Served before the next `RECEIVE`, so a stream never drops bytes.
    res_off: usize,
    res_len: usize,
    res: [u8; DATA_MAX],
    // UDP: the connected peer (set by `connect`) and the most recent send destination, used as the
    // reported source of `recv_from` since the contract's RECEIVE does not carry the datagram source.
    peer: Option<(Ipv4Addr, u16)>,
    last_dst: Option<(Ipv4Addr, u16)>,
}

impl Slot {
    const fn new() -> Slot {
        Slot {
            in_use: false,
            frame: NO_FRAME,
            socket: NO_FRAME,
            res_off: 0,
            res_len: 0,
            res: [0; DATA_MAX],
            peer: None,
            last_dst: None,
        }
    }
}

struct Registry {
    locked: AtomicBool,
    slots: UnsafeCell<[Slot; SOCKETS]>,
    // A round-robin cursor for id allocation. Kept from when net_stack derived a socket's local port
    // from the id: rotation still spreads the pages a busy program reuses. See notes/std/net.md.
    next: UnsafeCell<usize>,
}

// SAFETY: all access to `slots` goes through the `locked` spinlock.
unsafe impl Sync for Registry {}

static REG: Registry = Registry {
    locked: AtomicBool::new(false),
    slots: UnsafeCell::new([const { Slot::new() }; SOCKETS]),
    next: UnsafeCell::new(0),
};

struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        REG.locked.store(false, Ordering::Release);
    }
}

fn lock() -> Guard {
    while REG
        .locked
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        crate::hint::spin_loop();
    }
    Guard
}

impl Guard {
    #[allow(clippy::mut_from_ref)]
    fn slots(&mut self) -> &mut [Slot; SOCKETS] {
        // SAFETY: holding the guard means holding the spinlock; access is exclusive.
        unsafe { &mut *REG.slots.get() }
    }
}

/// Claim a free socket id in round-robin order, or `None` if all `SOCKETS` are in use.
fn alloc_id() -> Option<u64> {
    let mut g = lock();
    // SAFETY: the cursor is only touched under the spinlock, like the slots.
    let start = unsafe { *REG.next.get() };
    let slots = g.slots();
    for step in 0..SOCKETS {
        let i = (start + step) % SOCKETS;
        if !slots[i].in_use {
            slots[i].in_use = true;
            slots[i].res_off = 0;
            slots[i].res_len = 0;
            slots[i].peer = None;
            slots[i].last_dst = None;
            // SAFETY: as above; advance so the next open prefers a different id (and port).
            unsafe { *REG.next.get() = (i + 1) % SOCKETS };
            return Some(i as u64);
        }
    }
    None
}

/// A `Slot::frame` with no page yet. No capability slot has this number.
const NO_FRAME: u64 = u64::MAX;

/// Release a socket id. Its page stays, to be handed to the next socket at this id.
fn free_id(id: u64) {
    let mut g = lock();
    let s = &mut g.slots()[id as usize];
    s.in_use = false;
    s.socket = NO_FRAME;
}

/// The capability of the socket open at `id`.
fn socket_of(id: u64) -> u64 {
    lock().slots()[id as usize].socket
}

// --- Shared-frame access ---------------------------------------------------------------------
//
// The frame is not locked: only the running operation on a socket touches it between its own
// `CALL`s, which assumes one operation per socket at a time (see BUGS in the module docs).

fn fw8(va: u64, v: u8) {
    // SAFETY: `va` is inside this socket's mapped, writable shared frame.
    unsafe { core::ptr::write_volatile(va as *mut u8, v) }
}

fn fr8(va: u64) -> u8 {
    // SAFETY: `va` is inside this socket's mapped shared frame.
    unsafe { core::ptr::read_volatile(va as *const u8) }
}

fn set_dst(id: u64, ip: Ipv4Addr, port: u16) {
    let base = page_frame_va(id);
    for (i, b) in ip.octets().iter().enumerate() {
        fw8(base + OFF_DST_IP + i as u64, *b);
    }
    // Port, little-endian, matching the frame header (netproto).
    fw8(base + OFF_DST_PORT, port as u8);
    fw8(base + OFF_DST_PORT + 1, (port >> 8) as u8);
}

fn write_payload(id: u64, buf: &[u8]) {
    let base = page_frame_va(id) + OFF_PAYLOAD;
    for (i, b) in buf.iter().enumerate() {
        fw8(base + i as u64, *b);
    }
}

fn read_payload(id: u64, off: usize, out: &mut [u8]) {
    let base = page_frame_va(id) + OFF_PAYLOAD + off as u64;
    for (i, b) in out.iter_mut().enumerate() {
        *b = fr8(base + i as u64);
    }
}

// --- Errors ----------------------------------------------------------------------------------

fn err_ipv6() -> io::Error {
    io::Error::UNSUPPORTED_PLATFORM
}

/// A CALL whose first reply word, read as `i64`, is negative failed at the syscall layer: an empty
/// `Stack` slot (this program was not given the network) or wrong rights. The honest answer is
/// `Unsupported`, the same as a program with no net grants at all.
fn is_syscall_err(r0: u64) -> bool {
    (r0 as i64) < 0
}

// --- Attaching a shared frame ----------------------------------------------------------------

/// Make sure id `id` has its page: minted from the net untyped and mapped at the id's VA, once, and
/// kept. Returns the page's frame capability.
fn ensure_page(id: u64) -> io::Result<u64> {
    {
        let mut g = lock();
        let frame = g.slots()[id as usize].frame;
        if frame != NO_FRAME {
            return Ok(frame);
        }
    }

    let va = page_frame_va(id);
    // Mint a fresh frame from the net untyped. A negative result means no untyped in slot 3, i.e.
    // this program was not endowed with the network: honestly Unsupported.
    // SAFETY: plain syscall; the kernel validates the slot and the budget.
    let frame = unsafe { rt::invoke(NET_MEMORY_REGION, abi::memory_region::RETYPE, 0, 0, 0) };
    if frame < 0 {
        return Err(io::Error::UNSUPPORTED_PLATFORM);
    }
    let frame = frame as u64;

    // Map it writable at this socket's VA; its page table comes from the same untyped.
    // SAFETY: plain syscall; the frame was just minted and is ours to map.
    if unsafe { rt::invoke(frame, abi::page_frame::MAP, va, 1, NET_MEMORY_REGION) } < 0 {
        return Err(io::const_error!(io::ErrorKind::Other, "mapping the socket frame failed"));
    }
    lock().slots()[id as usize].frame = frame;
    Ok(frame)
}

/// File `socket` as id `id`'s open socket and, if it carries bytes, hand it a copy of the id's page
/// (`SEND_CAP`, `OPERATION_ATTACH_PAGE_FRAME`), narrowed to read/write. On failure the socket is
/// closed and the id released.
fn take_socket(id: u64, socket: u64, carries_bytes: bool) -> io::Result<()> {
    lock().slots()[id as usize].socket = socket;
    if !carries_bytes {
        return Ok(());
    }
    let page = ensure_page(id).and_then(|frame| {
        // SAFETY: plain syscall; the frame carries its budget's GRANT (slot 3's, milestone 824 (a
        // retype mints no right its budget lacks)), narrowed here.
        let sent = unsafe {
            rt::invoke(
                socket,
                abi::rendezvous::SEND_CAP,
                frame,
                abi::rights::READ | abi::rights::WRITE,
                OPERATION_ATTACH_PAGE_FRAME,
            )
        };
        if sent < 0 { Err(io::Error::UNSUPPORTED_PLATFORM) } else { Ok(()) }
    });
    if page.is_err() {
        abandon(id);
    }
    page
}

/// Open a socket of the given kind, returning its id. Fails `Unsupported` if the network was not
/// granted, or `Other` if net_stack refused.
fn open(is_tcp: bool) -> io::Result<u64> {
    let id = alloc_id().ok_or_else(|| {
        io::const_error!(io::ErrorKind::Other, "too many open sockets (contract limit)")
    })?;
    let operation = if is_tcp { OPERATION_OPEN_TCP } else { OPERATION_OPEN_UDP };
    let (r0, _, socket) = rt::call_receiving(STACK, operation, 0);
    if is_syscall_err(r0) {
        free_id(id);
        return Err(io::Error::UNSUPPORTED_PLATFORM);
    }
    let Some(socket) = socket.filter(|_| r0 == REP_OK) else {
        free_id(id);
        return Err(io::const_error!(io::ErrorKind::Other, "the net server refused the open"));
    };
    take_socket(id, socket, true)?;
    Ok(id)
}

/// Close socket `id`: tell net_stack to drop it, drop our capability to it, and release the id
/// (keeping its page). Every copy of the capability reaches nothing after the `CLOSE`.
fn abandon(id: u64) {
    let socket = socket_of(id);
    let _ = rt::call(socket, OPERATION_CLOSE, 0);
    rt::cap_delete(socket);
    free_id(id);
}

fn v4(addr: &SocketAddr) -> io::Result<(Ipv4Addr, u16)> {
    match addr {
        SocketAddr::V4(a) => Ok((*a.ip(), a.port())),
        SocketAddr::V6(_) => Err(err_ipv6()),
    }
}

// --- TcpStream -------------------------------------------------------------------------------

pub struct TcpStream {
    id: u64,
    peer: SocketAddr,
}

impl TcpStream {
    pub fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<TcpStream> {
        let mut last = None;
        for addr in addr.to_socket_addrs()? {
            match TcpStream::connect_one(&addr) {
                Ok(s) => return Ok(s),
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap_or_else(|| {
            io::const_error!(io::ErrorKind::InvalidInput, "no addresses to connect to")
        }))
    }

    fn connect_one(addr: &SocketAddr) -> io::Result<TcpStream> {
        let (ip, port) = v4(addr)?;
        let id = open(true)?;
        set_dst(id, ip, port);
        let (r0, _) = rt::call(socket_of(id), OPERATION_CONNECT, 0);
        if is_syscall_err(r0) {
            abandon(id);
            return Err(io::Error::UNSUPPORTED_PLATFORM);
        }
        match r0 {
            CONNECT_ESTABLISHED => Ok(TcpStream { id, peer: *addr }),
            CONNECT_REFUSED => {
                abandon(id);
                Err(io::const_error!(io::ErrorKind::ConnectionRefused, "connection refused"))
            }
            _ => {
                abandon(id);
                Err(io::const_error!(io::ErrorKind::Other, "the net server could not connect"))
            }
        }
    }

    pub fn connect_timeout(_: &SocketAddr, _: Duration) -> io::Result<TcpStream> {
        // The contract is blocking-only; a bounded connect is not expressible. Honest refusal.
        unsupported()
    }

    pub fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        match t {
            None => Ok(()),
            Some(_) => unsupported(),
        }
    }

    pub fn set_write_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        match t {
            None => Ok(()),
            Some(_) => unsupported(),
        }
    }

    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        Ok(None)
    }

    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        Ok(None)
    }

    pub fn peek(&self, _: &mut [u8]) -> io::Result<usize> {
        unsupported()
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        // Serve any residual from a previous short read first, so a stream never drops bytes.
        {
            let mut g = lock();
            let s = &mut g.slots()[self.id as usize];
            if s.res_len > 0 {
                let n = s.res_len.min(buf.len());
                buf[..n].copy_from_slice(&s.res[s.res_off..s.res_off + n]);
                s.res_off += n;
                s.res_len -= n;
                return Ok(n);
            }
        }

        let (r0, _) = rt::call(socket_of(self.id), OPERATION_RECEIVE, 0);
        if is_syscall_err(r0) {
            return Err(io::Error::UNSUPPORTED_PLATFORM);
        }
        if r0 == REP_ERR {
            return Err(io::const_error!(
                io::ErrorKind::TimedOut,
                "the net server timed out waiting for data"
            ));
        }
        let total = (r0 as usize).min(DATA_MAX);
        if total == 0 {
            return Ok(0); // peer closed: end of stream
        }
        let take = total.min(buf.len());
        read_payload(self.id, 0, &mut buf[..take]);
        if total > take {
            // Stash the rest of the segment for the next read.
            let mut g = lock();
            let s = &mut g.slots()[self.id as usize];
            let extra = total - take;
            read_payload(self.id, take, &mut s.res[..extra]);
            s.res_off = 0;
            s.res_len = extra;
        }
        Ok(take)
    }

    pub fn read_buf(&self, mut cursor: BorrowedCursor<'_, u8>) -> io::Result<()> {
        let mut tmp = [0u8; 512];
        let want = cursor.capacity().min(tmp.len());
        if want == 0 {
            return Ok(());
        }
        let n = self.read(&mut tmp[..want])?;
        cursor.append(&tmp[..n]);
        Ok(())
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        for b in bufs {
            if !b.is_empty() {
                return self.read(b);
            }
        }
        Ok(0)
    }

    pub fn is_read_vectored(&self) -> bool {
        false
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let chunk = buf.len().min(DATA_MAX);
        write_payload(self.id, &buf[..chunk]);
        let (r0, _) = rt::call(socket_of(self.id), OPERATION_SEND, chunk as u64);
        if is_syscall_err(r0) {
            return Err(io::Error::UNSUPPORTED_PLATFORM);
        }
        if r0 == REP_ERR {
            return Err(io::const_error!(io::ErrorKind::Other, "the net server rejected the send"));
        }
        Ok((r0 as usize).min(chunk))
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        for b in bufs {
            if !b.is_empty() {
                return self.write(b);
            }
        }
        Ok(0)
    }

    pub fn is_write_vectored(&self) -> bool {
        false
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.peer)
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        // The local address (a DHCP-assigned IP, an ephemeral port net_stack picks) is not reported
        // back across the contract. Honestly unsupported rather than fabricated.
        unsupported()
    }

    pub fn shutdown(&self, _: Shutdown) -> io::Result<()> {
        // Teardown happens once, on Drop (`OPERATION_CLOSE`); a half-shutdown verb is not in the contract.
        Ok(())
    }

    pub fn duplicate(&self) -> io::Result<TcpStream> {
        unsupported()
    }

    pub fn set_linger(&self, _: Option<Duration>) -> io::Result<()> {
        Ok(())
    }

    pub fn linger(&self) -> io::Result<Option<Duration>> {
        Ok(None)
    }

    pub fn set_nodelay(&self, _: bool) -> io::Result<()> {
        Ok(())
    }

    pub fn nodelay(&self) -> io::Result<bool> {
        Ok(false)
    }

    pub fn set_keepalive(&self, _: bool) -> io::Result<()> {
        Ok(())
    }

    pub fn keepalive(&self) -> io::Result<bool> {
        Ok(false)
    }

    pub fn set_ttl(&self, _: u32) -> io::Result<()> {
        Ok(())
    }

    pub fn ttl(&self) -> io::Result<u32> {
        Ok(64)
    }

    pub fn set_hop_limit_v6(&self, _: u8) -> io::Result<()> {
        Ok(())
    }

    pub fn hop_limit_v6(&self) -> io::Result<u8> {
        Ok(64)
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        Ok(None)
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        if nonblocking { unsupported() } else { Ok(()) }
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        abandon(self.id);
    }
}

impl fmt::Debug for TcpStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpStream").field("socket", &self.id).field("peer", &self.peer).finish()
    }
}

// --- TcpListener: the inbound half (milestone 64, on milestone 107's contract) ----------------

/// A **listening port**, which is an authority this program was granted and not a call that
/// happened to succeed. See this module's header for the three answers `bind` can get and what a
/// caller should do about each.
///
/// It holds a socket id, which names its capability in this PAL's registry, and nothing else: no
/// shared frame is ever attached to a listener, because
/// no bytes cross on it (DECISIONS §25's decision that the frame is the granted resource; a
/// listener has nothing to grant). `port` is kept so `socket_addr` can answer without a contract
/// round trip, which the contract has no verb for anyway.
pub struct TcpListener {
    id: u64,
    port: u16,
}

impl TcpListener {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<TcpListener> {
        let mut last = None;
        for addr in addr.to_socket_addrs()? {
            match TcpListener::bind_one(&addr) {
                Ok(l) => return Ok(l),
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap_or_else(|| {
            io::const_error!(io::ErrorKind::InvalidInput, "no addresses to bind to")
        }))
    }

    /// `OPERATION_LISTEN` on a freshly claimed socket id.
    ///
    /// **The bound address is ignored and the port is not**, which is the honest reading of what
    /// this stack can do: `net_stack` holds exactly one interface with one DHCP-assigned address,
    /// so there is no second address a listener could be narrowed to and `0.0.0.0:P` is what every
    /// bind here means. The port, by contrast, is the whole authority, so it is sent as asked and
    /// refused as asked.
    ///
    /// **No frame is attached**: a listener carries no bytes. The page arrives at `accept` time, on
    /// the *connection's* id.
    fn bind_one(addr: &SocketAddr) -> io::Result<TcpListener> {
        let (_ip, port) = v4(addr)?;
        let id = alloc_id().ok_or_else(|| {
            io::const_error!(io::ErrorKind::Other, "too many open sockets (contract limit)")
        })?;
        let (r0, _, socket) = rt::call_receiving(STACK, OPERATION_LISTEN, port as u64);
        if is_syscall_err(r0) {
            free_id(id);
            return Err(io::Error::UNSUPPORTED_PLATFORM);
        }
        match (r0, socket) {
            (LISTEN_GRANTED, Some(socket)) => {
                take_socket(id, socket, false)?;
                Ok(TcpListener { id, port })
            }
            (LISTEN_DENIED, _) => {
                free_id(id);
                // The capability answer. Not `AddrInUse` and not a retry: this stack was never
                // granted this port, and no other port will help unless it was granted too.
                Err(io::const_error!(
                    io::ErrorKind::PermissionDenied,
                    "this program's net server was not granted that listening port"
                ))
            }
            (LISTEN_IN_USE, _) => {
                free_id(id);
                Err(io::const_error!(
                    io::ErrorKind::AddrInUse,
                    "another socket already listens on that port"
                ))
            }
            _ => {
                free_id(id);
                Err(io::const_error!(
                    io::ErrorKind::Other,
                    "the net server refused the listen"
                ))
            }
        }
    }

    /// The address this listener is bound to: `0.0.0.0` and the port that was granted.
    ///
    /// The port half is a fact this PAL knows (it asked for it and was granted it); the address
    /// half is not fabricated but *true*, in the same sense a POSIX bind to `INADDR_ANY` reports
    /// it: the listener is not narrowed to any of the interface's addresses, because the contract
    /// offers no way to narrow it. `TcpStream::socket_addr` stays `Unsupported` for the opposite
    /// reason: there the local endpoint is an ephemeral port `net_stack` picked and never reported.
    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        Ok(SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::UNSPECIFIED,
            self.port,
        )))
    }

    /// **`OPERATION_ACCEPT`: block until a peer connects, and take the connection at a second socket id.**
    ///
    /// A listener and a connection are two objects: net_stack answers `ACCEPT` with the connection's
    /// own capability, and `accept` files it under a *new* id with that id's page, which the
    /// listener never had. The listener re-arms inside the same call, so accepting in a loop works.
    /// **The peer address is `0.0.0.0:0`**: the contract's reply carries no peer (module docs).
    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let conn = alloc_id().ok_or_else(|| {
            io::const_error!(io::ErrorKind::Other, "too many open sockets (contract limit)")
        })?;
        let (r0, _, socket) = rt::call_receiving(socket_of(self.id), OPERATION_ACCEPT, 0);
        if is_syscall_err(r0) {
            free_id(conn);
            return Err(io::Error::UNSUPPORTED_PLATFORM);
        }
        let Some(socket) = socket.filter(|_| r0 == REP_OK) else {
            free_id(conn);
            // Nobody connected inside the server's bounded wait, or the handshake was aborted. The
            // listener is still armed either way, so this is a retryable "try again", not a fault.
            return Err(io::const_error!(
                io::ErrorKind::WouldBlock,
                "no connection arrived before the net server's bounded wait expired"
            ));
        };
        // The connection carries bytes, so it is handed the page the listener never had.
        take_socket(conn, socket, true)?;
        let peer = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
        Ok((TcpStream { id: conn, peer }, peer))
    }

    pub fn duplicate(&self) -> io::Result<TcpListener> {
        // A second holder of one listening port is a second holder of the authority, and the
        // contract has no verb that would let `net_stack` know about it. Refuse rather than mint.
        unsupported()
    }

    pub fn set_ttl(&self, _: u32) -> io::Result<()> {
        Ok(())
    }

    pub fn ttl(&self) -> io::Result<u32> {
        Ok(64)
    }

    pub fn set_hop_limit_v6(&self, _: u8) -> io::Result<()> {
        Ok(())
    }

    pub fn hop_limit_v6(&self) -> io::Result<u8> {
        Ok(64)
    }

    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        // net_stack is IPv4-only, so "v6 only" is the one setting that cannot be honoured and
        // "v4 as well" is already true. Accepting `false` and refusing `true` says both.
        if only_v6 { unsupported() } else { Ok(()) }
    }

    pub fn only_v6(&self) -> io::Result<bool> {
        Ok(false)
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        Ok(None)
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        if nonblocking { unsupported() } else { Ok(()) }
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        // `OPERATION_CLOSE` on the listener id: `net_stack` drops the parked smoltcp socket and the port
        // becomes bindable again. The grant is untouched, because the grant was never the
        // listener's to hold.
        abandon(self.id);
    }
}

impl fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpListener")
            .field("socket", &self.id)
            .field("port", &self.port)
            .finish()
    }
}

// --- UdpSocket (outbound) --------------------------------------------------------------------

pub struct UdpSocket {
    id: u64,
}

impl UdpSocket {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<UdpSocket> {
        // The requested local address is validated (must be a numeric V4) but not honored: net_stack
        // binds an ephemeral local port per socket. A program that wants a specific local port is
        // not served by this phase of the contract.
        let mut chosen = None;
        for a in addr.to_socket_addrs()? {
            if v4(&a).is_ok() {
                chosen = Some(a);
                break;
            }
        }
        if chosen.is_none() {
            return Err(err_ipv6());
        }
        let id = open(false)?;
        Ok(UdpSocket { id })
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        let mut g = lock();
        match g.slots()[self.id as usize].peer {
            Some((ip, port)) => Ok(SocketAddr::V4(SocketAddrV4::new(ip, port))),
            None => Err(io::const_error!(io::ErrorKind::NotConnected, "the socket is not connected")),
        }
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        unsupported()
    }

    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        let n = self.recv(buf)?;
        // The contract's RECEIVE does not report the datagram source, so recv_from names the peer set
        // by `connect`, else the most recent send destination (correct for request/response, the
        // pattern the resolver-less demo uses). Recorded in notes/std.md.
        let src = {
            let mut g = lock();
            let s = &g.slots()[self.id as usize];
            s.peer.or(s.last_dst)
        };
        let addr = match src {
            Some((ip, port)) => SocketAddr::V4(SocketAddrV4::new(ip, port)),
            None => SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)),
        };
        Ok((n, addr))
    }

    pub fn peek_from(&self, _: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        unsupported()
    }

    pub fn send_to(&self, buf: &[u8], addr: &SocketAddr) -> io::Result<usize> {
        let (ip, port) = v4(addr)?;
        if buf.len() > DATA_MAX {
            return Err(io::const_error!(
                io::ErrorKind::InvalidInput,
                "datagram larger than the shared frame"
            ));
        }
        set_dst(self.id, ip, port);
        write_payload(self.id, buf);
        {
            let mut g = lock();
            g.slots()[self.id as usize].last_dst = Some((ip, port));
        }
        let (r0, _) = rt::call(socket_of(self.id), OPERATION_SENDTO, buf.len() as u64);
        if is_syscall_err(r0) {
            return Err(io::Error::UNSUPPORTED_PLATFORM);
        }
        if r0 != REP_OK {
            return Err(io::const_error!(io::ErrorKind::Other, "the net server rejected the send"));
        }
        Ok(buf.len())
    }

    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        let peer = {
            let mut g = lock();
            g.slots()[self.id as usize].peer
        };
        match peer {
            Some((ip, port)) => self.send_to(buf, &SocketAddr::V4(SocketAddrV4::new(ip, port))),
            None => {
                Err(io::const_error!(io::ErrorKind::NotConnected, "the socket is not connected"))
            }
        }
    }

    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        let (r0, _) = rt::call(socket_of(self.id), OPERATION_RECEIVE, 0);
        if is_syscall_err(r0) {
            return Err(io::Error::UNSUPPORTED_PLATFORM);
        }
        if r0 == REP_ERR {
            return Err(io::const_error!(
                io::ErrorKind::TimedOut,
                "the net server timed out waiting for a datagram"
            ));
        }
        let total = (r0 as usize).min(DATA_MAX);
        // A datagram is message-oriented: a short buffer truncates it, standard UDP semantics.
        let n = total.min(buf.len());
        read_payload(self.id, 0, &mut buf[..n]);
        Ok(n)
    }

    pub fn peek(&self, _: &mut [u8]) -> io::Result<usize> {
        unsupported()
    }

    pub fn connect<A: ToSocketAddrs>(&self, addr: A) -> io::Result<()> {
        // A UDP "connect" only fixes a default peer; there is no handshake and no contract call.
        for a in addr.to_socket_addrs()? {
            if let Ok((ip, port)) = v4(&a) {
                lock().slots()[self.id as usize].peer = Some((ip, port));
                return Ok(());
            }
        }
        Err(err_ipv6())
    }

    pub fn duplicate(&self) -> io::Result<UdpSocket> {
        unsupported()
    }

    pub fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        match t {
            None => Ok(()),
            Some(_) => unsupported(),
        }
    }

    pub fn set_write_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        match t {
            None => Ok(()),
            Some(_) => unsupported(),
        }
    }

    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        Ok(None)
    }

    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        Ok(None)
    }

    pub fn set_broadcast(&self, _: bool) -> io::Result<()> {
        Ok(())
    }

    pub fn broadcast(&self) -> io::Result<bool> {
        Ok(false)
    }

    pub fn set_multicast_loop_v4(&self, _: bool) -> io::Result<()> {
        Ok(())
    }

    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        Ok(false)
    }

    pub fn set_multicast_ttl_v4(&self, _: u32) -> io::Result<()> {
        Ok(())
    }

    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        Ok(1)
    }

    pub fn set_multicast_loop_v6(&self, _: bool) -> io::Result<()> {
        Ok(())
    }

    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        Ok(false)
    }

    pub fn set_multicast_hop_limit_v6(&self, _: u8) -> io::Result<()> {
        Ok(())
    }

    pub fn multicast_hop_limit_v6(&self) -> io::Result<u8> {
        Ok(1)
    }

    pub fn join_multicast_v4(&self, _: &Ipv4Addr, _: &Ipv4Addr) -> io::Result<()> {
        unsupported()
    }

    pub fn join_multicast_v6(&self, _: &Ipv6Addr, _: u32) -> io::Result<()> {
        unsupported()
    }

    pub fn leave_multicast_v4(&self, _: &Ipv4Addr, _: &Ipv4Addr) -> io::Result<()> {
        unsupported()
    }

    pub fn leave_multicast_v6(&self, _: &Ipv6Addr, _: u32) -> io::Result<()> {
        unsupported()
    }

    pub fn set_ttl(&self, _: u32) -> io::Result<()> {
        Ok(())
    }

    pub fn ttl(&self) -> io::Result<u32> {
        Ok(64)
    }

    pub fn set_hop_limit_v6(&self, _: u8) -> io::Result<()> {
        Ok(())
    }

    pub fn hop_limit_v6(&self) -> io::Result<u8> {
        Ok(64)
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        Ok(None)
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        if nonblocking { unsupported() } else { Ok(()) }
    }
}

impl Drop for UdpSocket {
    fn drop(&mut self) {
        abandon(self.id);
    }
}

impl fmt::Debug for UdpSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UdpSocket").field("socket", &self.id).finish()
    }
}

// --- DNS: the resolver this program was granted, if it was granted one -------------------------
// Milestone 801 (packages over the internet). A name goes to the resolver of milestone 384 (in a
// capability system the resolver is a grant) through the badged endpoints at `rt::RESOLVER_SLOTS`,
// one granted zone per badge (§252 (a resolver grant is one zone per client badge)). With slot 9
// empty nothing resolves, but numeric addresses work: std parses those before calling here. Badges
// are asked in slot order, the next only when the resolver says the name is outside this one's
// zone (milestone 892 (a std program at the prompt holds the network and a resolver); calef,
// 2026-10-19 (UTC): "probed in order"), a refusal that costs one round trip and no packet. The
// answer page is minted once from the socket frames' budget and lent to each badge the first time
// it is asked, which is why a resolver is only usable beside the network.

use crate::sys::pal::nife::resolveproto as resolve;

/// A method no object type defines: the probe `sys/args` makes, so an empty slot costs no page.
const NO_SUCH_METHOD: u64 = 0xffff;

/// The resolver page's frame once it is minted and mapped, 0 before. Guarded by `RESOLVER_LOCK`,
/// which is also held across each resolve, because the page is one buffer for the whole program.
static RESOLVER_FRAME: AtomicU64 = AtomicU64::new(0);
/// One bit per entry of `rt::RESOLVER_SLOTS`: that badge has been lent the page.
static RESOLVER_ATTACHED: AtomicU8 = AtomicU8::new(0);
static RESOLVER_LOCK: AtomicBool = AtomicBool::new(false);

struct ResolverGuard;

impl Drop for ResolverGuard {
    fn drop(&mut self) {
        RESOLVER_LOCK.store(false, Ordering::Release);
    }
}

fn resolver_lock() -> ResolverGuard {
    while RESOLVER_LOCK
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        crate::hint::spin_loop();
    }
    ResolverGuard
}

/// Whether `slot` holds a capability: an empty one answers -1 to a method nothing defines.
fn granted(slot: u64) -> bool {
    // SAFETY: a plain syscall that cannot succeed; -1 is "the slot is empty".
    unsafe { rt::invoke(slot, NO_SUCH_METHOD, 0, 0, 0) != -1 }
}

/// Mint and map the resolver page once, and lend it to the badge at `rt::RESOLVER_SLOTS[i]` the
/// first time that badge is asked. `Unsupported` with no network budget to pay for the page.
fn ensure_resolver_page(_held: &ResolverGuard, i: usize) -> io::Result<()> {
    let mut frame = RESOLVER_FRAME.load(Ordering::Relaxed);
    if frame == 0 {
        // SAFETY: plain syscall; the kernel validates the slot and the budget.
        let minted = unsafe { rt::invoke(NET_MEMORY_REGION, abi::memory_region::RETYPE, 0, 0, 0) };
        if minted < 0 {
            return Err(io::Error::UNSUPPORTED_PLATFORM);
        }
        frame = minted as u64;
        // SAFETY: plain syscall; the frame was just minted and is ours to map.
        if unsafe { rt::invoke(frame, abi::page_frame::MAP, rt::RESOLVER_PAGE, 1, NET_MEMORY_REGION) } < 0 {
            return Err(io::const_error!(io::ErrorKind::Other, "mapping the resolver page failed"));
        }
        RESOLVER_FRAME.store(frame, Ordering::Relaxed);
    }
    let bit = 1u8 << i;
    if RESOLVER_ATTACHED.load(Ordering::Relaxed) & bit != 0 {
        return Ok(());
    }
    // SAFETY: plain syscall; the frame carries its budget's GRANT (slot 3's, milestone 824 (a
    // retype mints no right its budget lacks)), narrowed here.
    let sent = unsafe {
        rt::invoke(
            rt::RESOLVER_SLOTS[i],
            abi::rendezvous::SEND_CAP,
            frame,
            abi::rights::READ | abi::rights::WRITE,
            resolve::request(resolve::OPERATION_ATTACH_PAGE_FRAME),
        )
    };
    if sent < 0 {
        return Err(io::Error::UNSUPPORTED_PLATFORM);
    }
    RESOLVER_ATTACHED.fetch_or(bit, Ordering::Relaxed);
    Ok(())
}

pub struct LookupHost {
    addresses: [Ipv4Addr; resolve::MAX_ADDRESSES],
    count: usize,
    next: usize,
    port: u16,
}

impl Iterator for LookupHost {
    type Item = SocketAddr;
    fn next(&mut self) -> Option<SocketAddr> {
        let address = *self.addresses[..self.count].get(self.next)?;
        self.next += 1;
        Some(SocketAddr::V4(SocketAddrV4::new(address, self.port)))
    }
}

/// Resolve `host` through the granted resolver badges, in slot order. Each refusal keeps its
/// reason: `PermissionDenied` for a name outside every granted zone (nothing was asked),
/// `NotFound` for a name the server says does not exist, `Unsupported` for a program granted no
/// resolver.
pub fn lookup_host(host: &str, port: u16) -> io::Result<LookupHost> {
    let name = host.as_bytes();
    if name.is_empty() || name.len() > resolve::NAME_TEXT_MAX {
        return Err(io::const_error!(io::ErrorKind::InvalidInput, "not a host name"));
    }
    let held = resolver_lock();
    let page = rt::RESOLVER_PAGE;
    let mut outcome = None;
    // The badges are filled from slot 9 onward, so the first empty slot ends the list.
    for i in (0..rt::RESOLVER_SLOTS.len()).take_while(|&i| granted(rt::RESOLVER_SLOTS[i])) {
        ensure_resolver_page(&held, i)?;
        // The resolver may have written answers over the name, so it goes in again each time.
        for (k, &b) in name.iter().enumerate() {
            fw8(page + (resolve::OFF_NAME + k) as u64, b);
        }
        let (word, _ttl) = rt::call(
            rt::RESOLVER_SLOTS[i],
            resolve::request(resolve::OPERATION_RESOLVE),
            name.len() as u64,
        );
        let Some(answer) = resolve::Outcome::from_word(word) else {
            return Err(io::Error::UNSUPPORTED_PLATFORM);
        };
        outcome = Some(answer);
        if answer.status != resolve::status::DENIED {
            break;
        }
    }
    let Some(outcome) = outcome else {
        return Err(io::Error::UNSUPPORTED_PLATFORM);
    };
    let kind = match outcome.status {
        resolve::status::OK => {
            let count = (outcome.count as usize).min(resolve::MAX_ADDRESSES);
            let mut addresses = [Ipv4Addr::UNSPECIFIED; resolve::MAX_ADDRESSES];
            for (k, a) in addresses.iter_mut().take(count).enumerate() {
                let at = page + (resolve::OFF_ADDRESSES + 4 * k) as u64;
                *a = Ipv4Addr::new(fr8(at), fr8(at + 1), fr8(at + 2), fr8(at + 3));
            }
            return Ok(LookupHost { addresses, count, next: 0, port });
        }
        resolve::status::DENIED => io::ErrorKind::PermissionDenied,
        resolve::status::NO_SUCH_NAME => io::ErrorKind::NotFound,
        resolve::status::BAD_NAME => io::ErrorKind::InvalidInput,
        // The resolver asked and heard nothing it could use in time.
        resolve::status::NO_ANSWER => io::ErrorKind::TimedOut,
        // The resolver's own socket was refused a step, and the message names which, so a failure
        // read from a transcript says where to look (2026-10-13 UTC).
        resolve::status::NETWORK => {
            return Err(match outcome.detail {
                resolve::network_step::NO_SOCKET_PAGE => io::const_error!(
                    io::ErrorKind::NetworkUnreachable,
                    "the resolver had no socket page"
                ),
                resolve::network_step::OPEN => io::const_error!(
                    io::ErrorKind::NetworkUnreachable,
                    "the stack refused the resolver a socket"
                ),
                resolve::network_step::SEND => io::const_error!(
                    io::ErrorKind::NetworkUnreachable,
                    "the stack refused the resolver's send"
                ),
                resolve::network_step::CONNECT => io::const_error!(
                    io::ErrorKind::NetworkUnreachable,
                    "the resolver's connection to its name server was not established"
                ),
                _ => io::const_error!(
                    io::ErrorKind::NetworkUnreachable,
                    "the resolver was refused a step this program does not know"
                ),
            });
        }
        // A reply came back and was not believed: forged, poisoned or malformed.
        resolve::status::REFUSED => io::ErrorKind::InvalidData,
        _ => io::ErrorKind::Other,
    };
    Err(io::Error::from(kind))
}
