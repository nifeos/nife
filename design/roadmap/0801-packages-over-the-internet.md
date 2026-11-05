---
status: PARTIAL
raised: 2026-09-19
milestone_dependencies: 384, 494, 501, 809, 855
decision_dependencies: 195, 220, 250
machine_requirements: riscv64, aarch64 and x86_64 silicon, each on a wired network that reaches the internet
specific_machine: radon (with an Oracle Cloud Ampere A1 instance and xenon's installed system; the exit criterion names the three)
needs_person: yes
---
# 801. Packages over the internet: a host name, HTTPS and the distribution's index

Rung 3c of milestone 198 (a package manager), as a milestone of its own. Milestone 576 (how many
systems are out there, and what do they run) depends on this rung and on nothing else of 198. A
dependency field names whole milestones, so the rung is its own block rather than a fraction of
198's. The title and slug are drafts. `raised` is the day §157 put the rung on the path.

## What it is

On real silicon of each architecture, a package is fetched through `basalt.nifeos.org`'s index
by host name, verified by its digest and installed: radon, an Oracle Cloud Ampere A1 instance and
xenon's installed system. That is rung 3c of §157 (a trivial install is a web page, a USB drive, and
packages over the internet), which [milestone 198's rung table](0198-package-manager.md#the-rungs)
still maps.

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

Item 2's five questions, ruled on #1884, add four items:

5. Q1: each target may list HTTPS locations, tried in order before the repository's own
   `targets/`; a listed location never reaches a private or link-local address; an owner may pin
   one mirror instead. Built in `package_index` and the client.
6. Q2: the image carries a second index address, tried when the first stops answering. Built in
   the client; §250 clause 6 is its wording (question A below).
7. Q4: the index at `/<channel>/metadata/`, packages at `/<channel>/targets/`, one channel per TUF
   repository, all three architectures in one index. Built; the channel name is provisional.
8. Q5: the client holds the root zone. Built in the test's grant. Q3 is milestone 894 (an image
   carries its distribution's TUF root as a default owner trust line).

## Where the index and the bytes live

- **The index is served by GitHub Pages; package bytes are GitHub Release assets.** The index
  (`/rolling/metadata/`) is a project site of `nifeos/basalt` at `basalt.nifeos.org`, a CNAME to
  `nifeos.github.io`, and not the organization's Pages site, which `nifeos/nifeos.github.io` serves
  as `www.nifeos.org`. Its certificate chains to ISRG Root X1 (measured 2026-10-13), so §250's pin
  holds unchanged. Each publish makes one Release, and each index line lists its asset as a
  location, which §250 clause 2 allows. An asset's `download_count` is the first adoption signal
  for milestone 576. basalt's `publish` job (nifeos/basalt#13) builds both, and keeps clause 9's
  copy under `targets/` on Pages as the fallback when the Release cannot be reached. The other
  hosts on offer were a VPS running Caddy, Cloudflare, and S3 with CloudFront; no tree record gives
  a reason against them.
- **A. The second index address.** §250 clause 6: one index at up to two addresses on different
  registrable domains. The second is `basalt.crickertech.com`, a second Pages site, and clause 6
  lists the failures both addresses share.
- **B. The root a listed HTTPS location chains to.** X1, §250 clause 10, unless the signed index
  names another (question C).
- **C. How `jig` reaches a Release asset**, whose download starts at `github.com` (Sectigo, not
  X1) and redirects. §250 clauses 11 to 13, option C2 in `notes/packages/the-index-format.md`:
  - The signed index may name, per listed location, the root its certificate must chain to, among
    the roots the image carries: `<root>=https://...` in the stand-in encoding. A location naming
    none chains to the index's root. The image carries further roots on `location-root` lines in
    its sources; basalt's names `sectigo-root-e46`, the root `github.com` chains to.
  - A fetch from a listed location follows exactly one redirect. It must be an absolute HTTPS URL,
    its host is held to Q1's public-address check and to the index's root, and a second is refused.
  - The digest check is unchanged.
- **A manifest names a fixed directory** with `Manifest::indexes` (809's fork, reversible and
  in-tree), granted like the installer. It puts `activation/indexes` at the directory slot, under
  `activation/` because it decides what installs and because `packages/indexes` could collide with
  a package named `indexes`.

## Built under QEMU: the fetch, and item 2's survey

`notes/packages/over-the-internet.md` has the account, the transcripts and the seams.

- Item 1. `crates/package_index` (provisional) reads an index, finds the one entry a name asks for,
  and admits fetched bytes through `package_archive::installable_as` with the entry as a one-line
  catalog, the installer's own check. The encoding is a stand-in until the TUF client exists.
- Item 3. A std program's `ToSocketAddrs` asks the resolver badge at
  `std_runtime_contract::RESOLVER_SLOT` (9, provisional), keeping each refusal's reason.
  `a_std_program_resolves_its_granted_zone_and_nothing_without_a_grant` gates it on all three
  architectures in CI, falsified once.
- Items 4 to 8. `package_fetch_exerciser` (provisional) finds the image's first index address gone
  and reads the index from the second, by name, over TLS pinned to the test root. It passes over a
  listed location that resolves to a private address, fetches `greeting` from the repository's own
  `targets/` and admits it by the index's digest, and refuses an altered copy. Green on all three
  architectures, and in CI since milestone 855 (the TLS graph enters the gated build).
- Item 2. The format family is TUF (§250's amendment and milestone 858 (lab machines update
  themselves through packages)'s Fork 9); `notes/packages/the-index-format.md` has the rulings on
  what TUF leaves open.
- Question C. `pinned_tls_client::listed` is the fetch, which `jig` and `package_fetch_exerciser`
  both call. `package_index` reads the root token, `Sources::root_for` chooses it, and
  `pinned_tls_client::SECTIGO_ROOT_E46` is the root, checked byte for byte against Sectigo's and
  macOS's copies. An ignored host test fetched ripgrep's Release asset through it: `github.com`
  under E46, a `302` with a 931-byte path, then the bytes from an X1 host.
  `a_package_is_fetched_through_the_index_by_name_over_tls_and_judged_by_its_digest` runs six
  redirect probes on all three architectures: one good redirect fetched and admitted, and five
  refused (an unnamed root, a root the image does not carry, a second redirect, a private address,
  a host off the index's root). Falsified on aarch64 twice: a redirect pinned to the location's
  root fetched from the off-root host, and two redirects allowed fetched through both.
- Every std program the test harness starts hands back its frames when it ends: the clock service,
  the configuration page and the stack come from one region a holding reclaims, proved by
  `a_std_program_gives_back_every_frame_it_was_given`.

## Built under QEMU: `jig` installs from the index at the prompt

This is the exit criterion short of silicon, on all three architectures, in `script/swish-check`'s
second boot. [notes/packages/jig.md](../../notes/packages/jig.md) has the transcript and the BUGS.

- The image names its index. A measured archive entry, `package_index::SOURCES_ENTRY`, lists the
  index's addresses in the order tried and the one root their TLS must chain to (`root
  isrg-root-x1`, or a certificate in hex). basalt can rewrite it without rebuilding `jig`
  (milestone 909 (basalt composes the bootable image from its packages)). The production sources
  are `package_index::BASALT_SOURCES`, packed when an image is built with
  `NIFE_INDEX_SOURCES=basalt`.
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
- The host answers. `basalt.nifeos.org` is a CNAME to `nifeos.github.io`, and
  `https://basalt.nifeos.org/rolling/metadata/stand-in-index` answers 200, listing every package
  against one Release (checked 2026-10-24 UTC, `rolling-2026-10-24-87a3b29`). From an installed disk
  built with `NIFE_INDEX_SOURCES=basalt`, `jig update` resolves `basalt.nifeos.org` by name and
  reads the index over pinned TLS, and `jig install ioping` fetches the Release asset through
  `github.com`'s redirect, judges it and installs it ([notes/packages/jig.md](../../notes/packages/jig.md),
  "A rehearsal with basalt's sources").

The status is PARTIAL rather than BUILT. Items 1 to 8 are built under QEMU on all three
architectures; none of the three machines has run the exit criterion. Follow-on's `Outstanding`
lines say what is left.

## The exit criterion is three machines

Real silicon on three machines, in addition to the QEMU parity CI gives on all three. AWS Graviton
is refused for aarch64: its NIC is ENA and nife has no driver for it, while an A1's is
`virtio-net-pci`, which nife drives.

| leg | machine | state, 2026-10-24 | waits on | procedure |
|---|---|---|---|---|
| x86_64 | xenon's installed system | rehearsed under QEMU against the live host | 494's bench boot and a fresh install, both on the same evening | `notes/xenon-bench-2026-10-18.md`, steps 1, 2, 4 and 5 |
| riscv64 | radon, booted over the network | cannot pass: no entropy at the prompt, no file service, no wall clock | milestone 966 (radon's booted system runs `jig`), and the ruling in it on where the time comes from | `notes/packages/jig-on-radon.md` |
| aarch64 | an Oracle Cloud Ampere A1 instance | cannot pass: the booted system reaches no PCIe device, and has no entropy, disk or clock | milestone 964 (an aarch64 cloud instance drives its PCIe devices), after milestone 578 (an ACPI discovery path for aarch64)'s item 7 boots the instance | `notes/rented-metal/oracle-a1.md`, section 5b |

## What is calef's

- The channel's name. `rolling` is provisional, and naming it is calef's (§250 clause 7).
- The three runs, `needs_person`.
- The second address's DNS record and Pages settings: basalt's README steps 1 to 6 for
  `basalt.crickertech.com` and `nifeos/basalt-backup`. No leg waits on it.

## What it waits on

- Milestones 384, 494 and 501, in the frontmatter, and 809 and 855. Of 809, only items 1 to 6
  (`jig` itself), which are built.
- §250 (an image names its distribution's package index, and the bytes may live anywhere) is
  ruled, with its path (Q4) and clauses 6 and 10 to 13. Q3 amends §220 (signed builds: a vendor
  signs, a developer self-signs, and trusting a key is scoped) and §195 (a reviewed recipe vouches
  for a package); that text is milestone 894's.
- xenon's run waits on its I219, which the kernel refuses until milestone 494's bench boot proves
  it, and on a fresh install (`bench/xenon-2026-10-21/README-801.md`). radon's and the A1's wait on
  milestones 966 and 964.

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
- A fresh `net_stack` starts its ephemeral ports at a counter-derived point, not from entropy.
  Milestone 783 (the network stack seeds its random generator from the clock) is where that seed
  should come from entropy.
- A laptop with no Ethernet port cannot reach this rung. That is milestone 788 (Wi-Fi on a PC that
  has no Ethernet).

## Follow-on

- **Milestone 894.** Milestone 894 (an image carries its distribution's TUF root as a default owner
  trust line):
  `design/roadmap/0894-an-image-carries-its-distributions-root-as-a-default-trust-line.md`: Q3,
  basalt's root SHA-256 as a default owner trust line. The §220 and §195 amendments the ruling
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
  milestone 966 (radon's booted system runs `jig`) is built (checked 2026-10-24 against
  `bench/radon-2026-10-22/53-ordinary-and-reboot.log`: `entropy : NONE`, no resolver, no file
  service).
- **Outstanding.** The aarch64 leg on an Oracle Cloud Ampere A1 instance,
  `notes/rented-metal/oracle-a1.md` section 5b. It cannot pass until milestone 964 (an aarch64
  cloud instance drives its PCIe devices) is built; the ACPI-derived tree has no PCI
  host bridge (`uefi_loader/src/device_tree_from_acpi.rs`'s BUGS).
- **Outstanding.** The second index address, calef's hands: basalt's README steps 1 to 6 for
  `basalt.crickertech.com`. Checked 2026-10-24: the name does not resolve and `nifeos/basalt-backup`
  has no Pages site. The client's fallback is built and gated against the test peer.
- **Milestone 858.** The TUF client and verifier, items 4 and 5 of its work list, replace
  `Index::parse`. Who signs a target's locations is decided with the signing setup (Q1), and so is
  following a root that names a new location (Q2).
- **Milestone 892.** Milestone 892 (a std program at the prompt holds the network):
  `design/roadmap/0892-a-std-program-at-the-prompt-holds-the-network-and-a-resolver.md`: the booted
  system starts the resolver from the lease, and a std program at the prompt is granted the network
  and a resolver zone. `jig` needs it to reach the index by name.

## Index row

Rung 3c of the trivial install, a milestone of its own so that milestone 576 (how many systems
are out there) can depend on it alone. A package is fetched on silicon through
`basalt.nifeos.org`'s index by host name, over HTTPS, verified by digest and installed. It waits on
the resolver (384), the TLS client (501) and xenon's network card (494). Built under QEMU: the index
split from where the bytes live, a std program resolving through its grant, and the whole fetch
over pinned TLS. The fetch has HTTPS locations and a fallback, a backup address, the channel layout
and the root zone. The index is on GitHub Pages and the bytes on Releases, reached through one redirect.
`jig update` and `jig install` do it at the prompt. What is left is the run on three machines:
xenon's installed system, radon and an Oracle Cloud A1.
