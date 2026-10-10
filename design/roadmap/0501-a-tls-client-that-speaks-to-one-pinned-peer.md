---
status: BUILT
raised: 2026-09-19
built: 2026-10-06
promoted_from: a-tls-client-that-speaks-to-one-pinned-peer
milestone_dependencies: none
decision_dependencies: 198, 250
machine_requirements: none
specific_machine: none
needs_person: no
---
# 501. A TLS client that speaks to one pinned peer

*(Number provisional until the merge queue lands it.)* Promoted from the proposal
`a-tls-client-that-speaks-to-one-pinned-peer`, filed 2026-09-19 by the lane for milestone 442 (a
crypto provider `rustls` can use on all three bare-metal targets), which built the provider and
repriced the client out of its own clause 3. Built 2026-10-06 (UTC) by `lane/501-tls`.

## In brief

The provider 442 assembled has now completed TLS 1.3 handshakes, on all three architectures, with
a peer that is not ours (OpenSSL, through Python's `ssl`), trusting one pinned root and refusing
everything else. Off the guest, the same client verified two real Let's Encrypt hosts against ISRG
Root X1, the root §250 (an image names its distribution's package index) pins for
`basalt.nifeos.org`.

| piece | where |
|---|---|
| The client: a `Session` made from a `PinnedPeer` (one DNS name, one root) and nothing else | `pinned_tls_client/` |
| The production pin, ISRG Root X1, checked byte for byte against two sources | `pinned_tls_client/roots/` |
| The peer: TLS 1.3 over stdin and stdout, certificate chosen by SNI | `helpers/tls-peer`, a `guestfwd` at 10.0.2.9:8443 in all four runners |
| Test authorities, committed with the script that made them | `pinned_tls_client/fixtures/` |
| The in-guest program and its test | `pinned_tls_exerciser/`, `system_tests/src/user/pinned_tls_tests.rs` |
| A std program granted the network, a clock and entropy at once, which no test could start before | `std_service::start_networked` |

No dependency is new, which corrects this lane's own brief. It asked for a survey of `rustls`,
`embedded-tls` and the rest and an architect's ruling before taking any. That was ruled
twice already: §196 (nife carries TLS: `rustls` for the protocol) took `rustls` over `embedded-tls`
on 2026-09-19, and §198 (the glue is ours, the primitives are not) plus calef's "Take rsa" settled
the provider. This milestone links those and `http_response` (milestone 198 (a package manager)'s
rung 3a) and writes the pin, the session and the tests.

## What the pin is, and how the client holds it

§196's clause 4: one root for the one source, not a system store. The API makes that the only
shape there is: there is no root store to add a second root to, no verifier to swap and no
"accept anyway". The tests show it both ways on one port. A well-formed chain from another
authority is refused as `UnknownIssuer`. A control test pins that authority instead and is
admitted, so the refusal is about which root. A valid chain from the pinned authority, for another
name, is refused as `NotValidForName`.

The production pin is compiled into the program (`PinnedPeer::basalt_index`). That is §250's
shape rather than a choice made here. An image names its index, so the root that index must chain
to is part of the image and covered by its measurement. Granting the root at run time instead
would let whoever holds the grant choose what the client trusts, and nothing in §196 or §250 asks
for that.

## What it costs

Measured on patagonia, under QEMU's TCG, three runs per architecture (two legs each on x86_64),
through `net_stack` over the `e1000e` to `helpers/tls-peer`. Each figure includes slirp and a
Python peer started per connection, so it bounds the client from above rather than isolating it.

| | TCP connect | handshake (median, range) | 256 KiB over TLS (median) |
|---|---|---|---|
| aarch64 | 44 to 70 ms | 162 ms (89 to 682) | 208 ms |
| riscv64 | 64 to 94 ms | 139 ms (121 to 158) | 441 ms |
| x86_64 | 63 to 98 ms | 141 ms (99 to 887) | 175 ms |

The outliers are each architecture's first run, cold. On the host, the same client took 128 ms
and 156 ms to handshake with `letsencrypt.org` and `pages.github.com`, which is mostly the round
trips. The suite's own test takes under five seconds per architecture. None of this says what
radon or xenon will measure, where nothing is emulated and every cipher runs its portable path.

## Follow-on

- **Milestone 855.** Milestone 855 (the TLS graph enters the gated build), promoted 2026-10-09 and
  ruled 2026-10-10 (UTC): `cargo xtask test` builds `pinned_tls_exerciser` for every leg it boots,
  so its test runs in CI, and the host tests below run in the suite's host phase.
- **Milestone 595.** Milestone 801 also needs a `std` program to hold the network from the prompt.
  Milestone 595 (the shell runs a `std` program)'s `BUGS` already carries why it cannot: the
  progenitor does not mint the socket frames' budget. The rung 3a fetch lives in the progenitor,
  which has no allocator, so it cannot link `rustls` where it is.
- **Recorded.** Two facts §250's `BUGS` should carry, which a lane may not write there: ISRG Root
  X1's expiry is now checked (2035-06-04, from the certificate), and the cross-signatures in this
  block's `BUGS`. Both are in `design/roadmap/0501-a-tls-client-that-speaks-to-one-pinned-peer.md`
  until the maintainer carries them.

## BUGS

- The pin reaches today's chains only through cross-signatures that end on 2032-09-02.
  Measured 2026-10-06: Let's Encrypt now issues from `Root YE` and `Root YR`. `letsencrypt.org`
  served leaf, `YE2`, `Root YE`, then `ISRG Root X2` signed by X1. GitHub Pages served leaf, `YR1`,
  then `Root YR` signed by X1. Both work with X1 pinned. Both cross-signatures expire 2032-09-02,
  three years before X1 does, and a host that stops sending them breaks every image with no change
  on our side. The rotation story this block always lacked now has a date.
- No wall clock a stranger's machine can trust. Expiry is checked against the clock service's
  time, so a clock far behind accepts an expired certificate; one far ahead fails closed.
- The host test in `pinned_tls_client/` that reaches the internet is `#[ignore]`d, so no gate
  checks the production pin against a live Let's Encrypt chain.
- The test authorities' private keys are committed. They protect nothing, and
  `fixtures/regenerate.sh` says why they must be readable.
- TLS 1.3 only, no client certificate, one request per connection: the provider's and
  `http_response`'s limits, taken whole.

## Index row

A TLS 1.3 client that trusts exactly one root for one host name, built over the provider of
milestone 442. Proved on all three architectures against OpenSSL: the pinned peer answers, and
another root or another name is refused. ISRG Root X1 verifies real Let's Encrypt hosts today,
through cross-signatures that end on 2032-09-02.
