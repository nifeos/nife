# Can GLM-5.3 Flash do lane work?

*Name: provisional. calef asked on 2026-10-10 (UTC): "Can you launch a lane to
assess GLM-5.3 Flash for lane work?". The method below was committed before the first trial run;
anything that changed after it is recorded as a deviation.*

## Method

Two halves. A retrospective over the window when lanes ran on GLM under opencode, which costs
nothing, and a controlled trial of three tasks, each run once on `glm-5.3-flash` and once on
`glm-5.3`, six runs in all.

### The trial

Each task replays lane work that has already merged, from its base commit, so the merged commit is
the known-good answer and the Claude baseline. Claude is not rerun.

| task | kind | base | merged answer |
|---|---|---|---|
| (a) | debt paydown, comments only: stale `#[path]` claims | `ea7a58d60` | #1863 (`ce3ef6c8b`) |
| (b) | host crates: tests that kill nine census survivor keys (milestone 637 (triage the crates the 2026-09-21 mutation census measured for the first time), batch 7) | `ad4ca79eb` | #1838's test commit (`1f37b3cb4`) |
| (c) | kernel: milestone 95 (an unmap primitive, and the mappings init never lets go): `AddressSpace::UNMAP` on three ISAs, system tests, aarch64 QEMU | `65ee81831` | #1678 (`c9afcb837`, `4a327062c`) |

How each run is made:

- Harness: opencode 1.18.35, headless: `opencode run --auto -m <model> --dir <clone> --format json "<brief>"`.
  The JSON event stream records every tool call, so rule-following is graded from what was run.
- Provider: OpenRouter, through a LiteLLM proxy the maintainers run, on routes pinned to
  `z-ai/glm-5.3-flash` and `z-ai/glm-5.3`. Cost per run is the proxy's own per-request cost log,
  summed over the run's model group; opencode reports no cost for a custom provider.
- Isolation: Each run gets its own clone made by `git init` and a fetch of the base commit alone,
  so the answer is absent from its history. No remote, `GIT_SSH_COMMAND=false`, an unauthenticated
  `gh`, and a throwaway `HOME` so opencode loads no user-level configuration or instructions. The
  only instruction file it loads is the clone's `AGENTS.md`. Web fetch is denied.
- Brief: A common preamble (no remote, skip the claim, read `AGENTS.md` and the skills it
  names, run `script/lint` and the tests, report at the end), then the task text a lane brief would
  carry. Tasks (a) and (b) are host-only. Task (c) may boot aarch64 only.
- Bounds: 60 minutes of wall time per run, enforced by the runner. The two runs of a task go at
  once, so both see the same machine load.
- Grading: Blind first: the run's diff against the merged answer before its narration is read.
  Then `script/lint` (Homebrew Python first on `PATH`) and the task's tests, pass or fail per gate,
  rerun by the grader. Then correctness against the merged answer, rule-following (`AGENTS.md`, the
  skills, no em-dashes, cited sections glossed, names marked provisional, no invented decisions),
  and the number of corrections a reviewer would have had to make.

### Deviations from the method

- The two runs of each task were meant to start together. On task (b) the second process died in
  its first second on a locked opencode database, because both shared one throwaway `HOME`. It was
  restarted 18 seconds later; task (c)'s pair was started 10 seconds apart for the same reason.
- The brief said not to run `cargo xtask std-src`, and the harness set `RUSTUP_HOME` to the real
  one. `script/test` runs `std-src` itself, so task (c)'s runs, and the grader's own baseline,
  repointed the machine-wide `nife-dev` link at a trial clone between 23:02 and 23:37 UTC on
  2026-10-10. Other lanes that use the shared name broke for that window; the link was put back on
  the main checkout's farm afterwards. No trial result depended on it, since each suite built and
  used its own farm. A rerun needs its own `RUSTUP_HOME`, or the save and restore that
  `script/stranger-test` does.

## Results

Every run finished inside the bound except task (c), where both hit the 60-minute wall with work
uncommitted. The grader reran every gate in each clone; the model's own claims were not taken.

| run | wall min | finished | `script/lint` | tests | matches the merged answer | corrections a reviewer would make | cost, USD |
|---|---|---|---|---|---|---|---|
| (a) Flash | 12.3 | yes, 1 commit | pass | n/a (comments) | same 7 files, plus 2 stale `swap.rs` mentions the merged one missed | 1: `c_confiner`'s comment names `c_seam` and omits `supervision_protocol` | 0.07 |
| (a) GLM-5.3 | 24.0 | yes, 1 commit | pass | `swap_protocol` pass | wider: 16 files, the supervision and NTP families too, and two notes | 0 | 1.93 |
| (b) Flash | 18.3 | yes, 1 commit | pass | 5 crates pass | yes; one test pins a literal where the merged one pins `argon2`'s own constant | 1: that pin; its report says Kani proves a branch dead, which a test does | 0.14 |
| (b) GLM-5.3 | 14.3 | yes, 1 commit | pass | 5 crates pass | yes | 1: British spelling of "behavior", twice | 1.53 |
| (c) Flash | 60, cut off | no commit, no report | fail: a quote §162 (whether a holder can give up a mapping) cites was edited out of `notes/trusted-init.md` | aarch64: every test passes, the suite's frame budget fails by 29 frames | the method is right; an unmapped `va` answers 0 where calef later ruled "Refuse" (marked provisional) | 4 or more, plus finishing; one em-dash | 0.75 |
| (c) GLM-5.3 | 60, cut off | no commit, no report | fail: a clippy error in `system_tests` | aarch64: stops at `counter_frequency_tests`, a regression | no: see below | not salvageable as it stands | 6.38 |

Task (b)'s kills were checked by the grader with cargo-mutants restricted to each site: all nine
keys caught in both runs, nothing missed. Flash's run had already checked its own kills the same
way, and so had GLM-5.3's.

Task (c) is where the two models parted. Both found that `ThreadControlBlock::CONFIGURE` retires a
space's name, which is the merged lane's third finding: `UNMAP` cannot reach a running space. Flash
built the method for spaces under construction, marked both open semantics and the method number
provisional, and wrote a note saying what it could not close. GLM-5.3 instead answered the fork
itself: `CONFIGURE` now mints a capability to the child's own space into a new reserved slot. That
is a change to the kernel's object model, an architect's call, built unmarked. It has the shape of
option B in the proposal the Claude lane raised for the same question, and calef chose option A
(#1678, ruling 3 of 3, 2026-10-05 14:39 UTC). It also broke an existing system test.

Across the four committed runs: no em-dash, every cited section glossed (lint's citation check
passed), and a co-author trailer on every commit, though both models wrote "GLM 5.3" for Flash's
runs too. Of the two unfinished runs, Flash marked its method number and both open semantics
provisional; GLM-5.3 left its new reserved slot unmarked. That matters to `notes/project-metrics/commit-models.csv`,
which reads trailers.

Cost: $0.97 for Flash's three runs and $9.84 for GLM-5.3's, $10.81 in all with the routing probes.
Flash spent fewer tokens per task as well as fewer dollars per token (1.6M input tokens on task (a)
against 8.2M), and on every run more than 95% of its input was a cache read.

### The Claude baseline

Each merged answer is Claude's (Opus 5.5), written by a lane and merged after the maintainer's
review. On task (a) Claude did less than both GLM runs: #1863 left two stale `swap.rs` mentions in
the same files and the supervision family untouched, and its lint run stopped early on a missing
tool. On task (b) the three answers are equivalent. On task (c) Claude finished inside one session,
committed, and stopped at three architect questions, which calef ruled on (#1678's comments).
Claude's wall time and spend for those lanes were not recorded at the time, so no cost ratio
against Claude is claimed here.

## The retrospective

The z.ai window (2026-09-29 02:33 to 2026-10-01 05:07 UTC, [the merge-rate COE](coes/2026-10-03-the-merge-rate.md))
separates the two models by session, since no session mixed them. Source: opencode's database
grouped by model id, `notes/project-metrics/commit-models.csv`, and `gh` for each pull request.

| | GLM-5.3 | GLM-5.3 Flash |
|---|---|---|
| sessions | 33 | 17 |
| assistant messages | 3,900 | 632 |
| commits attributed | 135 | 17 |
| em-dashes added | 0 | 0 |

The comparison is confounded, and the data cannot remove it. The maintainer session routed by kind
on purpose: rebases, ports and promotions went to Flash, and verification, prose and investigation
to GLM-5.3. Flash ran only on 2026-09-29, from 02:24 to 21:22 UTC. Every Flash result was checked or
finished by the GLM-5.3 maintainer, so merge outcomes are joint. Attribution by commit subject also
counts rebased commits from before the window.

What the window does show:

- Flash's rebases and its one promotion lane merged, and none was reverted. Two rebases it refused,
  correctly, as a wire fork and as a port rather than a rebase; one died on opencode's guard against
  writing outside the worktree.
- Flash wrote new code once, milestone 614 (two installed versions of one program) in #1443. Its commit `bdc7cba75` added a recipe that
  broke every fetching CI leg. GLM-5.3 then called the failure a flake (`1582630b0`), and Claude
  found the cause on 2026-10-03 (`40924102d`, `0a5f885c8`).
- GLM-5.3's adversarial passes found one re-discovery in pass 4 and one new booted escape in pass 6
  (#1901), which is review work and not lane work.

## What six runs can support

This is anecdote, not a benchmark. One run per model per task cannot separate a model from luck,
and the three tasks were picked by one lane. It can support three things. Flash can finish a
small, gated host or comment task to the standard the tree merges. On such tasks it was not worse
than GLM-5.3 here, at about a tenth of the cost. Neither model finished a medium kernel milestone
within an hour. It cannot rank Flash against Claude on quality or cost, say anything
about riscv64 or x86_64, or say whether GLM-5.3's unilateral fork answer is typical.

## Recommendation

Use Flash for lanes whose result a gate can judge and whose scope the brief names: comment and
prose debt with named targets, mutation-survivor tests, rebases with known conflict classes,
promotions. The conditions: run it through `helpers/open-lane.sh` or a harness with the same gate
lock, and give it its own `RUSTUP_HOME`. A reviewer reads the diff before merge, as with any lane.
The trailer is fixed to name Flash, so attribution stays true.

Do not use either GLM model for kernel, syscall-surface or design-fork lanes yet. Flash ran out of
time there; GLM-5.3 decided a fork it should have raised.

To learn more, rerun task (c) on both models with a 120-minute bound, and run five more
debt-paydown and survivor tasks per model, scored the same way, before widening Flash's class.
