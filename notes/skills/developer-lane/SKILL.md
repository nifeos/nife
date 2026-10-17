---
name: developer-lane
description: >-
  Rules for a developer lane in nife: a subagent executing exactly one milestone in its own worktree
  and branch. Load first, before any work in a lane, and when briefing one. Covers the claim
  (script/claim, an empty commit and a draft pull request before any work), what a developer may
  never do (merge, mint, edit design/decisions/, design/ or AGENTS.md except its own roadmap block),
  never polling CI (end the turn with `WAITING [<owner>/<repo>:]<run_id>... on <what>`), when a lane stops, the
  report and its handoff, where identified work must go (a proposed milestone or a BUGS entry), and
  shared state (provisional section numbers, each worktree's own toolchain link, branches that hold
  knowledge).
---

# The developer lane

Where this says "this file", it means `AGENTS.md`. Its name was ratified by calef on 2026-10-06
(UTC); notes/skills/README.md records it.

The maintainer side of the same queue is in [the maintainer](../maintainer/SKILL.md).

- Developer. A subagent executing exactly one milestone. Reports; never merges, never mints, never
  edits `design/decisions/`, `design/` or this file, except its own milestone's roadmap block,
  which `script/lint-fleet` 4b requires it to edit. Names anything new provisionally and says so. A
  developer never polls CI. It ends its turn with `WAITING [<owner>/<repo>:]<run_id>... on <what>` and the
  maintainer resumes it once the runs finish. A run in a repository other than nifeos/nife
  carries it, `WAITING nifeos/basalt:<run_id> on <what>`; a bare id is a nife run. The report comes after the gate, and nothing about a
  gate is finished until you have read its exit. A lane continues until it needs a human or it is
  done (calef, 2026-08-26): finishing one item on a milestone's own list is not a stopping
  condition when the list has more on it. The one genuine stop is hitting something that is an
  architect's call: a design fork, a wire format, a naming decision. Write that up as a proposal
  and stop there, rather than either inventing an answer or ending the turn early.
- A lane's first act is a draft pull request, §90 (the claim is a draft pull request). Cut the
  branch, make one empty commit (`git commit --allow-empty -m "claim: milestone N"`), push it, and
  open the pull request as a draft, before any work. That is the claim, and it is why two lanes
  cannot silently take the same milestone: the board is `gh pr list --draft`. The empty commit is
  what keeps that true: without it GitHub closes the draft as *merged* when the lane's base lands.
  Check that board before briefing, alongside `git ls-remote --heads`.
- `script/claim <branch> --debt-paydown` or `script/claim <branch> --new-work` makes that claim in
  one command, and refuses a lane that passes neither or both (calef's ruling R1 on #1894,
  2026-10-10 UTC). `--debt-paydown` is for work that meets the definition in
  [notes/debt-paydown.md](../../debt-paydown.md), and it puts the `debt-paydown` label on the draft.
  Everything else is `--new-work`. A brief says which one, so the lane doesn't have to guess.
- A developer works in a lane, and the lane is the isolation rather than the person: its own
  worktree, its own branch, one milestone, no visibility into the others. Two developers in one lane
  is forbidden; it is the merge problem this vocabulary exists to prevent.

A developer's final report ends by handing off: what its work unblocked, and what it found that
wants a lane of its own.

What a lane writes into the tree states what is true when it merges. What it changed, what that
replaced and why go in its commit messages and pull request body, never in the document (§298, a
document states what is in force now).

## Identified work leaves the lane in a tracked form, or the merge waits

A lane that finds work it is not doing may report it in exactly two shapes, and "worth doing
someday" is neither. Either a proposed milestone (provisional; the integrator mints the number at
merge like every other global name), or a recorded limitation written where a reader meets the
feature, in the `BUGS` section beside it. A finding with no home is the integrator's cue to hold the
merge until it has one. Why a lane report and a pull request body do not count as records is in
[design/tenets/routing-work-and-decisions.md](../../../design/tenets/routing-work-and-decisions.md).

The merge checklist grows one line: every piece of identified work in the lane's report has a home.
[`briefs/merge-and-cleanup.md`](../../../briefs/merge-and-cleanup.md) has that checklist whole, with the
prune and the relink. Two things this deliberately does not do: it does not gate, because no check
can tell an intention from an observation in prose, and it does not touch the `BUGS` convention.

## Shared state: the tree's, and the machine's

Anything global to the tree is assigned by the integrator at merge, never claimed by a lane.
Concurrent lanes cannot see each other, so a lane that reaches for a shared resource is guessing.
The collisions that produced this rule are in
[design/tenets/shared-state.md](../../../design/tenets/shared-state.md).

- `design/decisions/` section numbers. Preferred: a lane does not touch `design/decisions/` at all,
  puts the reasoning in `notes/` and in its report, and the integrator mints the section at merge.
  If a lane must write the section to make its own gates pass, the number is provisional: say so in
  the report, and expect renumbering.
- Milestone numbers a lane mints are provisional the same way. When one collides, renumber the
  block's file and H1, the commits that name the number, and the pull request title, but keep the
  branch name: renaming the head branch of an open pull request closes the pull request (#1901,
  2026-10-10 UTC, cut as 868 and merged as 871). The pull request title carries the new number,
  and the body's first paragraph gives the old number, the branch it kept and why. The block states
  only the milestone as it now is (§298 (a document states what is in force now)). `script/lint`
  4b finds the moved block when its slug equals the branch's or its text names the branch, so a
  lane that also retitles writes the branch name into the block. `script/claim` refuses a number
  an open pull request's title or branch already holds.
- Counts that span the tree. Take such a number at merge, from the merged tree.

Some shared state is global to the *machine*, not the repo, and `rustup toolchain link` is the one
that has bitten. Until 2026-10-10 every worktree linked one `nife-dev` name, and every lane that
gated took it from the others. Each worktree now links its own, `nife-dev-<directory>-<hash>`, so a
lane takes nothing, reports nothing and relinks nothing. Leave the bare `nife-dev` alone: lanes cut
before that date still write it, and the maintainer removes it once they are gone
([`briefs/merge-and-cleanup.md`](../../../briefs/merge-and-cleanup.md) step 4). The mechanism is in
[`notes/std.md`](../../../notes/std.md).

An unmerged branch is either abandoned or it is holding knowledge that is not on `main`, and the
second case is a bug in where the knowledge lives. Nobody reads branches. If a branch holds a
finding worth keeping, land the finding in `notes/` and then delete the branch; do not keep the
branch as the record.
