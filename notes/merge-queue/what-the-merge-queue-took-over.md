# Before the merge queue: four shapes of the drain, and the prevention half

*Name: ratified 2026-10-18 (calef, "Approve as a group"). Refused `before-the-queue`.*

An appendix to [notes/merge-queue.md](../merge-queue.md). It holds what the platform's merge queue
took over from `helpers/merge-drain.sh`, the four shapes the drain's loop took before it, and why
the prevention half was never these scripts.

## What the merge queue took over

GitHub's merge queue was enabled on this repository by the organization move of milestone 120 (the
rename: the OS becomes `nife`, and the project gets an organization). The setting exists only for
organization-owned repositories. The queue serializes candidates, tests each against the tip,
and ejects what fails. That is precisely what the script had been reconstructing from outside. Three
things changed at once:

- Ordering stopped being ours. Enqueue everything eligible and let the queue decide.
- Updating a branch became neither necessary nor possible. The queue builds the merge candidate
  itself, and GitHub answers `update-branch` on a queued pull request with a 422.
- "Arm exactly one" became the wrong answer rather than a redundant one, because it holds ready work
  back for a cycle when arming is free.

## The four shapes

The history is kept because it is evidence about the up-to-date rule, not about this script. The
merge queue can be turned off, and if it is, every one of these failures returns. The loop took four
shapes, and three of them starved something:

1. Arm the head only. #134 sat CLEAN with twelve green checks behind a lower-numbered pull request
   that was still building. calef found it, not the script.
2. Arm everything. That starved the head instead. Under the up-to-date rule a merge stales every
   other branch. So a small doc-only pull request goes green during a big one's thirty-minute cycle,
   merges, and sends the big one back to the start. #117 was re-updated twice that way.
3. One target. Both failures are one fact from two sides. A merge is exclusive, so the queue can only
   land one thing at a time, and the only question is which.
4. Whatever is in flight finishes first. The third shape preferred a CLEAN pull request, on the
   reasoning that it lands in minutes. Wrong: merging the cheap one stales the one in flight. So a
   five-minute merge costs a thirty-minute one a whole further cycle, and saves nothing, because the
   cheap one would have landed straight afterwards anyway. #120 paid three cycles while #137 and #139
   went past it.

The rule those four shapes were groping toward: order the two operations by what they cost the
queue, not by what they cost themselves. A merge queue is that rule implemented by the platform,
which is why the script no longer needs to hold it.

## The prevention half, which is not these scripts

`main` went red on 2026-08-04 because two pull requests, each green against the base it was cut
from, merged in an order neither had ever been tested in. One added `script/citations`. The other
added a gate requiring every `script/` entry point to carry a provenance block. Neither branch ever
contained the other.

No per-pull-request check can see that, because the failing input is the merge order, which is not a
property of either branch. GitHub's require branches to be up to date before merging is the
mechanical answer, and was applied the same evening (§73 (ten admin minutes)). It converts that failure from a red trunk into one re-run. These scripts are
the detection half; that rule is the prevention half, and it is the better one.

The merge queue is the same prevention with the cost removed (2026-08-16). Up-to-date-before-merge
buys the guarantee by making every author pay for it serially, in full CI cycles. That is what made
the ordering brain above necessary, and what milestone 119 (the merge queue is the bottleneck, and
the long pole is one prover) measured as the bottleneck. The queue tests the same thing, the
candidate against the tip, without staling anybody's branch to do it. Same prevention, one rung up:
the platform holds it, rather than a rule everybody has to route around.
