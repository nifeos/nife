# The pinned TLS client and its exercisers

Name: provisional, for this stem. calef names files.

An appendix to [notes/packages.md](../packages.md). It holds the design of `pinned_tls_client`,
built for milestone 501 (a TLS client that speaks to one pinned peer), and of the two programs that
prove it under QEMU: `pinned_tls_exerciser` and `package_fetch_exerciser`. The crate's doc comments
give the contract and link here.

## One root for one host

§196 (nife carries TLS) clause 4 rules the shape: *"the package client holds one root or one pinned
key for the one source it talks to"*. A system trust store is refused because it has to be updated
independently of the system, and the thing that updates it is the package manager. §250 (an image
names its distribution's package index) names the pair: ISRG Root X1, for `basalt.nifeos.org`.
`pinned_tls_client` is that shape in types.

## The pin is the only way in

A `Session` is made from a `PinnedPeer` and nothing else, and a `PinnedPeer` is exactly one host
name and exactly one root. The API has no root store to add a second root to, no verifier to swap,
and no "accept anyway" switch. A caller cannot widen what the client trusts without changing the
crate. That is the strongest rung this tree has (CLAUDE.md's ladder: make the wrong state
unrepresentable). It is cheap here because `rustls` already refuses everything outside the roots it
is given.

What the client checks, and where each check lives:

| check | done by |
|---|---|
| the chain ends at the pinned root | `rustls-webpki`, against a store holding one anchor |
| each signature in the chain | `cryptography_provider`'s verification algorithms |
| the leaf names the host asked for | `rustls-webpki` |
| the server holds the leaf's key | `rustls`, over the handshake transcript |
| the certificates are in date | `rustls-webpki`, against `std`'s clock (see BUGS below) |
| TLS 1.3 and nothing older | `cryptography_provider` offers no TLS 1.2 suite at all |

TLS does not buy the integrity of a package. Under §195 (a reviewed recipe vouches for a package) a
package's digest decides whether its bytes may run, over any transport. TLS here protects the
index, where the digests come from, and keeps the path from learning what was asked for.

`Session::handshake` runs the handshake to completion before it returns, rather than lazily on the
first write as `rustls::Stream` would. That lets a caller time the handshake apart from the
transfer, and see a refusal as a refusal.

## A listed location: up to two hops

calef ruled C2 *"a"* on #2048 (2026-10-12, UTC); `the-index-format.md` records it as question C.
A GitHub Release asset's download URL is on `github.com`, which chains to Sectigo Root E46. It
answers `302` with a signed URL on `release-assets.githubusercontent.com`, which chains to ISRG
Root X1. So a fetch from a listed location is up to two hops, and `listed::fetch` holds each one to
the ruling:

| hop | its root | its address |
|---|---|---|
| the listed location | the one the signed index named for it, among those the image carries (`package_index::Sources::root_for`) | Q1's check: every resolved address public, and only those dialed |
| the one redirect | the index's own root (X1 in production), whatever the first hop's was | Q1's check again, on the new host |
| a second redirect | refused, never followed | |

The redirect must be an absolute `https://` URL (`package_index::Location::redirect`). Nothing in
the client decides whether the bytes are authentic: the caller judges them by the index's digest,
as it judges bytes from anywhere.

## `pinned_tls_exerciser`: a handshake with a peer that is not ours

`cryptography_exerciser` proved that the provider's primitives compute what their specifications
say. It stopped short of a handshake on purpose: there was no peer, no socket and no client.
`pinned_tls_exerciser` is the rest. It connects through `std::net` to `helpers/tls-peer`, which is
Python's `ssl` (so OpenSSL), run by slirp at 10.0.2.9:8443 once per connection. It asks five things
of the same pin:

| line | what it proves |
|---|---|
| `handshake ok ...` | a TLS 1.3 handshake completes here, through this tree's provider |
| `index ok ...` | a body arrives intact through the record layer |
| `bulk ok ...` | 256 KiB, many records, the pattern checked byte for byte |
| `refused stranger.test: unknown issuer` | a good chain from another root is refused: the pin |
| `refused elsewhere.test: not valid for name` | the pinned root's chain for another name is refused |

A client that refused everything would print both refusals. One that accepted everything would
print the first three. Only a client that does what the pin says prints all five.

The `cost` lines are milestone 501's measurement. They are printed rather than asserted, because a
number that changes with the host running QEMU is not a pass or a fail.

The host tests in `pinned_tls_client/tests/peer.rs` run the same `helpers/tls-peer` as a child
process over its stdin and stdout, which is how slirp runs it per connection. So the peer a guest
meets and the peer the host tests meet are one program.

## `package_fetch_exerciser`: a package through the index

It proves milestone 801 (packages over the internet), items 1, 3 and 4, under calef's five rulings
of 2026-10-10 (UTC) on #1884. `system_tests/src/user/package_index_tests.rs` starts it holding the
network, a clock, entropy and a resolver badge granted the root zone, as `jig`'s is (Q5). Its lines,
and what each proves:

| line | what it proves |
|---|---|
| `index unreachable at gone.basalt.test` | the image's first address is lost, so it tries the second (Q2) |
| `index ok ... from basalt.test` | the backup resolved by name, TLS 1.3 to the pinned root, the index read from `/<channel>/metadata/` (Q4) |
| `passed over https://packages.basalt.test...` | a listed location resolving to a private address is refused (Q1) |
| `fetched greeting-... from basalt.test's targets` | the repository's own copy is the fallback, admitted by the index's digest |
| `refused uptime-...: NotCataloged` | a repository copy with one byte flipped: only the digest can tell |
| `refused nosuch: not in the index` | nothing is fetched for a name the index does not list |
| `passed over https://rebind.basalt.test...` then `fetched rebound-... from basalt.test's targets` | the twin listed under the rebinding name: the client dials only the public address its check passed, never the private one a second resolution would name (milestone 871 (a sixth outsider pass attacks the confinement claim)) |
| `redirect releases: fetched greeting-... via releases.basalt.test` | one redirect under the root the location names, admitted by the digest (calef's C2 on #2048) |
| `redirect <probe>: refused: ...` | an unnamed root, an uncarried root, a second redirect, a private address, a host off the index's root |

The redirect probes call `pinned_tls_client::listed::fetch`, as `jig` does, so the boot test that
proves the refusals proves `jig`'s path and not a copy of it. They judge addresses with
`stand_in_public`. slirp reaches no public address (a `guestfwd` outside 10.0.2.0/24 is refused), so
the peer at 10.0.2.9 stands in for one. Every other fetch uses `public_address`.

It installs nothing: installing from an index is `jig`'s, from milestone 809 (the package client
becomes a program). Under slirp every listed location is private, so taking bytes from one is proved
only by `package_index`'s host tests.

## BUGS, in full

The code keeps one line per entry; this is the longer account.

- Certificate expiry is checked against whatever `std` says the time is. On nife that is the clock
  service's page, which no stranger's machine can vouch for (milestone 501's block). A machine whose
  clock is far wrong refuses a good chain as not yet valid or expired, and one whose clock is held
  back accepts an expired one. Failing closed on the first is right; the second is the open half.
- No rotation story. When the pinned root is retired, every image carrying it stops reaching its
  index. ISRG Root X1's certificate in `roots/` is valid until 2035-06-04. The chains served today
  reach it through cross-signatures that end sooner, as `PinnedPeer::basalt_index` records.
- One request per connection, HTTP/1.0, `Content-Length` required. `Session::fetch` reports a
  redirect, and only `listed` follows one, on a new session with its own pin.
- The caller connects the stream. Turning a name into an address is
  `components/src/domain_name_resolver.rs`'s, and the name verified is whatever the pin says, never
  what the resolver used.
- In `package_fetch_exerciser`, the fix for the rebinding escape is a type at the listed path and a
  test at the rest. A listed location's addresses come only from `package_index::Location::check`.
  Nothing stops a future caller from resolving the name again and handing `get` the fresh answer.
  What catches that is `package_index_tests`' rebinding assertion, falsified on all three
  architectures.
- Under slirp the fixed client dials the checked public address, 192.0.2.1, for real. A SYN leaves
  the emulator for RFC 5737 documentation space, and the net server's connect gives up within its
  15 s bound. That is the honest cost of connecting where the check approved.
- `pinned_tls_exerciser` pins the test authority, not ISRG Root X1. Nothing in a gate can reach the
  internet, so `pinned_tls_client`'s host tests are where X1 meets a real Let's Encrypt chain, and
  those need the internet too (`cargo test -- --ignored`).
- Its timings are QEMU's, under TCG on the host that ran it. They bound nothing, and they compare
  architectures only on one host and one run.
