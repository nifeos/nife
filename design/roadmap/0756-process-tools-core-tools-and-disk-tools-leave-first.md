---
status: NOT-STARTED
raised: 2026-10-04
milestone_dependencies: 607, 610, 691, 755
decision_dependencies: 247
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 756. `process-tools`, `core-tools` and `disk-tools` are the first code to leave this repository

*(Number provisional until the merge queue lands it. Title and slug are drafts.)* It began as
the proposal `which-repository-split-goes-first`, written and promoted inside pull request #1609
on 2026-10-04 (UTC), so it never reached `main` and carries no `promoted_from` line. Written by the lane
`lane/split-order-proposal` on calef's prompt of 2026-10-04: *"I do think we should soon be breaking
things out of the mono repo and into their own repos. I also think the basalt repo is sitting there
waiting to package up those repos for its distribution."* §151 (the goal of the repository split is
independent release) ruled the destination and left the order "to be ruled on its own". The
measurements are in [notes/the-first-split.md](../../notes/the-first-split.md).

## The ruling

calef ruled both questions on 2026-10-04 (UTC), relayed by the maintainer on pull request #1609.
§247 (the split begins with basalt holding nife, and `procps` moves first) records them.

- **Yes, basalt v0 is the first step**, including the go-ahead for a lane's first write to
  `nifeos/basalt`. That is milestone 755 (basalt v0 pins nife and runs its gate).
- **Yes, the first code cut is `procps` + `coreutils` + `util-linux`, and only once all six
  conditions below hold.** This block is that cut.
- Ruled 2026-10-28 (UTC): the cut moves to three repositories named after the packages,
  `nifeos/process-tools`, `nifeos/core-tools` and `nifeos/disk-tools`. One repository holding all
  three packages came up; the ruling refuses it. The packages' `home` lines in
  `packages/*.package.toml` keep `status = "provisional"` until each repository exists; this rules
  the destination and does not flip that status. §201 (one roadmap until a citation has to cross),
  amended the same day, rules the citation identity across repositories. Ruled 2026-10-28: the
  home's grain, one repository per package (§316 (one repository per package leaves the tree)), and
  the names, ratified the same day on the naming review
  (design/roadmap/proposals/first-cut-names.md, pull request #2870). Still open, and not blocking
  until this block starts: the citation identity the first crossing writes (§201), and the SDK's
  place in milestone 691 (packages move out of this repository, one per pull request)'s order.
- Sequenced 2026-10-28 (UTC): the update path goes first, milestones 911 (scripted installs grant
  only what the owner's approval file allows), 909 (basalt composes the bootable image from its
  packages), then milestone 607 item 5. This cut waits for its conditions.

Renamed 2026-10-06 (UTC). calef ruled that `util-linux` becomes `disk-tools` ("util-linux is a
horrible package name for a nife package"), `coreutils` becomes `core-tools` and `procps` becomes
`process-tools`, and moved `uuid` from the disk tools to `core-tools`. The cut holds the same 15
programs, so the six conditions and the measurements stand. The title and slug changed with it.
The title was `procps`, `coreutils` and `util-linux` are the first code to leave this repository,
and older records cite it by that or as procps, coreutils and util-linux leave first. The ruling
above and the analysis below keep the names they were written with. The homes are
`nifeos/process-tools`, `nifeos/core-tools` and `nifeos/disk-tools` (ruled 2026-10-28); the
packages' `home` lines keep `status = "provisional"` until each repository exists.

What is below is the proposal's analysis, unedited apart from this heading block: the argument is
its author's and promotion is not the moment to improve it.

## The finding that shapes the options

§151's preconditions are not met, and one of the two it named is the furthest from met. The
contract crates took 157 breaking changes in the last four weeks, against milestone 610 (the
interface's stability is measured weekly)'s proposed threshold of four. No IPC protocol carries a
version a peer could check. Three more preconditions block any cut that moves code. One crate
(`components`) holds 56 programs from 18 packages. There is no SDK archive. And §235 (the OS is built
and updated from packages)'s own amendment says no package moves before 610's per-package co-change
column exists. None of the nine is met but the package format.

One first step needs none of them: basalt itself, holding one component. It moves nothing, so
the amendment does not bind it. It builds the piece §151's BUGS calls the real cost of a split (the
whole-system gate living somewhere other than the monorepo) before any repository pays the price of
a two-repo change.

## Options for the first cut

| option | what moves | cost, measured | verdict |
|---|---|---|---|
| A. basalt v0 | nothing; basalt pins `nife` at a commit and runs its image build and system test | zero cross-repository changes; one scheduled basalt run per pin bump | recommended now |
| B. `procps` with `coreutils` and `util-linux` | 8 crates and 15 programs | 32 commits in thirty days, 26 crossing; 21 of those were sweeps; 1 Cargo dependent; 0 path references | recommended as the first code cut, once its preconditions hold |
| C. `redoxfs` | the vendored engine, server and host tool | 31 commits, 26 crossing, 12 of 17 focused ones crossing; 0 Cargo dependents; 254 path references; links the least settled interfaces (`filesystem_contract` 10 breaking in four weeks, the partition table 27) | second; cheapest to move today, which is an effort argument |
| D. `contracts` and `runtime` (milestone 691 (packages move out of this repository, one per pull request)'s order) | 42 crates | 298 commits, 187 crossing; 35 dependents | refused for the first cut |
| E. `host-tools` | `xtask` and friends | 227 commits, 139 crossing; 276 references in 152 files | refused |
| F. Wait for every precondition before doing anything | nothing | the integration gate is still unbuilt when the first cut arrives | refused |

## The recommendation

1. **Now: basalt v0.** One manifest file pinning `nife` by repository and commit, a workflow that
   checks that commit out and runs its own image build and system test on all three
   architectures, and the image as an artifact. A scheduled job bumps the pin the way
   `toolchain-bump.yml` bumps the nightly. No toolchain of its own: it builds at the pin the
   component's commit carries. It cites no milestone and no section, so it does not trigger §201 (one roadmap until a citation has to cross)'s
   revisit. The file's name and format are provisional.
2. **Next: the first code cut is option B**, when all of these hold, each checkable by a command:
   - 610 carries the per-package co-change column (§235's amendment).
   - `components` is split so the moving programs are in crates of their own (milestone 691).
   - The moving repository builds without a clone of this one: §235's S2 archive (milestone 607 (nife is built and updated from packages)
     item 4), or a dated, recorded exception for a git dependency on `nife` at a commit.
   - Every interface the cut links (`abi`, `address_space_map`, `glob`,
     `machine_statistics_contract`) carries a checkable version, and the moved package declares
     what it links. The cut re-derives the linked set at its start rather than trusting this
     2026-10-04 list.
   - basalt can take a component's output as a package by digest: §235's P1, milestone 607 item 1.
   - The home and its repository name are ratified, and §201's citation identity is ruled, since
     this cut is its revisit event. Ruled 2026-10-28 (UTC): the three repositories above, the names
     ratified on the naming review (pull request #2870), and §201 (one roadmap until a citation
     has to cross) as amended. What remains of this precondition is the homes: each flips to
     ratified when its repository exists.
3. Milestone 691's order is amended: leaves of the release graph first, the SDK last of the
   leaves, not first. Its rule is right for a settled tree and moves the most volatile piece first
   on this one.

## The seven questions

1. What else was considered? C loses to B on coupling with sweeps excluded (12 of 17 focused
   commits crossing against 5 of 11) and on the stability of what it links. D makes 187 of 298
   changes in a month into two-repo landings. E is the build of everything else. F leaves the
   integration gate unbuilt when it is most needed.
2. What does the tree already do? It builds `redoxfs` as three workspaces outside the root one
   and gates them by `--manifest-path`, which is the in-tree rehearsal of a separate repository. It
   pins artifacts by digest in reviewed recipes (§195 (a reviewed recipe vouches for a package)), and bumps a pin on a schedule
   (`toolchain-bump.yml`). basalt v0 is those two habits pointed at a commit.
3. Prior art. Android's `repo` manifests, Yocto with kas, and Fuchsia's integration repository
   all pin components by revision and roll the pin forward with a bot. Fuchsia split its tree into
   layered repositories and later folded them back. All of this is recalled, not re-read for this
   proposal, and should be read before a ruling leans on the Fuchsia case.
4. Is the premise true? Partly. "Soon" is true for basalt and false for moving code: the
   protocol churn §151 named is 157 breaking changes in four weeks.
5. What does each option cost? The table above, from the note. Unmeasured: the gate subset a
   moved repository needs, until a cut is chosen.
6. How reversible? A is fully reversible: nobody outside the project builds from basalt, and
   archiving it undoes it. B is reversible as code and expensive as names: the repository name, the
   ratified home and the first cross-repository citation are acted on by every later reader.
   Nothing published reaches a stranger until milestone 198 (a package manager)'s rung 4.
7. Same cost, same choice? Yes for A over F and for B over D and E. C over B would be about
   effort: `redoxfs` is cheaper to move today only because its build is already separate, while B
   needs the `components` split first. That split is owed by milestone 691 for every package
   anyway, so it is not extra work, only earlier work. On equal effort B wins on coupling and on
   the stability of what it links.

## BUGS

- The coupling numbers are a one-off at one commit over one month, not 610's owed weekly column.
- basalt v0 runs nife's system gate a second time per pin bump. That doubles the system-test minutes
  for whatever crosses, and it is the price of the gate living in basalt before it has to.

## Index row

The first code to leave the monorepo: the `process-tools`, `core-tools` and `disk-tools` packages
move to three repositories named after the packages and reach the image through basalt, once six
measured preconditions hold (ruled by calef 2026-10-04). The homes and the sequencing carry the
2026-10-28 ruling.
