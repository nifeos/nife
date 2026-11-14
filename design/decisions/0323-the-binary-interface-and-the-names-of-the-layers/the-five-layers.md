# §323 appendix: the five layers

Part of the survey behind [§323 (the binary interface, and the names of the project's
layers)](../0323-the-binary-interface-and-the-names-of-the-layers.md). That page holds the
rules in force. This one holds what each layer's name touches, how reversible
each is, and the candidates. Candidate names here are options, not rulings, and a leaning appears
only where evidence decides it.

## What each name touches today

`git grep` at `nifeos/nife@a3c7d4647` counts lines, not occurrences, except where a row says
matches. The tree has 6,931 tracked files.

| what | lines | files | command |
|---|---|---|---|
| `nife`, any case, as a word | 12,011 matches | 2,613 | `git grep -iow nife` |
| `nife`, any case, as a substring | 20,195 | 3,002 | `git grep -ic nife` |
| `nifeos` | 1,447 | 497 | `git grep -ic nifeos` |
| `github.com/nifeos` | 294 | 135 | `git grep -c github.com/nifeos` |
| git pins to `https://github.com/nifeos` in `Cargo.toml` | 22 | 8 | `git grep -c 'git *= *"https://github.com/nifeos' -- '*Cargo.toml'` |
| `nifeos.org` | 135 | 57 | `git grep -c 'nifeos\.org'` |
| `target_os = "nife"` | 45 | 22 | `git grep -c 'target_os *= *"nife"'` |
| `unknown-nife` (target triples) | 155 | 79 | `git grep -c unknown-nife` |
| `std-nife` | 114 | 86 | `git grep -c std-nife` |
| `nifefs` | 960 | 331 | `git grep -c nifefs` |
| `nifepkg` | 221 | 47 | `git grep -c nifepkg` |
| `nife-dev` (the toolchain link) | 134 | 69 | `git grep -c nife-dev` |
| `NIFE_` (171 distinct identifiers) | 1,945 | 497 | `git grep -c NIFE_` |
| byte literals containing `nife` or `NIFE` | 160 | 63 | `git grep -c -E 'b"[^"]*(nife\|NIFE)' -- '*.rs'` |
| `nife-smelter` and `nife-fuzz` (the automation's App names) | 205 | 70 | `git grep -c -E 'nife-(smelter\|fuzz)'` |
| `basalt` | 1,411 | 267 | `git grep -c basalt` |
| `abi::` | 2,179 | 441 | `git grep -c -E '\babi::'` |

Of the 119 entries under `crates/`, one package name contains `nife` (`nifefs`), and none carries it
as a prefix. `abi` would take the capability binary interface's prefix on publishing, and so might
the 25 `*_contract` crates (`ls -d crates/*_contract`). The 1,784 uses of `abi` in 238 files,
counted for milestone 1075 (`abi`: what an outside maintainer would fix before the move), measure
uses of the crate rather than lines naming the path.

basalt at `nifeos/basalt@ecdfee7` has 32 tracked files. `nife` appears on 448 lines in 26 of
them and `basalt` on 147 lines in 26. No line sets `target_os`, and `pins.toml` holds one pin,
`repository = "https://github.com/nifeos/nife"`.

## How reversible each layer is, and who has acted on it

The test is who else has already acted on a name. Today almost nobody outside the project has.

- **The GitHub organization.** It holds 15 repositories, 14 of them public. `nifeos/nife` has 4
  stars and 1 fork, and no other repository has either. GitHub's documentation on renaming an
  organization says repository links and git remotes redirect. The old profile page and the API
  paths under the old name return 404, though. Anyone may then claim the old name and create
  repositories that override the redirects. The tree's side of the last organization rename took
  one commit across 57 files (`nifeos/nife@acb285e00`).
- **The domain.** whois gives `nifeos.org` a registration date of 2026-10-03. `basalt.nifeos.org`
  serves basalt's GitHub Pages site. §250 (an image names its distribution's package index, at
  `basalt.nifeos.org`) puts that host name in every image, and `capability_manifest_note`'s
  examples use it as a zone. An installed image asks that name for as long as it runs. The project
  can add a domain beside it but cannot retire it without stranding those images. Nobody outside
  the project runs nife yet, so a change costs least now.
- **crates.io.** Nothing has gone out, and publishing stays on hold (ruled 2026-11-06). crates.io
  has no rename. Its policy allows a deletion only within 72 hours of publishing, or for a crate
  with one owner, fewer than 500 downloads a month and no dependents. A rename means a second crate
  and a deprecation notice on the first, as Hermit did with `hermit-sys`. One-way.
- **Rust's `target_os`.** It lives only in this tree's three target specifications and the `std`
  overlay. §235 (the OS is built and updated from packages, and its source divides by purpose)
  sets an upstream tier 3 target as the destination. The Rust target tier policy says a name must
  not introduce "undue confusion". It adds that "changing the name of a target can be highly
  disruptive, especially once the target reaches a higher tier, so getting the name right is
  important even for a tier 3 target." Before upstream, a rename is a tree-wide edit. After it, a
  rename reaches the Rust project and every crate that matches on the name, `libc` and `getrandom`
  among them. One-way from the first upstream merge.
- **Names inside binaries.** Programs and boot code read the ELF note owner `nife`, `NIFESLOT`,
  `NIFESTK1` and the partition GUID. Milestone 120 (the rename: the OS becomes `nife`, and the
  project gets an organization) left the on-disk magic `CRKR0002` and the GUID alone, because
  existing images carry them. The same reasoning holds for any later rename. These need not follow
  a name change, and each layer's ruling should say whether they do.
- **Release artifacts.** basalt has 57 releases (`rolling-<date>-<commit>`) with 1,153 assets, and
  their download counters sum to 171,035 (`gh api repos/nifeos/basalt/releases`). The counters
  cannot tell automation from people. Asset names carry the package, version and architecture
  (`ripgrep-14.1.1-x86_64.nifepkg`), not the distribution.

## How the candidates were checked

All checks ran on 2026-11-12. crates.io answered through `https://crates.io/api/v1/crates/<name>`
with the User-Agent `nifeos-maintainer-check`. GitHub answered through `gh api users/<name>`, which
covers organizations too. A web search looked for an existing project or mark of the same name.
A bare English or geological word nearly always belongs to some GitHub account already. Of the 23
names checked there, only suffixed forms came back free: `nife-os`, `craton-os`, `quarry-os` and
`suessos`.

## Layer 1: the organization

It touches the 294 `github.com/nifeos` lines, the 22 Cargo pins, basalt's `pins.toml`, 15
repositories, and the domain, and through the domain every image (§250).

Ruled 2026-11-14 (UTC), ruling 3: the organization stays `nifeos` for now. Ruling 4, the same
day, gives it the library's name (layer 2), with the domain that name plus `.org`, chosen before
the first crates.io publish. The options below are the candidates for that one name.

| option | what it says | checks | weakness |
|---|---|---|---|
| Keep `nifeos` | the nife operating system | ours on GitHub; crates.io `nifeos` free; the domain ours | names the kernel of one binary interface; milestone 120 kept the organization apart from the OS for this reason, and an ambient-authority binary interface under it would read as a subsidiary of the capability one |
| A publisher name tied to no binary interface, the library's (layer 2) | the maker of the library | depends on the word | a second organization rename within two months; others may claim the old name; the old domain stays, or the images strand |
| OSOS | open source operating systems; one name for the organization and the library | not yet run; they come back before any rename | availability unknown until the checks run |
| `nife` | the core everything rests on | GitHub `nife` belongs to a user (0 repositories, created 2010); others hold `nife.org` and `nife.dev` | not available without that user; `nife` now names one kernel (layer 4) |

`nife` now names today's kernel, the capability microkernel, so `nifeos` names the organization
after one kernel.

## Layer 2: the library of components

It touches almost nothing in code. Crates take the name of what they do (`file_allocation_table`,
`e1000e`), and the rule for a leaving crate says it never mentions nife. Milestone 1082
(`capability_manifest_note`: what an outside maintainer would fix before it leaves) states that
rule. So the library's name lives in the website, the organization profile and README text. It is
not a platform: the binary interface is.

Ruled 2026-11-14 (UTC), ruling 4: the library and the organization share one name, the
organization expects to take it, and the domain is that name plus `.org`. The architect chooses the
name before the first crates.io publish, after its checks. Crate names stay descriptive, and the
page contracts, which every binary interface shares, take no common prefix. The project can add a
prefix later, but it cannot take one away. The options below are the candidates for the shared
name.

| option | what it says | checks | weakness |
|---|---|---|---|
| OSOS (layer 1) | open source operating systems | not yet run | availability unknown until the checks run |
| `craton` | the old, stable core of a continent, on which the rest grows | crates.io free; GitHub `craton` a user (1 repository), `craton-os` free; others hold `craton.org` and `craton.dev`; an OpenStack inventory project shares the name (`python-cratonclient`) | a craton is neither core nor rock nor mineral, so it breaks milestone 120's tiers |
| `quarry` | where every distribution's stone comes from | crates.io `quarry` taken (0.2.0, 729 downloads, "mining type information"); GitHub `quarry` a user, `quarry-os` free; `quarry.org` parked | a verb as well as a noun; the crate name has gone |
| `lithosphere` or `outcrop` | the rock layer; where rock shows at the surface | crates.io free; GitHub users hold both (0 repositories) | long; no tie to components |

Genode has the same shape: Genode Labs publishes the Genode OS Framework. Nothing in the tree
depends on the name.

## Layer 3: binary interfaces

This layer carries the expensive names: `target_os`, the published ABI crate, the `std` port's
directory and triple, the SDK crates and the in-binary names. Only a binary interface the project
defines needs a name; one it adopts, such as Linux's, already has one.

Every published ABI crate names its system: `sel4-sys`, `fuchsia-zircon-sys`, `hermit-abi`,
`wasi`, `r-efi`, `redox_syscall`.

| binary interface | name | ABI crate | status |
|---|---|---|---|
| capability | Moho | `moho_abi` | ruled 2026-11-14 (UTC); the main page holds the checks and what moves when |
| ambient-authority | open | open | named in a later ruling |

The ambient-authority binary interface needs no name until the project defines one rather than
adopting Linux's. The tree already holds two pairs of candidates, and milestone 120 turned both
down for other uses.

- `sial` and `sima`, Suess's names for the crust. crates.io `sial` is free and `sima` taken (a
  string matcher, 795 downloads). GitHub users hold both. SiMa.ai makes edge-AI chips, and SIAL
  Paris is a large food trade fair. Milestone 120 turned them down for the distribution because
  they needed a pronunciation note.
- `kamacite` and `taenite`, the two nickel-iron phases in meteoric iron. crates.io has neither, and
  GitHub users hold both. Milestone 120 keeps them for components.

## Layer 4: individual kernels

A kernel's name touches its repository, its image file name and its boot log, and no program sees
it. `nife` is the name of today's kernel, the capability microkernel (ruled 2026-11-14 UTC, with
ruling 2).

Ruled 2026-11-14 (UTC), ruling 6: each kernel has a name of its own, never a qualified form of
another's. `nife` stays the capability microkernel's name, and the capability monokernel and the
ambient-authority kernels take theirs when they exist. The options weighed were these.

| option | what it says | checks | weakness |
|---|---|---|---|
| Keep `nife-kernel` for the microkernel's repository (ruled 2026-11-03, not created) and add a named monokernel beside it | the kernel's name, then what it is | crates.io `nife_kernel` free; the repository name free | `nife-kernel` reads as the project's only kernel once a second exists |
| `nife` for the microkernel and a proper name for each other kernel, such as `kamacite` and `taenite` | each kernel a word of its own | as layer 3 | takes the names milestone 120 keeps for components |
| `nife-microkernel` and `nife-monokernel` | one name and the shape | crates.io `nife_micro` and `nife_mono` free; both repository names free | makes the monokernel a variant of nife rather than a kernel of its own |

The ruling is a variant of the second row: each further kernel takes a word of its own, chosen
when it exists, with its availability checked before it is first published.

## Layer 5: distributions

It touches the repository `nifeos/basalt` and its mirror `nifeos/basalt-backup`, the index host
`basalt.nifeos.org` that every image names (§250), the Pages site, and 1,411 lines in nife and 147
in basalt. Release tags and asset names do not carry the distribution's name, so no published
artifact changes with it. crates.io `basalt` belongs to a Vulkan UI framework (60,108 downloads),
and GitHub `basalt` to another organization (7 repositories). Neither matters unless the
distribution publishes a crate or leaves the organization.

Prior art, read unless marked:

- Debian's ports page lists GNU/kFreeBSD as "the first non-Linux port of Debian as a technology
  preview". It now reads "dead", and Debian dropped it in release 8. GNU/Hurd remains a port. A
  distribution can span kernels, at the cost of building every package per port, and here the
  second kernel did not last.
- TrueNAS spans two kernels under one name: CORE on FreeBSD and SCALE on Debian Linux ("TrueNAS
  is now officially multi-OS"). FreeNAS became TrueNAS CORE, so a product name can outlive a change
  of kernel underneath it.
- FreeBSD keeps "a single repository for all of its components, both kernel and userspace", so its
  distribution and its OS are one thing. GhostBSD builds a desktop distribution on it (from
  memory).
- Ubuntu builds on Debian, and Fedora feeds CentOS Stream, which feeds RHEL (from memory; the
  Fedora page refused the fetch). One distribution can serve as another's base.
- A Fuchsia build combines one board (the hardware) with one product (the software configuration:
  `bringup`, `core`, `minimal`, `workbench` and others). A product is a distribution over one
  platform.
- Yocto calls Poky "a reference embedded distribution and a reference test configuration", "not a
  product level distro", and a starting point for others. basalt plays that role here: the
  first-party distribution that tests the components.

A distribution built for one binary interface runs on any kernel that serves it. It belongs to
that binary interface and picks a kernel per image, as a Fuchsia product binds one platform. It can
span binary interfaces, as Debian's ports show, but then builds every package once per binary
interface.

| option | what it says | checks | weakness |
|---|---|---|---|
| Keep `basalt` | the commonest volcanic rock and the workhorse; the rock that sima is (milestone 120) | as above; the repository and the host belong to the organization | none found in the tree; says nothing of the binary interface |
| A rock for each binary interface, keeping the convention | the distribution's character | each candidate needs its own check | moves the index host in every image, which the project can only add beside the old one |

Milestone 120's convention needs no change for several binary interfaces: distributions are rocks,
with no limit on their number, named by whoever makes one, and only `basalt` reserved.

Ruled 2026-11-14 (UTC), ruling 5: `basalt` stays the first-party distribution, for the Moho binary
interface. It builds images on either capability kernel and picks the kernel per image. A
distribution for the ambient-authority binary interface takes its own name when one exists. No
name changes, and the index host that every image names stays valid.