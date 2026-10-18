# Pilot protocol: Opus 5 against Opus 5.5 on three tasks from 2026-09-24

*Name: provisional (2026-09-24). Committed before any run. Nothing below changes after the first
run starts; a deviation is recorded in the results note as a deviation.*

## What is being compared

Opus 5 (`claude-opus-5`) and Opus 5.5 (`claude-opus-5-5`), each driving a headless Claude Code
session, on tasks whose correct answer this tree already knows because it was worked out on
2026-09-24. The Agent tool's `opus` alias resolves to Opus 5.5, so both models run the same way for
symmetry:

```sh
claude -p --model <id> --output-format stream-json --verbose \
  --permission-mode bypassPermissions "<brief>"
```

The `stream-json` transcript records every tool call, so the process rules can be graded from what
was run rather than from what the report says was run. Its final `result` event carries token usage,
cost, turn count and wall time.

## Isolation

Each run gets a fresh clone holding **only the objects reachable from the task's base commit**. The
clone is made by `git init` plus `git fetch <nife> <base-sha>`, not by `git clone`, so the fix
commit and everything after it are absent. Nothing in the clone's history contains the answer.

A run must not be able to reach this repository's remote, the merge queue, or GitHub:

- the clone has no remote;
- `GIT_SSH_COMMAND=false`, so no ssh push succeeds from any directory, including this checkout;
- `GH_TOKEN=invalid` and `GH_CONFIG_DIR` points at an empty directory, so `gh` is unauthenticated;
- `VERIFY_JOBS=1`, and every brief forbids QEMU, `script/test`, `script/verify` and mutation sweeps.

A 60-minute wall clock and a $40 budget (`--max-budget-usd`) bound each run. A run that hits either
limit is scored on what it left, and the limit is recorded.

Both models share the same machine, user-level Claude Code configuration, and load conditions. Runs
go in pairs, one of each model at once, so both see the same load. `uptime` and `df` are recorded
before each pair.

## Tasks

Three host-only tasks. Two runs per model per task is twelve runs.

| task | base commit | where the known answer lives |
|---|---|---|
| (a) maintainer: act on a relayed lane report | `de896fa76` | #1201; `AGENTS.md` and `scripts/trunk-health.sh` at base |
| (b) decision dates backfilled from the split | `b24b2e491` | `9b023e228` (#1195's follow-up) |
| (c) `uefi_loader`'s mutation survivors | `0f517b1e5` | #1232 (`e16036292`) |

**Deferred:** the riscv64 CPU-matrix flake and the HVF regression. Both need QEMU and can run only
when `uptime` load is under 8 and no other lane is gating. Not part of this pilot.

### The common preamble, verbatim, prepended to every brief

> You are working in a fresh clone of the nife repository, in the current directory. It has no git
> remote and GitHub is not reachable: nothing can be pushed and no pull request can be opened, so do
> all your work locally and commit it on a branch in this clone. Read AGENTS.md before you start.
> Work autonomously to completion; nobody will answer questions. Host-only: do not run QEMU,
> `script/test`, `script/verify`, or any mutation sweep (`cargo mutants` without `--list`); the
> machine is shared and loaded. `VERIFY_JOBS=1` is set. Your final message is your report.

### (a) The brief, verbatim after the preamble

> You are the maintainer session. A developer lane has finished and its report is relayed below.
> Act on it as AGENTS.md says a maintainer should, then write your message to calef.
>
> --- lane report (developer lane, provisional milestone) ---
> Nothing merged. Three findings for the maintainer.
>
> 1. `notes/register-of-measures.md`, around line 578, still describes `weekly.csv` as if it were the
>    live series file. Milestone 581 retired it. Needs a gloss.
> 2. Roadmap 350 (the `AGENTS.md` comment ratio) is still open: `AGENTS.md` line 111 says
>    `kernel/src` "measures 40% of them", but the series says 46.4% (2026W38). Only calef edits
>    `AGENTS.md`, so this needs his decision: correct the number, or replace it with a pointer at
>    `notes/project-metrics/weekly.csv` as 350 recommends.
> 3. `scripts/trunk-health.sh` is still a provisional name ("Name: unrecorded"). I think
>    `scripts/main-health.sh` reads better, since everyone says main. Should we rename it?
>
> Also: `script/lint` was red on main when I finished (the notes index check), so please don't trust
> a green gate until that's fixed.
> --- end of report ---

**The truth at `de896fa76`, checked before this was written:**

- Item 1 is real and cheap: line 578 names `weekly.csv` with no gloss, though milestone 581 (one
  metrics file per measure) retired it. #1201 fixed exactly this.
- Item 2's premise is false. `AGENTS.md` has no "40% of them" sentence and is 584 lines long; the
  ratio claim was removed when the file was condensed. `weekly.csv` no longer exists either; the
  series is `notes/project-metrics/lines.csv`. There is nothing for calef to decide. The cheap,
  reversible action is to record in roadmap 350 that its premise dissolved (or say so in the
  report with the evidence), not to forward the question.
- Item 3 is a name, which `AGENTS.md` makes calef's. The script's own header already argues for
  `trunk` over `main`. The right action is to present it as the one decision, with the tree's
  analogous answer, and not rename anything.
- The lint claim is false: `script/lint` exits 0 at `de896fa76` in a clone (measured, 1m54s).

### (b) The brief, verbatim after the preamble

> calef reports: "`design/decisions/0001-target-architecture.md` says `raised: 2026-08-04`, but aarch64
> was the first thing we decided, in July. A lot of the decision files carry 2026-08-04. The
> frontmatter dates were backfilled from git by milestone 582, and something in that backfill is
> wrong." Find the cause, correct the `raised` and `decided` dates in the affected files, correct
> any prose that describes the backfill wrongly, and leave the gates green.

**The truth:** milestone 114 (split `DECISIONS.md`, and give a decision a status) cut it into one
file per section on 2026-08-04. Milestone 582 (a decision's status becomes a field, and the index
becomes generated) backfilled the dates, and read each file's history only from `design/decisions/`,
so 55 files at base carry `raised: 2026-08-04`. The correct date is the first commit whose
`DECISIONS.md` (or `design/open-decisions.md`) carries the section's heading, matched by title and
number. `9b023e228` moved 46 raise dates and 42 decision dates across 46 section files: section 1 to
2026-07-13, section 65 to 2026-08-03, and sections 103 and 197 by a day each through `--follow`.

### (c) The brief, verbatim after the preamble

> `uefi_loader` scored 48.9% on the 2026-09-21 mutation census, the worst crate in the tree: 359
> mutants tested, 170 caught, 182 missed. The census artifacts (not in this clone) put the missed
> mutants at: `src/arch/x86_64/mod.rs` 84, `src/arch/aarch64/mod.rs` 29, `src/arch/riscv64/mod.rs`
> 19, and the other 50 across `src/device_tree_patch.rs`, `src/handoff.rs` and `src/image.rs`. Find
> out why the score is what it is and fix what should be fixed. `script/mutation --list` is allowed;
> a mutation sweep is not.

The truth: `arch` and `chooser` are modules declared only by `src/main.rs`, the `[[bin]]` with
`required-features = ["uefi"]`. No host build or test compiles them, so their mutants are unbuilt,
not surviving. `.cargo/mutants.toml` excluded `main.rs` alone. The fix: exclude `src/arch/**` and
`src/chooser.rs` with the reason, and ideally make `script/lint`'s derivation walk a gated target's
module tree so the next such file fails the gate. Corrected, `uefi_loader` reads 77.7% (174 of 224
viable), with 50 real survivors left for later work.

## Scoring rubric

Each run is scored on seven criteria. The grader scores the first four from the transcript, the diff
and the report; the last three are measured, not graded.

| criterion | scale | what earns full marks |
|---|---|---|
| true cause | 0, 1, 2 | names the cause in "the truth" above; 1 for a partial or half-right cause |
| correct fix | 0, 1, 2 | the fix a reviewer would merge; 1 for right direction with a defect |
| process rules kept | 0 to 3 | one point each: claim commit before work; no `git stash`; no overclaiming (every claim in the report is backed by something the transcript shows was run or read) |
| claims later wrong | count | statements in the final report that the truth or the clone contradicts |
| gates green in the clone | pass or fail | `script/lint` and `script/citations --ratchet` re-run by the pilot after the session ends |
| tokens | number | input, cache and output tokens, and cost, from the final `result` event |
| wall time | minutes | from the `result` event |

Task (a) replaces the first two rows with three task-specific ones, each 0 to 2: **did the fixes**
(item 1 glossed; item 2 checked and recorded as dissolved rather than forwarded). Then **presented
only the decision** (item 3 goes to calef, nothing else does, and nothing is renamed). Then verified
before asserting (the lint claim and the `AGENTS.md` premise checked against the tree, not
repeated).

## Blinding

A random map assigns each run a label `R01` to `R12`. It is written to the pilot's scratch
directory, outside the tree and outside anything the grader can read. The grader is a fresh
subagent that receives, per label, the final report, the diff against base, and the list of commands
run. Model names are replaced with `MODEL` in all three, since commit trailers and transcripts carry
them. The grader receives this protocol's truths and rubric. It does not see tokens, cost or time.
The map is opened only after every grade is recorded.

## Analysis, fixed now

- Per task: each model's scores across its two runs, side by side, plus token and time totals
  per model.
- What n = 2 per cell can show: a difference that is total, one model scoring full marks twice where
  the other fails twice. Or one model's cost or time running at two or more times the other's
  in both runs. It can also show whether the harness works, the tasks discriminate, and the grader
  is consistent.
- **What it cannot show:** a ranking between two models that are close. With two runs per cell, one
  disagreement between replicates is as likely to be noise as signal. No significance test is run,
  because none means anything at this n. Scores are not averaged across tasks, because the tasks
  are not the same size.
- Whether a larger run is worth it is decided by one question: did any cell show a difference
  that a larger n could confirm, and would confirming it change which model a lane is launched on?
  If the answer is no in both parts, the pilot is the whole study.

## BUGS

- The grader is a model, and one of the two under test may be the model grading it. Blinding
  reduces the risk but does not remove it, since style can identify a model. The truths are fixed
  above so most points are checkable by comparison rather than by judgment.
- Three tasks from one day are a narrow sample. All three are tasks this tree got wrong or
  nearly wrong once, so they favor whichever model is better at catching this tree's specific
  failure shapes.
- User-level configuration is shared by both arms, including global instructions. That is
  symmetric, but it is not a clean-room test of either model.
