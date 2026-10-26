# The stranger test

Milestone 117 (the stranger test) turns `AGENTS.md`'s third principle into a measurement. The
principle is one sentence: *could a competent stranger, with only this repository, reach a passing
build and a correct mental model without opening a chat window?* Where the answer is no, that is a
bug in the tree and not in the stranger.

This page is the instrument's current state: what the test is, how to run it, the rubric, the
latest result, and what still fails a stranger. Each run's full record is an appendix, linked from
its heading under [Runs](#runs). The rubric was written on 2026-08-14, before any run, because a
rubric written afterwards grades generously.

## Why the tree cannot grade itself

calef cannot take this test, because he wrote the system. Nor can any agent that has worked in this
tree. An agent that spent a night merging pull requests here knows why `nife-dev` is a symlink and
what a lane is. Knowing the answer disqualifies you from being the instrument.

So a run needs a fresh context that has never seen the repository. It gets the repository and a
task, and nothing else: no brief on the conventions, no pointer to the right note, and no answer to
any question it asks.

## The protocol

1. A fresh context, not a summarized one. A handoff that says "read CLAUDE.md first" has already
   given away the finding a newcomer would not know to make.
2. One task, stated the way a new contributor would receive it. Not "evaluate the docs", which
   invites a review rather than an attempt.
3. Every question it asks is a defect, recorded verbatim. The questions are the deliverable, more
   than any score.
4. Every confident wrong answer is a worse defect, because a document that misleads costs more than
   one that is silent.
5. No help mid-run. If it is stuck, the measurement has finished.

The task text has been the same since run 1: *"Get the project building and its tests passing, then
write up what this system is and how you would add a new user program to it."* Since run 5 it also
tells the stranger that it is being measured.

## How to run it

`script/stranger-test` runs the protocol. Its header is the manual, and its `BUGS` section says what
it cannot do.

    script/stranger-test                       run against HEAD
    script/stranger-test --commit origin/main  run against a named commit
    script/stranger-test --prepare-only        build the isolated tree, probe it, and stop
    script/stranger-test --smoke               exercise the whole pipeline; not a measurement

Each thing it does is something an earlier run got wrong by hand:

- It clones the repository inside the stranger's working directory, not as it. Project
  instructions load from a directory and its ancestors, never its descendants, so no `AGENTS.md`
  arrives at turn zero. The file is still in the tree, where a stranger from GitHub would find it.
- It withholds the answer key: this page, its appendices under `notes/stranger-test/`, and its
  `notes/README.md` entry. It amends the deletion into the tip, so the working tree is clean.
  Markdown links to them elsewhere become plain text, so no link in the clone dangles.
  Mentions of earlier runs elsewhere stay, because cutting them would fabricate a different tree.
- It writes its own logs to a sibling directory, with no run number in any path.
- It shadows `pkill` and `killall` so a stranger following `README.md`'s quit instruction cannot
  kill another lane's emulator.
- It probes the isolation with a throwaway session before the run, and stops unless the answer is
  `NONE`.
- It debriefs the stranger in two passes, contamination first and then the rubric's questions.
- It leaves the machine's toolchain links alone. The clone links its own (`nife-dev-nife-<hash>`),
  which the next `cargo xtask std-src` anywhere removes once the clone is gone.

The harness reads the rubric's questions out of this page, so the table below is the only rubric.
Only the question column ever leaves this file. [The instrument's
history](stranger-test/instrument.md) records how each of these pieces was learned, and the rubric
amendments of 2026-08-18.

A run costs one stranger session plus one lane to brief, score and write it up. That is roughly
200k lane tokens on top of the stranger, and about half an hour of wall clock. The stranger process
alone cost between `$6.54` (run 6) and `$12.66` (run 4) in the four runs that recorded it.

## The rubric, written 2026-08-14, before the first run

Two halves. Only the first is mechanical.

### The build

From a clean clone: does `script/setup` then `script/test` reach green, following only what the
repository says?

| # | checked | pass means |
|---|---|---|
| B1 | a reading order exists | the stranger knows where to start without guessing among `README.md`'s sections |
| B2 | `script/setup` completes | or fails with a message that says what to install |
| B3 | `script/test` reaches green | on at least one architecture |
| B4 | nothing undocumented was needed | no step the reader had to know that no file states |

Record what it actually took, including anything the reader had to know that no file said. B4 is
the row that matters; the first three are hygiene.

### The mental model

Each question below is one the tree *claims* to answer. Grade against what the tree actually says,
not against what a maintainer knows. A question the tree answers only in a commit message is a
question the tree does not answer.

| # | question | pass means |
|---|---|---|
| M1 | What is a capability here, and what is the relationship between holding a reference to a thing and being permitted to use it? | names that holding the reference *is* the permission, with no separate check |
| M2 | Why is there no ambient network, and what must a program hold to reach one? | names a held capability rather than a config flag or a permission bit |
| M3 | Where does architecture-specific code live, and what breaks if it lives elsewhere? | `kernel/src/arch/`, and that the port becomes a diff across every file |
| M4 | What does `BUILT` mean on a roadmap row, and `PARTIAL`? | that it is a claim about the tree, and that the index and the block must agree |
| M5 | Why is there a `crates/` and a `components/src/`, and what decides which? | shared-by-two-binaries goes in `crates/`; host-testable and Kani-reachable is the reason |
| M6 | What is a `BUGS` section for? | a promise about known limits, not an apology, and next to the feature |
| M7 | How would you add a program, and what must you declare about it? | the grant manifest, and that a provisional name is expected |
| M8 | Who decides a name, and what provenance states can one be in? | calef; the states `script/names` accepts, which is four as of §89: `ratified`, `recorded`, `unrecorded`, `provisional` |

M8's states are those of §89 (`provisional` becomes the fourth provenance state). M1 and M8 were
amended on 2026-08-18, before run 4, after run 3 showed that M8 counted three states where there
were four and that M1 quoted a phrase the tree never wrote. The
[instrument appendix](stranger-test/instrument.md)
has both corrections.

Scoring is per question: answered, partly answered, wrong, or absent. "Wrong" is worse than
"absent" and is recorded separately, because a misleading document costs more than a silent one.

## The honest limits

An agent is not a person. It will not get bored or give up out of frustration, and it will read
further before asking than a human would. So every result here is a lower bound on the friction a
real newcomer would meet.

One run is an audit. The milestone is the loop: fix what a run finds, then run again with a
different stranger.

The rubric can be wrong. If a stranger falls down somewhere the rubric does not ask about, the
finding is real and the rubric is what needs amending.

## The cadence: monthly, decided 2026-08-18

calef decided on 2026-08-18 that the stranger test runs monthly.

**A run is due every 30 days.** Thirty rather than a calendar month, because a month is not a number
of days. `script/stranger-test --due` reads the integer out of that sentence, so the interval a
reader meets and the interval the tripwire uses are the same characters. Changing the cadence is
editing that sentence. The date of the last run comes from the `### Run <n>, <date>:` headings
under [Runs](#runs), which every run already writes.

The mechanism is notification. A run spawns a `claude` process, spends real budget, needs the pinned
toolchain and both QEMUs, and ends in a debrief a person scores. CI can do none of that. So:

- `script/stranger-test --due` exits 1 when a run is due and prints the command to run. It runs
  nothing.
- `.github/workflows/stranger-cadence.yml` asks it every Monday, in its own workflow, because a due
  run is information about the tree and not a defect in that week's commit.
- `script/stranger-test --check` runs in `script/lint`. It checks that the cadence sentence appears
  once, that the run headings are numbered from 1 without a gap, and that their dates are in order.
  A malformed record is a defect in the commit that malformed it.

Red means run the test. Closing it by editing a heading is available, cheap, and the one thing that
makes the whole mechanism a lie. Run 7 is due on 2026-10-19.

## The latest result: run 6, 2026-09-19

Run 6 went against `ebe6b784`, a month and about 2,500 commits after run 5. It ran on
`claude-sonnet-5`, because the CLI's default model refused the task text three times. Runs 1 to 5
did not record their model, so some of the difference may be the model.

Build half:

- B1 passed. For the first time a stranger opened a `design/decisions/` file, and it read
  `AGENTS.md` in two slices.
- B2 was not measured. The machine was provisioned, as on every run.
- B3 was green on aarch64, riscv64, x86_64 under PVH and the NVMe leg, then red at the last step:
  `uefi-test` exited 1 after its own suite passed. The operator's re-run on the same tree passed.
- B4 failed with six entries. One is that the pinned QEMU has no documented route on macOS.

Mental model: seven answered, M2 absent. The stranger answered M7 by adding a program
through milestone 150 (adding a program should not need eight hand-maintained lists) on the day it
landed, and the page held.

Its costliest mistake was a confident wrong answer the tree invited. Two expected VT-d fault lines
printed before the red, and nothing says they are expected. The stranger blamed the QEMU version and
built the pinned QEMU into an account-wide prefix, which would have changed the emulator under two
other lanes. The operator killed that build, and the stranger rebuilt it anyway. The session then
ended waiting on background work, so no write-up exists; the debriefs recovered its findings. [Run
6's appendix](stranger-test/run-6.md) has the configuration, both interventions, the scores, eight
findings with their homes, and the four predictions.

## What still fails a stranger, as of 2026-09-24

- M2 is unanswerable by ordinary work. It was absent for the fourth run in six. No stranger has
  ever opened `notes/net.md`, which answers it. Nor has any stranger opened `notes/scripts.md` or
  `notes/README.md`. The notes index was cut to one line per note on 2026-09-24; no stranger has met
  that version.
- No stranger has ever run `script/apropos`. Run 5 had its name in front of it three times.
  It was placed in `README.md` and `CONTRIBUTING.md` on 2026-09-19, after run 6's clone was cut, so
  run 7 is the first measurement of that placement.
- `uefi-test` can exit 1 after its own suite passed, beside fault lines that read as the cause. It
  is milestone 516 (`uefi-test` can exit 1 after its own suite has passed), not started. The VT-d
  lines are documented as expected in `notes/x86-uefi-boot.md`'s `BUGS`.
- The pinned QEMU cannot be had on macOS by any documented route, and a hand-built one lands in a
  prefix every checkout on the account uses. Both are in `notes/qemu.md`'s `BUGS`.
- B2 has never been measured on a cold machine. Every run started with the toolchain and QEMU
  installed; run 2 came closest.
- A stranger knows it is measured within minutes, from records the tree is required to keep. The
  disclosure since run 5 makes that legible, not absent.

Fixed since the run that found them:

- The Linux bootstrap loop from run 2, by milestone 287 (installs a working QEMU on Linux) on
  2026-09-13.
- The eight hand-maintained program lists that runs 3, 4 and 5 named, by milestone 150 on
  2026-09-19.
- `std-aborts` blaming upstream source for another worktree's farm, from run 5. It asserts its
  dep-info paths are under `farm_dir()` since before run 6 (2026-09-19).
- Four of run 6's findings, fixed in its own lane on 2026-09-19. They were `CONTRIBUTING.md`'s
  two-ISA claim, `README.md`'s claim that QEMU is pinned exactly on every machine, the missing
  warning that a bare `cargo build` fails on the host, and the harness debriefing a refused session.

## Runs

Each heading is what `--due` and `--check` read. The appendix under each is the full record: what
was decided before the run, the configuration, the scores, and what it cost.

### Run 1, 2026-08-14: an x86_64 container, no QEMU

The stranger found that the workspace had not built on an x86_64 host since 2026-08-03, because CI
had moved to an arm runner the same day. It also found the rubric by grep, so two answers were
contaminated and no score was recorded. [Run 1](stranger-test/run-1.md).

### Run 2, 2026-08-16: a stock Linux box, and a stranger that was not one

Green on both ISAs, with a program added. `script/setup` could not complete on stock Ubuntu, and the
first fix for that was half a fix, completed on 2026-09-13. The stranger was a subagent with `AGENTS.md` in
context at turn zero, so five rubric rows went unscored. [Run 2](stranger-test/run-2.md).

### Run 3, 2026-08-18: a process that could not see the repository, and a harness that told it anyway

The first properly isolated stranger, a separate `--safe-mode` process started above the clone.
`script/test` went red 2 times in 13 at load 45 to 63, from other lanes, and nothing told it. B1
failed: there was no reading order. The harness's own log names told it which run it was.
[Run 3](stranger-test/run-3.md).

### Run 4, 2026-08-18: the harness stopped leaking and the tree took over the job

Green first try, eight of eight answered, B1 passed on the new `## Start here` order. The stranger
learned it was measured from `notes/adding-a-program.md`'s record of earlier walks. The stranger
read `CONTRIBUTING.md`, item 2 of 8, sixteenth of twenty-two. [Run 4](stranger-test/run-4.md).

### Run 5, 2026-08-18: the first run through the harness, and the machine leaked where the tree did not

The first run through `script/stranger-test`, and the first told it was measured. The account-wide
`nife-dev` link made `std-aborts` fail with paths inside another checkout. `AGENTS.md` was never
opened, and `script/apropos` was never run. [Run 5](stranger-test/run-5.md).

### Run 6, 2026-09-19: a different model, a program in two edits, and a stranger that provisioned the machine

Summarized in [the latest result](#the-latest-result-run-6-2026-09-19). [Run
6](stranger-test/run-6.md).

## BUGS

These are the limits still open on 2026-09-24.

- The fact of the test leaks and always will. `README.md`, `CONTRIBUTING.md` and
  `notes/adding-a-program.md` cite earlier runs, and each run adds to that record. The harness
  withholds only the answer key. So a stranger's prose is performance for a known reader, and runs
  4, 5 and 6 each said so; discount it by default.
- The withheld key is recoverable from git. `git show HEAD~1:notes/stranger-test.md` returns it.
  Closing that would rewrite every commit, so it is a deliberate trade, recorded in the harness's
  `BUGS`.
- The harness isolates a tree, not a machine. A stranger can reach the QEMU prefix and other
  machine-wide state, and run 6 did. A container is the only real answer.
- The cadence has no reader. `--due` goes red in an Actions tab that nobody watches, and the watcher
  cannot tell due from dead. That is milestone 495 (a cadence that says "due" reaches nobody), not
  started. Run 6 happened because a maintainer briefed a lane.
- The operator is never a stranger. Whoever runs and scores this has read `AGENTS.md`. The
  mitigations are an unchanged task text, a rubric that predates every run, and answers recorded as
  the stranger gave them.
- The rubric ages against a moving tree. A row that counts something goes stale first, as M8 did.
- `**A run is due every 30 days.**` is bold because `script/stranger-test` parses it. It is markup,
  not emphasis, and it counts against the bold budget of §213 (writing standards) like any other
  span.
- Six runs by six agents are not six data points about a person. Every number here is a lower bound.
