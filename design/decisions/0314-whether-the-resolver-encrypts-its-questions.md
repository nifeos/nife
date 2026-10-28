---
status: PROPOSED
raised: 2026-10-28
---

# 314. Whether the name resolver encrypts its questions, and over what

calef asked on 2026-10-28 (UTC) whether nife should encrypt its DNS, and if so whether over TLS
(DoT), over HTTPS (DoH), or neither for now. This file is the proposal; he rules on it. *(Section
number, title and slug provisional until the merge queue lands it.)*

## What is being decided

The name resolver is `domain_name_resolver`, §248 (the name resolver is its own confined program).
This decides whether it keeps asking in the clear or asks inside an encrypted channel. If it
encrypts, four smaller choices follow. What happens when encryption is not available? How does the
resolver learn that a server speaks it? Does nife ever pick a resolver on the owner's behalf? And
which code does nife take rather than write?

## What the resolver does today

I read each claim below from the tree at `790878c03`.

- It asks one name server: the address the spawner hands it, which the progenitor takes from the
  first DNS server in the DHCP lease (`socket_contract::lease`) or from configuration
  (`domain_name_resolution_contract::endowment`).
- It asks over UDP, and over TCP when the UDP reply comes back truncated (RFC 1035, RFC 7766). The
  endowment can also ask for TCP alone, which the QEMU runners use.
- IPv4 only, no EDNS(0), no DNSSEC, one request at a time. Both BUGS sections say so
  (`crates/domain_name_system/src/lib.rs`, `components/src/domain_name_resolver.rs`).
- The crate's BUGS states the threat plainly: an on-path attacker can forge the reply, id and all.
  The transaction id comes from the entropy service, which defeats an off-path guesser and nothing
  closer.
- Kani proves the name decoder, the header codec and the TCP length-prefix reassembly (`proofs.rs`:
  `tcp_reassembly_is_total_and_honest`). DoT carries exactly that framing.
- The resolver is a `no_std` program in `components`. The TLS client,
  `pinned_tls_client`, is milestone 501 (a TLS client that speaks to one pinned peer). It is a
  `std` package outside the main workspace. It runs over `rustls` 0.23 and this tree's
  `cryptography_provider`: §196 (nife carries TLS) and §198 (the glue is ours, the primitives are
  not). TLS 1.3 only, one pinned root for one host name.

## What encryption buys, and what it does not

It buys two things on the hop between nife and its resolver. A watcher on the local network cannot
read which names nife asks for, and an on-path forger cannot plant an answer without also defeating
TLS.

It does not protect against the resolver itself, which still sees every question and can lie in
every answer. Only DNSSEC validation protects an answer from the zone's signer to the client. And it
leaves three leaks open that matter more than they first appear:

1. The next connection names the host anyway. `jig` opens TLS to `basalt.nifeos.org`, and the
   ClientHello carries that name in the clear as SNI. Encrypted DNS hides the lookup; the handshake
   a moment later announces it. Encrypted Client Hello closes this, and its keys arrive in an HTTPS
   resource record over DNS, so ECH is worth little without encrypted DNS and encrypted DNS is worth
   little to a TLS client without ECH. `rustls` 0.23.46 carries an ECH client (`src/client/ech.rs`,
   read); `pinned_tls_client` does not enable it.
2. Size. Without padding, the length of a TLS record still hints at the name. RFC 8467 recommends
   padding queries to multiples of 128 octets with the EDNS(0) Padding option, and the resolver has
   no EDNS(0).
3. The address. The watcher sees which address nife connects to next.

**Under §195 (a reviewed recipe vouches for a package) this is not load-bearing.** A forged address
cannot make a package run: its digest decides that. And `pinned_tls_client` verifies the pin's name,
never the resolver's, so a forger who sends `jig` to its own host fails the handshake. What a forger
can still do is deny service and learn what the machine looks up. That makes encrypted DNS privacy
work and a defense for future programs that speak a cleartext protocol to a name. It is not urgent.

## Prior art, read 2026-10-28 (UTC)

| System | Transport | Default | How it finds the endpoint | When encryption fails |
|---|---|---|---|---|
| Firefox | DoH only | On by default in the United States (from February 2020, Cloudflare) and Canada (from 2021, CIRA's Canadian Shield); off elsewhere | a Trusted Recursive Resolver list chosen by Mozilla, a third party to the network | falls back; a network disables the default by failing `use-application-dns.net`, which Firefox asks the system resolver. The canary binds only the default, never a user who turned DoH on |
| Chrome | DoH | automatic: upgrades only when the system's resolver is on its list of known providers | the same provider the system already uses ("does not change the DNS provider") | fallback to plain DNS in automatic mode is recalled, not read |
| Windows 11, Server 2022 | DoH; DoT not read | upgrades a configured server that is on a built-in list (Cloudflare, Google, Quad9 by address) | the list; DDR (RFC 9462), which Microsoft built with Quad9 and OpenDNS, read in a repost of Microsoft's post; DNR (RFC 9463) in Insider build 25982 | "Encrypted preferred, unencrypted allowed" falls back silently; "Encrypted only" fails |
| Android 9 and later | DoT (Private DNS), plus DNS over HTTP/3 from a 2022 Play system update on Android 11 and later, for well-known servers only | automatic: try DoT on the network's own resolver | the lease's resolver, or a hostname the owner types | automatic falls back and checks no identity; a named hostname is strict and fails closed |
| systemd-resolved | DoT only; DoH not in `resolved.conf` | `DNSOverTLS=no` | the configured server; `address#name` names the certificate to check, otherwise the address | `opportunistic` falls back and authenticates nothing; `yes` fails closed |
| OpenBSD `unwind` | DoT; DoH not in `unwind.conf(5)` | prefers configured DoT, then opportunistic DoT to forwarders and autoconf servers | configuration and autoconf | falls through its preference list; it also validates DNSSEC itself |
| macOS 13 and iOS 16 | DoT and DoH | DDR on by default | DDR to the network's resolver; profiles and apps (`NEDNSSettings`) for anything else | DDR keeps the same provider; DNSSEC validation available to apps |

Sources: Mozilla's canary article and its 2021 Canadian rollout post; chromium.org's DoH page;
Microsoft Learn's DoH client page; the Google Security Blog (July 2022) on DoH3. Also the systemd
and OpenBSD manual pages, Apple's WWDC 2022 session 10079 and an Apple engineer's post to the IETF
ADD list, and RFCs 7858, 8310, 8467, 8484, 9250, 9462 and 9463. Android's automatic and strict modes
come from the Internet Society's 2018 write-up and later secondary sources, not from AOSP's code.

The pattern. Browsers chose DoH: they already carry HTTP/2, and a network cannot block port 443
without blocking the web. Operating-system resolvers chose DoT: it is DNS over TCP, RFC 7766, inside
TLS on port 853, so it reuses the framing they already speak. Two departures fit the same reasoning. Android
added DoH3 for two providers to cut latency (24% at the median, Google's figure), and Apple supports
both because apps need both. No operating system makes a third party its default resolver.

DNS over QUIC (RFC 9250) is the third transport: UDP port 853, one query per QUIC stream, ALPN
`doq`. nife has no QUIC stack, and adding one for DNS alone is out of proportion, so this proposal
lists it and sets it aside.

## The standards the choice leans on

- RFC 7858 (DoT). Port 853, never 53. The two-octet length prefix of RFC 1035 section 4.2.2. A
  client SHOULD reuse one connection and pipeline queries, and MUST match each reply to its query by
  id, because replies may arrive out of order.
- RFC 8484 (DoH). HTTP/2 is the minimum *recommended* version. RFC 8484 permits HTTP/1.1, which "may
  result in very poor performance". So DoH does not strictly need HPACK; it needs it to be any good.
- RFC 8310 (profiles). Strict means encrypted and authenticated, against a name or an SPKI pin, or
  no answer. Opportunistic means try, and fall back as far as cleartext.
- RFC 9462 (DDR). The client asks its plain resolver for `_dns.resolver.arpa` SVCB records. Verified
  discovery requires the designated resolver's certificate to carry the plain resolver's IP address.
  A private address cannot carry one, so for a café's `192.168.1.1` only opportunistic use is
  possible, and only at the same address.
- RFC 9463 (DNR). DHCPv4 option 162 hands the client an authentication domain name, addresses and
  the transport. Unauthenticated, like the rest of DHCP. nife's resolver already takes its server
  from the lease, so DNR fits its shape more closely than DDR does.

## What the tree does in the analogous case

`jig`'s channel: one pinned root for one host, fail closed, §196 clause 4 and §250 (an image names
its distribution's package index). A program that must trust many hosts holds the authority list of
§306 (the web's authority list is a package and a capability) as a capability, never ambiently. The
spawner tells the network time client its server, and it asks nothing else. Nothing in the tree
falls back silently to a weaker channel.

## The options

The running example: an owner opens a laptop on café Wi-Fi. The lease names the router,
`192.168.1.1`, as its name server. `jig update` looks up `basalt.nifeos.org`. (`packages.nife.test`,
the test fixture's name, sits under the `.test` name RFC 6761 reserves, and only the runners' peer
answers it, so a café has no answer to give.)

**A. Neither, for now.** The question goes out in the clear. Everyone on the café network can read
it, and an on-path attacker can answer `203.0.113.66`. `jig` then fails its pinned handshake and
says so; nothing installs. Cost: nothing. Loses on privacy, keeps the denial of service.

**B. DoT.** The resolver opens TLS to port 853 at the server its spawner names and sends the same
framed messages it sends over TCP today. If the café router speaks DoT, the question is private and
unforgeable on that hop; the SNI of `jig`'s next handshake still names `basalt.nifeos.org`. If the
router does not (most consumer routers do not, recalled rather than measured), sub-fork 1 decides.

**C. DoH.** The same question as an HTTP POST of `application/dns-message` to a URL on port 443.
Indistinguishable from web traffic, and a network that blocks it blocks the web. Needs an HTTP/2
client with HPACK and stream multiplexing inside the resolver, or HTTP/1.1 at the cost RFC 8484
names. The café router almost never offers DoH, so in practice C means a public resolver.

**D. Both.** B and C behind one interface, the transport chosen per server. Every machine carries
twice the attack surface, and only the owner on a network that blocks 853 gains from it.

**E. DNSSEC validation, instead of or beside B.** The resolver checks signatures from the root down,
so a forged answer for a signed zone fails whoever forged it, the café or the resolver itself. It
hides nothing. It needs EDNS(0), larger answers over TCP, a root trust anchor that has to roll (the
2018 KSK rollover, recalled), and RSA and ECDSA verification, which `cryptography_provider` already
carries. E protects only signed zones, and the café lookup is not one: on 2026-10-28 (UTC) `dig DS`
through 1.1.1.1 returned a DS record for `org` and none for `nifeos.org` or for `github.io`, and
`basalt.nifeos.org` is a CNAME to `nifeos.github.io`. So E protects nothing `jig` asks today.

## Sub-forks, if B, C or D

1. Opportunistic or strict by default. Opportunistic tries TLS and falls back to plain DNS; against
   an active attacker it buys nothing, since the attacker blocks 853. Strict fails the lookup. Every
   system above defaults to opportunistic or off, because strict breaks captive portals and routers
   that speak only port 53.
2. Discovery. DDR to the lease's own resolver; DNR from the lease (option 162); or configuration
   only. DDR and DNR both keep the provider the network chose.
3. A third-party default. Whether nife ever ships with a resolver it chose, as Firefox does. That
   decides, for every owner who changes nothing, which company sees every name. It is a fact that
   leaves the machine, the irreversible kind.
4. Build or take, under §46 (thin primitives or whole subsystems). nife already takes TLS (`rustls`,
   §196). The framing is ours and proved, and DoT reuses it unchanged. DoH needs an HTTP/2 client:
   `h2` and `hyper` run on `tokio`, which needs the readiness poller the PAL lacks
   (notes/name-resolution.md), so taking one waits on that poller or means writing HPACK. A full
   taken resolver (`hickory-resolver`, `domain`'s stub) runs on `tokio` too.
   notes/name-resolution.md already says the build-or-take answer moves toward `domain` or hickory
   for DNSSEC or DoT; for DoT alone it moves less than that sentence suggests, because the part that
   changes is TLS, and TLS is already taken.

## Recommendation

**B, DoT, sequenced after the resolver serves more than one request at a time.** In detail:

- DoT first. It reuses the proved RFC 7766 framing inside `rustls`, and adds no protocol nife does
  not already speak. DoH would put an HTTP/2 client with HPACK into a confined program on the
  security path, for a benefit (port 443) that matters only on a network that blocks 853. Add C if
  that blocking proves real on a network an owner uses.
- Opportunistic by default, discovered through the lease: DNR when the lease carries option 162, DDR
  otherwise, and always to the lease's own provider. On a public resolver address, DDR verifies
  against §306's authority list, held by the resolver as a capability. On a private address, use it
  only at the same address, as RFC 9462 section 4.3 requires, and say plainly that this resists a
  passive watcher only.
- Never a third-party default chosen by nife.
- Strict when the owner names a resolver, by host name and pinned root, §196 clause 4: no fallback,
  and a lookup that fails says why.
- Not before concurrency. RFC 7858 expects one long-lived connection with pipelined queries matched
  by id, and the resolver handles one request at a time. A TLS handshake per lookup at one lookup at
  a time would stall every client behind a handshake.
- EDNS(0) Padding (RFC 8467) as part of B, since an unpadded encrypted query still leaks its length.
- E as its own later proposal, not a substitute. It answers the threat B leaves (the resolver
  itself), and the two compose.

Would this still be the choice if both options cost the same? Partly.

- DoT over DoH: yes for the operating-system layer, on elegance, not effort. DoT is the existing
  framing plus TLS, one moving part; DoH is that plus an HTTP layer whose only gain is the port.
  Every operating-system resolver read above made the same call.
- But the case against D is about effort. At equal cost, carrying both and choosing by network would
  serve more owners, since some networks do block 853.
- Sequencing after concurrency is about effort too, in part. A per-lookup handshake works; it is
  slow, and the recommendation prefers to build the concurrency first rather than ship that.
- Opportunistic over strict is about compatibility, not effort.

## What it costs

Not measured; no lane has built any of it. What the tree shows: B adds no new dependency, since nife
already takes `rustls` and the provider, but it moves them into the resolver, which is `no_std`
today. Either the resolver becomes a `std` program like `jig`, or `rustls` builds `no_std` with
`alloc` (§196 measured that it builds bare). The handshake's latency and the resolver's image growth
are the first numbers a lane should take. C adds an HTTP/2 stack, unpriced. E adds a validator,
unpriced.

## Reversibility, and who has acted on it

Reversible until an image ships to a stranger. Two parts are not: a third-party default (sub-fork
3), which sends strangers' questions to a company, and a strict default, which strangers' networks
would meet as broken lookups. Nobody has acted on any of this; no program depends on the transport,
which hides behind `domain_name_resolution_contract`. Clients would not change.

## What is blocked until this is answered

Nothing. Milestone 801 (packages over the internet) works over plain DNS because §195 and the pin
make a forged address harmless to it.

## What would change the answer

- A measured block of port 853 on networks owners use moves C up.
- A first program that resolves names for a cleartext protocol makes the forgery real and moves the
  priority up.
- ECH in `pinned_tls_client` would make the confidentiality real for `jig`, and argues for doing B
  and ECH together.
- A PAL readiness poller would make `tokio`-based crates takeable, and reopens taking a whole
  resolver.
