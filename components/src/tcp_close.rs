//! **Ending a TCP socket or a listener on `CLOSE`** (`net_stack`'s `Table::close`).
//!
//! Name: provisional, milestone 514 (the HVF std listener hangs in some full runs)'s lane,
//! 2026-10-12 (UTC). Moved out of `net_stack.rs` when the listener half grew, because §282 (when a
//! file is split, the parent becomes an index) gives a split parent's new code nowhere to land but a
//! child named for it. `tcp_close` names the operation; `listener_reset` was considered and names
//! only half of it.

use super::*;

/// Close the TCP socket at `handle`: a connection drains its close handshake, and a listener
/// resets any peer whose handshake already landed in it.
///
/// A listener closed with a handshake in it resets that peer, as Linux resets a closed listener's
/// unaccepted connections. `tcp_accept` re-arms before it returns, so a peer that connects between
/// the last `ACCEPT` and this `CLOSE` completes into a socket nobody accepts; dropping it unpolled
/// sends nothing and leaves the peer an established connection to nobody. Milestone 514 (the HVF
/// leg's std listener test hangs in some full runs) records the runs that showed it.
///
/// Name: provisional, milestone 514's lane, 2026-10-12 (UTC). Named for its module.
pub(super) fn close_tcp(
    iface: &mut Interface,
    dev: &mut Nic,
    sockets: &mut SocketSet,
    handle: SocketHandle,
    listener: bool,
) {
    let socket = sockets.get_mut::<tcp::Socket>(handle);
    if listener {
        if !matches!(socket.state(), tcp::State::Listen | tcp::State::Closed) {
            socket.abort();
            iface.poll(instant(), dev, sockets);
        } else {
            socket.close();
        }
        return;
    }
    socket.close();
    // Drain the close handshake before the drop: a socket dropped mid-close leaves the peer
    // half-open, which blocks the guestfwd echo peer's next connection. Both FINs (Closed or
    // TimeWait) suffice; full Closed would wait out TimeWait. Bounded, so a stuck peer returns.
    service_until(iface, dev, sockets, |s| {
        matches!(
            s.get_mut::<tcp::Socket>(handle).state(),
            tcp::State::Closed | tcp::State::TimeWait
        )
    });
}
