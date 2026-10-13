# The three roles, and the one rule that keeps work moving

*Appendix to [`AGENTS.md`](../../AGENTS.md), which carries the roles and their duties as rules. This
file carries the night that named them, why each role has the authority it has, and the failures
each clause was written against. What a maintainer session costs is in
[`notes/what-a-session-carries.md`](../../notes/what-a-session-carries.md). Moved here 2026-09-23
(UTC) on calef's authorization, unchanged in substance.*

Named 2026-08-04, after a night in which eleven agents shipped and the queue still went idle twice
because nobody's job was noticing. The roles were already real; only their names and the top-up rule
are new.

A maintainer session's own context, not lane count or token rate, is its largest cost: one
session measured at roughly 233 million tokens against 5 million for the twenty lanes it dispatched.
See `notes/what-a-session-carries.md`.

- Maintainer. One per session, the session itself, and sessions are plural (2026-08-15, the
  day two sessions' lanes met in one file). Three rules make plural maintainers safe, and two of
  them are machinery that already exists. The merge queue is the single merge authority: no
  session coordinates a landing with another, both enqueue, and the group build arbitrates.
  Anything minted stays provisional until the queue lands it: §-numbers, milestone numbers, and
  names can collide between sessions that cannot see each other, and the decisions and roadmap gates
  in every group build are what catch it, which is the old integrator-mints rule with the queue as
  the integrator of last resort. And the one new duty: a lane's branch is pushed the moment it is
  cut, and every session lists remote branches before briefing a lane (`git ls-remote --heads`),
  because the pushed branch is the only lane ledger another session can see. Machine-global state
  keeps its existing owner: whoever merges relinks the toolchain from the main checkout and prunes
  what they merged. Briefs developers, gates and merges their work, mints anything global to the
  tree (`design/decisions/` sections, milestone numbers, names an architect has ratified), and keeps
  hygiene: prune the worktree, delete the branch, relink `nife-dev`, leave no QEMU. Holds merge
  authority when an architect grants it. Maintainer, not project manager, because the name has
  to predict the authority: this role writes code, resolves conflicts and merges, and a
  coordinate-only reading of it would leave the tree unowned.
- Developer. A subagent executing exactly one milestone. Reports; never merges, never mints,
  never edits `design/decisions/`, `design/` or this file, except its own milestone's roadmap
  block, which `script/lint` 4b requires it to edit (calef, 2026-08-23, reaffirmed 2026-09-01
  after two lanes read the flat prohibition and reported the gate as impossible; the reason is
  beside the check). Names anything new provisionally and says so. A developer polls its own
  background work to completion; ending a turn to "wait for the notification" while your own gate
  is running is the failure mode, not patience (calef, 2026-08-14, after five lanes in one day
  stopped mid-gate and each needed a manual resume). The report comes after the gate, and nothing
  about a gate is finished until you have read its exit. A lane continues until it needs a human
  or it is done (calef, 2026-08-26), the standing default for every brief rather than a per-brief
  instruction: finishing one item on a milestone's own list is not a stopping condition when the
  list has more on it, and "ran out of easy things" is not the same as "ran out of things a lane can
  make progress on." The one genuine stop is hitting something that is an architect's own call (a
  design fork, a wire format, a naming decision) -- write that up as a proposal, the same shape this
  file already asks for elsewhere, and stop there, rather than either inventing an answer or ending
  the turn early because the next item looked harder than the last one.
- Every pull request and comment an agent writes opens by saying so. One line, first thing in
  the body: `**Lane:** <branch or milestone>, written by an agent; calef's account is the author
  GitHub shows.` Milestone 128 (the automation gets its own identity) is PARTIAL: its App exists and
  the scheduled workflows author as `nife-smelter[bot]`, but a lane opens its pull request with
  calef's `gh` token. This is rung four and it is honest about being rung four: the mechanism is
  128's App, and this is what the record says in the meantime. (calef, 2026-08-16: *"it looks like
  I'm talking to myself a lot and the record would be nice to clarify who is talking."*)
- A lane's first act is a draft pull request, §90 (the claim is a draft pull request),
  2026-08-16. Cut the branch, make one empty commit (`git commit
  --allow-empty -m "claim: milestone N"`), push it, and open the pull request as a draft, before
  any work. That is the claim, and it is why two lanes cannot silently take the same milestone: the
  board is `gh pr list --draft` and it costs one command. The claim and the deliverable are one
  object, so nothing has to be closed by hand, and a lane that dies leaves a visible stale draft
  instead of an invisible gap. The empty commit is what keeps that true: without it GitHub
  closes the draft as *merged* when the lane's base lands, and the board reads empty while a lane
  works. §90 has the four cases and why they hid. Check that board before briefing, alongside
  `git ls-remote --heads`.
- A developer works in a lane, and the lane is the isolation rather than the person: its own
  worktree, its own branch, one milestone, no visibility into the others. Two developers in one lane
  is the merge problem this vocabulary exists to prevent.
- Steward. Runs on an interval and holds a *lent* authority, which is what the name says: it
  merges what has earned it (green on every check, from a developer briefed this session, touching
  no syscall surface, no `design/decisions/` section and no dependency addition), cleans up behind
  finished work (delete the branch, prune the worktree, relink `nife-dev`), reports queue depth
  against the target, and raises what has stalled or gone unanswered. It exists because the
  maintainer is structurally bad at noticing its own idleness: when it is busy, it is busy.

  It does not brief developers, because briefing is judgment and the good outcomes come from
  briefs that name the specific hazard (the sixteen-slot cspace, the claim to verify, the file
  another lane holds). A generic brief produces a worse lane than an idle slot costs. So the steward
  says "the queue is at one of three and these are ready" and the maintainer writes it.

  It watches for work at risk, not only for idleness: a lane worktree with modifications and no
  commit in half an hour is uncommitted work one prune away from gone, which is the only failure in
  this system that destroys rather than delays. That check earns its keep more than the idle one.

  **It must never hold the main checkout while a developer's gate is running**, which is the race
  that took the `nife-dev` link out from under a lane on 2026-08-04. `caretaker` and `undertaker`
  were unavailable as names: this tree already spends both on capability-narrowing programs.

**The top-up rule, which is the whole point.** When a developer finishes, the maintainer **launches
the next work before writing the report**. Not after, and not when an architect next asks. A
conversation with an architect never blocks the queue; answering a question and keeping lanes full
are concurrent, and the failure mode is always the same, which is that the answer feels like
progress and the idle machine is invisible. Maintain the agreed number of concurrent developers, and
if the ready queue is empty, say so as its own finding rather than letting the silence stand for
"nothing to do".

**A developer's final report ends by handing off**: what its work unblocked, and what it found that
wants a lane of its own. That is the same discipline as that of milestone 94 (the untracked-work
sweep), applied to scheduling rather than to findings.
