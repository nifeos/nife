---
name: developer-lane
description: >-
  Rules for a developer lane in nife: a subagent executing exactly one milestone in its own worktree
  and branch. Load first, before any work in a lane, and when briefing one. Covers the claim
  (script/claim, an empty commit and a draft pull request before any work), what a developer may
  never do (merge, mint, edit design/decisions/, design/ or AGENTS.md except its own roadmap block
  or a ruling it was given),
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
  edits `design/decisions/`, `design/` or this file, with two exceptions. It edits its own
  milestone's roadmap block, which `script/lint-fleet` 4b requires. And it may edit
  `design/decisions/` to record a ruling it was given, only once `script/record-ruling` has cleared
  `needs-architect` on its pull request (§88 (`needs-architect` as a required check); ruled
  2026-10-25 UTC). Names anything new provisionally and says so. A
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
  Check that board before briefing, alongside `git ls-remote --heads`. One kind of lane claims
  otherwise: a fix lane for a held fuzz finding, below.
- `script/claim <branch> --debt-paydown` or `script/claim <branch> --new-work` makes that claim in
  one command, and refuses a lane that passes neither or both (calef's ruling R1 on #1894,
  2026-10-10 UTC). `--debt-paydown` is for work that meets the definition in
  [notes/debt-paydown.md](../../debt-paydown.md), and it puts the `debt-paydown` label on the draft.
  Everything else is `--new-work`. A brief says which one, so the lane doesn't have to guess.
- **Push the real commits, confirm the remote head, and only then mark the pull request ready.**
  Push with `script/push`, which reads the branch back from origin. Then confirm the head is the
  commit you mean: `git ls-remote origin refs/heads/<branch>` prints the same commit as
  `git rev-parse HEAD`, and `gh pr view <number> --json headRefOid --jq .headRefOid` agrees. Run
  `gh pr ready <number>` and `helpers/arm.sh <number>` after that and not before. `helpers/arm.sh`
  is the one way to arm: it refuses a draft and a held pull request, and runs
  `script/queue-conflicts`, which merges the pull request against `main` plus the entries ahead of
  it and refuses a conflict the queue would remove you for. A push to a pull request that is already
  armed or queued is `script/push --requeue`, which dequeues, pushes and arms again. The
  `empty diff` check has no draft skip, so marking ready before the commits arrive makes it run on
  the claim-only head and fail. The failure is accurate and clears on the next push, but a pull
  request that is already waiting loses minutes to hours before the check passes again.
- A developer works in a lane, and the lane is the isolation rather than the person: its own
  worktree, its own branch, one milestone, no visibility into the others. Two developers in one lane
  is forbidden; it is the merge problem this vocabulary exists to prevent.
- **Run `script/lint --quick` before the full lint, and again before each push.** It runs the
  cheap sections that fail a lane's first run most often (citations and their glosses, roadmap
  status, the prose, module-block and parent-index ratchets, em-dashes, package boundaries;
  `script/lint --quick --list` names them) in a fraction of the full run's time, so a missing gloss
  costs seconds and not a rerun. Commit first: the citations sections judge the committed tip and
  refuse uncommitted edits. It covers fewer sections than the full run, which still gates the push
  and CI, so it never replaces `script/lint`. Run the full lint once when the work is done, read
  every failure it names (it runs all sections and lists them all at the end), and fix them
  together.

A developer's final report ends by handing off: what its work unblocked, and what it found that
wants a lane of its own.

What a lane writes into the tree states what is true when it merges. What it changed, what that
replaced and why go in its commit messages and pull request body, never in the document (§298, a
document states what is in force now).

A lane that writes a note reads it against the code it describes, [§310 (read a note against its
code)](../../../design/decisions/0310-read-a-note-against-its-code.md): the note names the item and
keeps the reason, and what is about one crate alone goes into that crate. An agreement between two
programs is code both sides use, never prose alone, [§311 (a contract is code, not
prose)](../../../design/decisions/0311-a-contract-is-code-not-prose.md). A lane that finds one held
only in prose proposes a milestone with a sketch of that code, and does not write it.

## A fix lane for a held finding

A held finding is a fuzz finding whose fix has not reached main, so it stays private. The fuzz host
files one private security advisory per finding, and the advisory holds the finding class and the
reproducer. By default this kind of lane develops its fix in the GitHub temporary private fork of
its advisory, as §90 (the claim is a draft pull request) allows, and a person reviews it there.
Only then does the reviewed fix land on main, as an ordinary pull request through the merge queue.
The reason is exposure: a draft pull request and its pushed branch are public from the first
commit, so the bug is public only while the reviewed fix waits in the queue.
The rulings are in [the proposal](../../../design/roadmap/1015-fuzz-findings-without-a-person.md),
and [fix lanes read the advisory](../../../design/roadmap/1014-fix-lanes-read-the-advisory.md)
built the reading side. A finding the architect judges low severity is the exception: its brief
says so, and the lane follows the regular workflow with a public draft pull request, keeping the
rules below on reading the advisory and naming no reproducer.

- **Read the advisory, and only the advisory.** No lane opens, copies, prints or moves anything under
  the fuzz host's held directory, and none looks for a credential. The advisory is the one way a lane
  learns a finding. `script/read-advisory GHSA-xxxx-xxxx-xxxx` reads it with the one-hour reader token
  the lane was launched with, from the channel named at the top of that script, and refuses without
  one. It prints to the terminal and writes nothing; it refuses a redirect into the repository. If a
  lane wants the text in a file, the file goes in the lane's own scratch space outside every
  repository, and is never committed.
- **The claim is a note on the advisory.** The maintainer writes it when it launches the lane, and
  the advisory list is the claim board for this kind of lane. Cut the branch and worktree with
  `script/claim <branch> --new-work --no-claim`, which pushes nothing to nifeos/nife. Push nothing
  to nifeos/nife, not a branch and not a draft, until a person has approved the fix.
- **Work in the private fork.** The brief names the fork, `nifeos/nife-ghsa-xxxx-xxxx-xxxx`. Add it
  as a remote, commit often, and push the branch there, since a pushed commit survives the loss of
  the worktree. `script/push` reads back origin only, so push to the fork with `git push` and
  confirm the head with `git ls-remote`. Open one pull request in the fork against `main`, with a
  neutral title that names the area and says "fix", never the finding.
- **Run the local gates, because the fork runs none.** GitHub runs no status checks in a temporary
  private fork, so nothing else checks the fix while a person reviews it. Run `script/fmt`, the full
  `script/lint` and the tests the change reaches, read each exit, and put each gate and its exit in
  the report and the fork's pull request body.
- **Land the reviewed fix through the queue.** Nobody merges from the private fork: main's ruleset
  has no bypass actor, and the fork's merge button needs one. Once a person approves the fix in the
  fork, push the same commits to the lane's branch on nifeos/nife with `script/push`, confirm the
  remote head, open the pull request ready (not as a draft) with the same neutral title, and arm
  auto-merge. The merge queue runs every gate on it. A person publishes the advisory when it merges,
  and no lane publishes.
- **The public window is the time the pull request spends in the merge queue.** Push only once the
  fix is approved, so the window holds no review. If a hold lengthens it (`needs-architect` on a fix
  that touches the syscall surface, or a red trunk), say so in the report, with the push and merge
  times, so the window is measured.
- **Name no reproducer in public text.** The fix carries the fix and a regression test of the
  invariant that broke. The test names no seed, input or replay line. The title, body, commit
  messages and comments name none either, and neither do logs or artifacts the lane makes. A seed is
  not a single token to avoid: a description detailed enough to rebuild the input is the same leak.
  `script/lint` refuses a commit message with a replay line or a seed assignment
  (`helpers/replay-line-check.sh`). A workflow (`.github/workflows/pr-text-replay-check.yml`) runs
  the same script over the pull request's title and body, on open and on every edit, and fails the
  pull request's check, so the lane does not pipe them by hand. The check
  catches the two forms the drivers print and not a seed written some other way, so the lane also
  reads its own text before pushing.
- **The seed joins the fixed-seed regression list later.** A second pull request, opened after the
  advisory turns public, adds it. The lane that fixed the finding does not add it in the fix.

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

## What drives the work

Ruled 2026-10-31 (UTC). A lane brief answers one question before a lane starts, and the review of
the lane's pull request asks it again: what drives this work now, and what does it save? The answer
is a sentence, such as "the updater in milestone 976 (an OS update is a set of packages that lands through a slot) calls it" or "it cuts a kernel download by under
1 MB per update". "Nothing drives it yet" is an allowed answer. It is a reason to hold the work for
its driver, not a failure, because a program that waits for its users is not waste. There is no
field for it and no lint check. `helpers/consumer_census.py` counts delivered work nothing uses,
report-only, and the maintainer runs it monthly; its counts cannot tell work not yet consumed from
work built with no reason, so a person asks. The reasoning is in
[the proposal](../../../design/roadmap/0996-delivered-work-names-its-consumer.md).

## Shared state: the tree's, and the machine's

Anything global to the tree is assigned by the integrator at merge, never claimed by a lane.
Concurrent lanes cannot see each other, so a lane that reaches for a shared resource is guessing.
The collisions that produced this rule are in
[design/tenets/shared-state.md](../../../design/tenets/shared-state.md).

- `design/decisions/` section numbers. Preferred: a lane does not touch `design/decisions/` beyond
  recording a ruling it was given, puts the reasoning in `notes/` and in its report, and the
  integrator mints the section at merge.
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
