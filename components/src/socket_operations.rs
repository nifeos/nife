//! **What a socket's capability may ask**: send, receive, connect, listen, accept and bind, each
//! run against smoltcp until it completes or its bound runs out.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Drive the poll loop until `cond` holds, servicing the NIC on each wakeup. Bounded so a stuck
/// exchange returns rather than spinning forever; a genuinely lost packet still relies on the
/// QEMU-level timeout, the disk driver's discipline.
pub(crate) fn service_until(
    iface: &mut Interface,
    dev: &mut Nic,
    sockets: &mut SocketSet,
    mut cond: impl FnMut(&mut SocketSet) -> bool,
) -> bool {
    // A stuck exchange still returns rather than blocking a hart forever; 15 s is far beyond any
    // slirp round trip yet well under the 60 s watchdog. The wait below ends by it too, so it holds
    // when nothing arrives at all. The disk driver's bounded discipline.
    let deadline = instant().total_millis() + 15_000;
    loop {
        iface.poll(instant(), dev, sockets);
        if cond(sockets) {
            return true;
        }
        if instant().total_millis() >= deadline {
            iface.poll(instant(), dev, sockets);
            return cond(sockets);
        }
        wait_for_nic(iface, dev, sockets, Some(deadline));
    }
}

pub(crate) fn udp_sendto(
    iface: &mut Interface,
    dev: &mut Nic,
    sockets: &mut SocketSet,
    table: &Table,
    entry: usize,
    len: usize,
) -> u64 {
    let Some(sk) = table.socks[entry] else {
        return REP_ERR;
    };
    if sk.kind != Kind::Udp || len > DATA_MAX {
        return REP_ERR;
    }
    let Some(window) = sk.window else {
        return REP_ERR;
    };
    let dst = read_dst(window);
    let mut buf = vec![0u8; len];
    window.copy_to_slice(OFF_PAYLOAD, &mut buf);
    if sockets
        .get_mut::<udp::Socket>(sk.handle)
        .send_slice(&buf, dst)
        .is_err()
    {
        return REP_ERR;
    }
    iface.poll(instant(), dev, sockets); // push the datagram out
    REP_OK
}

pub(crate) fn sock_receive(
    iface: &mut Interface,
    dev: &mut Nic,
    sockets: &mut SocketSet,
    table: &Table,
    entry: usize,
) -> u64 {
    let Some(sk) = table.socks[entry] else {
        return REP_ERR;
    };
    let Some(window) = sk.window else {
        return REP_ERR;
    };
    let handle = sk.handle;
    let is_tcp = sk.kind == Kind::Tcp;
    let ready = if is_tcp {
        service_until(iface, dev, sockets, |s| {
            s.get_mut::<tcp::Socket>(handle).can_recv()
        })
    } else {
        service_until(iface, dev, sockets, |s| {
            s.get_mut::<udp::Socket>(handle).can_recv()
        })
    };
    if !ready {
        return REP_ERR;
    }

    let mut buf = [0u8; DATA_MAX];
    let n = if is_tcp {
        sockets
            .get_mut::<tcp::Socket>(handle)
            .recv_slice(&mut buf)
            .unwrap_or(0)
    } else {
        match sockets.get_mut::<udp::Socket>(handle).recv_slice(&mut buf) {
            Ok((n, meta)) => {
                // The datagram's source endpoint rides back in the frame's dst fields, which are
                // dead space on a reply (socket_contract's layout note): a responder needs it to
                // answer whoever asked, as TFTP's transfer ids do.
                let IpAddress::Ipv4(src) = meta.endpoint.addr;
                for (i, &b) in src.octets().iter().enumerate() {
                    window.w8(OFF_DST_IP + i as u64, b);
                }
                window.w16(OFF_DST_PORT, meta.endpoint.port);
                n
            }
            Err(_) => 0,
        }
    };
    window.copy_from_slice(OFF_PAYLOAD, &buf[..n]);
    window.w16(OFF_LEN, n as u16);
    n as u64
}

pub(crate) fn tcp_connect(
    iface: &mut Interface,
    dev: &mut Nic,
    sockets: &mut SocketSet,
    table: &Table,
    entry: usize,
) -> u64 {
    let Some(sk) = table.socks[entry] else {
        return REP_ERR;
    };
    if sk.kind != Kind::Tcp {
        return REP_ERR;
    }
    let Some(window) = sk.window else {
        return REP_ERR;
    };
    let dst = read_dst(window);
    let handle = sk.handle;
    let local = sk.local_port;
    {
        let cx = iface.context();
        if sockets
            .get_mut::<tcp::Socket>(handle)
            .connect(cx, dst, local)
            .is_err()
        {
            return REP_ERR;
        }
    }
    // Drive until the connection settles: established, or back to a closed state (a RST from an
    // unused port, the deterministic refusal outcome).
    service_until(iface, dev, sockets, |s| {
        let st = s.get_mut::<tcp::Socket>(handle).state();
        st == tcp::State::Established || st == tcp::State::Closed
    });
    match sockets.get_mut::<tcp::Socket>(handle).state() {
        tcp::State::Established => CONNECT_ESTABLISHED,
        _ => CONNECT_REFUSED,
    }
}

/// Park a fresh TCP socket in `Listen` state on `port`. `None` if smoltcp refuses the port (only
/// port 0 does that), in which case the socket is removed rather than left in the set as a
/// `Closed` socket nothing owns.
pub(crate) fn arm_listener(sockets: &mut SocketSet, port: u16) -> Option<SocketHandle> {
    let s = tcp::Socket::new(
        tcp::SocketBuffer::new(vec![0u8; TCP_RECEIVE_BUF]),
        tcp::SocketBuffer::new(vec![0u8; SOCK_BUF]),
    );
    let handle = sockets.add(s);
    if sockets.get_mut::<tcp::Socket>(handle).listen(port).is_err() {
        sockets.remove(handle);
        return None;
    }
    Some(handle)
}

/// **`LISTEN`: claim a port, if this stack was granted it** (milestone 107).
///
/// Three refusals, and they are deliberately distinguishable (`socket_contract`): outside the grant is
/// a refusal of *authority* and no retry will fix it; already-listening is a collision on an
/// exclusive name and another port would work; `REP_ERR` is a full table. A granted listen is
/// answered with the listener's own capability (§255 (each socket is its own capability)).
///
/// A listener gets no shared frame. That is the contract's claim that a listener and a connection
/// are different objects, made concrete: there is nothing to map, because nothing is carried.
pub(crate) fn tcp_listen(
    sockets: &mut SocketSet,
    table: &mut Table,
    port_word: u64,
    grant: u64,
) -> Result<(usize, u64), u64> {
    // Not truncated to 16 bits: a request for 65536 is a request for a port no grant can name, and
    // silently turning it into port 0 would answer a question nobody asked.
    if port_word > u16::MAX as u64 {
        return Err(LISTEN_DENIED);
    }
    let port = port_word as u16;
    if !grant_allows(grant, port) {
        return Err(LISTEN_DENIED);
    }
    if table
        .socks
        .iter()
        .flatten()
        .any(|s| s.kind == Kind::Listener && s.local_port == port)
    {
        return Err(LISTEN_IN_USE);
    }
    let entry = table.free().ok_or(REP_ERR)?;
    let handle = arm_listener(sockets, port).ok_or(REP_ERR)?;
    table.install(entry, handle, Kind::Listener, port);
    Ok((entry, LISTEN_GRANTED))
}

/// **`ACCEPT`: block until a peer connects, then hand the connection over as its own socket.**
///
/// The handshaken smoltcp socket becomes a connection in a free entry with a new badge, never the
/// listener in place; the listener keeps its badge and port and is re-armed on a fresh socket
/// before this returns. Returns the connection's entry, or `REP_ERR` for a full table, a socket
/// that is not a listener, or no peer within `service_until`'s 15 s. See
/// notes/net/the-inbound-half.md.
///
/// BUGS: the backlog is one connection deep (smoltcp has no accept queue), so a peer arriving
/// between a handshake completing and the re-arm gets a RST rather than a wait.
pub(crate) fn tcp_accept(
    iface: &mut Interface,
    dev: &mut Nic,
    sockets: &mut SocketSet,
    table: &mut Table,
    listening: usize,
) -> Result<usize, u64> {
    let Some(listener) = table.socks[listening] else {
        return Err(REP_ERR);
    };
    if listener.kind != Kind::Listener {
        return Err(REP_ERR);
    }
    // Room for the connection first: a handshake taken off the listener with nowhere to put it
    // would be a peer accepted and then dropped.
    let target = table.free().ok_or(REP_ERR)?;
    let handle = listener.handle;
    let port = listener.local_port;

    // `Listen` means no SYN yet and `SynReceived` means the handshake is in flight; any other state
    // means it either landed or was aborted, and both of those end the wait.
    service_until(iface, dev, sockets, |s| {
        !matches!(
            s.get_mut::<tcp::Socket>(handle).state(),
            tcp::State::Listen | tcp::State::SynReceived
        )
    });
    let landed = !matches!(
        sockets.get_mut::<tcp::Socket>(handle).state(),
        tcp::State::Listen | tcp::State::SynReceived | tcp::State::Closed
    );

    // Re-arm whatever happened, including the aborted case: the listener's authority did not expire
    // because one peer sent a RST, and a client that gets `REP_ERR` should be able to accept again.
    match arm_listener(sockets, port) {
        Some(fresh) => {
            table.socks[listening] = Some(Sock {
                handle: fresh,
                ..listener
            });
        }
        None => {
            // The listener is gone; its badge goes with it, so its capability reaches nothing.
            table.socks[listening] = None;
            sockets.remove(handle);
            return Err(REP_ERR);
        }
    }
    if !landed {
        sockets.remove(handle);
        return Err(REP_ERR);
    }
    table.install(target, handle, Kind::Tcp, port);
    Ok(target)
}

/// **`BIND_UDP`: claim a fixed UDP port, if this stack was granted it** (milestone 55's mDNS stack
/// half). `tcp_listen`'s refusals, verbatim, because they are properties of claiming an exclusive
/// port and not of TCP: outside the grant is a refusal of *authority* (`LISTEN_DENIED`), and a port
/// some live socket already holds is a collision (`LISTEN_IN_USE`, and the check spans *all*
/// sockets, so a fixed bind cannot silently shadow an ephemeral port a live socket was allocated).
/// Unlike a listener the socket carries bytes, so its holder attaches a frame to it exactly as to
/// one `OPERATION_OPEN_UDP` made.
pub(crate) fn udp_bind(
    sockets: &mut SocketSet,
    table: &mut Table,
    port_word: u64,
    grant: u64,
) -> Result<(usize, u64), u64> {
    // Not truncated to 16 bits, for tcp_listen's reason.
    if port_word > u16::MAX as u64 {
        return Err(LISTEN_DENIED);
    }
    let port = port_word as u16;
    if !udp_grant_allows(grant, port) {
        return Err(LISTEN_DENIED);
    }
    if table.port_held(port) {
        return Err(LISTEN_IN_USE);
    }
    let entry = table.free().ok_or(REP_ERR)?;
    let handle = sockets.add(udp_socket());
    if sockets.get_mut::<udp::Socket>(handle).bind(port).is_err() {
        // Only port 0 is refused, and the grant already excludes it; kept so a surprise is a
        // clean error rather than a socket nothing owns parked in the set.
        sockets.remove(handle);
        return Err(REP_ERR);
    }
    table.install(entry, handle, Kind::Udp, port);
    Ok((entry, LISTEN_GRANTED))
}

pub(crate) fn tcp_send(
    iface: &mut Interface,
    dev: &mut Nic,
    sockets: &mut SocketSet,
    table: &Table,
    entry: usize,
    len: usize,
) -> u64 {
    let Some(sk) = table.socks[entry] else {
        return REP_ERR;
    };
    if sk.kind != Kind::Tcp || len > DATA_MAX {
        return REP_ERR;
    }
    let Some(window) = sk.window else {
        return REP_ERR;
    };
    let mut buf = vec![0u8; len];
    window.copy_to_slice(OFF_PAYLOAD, &mut buf);
    let sent = sockets
        .get_mut::<tcp::Socket>(sk.handle)
        .send_slice(&buf)
        .unwrap_or(0);
    iface.poll(instant(), dev, sockets);
    sent as u64
}
