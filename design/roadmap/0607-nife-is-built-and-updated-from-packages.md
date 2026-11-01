---
status: NOT-STARTED
raised: 2026-09-26
milestone_dependencies: none
decision_dependencies: 235
machine_requirements: none
specific_machine: none
needs_person: no
---
# 607. nife is built and updated from packages, and the repository divides along what releases together

The number is provisional: the integrator mints it at merge. calef asked for it on 2026-09-26
(UTC). A design lane (`milestone/607-packages-and-divisions`) wrote the forks and built nothing.

Four records touch the question and none joins the others. Milestone 39 (repository structure for a loosely-coupled OS) recorded a monorepo now and
a split later. §151 (the goal of the repository split is independent release) made the split's goal independent release and third-party programs. Milestone
198 installs programs and does not build the OS. Milestones 525 and 554 swap a whole image between
two boot slots. This block asks how a package becomes part of the OS, and where the repository's
seams are.

## What the design lane found

The forks are in [§235 (the OS is built and updated from packages)](../decisions/0235-packages-build-the-os-and-the-tree-divides-by-release.md)
(provisional number), DECIDED 2026-09-27. The measurements and the prior art are in
[notes/packages-and-divisions.md](../../notes/packages-and-divisions.md). Three findings shape it:

- The kernel's compiled-in trust root pins every base program, so the kernel relinks whenever
  userspace changes. It cannot release on its own while that holds.
- The kernel crate is also the integrator and the system test suite: `kernel/src/user/` is 28,441
  of its 89,885 lines, and it links nine service crates and three fixture crates.
- Contracts still change with their consumers: 76% of contract commits in 60 days touched another
  division. §151's precondition for ruling the split's order is not met.

## The rulings

calef ruled on every fork on 2026-09-27 (UTC), relayed by the maintainer on pull request #1389.
§235 records them with his words.

- The trust root is T4 with T2: the loader hands over the progenitor digest and the digest of the archive's `program_measurements` table (ruled 2026-11-01 UTC), the
  kernel checks the progenitor, and the progenitor checks the base set.
- Packages are the update unit, and each slot holds a full copy (U4), ruled 2026-09-27. The shared
  store (U2) is refused: calef judged its disk saving not worth the risk of a brick.
- Item 2 below is milestone 609 (the system tests leave the kernel crate), BUILT.
- The SDK is S2 now, one archive per release with C headers and the prebuilt runtime library, and
  S3 (an upstream Rust target) once `milestone/610-interface-stability` shows the interfaces stable.
  S1 is not planned.
- The ABI revision is a field in the manifest note of milestone 597 (a program carries its manifest in an ELF note), not a separate note.
- The repositories: R1 now, with package boundaries drawn in-tree and enforced by lint. The end
  state is every package leaving this repository. Each package's definition carries a required
  `home` with no default. `milestone/611-package-boundaries` is building the boundaries.
- Composition follows the Linux distributions: a package is what releases together and may hold
  several programs, while authority stays per program. Base services with no Linux counterpart
  still need nife's own grouping, which is a follow-on.

## What is built

Nothing. The block is NOT-STARTED until a proposal below has a lane.

## What is left

The proposals, none minted, in the recommended order. §235 has the reasoning.

1. The image is assembled from base packages by digest (host side; waits on nothing).
   Since 2026-10-12 (UTC) this is milestone 909 (basalt composes the bootable image from its
   packages): calef ruled that basalt composes it, under §297 (software reaches a nife system as a
   package `jig` installs from basalt's index, the kernel included).
2. The kernel crate stops carrying the userspace bring-up and the system tests: milestone 609
   (the system tests leave the kernel crate), BUILT.
3. A contract never links the runtime: `supervision_contract` and `hot_swap_contract` (landed
   2026-10-29 UTC; the runtime halves are `crates/process_builder` and `crates/hot_swap_server`).
4. A third party builds `greeting` with no clone of this repository (S2 and the ABI revision's note field ruled; waits on nothing).
5. An OS update is a set of packages that lands through a slot (U4 ruled). Item 1
   (milestone 909 (basalt composes the bootable image from its packages)) landed 2026-10-28 UTC,
   and the block is milestone 976 (an OS update is a set of packages that lands through a slot),
   `design/roadmap/0976-an-os-update-through-a-slot.md` (raised 2026-10-28 UTC).
6. The loader hands over two digests and the progenitor checks the base set (ruled): milestones
   695 (the kernel trusts two digests handed to it by the loader) and 697 (the trust root leaves
   the kernel binary). Its first consumer is item 5's updater, milestone 976 part 4, and its code
   waits until that part shows what a kernel download costs per update (calef, 2026-10-31 UTC).
   The handoff's bytes and the source of the loader's digests are two questions for calef, in
   `design/roadmap/proposals/how-the-loader-hands-the-kernel-two-digests.md`.
   Ruled 2026-11-01 (UTC) by the architect: this item also holds the anchor for installed rows
   and their grant lines in the installed-programs table (milestone 990 (the base image's first
   installed-programs table)). Candidates are a MAC the progenitor makes with a key the volume
   cannot supply, and a digest of the live generation in a measured or sealed place. Milestone
   917 (basalt's packages build reproducibly) holds it with this item.
7. Package boundaries drawn in-tree and checked by lint, each with a required `home` (decided; in
   the lane `milestone/611-package-boundaries`).

## BUGS

- Division names are this lane's and provisional; calef names them. Package names are his too.
- Open follow-ons, none of them a fork: the grouping of base services with no Linux counterpart,
  package names, and the format of a package's in-tree definition.
- The image size quoted in §235 is a stale local build, and item 1's lane should measure a fresh one.

## Index row

how a package becomes part of the OS, and where the repository divides
