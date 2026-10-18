# mDNS/DNS-SD: the Time Machine advertisement, and why it is no longer here (milestone 55)

The code this note describes was retired from the tree on 2026-09-15, on calef's ruling ("Retire
all three"), by milestone 298 (retire). Every present tense below is the tree as it stood at commit
`0652c981`, the last commit that holds it; nothing described here can be built or run from `main`
any more. This follows the precedent milestone 54's removal set for [smb.md](smb.md): the note is
kept in full, with this header, because a finding worth keeping lands in `notes/` rather than in a
commit nobody will check out. Read it in the past tense.

Why it went. It was never a general responder. `multicast_dns_protocol` hardcoded exactly Time
Machine's three services (`_smb._tcp`, `_adisk._tcp`, `_device-info._tcp`), the
`multicast_dns_config` grammar could describe nothing else, and the shipped configuration was
calef's router, advertising two family backup disks. The SMB share it advertised was removed on
2026-08-30, the day milestone 55 (Time Machine) was retired because the family's backups moved to
borg on cordoba. After that nothing ran the responder but tests, and no live milestone wanted it. It
came up as a naming question (calef had ruled the `multicast_dns` stem without seeing the whole
names) and dissolved into a retirement once the names said what the program was for.

What went with it, all at `0652c981` in git:

- `components/src/multicast_dns_responder.rs`, `components/multicast_dns_responder.conf`,
  `crates/multicast_dns_protocol` (with its three Kani harnesses) and `crates/multicast_dns_config`.
- xtask's multicast prober, its independent DNS decoder, and its term in the test run's pass
  condition. The scanout and inbound checks are unchanged.
- The stack half's multicast pieces: the runners' frame-level injection hub (`NIFE_MCAST_PORT`),
  `net_stack`'s join of 224.0.0.251, and smoltcp's `multicast` feature. Each existed only for the
  responder and its prober, and without them nothing could exercise them. Nothing in the tree
  proves multicast receive or send any more; a future multicast client puts back the feature, the
  join, and a host-side peer below slirp, and "The smoltcp multicast answer" and "The QEMU gate"
  below say what each took.

What stayed, because it is not mDNS-shaped: the UDP bind grant (`BIND_UDP`,
`socket_contract::udp_bind_grant`), still proved by `socket_test_client`'s refusal and exclusivity
checks inside the accept test, and the source endpoint on a UDP `RECEIVE`, still proved by the TFTP
exchange.

The one real loss is general DNS message and name parsing, Kani-checked at the name decoder
(compression pointers cannot loop or overrun), which sat beside the Time Machine records. A unicast
resolver would want that half; milestone 384 (nothing here resolves a hostname),
`design/roadmap/0384-a-name-resolver-and-who-holds-it.md`, says where to find it.

Milestone 55's second protocol. A Mac's Time Machine UI lists only servers it discovered over
multicast DNS. The reference router advertises exactly three service types: `_smb._tcp` (the
file service and its port), `_adisk._tcp` (the Time Machine flags, which are what populate the
backup-disk list), and `_device-info._tcp` (a model string, which picks the icon). The requirement
was measured, not assumed: `dns-sd -B _adisk._tcp` on the family network returns the router, so the
working reference does it, and proving it unnecessary would mean disabling it on a working family
backup system (design/roadmap/0055-time-machine/reference.md, "mDNS is required after all").

Four pieces, all built:

- `crates/multicast_dns_protocol`: the DNS wire format, compression handling, the DNS-SD PTR/SRV/TXT
  structuring, the probe-before-claim wire halves, `respond()` (the responder's entire decision as a
  pure function) and `announcement()` (RFC 6762 section 8.3's unsolicited response). Host-tested
  against real captured router packets; Kani harnesses cover the parser's termination and bounds.
- The stack half (both ISAs). smoltcp's `multicast` feature is on and `net_stack` joins
  224.0.0.251 at startup. `BIND_UDP` claims a fixed port against a granted range
  (`socket_contract::udp_bind_grant`, the UDP twin of milestone 107's listen grant, riding the high
  half of the same spawn word). A UDP `RECEIVE` reply carries the datagram's source endpoint in the
  frame's dst fields.
- `crates/multicast_dns_config` and `components/multicast_dns_responder.conf`: what this machine
  advertises, as a document a person edits rather than constants in a program. See "The
  configuration" below.
- `components/src/multicast_dns_responder.rs`: the program. Binds 5353 through the grant, announces,
  then answers queries with `respond()` until it has served its rounds. One authority and nothing
  else: it holds no share, no file, no TCP port. So the process that tells a Mac a backup target
  exists cannot serve a byte of it, and the process that serves the bytes (`smb_server`) cannot be
  found. On the reference implementation both are one process with one configuration file.

## The measured reference, captured 2026-08-15

All three service types were captured from calef's router on 2026-08-15 and became the crate's test
vectors. What they showed beat the documentation. One `_adisk` instance carries every disk as `dkN=`
entries in a single TXT record, and the reference advertised two disks, not the roadmap's three.
`_device-info` says `model=MacSamba` even where Samba's own `fruit:model` says `TimeCapsule`. The
two flag services advertise SRV port 0, and a legacy unicast query gets the RFC 6762 section 6.7
shape with every TTL capped at 10. The decoded records are in
[mdns/router-capture.md](mdns/router-capture.md).

## The smoltcp multicast answer (the question the roadmap block asks first)

smoltcp 0.13.1 supported what mDNS needed, and the tree had it switched off: its `multicast`
feature, in the default set that `default-features = false` discards, provides group joins and the
IPv4 receive filter that otherwise drops datagrams to 224.0.0.251. Three small pieces landed with
the stack half: the feature flag, `net_stack`'s join after DHCP, and the socket surface
(`OPERATION_BIND_UDP` against a UDP bind grant, and a UDP `RECEIVE` that reports its source).
Nothing in smoltcp itself changed. The detail is in
[mdns/smoltcp-multicast.md](mdns/smoltcp-multicast.md).

## The configuration: what a person edits

`components/multicast_dns_responder.conf`, parsed by `crates/multicast_dns_config`, host-tested, and
the responder's only source for what it says:

```
host      = GL-BE9300
smb-port  = 445
model     = MacSamba
sys-flags = 0x100
disk      = alice   0x82
disk      = bob     0x82
```

Two properties are worth stating because both were decisions rather than defaults.

The IP address is not in it. Which address this machine holds is established by the DHCP lease,
so the spawn site drains the lease *before* spawning the responder and passes it as an argument. The
responder announces an A record with it, or no A record at all when it is zero. A configuration file
naming an address goes stale the first time the network changes, and an announcement with a wrong A
record is worse than one with none: a Mac caches it and then cannot connect.

The document is compiled in, not read from disk. `include_str!`, because a program reading a file
needs a file capability wired through the spawn and a fixture the QEMU gate cannot seed today. That
is a delivery limitation and not a design one: the format, the parser, the line-numbered errors and
every test are unaffected by where the bytes come from. The fix is a `FileGrantDeclaration` grant plus an
`filesystem_contract` open-and-read at startup, and it is the shortest remaining piece of
milestone 55's discovery half.

The gate reads the same document (`xtask` depends on `multicast_dns_config`) and derives its
expectations from it, so editing what this machine advertises moves the assertion with it. What the
gate does not share is the wire format: it decodes the guest's answers with a parser of its own,
because a check that decoded them with `multicast_dns_protocol` would agree with
`multicast_dns_protocol` about anything wrong.

## The QEMU gate: what it proves, and how

Slirp cannot carry multicast, so the gate went under it: the runners put the NIC on a QEMU hub with
slirp and a socket backend that xtask's multicast prober spoke to. The exchange rode inside
milestone 107 (socket)'s accept test rather than a spawn of its own, because a twelfth net server
spawn ran out of frames. The responder announced, the prober injected a multicast browse and then a
legacy one-shot, and each answer was checked record by record. Two findings matter beyond mDNS.
Slirp forwards group-addressed datagrams to the host's real network, and on 2026-08-16 the real
router answered the gate's browse, so the prober accepts only datagrams from the guest. And smoltcp
drops the datagram that triggers a neighbor resolution, so the prober asks the guest for its own
address first. The steps and the retry scheme are in [mdns/qemu-gate.md](mdns/qemu-gate.md).

## EXAMPLES

Re-deriving the reference vectors needs no special tooling. From any machine on the router's
network, a one-shot legacy query (Python, ephemeral port, so the answer arrives unicast):

```python
import socket, struct
def qname(n):
    return b"".join(bytes([len(l)]) + l.encode() for l in n.split(".") if l) + b"\x00"
q = struct.pack(">HHHHHH", 0, 0, 1, 0, 0, 0) + qname("_adisk._tcp.local") + struct.pack(">HH", 12, 1)
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.settimeout(3)
s.sendto(q, ("224.0.0.251", 5353))
print(s.recvfrom(4096)[0].hex())
```

Or, on macOS, the resolver's own view (which exercises the multicast path a Mac uses):

```sh
dns-sd -B _adisk._tcp             # browse: who advertises Time Machine disks
dns-sd -L GL-BE9300 _adisk._tcp local   # resolve: the TXT keys and SRV
```

The `-L` output presents decoded TXT entries; the Python capture gives the wire bytes, which is
what a test vector needs.

Running the responder here. The serve-forever boot starts the net server, the SMB adapter and
the responder together, each with exactly the port its spawn granted it:

```sh
cargo xtask smb-serve      # boots with a RedoxFS disk and a NIC, and parks
```

It prints the DHCP lease, the mount line for the SMB share, and then:

```
smb-serve: the mDNS responder is advertising _smb._tcp, _adisk._tcp and _device-info._tcp on 5353.
smb-serve:   on a Mac on the SAME SEGMENT: dns-sd -B _adisk._tcp
smb-serve:   what it advertises is components/multicast_dns_responder.conf, not compiled-in.
```

That `dns-sd` will find nothing under QEMU, and the reason is the same one the gate exists for:
user-mode networking does not carry multicast, so the announcement never leaves the emulator for the
host's segment. The responder is doing its job and nobody can hear it. Proving discovery end to end
needs the kernel on hardware with a real NIC on the family network, which is milestone 55's bench
and the first place a Mac's Time Machine UI could list this share.

**Changing what it advertises** is one file and a rebuild:

```sh
$EDITOR components/multicast_dns_responder.conf
cargo test -p multicast_dns_config     # the shipped document must parse, and the disks must reach the TXT
cargo xtask build
```

A mistake in the document does not put a half-formed advertisement on the network: the responder
refuses to start and reports `0xE20L`, where `L` is the line number.

## BUGS

- Everything in this section describes code retired on 2026-09-15 (milestone 298; see the
  header). It is kept as the list a future multicast responder would start from, not as a
  description of the tree.
- What the QEMU gate cannot prove, for the bench to pick up. The hub is a wire with no router
  on it, so everything a real network's multicast turns on is out of its reach. IGMP snooping is
  one: a switch forwards group traffic only to reported members, and the gate never checks the
  guest's membership report is well-formed enough to satisfy one, only that acceptance works.
  TTL handling by real forwarding is another (the injected frame carries TTL 255 but nothing
  routes it). Coexistence with a real network's mDNS chatter is a third. The gate's group traffic
  is a handful of known datagrams; a live segment delivers a firehose of other hosts' queries and
  announcements to every member. Nothing here proves the stack keeps up, or that the 2048-byte
  socket buffer survives it. The last is a real querier: no Mac's mDNSResponder has asked this
  stack anything, and no Time Machine UI has listed this share. The bench on hardware, on the family network, with `dns-sd -B`
  and then with System Settings, is where those claims get proven; the gate's job is that the
  stack's filters, grants, headers and records are right.
- No probing before claiming the name (RFC 6762 section 8.1). `multicast_dns_protocol` has the wire halves
  (`probe_query`, `conflicts`, `tiebreak`) and the responder does not use them: it announces
  straight away. Two nife machines with the same `host` in their configuration would both claim it
  and neither would notice. The missing part is a timer and a state machine, not bytes.
- One announcement, not the RFC's two to eight, and no goodbye records when the responder stops.
  Both need the same timer. A Mac that misses the announcement finds us at its next browse, which is
  frequent; a Mac that was watching keeps a dead server in its list until the TTL runs out.
- A response that would exceed the MTU is not sent. The virtio transport carries 576 bytes (a
  single-page DMA region), so `multicast_dns_protocol` composes at most 548 and the responder stays silent
  rather than handing smoltcp a datagram it will not fragment. `announcement()` emits one service
  type per call for the same reason, and a host test pins that all three fit, including at
  `multicast_dns_config`'s eight-disk ceiling. RFC 6762's own answer is the TC bit and a follow-up message,
  which nothing here implements.
- The configuration is compiled in. See "The configuration" above; the fix is a file capability.
- The prober holds one TCP connection and never reconnects. If QEMU drops the frame socket
  mid-run the check fails as "reading frames failed" rather than retrying; acceptable for a gate,
  recorded so its first flake is not a mystery.
- The multicast-response shape is asserted from the RFC, not from a capture. All three captured
  packets are legacy unicast responses (the capture tool cannot bind 5353 while mDNSResponder holds
  it). The crate's multicast responses (ID 0, additionals, cache-flush, TTL 4500/120) follow RFC
  6762 and are now pinned by the QEMU gate as well as by host tests, but no working implementation's
  multicast bytes have been compared. Capturing the router's *multicast* answer to a Mac's real
  browse (tcpdump on port 5353) would close that, and is cheap for whoever next holds a root shell
  on the network.
- No AAAA emission. `Advertisement` carries an optional IPv4 address only. The reference emits
  AAAA; a Mac on an IPv6-only network would not find us. The responder joins no IPv6 group either.
- `respond()` does not act on the QU bit (it parses; the responder answering multicast either
  way is always legal, occasionally chattier).
- Known-answer suppression is PTR-only, and the responder inherits that: a querier that already
  holds our SRV or TXT is told again. Chatty, not wrong.
- The crate's own BUGS section (`crates/multicast_dns_protocol/src/lib.rs`) records the remaining wire-level
  limits: uncompressed emission, probe and announce timing left to the caller, no TC-bit delay.
- smoltcp's `socket-mdns` feature was not evaluated. It exists in 0.13.1 for *client-side*
  DNS-over-multicast lookups (a `socket-dns` variant), not for a responder, so it does not change
  the verdict above; recorded so nobody re-derives that.
