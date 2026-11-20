---
status: PROPOSED
raised: 2026-09-19
---

# 183. What `script/ci-build` with no arguments means, and what the two tiers are called

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice-c lane, which found milestone 440 (what `script/ci-build` with no arguments should mean)'s
`DECISION` gate naming no section. The mechanism is built and shipped under the recommendation
below; only the spelling and the default are open. *(Section number provisional until the merge
queue lands it.)*

## What is being decided

Milestone 286 collapsed two enumerations of "the checks that gate a pull request" into one table
inside `script/ci-build`, one row per check with a tier. Two things in it are calef's:

1. What "no arguments" means. A contributor meets this in `CONTRIBUTING.md` and in
   `.github/pull_request_template.md` as the one command to remember.
2. What the two tiers are called. The tag appears in `--list` output and in every row.

## Is the premise true

Checked 2026-09-19 in this worktree. Yes. `script/ci-build`'s table carries 17 rows, 9 `local` and
8 `ci`, and the no-argument path runs the `local` tier in the table's order. The script's own
header says in capitals at line 37 that **the tier tags and the no-argument meaning are PROVISIONAL
pending calef**.

## Why it cannot mean everything

`script/verify` is about 47 minutes of Kani. `script/cpu-matrix` is five QEMU boots of the riscv64
suite. `script/coverage` instruments and re-runs the whole host suite, and `script/fuzz` is 60
seconds per target by construction.

The retired `script/gates` carried the sentence that decides this and it has to survive the
milestone that deleted the file: **a gate people skip is not a gate.** A command that took an hour
would be run once, by whoever wrote it. So the no-argument path is a subset, and the only question
is how the subset is named and defended.

## The options for the default

| | what "no arguments" means | how the subset is decided | cost |
|---|---|---|---|
| **A** | the `local` tier | a tier tag, one column | shipped; one word per row |
| **B** | everything, with `--fast` for the subset | inverted | the default becomes an hour, which is what the retired script refused |
| **C** | everything the machine can do now | derived from the host | no tag, and the set depends on the contributor's laptop |
| **D** | nothing; require a name every time | no subset exists | deletes the one command to remember |

C is the tempting one and fails on the third principle rather than on mechanism. It removes a
hand-maintained tag, which is what milestone 286 was about. But two contributors would run different
sets from the same command and neither could say what the other's green meant. The `hvf` row already
shows the shape: it skips loudly where Hypervisor.framework does not exist and says in plain words
that nothing executed on a physical core. One such row is a recorded gap; seventeen would be a
lottery.

## The tags, and the refusals, which are the useful half

Shipped provisionally as `local` and `ci`.

- `local` / `ci`. `local` says who waits, a person at a checkout; `ci` says the same of a runner.
  The strongest argument against the pair is that `ci` names a deployment rather than a property
  of the check. Nothing about `script/coverage` changes if this repository stops using GitHub
  Actions, and the tag would be wrong that day while the check was untouched. `local` is separately
  one of the vaguest words available in an operating system.
- **`runner` is not available**, and this is a lookup rather than a preference. This tree already
  spends the word in two senses, the CI machine and a script that runs something, across 163
  files (`git grep -lw runner`, measured 2026-09-13), including four entry points named for it:
  `helpers/qemu-runner-aarch64.sh` and its two siblings, `helpers/memory-bounded-runner.sh`, and
  `script/runner-container`. A third sense costs a reader the recognition, which is the ground §31
  already refused `witness` on.
- `before-push` / `ci-only`. Says what a contributor does rather than where it happens. Against:
  two hyphenated compounds where a column wants a word, and `before-push` names a git hook that
  already exists (`.githooks/pre-push`) and runs a different, smaller set.
- `fast` / `slow`. Refused, and false in both directions: `image-permissions` is `local` and
  builds three kernels, `supply-chain` is `ci` and takes seconds on a warm cache. The real criterion
  is what a person will wait for, which is not a duration.
- `gate` / `report`. Refused: §134 already spends that split on the `script/` family itself
  (what does something is a verb, what reports is a noun). Reusing the words one level down
  would make `coverage` a report in one sense and a gate in another on the same page.

**The noun rule does not reach this**, and the misreading is corrected here rather than deleted
because it is easy to make. `design/naming.md` governs crates, programs and shared modules, and a
value in a table column is none of the three; this tree's own enum variants already sit where verbs
are right (`Direction::Serve`, `Direction::Use`). So the case against the shipped pair rests on `ci`
naming a deployment, not on its part of speech.

## What each option costs, measured

- A costs one word per row, on seventeen rows, and nothing else.
- The drift A prevents was measured on 2026-09-13, before the change: three prose comments in
  `.github/workflows/ci.yml`, a row in `notes/scripts.md` and two contributor-facing sentences all
  described the local set, and four of the six were wrong. `ci.yml` said `script/icount` was not
  in it (milestone 62 put it there). `notes/scripts.md` listed four checks where the script ran six.
  `CONTRIBUTING.md` and the pull-request template both said "five". Every one had been correct when
  written.
- C costs nothing to build and costs the third principle, which is the expensive currency here.

## Recommendation

Keep A as the default. On the tags, this section deliberately names no winner, because a tag a
contributor learns is a name and names are calef's. What it offers is the refusals with their
reasons, which is the half that is a lookup rather than an argument.

## How reversible, and who has acted on it

The tags are high, the default is not, and they should be answered as two questions rather than
one. The tags appear in one column of one table, in `--list` output and in three sentences of prose,
so renaming them is one commit and touches no wire format and no system call. The default is already in
`CONTRIBUTING.md` and `.github/pull_request_template.md` telling a stranger this is the one command
to remember. A contributor who learns it and then finds it means something else is the expensive
half of *move fast on what can be undone*.

## What is blocked until this is answered

Nothing. The tags are provisional and say so in three places. Milestone 440 is the only thing
waiting, and a `**Proposed.**` bullet in milestone 286's block points at it.
