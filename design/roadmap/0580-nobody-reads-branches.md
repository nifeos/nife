---
status: BUILT
raised: 2026-09-23
built: 2026-09-23
promoted_from: nobody-reads-branches
---
# 580. Nobody reads branches, so give the maintainer a command that does

*(Number minted at promotion.)* Promoted from the proposal
`nobody-reads-branches`, filed 2026-09-23, on calef's instruction of 2026-09-23 to give the two
unnumbered proposals on `main` a number. The text below is the proposal's own, unedited except for
this paragraph, the removal of its `**Gate: NONE.**` line and the two sections appended at the end.
Its long sentences are split to meet §213 (writing standards). The argument is its author's and promotion is not the moment to improve it. On 2026-10-12 (UTC) two
of its sections moved to appendices under §212 (a prose budget), with long sentences split and the
argument unchanged. The page's bold was dropped to meet §213 (writing standards). The gate line
goes because a BUILT block may not carry one. What it said (nothing here touches the system call
surface, adds a dependency, or needs hardware) was true and is now moot. The mechanism landed in
`helpers/lane-claim-check.sh` the same day it was proposed, in commit `448928d93`. As filed: decided
the same day; see "What calef decided" below. It was kept in the proposals directory rather than
promoted at the time, because the disposition is a fix to an existing script, not a numbered
milestone. There was no numbered block for it to be drained into. Raised by the
`maintainer/nobody-reads-branches` lane. It was written while surveying the 25 remote branches
calef's 2026-09-23 sweep found carrying no open pull request, the oldest last touched nine days
earlier. Verified independently against the live repository while drafting this: 24 of the 25 are
real lane branches (`milestone/*`, `maintainer/*`, `fix/*`). The 25th,
`gh-readonly-queue/main/pr-1080-...`, is the merge queue's own synthetic candidate branch, not a
lane's. Any mechanism built from this proposal must exclude that shape the same way
`lane-claim-check.sh` already excludes the ones it found.

## What calef decided

calef read this the same day it was written and refused the closing recommendation. A fourth
watcher script, even a narrow one, competes for the same attention the tree already fails to spend
on the three it has. The exhibit is AGENTS.md's own "reported and never acted", which this proposal
itself leans on. His ruling: one script that already exists, already runs, and already has a
reader, instead of a second competing for the same attention. So the mechanism below landed as
three changes to `helpers/lane-claim-check.sh` rather than as `helpers/orphan-branch-check.sh`. It
landed in the same lane, the same day.

That reverses one specific refusal from "What else was considered" (now
[an appendix](0580-nobody-reads-branches/what-else-was-considered.md)): extending
`lane-claim-check.sh` was refused there because "the two problems want thresholds three orders of
magnitude apart... folding the second into the first risks breaking the tuning that is currently
working." The fix is to keep both thresholds rather than pick one. `milestone/*` keeps the existing
15-minute `GRACE_MINUTES` (the push-to-draft gap §90 protects). Every other branch gets its own,
much longer window (`GRACE_HOURS`, default 24, see below). So widening scope does not touch the
tuning that was already correct. The rest of that refusal's reasoning is that a fourth-watcher
script and a wider `lane-claim-check.sh` are different shapes. calef overruled that directly; this
lane did not re-argue it.

The original recommendation, its measurements, and every refusal below are kept as written. They
are the reasoning that produced the ruling above. This tree keeps refusals rather than erasing the
record of a path not taken.

## The failure is at least four shapes, and one bystander shape worth naming alongside them

Lumping these into "orphaned branches" is how the existing tooling already missed most of them.
Each shape has a different cost if handled wrong. That is the reason to keep them apart rather
than write one blanket rule.

1. A branch with real unmerged work and no pull request, ever. The dangerous case: deleting it
destroys work nobody landed anywhere else. Measured against today's tree: `maintainer/294-and-510`
(6 commits, 47 files, +918/-1517), `milestone/434-retire-the-proposed-disposition` (8 commits, 10
files, +503/-438), `milestone/326-machine-discovery-truncation` (5 commits, 26 files, +881/-21),
`milestone/435-slice-a` (3/29), `milestone/435-slice-b` (3/17), `milestone/435-slice-c` (5/31), and
`maintainer/metrics-2026-09-19` (1/7). Every file and commit count above was re-derived from
`git diff --stat origin/main...origin/<branch>` while writing this. None was copied from the sweep
uncrosschecked, and they match. Ages at measurement time ran two to four days.

2. A branch that never grew past its §90 claim commit. §90 (the claim is a draft pull request;
the status flip is a gate) is the mechanism this proposal leans on throughout. A lane died between
`git push` and `gh pr create --draft`, or never even reached the push in the shape §90 expects.
Zero cost to delete: nothing on the branch differs from `main`. `milestone/433-slice-1` through
`-4` and
`maintainer/ratify-boot-slot-format` carry no commits ahead of `main` at all. `fix/qemu-bounded-
stdin-under-dash`, not named in the original sweep, is the same shape one prefix over. That matters
because it shows the pattern is not confined to the two prefixes `lane-claim-check.sh` and the
sweep both happened to be looking at.

3. A branch whose pull request already merged and was never deleted. Also zero-cost to delete;
this is `lane-claim-check.sh`'s own `LEFTOVER` case, working exactly as designed, for the one
prefix it watches. The gap is prefix, not shape. Five such branches were found live:
`maintainer/mutation-sweep-2026-09-14`, `maintainer/open-model-lanes`,
`maintainer/the-counter-thesis`, `maintainer/what-a-component-costs` and
`milestone/442-crypto-provider`. Four are `maintainer/*`, which `lane-claim-check.sh`'s own `BUGS`
section says it does not watch, on purpose. Widening its pattern would also catch the short-lived
branches its narrow scope is protecting.

4. A branch whose pull request was closed without merging. Two found:
`maintainer/baselines-and-282` (PR #1078) and `maintainer/icount-baselines-stale` (PR #883). Both
closed PRs carry a body and at least one comment. So the reasoning likely survives on GitHub even
after the branch goes, but "likely" is doing real work in that sentence. `lane-claim-check.sh`
today does not distinguish this from case 3: its `LEFTOVER` line fires on any non-`OPEN` state and
recommends deleting either way. That conflation is a real bug in a script this proposal otherwise
leans on, worth fixing on its own account. Flagging it here gives it a home rather than letting it
die a second time in a lane report nobody re-reads. That is the exact failure this whole document
is about.

The bystander shape: a stale draft holding real work, whose one existing report already fired and
was ignored. PR #1087 is not branch-without-PR (it has one, open, draft), so it is out of this
proposal's scope by definition. It earns a mention because it is the cleanest evidence available for
the design argument below. `merge-drain.sh`'s `stale_drafts` check fired on it correctly, at
2026-09-22T02:22:54Z, posting the comment it is built to post ("has not committed in over 75
minutes... If its lane is finished: `gh pr ready`"). Twenty-four hours later the draft, 8 commits and
+1398/-83, was still sitting there, untouched. It was found again only because calef looked by
hand. The mechanism worked. The report changed nothing. That is not a case this proposal fixes. It
is the proof that adding a fifth one-shot reporter to the four failure shapes above would not fix
them either.

## Case 3's assumption was wrong, and this branch proved it

Case 3 above, and `lane-claim-check.sh`'s `LEFTOVER` check that this proposal leans on, both treated
"pull request merged" as "safe to delete," the same zero-cost case as case 2's dead claim. That
looked right because it usually is. Most of the time a branch tip is exactly the commit GitHub
merged, and nothing else lands on it afterward. So checking pull request state is a cheap stand-in
for checking the tree itself.

It is wrong whenever more commits get pushed to the branch after its pull request merges.
`maintainer/open-model-lanes` is the branch that disproved it. PR #1089, cut from this branch,
merged. Three more commits (on the gateway's no-key posture, the token, and the bind address) were
then pushed to the same branch and never landed anywhere else. The old rule, "pull request merged,
therefore delete the branch," would have told a reader those three commits were safe to discard.
They were not. This document is being written from the branch that carries them, being rebased
onto `main` in the same pull request that adds this subsection.

The fix does not trust pull request state at all. `git merge-base --is-ancestor <branch>
origin/main` asks whether every commit on the branch is already reachable from `main`. That is the
question "safe to delete" actually depends on. A merged-and-untouched branch and a merged-and-then-
extended branch give pull request state the same answer and this test different ones.
`helpers/lane-claim-check.sh` (PR #1116) now tests the tree this way instead of trusting the merge
flag.

## What else was considered

The proposal weighed six alternatives and refused each. They are extending
`helpers/merge-drain.sh`, and extending `helpers/lane-claim-check.sh` (the closest contender, and
the refusal calef overruled on 2026-09-23). The rest are a new `helpers/` watcher of the same
shape, a scheduled GitHub Actions workflow, a standing duty for the steward in prose, and doing
nothing.

- [what-else-was-considered.md](0580-nobody-reads-branches/what-else-was-considered.md): each
  alternative with its refusal as filed, and the note on the one overruled.

## Which rung, and what this does about "reported and never acted"

Honestly: this stays on rung four. A gate cannot fail a build over a branch that is not part of any
pull request. That is the exact reason `lane-claim-check.sh` refused to be a gate for the narrower
version of this same question, and that reasoning holds here without modification. There is also no
single place in the tree to put rung three's "record beside the thing," because the thing, an
abandoned branch, has no reader who meets it by default. That is the whole problem. So this
proposal does not climb the ladder. What it changes is which rung-four channel the report lands in, and that
distinction is not cosmetic. PR #1087's comment is also rung four, correctly built, and it changed
nothing. A PR comment and a `launchd` log are both channels nobody has a standing habit of
rereading once the moment passes.

Two design choices aim this report at a channel that is already read, rather than inventing one
that will not be:

- Land the output next to the check AGENTS.md already makes a session read every time.
  AGENTS.md's own merge-queue section already tells a maintainer session to read
  `gh pr list --json number,mergeStateStatus,statusCheckRollup` for `DIRTY`, `CONFLICTING`, or a
  `FAILURE` conclusion, "a standing check, same priority as keeping lanes full." Every fact this
  proposal is built on came from running commands in that same family by hand. Putting the orphan
  survey there, as one more thing that command line checks, costs a maintainer nothing new to
  remember. It extends a habit already proven to work rather than asking for a second one.
- Let branch age do the escalating, because it is already the only state that exists. A branch
  that shows up in this survey unresolved does not go quiet between one session and the next; it
  shows up again, older. No counter, no database, no second file to keep synchronized with the
  branches themselves. `lane-claim-check.sh` already applies the same reasoning by reading the
  branch's own birth time instead of maintaining one. This does not make the report act on its
  own, and it should not. Deleting a branch is exactly the kind of judgment AGENTS.md reserves for
  a person. What it does is make *not acting* visibly compound, the same age climbing every
  session. It does not reset to silence the moment the report has been printed once.

## What this would cost

The proposal measured the grace window against the branches that day, and the judgment each flagged
shape costs a maintainer. The 24-hour window in the table below comes from it.

- [what-this-would-cost.md](0580-nobody-reads-branches/what-this-would-cost.md): the branch ages
  behind the 24-hour window, and the count of branches per shape that need judgment.

## Threshold and action, per shape

No single threshold, because no single action fits every shape. All four share one constraint:
the mechanism reports; a person deletes. Nothing here is proposed to run unattended and delete
anything, for the reason the refused Actions-workflow option already names.

| Shape | Past the 24h grace window | Action, and who decides |
|---|---|---|
| Real work, no pull request (case 1) | Report age, commit and file counts | A person's call: `gh pr create --draft` to claim it visibly per §90, or, if it is genuinely abandoned, land what it found in `notes/` and delete, per AGENTS.md's own line that "a branch holding a finding should have the finding landed in `notes/` first" |
| Empty §90 claim, dead lane (case 2) | Report age | Delete; nothing is lost |
| Merged, not deleted (case 3) | Report age and pull request number | Delete; this widens `lane-claim-check.sh`'s existing `LEFTOVER` check past `milestone/*` |
| Closed, not merged (case 4) | Report age and pull request number, kept visually distinct from case 3 | Read the closed pull request's thread for the reason it closed. Never recommend deletion from this line alone; the work may exist nowhere but this branch |

Implemented, not as a new script. calef's ruling (see "What calef decided" above) put this in
`helpers/lane-claim-check.sh`. It already exists, already runs, and already has a reader, which beat
a fourth thing competing for the same attention. The script now covers every branch except `main`
and `gh-readonly-queue/*`. It splits `MERGED` from `CLOSED` (`state` alone is a reliable GraphQL
enum for this, no need for `mergedAt`). It classifies each unclaimed branch as empty or holding
work before printing it. See its own header for the mechanism and its `BUGS` section for what this
widening cost.

## What is out of scope

- Not a general queue-automation proposal. This does not touch `merge-drain.sh`'s admission
  policy, the stuck-checks report, or how the merge queue itself orders candidates.
- Not auto-deleting anything, ever, regardless of which shape or how old. Every deletion in the
  table above is a person's action after reading the report.
- Not resurfacing stale drafts. PR #1087 is evidence for this proposal's design argument, not a
  case it handles; that gap belongs to `merge-drain.sh`'s existing `stale_drafts` check, or to a
  proposal of its own about re-surfacing a report that already fired once.
- Not fixing `lane-claim-check.sh`'s `LEFTOVER`/`CLOSED` conflation. Named above so it has a
  home; fixing it is a separate, smaller piece of work.
- Not replacing `lane-claim-check.sh`'s early-window check or `merge-drain.sh`'s stale-draft
  check. Both are tuned for a different clock, minutes rather than days, and both are working for
  the case they were built for.
- Not a change to §90's claim mechanism itself. The claim stays a draft pull request; this
  proposal is about what happens when a branch never reaches one, or reaches one and then nobody
  looks again.

## Follow-on

- **Done.** The `LEFTOVER`/`CLOSED` conflation this proposal named as out of scope was fixed in the
  same change that implemented it. `helpers/lane-claim-check.sh` now reports a merged pull request
  and a closed-unmerged one on separate lines, and never recommends deleting from the closed line.
- **Recorded.** What the widening cost stays a limitation, in that script's own `BUGS` section. The
  activity feed is read one page deep and an older branch reports with an unknown age. The script
  writes to stdout only, so nothing reaches a lane that is not looking. And "holds work" is sized
  by `git diff --shortstat` rather than read.
- **Refused.** A fourth watcher script, `helpers/orphan-branch-check.sh`, which was this proposal's
  own closing recommendation. calef refused it on 2026-09-23: a narrow new watcher competes for the
  same attention the tree already fails to spend on the three it has. The same reasoning covers
  re-surfacing a stale draft that `merge-drain.sh` already reported once. This proposal listed that
  as out of scope, and it stays with `merge-drain.sh`.

## Index row

A branch with no open pull request is invisible to every watcher the tree had, and a 2026-09-23
sweep found 25 of them, the oldest nine days old. This block separates that failure into four
shapes with different correct actions. It measures the grace window that tells a dead branch from a
live one. It records calef's ruling that the fix belongs in a script that already has a reader
rather than in a new one. `helpers/lane-claim-check.sh` now covers every branch except `main` and
`gh-readonly-queue/*`, on a second and much longer clock.
