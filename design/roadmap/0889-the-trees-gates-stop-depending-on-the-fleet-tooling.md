---
status: PARTIAL
raised: 2026-10-12
promoted_from: the-trees-gates-stop-depending-on-the-fleet-tooling
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 889. The tree's gates stop depending on the fleet tooling

*(Promoted 2026-10-12 (UTC) by lane `milestone/889-the-trees-gates-stop-depending-on-the-fleet-tooling`
from the proposal `the-trees-gates-stop-depending-on-the-fleet-tooling`, at the next free number.
The number is provisional until the merge queue lands it; the title, the slug and every new name
below are drafts for calef's naming pass.)*

Raised 2026-10-12 (UTC) by lane `maintainer/tooling-inventory`. It is the first step calef ruled
on pull request #1976: Fork 4 of
[nife's tooling, sorted by who needs it](0902-tooling-sorted-by-who-needs-it.md), answered
[*"A"*](https://github.com/nifeos/nife/pull/1976#issuecomment-6117241317) on 2026-10-12 (UTC). The
edges are cut in place, in one nife lane, before any fleet tooling leaves. The title, the slug and
every new name below are provisional, for calef's naming pass.

Reuse: nothing new is written beyond a split of existing files. `toolchain-bump.yml` already skips
its App token step when the App is absent, and the other four workflows take that pattern as it is.

## Why

The inventory found six places where a build, proof or standards item cannot run without the
agent-fleet tooling (class 3). Until they are cut, moving class 3 to its own repository (Fork 2,
ruled A) is a rewrite rather than a move. Cutting them also gives a stranger a `script/lint` and a
`pre-push` that never touch the merge queue or GitHub.

## The work

Each item is one edge from the inventory. Nothing moves to another repository in this milestone.

1. `script/lint` stops running class-3 checks. The 22 checks (19 selftests of the queue, label and
   identity machinery, rules 4 and 4b, and the agent-instruction index) move into a new entry point,
   provisionally `script/lint-fleet`. A new `ci.yml` job runs it on every pull request and merge
   group, so no check stops gating. `script/claim` keeps calling `helpers/branch-name-check.sh`.
2. `.githooks/pre-push` runs `helpers/push-queued-check.sh` only on a checkout that opts in,
   provisionally with `git config nife.fleet true`. `script/bootstrap` sets it where it already
   installs the hooks for a lane. A stranger's push then asks GitHub nothing.
3. `falsifications`, `mutation`, `vendor-watch` and `verify-shard-refresh` gate their
   `create-github-app-token` step on the App's id being present, as `toolchain-bump.yml` does. They
   fall back to `GITHUB_TOKEN`, so a fork runs them without the `nife-smelter` App.
4. `metrics.yml` splits in two. Its method collectors, `merge_queue_share.py` and
   `debt_paydown_share.py`, move to a new scheduled workflow, provisionally `fleet-metrics.yml`,
   that writes the same CSV files. Everything else, CI and proof times included, stays.
   `script/metrics` already reads those files and needs no change.
5. `script/cadence-check` stops calling `script/effort --check` and `helpers/watcher_watch.py`
   itself. Both checks move into `fleet-metrics.yml`.
6. `notes/scripts.md` lists the fleet entry points in their own section. `AGENTS.md`'s skills index
   is left alone: it is calef's to edit, and the lane proposes the wording in its report.

Two edges are kept on purpose, and the lane records why where a reader meets each:

- `ci.yml`'s draft gate. Skipping CI on a draft costs a contributor nothing and saves runner time,
  so it is a sensible default for any repository, not a fleet dependency.
- `helpers/scope-merge-base.sh` and `helpers/job-budget.py`. Both are class-1 CI plumbing that the
  fleet also reads. A fleet item reading a class-1 one is the normal direction and needs no cut.

## Exit

- A throwaway copy of the tree with every class-3 helper, script and workflow deleted passes
  `script/lint --no-cargo`. The appendix of the inventory lists which files those are.
- `script/lint-fleet` runs green in CI on the same commit, and each of its 22 checks fails when the
  file it tests is broken, as each did inside `script/lint`.
- `git push` from a checkout without `nife.fleet` runs no `gh` command, checked with a stub `gh` on
  `PATH` that fails if called.
- Each of the four workflows runs to completion on a dispatch with the App secrets unset.
- `fleet-metrics.yml` writes byte-identical CSV rows to `metrics.yml`'s last run for the same week.

## Where it stands, 2026-10-20 (UTC)

PARTIAL. Every cut is made, and four of the five exit checks were run and pass. calef ruled on the
fifth (below): with the App absent, a workflow completes through its push. Three workflows do or
are measured doing so; `falsifications` cannot reach its push for two reasons the App does not
touch. calef, 2026-10-20 15:00 (UTC): "Hold 889 until we fix both issues." Both are this
milestone's remaining items (Follow-on). Each tie from the inventory, checked against the code
first:

| Tie | What the code did | What happened |
|---|---|---|
| 1, lint | 22 fleet checks ran inside `script/lint`, about 170 lines | Moved whole to `script/lint-fleet`, a step of CI's `lint` job |
| 2, pre-push | Every push asked GitHub twice, queued and ready | Behind `nife.fleet`, in `helpers/pre-push-fleet.sh` |
| 3, App token | Four workflows could not run without the App | Token step skipped without it; `github.token` falls back |
| 4, draft gate | Reads GitHub's own draft flag only | Kept; the reason is written in `ci.yml` |
| 5, metrics | `metrics.yml` ran two fleet collectors | Moved to `fleet-metrics.yml`, same CSV files |
| 5, cadence | `cadence-check` called `script/effort` and `watcher_watch.py` | Both asked by `helpers/trunk-health.sh` instead |
| 6, shared helpers | `ci_job_times.py`'s selftest read two fleet files | That check moved to the fleet's selftest; the rest kept |

Four places differ from the text above, each for a reason found in the code:

- The fleet checks run as a step of the `lint` job, not a new job. A new job gates nothing until
  the ruleset requires it, and `lint`'s verdict is already required.
- `script/claim` sets `nife.fleet`, not `script/bootstrap`. Bootstrap runs on every clone,
  a stranger's included, and the hooks are installed by `script/setup`.
- The effort and watcher checks went to `helpers/trunk-health.sh`, not `fleet-metrics.yml`. The
  watcher rule promises a stop is reported within three five-minute intervals, and a daily
  workflow would break that.
- `script/lint-fleet` has 27 checks. The 23rd tests the hook's fleet half, which is new. The last
  four test the trunk watcher, `helpers/trunk-health.sh`.

Exit checks, as run on 2026-10-19 (UTC) against `42836328b` plus this block's commits:

- **Stranger clone.** With all 72 class-3 helpers, scripts and workflows deleted (79 files, fixture
  directories counted whole), 72 of the 74 sections of `script/lint --no-cargo` pass, with a `gh`
  that logs and fails on `PATH` and no token: `gh` was called 0 times. The two that fail are record
  checks. `script/roadmap --check` fails on `Recorded.` paths in old blocks that name class-3
  files. The prose environment-name check fails on names that only class-3 notes and briefs
  mention. Both are records citing tooling that has not moved yet, which is §201 (one roadmap until
  a citation has to cross)'s event, not a gate calling the fleet.
- **The checks still fire.** `script/lint-fleet` ran green in CI's `lint` job on `42836328b`
  (merge-group run 39267144306, step "The fleet tooling's checks"). The 39-break sweep of
  2026-10-12 covers the first 23 checks. For the four trunk-watcher checks, replacing
  `helpers/trunk-health.sh` or `helpers/repeat_issue.py` with `exit 3` turns `script/lint-fleet`
  red.
- **No token.** A fresh clone with `nife.fleet` unset, the hooks installed, no `GH_TOKEN` and the
  same failing `gh`: a real `git push` of a non-empty commit to a local bare remote ran
  `script/fmt --check` and `script/lint --no-cargo`, passed, and called `gh` 0 times.
- **Same CSV rows.** Both collectors, run locally on 2026-10-12, wrote rows byte-identical to
  `metrics.yml`'s last run before the split (2026-10-11 12:25 UTC) for every finished day.
  `fleet-metrics.yml` is now the only writer of both files and has run green daily through
  2026-10-19 (run 39241978532).
- **App absent, dispatched on a fork.** On nife the secrets sit in the `automation` environment,
  whose deployment branches are `main` only, so a dispatch on a branch is refused and one on `main`
  mints the App's token. The fork `calef/nife` has no secrets and no environments. Its `main` was
  fast-forwarded to nife's `01be10059`, which registered its workflows, and all four were
  dispatched there on 2026-10-20 at 03:32 UTC. Referencing `automation` created an empty
  environment with no protection rules. On every job that has the token step, the step was skipped.

| Workflow | Run on `calef/nife` | Exit, measured |
|---|---|---|
| `vendor-watch` | 39364813031 | Green. Upstream had moved, but the report was unchanged, so the job exited before `gh pr create`. The pull request path did not run. |
| `verify-shard-refresh` | 39364815886 | Red at its last step. The branch `metrics/verify-shards` pushed on `github.token`, then `gh pr create` was refused: "GitHub Actions is not permitted to create or approve pull requests". |
| `mutation` | 39364805264 | Red at its last step. All eight shards, the baseline report and the inflow check passed. The record job pushed `mutation/census` on `github.token`, then `gh pr create` was refused the same way. |
| `falsifications` | 39364802502 | Red. The seeded syscall driver and the arm64 replay passed. The confined fuzzer failed on its own finding (BUGS). The replay job was cancelled at its two-hour limit (05:32 UTC), so the record job was skipped and never reached `gh pr create`. |

calef, 2026-10-20 14:30 (UTC), on the fallback: "a". With the App absent, a workflow that has
something to record measures, writes and pushes, ends green, and puts the compare link in its run
summary. No repository is asked to let Actions open pull requests, and the exit is read as "the run
completes through the push". All four workflows now call `helpers/pull-request-by-hand.sh`
(provisional) after their push when no App token was minted. The fork's `main` was fast-forwarded
to the branch carrying that change and two were dispatched again:

| Workflow | Run on `calef/nife` | Exit, measured |
|---|---|---|
| `verify-shard-refresh` | 39441127380 | Green. It pushed `metrics/verify-shards`, opened no pull request, and wrote the compare link to its summary. |
| `mutation` | 39442048323 | Dispatched 2026-10-20; its shards take about two hours. |

## BUGS

- A gate split in two can lose a check in the gap. A new selftest of a class-3 file that is added
  to `script/lint` makes a stranger's lint read the fleet, and only rerunning the stranger clone
  catches it.
- Without the App, nobody is told a record is waiting. The run is green and the compare link is
  in its summary, so a person has to look at the run to find it.
- The confined fuzzer's long sweep fails on seed `0x2f2e76cb02` at step 34: fuzzer 0's slot 2 is
  filled with a page frame after a syscall on a held page frame, which the fuzzer's oracle says
  "grants nothing" (replay `NIFE_CONFINED_FUZZ_SEEDS=202643000066:1`). It fails identically on
  nife's scheduled run of 2026-10-19 (39243857792). The oracle is wrong: the `justify_fill` of
  milestone 779 (fuzz the surface a confined process can reach) has no `PageFrame::SLICE` arm.
  Fixed by milestone/779-slice-oracle.
- `falsifications` has not reached its record job on `main` since at least 2026-10-12: the replay
  job hits its two-hour limit and is cancelled (run 39243857792). A dispatch with the App absent
  stops at the same place.
- The opt-in is per clone. A fleet machine that never ran `script/claim` pushes without the queued
  check until `git config nife.fleet true` is set.
- Four current-tense statements say a fleet check runs in `script/lint`, in files a lane may not
  edit. Three are the "`script/lint` 4b" mentions in `design/naming.md`,
  `design/tenets/roles-and-the-queue.md` and `notes/skills/developer-lane/SKILL.md`. The fourth is
  milestone 495 (a cadence that says "due" reaches nobody), whose block says
  `helpers/trunk-health-cadence-selftest.sh` "runs in `script/lint`". All four now mean
  `script/lint-fleet`.
- The new names are provisional: `script/lint-fleet`, `nife.fleet`, `fleet-metrics.yml`,
  `helpers/pre-push-fleet.sh` and its two selftests.

## Follow-on

- **Outstanding.** Record `mutation`'s exit from run 39442048323.
- **Outstanding.** `falsifications`' replay job fits inside its two-hour limit, so the workflow
  reaches its push with the App absent (BUGS).
- **Outstanding.** The confined fuzzer's false failure on seed `0x2f2e76cb02` (BUGS), fixed by
  milestone/779-slice-oracle. Whichever of that branch and this block's next change merges second
  marks it done here.
- **Milestone 902.** Milestone 902 (nife's tooling, sorted by who needs it, before any of it moves).
  Moving class 3 as a unit, with the `Recorded.` paths and the class-3 notes that make the
  stranger clone's two record checks fail, is the inventory's next step:
  `design/roadmap/0902-tooling-sorted-by-who-needs-it.md`.
- **Recorded.** The four stale "runs in `script/lint`" statements, in BUGS above.

## Index row

The tree's own gates stop calling the agent-fleet tooling. script/lint's 22 fleet checks move to
script/lint-fleet, which CI's lint job still runs. The pre-push hook asks GitHub nothing unless a
clone opts in, four scheduled workflows run without the automation App, and the method-metric
collectors leave metrics.yml. It is the first step of calef's Fork 4 ruling on #1976, and what
makes moving the fleet tooling to its own repository a move rather than a rewrite.
