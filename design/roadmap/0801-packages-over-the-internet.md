---
status: PARTIAL
raised: 2026-09-19
milestone_dependencies: 384, 494, 501, 809, 855
decision_dependencies: unwritten
machine_requirements: riscv64, aarch64 and x86_64 silicon, each on a wired network that reaches the internet
specific_machine: radon (with an Oracle Cloud Ampere A1 instance and xenon's installed system; calef's exit criterion of 2026-10-24 UTC names the three)
needs_person: yes
---
# 801. Packages over the internet: a host name, HTTPS and the distribution's index

Rung 3c of milestone 198 (a package manager), as a milestone of its own. calef, 2026-10-06 (UTC): *"Don't fix the tooling to name a rung. We should make our milestones finer grained if
we're going to express dependencies on a fraction of them."* Milestone 576 (how many systems are out
there, and what do they run) depends on this rung and on nothing else of 198, and a dependency
field names whole milestones. The title and slug are drafts. `raised` is the day §157 put the rung on the path.

## What it is

On real silicon of each architecture, a package is fetched through `basalt.nifeos.org`'s index
by host name, verified by its digest and installed: radon, an Oracle Cloud Ampere A1 instance and
xenon's installed system (below, "Ruled 2026-10-24"). That is rung 3c of §157 (a trivial install is a web
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

The status is PARTIAL rather than BUILT. Items 1 to 8 are built under QEMU on all three
architectures; none of the three machines has run the exit criterion. Follow-on's `Outstanding`
lines say what is left. The TUF client is 858's.

## Ruled 2026-10-24 (UTC): the exit criterion is three machines

calef widened the exit criterion from xenon alone to real silicon on three machines, in addition to
the QEMU parity CI gives on all three. On aarch64 he offered the cloud host himself: *"On aarch64
need not be argon. We could use a cloud host."* AWS Graviton was set aside, because its NIC is ENA
and nife has no driver for it; an A1's is `virtio-net-pci`, and nife has a virtio-net driver.

| leg | machine | state, 2026-10-24 | waits on | procedure |
|---|---|---|---|---|
| x86_64 | xenon's installed system | rehearsed under QEMU against the live host | 494's bench boot and a fresh install, both on the same evening | `notes/xenon-bench-2026-10-18.md`, steps 1, 2, 4 and 5 |
| riscv64 | radon, booted over the network | cannot pass: no entropy at the prompt, no file service, no wall clock | the proposal `radons-booted-system-runs-jig`, and calef's ruling in it on where the time comes from | `notes/packages/jig-on-radon.md` |
| aarch64 | an Oracle Cloud Ampere A1 instance | cannot pass: the booted system reaches no PCIe device, and has no entropy, disk or clock | the proposal `an-aarch64-cloud-instance-drives-its-pcie-devices`, after milestone 578 (an ACPI discovery path for aarch64)'s item 7 boots the instance | `notes/rented-metal/oracle-a1.md`, section 5b |

The A1 does have a NIC nife can drive, but only once the loader describes the PCI host bridge from
ACPI and the booted system claims a virtio-net over PCIe. Both proposals are in
`design/roadmap/proposals/`.

The second index address is scheduled, and it is calef's hands: he will do basalt's README steps 1
to 6 for `basalt.crickertech.com` and `nifeos/basalt-backup` himself. No leg waits on it.

## What is calef's

Item 2's five questions and the two they raise, A and B, are ruled on #1884, and §250's
amendment of 2026-10-10 (UTC) records A and B (`notes/packages/the-index-format.md` has the
options):

- A. The second index address against §250 clause 1's one fixed name, and clause 3's pin. Ruled
  A1, *"two domains"*: §250 clause 6, one index at up to two addresses on different registrable
  domains. The second domain is `crickertech.com`, chosen 2026-10-21 (UTC), so the image's second
  address is `basalt.crickertech.com`, a second Pages site (clause 6, which also lists the
  failures both addresses share). Its DNS record and Pages settings are calef's hands.
- B. Which root a listed HTTPS location must chain to. Ruled *"X1 only"*: §250 clause 10. Clauses
  11 to 13 (question C, below) let the signed index name another root the image carries.
- The channel's name. `rolling` is provisional, and naming it is calef's (§250 clause 7).
- C. How `jig` reaches a GitHub Release asset, whose download starts at `github.com` (Sectigo, not
  X1) and redirects. Ruled 2026-10-12 (UTC) on #2048, below.

Not a ruling but calef's hands: the three runs. The host is built:
`basalt.nifeos.org` is a CNAME to `nifeos.github.io`, and
`https://basalt.nifeos.org/rolling/metadata/stand-in-index` answers 200, listing every package
against one Release (checked 2026-10-24 UTC, `rolling-2026-10-24-87a3b29`). The bench step is
rehearsed under QEMU from an installed disk built with `NIFE_INDEX_SOURCES=basalt`. `jig update`
resolves `basalt.nifeos.org` by name and reads the index over pinned TLS. `jig install ioping`
fetches the Release asset through `github.com`'s redirect, judges it and installs it
([notes/packages/jig.md](../../notes/packages/jig.md), "A rehearsal with basalt's sources").

One fork was decided rather than asked, because it is reversible and in-tree: how a manifest
names a fixed directory (809's open fork). `Manifest::indexes`, granted like the installer, puts
`activation/indexes` at the directory slot. It lives under `activation/` because it decides what
installs, and because `packages/indexes` could collide with a package named `indexes`.

## Ruled 2026-10-12 (UTC): Pages for the index, Releases for the bytes, and one redirect

The index is served by GitHub Pages at `basalt.nifeos.org` and package bytes are GitHub Release
assets (calef: *"Packages on Releases, index on Pages"*). A listed location names its root and one
redirect is followed (question C, §250 clauses 11 to 13). Both rulings, and what built them, are in
[rulings-of-2026-10-12.md](0801-packages-over-the-internet/rulings-of-2026-10-12.md).

## What it waits on

- Milestones 384, 494 and 501, in the frontmatter, and 809 and 855. Of 809, only items 1 to 6
  (`jig` itself), which are built.
- §250 (an image names its distribution's package index, and the bytes may live anywhere) is
  ruled, and its path was ruled on #1884 (Q4). The `unwritten` decision dependency is now the
  amendments those rulings make. Q3 amends §220 (signed builds: a vendor signs, a developer
  self-signs, and trusting a key is scoped) and §195 (a reviewed recipe vouches for a package).
  The backup address amends §250 clause 1, as clause 6 (question A).
- Not a ruling but calef's hands: the three runs, `needs_person`. xenon's waits on its I219,
  which the kernel refuses until milestone 494's bench boot proves it, and on a fresh install
  (`bench/xenon-2026-10-21/README-801.md`). radon's and the A1's wait on the two proposals.

Reuse: the package client, digest check and installer of milestone 198 are this milestone's
base. The §46 (thin primitives or whole subsystems) survey of index formats is
`notes/packages/the-index-format.md`; the TUF client it points to is taken, not written (858).

## Architectural parity

The capability here is the resolver, the TLS client and fetching the distribution's index over
HTTPS. It ships on aarch64, riscv64 and x86_64, proven under QEMU by the same suite on each, as rung
3a's fetch already is: aarch64 and riscv64 since 2026-09-24, x86_64 since 2026-10-05 (milestone
198's rung table and its "x86_64 fetches" entry). On silicon the exit criterion names one machine
per architecture. radon's NIC is milestone 53 (the board's own peripherals: network and storage on
real silicon)'s network half, which leases; aarch64's machine is a rented A1 instance, not argon.

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
- The test sources give the peer's address, so the gated boot never resolves the index host by
  name at the prompt: slirp's name server is the host's, which knows no `.test` name. basalt's
  sources give none, and `jig` resolves through its root-zone badge (milestone 892); the rehearsal
  above shows it, and the kernel harness gates resolution by name.
- GitHub sets the size of its redirect's head (10,806 bytes on 2026-10-24 UTC), and no gate sees
  it. Past `http_response::REDIRECT_HEAD_MAX` (32 KiB), `jig` passes over the Release and installs
  from the Pages copy, uncounted. `notes/packages/jig.md`'s BUGS name the hand check.
- The index copy is one page: this architecture's lines, at most 4,095 bytes.
- `notes/packages/over-the-internet.md`'s BUGS carry the stand-in's limits (no "moved to", one plain
  HTTP location, no producer in the tree).

- A laptop with no Ethernet port cannot reach this rung. That is milestone 788 (Wi-Fi on a PC that
  has no Ethernet).

## Follow-on

- **Milestone 894.** Milestone 894 (an image carries its distribution's TUF root as a default owner
  trust line):
  `design/roadmap/0894-an-image-carries-its-distributions-root-as-a-default-trust-line.md`: calef's
  Q3, basalt's root SHA-256 as a default owner trust line. The §220 and §195 amendments the ruling
  makes are the integrator's text.
- **Milestone 809.** `jig` holds the root zone (Q5) and installs from any repository whose root is
  an owner trust line with a §220 ceiling: its `add-repository`. That verb waits on §195's
  2026-10-10 amendment, clause 3, and on an unproposed milestone for §306 (the web's authority
  list is a package and a capability). 801 waits on neither: it installs from the image's own
  index under the image's root (checked 2026-10-24 against `jig update` and `jig install`, which
  read only the image's sources).
- **Outstanding.** The x86_64 leg: on xenon, install, boot from the disk, `jig update`, `jig
  install ioping`, as steps 4 and 5 of `notes/xenon-bench-2026-10-18.md`, the evening that also
  runs 494's bench boot and 260's network boot. Checked 2026-10-24: no xenon log under `bench/`
  runs `jig`, and the QEMU rehearsal passes.
- **Outstanding.** The riscv64 leg on radon, `notes/packages/jig-on-radon.md`. It cannot pass until
  the proposal `radons-booted-system-runs-jig` is built (checked 2026-10-24 against
  `bench/radon-2026-10-22/53-ordinary-and-reboot.log`: `entropy : NONE`, no resolver, no file
  service).
- **Outstanding.** The aarch64 leg on an Oracle Cloud Ampere A1 instance,
  `notes/rented-metal/oracle-a1.md` section 5b. It cannot pass until the proposal
  `an-aarch64-cloud-instance-drives-its-pcie-devices` is built; the ACPI-derived tree has no PCI
  host bridge (`uefi_loader/src/device_tree_from_acpi.rs`'s BUGS).
- **Outstanding.** The second index address, scheduled and calef's hands: basalt's README steps 1 to 6 for
  `basalt.crickertech.com`. Checked 2026-10-24: the name does not resolve and `nifeos/basalt-backup`
  has no Pages site. The client's fallback is built and gated against the test peer.
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
- **Done.** A fresh `net_stack` starts its ephemeral ports at a counter-derived point, not 49152
  (CI run 38009701250). Milestone 783 (the network stack seeds its random generator from the
  clock) is where that seed should come from entropy.
- **Done.** Every std program the test harness starts hands back its frames when it ends: the clock
  service, the configuration page and the stack now come from one region a holding reclaims
  (calef, 2026-10-10 (UTC): "Lets fix the leaks."), proved by
  `a_std_program_gives_back_every_frame_it_was_given`. About 56 frames a spawn had been kept for
  the boot.

## Index row

Rung 3c of the trivial install, a milestone of its own so that milestone 576 (how many systems
are out there) can depend on it alone. A package is fetched on silicon through
`basalt.nifeos.org`'s index by host name, over HTTPS, verified by digest and installed. It waits on
the resolver (384), the TLS client (501) and xenon's network card (494). Built under QEMU
2026-10-09 and 2026-10-10: the index split from where the bytes live, a std program resolving
through its grant, and the whole fetch over pinned TLS. That fetch follows calef's five rulings on
the index: HTTPS locations with a fallback, a backup address, the channel layout, the root zone.
Since 2026-10-12 `jig update` and `jig install` do it at the prompt. What is left is the run on
three machines (calef, 2026-10-24): xenon's installed system, radon and an Oracle Cloud A1.
