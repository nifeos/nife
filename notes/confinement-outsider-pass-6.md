# Confinement outsider pass 6 (2026-10-10 UTC, in progress)

The sixth outsider pass at risk 7 (the confinement claim is false), milestone 868 (a sixth outsider
pass attacks the confinement claim), whose number is provisional. By GLM 5.3, the non-Anthropic
model that ran pass 4, run by calef's opencode. It serves criterion (c)'s non-Anthropic half: if
the fifth pass (PR #1895, Anthropic) lands clean, this pass can be the second of the two
consecutive clean passes, and a booted escape here restarts the count instead.

Informed, the posture milestone 800 (a non-Anthropic model attacks the confinement claim) set: the
whole tree and its git history were in hand. The counting rule, the report format and the refusal
log are that milestone's, unchanged. An attack counts when it boots. A verdict from reading alone
is graded `read`, the weakest grade. Only a new finding counts; a re-found fixed escape is a
re-discovery, scored apart.

**Status: one booted escape on the newest shipped surface, committed as a failing test before any
fix.** The 34-row sweep and the variant work have not run yet; this note grows until they have.

## The escape: the package client's address check and its connection disagree

Milestone 801 (packages over the internet) shipped its fetch path on 2026-10-10 (PRs #1884 and
#1890). Q1's safeguard, calef's ruling of the same day: *a listed location may never reach a
private or link-local address*, and a client applies `package_index::public_address` to every
address a listed host resolves to. The client
(`pinned_tls_exerciser/src/bin/package_fetch_exerciser.rs`, `from_location`) does exactly that,
on one resolution of the name, and then hands the **name**, not the checked address, to
`TcpStream::connect((host, port))`, which resolves the name a **second** time inside the connect.
The resolver has no cache (components/src/name_resolver.rs: "every resolve is a query"), so the
two resolutions are two independent queries, and a rebinding name server answers them
differently: public for the check, private for the connect.

What that reaches: the check approved 192.0.2.1 (RFC 5737 documentation space, public to
`public_address`, never connected to in this attack), the connect went to 10.0.2.9, and the
client completed a TCP connection and a TLS handshake attempt with the peer there. Q1's sentence
is about reach, and the reach happened: a signed index's listed location, checked and approved,
connected to a private address the ruling forbids it from reaching. In production the same shape
is a mirror's DNS rebinding the package client into the owner's own network, with the client's
printed refusals and timings as a probe.

### The boot (aarch64, 2026-10-10 UTC, commit aa7ec3909)

`script/test --arch aarch64 --test a_package_is_fetched_through_the_index_by_name_over_tls_and_judged_by_its_digest`,
with `helpers/name-server-peer` answering `rebind.basalt.test` public on a boot's first query and
at the private peer on every later one, and `helpers/tls-peer` listing a `rebound` twin of
`greeting` under that name. The client printed:

```
passed over https://rebind.basalt.test:8443/rolling/targets/rebound-0.1.0-aarch64.nifepkg: Tls(InvalidCertificate(NotValidForNameContext { expected: DnsName("rebind.basalt.test"), presented: ["DnsName(\"basalt.test\")"] }))
```

The reason is the proof. A certificate name check runs on a certificate a **server sent over a
completed TCP connection**; slirp has exactly one TLS server, the guestfwd peer at the private
10.0.2.9:8443; the check had approved 192.0.2.1. So the connection reached 10.0.2.9, and the
refusal the client reports is the private peer answering. The digest admission held afterwards
(the fetch fell back and was judged), so no untrusted bytes were taken; the escape is the reach
itself, the exact sentence Q1 was ruled to prevent.

The test asserts the confinement (`system_tests/src/user/package_index_tests.rs`: no `Tls(` reason
may appear for a listed location) and is **red on the vulnerable tree**, committed before any fix
per the standing rule. Any sound fix (connect by a checked address; or re-resolve and re-check
inside the connect) keeps it red-free, because no correct client can complete a TLS exchange with
a host the resolver rebound past the check.

The first answer's state is keyed per boot: the runners export `NIFE_BOOT_TAG` (a fresh value per
emulator start) and the name server keys its first-answer file on it, because slirp runs each
guestfwd connection through a short-lived shell whose parent chain is gone before the query is
read. Two parent-chain keyings were tried and broke first (pid reuse poisoned the demo); the tag
is deterministic.

### What this is and is not

- It is a **client** defect (check-then-use on two resolutions), not a kernel or resolver defect:
  the kernel granted exactly what was asked; the resolver answered what it was asked, twice.
- It is on the fetch path as shipped on main. The boot skips in CI (milestone 855 (the TLS graph
  enters the gated build) owns that), so whether it is an escape "on a shipped path" for
  criterion (c)'s count is calef's verdict to make, with this pass's record in hand.
- The fix shape is the client's to take: resolve once, check, and connect by an address that was
  checked. The fixture's first answer (192.0.2.1) is never connected to on the vulnerable path;
  a fixed client will try it, and the fix's own determinism (an instantly refused public address
  inside slirp) is a question for the fix, not the attack.

## Read-grade findings, homed

- **The address check skips IPv6.** `from_location`'s check loop only inspects `SocketAddr::V4`;
  a `SocketAddr::V6` answer passes the check untouched, while the ruling's words are "a private or
  link-local address" with no address family named. Unreachable today (the tree carries no IPv6),
  so graded `read`. Homed in the exerciser's BUGS, beside the loop that has the gap.
- **`public_address` omits ranges beyond the classics**: 192.0.0.0/24 (RFC 6890 special purpose)
  and 198.18.0.0/15 (RFC 2544 benchmark) are admitted. Both are "not the owner's public internet"
  in spirit; whether Q1's "private" reaches them is a ruling, not a fact. Read, homed here for
  the ruling to cite.

## Machine findings this pass hit (same family as the falsifications grep)

- The TLS peers need a `python3` whose OpenSSL speaks TLS 1.3. Apple's CLT Python 3.9 (what a
  bare `env python3` resolves to in some sessions) raises `ValueError: Unsupported protocol
  version 0x304` at startup, and the guest sees a dead peer. Homebrew's python3.14 works; there
  is no unversioned Homebrew `python3`. Recorded in `helpers/tls-peer`'s BUGS. Boots in this pass
  ran with a session-local `python3` shim to python3.14 on `PATH`.

## Refusal log

Grows with the pass. Format per milestone 800's standing rule; each entry names the claim, says
what was tried in one sentence, and whose refusal it was.

1. **Claim 19/24, the redoxfs name-window TOCTOU.** Declined to boot it here, exactly as passes 4
   and 5 did: a disk fixture plus a racing writer is a milestone of its own, milestone 825 (a
   hostile client races the file server's name window), NOT-STARTED. This pass's own refusal;
   a shipped-path target already homed, so examined, not open.

## BUGS

- This note covers the pass's first day: the booted escape above, two read-grade findings, one
  machine finding. The 34-row table, the variant work against the fixed escapes, and the
  re-discovery count are unwritten; the pass is not finished and nothing here says it is.
- The rebound twin is served `greeting`'s bytes under a `rebound` stem, so the digest admits the
  fetch but the member check refuses it (`NotRequested`): harmless for the reach proof (the reach
  precedes admission), visible as a line in the exerciser's report.
