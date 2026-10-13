# A paused draft names its blocker, and the drain reads it back

Provisional name, in the family of `notes/merge-queue.md`, which this note extends without editing
(its own prose is already over the bold-density budget calef ratified on 2026-09-26, and fixing
that is a separate piece of work). See that note's "`Blocked-by: #N`: a hold that releases itself"
section for the older half of this convention, the one that holds a queued, non-draft pull request
out of the merge queue.

## The gap this closes

Draft #1289 was paused on 2026-09-25, waiting on #1288. #1288 merged an hour later. Nobody resumed
#1289 for two days, because the pause was recorded only in prose, a comment or a lane report, and
nothing re-reads prose once it is written. (The drain's stale-draft note, which assumed a quiet
draft was finished, was the wrong note for a lane that is waiting, and was removed on 2026-10-03
with the drain's other stall comments.)

## The convention

A draft that is paused waiting on another pull request names it in its own body:

    Blocked-by: #1288

or, for more than one:

    Blocked-by: #1347, #1354

Both spellings work; the numbers are found wherever they sit on the line. Only the first line
naming `Blocked-by:` is read, so every number a draft depends on has to sit on that one line.

## What the drain does with it

`merge-drain.sh`'s `unblocked_drafts` pass reads every open draft carrying such a line. For each
named pull request it asks GitHub for the state: merged, closed, or still open. Three outcomes
follow, decided by `helpers/blocked-by-resolution.jq` (checked by its own selftest against fixtures,
with no GitHub call):

- Still waiting: at least one named pull request is still open. Nothing happens.
- Unblocked: every named pull request merged. The draft gets the `unblocked` label and one
  comment, naming each one and when it merged, ending "resume this lane."
- Closed unmerged: at least one named pull request closed without merging. The draft gets the
  same label, but the comment says so plainly and asks whoever reads it to check whether the plan
  changed, because that is usually what a closed-unmerged blocker means.

The comment posts once, the same marker-comment mechanism `notify` already uses for a stalled or a
stale-draft note, so a lane is not renotified every five minutes once its blockers have resolved.

The board is one command, in the shape `gh pr list --draft` and `gh pr list --label
needs-architect` already are:

    gh pr list --label unblocked

## The label removes itself

`release_unblocked_labels` runs every pass too. It takes the label back off a draft the moment its
reason stops applying: a new commit (the lane resumed before anyone read the comment) or
`ready_for_review` (the draft is not paused any more, it is asking to merge). Nothing here needs a
person to remember to take the label off.

## Where to write it

`briefs/session-start.md` (in the section on parked lanes) and `briefs/gate-in-ci.md` (its `WAITING`
protocol) both say to add `Blocked-by:` to a draft's own body before ending a turn that pauses on
another pull request rather than on CI.

## How `Blocked-by:` is read

The parser is `helpers/blocked-by.sh`. It reads every number on the line, and both consumers, the
admission hold and this draft pass, honor the whole list. Only the first *line* naming
`Blocked-by:` counts; a second one lower in the body is ignored. This belongs in
`notes/merge-queue.md` itself and does not fit there yet for the reason in this note's own opening
paragraph; whoever next touches that document's bold count should fold this note back in.

## BUGS

- This note duplicates a little of `notes/merge-queue.md`'s framing (why a generic hold label
  was refused, why a marker comment posts once) rather than editing that document, for the reason
  given above. Once somebody brings that document's bold spans under 2026-09-26's budget, this note
  should be folded back into its `Blocked-by:` section and deleted.
- `release_unblocked_labels` costs one `gh api .../timeline` call per draft still carrying
  `unblocked`, to find when the label was added and compare it against the last commit. The set is
  normally small, but it is an extra round trip nothing else in the drain needs.
- A draft's `unblocked` label can be stale for one pass. If a lane pushes a commit and the drain has
  not run since, the label still reads as if the draft were paused. The next pass clears it, five
  minutes later at most under the scheduled workflow.
