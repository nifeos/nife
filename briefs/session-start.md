Provisional name. What a maintainer session does before it briefs anything. You have just opened
a session, on patagonia or in a cloud container, and you do not yet know what happened while nobody
was here. Do the work; do not ask questions.

Why a brief. The two clauses below (the watchers, and reading the queue for what they already
found) fire at a specific, recurring, nameable moment, the start of a session. That is
milestone 579 (which of the constitution must be carried, and which is a brief)'s test for a brief.

Why it is a checklist rather than a habit. Both of these are duties that belong to whoever
happens to notice, and that is precisely the arrangement this project has already watched fail. On
2026-08-04 three such duties went unperformed in one evening: two green pull requests sat unmerged
for hours, `main` went red with nobody assigned, and merging one pull request staled eight others
that nothing picked back up.

## Which identity this session acts as

Nothing changes unless the session opted in. One that set `NIFE_SMELTER=1` checks, in any harness,
before its first `gh` or `git push`:

    helpers/smelter-token check

Silent means it is on the `nife-smelter` App. A warning means it opted in but never applied the
environment, so `gh` and `git` act as the personal login; prefix each command with
`eval "$(helpers/smelter-token env)";` ([the note](../notes/session-identity.md)).

## 0. Is the week's budget burning faster than the week

Write one line before anything else: the share of the weekly usage allowance used, against the
share of the week elapsed. The window opens when the last limit message said it resets (5pm
Pacific on 2026-10-02). The used figure is in Claude Code's `/usage` and on claude.ai's usage
settings page, which only the person at the keyboard can read, so a session that cannot see it asks
once. Used ahead of elapsed by more than ten points means fewer lanes, cheaper models, or a planned
z.ai bridge (see the ledger), decided now rather than at the limit.

Claude Code warns on its own, within a five-hour window and as the weekly limit nears, but both
warnings come when most of the week is spent. This line exists to be earlier than they are. It is a
rung-4 exception, and a foot gun: the meter is outside the tree, and nothing fails when the line is
skipped. On 2026-09-28 the week's allowance ran out two and a half days in, and the merge rate fell
for five days ([the correction](../notes/coes/2026-10-03-the-merge-rate.md)).

## 1. Are the watchers alive

Two of the three moved into GitHub Actions on 2026-09-24 and run as `nife-smelter[bot]`, so what
you are checking is a run list rather than a laptop:

    gh workflow list --repo nifeos/nife | grep -Ei "merge drain|trunk health"
    gh run list --repo nifeos/nife --workflow "merge drain" --limit 3
    gh run list --repo nifeos/nife --workflow "trunk health" --limit 3

Both must read `active` in the first command, and both must show a run within the last fifteen
minutes in the others. A `disabled_inactivity` or `disabled_manually` workflow is a stopped watcher;
re-enable it with `gh workflow enable`. GitHub also drops scheduled runs under load, so one missing
tick is not a fault and an hour of them is.

A failed `trunk health` run is the finding, not a fault in the watcher. That job exits non-zero
when `main` is red or a cadence is dead, deliberately, because a printed line inside a green run is
a line nobody reads. Open it and act on it.

The third watcher is per developer and stays on your own machine, because it reads your own lane
worktrees and nothing else can:

    launchctl list | grep nife

One entry must be present, `org.nifeos.at-risk`. An install from before 2026-10-12 (UTC) is
labeled `com.nife.at-risk`, which still works. If it is missing, `notes/merge-queue.md` has the
plist and the rename; until it is loaded, `helpers/at-risk-check.sh` is one pass you can run by
hand from the main checkout. Nothing else should be in that list: `com.nife.merge-drain` and
`com.nife.trunk-health` are retired, and a laptop still running either is a second watcher acting
beside the workflow, an old enough one still arming pull requests. The retirement commands are in `notes/merge-queue.md`.

In a cloud session there is no `launchctl` and no lane worktree to watch, so skip this check and
go to step 2. The risk it covers moves to you: an ephemeral container that ends with uncommitted
work loses it, and nothing watches for that, so commit and push before every pause.

`notes/merge-queue.md` has the workflows, the plist, the tested premise they rest on, the cadence
costs calef accepted, and a `BUGS` section honest that nothing reports a watcher's death.

## 2. Read what they already found, the `needs-maintainer` label first

A running watcher is not the same as a watcher that has been read, and this is the half that gets
skipped. A watcher that reported at 03:00 and a session that opens at 09:00 never meet unless
somebody goes looking. So the drain leaves its findings where one command finds them:

    gh api --paginate 'repos/nifeos/nife/issues?labels=needs-maintainer&state=all' \
        --jq '.[] | [.number, (if .pull_request then "pr" else "issue" end), .state, .title] | @tsv'

Every item in that list is yours before any lane is briefed. It holds pull requests and issues
together, because the issues endpoint returns both (a row marked `pr` is a pull request). One
command rather than `gh pr list` plus `gh issue list`, since a second command is one more thing a
session must remember (milestone 808 (every gate accounts for its time), §254 (a gate prints what
each item cost)).
The drain labels a pull request that was The drain labels one that was
ejected from the merge queue, conflicts with `main`, is still queued after it merged, or has been
ready and unarmed for 30 minutes, and comments once with the evidence (milestone 727 (a queue eviction goes to a maintainer session), provisional,
and so is the label's name). It also labels orphan work, a branch holding commits that no open pull
request carries, and opens a draft for one that never had a pull request; notes/queue-ejection.md
lists every cause. Read the comment, then fix it or hand it to its lane: rebase, re-arm a
flake (`gh pr merge N --auto --merge`), dequeue a stale entry with the command the comment gives,
or make an unfinished one a draft again. The label comes off by itself on the drain's next pass
once the cause is gone, so never remove it by hand. `state=all` because a stale queue entry
belongs to a pull request that is no longer open.

An issue in the list is a CI job near its budget (the `budget` cause): its merge-group runs passed
about 85% of the job's budget in `.github/ci-job-budgets`. Nothing has failed. The issue names the
job's slowest steps; decide whether to make it faster, split it, or raise the budget with a reason
line. The issue closes itself when the job's runs drop back under, and the label goes with it.

calef ruled on 2026-10-03 (#1564): *"I don't want the job of watching the queue."* Nothing re-queues
for him any more, so this list going unread is the queue going unwatched. It is rung three of the
ladder, a written record at the moment a session starts, and the BUGS below says what that costs.

Then read the rest of the queue by running the brief that already exists:

    briefs/survey-the-queue.md

That brief is the queue read, in full: it fetches, lists `mergeStateStatus` and `statusCheckRollup`,
distinguishes a `DIRTY`/`CONFLICTING` pull request that needs a rebase from a `BLOCKED` one whose
checks are merely still running, treats a `needs-architect` hold as a deliberate hold rather than a
failure, and names the traps (`auto=false` does not mean unqueued; an absent pull request may have
merged rather than vanished). Do not restate its commands here or type them from memory. This
step is one line on purpose: a second copy of those instructions is a second thing to keep correct,
and the copy that drifts is always the one nobody is looking at.

**The mutation census is read here too (milestone 740 (the survivors a merged pull request adds are checked against a triage record)).** Fatal risk 3's green condition
is that the survivors a merged pull request adds are triaged, and the weekly `mutation testing`
workflow's `inflow` job is what checks it. Read its last scheduled conclusion:

    gh run list --repo nifeos/nife --workflow "mutation testing" --limit 3 --json conclusion,createdAt,databaseId

If the latest scheduled run failed, open its `inflow` job's summary. The untriaged survivors it
lists (crate, function, mutation, pull request) are the next triage lane's input: that lane writes
a row per survivor into `notes/project-metrics/mutation-triage.csv` and a test or a reason for each
(notes/mutation-testing.md, the ledger). A red `mutants` job is a different finding (a broken census,
not untriaged survivors). Nothing here blocks a merge. It is rung 2 only for the job and rung 4 for
whoever reads it, which is why it is in this brief.

## 3. Treat what it reports as tasks, not as a status line

This is the whole reason step 2 exists, and it is the failure the steward role was meant to cover
and did not, for a reason worth keeping: it reported and never acted. A stalled queue announced
in a message is only useful if somebody reads the message and then does something.

Every `DIRTY` or `CONFLICTING` pull request and every `FAILURE` conclusion the survey names is a task
to resolve, ranked the same as keeping lanes full, not a line to skim past. A conflict needs judgment
a watcher does not have: a queue reports, it does not resolve.

Two of them already have briefs of their own:

- a rebase needed: `briefs/rebase-onto-main.md`
- a failing check to diagnose: `briefs/triage-a-failing-check.md`

## 4. Tokens: lane count, model, and the watch list

calef, 2026-09-26, from a session at 60% of the weekly Claude Max limit by Saturday: cache reads are
about 96% of tokens here, so every tool call re-reads the caller's whole context. A lane that polls
CI re-reads 200 to 400k tokens per check; one lane made 309 tool calls over six hours, nearly all of
them waiting. Three rules follow.

A lane never polls CI. `briefs/gate-in-ci.md` has the mechanics; the shape is that a lane ends its
turn after dispatching with one line, `WAITING <run_id> [<run_id>...] on <what>`, and stops there. A
`PreToolUse` hook on Bash (`.claude/settings.json`, script at `helpers/hooks/refuse-ci-polling`)
refuses `gh run watch`, `gh pr checks --watch`, a `sleep` of 30 seconds or more, and a shell loop
combining `sleep` with `gh run view`, `gh run list`, `gh pr checks`, or `gh pr view`. It does not
refuse the two watcher scripts below.

At most about five lanes work at once. The rest are PARKED: branch pushed, turn ended, resumed on a
failure, a rebase, or calef's ask. Mechanical lanes (rebases, recording rulings, queue cleanup)
launch on Sonnet (Agent `model: "sonnet"`), per §202 (mechanical work goes to a cheaper model).

**A lane parked waiting on another pull request, not on CI or on calef, says so where a machine can
act on it.** Add `Blocked-by: #N[, #M ...]` to the draft's own body before ending the turn (a
stacked branch, a shared file, a decision only that other pull request settles). Without it, the
pause lives only in prose, and prose is not a mechanism: draft #1289 was paused on 2026-09-25
waiting on #1288, #1288 merged an hour later, and nobody resumed #1289 for two days because nothing
was watching the pause itself. `helpers/merge-drain.sh` labels a draft `unblocked` and comments once
every listed pull request has merged or closed (`notes/blocked-by-drafts.md` has the mechanism,
alongside `notes/merge-queue.md`'s older `Blocked-by:` section for a queued, non-draft pull
request); a listed pull request closed without merging is called out rather than silently released,
because it usually means the plan changed. The label removes itself on the next commit or on
`ready_for_review`, so it never has to be remembered either.

The watch list is what turns a WAITING line into something that actually wakes you: every run id a
lane reports goes on it, because a lane nobody is watching for stops forever. Two provisional
scripts under `helpers/` do the watching, and their names are not yet ratified. Append one line per
run to `$NIFE_WATCH_DIR/runs.txt` (`<run_id> <lane_agent_id> <label>`), then start, or leave running:

    python3 helpers/runwatch.py &   # per-lane run watcher: exits when a lane's runs all finish
    python3 helpers/nanny.py &      # merge-queue watcher: exits on the first event needing you

`nanny.py` also exits when a pull request newly carries `needs-maintainer`, so a running session
is woken by the label rather than finding it at the next start. Before ending a turn that leaves no
watcher running, read the label list once more; a labelled pull request is left for the next
session only on purpose, and said so.

Start both with `run_in_background` rather than in the foreground; a maintainer session polling its
own watcher script in a loop is the same mistake with extra steps. Each exits on its own event or
timeout and prints why, so relaunch it to keep watching. State lives under `NIFE_WATCH_DIR` (an
environment variable), defaulting to a directory under the system temp dir rather than beside the
script, so two sessions on the same machine do not collide. When a watcher reports, resume the lane
with SendMessage; do not let a lane guess whether its own run finished.

## EXAMPLES

The two that run in Actions, healthy:

    $ gh run list --repo nifeos/nife --workflow "merge drain" --limit 3
    completed	success	merge drain	schedule	main	...	2m
    completed	success	merge drain	schedule	main	...	7m
    completed	success	merge drain	schedule	main	...	12m

Five minutes apart is the cadence. Read the gaps, not only the conclusions: three green runs an hour
old is a stopped watcher wearing a green badge.

The one that stays on your machine, alive (real output shape, patagonia):

    $ launchctl list | grep nife
    -	0	org.nifeos.at-risk

The first column is the process id, `-` meaning the job is loaded but not currently running, which is
correct for an interval job between ticks. The second column is the last exit status: `0` is what
you want, and a non-zero number there is a watcher that ran and failed, which looks identical to a
healthy one if you only check that the line exists.

A session that finds the retirement half-done:

    $ launchctl list | grep nife
    -	0	org.nifeos.at-risk
    -	0	com.nife.merge-drain

That second line is a second drain beside the workflow, acting as `calef` where the workflow acts
as `nife-smelter[bot]`, and one from before 2026-10-03 still arms pull requests. Retire it with the commands in `notes/merge-queue.md`; do not
leave it because it looks harmless.

## Stop, do not improvise

- A watcher whose `launchctl` exit status is non-zero, or an Actions run that failed for a reason
  that is not a red trunk. Report it with the number or the run link. Do not reload the job or
  re-run the workflow repeatedly hoping it takes; something is failing and that status is the only
  evidence you have of what.
- Anything the survey reports that is not a rebase, a failing check, a `needs-maintainer` cause, or
  a `needs-architect` hold.
  Say what you saw and stop.

## BUGS

- No watcher reports its own death, which is the gap calef accepted rather than solved. Moving
  two of them into Actions made the evidence public rather than removing the gap: a disabled workflow
  is visible to anyone who looks, and looking is still step 1 of a checklist a session has to
  remember. A watcher can be down for as long as nobody opens a session.
- A green run list and a loaded job both show the mechanism, not the work. A run that read the
  wrong repository, or a `launchd` job exiting 0 against a checkout nobody uses, looks exactly like
  a healthy one here.
- Scheduled runs are delayed under load and dropped at peak. So "no run in the last five
  minutes" is not evidence of anything on its own, which makes this check softer than the one it
  replaced: you are reading a trend rather than a fact.
- Nothing measures whether a session actually runs this. There is no gate, no log, and no record
  that a session started; the honest position is that this is rung three of the constitution's
  ladder, a written record for whoever opens it.
- The `needs-maintainer` label waits for a session. With none running, an ejected or conflicting
  pull request waits too, which is slower than the drain's old re-arm and is not calef's job. The
  label is rung two (the drain sets it without being remembered); reading it is rung three.
- Steps 2 and 3 are the same instruction split across two files, deliberately, and that split
  costs something: a reader who opens only this brief learns that the queue must be read, and has to
  open `briefs/survey-the-queue.md` to learn how. The alternative was a third copy of the queue-read
  instructions, which is the failure mode this extraction was written to remove.
