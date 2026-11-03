---
status: PROPOSED
raised: 2026-11-03
milestone_dependencies: 857
decision_dependencies: 151, 201
machine_requirements: none
specific_machine: none
needs_person: yes
---
# The kernel leaves nife

The lane `lane/stranger-3099` wrote this on 2026-11-03 (UTC), while it paused the stranger-test
cadence on that day's ruling. The milestone has no number; an integrator mints one at promotion.

## Background

§151 (the repository's goal is independent release) sets the order of the split: bottom-up, so the
crates a program links leave before it, and nothing that has left refers back into this
repository. The kernel leaves last among the code, to `nife-kernel`, a provisional name. Milestone
857 (crates released on their own) carries the crates out one at a time, and each extracted crate
runs the stranger test that milestone 887 (crate repositories share one CI template) adapts for a
crate repository.

On 2026-11-03 (UTC) the architect ruled that a stranger test of this repository is not useful while
code moves out of it, so the cadence pauses until the kernel leaves. The restart is an acceptance
item of the milestone that moves the kernel. This proposal is that milestone.

## What moves

The kernel, its architecture code under `kernel/src/arch/`, and whatever the kernel alone links,
moves to `nifeos/nife-kernel` by a path-filtered extraction that keeps each commit's message,
author and date. It moves last among the code. After it, `nifeos/nife` holds only history, as §151
states.

## What the order depends on

- Every crate and program the kernel links has already left, each to its own repository, and the
  kernel takes each one as a pinned dependency.
- Nothing that has left references nife: no path, no pin, no `#[path]` module, no script that
  reads a nife file.
- The design record has moved to `nifeos.org`, or its move has a date. [The proposal for that
  move](the-design-record-moves-and-is-published.md) holds the plan.
- basalt pins `nife-kernel` beside the other components, so the whole-system test runs from basalt
  and not from nife.

## Acceptance

1. `nifeos/nife-kernel` exists, created with the architect's approval, and holds the kernel with
   its history.
2. The kernel builds and passes its own suite on aarch64, riscv64 and x86_64 from a clean clone of
   that repository, as §19 (architectural parity is a tenet) requires.
3. basalt's manifest pins `nifeos/nife-kernel@<sha>`, and basalt's full-system test is green on
   that pin.
4. No file in any extracted repository names a nife path or pin; a check proves it, not a reading.
5. The stranger test runs against `nife-kernel` and its result gates the milestone. The catch is
   that the instrument lives in nife: `script/stranger-test`, the rubric and run record in
   `notes/stranger-test.md`, its appendices, and `.github/workflows/stranger-cadence.yml`. The
   design record is moving to `nifeos.org`, so the milestone gives each piece a home where it still
   works against the kernel repository. There the harness clones and withholds, the rubric asks,
   the run record parses, and the weekly workflow reads it. One run then goes, scored against a
   rubric fixed before the run.
6. The change that restores the cadence deletes the pause line,
   `**The cadence pauses until the kernel leaves nife.**`, from the stranger note in the same
   commit. `script/stranger-test --selftest` already proves that deleting that line restores the
   30-day cadence.

## What it costs

The extraction itself is a filter and a push. The cost sits in items 2, 3 and 5. Item 2 moves the
build and the QEMU gates the kernel needs, a subset of `script/lint` and `script/test` that section 4 of
[the first split](../../../notes/the-first-split.md) leaves unmeasured. Item 3 is a pin bump
through basalt's queue. Item 5 is one stranger session plus one lane to brief and score it, about
200k lane tokens and half an hour of wall clock by the stranger note's own figures. Before that, a
lane moves the instrument and rewrites the rubric.

## A concrete example

Five of the rubric's eight mental-model rows ask about things that will not be in `nife-kernel`.
M4 asks what `BUILT` means on a roadmap row, and the roadmap will be on `nifeos.org`. M5 asks why
there is a `crates/` and a `components/src/`, and both will have left. M7 asks how to add a
program, which will happen in a program repository and in basalt. M8 asks who decides a name and
what `script/names` accepts, and that script moves with the tooling. M2 asks about the network,
whose stack lives in userland. Only M1 (capabilities), M3 (`kernel/src/arch/`) and M6 (`BUGS`)
still point at the kernel repository. B3 asks whether `script/test` reaches green, and the
kernel's `script/test` boots images built from components in other repositories. So the run
against `nife-kernel` measures a different tree with a rubric written for this one, unless the
rubric changes first.

Reuse: the extraction reuses `git filter-repo`, as the design-record proposal does, and the
stranger test reuses `script/stranger-test` and milestone 887's crate template rather than a new
harness.

## Prior art

Recalled, not read on the day of writing.

- Kubernetes publishes its `staging/` directories as separate repositories (`client-go`,
  `apimachinery`) through a publishing bot, which builds each published repository on its own
  after every sync. A repository that stops building alone fails the bot, not the monorepo.
- Symfony splits its monorepo into read-only component repositories with `splitsh-lite`, and each
  component repository runs its own test suite against its own `composer.json`, so a component
  that silently depends on a sibling fails there.
- X.Org's 2005 move from one monolithic tree to separate modules kept a whole-system build working
  through a build script that fetched and built every module in order, the role basalt plays here.
- Debian and Fedora rebuild every package from source in a clean chroot (`sbuild`, `mock`), which
  is the distribution's form of "a stranger with only this source can build it".

Where nife differs: the check above is mechanical and the stranger test is not. A bot can prove a
repository builds alone; only a fresh reader can say whether it explains itself. So nife keeps both,
the build check in each repository's CI and the stranger run as a scored measurement, and the move
must carry the second as carefully as the first.

## Open questions for the architect

Each goes to the architect alone, with background and a recommendation.

1. Does the stranger test run against `nife-kernel` alone, or against basalt, where a stranger
   could build the whole system? The rubric's build half reads more naturally against basalt.
2. Where does the instrument live after the move: in `nife-kernel` beside the code it measures, in
   `nifeos.org` with the design record, or in the crate template of milestone 887?
3. Does the rubric change before the restart, and if so, does the change count as a new rubric
   written before its first run, with runs 1 to 6 kept as a separate series?
4. Does the restart keep 30 days, or does it take the crate repositories' cadence?
5. Is `nife-kernel` the name? It is provisional in §151.
