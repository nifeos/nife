# Packages over the internet: the index, a host name and HTTPS

The appendix to [notes/packages.md](../packages.md) for milestone 801 (packages over the internet),
rung 3c of milestone 198 (a package manager). Built 2026-10-09 (UTC) by lane
`milestone/801-packages-over-the-internet`, under QEMU only. The exit criterion is xenon's installed
system, which needs a person, a host for `basalt.nifeos.org` and milestone 494 (a driver for the network
card a PC actually has) on silicon.

## What a fetch over the internet does now

From a program holding the network, a clock, entropy and a resolver grant of the root zone (calef's
Q5 on #1884), under the five rulings `the-index-format.md` records:

1. Try the image's index addresses in order (Q2): `gone.basalt.test` stands in for a lost first
   address, `basalt.test` for the backup. Each is a `package_index::Repository`, one channel.
2. TLS 1.3 to the first that answers, trusting one pinned root (`pinned_tls_client`, milestone 501
   (a TLS client that speaks to one pinned peer)), and `GET /<channel>/metadata/stand-in-index`
   (Q4). `Index::parse` reads every line or refuses the index whole.
3. `Index::find` picks the one entry a name asks for, and `Index::sources` gives where to fetch it
   (Q1): its listed HTTPS locations, then the repository's own `/<channel>/targets/`, or only the
   mirror an owner pinned.
4. A listed location whose host resolves to a private or link-local address is passed over
   (`package_index::public_address`).
5. `package_index::accept` admits the bytes only if they are the package asked for and hash to the
   index's digest.

### EXAMPLES

`package_fetch_exerciser`'s transcript on aarch64, 2026-10-10 (UTC). riscv64 and x86_64 print the
same lines for their own architecture.

```
$ script/test --arch aarch64 --test package_is_fetched_through
package_fetch_exerciser start
index unreachable at gone.basalt.test: Other
index ok 12 packages from basalt.test over TLS
passed over https://packages.basalt.test:8443/rolling/targets/greeting-0.1.0-aarch64.nifepkg: a private address (10.0.2.9)
fetched greeting-0.1.0-aarch64 from basalt.test's targets, 83875 bytes, program greeting
passed over https://packages.basalt.test:8443/rolling/targets/uptime-0.1.0-aarch64.nifepkg: a private address (10.0.2.9)
refused uptime-0.1.0-aarch64: NotCataloged
refused nosuch: not in the index
package_fetch_exerciser done
```

Under slirp every listed location is a private address, so this run proves the refusal and the
fallback; taking bytes from a public listed location is proved only by `package_index`'s host tests.

`std_resolve` shows the std half of the name, with a grant and then without one:

```
lookup packages.nife.test: 10.0.2.9:7777
lookup nosuch.nife.test: NotFound
lookup example.com: PermissionDenied
echo by name ok
...
lookup packages.nife.test: Unsupported
```

## The pieces

| Piece | Where | Gated |
|---|---|---|
| The index model, the stand-in reader, `accept` | `crates/package_index` (provisional) | host tests, in CI |
| What `name@version` means, written once | `package_archive::matching_stem` | host tests, in CI |
| A std program resolves through its grant | `lookup_host` in `patches/std-nife`, `std_runtime_contract::RESOLVER_SLOT` (9, provisional) | `a_std_program_resolves_its_granted_zone_and_nothing_without_a_grant`, all three, in CI |
| The resolver's client words, copied into std | `crates/domain_name_resolution_contract/src/wire.rs` | the same test |
| The whole fetch | `pinned_tls_exerciser/src/bin/package_fetch_exerciser.rs` | `a_package_is_fetched_through_the_index_by_name_over_tls_and_judged_by_its_digest`, all three, in CI |
| The test hosts | `helpers/tls-peer` (index), `helpers/name-server-peer` (two names) | the same test |

The whole-fetch test runs in CI since milestone 855 (the TLS graph enters the gated build), which
builds the TLS graph wherever the kernel suite runs.

### Why the resolver is a std slot

`std::net` is how a program written for any OS reaches a host by name, and `jig` (milestone 809
(the package client becomes a program)) is a std program. So the grant milestone 384 (in a
capability system the resolver is a grant) built became what `ToSocketAddrs` asks. A program
holding no resolver still connects to numeric addresses, because std parses those first. Each
refusal keeps its reason: `PermissionDenied` is the grant, `NotFound` is the name server.

The page the resolver writes answers into is minted from the socket budget (slot 3) on the first
lookup, so a resolver grant is only usable beside the network. A program with the slot empty pays
nothing; the probe is a refused method call.

## The index, split from where the bytes live

§250 (an image names its distribution's package index, and the bytes may live anywhere) separates
two things the image's catalog holds together today.

- `Index::parse` refuses a whole index at its first unreadable line, so a client never installs
  from an index it only partly understood.
- `Index::find` reads `name` and `name@version` by the catalog's own rule.
- `accept` hands the bytes to `package_archive::installable_as` with the entry as a one-line
  catalog. The progenitor's installer calls the same function with the image's catalog.

Until the TUF client of milestone 858 (lab machines update themselves through packages) exists,
the crate reads a stand-in: the catalog's line
followed by zero or more HTTPS locations. That client replaces `Index::parse` and nothing above it.
The survey and calef's rulings are [the-index-format.md](the-index-format.md).

```
greeting-0.1.0-aarch64 sha256:<64 hex> https://packages.basalt.test:8443/rolling/targets/greeting-0.1.0-aarch64.nifepkg
```

## The seams, each stubbed behind a test host

| Production | Under QEMU | Whose |
|---|---|---|
| `basalt.nifeos.org`, then `basalt.crickertech.com` | `gone.basalt.test` (refused by the name server), then `basalt.test`, `helpers/tls-peer` at 10.0.2.9:8443 | DNS and hosting: calef's hands; the backup is on a second registrable domain (§250 clause 6) |
| The channel's name | `package_index::PROVISIONAL_CHANNEL`, `rolling` | calef's |
| ISRG Root X1, for the index and listed locations (§250 clause 10), or a root the index names (clauses 11 to 13) | the test authority in `pinned_tls_client/fixtures/` | ruled for the index; X1 meets a real chain only in 501's ignored host test |
| The index's format | the stand-in encoding | the TUF client, milestone 858 |
| The resolver started from the lease | the test harness starts it, pointed at `helpers/name-server-peer` | identified work (milestone 801's block) |
| Where the image says its index is | `package_index::SOURCES_ENTRY` names `basalt.test` at 10.0.2.9 and the test root | milestone 909 (basalt composes the bootable image from its packages) writes basalt's |

## The index fetch test

`package_index_tests::a_package_is_fetched_through_the_index_by_name_over_tls_and_judged_by_its_digest`
has a replayable falsification and two breaks attested by hand on 2026-10-10 (UTC).

- With `package_index::public_address` admitting every address (on aarch64), the client connected
  to the listed location at 10.0.2.9 and the test went red on the private-address line.
- With `accept` hashing the fetched bytes instead of reading the entry's digest, `uptime` was
  refused as `MemberMismatch` rather than `NotCataloged`. The flipped byte is also caught by the
  package's own table of contents, which is why the test names the refusal.

The rebinding half is milestone 871 (a sixth outsider pass attacks the confinement claim). The index
lists `rebound` under `rebind.basalt.test`, whose name answers 192.0.2.1 (public) on the first query
and the private peer on every later one. The resolver has no cache
(`components/src/domain_name_resolver.rs`), so a client that checks one resolution and then connects
by name asks twice, and a rebinding server answers the two apart. A client that did so printed this
on aarch64 on 2026-10-10:
`Tls(InvalidCertificate(NotValidForNameContext { expected: DnsName("rebind.basalt.test"), ...`.
A `Tls(...)` reason means a TLS server answered, and the only one in slirp is the private peer at
10.0.2.9:8443. The client dials only the address its check passed (`package_index::Location::check`),
slirp has nothing at 192.0.2.1, and the reason is `ConnectionRefused`.

The replayable record's patch makes `get` in
`pinned_tls_exerciser/src/bin/package_fetch_exerciser.rs` connect by name again
(`TcpStream::connect((server_name, to[0].port()))`). It went red on aarch64, riscv64 and x86_64
(OVMF included) on patagonia on 2026-10-10, and green with it reverted. Row 35 of
[confinement-claims.md](../confinement-claims.md) cites the test.

## BUGS

- `package_fetch_exerciser` installs nothing; `jig` does since 2026-10-12, at the prompt, from the
  copy `jig update` writes ([jig.md](jig.md)). At the prompt it reaches the index by the address the
  image's sources give, since slirp's name server is the host's (milestone 892 (a std program at
  the prompt holds the network and a resolver)).
- The stand-in has no signed root, so no "moved to" (Q2's move is a signed root's), and no
  version, expiry or signature. Each is the TUF client's.
- No producer in this tree. The test host composes the stand-in from the build's catalog, in
  Python, apart from the reader on purpose.
- An address literal in a location is checked when the index is read, a host name only after it
  resolves. A name that resolves to a public address and later to a private one is checked again
  on every fetch, never cached.
- The owner's pinned mirror (`Index::sources`) is proved by host tests only: no owner setting
  exists for it yet.
- The peer's tampered copy flips one byte and leaves the package's own table of contents alone, so
  the member digest would also refuse it. The test asserts `NotCataloged`, which only the index's
  digest gives, and the falsification shows the difference.
- The resolver page is never returned. One page per program for its life.
