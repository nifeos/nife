---
status: BUILT
raised: 2026-09-13
built: 2026-09-13
---
# 286. One enumeration of the checks that gate a pull request

`script/gates` is retired. `script/ci-build` carries one table of every check a pull request must
pass, with a tier saying whether a developer waits for it. With no arguments it runs that tier,
cheapest first. With names it runs exactly those, which is how `.github/workflows/ci.yml` fans
them into parallel jobs. The tier names and what "no arguments" means are provisional pending
calef: milestone 440, `design/roadmap/0440-what-no-arguments-means.md`, states the options, their
costs and the recommendation.

## The defect, and how it was already failing

The set was written down twice. `script/gates` ran six checks locally, serial, cheapest first;
`ci.yml` ran its own set as parallel jobs plus everything `gates` deliberately omitted. Nothing
compared them, and the tell that this was rung four rather than a design is that `ci.yml` carried
three separate prose comments explaining why a given check was *not* in `script/gates`.

Six places described the local set on 2026-09-13. Four of them were wrong, and every one had
been correct when it was written:

| where | said | true since |
|---|---|---|
| `.github/workflows/ci.yml` | `script/icount` "is not in script/test and it is not in script/gates" | it had been in `script/gates` since milestone 62 |
| `notes/scripts.md`, the `gates` row | "`script/fmt --check`, `script/lint`, `script/test`, then `script/test --hvf`" | `image-permissions`, `icount` and `swish-check` had joined |
| `notes/scripts.md`, the `icount` row | "Not in `test` or `gates`" | same |
| `notes/instruction-clock.md` | "it does not run under `script/test` or `script/gates`" | same |
| `CONTRIBUTING.md` | "the five checks a PR must pass" | there were six |
| `.github/pull_request_template.md` | "runs all five", with a five-item checklist | there were six, and the checklist omitted two |

The two that were right were `ci.yml`'s comments on `stack-frame-check` and `image-permissions`.
A score of two out of six on a fact that one command prints on every run is what a hand-maintained
second copy is worth.

The failure this prevents is the one `script/gates` was created for on 2026-08-03: the local
wrapper passes and CI fails on a check the wrapper never heard of. Nothing made `gates` learn about
a seventh required check, so the next one added to CI would have reproduced the original defect
exactly.

## What was built

One table in `script/ci-build`, `name|tier|command`, in cheapest-first order. Fifteen rows.
Adding a check is adding a row: the no-argument path picks it up if the tier is `local`, `--list`
prints it, and a CI job names it. Adding a job to `ci.yml` without a row here is the defect the
table exists to prevent.

```
script/ci-build                 the local tier, in the table's order, cheapest first
script/ci-build <check>...      exactly those checks, in the order given
script/ci-build --list          the table: name, tier, command
```

Every job in `ci.yml` now names a check out of that table (`script/ci-build fmt`,
`script/ci-build lint`, `script/ci-build test swish-check`, and so on down the file). No job name
changed, which matters because branch protection matches on the display name and renaming one is a
merge-queue change rather than a wording one.

`script/gates` is deleted rather than kept as a wrapper. The wrapper was the cheap option and it
was refused on calef's ruling, 2026-09-13: *"We shouldn't keep a thin wrapper to avoid making the
changes."* Keeping it would have been an argument from implementation cost, which this tree treats as
the weakest argument available, and it would have left two names for one thing plus an unratified
name on `script/names --unratified` that nobody intended to rule. The `script/` count went 55 to 54
and the names census 207 to 206, 39 unratified to 38.

**The `hvf` check survives as a row**, with its probe, its four host conditions and its loud skip
intact. It is `local` and has no CI job, because GitHub's hosted macOS arm64 runners are virtual
machines with no nested virtualization.

**Bootstrapping became explicit.** `script/ci-build` provisions on its no-argument path only, since
that is the command a person runs on a cold checkout; a named check runs exactly what it names,
because the caller naming one check is a CI job whose own steps installed what it needs. Without
that split, `script/ci-build fmt` would apt-install QEMU onto the rustfmt runner. The `test` job
gained an explicit `script/bootstrap` step, which is the shape the `bench` and `cpu matrix` jobs
already had.

## What the three `ci.yml` comments became

Asked directly, because it was the point of the milestone. **The membership half is now enforced by
construction and the reason half is still prose**, and the second half cannot be mechanized: "it
builds the kernel test binary twice, which is more than a developer will wait for" is a judgment
about a person, not a fact about a build.

What changed is where the prose lives. All three explanations moved into `script/ci-build`, in a
block directly under the table, one line per `ci` row. **A comment there cannot claim a membership
the code contradicts, because the tag beside it *is* the membership.** The `icount` comment could
say "not in the local set" while the script ran it; its replacement cannot, because the word `local`
in that row is what the no-argument path reads.

`ci.yml` keeps one sentence per affected job pointing at the table, and the `icount` job's comment
now records that it asserted the opposite for a month.

## BUGS

- A CI job can still run a script directly and bypass the table. Nothing gates that. The rung
  above would be a lint check that every `script/` invocation in `ci.yml` resolves to a table row,
  and it is not built here: `script/ci-qemu` and `script/bootstrap` are provisioning rather than
  checks, so the check would need an exception list on its first day, and an exception list is the
  hand-maintained second copy this milestone deleted. Recorded rather than built, deliberately.
- The table does not claim `verify.yml`. Kani (`script/verify`, about 47 minutes) and the
  re-falsification sweep are a different workflow with their own sharding and scope predicate.
  Naming them in the table without running them would be a fourth kind of prose nothing keeps true;
  leaving them out means the table is the enumeration of `ci.yml`'s checks, not of every check.
  `notes/check-inventory.md` is the whole surface.
- The tier tags are adjectives where the naming tenet wants nouns. `local` and `ci` are
  provisional for that reason among others; the proposal carries the refusals.
- The no-argument path runs `script/bootstrap` first, which `script/gates` did not. calef ruled
  on 2026-09-13 that this is correct and it is the shipped behavior: a machine that cannot
  provision will fail the later rows anyway, and failing early is honest. The residue is real: on a
  warm machine bootstrap prints a few lines and exits, but on a machine missing QEMU it will
  `brew install` or `apt-get install`, which is a surprise the pre-push command did not previously
  carry, and it runs BEFORE `fmt`, so a formatting slip costs provisioning plus twenty seconds.
  Lazy provisioning was considered and is worth less than it looks: `lint` is the second row and
  needs three tools bootstrap installs, so the saving is one row wide and the cost is a second
  column saying which rows need it.
- A machine whose bootstrap fails still gets nothing automatic, and that is accepted rather than
  solved. What changed is that it is no longer silent. Measured on this lane's own container, whose
  packaged QEMU is 8.2.2 and lacks `riscv-iommu-pci`: `script/bootstrap` exits 1 having installed
  nothing and broken nothing, and before that fix the developer was left holding one error about a
  QEMU device with no statement anywhere that the whole tier had been skipped. Read that way it is a
  gate silently giving somebody nothing, which is what *a gate people skip is not a gate* names,
  arriving from the other side. The exit now says **NO CHECKS RAN** in those words, lists the
  skipped tier **out of the table** rather than out of a second hand-written list, and names
  `script/ci-build fmt` as the one row safe under every failure mode.
- `fmt` is named by hand in that message and the rest are not. Which rows survive depends on
  *which part* of bootstrap failed, and the table has no column for that: an adequacy failure leaves
  `lint` and `image-permissions` perfectly runnable where a missing rustup leaves nothing. The
  message says so in prose rather than guessing, because a derived list that is wrong half the time
  is worse than a short one that is always right.
- `script/bootstrap` conflates two jobs, and that conflation is why the paragraph above exists.
  It installs what is missing *and* it verifies the environment is adequate, and the second can fail
  on a machine where the first had nothing to do and where most of the local tier would have run. A
  `--no-verify`, or a split between provisioning and adequacy checking, would let the no-argument
  path proceed on a machine that is merely out of date. That is independent of this milestone and is
  proposed rather than built here.
- **Eighteen roadmap blocks and `design/roadmap/README.md` still say `script/gates`**, and that is
  correct rather than outstanding for most of them: a `BUILT` block is an account of what happened
  under the names it happened under. Fifteen were judged accounts and left alone. The two live ones
  are listed in `## Follow-on`.

## Follow-on

- **Done.** `script/gates` deleted; `script/ci-build` carries the table; every `ci.yml` job names a
  check out of it; `CONTRIBUTING.md`, `.github/pull_request_template.md`, `notes/scripts.md`,
  `notes/check-inventory.md`, `notes/hvf-leg.md`, `notes/instruction-clock.md` and the six sibling
  scripts that referred to the old name are current.
- **Milestone 440.** What "no arguments" means and what the two tiers are called is an architect's,
  and it is milestone 440 (what `script/ci-build` with no arguments should mean); the mechanism
  shipped under the recommendation. Numbered on 2026-09-19 by milestone 433's drain of the pile.
- **Recorded.** A CI job can still bypass the table, and the gate that would catch it is refused for
  now; in this block's `BUGS`, with the reason.
- **Recorded.** Live references to the retired name in blocks a lane may not edit, handed to
  the integrator with file, line and replacement: `design/roadmap/0274-apple-silicon-isa-support.md`
  (a `NOT-STARTED` block, so live intent) and
  `design/roadmap/0341-instruments-nothing-runs.md`, whose `Gate:` line asked which instrument joins
  the retired script and was corrected when milestone 433 (drain the proposal pile to zero, and keep it there) numbered it on 2026-09-19. The other fifteen blocks are accounts
  and keep the old name, as did the four index rows, until the index was retired on 2026-09-21.
- **Milestone 397.** `script/bootstrap` conflates installing what is missing with verifying the
  environment is adequate, and the second failing is what costs a developer the whole local tier.
  Numbered on 2026-09-19 by milestone 433's drain of the pile, with the note that milestone 287's
  Linux source-build fallback landed the same day this was written and covers the container failure
  it was measured on.
- **Recorded.** `notes/scripts.md` claimed `script/lint`'s row was the longest markdown line in the
  repository; the counted-claim marker vouches for the number and nothing vouched for which line
  carried it, and it had moved to the roadmap index, since retired. Corrected in place.

## Index row

The gating set was written down twice, in `script/gates` and in `.github/workflows/ci.yml`, with nothing comparing them; `ci.yml` carried three prose comments
explaining why a check was not in the other list. Six places described the local set and four were
wrong, each having been correct when written. `script/gates` is retired rather than kept as a
wrapper (calef: a wrapper to avoid making the changes is an argument from implementation cost, and
it leaves two names for one thing). `script/ci-build` now carries one table of name, tier and
command, cheapest first: no arguments runs the `local` tier, names run exactly those, and every CI
job names one out of it. Tier names and the no-argument default are provisional pending calef.
