---
status: AMENDED
raised: 2026-09-26
decided: 2026-09-27
ratified_by: calef
---

# 235. The OS is built and updated from packages, and the tree divides by what releases together

*Amended 2026-09-27T15:06Z: Fork 3 gains a home-sizing rule, below.*

*Amended 2026-10-07 (UTC): a slot holds only the kernel and what nothing can restart; see §159 (only a new kernel needs a reboot).*

Raised 2026-09-26 (UTC) by calef, who asked for milestone 607 (provisional number): nothing ties
together how packages build and update the OS and how the monorepo splits into divisions. The measurements and
the prior art are in [notes/packages-and-divisions.md](../../notes/packages-and-divisions.md); this
section holds the forks and does not restate the numbers beyond what a fork needs.

## The rulings

calef ruled on every fork on 2026-09-27 (UTC), in the maintainer session, relayed in comments on
pull request #1389. What is left open is a follow-on, not a fork.

- Fork 2, the trust root: T4 with T2, an option added for the ruling (mechanism in Fork 2's table,
  below). calef: *"T4 with T2."* The comparable shape is Fuchsia's: the system image's Merkle root
  in the boot arguments, checked in userspace.
- Fork 1, the update unit: packages, as the direction. calef: *"I think the unit of update should
  be packages."*
- Fork 1, the slot layout: U4, a full copy per slot, ruled 2026-09-27. calef: *"A shared store
  disk savings doesn't seem worth the potential of a brick. Full copy looks right."* U2 is refused
  for that reason.
- Fork 4, the SDK: S2 now, S3 as the destination, ruled 2026-09-27. calef: *"confirm S2, with S3
  as the destination once the metrics show stability."* The S2 archive also carries C headers
  generated from the contracts and the prebuilt runtime library for C. S3 waits on the
  interface-stability metrics of `milestone/610-interface-stability` (provisional number 610).
- The ABI revision is a field in the manifest note of milestone 597 (a program carries its
  manifest in an ELF note), not a separate note. calef: *"yes"*. Whoever builds it picks the
  encoding: the descriptor's reserved zero bytes, or a longer layout. Amending version 1 in place
  holds only while nothing outside the tree has acted on the note.
- Fork 3, the repositories: R1 now, and every package leaves this repository in the end. calef:
  *"R1 for now is right with package boundaries drawn and enforced inside it by lint check."* And:
  *"I want to force decisions on homes versus there being a default of sticking around if we don't
  find a home for something."* So a package's in-tree definition carries a required `home` field
  with no default, and lint fails when it is missing or undecided. The lane
  `milestone/611-package-boundaries` (provisional number 611) is building the boundaries. And:
  *"Everything in the tree may not be in a package, but it may be in a repo."* Every tracked path
  has exactly one home repository. Only shippable things are packages: programs, crates, the `std`
  overlay, and the docs that ship with them. Project records get a home but are not packages.
- Composition, borrowed from the Linux distributions. calef: *"I think we can borrow the
  composition of packages from linux distros, which look markedly similar in what they bundle
  together in their packages."* A package is what releases together and may hold several
  programs. Authority stays per program, through each binary's manifest note and §208's grants.
  A tool with a Linux counterpart follows the distributions' grouping (procps, coreutils,
  findutils, util-linux, iproute2), as milestone 126 (the `procps` package) already does. Consumer
  splits follow Debian where needed: `-dev` and SDK parts, and debug symbols.
- P2 is decided. calef: *"We now move the system tests out of the kernel crate. So that's
  decided."* The lane `milestone/609-system-tests-leave-the-kernel` (provisional number 609)
  is building it.

## What is already decided, and holds under every option here

- §151 (the goal of the repository split is independent release): the split's goal is independent release and third-party programs. The order is open.
- §197 (a package is one archive file), named by name and version.
- §195 (a reviewed recipe vouches for a package): the recipe carries the digest.
- §208 (installing a package is granting it): the set of what is installed is versioned.
- §220 (signed builds): a vendor signs, a developer self-signs, a signature is checked at install and the digest is
  then pinned. No key ships in any image.
- Milestone 525 (a bad upgrade cannot brick the machine) and 554: two boot slots with priority, tries and a success mark. Each slot holds one
  complete image.
- Milestone 104 (the measurement continues past init): the kernel's compiled-in trust root pins the progenitor and the table of program
  digests. Milestone 450 (a signature over the init image) refused a signature in its place.
- Milestone 597: a program's manifest travels in an ELF note, version 1.
- §201 (one roadmap until a citation has to cross): one roadmap until a citation has to cross a repository.

## The premise, checked

The question assumes packages could build the OS and the tree could split along release lines.
Three facts from the tree qualify that. Each is measured in the note.

1. The kernel binary is a function of every base program. `kernel/build.rs` compiles the digests of
   the progenitor and of `PROGRAM_MEASUREMENTS` into `TRUST_ROOT`. Any base service change relinks
   the kernel. §151 calls the kernel an independently released component; the build makes it the
   last link of the image.
2. The kernel crate is also the integrator and the test harness. `kernel/src/user/` is 28,441 of the
   kernel's 89,885 lines. The kernel depends on nine service crates and three fixture crates, and
   half of the commits in that directory touch another crate.
3. Contracts still move with their consumers. Of 215 commits touching a contract crate in 60 days,
   76% touched another division too. §151 names protocols that stop churning as a precondition of
   the split. It is not met.

None of these makes the question wrong. They say where the work is, and fact 1 decides which forks
are live.

## Fork 1: what an OS update ships

What a slot receives. The on-disk layout and the update manifest are formats the updater and the
booting system agree on.

| option | shape | what it costs |
|---|---|---|
| U1. The image stays the unit (not taken) | Packages are only a build input; a slot receives the whole image | Every base fix is a full image, about 10 MB. |
| U2. One shared store (refused 2026-09-27) | Both slots read one content-addressed store of base packages (Fuchsia, OSTree). | A mistake in the store's collection rule bricks both slots. The disk saved is not worth that. |
| U4. Packages downloaded, each slot a full copy (ruled) | Packages are the download unit, as in U2, but a slot holds its own full copy of every base package. The inactive slot is wiped whole before it is reused, then filled from the new manifest. | No collection rule, so nothing can delete a blob the other slot needs, and the slots stay independent. Disk holds two full base sets. A download can still skip packages whose digests match the running slot, by copying them locally. |
| U3. Two tiers (not taken) | U1 below the package manager, §208 activation above it | A tier line to maintain, and two rollback mechanisms. |

Every option keeps 525's slots. U1 and U3 need no new boot format; U2 and U4 each need one. The
ruling takes packages as the unit and U4 as the slot layout, per the table above.

The base image is the resolved lock of a dependency graph. Resolution happens at build time, not
on the device, the way `Cargo.lock` and a Nix closure work and apt does not. Programs outside the
base are resolved at install time and pinned by digest under §220. An installed package can fill a
service's role through a grant (§208) without changing the base list. And an owner-pinned base,
built from §220's scoped keys, is a door T4 with T2 leaves open, since the progenitor checks
whatever manifest digest the loader hands it.

## Fork 2: where the trust root lives

This decides whether the kernel can ever release on its own. The kernel and the progenitor agree on
it, so it is irreversible once an installed machine boots it.

| option | shape | what it costs |
|---|---|---|
| T1. Compiled into the kernel (today) | `TRUST_ROOT` pins the progenitor and the table | Kernel and base image release as one unit. |
| T2. Handed over by the loader | The slot carries one manifest digest, the loader passes it to the kernel, and the kernel checks the progenitor and table against it | The kernel binary stops depending on userspace. Trust moves to the loader, which only Secure Boot (milestone 500 (a stick that boots with Secure Boot on)) or the slot checksum protects. Three handoff paths to change: a device tree `/chosen` on aarch64 and riscv64, PVH on x86_64. Milestone 525 records that the device-tree path has one initrd slot. |
| T4 with T2 (ruled) | The loader hands over the progenitor digest and the manifest digest. The kernel checks only the progenitor. The progenitor checks the base set against the manifest digest. | The kernel binary stops depending on the base set, and the checking of it moves to userspace, which already refuses unlisted programs (milestone 104 (the measurement continues past init)). T2's costs stay: trust in the loader and three handoff paths. The kernel still depends on the progenitor, so the two release together. |
| T3. A signature over the system manifest | The kernel verifies a vendor key | Reopens milestone 450's refusal: keys and verification code in the kernel. |

T1 fits U1 and U3 without change. U2 works under T1 too: the kernel pins the manifest's digest, and
the manifest pins the blobs. So Fork 2 is about the kernel's independence, not about packages. The ruling makes the kernel
independent of the base set, though not of the progenitor.

## Fork 3: the divisions, and whether they become repositories

The names of divisions and repositories are calef's; those below are provisional. Seven candidates
come out of the dependency graph: kernel, contracts, runtime, boot, services, fixtures and host.
Proofs are not a division: each lives beside its code in 28 places.

| option | shape | what it costs |
|---|---|---|
| R1 (ruled, now). One repository, boundaries enforced inside it | Each package's boundary is drawn in-tree and checked by lint, in the spirit of milestone 39 (repository structure for a loosely-coupled OS)'s option B. `script/test` stays one command. | Nothing moves between repositories yet. |
| R2. Split out the SDK alone (not taken) | Contracts and runtime move out | 76% of contract commits touch another division, so most become two pull requests. |
| R3. Split along all seven (not taken) | Milestone 39's option C, with `basalt` as the gate | 27% of code commits cross a division. |
| R4 (ruled, the end state). Every package leaves | Each package has a required `home`, no default | Forces a decision per package rather than a default of staying. |

Two things stay single-tree under every option. One `script/test` proves every architecture (§19 (architectural parity is a tenet)),
and the integration test suite needs every division at a known version to run. R1 keeps both. The
end state has to rebuild them wherever the base's gate goes.

### Amendment, 2026-09-27T15:06Z: how big a home is (question 7c)

calef ruled, once R4 has packages leaving: **a home is sized by what changes together**, option C.
A (one repository per package) is refused: it maximizes two-PR commits wherever packages
co-change. B (one repository per division) is refused too: a division comes from the dependency
graph, not co-change ("The premise, checked" above measures 76% of contract commits crossing one).

Milestone 610 (the interface's stability is measured weekly) gains a per-package co-change column
beside the division figure it carries; packages that move together share a home. Measure first: no
package moves before that column exists. R1 is unchanged.

Every `.package.toml` file's provisional `home` field stays provisional. `packages/homes.toml`'s
header cites this. The column is owed work against 610's own block, not built here.

## Fork 4: how a third party builds against the ABI

This is milestone 198 (a package manager)'s SDK gap: an outside author must clone this repository to get the toolchain.
The closure a `std` program needs is ten crates, about 24,400 lines, plus the 4,486-line `std`
overlay, the target files and the linker script.

| option | shape | what it costs |
|---|---|---|
| S1. Publish crates and a toolchain (not planned) | The ten crates on crates.io, plus a toolchain archive | A semver promise to strangers, and it still needs S2 for the overlay. |
| S2. One SDK archive per release (ruled, now) | Fuchsia's IDK and Genode's `api` archive: the crates and toolchain pieces in one file, used through `[patch]` or a path. | Nothing published to a registry. A bespoke install step for every third party. |
| S3. An upstream Rust target (ruled, the destination) | The `std` overlay moves upstream as a tier 3 target. | Upstream review, and a claim that the ABI is stable enough for someone else's tree. Months, not a milestone. |

Under the ruling, a program carries the ABI revision it was built against, in milestone 597's
manifest note, so the progenitor can refuse one it no longer supports.

## Recommendations on the reversible parts

Sequencing is reversible, and so is the first step, because it lives on the host and leaves the
machine in no form.

Three pieces are right under every option, and cost nothing on the device. P1 builds the image
from packages. P2 pulls the integrator and the test harness out of the kernel crate; decided, in a
lane. P3 cuts the two contract-to-runtime edges. Descriptions are in the table below. Then comes
the ruled work: the SDK archive (P4), the update path (P5), the trust handoff (P6) and the package
boundaries (P7).

The proposals, none minted:

| id | proposal | waits on |
|---|---|---|
| P1 | The image is assembled from base packages by digest | nothing |
| P2 | The kernel crate stops carrying the bring-up and the system tests | decided; in the lane `milestone/609-system-tests-leave-the-kernel` |
| P3 | A contract never links the runtime | nothing |
| P4 | A third party builds `greeting` from the S2 archive, with no clone of this repository | nothing (S2 and the note field ruled) |
| P5 | An OS update is a set of packages that lands through a slot | P1 (U4 ruled) |
| P6 | The loader hands over two digests, and the progenitor checks the base set (T4 with T2) | P2 |
| P7 | Package boundaries drawn in-tree, lint-checked, each with a required `home` | decided; in the lane `milestone/611-package-boundaries` |

## The seven questions

In [`seven-questions.md`](0235-packages-build-the-os-and-the-tree-divides-by-release/seven-questions.md), with the refusals each question turned up.

## What cannot be undone, and what can

The update manifest and store layout (Fork 1), the trust handoff (Fork 2), repository boundaries
and their names (Fork 3), and anything published to a stranger (Fork 4) are irreversible once a
machine or a person outside acts on them. Everything in P1 to P3 is host code and can be reverted.

## What this unblocks

Every proposal above. Milestone 198's rung 4 (the web page) can say the SDK is an S2 archive.
Milestone 39 and §151's order now have an answer: R1 now, every package out in the end.

## Follow-on

Open, and none of them a fork of this section:

- How base services with no Linux counterpart (the progenitor, the file server, drivers) are
  grouped. nife needs its own grouping here.
- Package names, calef's under the naming rule.
- The format of a package's in-tree definition, and of the non-package home record, proposed by
  `milestone/611-package-boundaries`.


## BUGS

- The division assignment is a judgment per crate. The note says which crates are ambiguous.
- The image size is a stale local build. P1's lane should measure a fresh one.
- The word "manifest" now means two things: a program's ask (milestone 597's ELF note) and the list of
  packages an image holds (U4 and T4 with T2, both ruled). The second needs its own name before it is built.
