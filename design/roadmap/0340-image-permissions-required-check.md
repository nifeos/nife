---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 340. `script/image-permissions` reports and does not gate, because it is not in the ruleset

Premise re-checked 2026-09-19 and
it holds. `script/image-permissions` is a `local` row in `script/ci-build`'s table and has a job of
its own in `.github/workflows/ci.yml`, and `notes/check-inventory.md`'s row for it still answers
no under required. So it still reports and does not gate, and the one checkbox is still
unflipped.

The decision is [§97 (six gates run on every pull request and none of them can stop one)](../decisions/0097-advisory-checks.md). It is one checkbox in the repository's merge queue ruleset, and only
calef can flip it: it is a GitHub setting rather than a file in this tree, so no lane and no pull
request can carry the change. There is nothing to build and nothing to review; the whole item is an
ask, and the ask is a person at the settings page, which is what the frontmatter's `needs_person`
carries. No decision waits: §97 ruled, and the checkbox is its unperformed half.

§97 is `DECIDED` and its ruleset edit is not performed, which is what makes this a citation
rather than a new fork. calef ratified it on 2026-08-25 (*"Ratify as written"*), it holds that four
advisory checks become required, and its own *Not yet built* section names the ruleset edit first,
wanting a quiet queue. That is the same checkbox page this block asks for, so `image-permissions`
rides with it rather than needing a second visit.

**And §97 predicted this block in its own `BUGS`**, which is the part worth citing where a reader
meets the gate: *"This section names six advisory checks as of 2026-08-18 and nothing keeps that
list current. A check added to CI is advisory by default, so the list grows silently in the
direction of less enforcement."* `script/image-permissions` is milestone 208's, filed after that
date, and it is the seventh. What §97 did **not** decide is this check by name, so the ask is still
an ask; what it did decide is that the arrangement is wrong and that a fix wants one visit to one
page. A reader who wants the argument should read §97 and not this paragraph.

In brief. Milestone 208 built `script/image-permissions`, which refuses a kernel image carrying
a writable-and-executable `PT_LOAD`. It runs in CI and its result is visible. It is not on the list
of checks the merge queue requires, so a red run merges anyway.

## Why this matters

A check that reports and does not gate is the shape this project keeps writing down as a failure.
It sits at rung two of AGENTS.md's ladder only if it fails loudly enough to stop something; as
configured it is rung four, a note in a run log that somebody has to read. The defect it exists to
catch is exactly the kind nobody reads a log for. `kernel/link-x86_64.ld` shipped a single `RWX`
segment for however long it did, and the tree found out because a lane deleting a duplicate ELF
parser tripped over `Error::WritableAndExecutable`. That is the same discovery path the check was
built to replace.

There is a second cost, which is what the green check now means. A reader who sees the check listed
reasonably concludes the W^X property is enforced on the image. Until the box is ticked, it is
enforced on the images whose authors happened to look.

## What the ask is, exactly

Add `image-permissions` to the required checks in the merge queue ruleset for this repository, in
the same list the rest of the gates are in. If the answer is no, the honest follow-up is to say so
in `design/roadmap/0208-boot-section-wx.md` and stop describing it as a gate.

## Index row

Milestone 208 built `script/image-permissions`, which refuses a kernel image carrying a
writable-and-executable `PT_LOAD`. It runs in CI and its result is visible, but it is not on the
merge queue's list of required checks, so a red run merges anyway. As configured it is rung four of
AGENTS.md's ladder, a note in a run log somebody has to read, and the defect it catches is the
kind nobody reads a log for. The green check also tells a reader that W^X is enforced on the image.
The whole item is an ask, because the ruleset is a repository setting and no pull request can
carry it. If the answer is no, stop describing the check as a gate in milestone 208 (the x86_64 kernel image ships an RWX)'s block.
