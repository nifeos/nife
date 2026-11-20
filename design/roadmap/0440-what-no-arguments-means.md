---
status: NOT-STARTED
raised: 2026-09-13
milestone_dependencies: none
decision_dependencies: 183
machine_requirements: none
specific_machine: none
needs_person: no
---
# 440. What `script/ci-build` with no arguments should mean

Filed 2026-09-13 as an unnumbered proposal by milestone 286's lane, out of
the milestone calef minted the same day; numbered 2026-09-19 by milestone 433's drain of the
proposal pile. **Premise re-read against the tree on 2026-09-19 and still true**. `script/ci-build`
still carries the `local`/`ci` tier column and still runs the `local` tier on the no-argument path.
Its own header still says in capitals that the tier tags and the no-argument meaning are
**PROVISIONAL** pending calef. The table has grown since: 17 rows, 9 `local` and 8 `ci`, where
the proposal priced option A against fifteen. That makes A's cost one word per row on seventeen rows
rather than fifteen, and changes nothing else. *(Number provisional until the merge queue lands
it.)*

The decision is
§183 (what `script/ci-build` with no arguments means, and what the two tiers are called), written up 2026-09-19 by
milestone 435's slice-c lane because this gate named no section.
Milestone 286 collapsed two enumerations of "the checks that gate a pull
request" into one table inside `script/ci-build`. The table needs a way to say which checks a
developer waits for before pushing and which only a runner waits for, and how that is spelled is
calef's. It is a name a contributor meets in `CONTRIBUTING.md`, in `--list` output and in every
row of the table. The mechanism is built and shipped under the recommendation below; only the
spelling and the default are open.

It was numbered 400 for four hours and is 440 because another session got there first. That
session landed its own milestone 400 (the shell on the firmware screen) while this branch waited in
the queue, so both meanings of "milestone 400" existed in the tree at once and the citations had to
be sorted by hand. Seven of them meant this block and five meant the other. §194 (sessions
interleave rather than serialize) rules that the
interleaving stays and the renumber is its price, which this is the fifth instance of in one
evening and the first in milestone numbers rather than section numbers. 

In brief. `script/ci-build` now carries one row per check, each with a tier. With no arguments
it runs the `local` tier in the table's order, cheapest first. With names it runs exactly those,
which is how `ci.yml` fans them into parallel jobs. The question is what "no arguments" should mean
and what the two tiers should be called.

## Why it cannot mean everything

`script/verify` is about 47 minutes of Kani. `script/cpu-matrix` is five QEMU boots of the riscv64
suite. `script/coverage` instruments and re-runs the whole host suite; `script/fuzz` is 60 seconds
per target by construction.

The retired `script/gates` carried the sentence that decides this, and it has to survive the
milestone that deleted the file: **a gate people skip is not a gate.** A command that took an hour
would be run once, by whoever wrote it. So the no-argument path is a subset, and the only question
is how the subset is named and defended.

## The options

| | what "no arguments" means | how the subset is decided | cost |
|---|---|---|---|
| **A** | the `local` tier | a tier tag, one column in the table | shipped; one word per row |
| **B** | everything, with `--fast` for the subset | inverted: the default is honest and unusable | the default is an hour, which is the thing the retired script's header refused |
| **C** | everything the machine can do **now** | derived: run what the host can support, skip the rest loudly | no tag at all, and the set a contributor gets depends on their laptop |
| **D** | nothing; require a name every time | no subset exists | deletes the one command to remember, which is why `script/gates` was written on 2026-08-03 |

A is what was built. B, C and D are refused for the reasons in the right-hand column, and C is
worth a sentence more than the others because it is the tempting one. It removes a hand-maintained
tag, which is exactly what this milestone is about. It fails on the newcomer principle rather than
on mechanism. Two contributors would run different sets from the same command, and neither would be
able to say what the other's green meant. The `hvf` check already shows the shape of that (it skips
loudly where Hypervisor.framework does not exist, and says in plain words that nothing executed on a
physical core). One such check is a recorded gap where fifteen would be a lottery.

## What the tags should be called, which is the actual open question

Shipped provisionally as `local` and `ci`. The refusals are the useful half:

- `local` / `ci` (shipped). `local` says who waits: a person at a checkout. `ci` says the same
  of a runner. The strongest argument against the pair is that `ci` names a deployment rather than
  a property of the check. Nothing about `script/coverage` changes if this repository stops using
  GitHub Actions, and the tag would be wrong the same day while the check was untouched. That is the
  shape of mistake this tree has made before with a display name matched by branch protection, and
  it is the reason to keep the pair provisional rather than ratify it in place. `local` is separately
  one of the vaguest words available in an operating system.

  **`runner` is not available**, which is worth recording because it is the obvious fix: it names
  who waits, in a noun, and it is GitHub's own word. This tree already spends it in two senses, the
  CI machine and a script that runs something, across 163 files (`git grep -lw runner`, measured
  2026-09-13), including four entry points named for it: `helpers/qemu-runner-aarch64.sh` and its two
  siblings, `helpers/memory-bounded-runner.sh`, and `script/runner-container`. A third sense would
  cost a reader the recognition, which is the ground `DECISIONS §31` already refused `witness` on.
- `before-push` / `ci-only`. Says what a contributor does rather than where it happens, which is
  the question they are actually asking. Against it: two hyphenated compounds where a column wants a
  word, and `before-push` names a git hook that already exists (`.githooks/pre-push`) and runs a
  different, smaller set.
- `fast` / `slow`. Refused. It is false in both directions: `image-permissions` is in the local
  set and builds three kernels, and `supply-chain` is out of it and takes seconds on a warm cache.
  The real criterion is what a person will wait for, which is not a duration.
- `gate` / `report`. Refused. `DECISIONS §134` (a harness carries a machine-replayable
  falsification record) already spends that split on the `script/` family
  itself (a verb does something, a noun reports). Reusing the words one level down would make
  `coverage` a report in one sense and a gate in another on the same page.

**The noun rule does not reach this.** An earlier draft of this proposal argued that `local` and
`ci` are adjectives where a tier, being a thing, wants a noun. That is a misreading and it is
corrected here rather than deleted, because the misreading is easy: `AGENTS.md` governs crates,
programs and shared modules ("a crate, a program or a module is a *thing*, so it takes the name of
a thing"), and a value in a table column is none of the three. This tree's own enum variants already
sit where verbs are right, `Direction::Serve` and `Direction::Use` among them, and nobody has ever
proposed renaming them. So the case against the shipped pair rests on `ci` naming a deployment, not
on its part of speech.

## What each option costs, measured

- A costs one word per row, and nothing else. There are fifteen rows.
- The drift A is meant to prevent was measured on 2026-09-13, before the change: three separate
  prose comments in `.github/workflows/ci.yml`, a row in `notes/scripts.md` and two contributor-facing
  sentences all described the local set, and four of the six were wrong. `ci.yml` said
  `script/icount` was not in it (milestone 62 put it there). `notes/scripts.md` listed four checks
  where the script ran six. `CONTRIBUTING.md` and `.github/pull_request_template.md` both said
  "five". Every one had been correct when written.
- B's cost is the whole point of the retired script, so it is not priced further.
- C costs nothing to build and costs the third principle: a newcomer cannot get a correct mental
  model of what green means without knowing the machine it was measured on.

## Reversibility

High, and that is why the mechanism shipped under a recommendation. The tags appear in one
column of one table, in `--list` output, and in three sentences of prose. Renaming them is one
commit and no wire format, no system call, and nothing two programs agree on. **What is not cheap is
the default**. `CONTRIBUTING.md` and `.github/pull_request_template.md` now tell a stranger that
`script/ci-build` is the one command to remember. A contributor who learns it and then finds it
means something else is the expensive half of `AGENTS.md`'s *move fast on what can be undone*.

## What is blocked until this is answered

Nothing. The tags are provisional and say so in `script/ci-build`'s header, in `notes/scripts.md`
and in milestone 286's block. A `**Proposed.**` bullet in that block points here.

## Index row

Milestone 286 collapsed two enumerations of "the checks that gate a pull request" into one table
inside `script/ci-build`, with a tier column saying which checks a developer waits for before
pushing and which only a runner waits for. How that column is spelled is an architect's, because it
is a name a contributor meets in `CONTRIBUTING.md`, in `--list` output and in every row. It cannot
mean everything: `script/verify` is about 47 minutes of Kani, and the retired `script/gates` carried
the sentence that decides it, which has to survive the milestone that deleted the file, that **a
gate people skip is not a gate**. So the no-argument path is a subset and the only question is how
the subset is named and defended. Four options are priced, and the tempting one is refused on the
newcomer principle rather than on mechanism. Deriving the set from what the machine can do now
removes the hand-maintained tag and makes two contributors run different sets from the same command,
neither able to say what the other's green meant. The shipped pair is `local` and `ci`, and the
strongest argument against it is that `ci` names a deployment rather than a property. So it would be
wrong the day this repository stopped using GitHub Actions while the check was untouched. `runner`
is the obvious fix and is not available: this tree already spends it in two senses across 163 files,
including four entry points named for it, and a third sense costs a reader the recognition. The tags
are cheap to change; the default is not, because `CONTRIBUTING.md` now tells a stranger this is the
one command to remember.
