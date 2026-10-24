# mDNS: the smoltcp multicast answer

*Name: provisional.*

An appendix to [`notes/mdns.md`](../mdns.md). The code it describes was retired on 2026-09-15; read
it in the past tense.

The tree's smoltcp is 0.13.1 (`Cargo.lock`; pinned in `user/Cargo.toml` with
`default-features = false` and features `alloc`, `medium-ethernet`, `proto-ipv4`, `proto-dhcpv4`,
`socket-udp`, `socket-tcp`, `socket-dhcpv4`).

smoltcp 0.13.1 supports what mDNS needs, and the tree has it switched off. The `multicast`
cargo feature (in smoltcp's own default set, which `default-features = false` discards) provides:

- `Interface::join_multicast_group` / `leave_multicast_group` (`iface/interface/multicast.rs`),
  with IGMP membership reports sent and IGMP queries answered for IPv4 groups (MLD for IPv6).
- Receive-path acceptance: `process_ipv4` drops any packet whose destination is not us, broadcast,
  or a joined group (`has_multicast_group`). Without the feature, the only IPv4 multicast group
  accepted is `224.0.0.1` (all-systems, hardcoded), so datagrams to `224.0.0.251` are discarded
  before UDP ever sees them. The ethernet layer already accepts multicast MAC frames either way;
  the filter that matters is the IP one.

So the responder is *nearly* ordinary socket code, and the distance was measured in small pieces.
All three landed with the stack half (milestone 55, the lane after the one that wrote this
note), the shapes the sizing proposed:

1. The feature flag: `"multicast"` in `user/Cargo.toml`'s smoltcp features. Landed as its own
   commit, being a change to a vendored-engine pin's configuration.
2. The join: `net_stack` joins 224.0.0.251 right after DHCP configures, then polls so the IGMP
   membership report carries a real source address. Membership is interface state, not socket
   state, so the join is unconditional; what is granted per client is the port.
3. Socket surface. The three gaps, closed:
   - `OPERATION_BIND_UDP` (name provisional) binds a fixed UDP port, checked against a UDP bind
     grant the spawn site packs with `socket_contract::udp_bind_grant` into the high half of the
     same spawn word milestone 107's listen grant occupies. The halves are independent
     authorities; the zero word still grants nothing anywhere. The reply vocabulary is `LISTEN`'s
     three outcomes, which are properties of claiming a port, not of TCP.
   - A UDP `RECEIVE` reply now writes the datagram's source endpoint into the shared frame's
     `dst_ip`/`dst_port` fields. Those fields were dead space on a reply, so the source fits
     without a format change; an earlier draft of this note proposed exactly that (commit
     cc2de9f7c, 2026-08-15). TCP `RECEIVE` leaves them untouched; the peer is fixed by the
     connection.
   - `OPERATION_SENDTO` to a multicast destination was measured, not trusted: the QEMU gate's host-side
     prober takes the guest's group-addressed datagram off the raw wire.

What was *not* needed is any change to smoltcp itself.
