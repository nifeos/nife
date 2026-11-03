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
repository. The kernel leaves last among the code, to `nife-kernel`. Milestone
857 (crates released on their own) carries the crates out one at a time, and each extracted crate
runs the stranger test that milestone 887 (crate repositories share one CI template) adapts for a
crate repository.

On 2026-11-03 (UTC) the architect ruled that a stranger test of this repository is not useful while
code moves out of it, so the cadence pauses until the kernel leaves. The restart is an acceptance
item of the milestone that moves the kernel. This proposal is that milestone. The architect ruled
on its five open questions on 2026-11-03 (UTC); the Rulings section below states each.

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
- The instrument has its own repository (see Rulings, item 2), so the stranger test of the
  kernel and of the whole system does not depend on a file in nife.

## Acceptance

1. `nifeos/nife-kernel` exists, created with the architect's approval, and holds the kernel with
   its history.
2. The kernel builds and passes its own suite on aarch64, riscv64 and x86_64 from a clean clone of
   that repository, as §19 (architectural parity is a tenet) requires.
3. basalt's manifest pins `nifeos/nife-kernel@<sha>`, and basalt's full-system test is green on
   that pin.
4. No file in any extracted repository names a nife path or pin; a check proves it, not a reading.
5. The stranger test of the whole system runs against basalt, and its result gates the milestone.
   Three pieces make that possible. The instrument (`script/stranger-test`, the rubric and run
   record `notes/stranger-test.md` with its appendices, and `stranger-cadence.yml`) has moved to
   its own repository, provisionally `nifeos/stranger-test`, which exists, created with the
   architect's approval. The new rubric is written, dated and committed there before its first
   run. And `nife-kernel` carries the crate template's stranger test of milestone 887 for its own
   build half. One run then goes, scored against the new rubric, which starts series 2 at run 1.
6. The pause line, `**The cadence pauses until the kernel leaves nife.**`, is gone, and the
   instrument repository holds the single 28-day cadence constant. Both live where the instrument
   lives, after the move; the stranger note in nife no longer exists to carry them.
   `script/stranger-test --selftest` proves that without the pause line the cadence runs at 28
   days.

## What it costs

The extraction itself is a filter and a push. The cost sits in items 2, 3 and 5. Scoring a run is a person's work, so the number of subjects
(basalt, `nife-kernel` and every crate repository) is the real cost of the 28-day cadence. Item 2 moves the
build and the QEMU gates the kernel needs, a subset of `script/lint` and `script/test` that section 4 of
[the first split](../../../notes/the-first-split.md) leaves unmeasured. Item 3 is a pin bump
through basalt's queue. Item 5 is one stranger session plus one lane to brief and score it, about
200k lane tokens and half an hour of wall clock by the stranger note's own figures. Before that, a
lane moves the instrument to its new repository and writes the new rubric.

## A concrete example

Five of the rubric's eight mental-model rows ask about things that will not be in `nife-kernel`.
M4 asks what `BUILT` means on a roadmap row, and the roadmap will be on `nifeos.org`. M5 asks why
there is a `crates/` and a `components/src/`, and both will have left. M7 asks how to add a
program, which will happen in a program repository and in basalt. M8 asks who decides a name and
what `script/names` accepts, and that script moves with the tooling. M2 asks about the network,
whose stack lives in userland. Only M1 (capabilities), M3 (`kernel/src/arch/`) and M6 (`BUGS`)
still point at the kernel repository. B3 asks whether `script/test` reaches green, and the
kernel's `script/test` boots images built from components in other repositories. So a run
against the kernel measures a different tree with a rubric written for this one, unless the
rubric changes first. The ruling in item 3 below changes it.

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

- rust-lang/crater runs the test suites of thousands of crates from a separate repository and
  infrastructure, and the Rust project's compiler repository does not hold it. The measuring tool
  lives apart from what it measures and serves many subjects.
- Kubernetes keeps `test-infra` (Prow, the job definitions) in a repository apart from
  `kubernetes/kubernetes` and the staging repositories it tests.
- Debian's `piuparts` is a standalone tool that installs, upgrades and removes packages in a clean
  chroot; one tool serves every package, and no package carries it.
- SPEC CPU ships numbered versions (CPU2006, CPU2017) and does not compare scores across them, since
  the workloads changed.
- Preregistered trials fix the protocol and the scoring in writing, with a date, before the first
  subject, so the measure cannot drift toward the result.

Where nife differs: the check above is mechanical and the stranger test is not. A bot can prove a
repository builds alone; only a fresh reader can say whether it explains itself. So nife keeps both,
the build check in each repository's CI and the stranger run as a scored measurement, and the move
must carry the second as carefully as the first.

## Rulings

The architect ruled on all five questions on 2026-11-03 (UTC).

1. **Where the stranger test runs.** The restarted stranger test of the whole system runs against
   basalt: a stranger clones the distribution and builds it. `nife-kernel` gets the crate
   template's stranger test (milestone 887) for its own build half, like any other extracted
   repository. The kernel's `script/test` boots images built from components in other
   repositories, so the build half cannot go green in `nife-kernel` alone.
2. **Where the instrument lives.** The instrument moves to a new repository, provisionally
   `nifeos/stranger-test`: the harness `script/stranger-test`, the rubric and run record
   `notes/stranger-test.md` with its appendices, and the weekly workflow `stranger-cadence.yml`.
   The architect approved the direction only. Creating the repository and its name still need the
   architect's separate go, which is why acceptance item 1 and item 5 keep "created with the
   architect's approval". The reasons: the stranger must not meet the rubric; one instrument serves
   `nife-kernel`, basalt and every crate repository; and the dependency runs one way, from
   instrument to subject.
3. **The rubric.** The rubric changes before the restart and counts as a new rubric. It is
   written, dated and committed before its first run. Runs 1 to 6 stay as series 1, the new rubric
   starts series 2 at run 1, and scores are not compared across series. A note may track B1 to B4
   across both. The reason: five of the eight M rows ask about things that will not be in the
   repository.
4. **The cadence.** One cadence serves every repository: 28 days, exactly four Mondays, so the
   weekly job notices on the due day. It is one constant in the instrument repository and replaces
   nife's 30 days. Scoring a run is a person's work, so the number of subjects is the real cost;
   no decision about that number has been made.
5. **The name.** The repository is named `nife-kernel` (the name only; creating it needs the
   architect's separate go). It stays distinct on disk from the `kernel/` directory inside nife
   and from generic names, and it matches the hyphenated non-crate repositories.
