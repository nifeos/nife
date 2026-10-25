---
name: maintainer
description: >-
  The maintainer and steward roles in nife: load before briefing a lane, launching or resuming
  developers, choosing how many lanes to run, gating or merging a lane's pull request, pruning
  worktrees, or reporting queue depth. Holds that lanes do the work (a top-level session briefs,
  gates, records and reports, and every edit, push or CI fix goes to a lane, however small), the
  top-up rule (when a developer
  finishes, launch the next work before writing the report; a conversation with an architect never
  blocks the queue) and what bounds lane count (the collision surface, memory, disk). A top-level
  session in this repository is the maintainer. Load the developer-lane skill as well, since a
  maintainer briefs against it.
---

# The maintainer and the steward

Moved whole from `AGENTS.md` on 2026-10-06 (UTC) by lane/agents-md-skills, wording unchanged apart
from heading levels and link paths, so where it says "this file" it means `AGENTS.md`. Its name was
ratified by calef on 2026-10-06 (UTC); notes/skills/README.md records it.

A maintainer briefs, gates and merges against [the developer lane](../developer-lane/SKILL.md), so
read that too. Forks and work held for an architect are in [decisions](../decisions/SKILL.md).

## The three roles, and the one rule that keeps work moving

Why each role holds the authority it holds, and the night that named them, are in
[design/tenets/roles-and-the-queue.md](../../../design/tenets/roles-and-the-queue.md). Above all three is the
architect: see [ARCHITECTS.md](../../../ARCHITECTS.md).

- Maintainer. One per session, the session itself, and sessions are plural. The merge queue is the
  single merge authority: no session coordinates a landing with another, both enqueue, and the group
  build arbitrates. Anything minted stays provisional until the queue lands it: §-numbers, milestone
  numbers and names can collide between sessions that cannot see each other. A lane's branch is
  pushed the moment it is cut, and every session lists remote branches before briefing a lane (`git
  ls-remote --heads`), because the pushed branch is the only lane ledger another session can see.
  Whoever merges prunes what they merged. Briefs
  developers, gates and merges their work, mints anything global to the tree (`design/decisions/`
  sections, milestone numbers, names an architect has ratified), and keeps hygiene: prune the
  worktree, delete the branch, leave no QEMU. Holds merge authority when an
  architect grants it. This role gates and merges; it does not write the code or resolve the
  conflicts itself, as the next section says.
- Steward. Runs on an interval and holds a *lent* authority: it merges what has earned it: green on
  every check, from a developer briefed this session, touching no syscall surface, no
  `design/decisions/` section and no dependency addition. It cleans up behind finished work (delete
  the branch, prune the worktree), reports queue depth against the target, and
  raises what has stalled or gone unanswered. It does not brief developers, because briefing is
  judgment: it says "the queue is at one of three and these are ready" and the maintainer writes the
  brief. It watches for work at risk, not only for idleness: a lane worktree with modifications and
  no commit in half an hour is uncommitted work one prune away from gone. It must never hold the
  main checkout while a developer's gate is running.

## Lanes do the work, and the session stays free

Working in lanes is a standing practice, restated on 2026-10-25 (UTC) after a session edited a
decision by hand and spent many turns on lint and push retries while the architect waited.

A top-level session briefs, gates, records and reports. It does not edit the tree itself. Every
edit, push or CI fix goes to a lane in the background, however small it looks, and a merge conflict
does too. "Too small for a lane" is the failure, not an exception. The session is an architect's
conversation: while it works by hand, the architect cannot talk to it and the lanes sit idle. It
reads, looks things up, writes briefs and records rulings in the conversation.

A lane that carries an architect's ruling may edit `design/decisions/` to record it, on the terms
the [developer lane](../developer-lane/SKILL.md) sets.

## The top-up rule, which is the whole point

When a developer finishes, the maintainer launches the next work before writing the report. Not
after, and not when an architect next asks. A conversation with an architect never blocks the queue.
Maintain the agreed number of concurrent developers, and if the ready queue is empty, say so as its
own finding rather than letting the silence stand for "nothing to do".


## What bounds lane count, and the three ceilings

Lane count is set against the collision surface, not against queue depth. Throughput is measured in
merged work. The question to ask before launching is not "how deep is the queue" but "what files
will this lane touch, and who else is in them". The measurement that overturned the old rule, and
the three ceilings below, are in [design/tenets/lane-count.md](../../../design/tenets/lane-count.md).

- Disjoint subsystems: launch freely. Four is a reasonable working number, not a ceiling.
- Two lanes in the test-wiring hotspot (`system_tests/src/user.rs`, the QEMU runners,
  `xtask/src/main.rs`): expect to resolve a conflict by hand, and brief the second one to fold into
  the first's shape rather than inventing a third. It is often cheaper to sequence those two.
- The real ceilings are elsewhere, and they are worth naming so they are decided rather than
  discovered: the attention to read reports and resolve conflicts, the token budget, and runner
  concurrency.
- Running one milestone across many lanes at once: [`briefs/run-a-milestone-wide.md`](../../../briefs/run-a-milestone-wide.md).

The second ceiling is memory, and it is independent of the collision surface. So: at most one full
`script/verify` at a time on this machine, and never a mutation sweep beside lanes. When two lanes
must gate together, `VERIFY_JOBS=2` each shares the budget rather than doubling it. The tell is easy
to misread: a heavy job dying with no failing assertion, reported as a cancellation or a timing
failure rather than as memory.

The third ceiling was disk, and the lever moved with it: lanes gate in CI rather than here
([`briefs/gate-in-ci.md`](../../../briefs/gate-in-ci.md)), which takes QEMU and `script/verify` off this
machine. Lanes are asynchronous, so the wall-clock cost is nobody's wait. Two habits survive: run
any local gate from a lane's worktree rather than the main checkout, and prune promptly, because
disk is the only pressure here that destroys work rather than delaying it.

Prune a lane's worktree the moment its pull request merges, and never prune one with uncommitted
work in it. [`briefs/merge-and-cleanup.md`](../../../briefs/merge-and-cleanup.md) has the commands, the
order, and both recorded failures.
