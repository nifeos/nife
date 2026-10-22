# Packages that build the OS, and divisions that release together: the measurements

The evidence behind [§235 (the OS is built and updated from packages, and the tree divides by what
releases together)](../design/decisions/0235-packages-build-the-os-and-the-tree-divides-by-release.md),
written for milestone 607 (nife is built and updated from packages). Both numbers are provisional. The decision holds the options; this note
holds what was measured and what the prior art does. Measured 2026-09-26 (UTC) at base `de0c3664b`.
The file name is a lane's coinage and provisional.

## The question

Four records touch this and none of them joins the others. Milestone 39 (repository structure for a loosely-coupled OS) recorded a monorepo now and
a split later. §151 (the goal of the repository split is independent release) made the split's goal independent release and third-party programs. Milestone 198 (a package manager)
installs programs but does not build the OS. Milestones 525 and 554 swap a whole image between two
boot slots. Nothing says how a package becomes part of the OS, or where the repository's seams are.

## The dependency graph, divided

`cargo metadata --no-deps --offline` over the root workspace lists 71 packages. Seven more workspaces
sit beside it (`redoxfs_server`, `cryptography_provider`, `entropy_backend`, `std_exerciser`,
`cryptography_exerciser`, `fuzz`, and `tools/redoxfs_host`). Every crate under `crates/` is version
`0.1.0`, all 80 of them, and none says `publish = false`.

Each package was put in one candidate division by what it is for. The division names are this lane's
and provisional.

| division | packages | what is in it |
|---|---|---|
| kernel | 19 | `kernel` and the crates only it uses: `capability`, `paging`, `page_frames`, `pci`, `device_tree_blob`, `machine_discovery` and 13 more |
| contracts | 27 | `abi`, every `*_protocol`, and the formats two programs agree on: `elf`, `measured_boot`, `package_archive`, `installed_programs`, `boot_slot`, `nifefs`, the partition table, `grant_plan`, `component_plan`, `compositor`, `line_editor` |
| runtime | 2 | `user_mode_runtime`, `allocation_cache`; with them the `std` overlay (`patches/std-nife`, 4,486 lines) and `targets/` |
| boot | 6 | `uefi_loader`, `boot_ladder`, the two consoles, `bitmap_font`, `sealed_pair` |
| services | 22 | `components` (50 binaries), `system_initializer`, drivers, `swish`, `ps` and its siblings, `calendar`, `glob` |
| fixtures | 6 | `fixtures` (44 binaries), `coremark`, `job_mix`, `soak_page`, `c_seam`, `loaded_image_check` |
| host | 3 | `xtask`, `portable_executable`, `stick_maker`; with them `script/` and `helpers/` |

Proofs are not a division. Kani harnesses live in 28 places, each beside the code it proves.
Splitting them out would separate a proof from its subject, which is the one thing the verification
claim cannot survive.

### Edges between divisions

Normal and build dependencies only, counted per package pair, dependent first.

| edge | count | reading |
|---|---|---|
| services to contracts | 49 | the intended direction |
| kernel to contracts | 23 | intended |
| fixtures to contracts | 19 | intended |
| host to contracts | 10 | intended |
| kernel to services | 9 | not intended: `ps`, `pgrep`, `pmap`, `video_terminal`, `network_time_protocol`, `block_roster`, `non_volatile_memory_express`, `calendar`, `jh7110_entropy` |
| kernel to fixtures | 3 | not intended: `coremark`, `job_mix`, `soak_page` |
| fixtures to services | 3 | `system_initializer`, `schedule_store`, `network_time_protocol` |
| contracts to runtime | 2 | `supervision_protocol` and `hot_swap_contract` link `user_mode_runtime` |
| boot to kernel | 3 | `device_tree_blob`, `machine_discovery`: leaf libraries, not the kernel |
| boot to fixtures | 1 | `serial_console` links `job_mix` |

Contracts depend on almost nothing. Runtime depends only on contracts. Those two are the boundary a
third party crosses, and they are nearly clean today.

The kernel is the exception, and the reason is one directory. `kernel/src/user/` is 82 files and
28,441 of the kernel's 89,885 lines. It holds the userspace bring-up (each `*_service.rs` knows a
service's grants and budgets) and the whole-system test suite. The kernel crate is three things: the
kernel, the integrator, and the test harness.

### How often a change crosses a division

`git log --no-merges --since='60 days ago' --name-only`, back to 2026-07-28: 4,055 commits. Paths were
mapped to divisions, and documentation was left out.

| measure | value |
|---|---|
| commits touching code | 1,794 |
| touching more than one division | 486 (27%) |
| the same, ignoring fixtures and host (test wiring) | 286 of 1,220 (23%) |
| contracts commits that touch another division | 76% of 215 |
| services commits that touch another division | 71% of 294 |
| kernel commits that touch another division | 43% of 916 |
| `kernel/src/user/` commits that touch another crate | 227 of 431 (53%) |
| the rest of `kernel/` that touches another crate | 94 of 392 (24%) |

The most common pairs were host with kernel (274), kernel with services (153) and contracts with
kernel (121). The contracts figure is the one §151 names as a precondition. A contract still changes
together with its consumers three times in four. That is the churn §151 says must stop before the
order of a split can be ruled.

Checkpoint commits inflate every count, since the tree squashes only before a merge. The ratios
matter more than the totals.

## What pins the image together

The kernel measures one program, the progenitor, and one table, `PROGRAM_MEASUREMENTS`. The build
writes both digests to `target/init-measure-<arch>.txt`, and `kernel/build.rs` compiles them into
`TRUST_ROOT` in the kernel's `.rodata`. Its comment says what that costs: the kernel relinks
whenever userspace changes. The progenitor then refuses to endow any program missing from the
table, the rule milestone 104 (the measurement continues past init) added.

So a kernel binary is a function of every base program's bytes. This is a hash chain, the property
Nix gets from a closure and Fuchsia from a Merkle root in the boot arguments. It is also why the
kernel cannot be released on its own while the root is compiled in. §151 calls the kernel "one
independently-released component", and today the build makes it the last link of the image.

Milestone 450 (a signature over the init image) was refused for this reason in reverse. A signature
would let the image change without the kernel changing, at the price of keys and verification code
in the trusted computing base. §220 (signed builds) since allowed signatures at install time for
packages, checked once and then pinned by digest, and it keeps them out of the boot path.

## What a third party needs today

basalt's package build (`recipes/ripgrep.toml` and `helpers/packages.py` in `nifeos/basalt`) is
the one out-of-tree build. It needs the target file, the linker
script, the patched `nife-dev` toolchain made by `xtask std-src`, and `-Zbuild-std`. The `std`
overlay reaches six crates: `abi`, `user_mode_runtime`, `filesystem_contract`,
`byte_sink_contract`, `clock_contract` and `counter_frequency_contract`. With their own
dependencies the closure is ten crates and about 24,400 lines, plus the 4,486-line overlay. That
closure is the SDK. It is small, and it sits inside the two cleanest divisions.

## How big an image is

The last x86_64 boot image built in the main checkout, `target/esp/EFI/BOOT/BOOTX64.EFI` on
2026-09-17, is 10,158,080 bytes: loader, kernel and archive in one file. The archives were 4.7 MB
(x86_64), 8.8 MB (riscv64) and 10.2 MB (aarch64) when last built. These are stale local builds,
kept as an order of magnitude only. A whole-image update is about ten megabytes. Any option that
saves bandwidth by shipping per-package deltas is saving on that figure.

## Prior art, read 2026-09-26

Read from primary sources by a research subagent; the URLs are listed below. Anything it could not
verify is marked from memory.

### Fuchsia

A package is a `meta.far` naming files by Merkle root, and every file is a blob in blobfs, named by
its root and verified on read. Base packages are pinned by hash in the `system_image` package, which
the boot arguments name by hash. An update package lists every package plus the kernel image; the
updater fetches blobs, writes the inactive slot, and a committer marks it healthy after reboot. The
SDK (the IDK) is a tarball, and each component manifest carries an ABI revision the platform checks.

Take: the image as a hash-pinned list of packages, which keeps the slot atomic while packages
become the unit of transfer. Take: an ABI revision in each program's manifest, which is where
milestone 597 (a program carries its manifest in an ELF note) already puts it, as a field ruled 2026-09-27. Refuse for now: TUF and the universe tier. One digest per image,
plus §220's per-source keys, cover one vendor and no mirrors.

### Nix and NixOS

A store path is named by a hash, and a system is one closure. Each rebuild is a generation, and the
bootloader lists every generation. Binary caches are trusted by signature unless the path is content
addressed. There is no SDK; a third party writes a derivation against nixpkgs.

Take: the closure as the meaning of "what this image needs", which `TRUST_ROOT` already is. Refuse:
input-addressed names, which need a signature for everything, and the source tree as the SDK, which
is the gap milestone 198's `BUGS` records.

### FreeBSD pkgbase

The base system becomes ordinary `pkg` packages, split by consumer into runtime, `-lib`, `-dev` and
`-dbg`, and grouped into sets such as `minimal`. FreeBSD 15.0 offers it as a technology preview.
Atomicity comes from ZFS boot environments, not from `pkg`, and two update paths run for a whole
stable branch.

Take: a `-dev` split as the way a third party gets interfaces without the source tree. Refuse: base
packages upgraded in place on the device, and two update paths at once. nife already has the slot
that FreeBSD borrows from ZFS.

### ChromeOS

The unit is the partition. Two kernel and root pairs carry priority, tries and successful bits,
which milestone 525 (a bad upgrade cannot brick the machine) copied. dm-verity's root hash sits on the signed kernel command line, so one
signature covers the whole root. Downloadable content (DLC) is a verified image the OS build carves
out, version-locked to the OS. There is no SDK for the OS itself.

Take: full images as the floor and deltas as an optimization that falls back. Take: DLC's rule that
an optional piece of the base is version-locked to the image. Refuse: block-level deltas, which
need a byte-identical source partition; content addressing gets blob-level deltas without that.

### OSTree, rpm-ostree and bootc

A commit is a complete bootable tree in a content-addressed store, and deployments hard-link from
it. Signing is per commit and can be checked at boot. rpm-ostree layers RPMs onto the image on the
client, and new work has moved to bootc, which ships the OS as an OCI image.

Take: one content-addressed store behind several bootable deployments, which is what two slots
become if they share blobs. Refuse: client-side layering, which rebuilds the image per device, and
OCI, which brings a Linux toolchain nife does not have.

### Genode's depot, and seL4

The depot keeps `api`, `src`, `raw`, `bin` and `pkg` archives per user and version. An `api` archive
holds headers and symbol files and depends on nothing. A `src` archive depends only on `api`
archives, so a library fix never forces a dependent rebuild. Publishing signs each archive, and a
download waits in quarantine until its signature checks. seL4's Microkit ships a per-board SDK and a
tool that builds one loader image; it has no packages and no updates.

Take: the `api` archive, which is rule 7's contract crate under another name. Take the rule that
implementations depend only on contracts, never on each other; the edge table above shows where
nife breaks it. Refuse: GPG, which §220's key shape already replaces.

### Translated for a capability system

A package on these systems is a file set placed into a shared namespace. On nife, §208 (installing a package is granting it) already says
installing is granting: a package becomes spawnable and its data a read-only directory a session
binds by name. So the store the prior art shares between deployments is, here, a set of objects the
progenitor may grant from. A capability system gains one thing the others do not have. A blob can
be verified at the moment it is granted, not only when it is written, because the grant is the
only path to it.

Sources: [Fuchsia packages](https://fuchsia.dev/fuchsia-src/concepts/packages/package), [OTA](https://fuchsia.dev/fuchsia-src/concepts/packages/ota), [blobfs](https://fuchsia.dev/fuchsia-src/concepts/filesystems/blobfs), [RFC-0002](https://fuchsia.dev/fuchsia-src/contribute/governance/rfcs/0002_platform_versioning); [Nix store paths](https://nix.dev/manual/nix/stable/store/store-path), [NixOS rollback](https://nixos.org/manual/nixos/stable/#sec-rollback); [FreeBSD 15.0 release notes](https://www.freebsd.org/releases/15.0R/relnotes/), [freebsd-base(7)](https://man.freebsd.org/cgi/man.cgi?query=freebsd-base&sektion=7&manpath=FreeBSD+15.0-RELEASE); [ChromeOS disk format](https://www.chromium.org/chromium-os/developer-library/reference/device/disk-format/), [update_engine](https://chromium.googlesource.com/aosp/platform/system/update_engine/+/HEAD/README.md), [DLC](https://chromium.googlesource.com/chromiumos/platform2/+/HEAD/dlcservice/docs/developer.md); [OSTree](https://ostreedev.github.io/ostree/introduction/), [bootc](https://bootc.dev/bootc/); [Genode package management](https://genode.org/documentation/genode-foundations/24.05/development/Package_management.html), [Microkit](https://docs.sel4.systems/projects/microkit/).

## BUGS

- The division assignment is a judgment per crate, not a measurement. A crate such as `calendar`
  or `glob` is a leaf library that any division could own; moving it moves a few edges.
- Commit counts include checkpoints, and the path mapping puts `script/` and `.github/` in host.
- The image sizes are stale local builds from 2026-09-10 to 2026-09-21, not a fresh build. Lanes
  gate in CI, and this lane built nothing.
- Fuchsia's TUF key handling, the Nix signature fingerprint and ChromeOS's per-DLC storage were not
  verified from a page; the subagent marked them from memory.
