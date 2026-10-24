Gate a branch's work in GitHub Actions rather than on this laptop. You are in the branch's git
worktree, your work is written, and you are ready to prove it. Do the work; do not ask questions.

Why this brief exists rather than "run `script/test`". The gates are the heaviest thing this
project does: three architectures of QEMU, and `script/verify`, whose own header records a Kani
harness reaching 3.5 GB. Running them here is what caps concurrent lanes at three or four, and
it has killed a session outright. CI has more memory than this machine and nobody is waiting on a
lane, so the wall-clock a CI round costs is free and the memory it saves is not.

## The flow

    git push -u origin HEAD
    gh pr ready <N>
    gh pr merge <N> --auto --merge     # Step 4: armed while CI runs
    gh run list --branch "$(git branch --show-current)" --limit 5
    # WAITING <ci-run-id> <verify-run-id> on ci.yml + verify.yml for <branch>

## Step 1: run the cheap gates locally, because a 25-minute run must not die on a typo

These take seconds, need no emulator, and catch most of what fails:

    script/lint
    script/roadmap --check
    script/fmt
    script/citations --ratchet

On a fresh clone, including a cloud session, run `script/bootstrap` first. Without it `script/lint`
exits 1 at `cargo-machete not found`. Read each gate's own exit status, never one piped through
`tail`: a cloud lane reported a failed lint as green that way on 2026-09-18.

`script/citations --ratchet` reads the committed state, so commit before running it. Fix what
they report and commit again. Do not push until all four are clean.

## Step 2: push, then mark the pull request ready

Marking the pull request ready is what starts the suite. `ready_for_review` is in the
`pull_request` types of both `ci.yml` and `verify.yml`, so the flip raises a run whose draft gate
asks the API whether the pull request is a draft right now, reads `draft: false`, and every job
executes.

    git push -u origin HEAD
    gh pr ready <N>

The push's own `synchronize` run, raised a moment earlier while the pull request is still a draft,
reports every job skipped. A skipped run carries no evidence, so the flip right behind it loses
nothing; the flip's run is the real one. The same skip is why a push to a draft is never gating on
its own (see "Gating while the pull request stays draft" below).

One timing rule, and it is the only one. The ready flip, a push, and a dispatch share one per-ref
concurrency group with `cancel-in-progress`. Never drop a ready next to a run that is actually
executing: after the last push, let the run settle, then flip. A `ready_for_review` arriving
within a second or two of an event whose real run is starting cancels that run, and a CANCELLED
conclusion sits in the check list permanently, reading as a failure that did not happen. Measured
across the open pull requests on 2026-09-23: 15 cancelled runs against 1 apparent failure. Every
cancelled one traced back to two events a second or two apart. Confirmed on #1123: a force-push
and a `ready_for_review` a second apart, three jobs cancelled at the same timestamp.

## Step 3: name the run ids, end your turn, and do not re-trigger

    gh run list --branch "$(git branch --show-current)" --limit 5

Take the run ids the flip raised and stop there. Do not watch them. A lane never polls CI
(`briefs/session-start.md` has the rule): every poll re-reads this whole session's cached
context, and one lane burned 309 tool calls over six hours doing exactly that. End your turn with
one line:

    WAITING <ci-run-id> <verify-run-id> on ci.yml + verify.yml for <branch>

A run in another repository (basalt, crate-workflows, crate-workflows-test) is written
`<owner>/<repo>:<run_id>`, for example `WAITING nifeos/basalt:<run_id> on build.yml for <branch>`;
a bare id means nifeos/nife. The maintainer adds those ids to the watch list; a watcher wakes the maintainer when both finish,
or when either fails, and resumes you with the result. If a run looks stuck, that is the
maintainer's call on resumption, not a reason to re-run it yourself.

Expect 23 to 29 minutes for `ci.yml` on real code, 2 to 3 on a documentation-only branch where
nearly every job skips on scope. `verify.yml` takes about 47 minutes and is the long pole; it is
a separate workflow precisely so a flaky three-minute job can be retried without waiting for it.

A red result is addressed the same way it arrived. Read it with
`briefs/triage-a-failing-check.md`, fix, commit, push. The pull request is already ready, so the
`synchronize` run re-proves the branch on its own and nothing is flipped again. Each round costs
about half an hour of wall-clock and nothing else, which is the trade this brief is making:
rounds are cheap because no person is waiting, and the laptop's memory is not spent at all.

## Step 4: arm after the last push, without waiting for green

    gh pr merge <N> --auto --merge

The merge strategy is the merge queue's; `--auto` hands the pull request to it and the group
build arbitrates. Arm as soon as the pull request is ready and its last push is made, while CI is
still running (calef, 2026-10-24 UTC). An early arm cannot land anything red. Auto-merge enqueues
the pull request only once its required checks pass, and the merge queue then runs every required
check again on the merge group. Waiting for green first only adds a polling round per pull request.
`briefs/run-a-milestone-wide.md` arms the same way.

Arming still waits in three cases:

- The pull request is a draft. Mark it ready first, as Step 2 or the dispatch path below does.
- It carries `needs-architect`. That hold is an architect's to release, not the queue's.
- A check has already failed through the pull request's own fault. Fix it and push first (Step 3's
  red path), then arm. A flake, or a tree-wide red that the diff cannot have caused, is no reason
  to wait.

Arming means you have stopped editing. If a check goes red after arming, the pull request never
reaches the queue; fix it the same way, then confirm that auto-merge survived the push with `gh pr
view <N> --json autoMergeRequest`, and arm again if it is null. Once the pull request is queued,
push nothing. A push to a queued branch removes it from the queue, throws away its group build and
cancels its auto-merge. The pre-push hook refuses it; see `notes/merge-queue.md`, "A push to a
queued branch".

An armed pull request whose checks are green can still sit outside the queue with GitHub reporting
its merge state `BLOCKED`. The usual cause is a cancelled run left over from its draft days (the
first BUGS entry below). Turn auto-merge off and on, which enqueues it again:

    gh pr merge <N> --disable-auto && gh pr merge <N> --auto --merge

If it stays `BLOCKED`, look for a cancelled run that is the newest of its workflow on the head
commit. That is the duplicate `synchronize` in `notes/merge-queue/limitations.md`, and the merge
drain reruns it.

A ready pull request left unarmed for 30 minutes is labeled `needs-maintainer` for a maintainer
session to pick up. The 30 minutes count only while no `CI` or `verify` run is queued or in
progress on its head. They restart when the last one finishes, so a pull request is never labeled
while its CI runs.

## Gating while the pull request stays draft

Some lanes must prove their work before they may leave draft: a `Blocked-by:` hold on another
pull request, a review that wants the marker up. A push cannot do it, and the draft gate is why.
Both workflows read `draft: true` on any `pull_request` event from a draft and skip the whole
suite. A skipped check still posts a conclusion and still satisfies a required check. That is how
#567 once sat mergeable with all thirteen checks skipped. A `workflow_dispatch` run is not a
`pull_request` event, so the gate returns `run=true` and everything executes:

    gh workflow run ci.yml --ref "$(git branch --show-current)"
    gh workflow run verify.yml --ref "$(git branch --show-current)"

Both commands are mandatory; skipping the second hides the long pole behind a green `ci.yml`.
Take the two run ids from `gh run list`, end the turn on the Step 3 WAITING line, and do not
dispatch twice: a second dispatch cancels the first. When these runs come back green, nothing has
pushed since they started, so the lane's one `gh pr ready` call has nothing to race against and
ready and arm land in the same command:

    gh pr ready <N> && gh pr merge <N> --auto --merge

## Pausing on another pull request, rather than on CI

`WAITING <run-id>` is for CI, and only for CI (in any nifeos repository, with the repository
prefix when it is not nife): it names a run the maintainer's watcher is polling on
your behalf. Your draft might instead be parked on another pull request: stacked on its branch,
sharing a file another lane is mid-edit on, or waiting on a decision only that other pull request
settles. That is not a `WAITING` line, and CI is not what will unblock it.

Say so where a machine can act on it instead: add `Blocked-by: #N[, #M ...]` to this draft's own
body before ending the turn. `helpers/merge-drain.sh` labels the draft `unblocked` and comments once
every listed pull request has merged or closed. `notes/blocked-by-drafts.md` has the mechanism;
`notes/merge-queue.md`'s `Blocked-by:` section has why a plain "held" label was refused, and what
the line does on a ready pull request. A listed pull request closed without merging is
called out as such rather than silently released, because it usually means the plan changed. The
label clears itself the moment you push a new commit or mark the draft ready, so resuming needs
nothing more than doing the work.

Without it, the pause lives only in the pull request's own prose, which nothing re-reads. That is
exactly what happened to #1289 in 2026-09: paused
waiting on #1288, which merged an hour later, and left alone for two days because the pause was
never machine-readable.

## What you must not do

- Do not run `script/verify` or `script/test` locally to "check first". That is the cost this
  brief exists to avoid, and two of them at once is the out-of-memory kill.
- Do not flip ready while a real run is in progress on the same ref, and do not push to a
  non-draft branch while its run is executing. The concurrency group cancels the older run, and
  the cancellation reads as a failure (Step 2's timing rule).
- Do not push to a branch that is in the merge queue. GitHub refuses it with `GH006`, and
  `auto=false` does not mean unqueued.
- Do not enqueue anything. Gating proves the work; merging is the maintainer's.
- Do not poll, with `gh run watch`, `gh pr checks --watch`, or a `sleep` loop around `gh run
  view`/`gh run list`/`gh pr checks`/`gh pr view`. End your turn with the WAITING line (Step 3)
  and let the maintainer's watcher resume you. A PreToolUse hook refuses the common polling shapes
  outright; see `briefs/session-start.md`.

## BUGS

- The claim raises suites before anyone flips anything. `opened` and `labeled` both fire the
  `pull_request` workflows on a fresh draft, and a ready flip while that wave is running cancels
  it. The flip's own wave re-proves the same head sha, so the cancellation is noise in the check
  list, not lost proof. It still reads as a failure to anything summing conclusions, and it can
  leave an armed, green pull request `BLOCKED` (Step 4 has the toggle that clears it). Wait out a
  running wave before flipping ready on a lane's first day.
- A dispatched run and a push-triggered run share a concurrency group, so pushing immediately
  after dispatching cancels your own run. Push first, then dispatch, in that order.
- The timing rule in Step 2 is a wait, not an ordering: there is no event ordering that avoids
  the collision, only a gap wide enough that the earlier event's run has already finished before
  the later one starts.
- Only some workflows are actually reachable by this trap on a normal lane. A file carrying
  `cancel-in-progress` is not enough by itself. The workflow also has to trigger on `pull_request`
  for a lane's own ref. Only `ci.yml`, `verify.yml`, and `architect-hold.yml` do so unconditionally
  (`coe-architect-label.yml` triggers the same way but does not cancel;
  `stick-maker-hosts.yml` cancels but only fires on a change under `crates/stick_maker/`). The rest
  of this tree's `cancel-in-progress` workflows are schedule- or `workflow_dispatch`-only and never
  see a lane's `synchronize` or `ready_for_review` at all. Count workflow files with the string
  before trusting the count; it overstates the exposure.
- Nothing here measures what a CI round costs against a local one. The claim that rounds are
  free rests on lanes being asynchronous, which is true today because one person reviews them. It
  stops being true the moment a lane blocks something a person is waiting for.
- The scope guard can skip jobs on a branch that genuinely needed them if the change only
  touches paths it considers documentation. A suite that finishes in three minutes on a code change
  has skipped, not passed.
