# mDNS: the QEMU gate, what it proved, and how

*Name: provisional. After the main page's own heading.*

An appendix to [`notes/mdns.md`](../mdns.md). The code it describes was retired on 2026-09-15; read
it in the past tense.

## Under slirp

Slirp cannot carry multicast in either direction, so the gate goes under it. When xtask runs the
suite, the runners attach the mmio NIC to a QEMU hub (`-netdev hubport`) with two backends. One is
slirp, unchanged: DHCP, TFTP, guestfwd and hostfwd all keep working, because a hub floods every
frame to every port. The other is a `-netdev socket` listener that xtask's multicast prober connects
to. It speaks QEMU's frame protocol, a 4-byte big-endian length and then the raw ethernet frame. The
prober is the multicast twin of milestone 107's inbound prober. It is constructed before the child so
the runner inherits `NIFE_MCAST_PORT`, passive for the whole boot, and reported after the suite.

The exchange rides inside milestone 107 (socket)'s accept test
(`a_host_process_connects_to_the_guest_and_is_answered`, both ISAs), after its TCP rounds. It does
not get a spawn of its own. A net server's spawn is ~154 frames nothing ever reclaims, and a twelfth
one died as `Unmappable(OutOfFrames)` in an unrelated later test. That is the exact failure
notes/net/memory-and-reclamation.md's memory receipt predicted. So that one spawn now carries three
clients on one `Stack` endpoint: `socket_test_client` (socket ids 0 and 1), `smb_server` (2 and 3),
and `multicast_dns_responder` (4). Its grant word is
`listen_grant(7778, 7779) | udp_bind_grant(5353, 5354)`, so the *composed* packing is what the
machine exercises, not one half alone.

## The guest side

The guest side splits in two, and the split says which half can prove what.

`socket_test_client`'s `udp_mdns_half` keeps only the refusals, because they are what a program
holding a granted port cannot demonstrate about itself. 4444 is outside the grant and is refused as
`LISTEN_DENIED`. That is authority, a different answer from "in use", calling for a different
response. 5354 is inside the grant and binds, and asking for 5354 again on a second socket id
collides.

`multicast_dns_responder` does the rest, with real DNS:

1. It binds 5353 (its whole authority), then announces all three service types to the group. The
   announcement's arrival at the prober, off the raw wire, is the proof that a multicast `SENDTO`
   leaves the guest at all. It is also where the prober learns the guest's address, from the
   announcement's own A record.
2. The prober sends an ARP request for the guest's address from the source it spoofs
   (10.0.2.99). This is load-bearing rather than polite; see the finding below.
3. The prober injects a multicast browse: a real PTR query for `_adisk._tcp.local`, addressed to
   the group rather than to the guest, from a spoofed source nothing on the virtual network holds.
   The guest's answer must come back to the group with the PTR in the answer section. The
   instance's SRV, TXT and the host's A must be additionals (RFC 6763 section 12.1). Cache-flush
   must be set on the three the responder owns and clear on the shared PTR, and every value must
   match `components/multicast_dns_responder.conf`. That the injected datagram is accepted at all
   is the RX-acceptance proof the `multicast` feature exists for. Without the join, the IPv4 input
   path drops it before UDP sees it.
4. The prober then asks the same question as a legacy one-shot, from source port 5399 with
   transaction id 0x4321. RFC 6762 section 6.7 makes the answer a different shape *and a different
   destination*. It must arrive unicast at 10.0.2.99:5399, with the id echoed and the question
   repeated. Every record must be in the answer section, with no cache-flush bits and every TTL
   capped at 10. This is the end-to-end proof that a datagram's source endpoint survives the socket
   contract, which is why that leg replaced the stack half's hand-rolled assertion of the same thing.

## The finding that could have produced a false green

Slirp is on the same hub, and it forwards a group-addressed datagram out to the *host's real
network*. On 2026-08-16 the gate's injected browse for `_adisk._tcp.local` therefore left the
laptop, and the reference router answered it. A 212-byte response came from 192.168.8.1, NATed back
onto the virtual network by slirp as a unicast to the spoofed source. It carried the five records
the capture in [router-capture.md](router-capture.md) describes. Those are the records this gate
expects, because the expectations were captured from that router. A prober that checked whichever
answer arrived first would have passed on the GL-BE9300's own bytes while the guest said nothing at
all, on the developer's network and nowhere else. So the prober takes only datagrams whose source is
the guest, and the guest's announcement must speak from the address its own A record advertises.

## The finding that cost the most to learn

smoltcp fills its neighbor cache only from an ARP packet whose target is an address the interface
holds (`process_arp` returns early otherwise, in 0.13.1). And `dispatch` drops the datagram that
triggers a neighbor resolution rather than queueing it. So a gratuitous ARP announcing the prober's
address is discarded. The guest would have to resolve 10.0.2.99 when it answered the legacy query,
and that first answer would be lost with nothing to retry it. Asking the guest for its own address
fills the cache in the same breath, which is why step 2 is a request rather than an announcement.

## Retries, and the TFTP half

Retries are self-synchronizing, which matters because the responder answers a fixed number of
queries. The responder re-announces whenever a receive times out, so an announcement means "the
last thing you sent me did not arrive". The prober re-injects the query for the stage it is in, and
advances only when it has *verified* an answer. Nothing counts a datagram that was lost.

The TFTP gate carries the slirp-shaped half of source reporting on both ISAs. It asserts the DATA
packet's source and ACKs to it. That is what TFTP's TID scheme (RFC 1350 section 4) wanted all
along, and it is the same reply-to-the-querier move the legacy leg above makes.
