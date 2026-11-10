---
status: BUILT
promoted_from: a-push-to-a-queued-pull-request-re-arms-it
raised: 2026-11-04
built: 2026-11-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1052. A push to a queued pull request re-arms it, and arming refuses a held one

COE action item: notes/coes/2026-11-03-merge-queue-backlog.md, action 7.

Raised by lane/coe-merge-queue-2026-11-03 on 2026-11-04 (UTC), from the correction of error for the
Pacific day of 2026-11-03 (`notes/coes/2026-11-03-merge-queue-backlog.md`).

Reuse: `helpers/push-queued-check.sh`, `script/push`, and the hold labels `helpers/merge-drain.sh`
already reads (`needs-architect`, `held-by-lane`, `Blocked-by:`).

## Background

The pre-push hook refuses a push to a queued pull request. On 2026-11-03 the session account
dequeued 21 entries by hand, 13 of them within 5 minutes of a push, and at 06:00 UTC on 11-04 two
clean pull requests (#3120, #3122) sat with auto-merge off after a push. Separately, the session
account enqueued #3186 three times while it carried `needs-architect`, and the drain dequeued it each
time, discarding three group builds.

## What exists

1. Re-arming push. `script/push --requeue` runs the pre-push hook first, then
   `helpers/push-requeue.sh before` dequeues the pull request (`dequeuePullRequest`, read back), the
   push lands and `script/push` reads it back from origin, and `helpers/push-requeue.sh after` arms it through
   `helpers/arm.sh`. A pull request that was not armed stays unarmed. Without the flag
   `helpers/push-queued-check.sh` keeps refusing.
2. One arming entry point. `helpers/arm.sh <n>` refuses a draft, a closed pull request and one
   wearing a hold label, naming the label, then runs `gh pr merge --auto --merge` and reads GitHub
   back (`helpers/pr-queue-state.sh`). The holds are one definition, `helpers/hold-labels.sh`
   (`hold_label_of`), which `helpers/merge-drain.sh` sources too. It also refuses a pull request
   that conflicts with the queue, milestone 1054 (an entry that conflicts with the queue is caught
   before it).
3. The drain reports an unarmed pull request. The `unarmed` cause (`nm_unarmed` in
   `helpers/needs-maintainer.jq`) labels a ready, unheld pull request (`nm_ready_unheld`) that is
   neither armed nor queued for 30 minutes (`NM_MINUTES` in `helpers/merge-drain.sh`), once per
   episode, and arms nothing.
4. The instructions name the entry point. The developer-lane skill, `CONTRIBUTING.md`,
   `briefs/gate-a-lane.md`, `briefs/gate-in-ci.md`, `briefs/run-a-milestone-wide.md` and
   `briefs/session-start.md` say `helpers/arm.sh <N>`, and the skill says `script/push --requeue`
   for a push to an armed or queued pull request. The refusal in `helpers/push-queued-check.sh`
   names `script/push --requeue`, and the `unarmed` and `ejected` comments `helpers/merge-drain.sh`
   posts name `helpers/arm.sh`. `gh pr merge --auto` by hand stays possible.

`helpers/push-requeue-selftest.sh`, run by `script/lint`, drives `script/push --requeue` and
`helpers/arm.sh` against a stub `gh` and `git`. It checks dequeue, push and arm in that order, a
no-op re-arm that must fail, every hold refused with its label named and no merge called, and that
`nm_held` in `helpers/needs-maintainer.jq` lists the same three holds. Its case 11 fails when a file
under `briefs/` or `notes/skills/`, `CONTRIBUTING.md`, `helpers/push-queued-check.sh` or
`helpers/merge-drain.sh` names `gh pr merge ... --auto`. It cuts out the body of the drain's
`requeue_stuck_head` first, which is code, the one re-queue ruled on #1833, and not an instruction.
A bare arm planted in a brief must trip it. `helpers/needs-maintainer-selftest.sh`, run by
`script/lint-fleet`, covers the `unarmed` cause, and its section 5 refuses a `helpers/arm.sh` call
in the drain as it refuses `gh pr merge`.

## Options considered

- Let the drain re-arm again. Refused: it fought the account's own dequeues before, which is why
  those rulings took arming away from it.
- Allow a push to a queued branch and let GitHub drop the entry. Refused: GitHub drops the entry
  silently and cancels auto-merge with it.

## Done when

- `script/push --requeue` on a queued fixture branch leaves it armed at the pushed head. Met: case 1
  of `helpers/push-requeue-selftest.sh`.
- Arming a pull request with `needs-architect` fails with the label named. Met: case 3 of the same
  selftest.
- No brief, skill, hook message or drain comment tells a person to arm with `gh pr merge --auto`; a
  grep of `briefs/`, `notes/skills/`, `helpers/push-queued-check.sh` and `helpers/merge-drain.sh`
  for `gh pr merge .*--auto` finds nothing outside `helpers/arm.sh`. The one match left is code, not
  an instruction: the drain's own call in `requeue_stuck_head`. Met: case 11 of the same selftest.

## Follow-on

- **Done.** The conflict refusal in `helpers/arm.sh` that the instructions now lead a person
  through is milestone 1054 (an entry that conflicts with the queue is caught before it), built in
  the same pull request.

## Index row

`script/push --requeue` re-arms a queued pull request, `helpers/arm.sh` refuses a held one, and the briefs, the skill, the hook's refusal and the drain's comments name those two instead of the bare `gh pr merge --auto`.
