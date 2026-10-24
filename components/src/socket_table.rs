//! **The open sockets of every client**, keyed by the badge on each one's capability (§255 (each
//! socket is its own capability)), and the front-door operations that add one.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// **How many sockets are open at once, across every client** (§255). A constant of this server's,
/// not of the wire: a client never names an entry. Sixteen, because every client of the stack
/// shares the table. A socket's real cost, its buffers and its mapped page, is paid only while it
/// is open.
pub(crate) const SOCKETS: usize = 16;

/// Where the shared frame of the socket in table entry `entry` is mapped in `net_stack`'s address
/// space. Above the DMA page (`0x90_0000`) and well below the heap (1 GiB).
pub(crate) fn socket_va(entry: usize) -> u64 {
    0x0000_0000_00A0_0000 + entry as u64 * 0x1000
}

/// One open socket: its smoltcp handle, what kind it is, the window onto its shared frame (if it
/// has one), the local port it holds, and the badge its capability carries.
///
/// A **listener** holds a smoltcp socket parked in `Listen` state, has no shared frame (`window ==
/// None`, and it never needs one, because no bytes cross on a listener), and its port is the one it
/// was granted rather than one the allocator handed out.
#[derive(Clone, Copy)]
pub(crate) struct Sock {
    pub(crate) handle: SocketHandle,
    pub(crate) kind: Kind,
    pub(crate) window: Option<MappedWindow>,
    pub(crate) local_port: u16,
    pub(crate) badge: u64,
}

/// What a socket is, which decides what its capability may ask.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Udp,
    Tcp,
    Listener,
}

/// A UDP socket's buffer each way, and a TCP socket's send buffer.
pub(crate) const SOCK_BUF: usize = 2048;

/// **A TCP socket's receive buffer, which is the window it advertises.** A window below two
/// segments lets a peer send one segment per round trip (the sender's silly-window avoidance holds
/// back the remainder), and every round trip then pays the stack's wait for the next frame.
/// 16 KiB lets eleven 1460-byte segments fly at once. Measured on `cargo xtask network-bench`'s
/// `e1000e` with added wire delay, 8 MiB in: 2 KiB moved 8.6 Mbit/s, 8 KiB 34, 16 KiB 67
/// (notes/designware-ethernet/the-receive-rate.md has the method). Two bounds hold it, checked
/// below.
pub(crate) const TCP_RECEIVE_BUF: usize = 16 * 1024;

/// A full window must fit in a polled NIC's receive ring, which holds one slot back and drops a
/// frame that finds no free slot. A window larger than the ring would turn a burst into loss.
const _: () = {
    let segment = 1460;
    assert!(TCP_RECEIVE_BUF <= (designware_ethernet::RX_ENTRIES as usize - 1) * segment);
    assert!(TCP_RECEIVE_BUF <= (e1000e::RX_ENTRIES as usize - 1) * segment);
};

/// Every socket open at once, each a TCP connection, must leave a quarter of the heap for smoltcp's
/// own state and the frames in flight; allocation failure panics the whole stack.
const _: () = assert!(SOCKETS * (TCP_RECEIVE_BUF + SOCK_BUF) <= HEAP_MAX as usize / 4 * 3);

/// **The open sockets of every client, keyed by the badge on each one's capability** (§255 (each
/// socket is its own capability)). An entry is a place in this table and a window VA, never a
/// name any client sees.
pub(crate) struct Table {
    pub(crate) socks: [Option<Sock>; SOCKETS],
    /// Whether entry `i`'s window VA has a page mapped. A closed socket's page is unmapped, and an
    /// entry whose page could not be unmapped never takes another frame, so no socket ever reads
    /// through a page a previous holder of its entry attached.
    mapped: [bool; SOCKETS],
    /// The next badge to mint. Starts in the socket range and only counts up, so no badge is ever
    /// minted twice and a stale copy can never come to name a later socket.
    next_badge: u64,
}

impl Table {
    pub(crate) fn new() -> Self {
        Self {
            socks: [None; SOCKETS],
            mapped: [false; SOCKETS],
            next_badge: SOCKET_BADGE | 1,
        }
    }

    /// The entry holding the socket `badge` names, if one is open.
    pub(crate) fn find(&self, badge: u64) -> Option<usize> {
        self.socks
            .iter()
            .position(|s| s.is_some_and(|s| s.badge == badge))
    }

    /// A free entry, if the table has one.
    pub(crate) fn free(&self) -> Option<usize> {
        self.socks.iter().position(Option::is_none)
    }

    /// Fill `entry` with a socket over `handle` and give it a fresh badge.
    pub(crate) fn install(
        &mut self,
        entry: usize,
        handle: SocketHandle,
        kind: Kind,
        local_port: u16,
    ) {
        let badge = self.next_badge;
        self.next_badge += 1;
        self.socks[entry] = Some(Sock {
            handle,
            kind,
            window: None,
            local_port,
            badge,
        });
    }

    /// **Answer `to` with `word` and the socket at `entry`'s capability**, minted here from the
    /// `WRITE | GRANT` copy of our own endpoint with the socket's badge. If either step fails the
    /// socket is taken down again and the caller hears `REP_ERR`: a socket nobody holds a
    /// capability for is one nobody could ever close.
    pub(crate) fn hand_over(
        &mut self,
        sockets: &mut SocketSet,
        to: Reply,
        entry: usize,
        word: u64,
    ) {
        let badge = self.socks[entry].map_or(0, |s| s.badge);
        let minted = user_mode_runtime::badge(MINT, badge);
        if minted < 0 {
            self.drop_entry(sockets, entry);
            reply(to, REP_ERR, 0);
            return;
        }
        let minted = minted as u64;
        match reply_capability(to, word, 0, minted) {
            Ok(true) => {}
            // Answered, but the copy did not land: the client's table was full. Nobody holds the
            // socket, so it goes now rather than fill an entry for good.
            Ok(false) => self.drop_entry(sockets, entry),
            Err((to, _)) => {
                self.drop_entry(sockets, entry);
                reply(to, REP_ERR, 0);
            }
        }
        // Ours goes either way: the client holds its copy now, or there is none to hold.
        cap_delete(minted);
    }

    /// `OPERATION_ATTACH_PAGE_FRAME`: map `frame` writable at the window of the socket `badge`
    /// names, paid for from `net_stack`'s untyped, and drop the capability (the mapping outlives
    /// it). A badge that names no open socket, or names a listener, which carries no bytes, gets
    /// nothing, and the frame is dropped. A second attach replaces the first page.
    pub(crate) fn attach(&mut self, badge: u64, frame: u64) {
        let entry = self
            .find(badge)
            .filter(|&e| self.socks[e].is_some_and(|s| s.kind != Kind::Listener));
        if let Some(entry) = entry
            && self.unmap(entry)
        {
            let va = socket_va(entry);
            if map_page_frame(frame, va, true, MEMORY_REGION) {
                self.mapped[entry] = true;
                if let Some(sk) = self.socks[entry].as_mut() {
                    // SAFETY: the MAP above just mapped one page read/write at `va`, and it stays
                    // mapped until `unmap` takes it back, which clears this window first.
                    sk.window = Some(unsafe { MappedWindow::new(va, PAGE) });
                }
            }
        }
        cap_delete(frame);
    }

    /// Take entry `entry`'s page out of our address space, if it has one. `false` if a page is
    /// there and could not be unmapped (a stack spawned without its own address space), and the
    /// entry then keeps the page and takes no other.
    fn unmap(&mut self, entry: usize) -> bool {
        if let Some(sk) = self.socks[entry].as_mut() {
            sk.window = None;
        }
        if !self.mapped[entry] {
            return true;
        }
        // SAFETY: the syscall; the kernel checks the capability, the right and the address.
        let gone = unsafe {
            user_mode_runtime::invoke(OWN_SPACE, abi::address_space::UNMAP, socket_va(entry), 0, 0)
        } == 0;
        if gone {
            self.mapped[entry] = false;
        }
        gone
    }

    /// **`CLOSE`: end the socket and unbind its badge.** Every copy of its capability, wherever it
    /// went, reaches nothing after this, and its page leaves our address space.
    pub(crate) fn close(
        &mut self,
        iface: &mut Interface,
        dev: &mut Nic,
        sockets: &mut SocketSet,
        entry: usize,
    ) {
        let Some(sk) = self.socks[entry] else { return };
        match sk.kind {
            Kind::Tcp | Kind::Listener => {
                close_tcp(iface, dev, sockets, sk.handle, sk.kind == Kind::Listener);
            }
            Kind::Udp => {
                sockets.get_mut::<udp::Socket>(sk.handle).close();
                iface.poll(instant(), dev, sockets);
            }
        }
        self.drop_entry(sockets, entry);
    }

    /// Forget entry `entry`: its badge, its smoltcp socket, and its page.
    fn drop_entry(&mut self, sockets: &mut SocketSet, entry: usize) {
        if let Some(sk) = self.socks[entry] {
            sockets.remove(sk.handle);
        }
        let _ = self.unmap(entry);
        self.socks[entry] = None;
    }

    /// Whether any open socket holds local `port`.
    pub(crate) fn port_held(&self, port: u16) -> bool {
        self.socks.iter().flatten().any(|s| s.local_port == port)
    }
}

/// `OPERATION_OPEN_UDP`: a UDP socket on an ephemeral port, in a free entry.
pub(crate) fn open_udp(
    sockets: &mut SocketSet,
    table: &mut Table,
    ports: &mut ephemeral::Ports,
) -> Result<(usize, u64), u64> {
    let entry = table.free().ok_or(REP_ERR)?;
    let handle = sockets.add(udp_socket());
    let local_port = ports.next(instant().total_millis() as u64, |p| table.port_held(p));
    let _ = sockets.get_mut::<udp::Socket>(handle).bind(local_port);
    table.install(entry, handle, Kind::Udp, local_port);
    Ok((entry, REP_OK))
}

/// `OPERATION_OPEN_TCP`: an unconnected TCP socket with an ephemeral port, in a free entry.
pub(crate) fn open_tcp(
    sockets: &mut SocketSet,
    table: &mut Table,
    ports: &mut ephemeral::Ports,
) -> Result<(usize, u64), u64> {
    let entry = table.free().ok_or(REP_ERR)?;
    let handle = sockets.add(tcp::Socket::new(
        tcp::SocketBuffer::new(vec![0u8; TCP_RECEIVE_BUF]),
        tcp::SocketBuffer::new(vec![0u8; SOCK_BUF]),
    ));
    let local_port = ports.next(instant().total_millis() as u64, |p| table.port_held(p));
    table.install(entry, handle, Kind::Tcp, local_port);
    Ok((entry, REP_OK))
}

pub(crate) fn udp_socket() -> udp::Socket<'static> {
    udp::Socket::new(
        udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; 8], vec![0u8; SOCK_BUF]),
        udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; 8], vec![0u8; SOCK_BUF]),
    )
}

/// Read the destination (octets + little-endian port) from a shared frame's header.
pub(crate) fn read_dst(window: MappedWindow) -> IpEndpoint {
    let ip = Ipv4Address::new(
        window.r8(OFF_DST_IP),
        window.r8(OFF_DST_IP + 1),
        window.r8(OFF_DST_IP + 2),
        window.r8(OFF_DST_IP + 3),
    );
    let port = window.r16(OFF_DST_PORT);
    IpEndpoint::new(IpAddress::Ipv4(ip), port)
}
