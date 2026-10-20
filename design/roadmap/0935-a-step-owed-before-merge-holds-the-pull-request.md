---
status: NOT-STARTED
promoted_from: a-step-owed-before-merge-holds-the-pull-request
raised: 2026-10-19
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 935. A step owed before merge holds the pull request until it is done

*(Title, slug and every name below provisional.)*

Raised 2026-10-19 (UTC) by `lane/before-merge-hold`, after #2393, milestone 118 (CLAUDE.md has
a budget), merged ahead of the step its own body said had to come first. calef ruled the mechanism on
2026-10-19 (UTC), below. Nothing here is built.

## What happened

#2393's body said, under "For the maintainer", that before merge a maintainer had to wire
`helpers/prune_worktrees.py --remove` into the hourly job that already runs the build-output
prune. That job is off-tree. The block it merged says the prune runs hourly, which was true only
once the wiring existed.

The pull request went ready and green with nothing armed. Thirty minutes later the merge drain
labeled it `needs-maintainer` with the `unarmed` cause, which tells a maintainer session to arm it
if it is done. The unattended maintainer routine picked it up, found it clean, green and outside
every architect surface it checks, and armed it. Its own comment named the off-tree step as left
for a maintainer session, so the routine read the precondition and armed anyway: nothing it checks
could say no. It merged at 08:43 UTC, and the block's "runs hourly" was false on `main`.

Every actor did what its rules said. The precondition was prose in a pull request body, which is
rung four of AGENTS.md's ladder, and the arming rules all read rung two.

## What decides what may be armed

Three things, and none reads the body for a precondition.

- `architect-hold.yml`, the required check `architect hold (needs-architect label)`. It fails
  while `held-by-lane` is on, passes on `architect-ruled`, and otherwise fails on
  `needs-architect`. A red required check makes `mergeStateStatus` read `BLOCKED`, so the queue
  will not merge the pull request even when it is armed.
- `helpers/merge-drain.sh` arms nothing. It dequeues anything carrying `needs-architect`,
  `held-by-lane` or `held-for-red-trunk`, and `helpers/needs-maintainer.jq` decides when to label
  `needs-maintainer`. Its `nm_held` list keeps `unarmed` off a pull request wearing one of those
  three labels. `Blocked-by: #N` in the body also holds `unarmed` off, and it releases itself
  when the named pull request resolves.
- The unattended maintainer routine, off-tree. It arms a ready pull request whose merge state
  is clean, whose checks are green, and which touches no architect surface. A red required check
  is enough to stop it, because the merge state is then not clean.

So any hold that turns `architect hold` red stops both the queue and the routine, with no edit to
the routine.

## The ruling

calef chose option C on 2026-10-19 (UTC), in his words: "A '- [ ] Before merge: ...' task-list
item in the PR body is the hold; architect hold fails while one is unticked, and ticking it
releases it."

The check folds into the existing `architect hold` job, so the ruleset is unchanged. The off-tree
maintainer routine is unchanged too: it arms only clean, green pull requests, and a red required
check is not clean.

## The written step is the hold

The lane writes the step as a task-list item whose text opens with a fixed key:

```
- [ ] Before merge: wire the worktree prune into the hourly job (off-tree; maintainer)
```

An unticked item is the hold. Ticking it, by whoever did the step, is the release. There is one
artifact, so a step with no hold cannot be written in this form, and the edit history records who
ticked it and when.

The parts:

1. One parser, `helpers/owed-step.jq`. It reads a body and yields each `Before merge:` item
   with its state. It skips fenced code and quoted lines, so this block's own example holds
   nothing. It is spliced into its consumers the way `helpers/queue-eligible.jq` is.
2. `architect-hold.yml` fails on an unticked item, as a second step of the same job, which
   gains `edited` among its `pull_request` types. The job's name is the ruleset string, so the job
   keeps it, and the name understates what the check holds.
3. The release reruns the old red runs. `architect-ruled-clears-hold.yml`'s `rerun` job also
   fires on `edited` when `changes.body.from` held an unticked item and the new body holds none.
   Without it, the #1766 shape recurs: an old red run on the same head keeps an armed pull request
   out of the queue.
4. The drain dequeues a queued pull request whose body gains an unticked item, beside the
   labels `dequeue_held` already reads.
5. A new `needs-maintainer` cause, `owed-step`, in place of `unarmed` on a ready, green pull
   request with an unticked item. Its comment quotes the step and says to tick it once done. This
   is what tells a maintainer session there is work, where `unarmed` says to arm.
6. The lane skill and the pull request front matter say to use the form for any step that must
   precede merge, and say that ticking it is the doer's job, never the lane's or the routine's.

What it does not do: it cannot stop someone ticking a box for a step they did not do. A person or
routine that edits a body to release a hold is acting on purpose, and the edit history shows it.
It cannot see a precondition written in prose outside the form, because no check can tell an
intention from an observation in prose (`notes/skills/developer-lane/SKILL.md`). The skill text is
the floor for that residue, and the skill says so where a reader meets it.

## Recommended, not ruled: do the step first where it can go first

calef's ruling covers the hold and says nothing on this, so it is a recommendation and the build
lane does not write it into a skill without his ruling.

Where the code can be made harmless until the step lands, the step would go before the pull request
is marked ready, and nothing would be owed at merge. Pull request #2393 had that shape: the job line
was behind an `[ -f ]` guard, so wiring it first was safe. As a skill rule it would be rung four,
and it complements the hold rather than replacing it. A step that can only follow merge (a pin
bump, a ruleset change that names a new check) takes the `Before merge:` item either way.

## Not taken

- Reusing `held-by-lane`. `helpers/architect-ruled.sh` treats it as a ruling already given, so
  both architect labelers skip a pull request wearing it, and a lane using it for an off-tree step
  would silence `needs-architect` on its own diff. It also means "calef ruled and the lane owes a
  change", where here the maintainer owes the step.
- A new hold label, set by the lane and cleared by whoever does the step. The step and the label
  are two artifacts that can disagree, and the #2393 failure, a step written with no hold, stays
  possible. `Blocked-by:` was chosen over a manual hold label for the same reason
  (`helpers/merge-drain.sh`, above `nife_blocked_by`).

## The work

- `helpers/owed-step.jq` and a selftest with fixtures: unticked holds, ticked releases, two items
  with one ticked hold, an item inside a code fence or a quote does not hold, and a near-miss key
  (`Before merging:`) does not hold.
- `architect-hold.yml`'s second step and the `edited` trigger; `architect-ruled-clears-hold.yml`'s
  rerun on a body release; `dequeue_held`; the `owed-step` cause in `helpers/needs-maintainer.jq`
  with fixtures in `helpers/needs-maintainer-selftest.sh`.
- The developer-lane skill and the maintainer skill: the form and who ticks.

## Falsification

- Each parser guard removed (code fences skipped, quotes skipped, ticked state read), its fixture
  must fail.
- `helpers/hold-rerun-selftest.sh` already pins that workflow's triggers. Extend it to pin
  `edited` on both workflows and the second hold step, so deleting either fails `script/lint`.
- Once on `main`, one throwaway pull request measures the premise end to end, as merge-drain.yml's
  header records for the App token. An unticked item turns `architect hold` red and the merge state
  `BLOCKED` with auto-merge armed. Ticking it turns the check green, reruns the old red run, and the
  queue takes it. The result goes in the block when it is built.

## Reuse

Nothing outside the tree does this. GitHub's task-list items are the release surface, and the
parser follows `helpers/blocked-by.sh` and `helpers/queue-eligible.jq`. The rerun path is
`helpers/hold-rerun.sh`.

## Index row

Ruled, not built: a step a pull request owes before merge is held by a `Before merge:` task-list item in its body (calef, 2026-10-19 UTC). `architect hold` fails while one is unticked, ticking it releases the hold, and a `needs-maintainer` cause names the owed step.
