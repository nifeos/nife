# `helpers/lane-claim-check.sh`, in full

*Name: provisional. After the script it documents.*

An appendix to [notes/merge-queue.md](../merge-queue.md). It holds what the claim check watches,
the three false positives designed out of it, why its clock runs from the branch's birth, and why
it is a report and not a gate.

```console
$ helpers/lane-claim-check.sh
lane-claim-check: LEFTOVER. milestone/121-ripgrep's #600 is MERGED; delete the branch
lane-claim-check: UNCLAIMED. milestone/194-falsification-roadmap-status has no pull request after 16 minutes. AGENTS.md §90: gh pr create --draft
```

Both of those lines are from the first run it ever did, which is the only evidence worth quoting.

## The gap it watches

It watches the gap the drain and the trunk watcher cannot see. Everything they do starts from
`gh pr list`, so a lane that pushed a branch and opened nothing is invisible to both. The rule such
a lane breaks is AGENTS.md §90 (the claim is a draft pull request; the status flip is a gate): *a
lane's first act is a draft pull request*. That rule exists because the draft is the claim. It is the
whole mechanism preventing two lanes from silently taking the same milestone, and the board is one
command.

Both briefs said so, in a section headed *First act*, with the command spelled out. That is rung four
behaving the way AGENTS.md says rung four behaves. It was the second instance of the shape in this
project's history; the first was lanes ending their turn mid-gate.

It runs first in `merge-drain.sh`'s pass, which is the unattended runner this project has. So siting
it there is the difference between a report and a report that happens.

## Three false positives, designed out

A report that cries wolf gets ignored, and then the real case goes unread with it.

| Shape | Why it is not a missing claim | What the script does |
|---|---|---|
| A branch pushed seconds ago | The lane is between its push and its create | 15-minute grace period |
| A merged lane's leftover branch | Hygiene, not a claim | Its own `LEFTOVER` line, with the pull request number |
| A branch with a ready (non-draft) pull request | A louder claim than a draft, not a quieter one | Any open pull request counts |

The grace period was measured, not picked. The branch that built this took three minutes from
`branch_creation` to its draft, and that included writing the file that made the branch non-empty.
GitHub refuses a pull request with no commits between the head and `main`. So the literal first-act
command block cannot be run straight through, and every lane has that delay. Fifteen minutes is five
times the observed case. It is comfortably under the 75 minutes `stale_drafts` waits, which is the
neighboring report and the one this must not shadow.

## The clock runs from the branch's birth

A later push does not reset it. That is the opposite of `stale_drafts` next door, and the pair is
worth reading together. A stale draft is one that stopped moving, so it watches the last commit. A
missing claim is due from the moment the branch exists, and a lane hard at work committing is
precisely the one whose absent claim matters most. A last-commit clock would go quiet for the
branches being worked hardest, which is backwards.

The birth time comes from the repository activity feed (`repos/{owner}/{repo}/activity`), because a
commit date cannot answer the question at all. A branch pushed empty carries `main`'s commit date,
and would be reported the instant it existed.

## A report, not a gate

`script/lint` was refused for a stated reason rather than by taste. The lane that most needs telling
is one mid-work and about to open its pull request anyway. Failing its build would be the least
useful moment to interrupt it. Nothing was missing from the enforcement; what was missing was
anything that looks.
