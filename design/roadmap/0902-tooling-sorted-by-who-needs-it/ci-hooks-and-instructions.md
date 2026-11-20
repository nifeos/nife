# CI, hooks, instructions and notes

An appendix of [nife's tooling, sorted by who needs it](../0902-tooling-sorted-by-who-needs-it.md),
measured 2026-10-12 (UTC) against `e3cc3f128`.
Everything under `.github/`, `.githooks/`, `.claude/`, `briefs/` and `notes/skills/`, the notes and tenets that exist only to run the agent workflow, and, last, the sections of `AGENTS.md`.

Columns. Class is 1 (build and prove), 2 (standards), 3 (the fleet) or 4 (one machine). Two
numbers mean the row straddles both. "Called or read by" lists the files that name this path,
comments included. The search covers `script/`, `helpers/`, `.github/`, `.githooks/`, `.claude/`,
`.cargo/` and `xtask/`. "Calls or reads" lists the inventoried paths this file names. Both show four and a
count. `s/` is `script/`, `h/` is `helpers/`, and a bare `.yml` is a workflow. The last
column counts the times the file names calef.

| Path | Class | Why | Called or read by | Calls or reads | Lines | calef |
|---|---|---|---|---|---|---|
| `.github/ci-job-budgets` | 1 | CI configuration | `ci.yml`, `h/ci_job_times.py`, `h/job-budget.py`, `h/merge-drain.sh` | `h/job-budget.py` | 12 |  |
| `.github/dependabot.yml` | 1 | CI configuration | `toolchain-bump.yml`, `s/lint` | `ci.yml` | 56 |  |
| `.github/runner-container/Containerfile` | 1 | CI configuration | `s/runner-container` | `s/bootstrap`, `s/ci-qemu`, `s/repeat-under-load`, `s/runner-container` | 60 |  |
| `.github/workflows/ci.yml` | 1 | CI or scheduled proof/test job | `.github/dependabot.yml`, `h/ci_job_times.py`, `h/job-budget.py`, `h/prose_only.py`, +7 | `s/bench`, `s/bootstrap`, `s/ci-build`, `s/ci-log-baseline`, +24 | 1576 | 43 |
| `.github/workflows/codeql.yml` | 1 | CI or scheduled proof/test job | none | `h/prose_only.py`, `h/scope-merge-base.sh` | 233 | 4 |
| `.github/workflows/falsifications.yml` | 1 | CI or scheduled proof/test job | none | `s/bootstrap`, `s/ci-qemu`, `s/falsifications`, `s/mutation`, +2 | 361 | 1 |
| `.github/workflows/kani-reach.yml` | 1 | CI or scheduled proof/test job | none | `s/verify`, `h/kani_reach.py` | 167 |  |
| `.github/workflows/mutation.yml` | 1 | CI or scheduled proof/test job | `h/memory-bounded-runner.sh` | `s/mutation`, `s/mutation-census`, `h/memory-bounded-runner.sh`, `h/mutation_inflow.py`, +2 | 440 | 1 |
| `.github/workflows/stick-maker-hosts.yml` | 1 | CI or scheduled proof/test job | `.cargo/mutants.toml`, `s/coverage` | `h/stick-maker-proof.sh` | 106 |  |
| `.github/workflows/toolchain-bump.yml` | 1 | CI or scheduled proof/test job | none | `s/bench`, `s/bootstrap`, `s/ci-qemu`, `s/lint`, +3 | 587 | 14 |
| `.github/workflows/toolchain-drift.yml` | 1 | CI or scheduled proof/test job | none | `s/drift`, `s/test` | 112 |  |
| `.github/workflows/undefined-behavior-check.yml` | 1 | CI or scheduled proof/test job | none | `s/cadence-check`, `s/test`, `s/undefined-behavior-check`, `h/trunk-health.sh`, +2 | 102 |  |
| `.github/workflows/vendor-watch.yml` | 1 | CI or scheduled proof/test job | none | `s/supply-chain`, `s/vendor-verify`, `s/vendor-watch`, `notes/session-identity.md` | 279 | 1 |
| `.github/workflows/verify-shard-refresh.yml` | 1 | CI or scheduled proof/test job | `h/verify_times.py`, `s/verify` | `s/verify`, `h/verify_times.py`, `notes/session-identity.md` | 121 | 2 |
| `.github/workflows/verify.yml` | 1 | CI or scheduled proof/test job | `h/ci_job_times.py`, `h/verify_times.py`, `s/verify` | `s/falsifications`, `s/lint`, `s/mutation`, `s/verify`, +3 | 576 | 8 |
| `notes/check-inventory.md` | 1 | check inventory | `mutation.yml`, `undefined-behavior-check.yml`, `s/cadence-check`, `s/ci-build`, +1 | `s/audits`, `s/cadence-check`, `s/ci-build`, `s/crate-probes`, +11 | 268 | 3 |
| `notes/ci.md` | 1 | check inventory | none | `s/bootstrap`, `s/ci-build` | 78 |  |
| `notes/skills/qemu-hygiene/SKILL.md` | 1 | QEMU hygiene | none | `h/qemu-bounded.sh`, `notes/skills/README.md` | 40 | 1 |
| `notes/ci-log-baseline.md` | 1/3 | CI log baseline | `s/ci-log-baseline` | `s/ci-log-baseline`, `s/coverage`, `s/cpu-matrix`, `s/icount`, +4 | 216 | 4 |
| `notes/dev-machine-disk.md` | 1/3 | build-volume runbook for any Mac, written after lanes filled one | `s/effort`, `s/preflight-queue`, `xtask/farm.rs` | `s/claim`, `s/effort`, `s/lint`, `s/nanny`, +6 | 277 | 1 |
| `.githooks/pre-push` | 2 | pre-push runs fmt, lint, roadmap; also the queued-push refusal | `h/lint_icount_baselines.py`, `s/lint`, `s/roadmap` | `s/fmt`, `s/lint`, `s/roadmap`, `s/setup`, +6 | 134 | 1 |
| `.github/ISSUE_TEMPLATE/bug-report.yml` | 2 | issue forms | none | `s/console`, `s/server`, `s/test` | 85 |  |
| `.github/ISSUE_TEMPLATE/config.yml` | 2 | issue forms | none | none | 19 |  |
| `.github/ISSUE_TEMPLATE/design-argument.yml` | 2 | issue forms | none | none | 54 |  |
| `.github/ISSUE_TEMPLATE/scope-request.yml` | 2 | issue forms | none | none | 51 |  |
| `.github/workflows/audit-cadence.yml` | 2 | cadence of a tree standard | `s/audits` | `s/audits`, `s/fmt`, `s/lint` | 90 |  |
| `.github/workflows/stranger-cadence.yml` | 2 | cadence of a tree standard | `s/stranger-test` | `s/lint`, `s/stranger-test` | 89 | 1 |
| `notes/follow-on-work.md` | 2 | follow-on work record | `s/roadmap` | `s/ci-build`, `s/citations`, `s/crate-probes`, `s/decisions`, +4 | 240 | 7 |
| `notes/scripts.md` | 2 | front door to script/ | `hook:pre-push`, `ci.yml`, `undefined-behavior-check.yml`, `h/at-risk-check.sh`, +22 | `s/apropos`, `s/architect-queue`, `s/audits`, `s/bench`, +67 | 347 | 3 |
| `notes/skills/naming-authority/SKILL.md` | 2 | who names things | none | `s/lint`, `notes/skills/README.md` | 39 | 2 |
| `.githooks/commit-msg` | 2/3 | commit-msg: agent commits carry Co-Authored-By | `h/commit-msg-selftest.sh`, `s/claim`, `s/lint` | `s/lint`, `s/metrics`, `s/setup`, `h/commit-msg-selftest.sh` | 45 | 2 |
| `.github/workflows/empty-diff.yml` | 2/3 | empty-diff merge gate | `h/empty-diff-check.sh` | `s/lint`, `h/empty-diff-check.sh`, `h/empty-diff-selftest.sh` | 88 | 1 |
| `.github/workflows/metrics.yml` | 2/3 | weekly metrics commit; mixes kernel and method series | `h/ci_job_times.py`, `h/prose_only.py`, `s/cadence-check`, `s/effort`, +1 | `s/coverage`, `s/metrics`, `h/ci_job_times.py`, `h/debt_paydown_share.py`, +8 | 370 | 1 |
| `notes/how-this-is-built.md` | 2/3 | the method as a published claim (principle 2) | none | none | 97 | 2 |
| `notes/project-metrics.md` | 2/3 | the method as a published claim (principle 2) | `metrics.yml`, `h/baseline_drift.py`, `h/interface_stability.py`, `h/merge-drain.sh`, +6 | `s/coverage`, `s/crate-standalone`, `s/effort`, `s/falsifications`, +9 | 364 | 7 |
| `notes/rule-violations.md` | 2/3 | rule-violation ledger | `h/handoff-check.py`, `s/rule-violations` | `s/audits`, `s/lint`, `s/rule-violations`, `claude-settings` | 141 | 5 |
| `notes/skills/decisions/SKILL.md` | 2/3 | how to decide and reach an architect; governance plus agent routing | `h/architect-label-comment.sh`, `s/architect-queue` | `s/architect-queue`, `s/bench`, `s/record-ruling`, `h/needs-maintainer.jq`, +4 | 162 | 15 |
| `.claude/settings.json` | 3 | Claude Code hooks and skill links | `h/handoff-check.py`, `h/hooks/refuse-ci-polling`, `h/prompt-reminder.sh`, `s/bootstrap`, +1 | none | 35 |  |
| `.claude/skills/decisions` | 3 | Claude Code hooks and skill links | none | none | 0 |  |
| `.claude/skills/developer-lane` | 3 | Claude Code hooks and skill links | none | none | 0 |  |
| `.claude/skills/maintainer` | 3 | Claude Code hooks and skill links | none | none | 0 |  |
| `.claude/skills/naming-authority` | 3 | Claude Code hooks and skill links | none | none | 0 |  |
| `.claude/skills/qemu-hygiene` | 3 | Claude Code hooks and skill links | none | none | 0 |  |
| `.claude/skills/worktree-commits` | 3 | Claude Code hooks and skill links | none | none | 0 |  |
| `.github/pull_request_template.md` | 3 | PR template opens with the agent Lane line | `s/ci-build` | `s/ci-build` | 41 | 2 |
| `.github/workflows/architect-hold.yml` | 3 | architect and queue bots | none | `h/hold-rerun.sh` | 166 | 1 |
| `.github/workflows/architect-label.yml` | 3 | architect and queue bots | `h/architect-label-comment.sh`, `h/architect-label-rules.py`, `h/scope-merge-base.sh`, `s/lint` | `s/lint`, `s/record-ruling`, `h/architect-label-comment.sh`, `h/architect-label-rules.py`, +4 | 239 | 3 |
| `.github/workflows/architect-ruled-clears-hold.yml` | 3 | architect and queue bots | none | `s/lint`, `h/architect-ruled.sh`, `h/hold-rerun-selftest.sh`, `h/hold-rerun.sh`, +1 | 118 | 1 |
| `.github/workflows/ci-failing.yml` | 3 | architect and queue bots | `h/ci-failing.sh` | `h/ci-failing.jq`, `h/ci-failing.sh`, `notes/session-identity.md`, `notes/merge-queue.md`, +1 | 66 | 2 |
| `.github/workflows/coe-architect-label.yml` | 3 | architect and queue bots | `h/architect-label-comment.sh` | `h/architect-ruled.sh` | 353 | 1 |
| `.github/workflows/merge-drain.yml` | 3 | architect and queue bots | `h/merge-drain.sh` | `h/lane-claim-check.sh`, `h/merge-drain.sh`, `h/watcher_watch.py`, `notes/session-identity.md`, +1 | 190 | 8 |
| `.github/workflows/ready-status.yml` | 3 | architect and queue bots | `hook:pre-push`, `s/lint`, `s/roadmap` | `s/roadmap` | 99 | 1 |
| `.github/workflows/trunk-health.yml` | 3 | architect and queue bots | `h/trunk-health.sh` | `s/cadence-check`, `h/at-risk-check.sh`, `h/merge_queue_share.py`, `h/trunk-health.sh`, +3 | 136 | 3 |
| `briefs/README.md` | 3 | maintainer brief | none | `s/citations`, `s/fmt`, `s/lint`, `s/roadmap`, +5 | 116 | 2 |
| `briefs/gate-a-lane.md` | 3 | maintainer brief | none | `s/citations`, `s/fmt`, `s/lint`, `s/roadmap`, +5 | 190 |  |
| `briefs/gate-in-ci.md` | 3 | maintainer brief | `h/hooks/refuse-ci-polling`, `h/runwatch.py` | `s/bootstrap`, `s/citations`, `s/fmt`, `s/lint`, +8 | 171 |  |
| `briefs/main-is-red.md` | 3 | maintainer brief | `h/queue-hold.sh`, `h/trunk-health.sh` | `s/cadence-check`, `h/merge-drain.sh`, `h/queue-eligible.jq`, `h/queue-hold.sh`, +2 | 108 |  |
| `briefs/merge-and-cleanup.md` | 3 | maintainer brief | `h/at-risk-check.sh`, `h/merge-drain.sh`, `h/prune_toolchains.py`, `xtask/farm.rs` | `h/at-risk-check.sh`, `h/prune_toolchains.py` | 169 | 1 |
| `briefs/rebase-onto-main.md` | 3 | maintainer brief | `h/merge-drain.sh`, `s/decisions`, `s/nanny`, `s/roadmap` | `s/citations`, `s/lint`, `s/metrics`, `s/roadmap`, +6 | 200 |  |
| `briefs/session-start.md` | 3 | maintainer brief | `h/hooks/refuse-ci-polling`, `h/merge-drain.sh`, `h/nanny.py`, `h/needs-maintainer.jq`, +1 | `h/at-risk-check.sh`, `h/hooks/refuse-ci-polling`, `h/merge-drain.sh`, `h/nanny.py`, +11 | 266 | 8 |
| `briefs/survey-the-queue.md` | 3 | maintainer brief | none | none | 49 |  |
| `briefs/triage-a-failing-check.md` | 3 | maintainer brief | none | none | 40 |  |
| `design/tenets/git-in-a-worktree.md` | 3 | tenet behind a lane rule | none | none | 56 | 1 |
| `design/tenets/lane-count.md` | 3 | tenet behind a lane rule | none | none | 76 | 2 |
| `design/tenets/roles-and-the-queue.md` | 3 | tenet behind a lane rule | none | none | 100 | 7 |
| `design/tenets/routing-work-and-decisions.md` | 3 | tenet behind a lane rule | none | none | 89 | 8 |
| `design/tenets/shared-state.md` | 3 | tenet behind a lane rule | none | none | 38 | 1 |
| `notes/automation-identity.md` | 3 | agent workflow note | none | `h/merge-drain.sh`, `h/needs-maintainer.jq` | 296 | 17 |
| `notes/blocked-by-drafts.md` | 3 | agent workflow note | none | `h/blocked-by-resolution.jq`, `h/blocked-by.sh`, `briefs/gate-in-ci.md`, `briefs/session-start.md`, +1 | 86 | 1 |
| `notes/ci-failing.md` | 3 | agent workflow note | `ci-failing.yml` | `s/lint`, `h/ci-failing-selftest.sh`, `h/ci-failing.jq`, `h/ci-failing.sh`, +2 | 78 | 3 |
| `notes/debt-paydown.md` | 3 | agent workflow note | `metrics.yml`, `h/claim-selftest.sh`, `h/debt_paydown_share.py`, `s/claim`, +1 | `s/claim` | 113 | 4 |
| `notes/delegated-review.md` | 3 | agent workflow note | `h/review-matrix.sh` | `s/citations`, `s/lint`, `s/roadmap`, `h/open-lane.sh`, +3 | 182 | 2 |
| `notes/delegated-review/` | 3 | records of the agent workflow (4 files) | none | none | 1835 | 3 |
| `notes/effort-levels.md` | 3 | agent workflow note | `h/open-lane.sh` | `h/open-lane.sh`, `notes/open-model-lanes.md` | 135 |  |
| `notes/glm-5.3-flash-for-lanes.md` | 3 | agent workflow note | none | `s/lint`, `s/stranger-test`, `s/test`, `h/open-lane.sh` | 157 | 4 |
| `notes/local-inference.md` | 3 | agent workflow note | none | `s/citations`, `s/lint`, `s/verify`, `notes/open-model-lanes.md` | 206 | 3 |
| `notes/main-is-red.md` | 3 | agent workflow note | `h/merge-drain.sh`, `h/queue-hold.sh` | `h/at-risk-check.sh`, `h/lane-claim-check.sh`, `h/merge-drain.sh`, `h/queue-eligible.jq`, +5 | 166 | 2 |
| `notes/merge-queue.md` | 3 | agent workflow note | `hook:pre-push`, `architect-label.yml`, `architect-ruled-clears-hold.yml`, `ci-failing.yml`, +15 | `s/ci-build`, `s/metrics`, `s/preflight-queue`, `s/verify`, +15 | 274 | 9 |
| `notes/merge-queue/` | 3 | records of the agent workflow (10 files) | none | none | 965 | 14 |
| `notes/model-attribution-review.md` | 3 | agent workflow note | none | `s/audits`, `s/names`, `s/stranger-test` | 265 | 6 |
| `notes/model-attribution-review/` | 3 | records of the agent workflow (5 files) | none | none | 352 | 10 |
| `notes/model-comparison.md` | 3 | agent workflow note | none | none | 30 | 2 |
| `notes/model-comparison/` | 3 | records of the agent workflow (8 files) | none | none | 1209 | 24 |
| `notes/open-model-lanes.md` | 3 | agent workflow note | `h/open-lane.sh`, `s/nanny` | `s/citations`, `s/lint`, `s/nanny`, `h/open-lane.sh`, +2 | 218 | 8 |
| `notes/push-while-queued.md` | 3 | agent workflow note | none | `s/lint`, `s/setup`, `h/push-queued-check.sh`, `h/push-queued-selftest.sh`, +1 | 53 |  |
| `notes/queue-ejection.md` | 3 | agent workflow note | `h/stuck-head.jq` | `s/fmt`, `s/lint`, `s/roadmap`, `h/at-risk-check.sh`, +9 | 237 | 9 |
| `notes/session-handoff.md` | 3 | agent workflow note | `s/catch-up`, `s/lint` | `s/bench`, `s/coverage`, `s/fmt`, `s/lint`, +4 | 138 | 4 |
| `notes/session-identity.md` | 3 | agent workflow note | `ci-failing.yml`, `falsifications.yml`, `merge-drain.yml`, `metrics.yml`, +8 | `s/claim`, `h/session_steps.py`, `h/smelter-bin/gh`, `h/smelter-token`, +1 | 176 | 17 |
| `notes/skills/README.md` | 3 | lane and maintainer roles | `h/agents-index-check.py`, `s/lint`, `s/names` | `s/claim`, `s/lint`, `s/names`, `h/agents-index-check.py`, +1 | 26 | 2 |
| `notes/skills/developer-lane/SKILL.md` | 3 | lane and maintainer roles | `h/milestone-branch-block.sh`, `s/claim` | `s/claim`, `s/lint`, `briefs/merge-and-cleanup.md`, `notes/skills/README.md`, +1 | 97 | 3 |
| `notes/skills/maintainer/SKILL.md` | 3 | lane and maintainer roles | none | `s/verify`, `briefs/gate-in-ci.md`, `briefs/merge-and-cleanup.md`, `notes/skills/README.md` | 86 | 1 |
| `notes/skills/worktree-commits/SKILL.md` | 3 | lane and maintainer roles | none | `s/metrics`, `notes/skills/README.md` | 45 | 1 |
| `notes/what-a-session-carries.md` | 3 | agent workflow note | `s/redo-rate` | `s/lint`, `briefs/rebase-onto-main.md`, `briefs/triage-a-failing-check.md`, `notes/open-model-lanes.md` | 89 | 1 |
| `notes/working-from-a-cloud-session.md` | 3 | agent workflow note | `s/bootstrap`, `s/ci-qemu` | `s/bench`, `s/board-console`, `s/board-netboot`, `s/bootstrap`, +13 | 95 | 2 |
| `briefs/bench-session.md` | 3/4 | bench-evening brief; outlets and serial ports, deferring unit facts to lab | none | `s/board-console`, `s/board-image`, `s/board-netboot`, `s/claim`, +1 | 178 | 22 |

## The sections of `AGENTS.md`

| Section | Lines | Class | Why |
|---|---|---|---|
| What this project is | 11 | 2 | What the tree is and who reviews it |
| How to work | 5 | 2/3 | Autonomous execution; commit per proven piece |
| Read these when the work calls for them | 25 | 3 | The index that links all six skills |
| Three principles | 61 | 2, with principle 2 at 2/3 | Ranking and the newcomer test are standards; the method as a result is Fork 3 |
| Nobody remembers | 27 | 2 | The mechanism ladder binds any contributor |
| We are all owners | 11 | 2 | Routing a found problem |
| Measure first | 6 | 2 | |
| Pull requests and comments an agent writes | 8 | 3 | The agent `**Lane:**` line and the smelter identity |
| Measuring, pushing back, correcting the record | 18 | 2 | |
| The rules that hold the codebase together | 31 | 2 | Rule 5 names machines by their public names only |
| The system call surface | 8 | 2 | |
| Testing | 13 | 1 | |
| Every date is UTC | 9 | 2 | |
| Comments | 10 | 2 | |
| Style | 10 | 2 | |
| Environment | 7 | 1 | |
