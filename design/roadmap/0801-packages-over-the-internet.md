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

Rung 3c of milestone 198 (a package manager), as a milestone of its own. calef, 2026-10-06 (UTC): *"Don't fix the tooling to name a rung. We should make our milestones finer grained if
we're going to express dependencies on a fraction of them."* Milestone 576 (how many systems are out
there, and what do they run) depends on this rung and on nothing else of 198, and a dependency
field names whole milestones. The title and slug are drafts. `raised` is the day §157 put the rung on the path.

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
   the client; §250 clause 6 is its wording (question A below).
7. Q4: the index at `/<channel>/metadata/`, packages at `/<channel>/targets/`, one channel per TUF
   repository, all three architectures in one index. Built; the channel name is provisional.
8. Q5: the client holds the root zone. Built in the test's grant. Q3, a default owner trust line,
   is proposed as its own milestone (Follow-on).

## Built 2026-10-09 (UTC): items 1, 3 and 4 under QEMU, and item 2's survey

PR #1884. The account, the transcripts and the
seams are `notes/packages/over-the-internet.md`.

- Item 1. `crates/package_index` (provisional) reads an index, finds the one entry a name asks for,
  and admits fetched bytes through `package_archive::installable_as` with the entry as a one-line
  catalog, the installer's own check. The encoding is a stand-in until item 2 is ruled.
- Item 3. A std program's `ToSocketAddrs` asks the resolver badge at
  `std_runtime_contract::RESOLVER_SLOT` (9, provisional), keeping each refusal's reason.
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

## Built 2026-10-12 (UTC): `jig` installs from the index at the prompt

This is the exit criterion short of xenon, on all
three architectures under QEMU, in `script/swish-check`'s second boot. [notes/packages/jig.md](../../notes/packages/jig.md)
has the transcript and the BUGS.

- The image names its index. A measured archive entry, `package_index::SOURCES_ENTRY`, lists the
  index's addresses in the order tried and the one root their TLS must chain to (`root
  isrg-root-x1`, or a certificate in hex). basalt can rewrite it without rebuilding `jig`
  (milestone 909 (basalt composes the bootable image from its packages)).
- `jig update` fetches the index from the first address that answers, pinned to that root, and
  writes this architecture's lines to `activation/indexes/image`. `jig list` reads the copy.
  `jig install <name>` looks the name up there first, fetches from the entry's listed locations
  (public addresses only) and then the repository's `targets/`, and judges the bytes by the
  entry's digest.
- The progenitor installs on the copy's word when the image's catalog does not vouch (ruling I2).
  `jig install greeting@0.3.0`, a package only the index names, passes; with the copy's check
  removed it is refused (falsified on aarch64, 2026-10-12).
- The store is a directory grant the owner's console alone makes, at the `std` layout's directory
  slot, on a line that names no directory of its own; the sources page comes with it (slot 18).

The status is PARTIAL rather than BUILT: the exit criterion is xenon's installed system, which
has not run it. The TUF client is 858's.

## What is calef's

Item 2's five questions and the two they raise, A and B, are ruled on #1884, and §250's
amendment of 2026-10-10 (UTC) records A and B (`notes/packages/the-index-format.md` has the
options):

- A. The second index address against §250 clause 1's one fixed name, and clause 3's pin. Ruled
  A1, *"two domains"*: §250 clause 6, one index at up to two addresses on different registrable
  domains. The second domain is `crickertech.com`, chosen 2026-10-21 (UTC), so the image's second
  address is `basalt.crickertech.com` (clause 6 also records that NS1 serves both domains).
  Serving it, and its DNS record, are calef's hands.
- B. Which root a listed HTTPS location must chain to. Ruled *"X1 only"*: §250 clause 10. Clauses
  11 to 13 (question C, below) let the signed index name another root the image carries.
- The channel's name. `rolling` is provisional, and naming it is calef's (§250 clause 7).
- C. How `jig` reaches a GitHub Release asset, whose download starts at `github.com` (Sectigo, not
  X1) and redirects. Ruled 2026-10-12 (UTC) on #2048, below.

Not a ruling but calef's hands: the exit criterion on xenon. The host is built:
`basalt.nifeos.org` is a CNAME to `nifeos.github.io`, and
`https://basalt.nifeos.org/rolling/metadata/stand-in-index` answers 200, listing every package
against one Release (checked 2026-10-18 UTC, `rolling-2026-10-18-7a6a6b5`). On xenon the run waits
on the I219 being admitted at boot, which takes milestone 494's bench boot first.

One fork was decided rather than asked, because it is reversible and in-tree: how a manifest
names a fixed directory (809's open fork). `Manifest::indexes`, granted like the installer, puts
`activation/indexes` at the directory slot. It lives under `activation/` because it decides what
installs, and because `packages/indexes` could collide with a package named `indexes`.

## Ruled 2026-10-12 (UTC): where `basalt.nifeos.org` is served

calef was asked whether the host should be a VPS running Caddy, GitHub Pages, Cloudflare, or S3 and
CloudFront. He answered *"b"*, GitHub Pages. Asked how adoption would then be measured, he answered
*"Packages on Releases, index on Pages"*. Later the same day: basalt must not become the
organization's Pages site, since `www.nifeos.org` will be its own site from
`nifeos/nifeos.github.io`; basalt is a project site of `nifeos/basalt` at its own custom domain.

- The index (`/rolling/metadata/`) is served by GitHub Pages at `basalt.nifeos.org`, a CNAME to
  `nifeos.github.io`, whose certificate chains to ISRG Root X1 (measured 2026-10-13), so §250's pin
  holds unchanged.
- Package bytes are GitHub Release assets, one Release per publish, each listed in its index line
  as a location. §250 clause 2 already lets bytes live anywhere. An asset's `download_count` is the
  first adoption signal (milestone 576).
- basalt's `publish` job (nifeos/basalt#13) builds this. It also keeps clause 9's copy under
  `targets/` on Pages, the fallback when the Release cannot be reached.
- The image's production sources exist: `package_index::BASALT_SOURCES`, packed when an image is
  built with `NIFE_INDEX_SOURCES=basalt`. Every check
  still boots the test peer.

## Ruled 2026-10-12 (UTC): a listed location names its root, and one redirect is followed

Question C, asked on #2048 with the options in `notes/packages/the-index-format.md`. calef answered
*"a"*, the recommended C2, and §250's amendment of that date records it as clauses 11 to 13.

- The signed index may name, per listed location, the root its certificate must chain to, among the
  roots the image carries: `<root>=https://...` in the stand-in encoding. A location naming none
  chains to the index's root. The image carries further roots on `location-root` lines in its
  sources; basalt's names `sectigo-root-e46`, the root `github.com` chains to.
- A fetch from a listed location follows exactly one redirect. It must be an absolute HTTPS URL,
  its host is held to Q1's public-address check and to the index's root, and a second is refused.
- The digest check is unchanged.

Built 2026-10-13 (UTC). `pinned_tls_client::listed` is
the fetch, which `jig` and `package_fetch_exerciser` both call. `package_index` reads the root
token, `Sources::root_for` chooses it, and `pinned_tls_client::SECTIGO_ROOT_E46` is the root,
checked byte for byte against Sectigo's and macOS's copies. An ignored host test fetched ripgrep's
Release asset through it: `github.com` under E46, a `302` with a 931-byte path, then the bytes from
an X1 host. `a_package_is_fetched_through_the_index_by_name_over_tls_and_judged_by_its_digest` runs
six redirect probes on all three architectures. One good redirect is fetched and admitted. Refused:
an unnamed root, a root the image does not carry, a second redirect, a private address and a host
off the index's root. Falsified on aarch64 twice: a redirect pinned to the
location's root fetched from the off-root host, and two redirects allowed fetched through both.

## What it waits on

- Milestones 384, 494 and 501, in the frontmatter, and 809 and 855.
- §250 (an image names its distribution's package index, and the bytes may live anywhere) is
  ruled, and its path was ruled on #1884 (Q4). The `unwritten` decision dependency is now the
  amendments those rulings make. Q3 amends §220 (signed builds: a vendor signs, a developer
  self-signs, and trusting a key is scoped) and §195 (a reviewed recipe vouches for a package).
  The backup address amends §250 clause 1, as clause 6 (question A).
- Not a ruling but calef's hands: the run on xenon. That is `needs_person`. It waits on xenon's
  I219, which the kernel refuses until milestone 494's bench boot proves it.

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

- An image names the test peer (`basalt.test`, at 10.0.2.9 by the sources' address) and the test
  root unless it is built with `NIFE_INDEX_SOURCES=basalt`. Nothing builds it that way until
  milestone 909, and no boot here proves the production text, since a check may not depend on a
  host outside the runner.
- Under slirp no public address can be reached, so the boot test's redirect probes judge addresses
  with a stand-in that admits the peer at 10.0.2.9 (`package_fetch_exerciser`'s
  `stand_in_public`). `jig` always uses `public_address`, and at the prompt it reaches no listed
  location, so its redirect path is proved by the exerciser calling the same function.
- If GitHub moves `github.com` to a root the image does not carry, `jig` passes over the Release
  and installs from the Pages copy, uncounted, until an image carries the new root.
- At the prompt the index host is reached by the address the sources give, not by name, because
  slirp's name server is the host's (milestone 892 (a std program at the prompt holds the network
  and a name resolver)). Resolution by name is proved in the kernel harness.
- The index copy is one page: this architecture's lines, at most 4,095 bytes.
- `notes/packages/over-the-internet.md`'s BUGS carry the stand-in's limits (no "moved to", one plain
  HTTP location, no producer in the tree).

- Proved on xenon alone until milestone 802's second-machine criterion runs.
- A laptop with no Ethernet port cannot reach this rung. That is milestone 788 (Wi-Fi on a PC that
  has no Ethernet).

## Follow-on

- **Milestone 894.** Milestone 894 (an image carries its distribution's TUF root as a default owner
  trust line):
  `design/roadmap/0894-an-image-carries-its-distributions-root-as-a-default-trust-line.md`: calef's
  Q3, basalt's root SHA-256 as a default owner trust line. The §220 and §195 amendments the ruling
  makes are the integrator's text.
- **Milestone 809.** `jig` holds the root zone (Q5) and installs from any repository whose root is
  an owner trust line with a §220 ceiling: its `add-index`.
- **Milestone 858.** Who signs a target's locations, decided with the signing setup (Q1), and
  following a root that names a new location (Q2).

- **Milestone 892.** Milestone 892 (a std program at the prompt holds the network):
  `design/roadmap/0892-a-std-program-at-the-prompt-holds-the-network-and-a-resolver.md`: the booted
  system starts the resolver from the lease, and a std program at the prompt is granted the network
  and a resolver zone. `jig` needs it to reach the index by name.
- **Done.** `jig` adopts `package_index` and the fetch, writes the index copy, and installs from it
  (2026-10-12, above).
- **Milestone 855.** Whether a gate builds the TLS graph, which is whether item 4's test runs in CI.
- **Milestone 858.** The TUF client and verifier, items 4 and 5 of its work list, replace
  `Index::parse` once item 2's questions are ruled.
- **Done.** The std farm's stamp now covers `byte_sink_contract`, found while adding the resolver's
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

Rung 3c of the trivial install, a milestone of its own so that milestone 576 (how many systems
are out there) can depend on it alone. A package is fetched from xenon's installed system through
`basalt.nifeos.org`'s index by host name, over HTTPS, verified by digest and installed. It waits on
the resolver (384), the TLS client (501) and xenon's network card (494). Built under QEMU
2026-10-09 and 2026-10-10: the index split from where the bytes live, a std program resolving
through its grant, and the whole fetch over pinned TLS. That fetch follows calef's five rulings on
the index: HTTPS locations with a fallback, a backup address, the channel layout, the root zone.
Since 2026-10-12 `jig update` and `jig install` do it at the prompt; what is left is the run on
xenon.
