---
status: PARTIAL
raised: 2026-09-19
milestone_dependencies: 384, 494, 501, 809, 855
decision_dependencies: unwritten
machine_requirements: x86_64 PC with a wired network card on the internet
specific_machine: xenon (rung 3c's exit criterion is xenon's installed system)
needs_person: yes
---
# 801. Packages over the internet: a host name, HTTPS and the distribution's index

Split out of milestone 198 (a package manager) on 2026-10-06 (UTC), where it was rung 3c. calef,
the same day: *"Don't fix the tooling to name a rung. We should make our milestones finer grained if
we're going to express dependencies on a fraction of them."* Milestone 576 (how many systems are out
there, and what do they run) depends on this rung and on nothing else of 198, and a dependency
field names whole milestones. The number is provisional until the merge queue lands it, and the
title and slug are drafts. `raised` is the day §157 put the rung on the path.

## What it is

From xenon's installed system, a package is fetched through `basalt.nifeos.org`'s index by host
name, verified by its digest and installed. That is rung 3c of §157 (a trivial install is a web
page, a USB drive, and packages over the internet), which [milestone 198's rung
table](0198-package-manager.md#the-rungs) still maps.

Rung 3a, the package client over plain HTTP on a LAN, is built and is milestone 198. Rung 3b, the
network card xenon has, is milestone 494 (a driver for the network card a PC actually has). This
milestone is what is left: the name, the transport, and §250's index.

## The work list

Today's `fetch` (`crates/system_initializer`) has the image's catalog and one compiled-in source.

1. Split the index from the package locations: fetch the index, then each package from where it
   says, verified by its digest.
2. The index's format, a crate by rule 7, with its "moved to" field. Ruled before rung 4, which is
   milestone 802 (the trivial install).
3. Resolve the index's host name: milestone 384 (in a capability system the resolver is a grant).
4. Speak HTTPS to it: milestone 501 (a TLS client that speaks to one pinned peer), under §196 (nife
   carries TLS). Under §195 (a reviewed recipe vouches for a package) a recipe's digest decides
   what may run, so TLS protects the index, not the package's bytes.

calef ruled item 2's five questions on #1884 on 2026-10-10 (UTC), and they add to the list:

5. Q1: each target may list HTTPS locations, tried in order before the repository's own
   `targets/`; a listed location never reaches a private or link-local address; an owner may pin
   one mirror instead. Built in `package_index` and the client.
6. Q2: the image carries a second index address, tried when the first stops answering. Built in
   the client; whether §250's wording changes is question A below.
7. Q4: the index at `/<channel>/metadata/`, packages at `/<channel>/targets/`, one channel per TUF
   repository, all three architectures in one index. Built; the channel name is provisional.
8. Q5: the client holds the root zone. Built in the test's grant. Q3, a default owner trust line,
   is proposed as its own milestone (Follow-on).

## Built 2026-10-09 (UTC): items 1, 3 and 4 under QEMU, and item 2's survey

Lane `milestone/801-packages-over-the-internet` (PR #1884). The account, the transcripts and the
seams are `notes/packages/over-the-internet.md`.

- Item 1. `crates/package_index` (provisional) reads an index, finds the one entry a name asks for,
  and admits fetched bytes through `package_archive::installable_as` with the entry as a one-line
  catalog, the installer's own check. The encoding is a stand-in until item 2 is ruled.
- Item 3. A std program's `ToSocketAddrs` asks the resolver badge at
  `std_runtime_protocol::RESOLVER_SLOT` (9, provisional), keeping each refusal's reason.
  `a_std_program_resolves_its_granted_zone_and_nothing_without_a_grant` gates it on all three
  architectures in CI, falsified once.
- Items 4 to 8 (2026-10-10). `package_fetch_exerciser` (provisional) finds the image's first
  index address gone and reads the index from the second, by name, over TLS pinned to the test
  root. It passes over a listed location that resolves to a private address, fetches `greeting`
  from the repository's own `targets/` and admits it by the index's digest, and refuses an altered
  copy. Green on all three architectures locally, and in CI since milestone 855 (the TLS graph
  enters the gated build) builds the TLS graph in every suite run.
- Item 2. The format family was already TUF (§250's amendment and milestone 858 (lab machines
  update themselves through packages)'s Fork 9); calef ruled what TUF leaves open on #1884, and
  `notes/packages/the-index-format.md` has the rulings. The encoding stays a stand-in until the
  TUF client exists.

The status is PARTIAL rather than BUILT: the exit criterion is xenon's installed system, the TUF
client is 858's, and installing from an index is `jig`'s (milestone 809 (the package client
becomes a program)).

## What is calef's

Five questions were ruled on #1884 on 2026-10-10 (UTC). Two that the rulings raise are asked
there, with options, under `needs-architect` (`notes/packages/the-index-format.md`):

- A. The second index address against §250 clause 1's one fixed name, and clause 3's pin
  (recommended: one index at up to two addresses on different registrable domains, one pin).
- B. Which root a listed HTTPS location must chain to (recommended: the index's own pin).
- The channel's name. `rolling` is provisional.

Not rulings but calef's hands: DNS and a host for `basalt.nifeos.org`, and the exit criterion on
xenon.

## What it waits on

- Milestones 384, 494 and 501, in the frontmatter, and 809 and 855.
- §250 (an image names its distribution's package index, and the bytes may live anywhere) is
  ruled, and its path was ruled on #1884 (Q4). The `unwritten` decision dependency is now the
  amendments those rulings make. Q3 amends §220 (signed builds: a vendor signs, a developer
  self-signs, and trusting a key is scoped) and §195 (a reviewed recipe vouches for a package).
  The backup address may amend §250's own wording (question A).
- Not a ruling but calef's hands: DNS and a host for `basalt.nifeos.org`. That is `needs_person`.

Reuse: the package client, digest check and installer of milestone 198 are this milestone's
base. The §46 (thin primitives or whole subsystems) survey of index formats is
`notes/packages/the-index-format.md`; the TUF client it points to is taken, not written (858).

## Architectural parity

The capability here is the resolver, the TLS client and fetching the distribution's index over
HTTPS. It ships on aarch64, riscv64 and x86_64, proven under QEMU by the same suite on each, as rung
3a's fetch already is: aarch64 and riscv64 since 2026-09-24, x86_64 since 2026-10-05 (milestone
198's rung table and its "x86_64 fetches" entry). xenon is the silicon reference for the exit
criterion, not the scope. radon follows on silicon once the network half of milestone 53 (the
board's own peripherals: network and storage on real silicon), a driver for the JH7110's GMAC,
exists. aarch64 silicon waits on an aarch64 board.

## BUGS

- Nothing installs from an index yet. The progenitor installs only what the image's catalog
  vouches for; an index copy it trusts is `jig` writing one, milestone 809's ruling I2.
- The index client cannot run at the prompt: the booted system starts no resolver and gives a std
  program no network. Proposed: `design/roadmap/proposals/a-std-program-at-the-prompt-holds-the-network-and-a-resolver.md`.
- `notes/packages/over-the-internet.md`'s BUGS carry the stand-in's limits (no "moved to", one plain
  HTTP location, no producer in the tree).

- Proved on xenon alone until milestone 802's second-machine criterion runs.
- A laptop with no Ethernet port cannot reach this rung. That is milestone 788 (Wi-Fi on a PC that
  has no Ethernet).

## Follow-on

- **Proposed.** `design/roadmap/proposals/an-image-carries-its-distributions-root-as-a-default-trust-line.md`:
  calef's Q3, basalt's root SHA-256 as a default owner trust line. The §220 and §195 amendments
  the ruling makes are the integrator's text.
- **Milestone 809.** `jig` holds the root zone (Q5) and installs from any repository whose root is
  an owner trust line with a §220 ceiling: its `add-index`.
- **Milestone 858.** Who signs a target's locations, decided with the signing setup (Q1), and
  following a root that names a new location (Q2).

- **Proposed.** `design/roadmap/proposals/a-std-program-at-the-prompt-holds-the-network-and-a-resolver.md`:
  the booted system starts the resolver from the lease, and a std program at the prompt is granted
  the network and a resolver zone. `jig` needs it to reach the index by name.
- **Milestone 809.** `jig` adopts `package_index` and `package_fetch_exerciser`'s fetch, writes the
  index copy, and installs from it. That is this rung's exit criterion short of xenon.
- **Milestone 855.** Whether a gate builds the TLS graph, which is whether item 4's test runs in CI.
- **Milestone 858.** The TUF client and verifier, items 4 and 5 of its work list, replace
  `Index::parse` once item 2's questions are ruled.
- **Done.** The std farm's stamp now covers `byte_sink_protocol`, found while adding the resolver's
  wire to the PAL.
- **Done.** A fresh `net_stack` starts its ephemeral ports at a counter-derived point, not 49152.
  A resolver's first `CONNECT` failed three runs in four after another stack's test had used the
  same 4-tuple (CI run 38009701250). Milestone 783 (the network stack seeds its random generator
  from the clock, and TCP sequence numbers come from it) is where that seed should come from
  entropy.
- **Done.** Every std program the test harness starts hands back its frames when it ends: the clock
  service, the configuration page and the stack now come from one region a holding reclaims
  (calef, 2026-10-10 (UTC): "Lets fix the leaks."), proved by
  `a_std_program_gives_back_every_frame_it_was_given`. About 56 frames a spawn had been kept for
  the boot.

## Index row

Rung 3c of the trivial install, split out of milestone 198 so that milestone 576 (how many systems
are out there) can depend on it alone. A package is fetched from xenon's installed system through
`basalt.nifeos.org`'s index by host name, over HTTPS, verified by digest and installed. It waits on
the resolver (384), the TLS client (501) and xenon's network card (494). Built under QEMU
2026-10-09 and 2026-10-10: the index split from where the bytes live, a std program resolving
through its grant, and the whole fetch over pinned TLS. That fetch follows calef's five rulings on
the index: HTTPS locations with a fallback, a backup address, the channel layout, the root zone.
