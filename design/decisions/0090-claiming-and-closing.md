---
status: DECIDED
raised: 2026-08-16
decided: 2026-08-16
ratified_by: calef
---

# 90. The claim is a draft pull request; the status flip is a gate

calef, 2026-08-16, from the observation that a team of humans would use an
issue tracker to stop two developers taking the same task, that this project has no analog, and
that **an issue tracker cannot be stuck in a merge queue.**

## Two problems that look like one

Claiming. Nothing stops two lanes taking the same work. The roadmap cannot serve as the claim
board, and the reason is structural rather than a matter of discipline: claiming would mean
editing `design/roadmap/`, which means a pull request, which means the merge queue, which is
twelve minutes and not atomic. By the time a claim lands the other lane is half finished.

Closing. The roadmap's status field lags the tree. Four instances in one week: milestone 43
was BUILT eleven days before its status said so, 65 and 107 the same, and 54 was hidden behind
them. Milestone 31's lane discovered its own phase 3 had been built by milestone 50. That is
§76's subject, and it is a *closing* failure, not a claiming one, so it needs its own mechanism.

Separating them is most of the answer, because the two want opposite properties. A claim is
ephemeral coordination state, true for two hours and then false forever, and it must be instant.
A status is a durable record, and it should be slow, versioned and gated exactly like every other
record here.

## The claim: a draft pull request, opened when the branch is cut

A lane opens a draft pull request the moment it cuts its branch, before any work. The board is
`gh pr list --draft`, and the milestone number is already in the branch name by the prefix
convention.

The property that makes this the right shape rather than a second tracker: the claim and the
deliverable are one object. It is atomic (a ref push and an API call), instant, and *cannot* be
stuck in the merge queue, because a draft is unmergeable by construction. It becomes the real
pull request when the lane is done, so nothing has to be reconciled or closed by hand. A lane that
dies leaves a visible stale draft rather than an invisible gap, which is the correct direction for
this to be wrong in.

**Why not GitHub Issues**, which is the honest human answer and was considered: it is a second
place where truth lives, and its state has to be reconciled with the roadmap by somebody. This
project's recurring failure is exactly a fact living in two places and disagreeing (§76 is a whole
decision about that). Issues are also disabled on this repository today and nobody has missed
them. If the day comes that outside contributors need to file things they cannot branch for,
issues are the right answer and this decision does not foreclose them.

**Why not the branch alone**, which the plural-maintainers rule already requires: a branch says
somebody is working on *a branch*, and only the naming convention connects it to a milestone. A
draft pull request carries a title, a body saying what the lane intends, and shows up in the same
list as everything else in flight. The branch remains the underlying claim; the draft is what
makes it legible.

## The close: a gate, not a discipline

**A branch named `milestone/N-*` may not merge without touching `design/roadmap/N-*.md`.**
Fifteen lines in `script/lint`, and it would have caught all four of this week's misrecordings.

It forces the status flip into the same merge as the work, which is where it belongs: merging is
what finishes a lane, and anything not attached to the merge is attached to whoever happens to
notice, which is rung zero.

**One caveat that must be understood or the gate reads as wrong.** Lanes are forbidden to edit
`design/` (numbers and names are global to the tree and minted by the integrator). So this gate
is aimed at the integrator at merge, not at the developer: the lane reports what status it
believes its milestone should carry, and the integrator lands that flip in the merge. A lane that
trips this check locally has found the integrator's job, not its own.

The escape, and it is deliberate. A `milestone/N-*` branch that genuinely changes nothing
about milestone N's status still has to touch the file, if only to record why it did not move.
That is a feature: "we worked on N and its status is unchanged, because X" is exactly the sentence
that was missing four times this week.

## What this does not solve

Two lanes can still collide in the same *files* without claiming the same milestone, which is what
happened twice on 2026-08-15 and is a different problem with a different answer (the lane-count
rule now reads against the collision surface, and `git ls-remote --heads` is the ledger). And a
draft pull request claims work that has a branch; a design question with no branch is a
`design/decisions/` entry with `**Status: PROPOSED.**`, which is where those already live.

## Amendment, 2026-09-05: the claim needs an empty commit, and one sentence here was false

A draft pull request whose head holds nothing its base does not will be closed by GitHub as
`merged`, while still a draft. So the claim disappears from the board without anybody doing
anything, and `gh pr list --draft` reads empty while a lane is working. That is the one outcome this
section exists to prevent.

The window is narrow, which is why it took three days to see. It opens when a lane bases on a
`maintainer/mint-*` branch (the milestone block is not on `main` yet, so the lane has to) and closes
at the lane's first commit. If the mint pull request lands inside that window, the head has no commit
of its own, GitHub concludes the work is already in `main`, and the draft is marked merged.

The fix is one line and it closes the window by construction, rather than making it smaller:

```sh
git commit --allow-empty -m "claim: milestone N"
```

The head then carries something the base never will, so nothing that happens to the base can conclude
the pull request is finished. AGENTS.md's instruction says this; this section carries the reason.

A draft cannot be enqueued. It can be merged, by GitHub, without passing through the queue at all,
so "a draft cannot be merged" is not a reason for the convention.

### The four cases, and why the rate mattered more than the mechanism

Measured 2026-09-05 over the last two hundred merged pull requests:

| branch | date |
|---|---|
| `milestone/221-soak-crossing` | 2026-09-02 |
| `milestone/220-jh7110-clock-and-reset` | 2026-09-04 |
| `milestone/259-notes-sweep` | 2026-09-05 |
| `milestone/260-xenon-netboot` | 2026-09-05 |

The first is the day the `maintainer/mint-*` pattern began, so this has been true for as long as the
pattern has. It went unnoticed because it is rate-dependent, and that is the part worth carrying
to the next rare failure in this tree: at one occurrence every couple of days it reads as noise and
nobody connects two incidents two days apart. Two in one evening reads as a pattern. Nothing about
the mechanism changed on 2026-09-05, only the number of lanes, which is AGENTS.md's own observation
about the bottleneck moving, arriving as a defect becoming visible rather than as a constraint.

Both 2026-09-05 lanes recovered on their own by opening a replacement pull request, so nothing was
lost. What was lost is the property the board is for: for part of that evening it showed no claim on
two milestones that were actively being worked.
