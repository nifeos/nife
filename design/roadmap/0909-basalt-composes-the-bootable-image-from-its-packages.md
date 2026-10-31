---
status: PARTIAL
raised: 2026-10-12
milestone_dependencies: 755, 607, 611, 810, 911, 919, 990
decision_dependencies: 297, 235, 241, 250
machine_requirements: none
specific_machine: none
needs_person: no
---
# 909. basalt composes the bootable image from its packages

*(Minted 2026-10-12 (UTC) by lane `maintainer/foreign-programs-via-jig`, under §297 (software
reaches a nife system as a package `jig` installs from basalt's index, the kernel included). The
number is provisional until the merge queue lands it; the title and slug are drafts.)*

calef, 2026-10-12 (UTC): *"The image should be composed from basalts packages."* The programs a
system needs before `jig` runs reach it this way. They are the kernel, the progenitor, the shell,
`jig` and the network stack.

## Where it stands

Partial, and the pair of verbs is the part that ships.

nife composes an image from packages. `packages/base-image.toml` (provisional; minted
2026-10-28 UTC) names the programs an image carries and one version for the base, because a
package releases together and an image carries what it boots. It lists the `drivers` package's
programs because the kernel's filesystem service panics without `block_driver`: an image that left
the drivers out boots to the progenitor and stops there. `cargo xtask package --package <name>
--arch <arch> [--version <v>]` packs one package from its milestone 611 (every program and crate
belongs to a package) definition, and `cargo xtask base-packages <arch> [--test]` builds the base
set. `--test` adds the `fixtures` package, the one the composed test image's boot self-test runs
and no release image carries.

`cargo xtask base-image <arch> [--from <dir>] [--catalog <path>] [--out <dir>] [--test] [--boot]`
composes the image. It takes every program's `bin/<program>` member out of the archive of the
package that owns it, and refuses a program the catalog does not vouch for or whose bytes disagree
with it. It writes `CATALOG` and the measurement table with the archive build's own writer, then
builds the kernel against that manifest, because a kernel built before its archive refuses to enter
the progenitor. An aarch64 image composed this way boots to the `swish` prompt with the boot
self-test green, and `script/ci-build`'s `base-image` row boots one on every pull request.

`--test` adds the programs of the `fixtures` package, which every archive build packs and no release
image carries: the kernel's boot self-test runs `kernel_test_subject` and `memory_region_depleter`,
so a composed image without them boots with two self-test checks red.

basalt builds the same set through its own entry point. `recipes/base.toml` (provisional) names
no program: it points at the pinned checkout's image definition, so membership stays nife's.
`helpers/packages.py base <nife> <repository> [--triples <t>] [--test]` stages the pin, runs that
commit's `cargo xtask base-packages` per architecture, copies the packages into the repository
layout `jig` reads, and writes each package's source archive. `base --test` asks the pin for the
`fixtures` package and lays it, and the index naming it, in a `test` channel beside `rolling`,
which `publish` never serves: a fixtures archive cannot reach a published index while the composer
still reads it.

Reused rather than written twice: milestone 611's package definitions and `helpers/packages.py
--check` as the format's reader, `cargo xtask package`'s one producer (`build`, `write_out`), the
archive build's `write_archive_with` and measurement table, `boot_check::boot_image` for the boot,
and basalt's existing repository, source-archive and publish path.

## Where it starts

- Milestone 755 (basalt v0 pins nife and runs its gate) is built. basalt pins a nife commit and runs
  that commit's own image build and tests. nife's tree builds the image it tests, from binaries
  nife compiled.
- Milestone 607 (nife is built and updated from packages, and the repository divides along what
  releases together) planned this step as its item 1: basalt assembles the image from base packages
  by digest, on the host. 607 did not say who assembles it. §297 says basalt does.
- Milestone 611 (every program and crate belongs to a package, and every package has a home) draws
  the package boundaries this build reads.
- Milestone 810 (`ripgrep` is packaged in basalt and installed with `jig`) builds basalt's first
  package. This milestone extends that build to the base.
- Milestone 911 (scripted installs grant only what the owner's approval file allows) gives the
  composition the check it runs with nobody at a prompt. The composition tests each package's asks
  against the image's approval file, and writes the same grant records an interactive install
  writes. 911's block names this dependency and leaves the edge here.
- Milestone 919 (a package carries several programs, and `jig` installs, finds and removes each
  one) lets a base package such as `core-tools`, with five programs, install at all.

## The work

1. basalt builds every base package from the pinned nife commit: the kernel, the progenitor, the
   shell, `jig` and the network stack. The rest of the base is what §241 (a threadbare base: the
   boot slot holds the kernel and what boots and repairs, and every other program is a package)
   names. nife builds it as `cargo xtask base-packages`, and basalt as `helpers/packages.py base`,
   which drives the pin's own builder. The pin bump's workflow job runs it (see BUGS).
2. basalt composes each architecture's image from those packages, by digest, as 607's item 1
   describes. The image names basalt's index (§250 (an image names its distribution's package
   index, at `basalt.nifeos.org`, and a package's bytes may live anywhere)). `cargo xtask
   base-image` builds it: it checks each archive's digest against the catalog, and the image
   carries that catalog plus `SOURCES_ENTRY` for the index it names.
3. Each package in the image passes 911's host-side check against the image's approval file, and
   the image's first installed-programs table holds the resulting grant records. A package whose
   manifest asks for more than the file allows fails the build, naming the extra grant. Open, and
   it waits on 911 landing. The seam is `compose`, one call per package with the image's approval
   file before it takes the program out of the archive, and 911's grant record is the one encoding
   in `crates/installed_programs` that the interactive install and `jig install --approved` already
   write. Where the records live is milestone 990 (the base image's first installed-programs
   table): generation 1 on the image's UFS2 launch volume.
4. basalt's gate boots the composed image rather than the one nife's tree built. A job of its own
   runs `helpers/packages.py base` and then `cargo xtask base-image <arch> --from <repository>
   --catalog <index> --test --boot` once per architecture, beside the `jig install` legs. It boots
   an image composed from the base packages the pin's builder packed, and that image reaches the
   prompt; the kernel still comes from the `kernel` package's crates, the gap the block's `BUGS`
   entry records. Both composed rows are two of the seven check names basalt's ruleset requires, so
   a composed image that stops booting holds a merge.
5. nife's own image build stays for nife's tests, composed from the same package definitions, so a
   nife change can be tested before basalt pins it. `cargo xtask image` still packs the tree's
   image for every existing check, and `cargo xtask base-image` composes the same membership from
   packages, booted by `script/ci-build`'s `base-image` row on every pull request.

## Done when

Under QEMU on aarch64, riscv64 and x86_64, basalt's gate boots an image it composed from its own
packages and reaches the swish prompt. `script/swish-check`'s rows pass on that image. No binary in
the image came from nife's tree build rather than from a package basalt built.

Reuse: 607's design for assembly by digest, 755's gate and 810's package build, extended rather
than replaced.

## Follow-on

- **Milestone 911.** Item 3's host-side check lands with 911's approval-file gate, which is in a
  lane now. The composition leaves the seam at `compose` and does not invent a second check.
- **Milestone 990.** Item 3's table half, where a composed image keeps its first generation, is
  milestone 990 (the base image's first installed-programs table).
- **Done.** `nifeos/basalt#50` carried item 4, the composed boot, into basalt's `packages.yml`.
- **Outstanding.** `script/swish-check`'s rows have not run against a composed image. They are the
  next piece after item 4, and until then a composed image proves it boots to the prompt only.

## BUGS

- **The kernel is built, not packaged.** The kernel is firmware's entry, and no archive carries it:
  its package declares crates rather than programs. `base-image` therefore builds it from those
  crates after composing the ramdisk. "No binary came from the tree" holds of the ramdisk and not
  of the kernel until 607's item 6.
- **The base packages record no digest in a recipe.** A recipe's `[package_digest]` holds one
  package per architecture; the base is fifteen packages times three. Their digests are the
  repository index lines, which the composed image's catalog read checks, and basalt's
  `reproduce` job compares two independent builds. Nothing yet compares a base package's bytes
  against a reviewed line, so a byte that moves without changing the index passes. Home: 917
  (basalt's packages build reproducibly), or a recipe table shaped for a set rather than a package.
- **`script/swish-check`'s rows have not run against a composed image.** `--test` composes the
  base plus the `fixtures` package, which is what the kernel's boot self-test and the shell's check
  run; the rows' own claims on that image are the next piece. Until they run, a composed image
  proves it boots to the prompt and no further.
- The image's trust root still pins the base in the kernel until 607's item 6 lands, so a base
  package change still relinks the kernel.
- **One version number for the whole base** (`0.1.0`, provisional). The base packages release
  together, so a split version per package buys nothing yet, and the image refuses a package whose
  archive carries another version.

## Index row

basalt composes the bootable image from the packages it builds, so the kernel and the base reach a
system the same way every other program does.
