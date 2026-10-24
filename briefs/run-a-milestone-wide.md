*Name: provisional, minted 2026-10-23 (UTC) by maintainer/brief-run-a-milestone-wide. "Wide" means
many lanes on one milestone at once; "run a fleet" was considered and names the agents rather than
the milestone.*

**Provisional name.** Run one large milestone across many concurrent lanes. You hold the maintainer
role, the milestone's work divides into many independent pull requests (a paydown, a sweep, a
rename across the tree), and you mean to run them at once rather than one after another. Do the
work; do not ask questions.

Why a brief. The judgment below was paid for by one measured run, and a maintainer who starts a
wide run from memory meets each of its failures again. Every part of it is tool-neutral except
where it names one harness's mechanism, and then it says so: anybody may run their agents on
this tree.

## What a wide run is bounded by, measured

One run measured all of this: milestone 929 (comments in the Linux style across first-party Rust), on
2026-10-23 (UTC), from one maintainer session on a Linux host. It is one milestone of comment-only
diffs, whose CI is short (milestone 929's Cost section), so treat each figure as a sample of
one.

- Agents. The session ran up to 20 concurrent agents, lanes and reviewers together, and reached
  its harness's cap of 20 concurrent agents.
- The host. It was not the limit. The host's CPUs sat about 85% idle, with memory mostly free. A lane spends most of its time waiting on the model and on CI, and CI
  runs off the host.
- The binding constraint. It was the maintainer's own loop per pull request: cut the lane, read a
  long report, file its findings, launch a reviewer, read the verdict, relay the fixes, watch CI,
  merge. That is about eight steps and two long reads per pull request, all through one context.
- Review. Every independent review came back MERGE AFTER FIXES, 15 of 15 when this was
  written. The fixes repeated by class (the checklist in `briefs/review-a-paydown.md`). The
  reviewers were fresh agents that read only the diff, never the lane's report, and that
  independence is what caught them.

So the order of the remedies below is by payoff against that constraint. Raising the agent cap
comes last, because the cap was never what held merged work back.

## The partition: disjoint scope files, sized by judged lines

Two lanes in one source file is a conflict resolved by hand. Prevent it before the first claim.

1. List every file the milestone touches, with its judged lines. For milestone 929 that is the
   lines over the cap plus the in-body lines, the measure its Cost table uses. Another milestone
   uses its own unit of review, the thing a reviewer must read and decide on.
2. Cut the list into scope files of about 1,000 judged lines each. A scope file names whole source
   files only, and no source file appears in two.
3. Keep the leftovers (files too small to matter alone, files whose owner is unclear) for one
   residual lane, briefed last, after the others have landed.
4. Hand each lane its scope file in its brief. A lane edits nothing outside it, and a file it finds
   that wants work goes to the residual lane's list through the maintainer, not into its own diff.

The shared baseline is the collision surface a partition does not remove. In milestone 929 it was
`design/comment-block-baseline.tsv`, which every lane's paydown changes. Its helper's `--bank`
rewrites the whole file, so two lanes that bank conflict on every row. The rule for a run:

- A lane never runs a whole-file rewrite of a shared baseline. It edits its own rows by hand, the
  rows for the files in its scope, and nothing else.
- Better, avoid the file. Pay a directory down under its current cap, land every lane, and only then
  step the cap and bank once, in one pull request of its own.

## Claiming: one after another

Lanes are claimed by the maintainer in sequence, one `script/claim` finishing before the next starts.
Every worktree shares one `.git`, so many agents running `script/claim` at once contend for the
same refs and index locks. A claim takes seconds. Sequence them and launch the agents in parallel.

## Each lane runs the follow-through contract

`briefs/gate-a-lane.md`'s PENDING file applies per lane, unchanged. Write one beside each lane's log
at dispatch, holding that lane's push, `gh pr ready` and `gh pr merge --auto`. Tail each log sixty
seconds after dispatch, because twenty dispatches make a dead one easy to miss. A wide run is
where an unexecuted ending is likeliest, because twenty of them live in one context at once.

## The remedies, in order of payoff

### 1. Take the maintainer out of the per-pull-request loop (proposal)

Script the pipeline for one lane, end to end:

    claim → work → self-check → independent review → fix round → CI → queue

The script runs every step and surfaces only exceptions to the maintainer: a HOLD verdict, a red
check whose cause it cannot name, or a finding that needs an architect (a design fork, a wire
format, a name). Everything else passes without being read. The maintainer reads the exceptions and
the merged list, and the eight steps drop to about one.

It is a proposal because it needs mechanism the tree does not hold yet: the pipeline script, and the
structured status (remedy 4) it routes on. One implementation is Claude Code's Workflow tool, which
runs such a script. It needs the person running the session to opt in explicitly, so a maintainer
does not start one on its own judgment. Another harness needs its own runner; the pipeline's steps
are the same.

### 2. Several maintainer sessions, each owning a disjoint scope (in force)

The maintainer skill allows plural sessions, and the merge queue arbitrates between them. Give each
session a disjoint part of the partition, a directory or a run of scope files. Each session brings
its own agent cap and its own context, so the loop that binds one session is divided rather than
queued. The rules the skill gives still hold: list remote branches before briefing, and treat
anything minted as provisional until the queue lands it.

### 3. First-pass quality (in force)

A lane runs `briefs/review-a-paydown.md` against its own diff before it pushes, and fixes what it
finds. Its brief names the recurring error classes from that checklist. This does not replace the
independent review, since a lane reviewing itself shares its own blind spots. It aims to make the
independent review's verdict MERGE more often than MERGE AFTER FIXES, which saves a whole fix round
per pull request.

### 4. Lanes write their own records

The records half is in force (the developer-lane skill and `notes/roadmap.md`'s "Anybody may add to
the roadmap"). A lane appends each finding to its tracked home itself: a proposal under
`design/roadmap/proposals/`, or a `BUGS` entry beside the feature. The maintainer then checks that a
home exists instead of writing one.

The status half is a proposal. A lane ends with a short structured status, not prose:

    STATUS: READY | HOLD | WAITING
    PR: #<N>
    REVIEW: MERGE | MERGE AFTER FIXES | HOLD
    RECORDS: <path of each proposal or BUGS entry it wrote, or none>
    HOLD-REASON: <one line, only on HOLD>
    WAITING <run_id>... on <what>

A pipeline routes on the first line, and the maintainer reads the rest only on HOLD.

### 5. A steward merges what is green, on an interval (in force)

The steward role is in the maintainer skill: it merges what has earned it, green on every check and
inside its lent authority. Run one beside a wide run so a green pull request does not wait for the
maintainer's attention. One implementation is a session on a timer (in Claude Code, `/loop`).

### 6. Only then raise the concurrent-agent cap

The cap is a harness setting. Raise it once remedies 1 to 5 have taken the maintainer's loop off
the critical path, and not before: more agents through the same loop only lengthen the queue of
reports waiting to be read.

## Two tree-wide failures, routed at once

A failure every lane meets is fixed once, in the tree, by one lane, and the rest wait for it.

- A host gap. In this run a missing `ruby` broke two lint checks, and a selftest failed when
  `TMPDIR` sat under the home directory. Together they blocked every lane's push at the pre-push
  hook until #2602 fixed both in the tree. Fix such a gap in the tree, not per lane. Until it lands,
  a lane may set the environment around it (`TMPDIR=/tmp`) and says so in its report.
- A tree-wide red. A new security advisory turned the supply-chain check red on every pull request
  mid-run (#2614). The tell is one check red on many pull requests whose diffs cannot have caused it.
  Brief one lane on the cause, and hold the rest: no lane fixes it in its own diff.

When `main` itself fails the check, the merge queue fails with it. Every merge group fails, and the
fix queues behind the pull requests it would unblock, so it never lands. In this run the remedy that
worked had three steps:

1. Dequeue every other pull request and turn off its auto-merge (`gh pr merge <N> --disable-auto`).
2. Let the fix land alone.
3. Have each branch merge `origin/main` and requeue.

How a run notices the second kind early is not built. `helpers/trunk-health.sh` watches `main`,
not the open pull requests, and a pipeline that sees the same red check on three lanes at once
should raise it as one exception rather than three.

## Arm as soon as the last fix is pushed

A wide run arms a pull request (`gh pr merge <N> --auto --merge`) as soon as its review passes and
its fixes are pushed, while CI is still running. That is `briefs/gate-in-ci.md`'s Step 4, which
every lane follows (calef, 2026-10-24 UTC). Auto-merge waits for the required checks itself, and the
merge queue runs them again on the merge group. Waiting on green only adds a polling round to every
pull request. Step 4 also holds the exceptions (a draft, `needs-architect`, a red check the
diff caused) and the toggle for an armed pull request stuck `BLOCKED`. One rule binds after
arming: push nothing to a queued branch.

## BUGS

- Every number above is from one run, of one milestone, whose diffs were comment-only and whose CI
  therefore skipped the heavy jobs. A milestone that changes code spends far longer in CI per round
  and may find the host or runner concurrency binding before the maintainer's loop.
- Fifteen reviews of one milestone is a small sample. The claim that a self-check raises the first-pass MERGE rate is
  unmeasured; the number that settles it is the share of independent reviews returning MERGE with
  and without the self-check.
- Nothing here measures how many concurrent sessions the merge queue absorbs before its group
  builds, not the maintainers, become the limit.
