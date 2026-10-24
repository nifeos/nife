# Name resolution: the wire format, the resolver, and the grant

Milestone 384 (in a capability system the resolver is a grant), lanes of 2026-10-04 and 2026-10-06
(UTC). Nothing in this tree turned a host name into an address. The first lane built the wire format
and stopped at three forks; calef ruled the first (§248 (the name resolver is its own confined
program)). The second built the resolver on the other two's leaning answers. calef ratified both
on its pull request (#1760) on 2026-10-06 (UTC), and §252 (a resolver grant is one zone per client
badge) records them. Fork 2 and Fork 3 below keep the options and the reasoning.

## What is built

| Piece | Where | Evidence |
|---|---|---|
| The DNS wire format a stub resolver needs: a query for one name's A records, and `Query::accept`, the checks a reply must pass | `crates/domain_name_system` | 35 host tests, three of them on replies captured from 1.1.1.1 |
| The name decoder, header codec and TCP reassembly, proved | `crates/domain_name_system/src/proofs.rs` | 3 Kani harnesses, about 34 s; each falsified by a replayable patch |
| `accept` fuzzed with a property, not only for panics | `fuzz/fuzz_targets/domain_name_system_reply.rs` | 5.2 million runs in 120 s clean; deleting the owner check was caught |
| A name resolved end to end through `net_stack`, with lies refused, then connected to | `components/src/socket_test_client.rs`, `helpers/name-server-peer` | green on aarch64 and riscv64; the same deleted check fails it with `0xe14f` |
| The resolver, §248's confined program: a `Stack` endpoint, an entropy endpoint, and a zone grant per client badge, judged before anything is sent | `components/src/domain_name_resolver.rs` | the gate below |
| What the resolver, its clients and its spawner agree on, and the grant table | `crates/domain_name_resolution_contract`; [its parties](name-resolution/the-contract.md) | 11 host tests |
| A granted client resolves inside its zone and is denied outside it; an ungranted one is denied everything | `system_tests/src/user/name_resolver_tests.rs`, `fixtures/src/name_resolver_test_client.rs` | green on aarch64, riscv64 and x86_64 over the `e1000e`; deleting the zone check fails it on `example.com` |

`Query::accept` believes an address only after the reply is a response with our transaction id, is
not truncated, echoes exactly our question and has response code zero. The address must also be
owned by the end of a CNAME chain of at most eight links starting from our name. Records for any other owner are
ignored, and the authority and additional sections are never read. The crate's own docs list the
checks in order and carry its `BUGS`.

The end-to-end test, `a_name_resolves_through_the_stack_and_a_real_one_when_the_host_answers`, asks
the runners' name server six names over TCP. `packages.nife.test` and `mirror.nife.test` (a
compressed CNAME) resolve to 10.0.2.9. `nosuch`, `forged`, `poisoned` and `loop` must each be
refused by its own check. The client then opens a TCP connection to the address it resolved and
reads an echo back. The second half of the same test is the old real-DNS check, now judged by the
crate, and it still does not gate.

### Why DNS over TCP in the test

slirp's `guestfwd` forwards TCP only. A UDP name server the test owned would have to bind a port on
the developer's machine, which the runners avoid everywhere else, and a fixed port collides between
two lanes running at once. So the gating exchange is DNS over TCP (RFC 7766), which every
general-purpose name server must serve, and the UDP path is exercised against the host's resolver
through slirp's 10.0.2.3, non-gating.

### EXAMPLES

```
$ cargo test -p domain_name_system
test result: ok. 35 passed
$ (cd crates/domain_name_system && cargo kani)
Complete - 3 successfully verified harnesses, 0 failures, 3 total.
$ script/fuzz --time 120 domain_name_system_reply
==> fuzz: no crashes in 1 targets at 120s each
$ script/test --arch aarch64 --test a_name_resolves_through_the_stack
test system_tests::user::tests::a_name_resolves_through_the_stack_and_a_real_one_when_the_host_answers ... ok
$ script/test --arch x86_64 --test a_granted_client_resolves
test system_tests::user::name_resolver_tests::a_granted_client_resolves_inside_its_zone_and_nothing_outside_it ... ok
```

A client code from the gating half reads `0xE1`, then the case's index in `NAME_SERVER_CASES`, then
the stage: `1` to `7` are transport steps and `F` is a wrong verdict, a lie believed or the truth
refused.

## The proposal: three forks

Each is a wire format two programs agree on or a choice 384 leaves open. The first lane built none
of them. The second built Forks 2 and 3 after Fork 1 was ruled, and calef ratified both (§252).
Whether to write the parser at all, or take a crate, comes first, because §46 (thin primitives or whole subsystems; we write everything in between) makes
it a decision.

### Write or take: the §46 question

calef asked on 2026-10-04 (UTC) whether to take an existing resolver crate instead. Everything in
this table was read from the published source or measured on that date, not recalled. "Bare" means
it built for `aarch64-unknown-none-softfloat` on the pinned nightly; "deps" counts normal transitive
dependencies of that configuration. Advisories are from the local RustSec database, dated 2026-09-23.

| Crate | License | Bare metal | Size, deps | Latest release | RustSec | Stub acceptance | TCP fallback | Id and source port |
|---|---|---|---|---|---|---|---|---|
| `hickory-proto` 0.26.3 | MIT or Apache-2.0 | builds with `no-std-rand`, needs `alloc` | 37,482 lines, 61 deps | 2026-09-10 | none open for 0.26.3; history: 2018-0007 stack overflow on a malicious packet, 2026-0118 unbounded NSEC3 loop, 2026-0119 quadratic compression | a codec; acceptance lives in the resolver | n/a | without std, one global `StdRng` from a 64-bit `seed()`, behind `critical-section`, panics unseeded |
| `hickory-resolver` 0.26.3 | MIT or Apache-2.0 | no: its transports run on `tokio`, which needs a poller the PAL lacks | 17,939 lines, 100 deps | 2026-09-10 | none | checks the question (optionally with 0x20 case randomization); its CNAME fold assumes chain order, and it scans `all_sections()`, authority and additional included | yes | `rand` thread generator; a fresh OS port per request |
| `domain` 0.12.3 (NLnet Labs) | BSD-3-Clause | the parser builds with defaults off | 133,146 lines, 11 deps (5 of them proc-macro, at build time) | 2026-09-25 | none | as strict as `Query::accept`: `is_answer` checks QR, id and question; host lookup follows the chain to a canonical name, refuses a loop, takes only A records it owns | yes, in the stub (`resolv`, on `tokio`, so the same poller gap) | `rand::random()`; binds port 0 and leaves it to the OS |
| `simple-dns` 0.12.0 | MIT | builds with `alloc` | 8,183 lines, 2 deps | 2026-07-26 | none | a codec only | n/a | none of its own |
| `dns-parser` 0.8.0 | MIT or Apache-2.0 | needs `std`, which the PAL provides | 2,466 lines, 3 deps | 2018-08-06 | none | a parser only | n/a | none |
| `smoltcp` 0.14.0 `socket::dns` | 0BSD | yes, already in the graph | 1,503 lines (`socket/dns.rs` and `wire/dns.rs`), 0 new | 2026-08-17 | none | checks id, port and question; follows a CNAME by renaming in place, assuming it comes first; reads only the answer section | no | PCG32 that `net_stack` seeds with `now()` |
| ours, `domain_name_system` | the tree's | yes, no `alloc` | 883 lines (297 of them comments), 0 deps | | | the seven checks above | the caller's, with `TcpReply` | the caller's, from the entropy service |

Both small parsers were tried on a self-pointing name and on a two-pointer cycle, bounded at ten
seconds: `dns-parser` returned `BadPointer` and `simple-dns` returned `InvalidDnsPacket`, so neither
hangs. simple-dns's guard is that `new_at` refuses a pointer that does not go backwards.

§46 asks two questions. First, is this on the verification path? Yes. The name decoder and the TCP
reassembly are Kani-proved here, and the proofs depend on the decoder being a small function that
could be split for the solver. That cannot be done to 37,000 or 133,000 lines of somebody else's
crate. Second, is correctness won by exposure, as with crypto, or by reading the spec? For what a
stub resolver needs, by reading the spec. RFC 1035's message format is small and closed, with no
secret-dependent timing and no arithmetic an attacker exploits. The exposure the large crates have
earned is in recursion, caching, DNSSEC and encoding. hickory's three advisories sit there, and the
one in parsing (2018-0007) is the class this crate's Kani harness rules out.

Recommendation: keep writing it, with two options recorded for the proposal.

- Option W (recommended): `domain_name_system`, as built. Nothing taken.
- Option T1: take `domain`'s parser alone, defaults off. It is the strictest crate surveyed, builds
  bare and is maintained. It costs 133,146 lines and 11 dependencies on the security path, none
  of it provable here.
- Option T2: take `simple-dns` as the codec and keep `Query::accept` in-tree. It is small and builds
  with `alloc`. It saves the decoder, which is the part already proved, and leaves the acceptance
  logic, which is the part that matters, still ours.

std is not what rules out hickory-resolver and `domain`'s stub: nife has a std port with `fs` and
`net` (notes/std.md). The blocker is the async reactor. Both run on `tokio`, and `tokio` (through
`mio`) needs a readiness poller, which nife's PAL does not provide: its socket contract is
blocking-only, with no poll verb (notes/std/caveats.md). If the PAL gains a poller, this comparison
should be revisited. `dns-parser` is out because it has not been released since 2018. Would W still win at equal cost? Yes:
its reasons are the proof and the strictness of acceptance, not effort. The decision changes if
DNSSEC validation or DNS over TLS is wanted. Those are crypto-adjacent and won by exposure, and then
`domain` or hickory should be taken.

### Fork 1. Where the resolver lives

| Option | Shape | Cost, measured or read |
|---|---|---|
| R1. Inside `net_stack` | Enable `smoltcp`'s `socket-dns`; add a resolve verb to `socket_contract` | 711 lines of `smoltcp/src/socket/dns.rs` plus 792 of `src/wire/dns.rs`, read 2026-10-04 |
| R2. Its own confined program | A resolver holding a `Stack` endpoint and an entropy endpoint, serving clients over an endpoint, using `domain_name_system` | A program, its spawn wiring and its protocol (Fork 3) |
| R3. A library in each client | Every client links the crate and holds a `Stack` endpoint | Nothing new to build |

Ruled 2026-10-04 by calef, R2: *"the resolver should be its own confined program"*. The record is
§248 (the name resolver is its own confined program), provisional until the merge queue lands it.

What the tree does in the analogous case: `network_time_client` is its own program holding a
`Stack` endpoint and an entropy endpoint, and the clock service is a separate holder of the
authority. `net_stack` is a server whose verbs are sockets, not protocols above them.

The premise of R1 was checked against `smoltcp` 0.14.0's source rather than assumed:

- Its transaction id and source port come from `smoltcp`'s PCG32 (`src/rand.rs`), and `net_stack`
  seeds it with `config.random_seed = now()`, the boot-relative counter. An off-path forger who can
  estimate boot time can predict both. R2 draws them from the entropy service, as the NTP client
  draws its nonce.
- It has no TCP fallback, so a truncated answer is parsed as if whole.
- It follows a CNAME by renaming the query in place, and its comment says it relies on the CNAME
  preceding the A records in the packet.
- It puts a DNS parser in the process that holds the NIC.

Prior art, read 2026-10-04: Fuchsia's `fuchsia.net.name` library defines `Lookup` (`LookupIp`,
`LookupHostname`) and a separate `LookupAdmin` with `SetDnsServers`, a resolver reached as its own
protocol. That the component serving it is `dns-resolver`, apart from the netstack, is recalled and
not read. Plan 9's `ndb/dns` serves `/net/dns`: a client writes a name and type and reads answers,
and only a process whose namespace holds that file can resolve at all.

R3 is refused by 384's own argument: every client would hold the network, which is the ambient
shape the milestone exists to remove.

Would R2 still win at equal cost? Yes. It costs more than R1, and the case for it is the seed, the
parser's location and the grant Fork 2 needs a holder for, none of which is effort.

### Fork 2. The grant's shape, and what enforces it

**Built on 2026-10-06 (UTC) as G3 by badge, the check on the name asked, and ratified the same day:**
calef, *"Yes on Fork 2"*, recorded in §252.

| Option | A client may resolve | Note |
|---|---|---|
| G1. Any name | everything the resolver can | barely better than ambient, as 384 says |
| G2. Exact names | a list fixed at grant time | rigid: a CDN's CNAME chain leaves the list on the first hop |
| G3. A zone | the zone and every name under it | `Name::is_within` is built and tested, label by label, so `evilnife.test` is not within `nife.test`; a zone of one host name is G2 in effect |

G2's rigidity is narrower than it sounds if the check applies to the name asked and not to the
chain the answer follows: a client granted `packages.example.org` would still follow its CNAME to a
CDN. Which of the two the grant means is part of this fork.

The enforcement is a second question with two answers the tree already uses:

- One resolver per grant, the scope in its endowment. `net_stack` is spawned with its listen and UDP
  bind grants as a word, and `fs_subtree_caretaker` is a process per subtree.
- One resolver, a scope per badge. §230 (badged endpoint capabilities) delivers the badge with each
  request, and the system log stamps writers from it.

What was built, and why. The spawner holds the resolver's one unbadged capability and grants each
client's badge a zone with `OPERATION_GRANT` messages, the shape the system log registers writers
in. The resolver judges the name the client asked against that zone before it sends anything. G1
and G2 are the same mechanism with the root zone and a one-host zone. So the one consumer in view,
801's package client, gets a zone of one host, and a client that truly needs every name says so in
its grant. Per-instance enforcement lost because it costs a process (an image, a stack, page
tables, unmeasured) per client, where a badge costs one table entry. Would this still win at equal
cost? Yes: the case is the tree's own precedent for a server with many clients (§230 and the system
log), not effort. If calef says no to G3 by badge, `Grants` and the resolver's `serve` change; the
client's code does not, because it never sees the grant.

### Fork 3. The client protocol

**Built as P1, in `crates/domain_name_resolution_contract`, and ratified on 2026-10-06 (UTC):** calef,
*"Yes on Fork 3"*, recorded in §252.

- P1. The socket contract's shape: the client attaches a page, writes the name, calls a resolve verb,
  and reads up to eight addresses, a TTL and a status word from the page. One round trip.
- P2. Plan 9's shape over `filesystem_contract`: the resolver serves a file the client writes a name
  to and reads answers from. No new protocol, and a grant is a directory capability, at the price
  of a file server's machinery for one exchange.
- P3. Names in message words. A name is up to 255 bytes, so it does not fit, which is why P1 and P2
  both use a page.

P1 won because it is the socket contract's shape, which every network client here already speaks,
and it is one round trip. P2 would put a file server's open, write, read and close around one
exchange. If calef says no, the crate's resolve words and page layout change, and so do the
resolver and its one client, the test client.
### A smaller piece: the nameserver from DHCP

Built after the Fork 1 ruling. `net_stack`'s lease report carries the first DNS server the lease
named in its second word, which was zero, laid out in `socket_contract::lease` (provisional). The
spawner that starts a resolver hands that address on as its endowment.

### What is left, and whose it is

Nothing in milestone 384 waits now: the forks are built and ratified (§252). Starting the
resolver at boot is milestone 801 (packages over the internet)'s, because the package client is its
first client. The progenitor reads the name server from `net_stack`'s lease, as the gate does, and
hands it to `domain_name_resolver`. It then grants the package client's badge its package source's host.

## BUGS

- `socket_test_client`'s test runs on aarch64 and riscv64 only. The resolver's gate runs on all
  three over the `e1000e`, and the x86_64 and UEFI runners now carry the name-server peer.
- Both gating exchanges are TCP. UDP to a name server the test owns is not gated (see above), and
  the real-DNS half that covers UDP skips when the host's resolver does not answer. The resolver's
  UDP path, with its source check and its keep-listening on a forged reply, has run on no boot.
- `socket_test_client`'s transaction id is fixed, because it holds no entropy endpoint. The
  resolver draws every id from the entropy service and asks nothing without one.
- The resolver's own `BUGS` (one request at a time, no cache, eight grants for its life) are in
  `components/src/domain_name_resolver.rs` and the crate's.
- `smoltcp`'s generator is seeded from `now()` for everything it randomizes in `net_stack`, which
  includes TCP's initial sequence numbers (`socket/tcp.rs`, read); `net_stack` picks ephemeral
  ports with its own rotating allocator, not the generator. That is a finding about
  `net_stack` beyond this milestone, proposed in
  `design/roadmap/0783-net-stack-seeds-its-generator-from-entropy.md`.
- Resolved 2026-10-11 (UTC): `notes/fuzzing.md`'s table now lists `domain_name_system_reply`, and
  its opening points at `notes/fuzzing-the-services.md` for the service targets. The row waited for
  that note's split under §212 (a prose budget), which shed the bold the prose ratchet asked for.

Name: provisional 2026-10-04 (milestone 384's lane). Retitled 2026-10-06 when the resolver was built.
