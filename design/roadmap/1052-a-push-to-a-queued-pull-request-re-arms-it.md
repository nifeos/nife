---
status: PARTIAL
promoted_from: a-push-to-a-queued-pull-request-re-arms-it
raised: 2026-11-04
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

1. **Re-arming push.** `script/push --requeue` runs the pre-push hook first, then
   `helpers/push-requeue.sh before` dequeues the pull request (`dequeuePullRequest`, read back), the
   push lands and is read back from origin, and `helpers/push-requeue.sh after` arms it through
   `helpers/arm.sh`. A pull request that was not armed stays unarmed. Without the flag
   `helpers/push-queued-check.sh` keeps refusing.
2. **One arming entry point.** `helpers/arm.sh <n>` refuses a draft, a closed pull request and one
   wearing a hold label, naming the label, then runs `gh pr merge --auto --merge` and reads GitHub
   back (`helpers/pr-queue-state.sh`). The holds are one definition, `helpers/hold-labels.sh`
   (`hold_label_of`), which `helpers/merge-drain.sh` sources too. `script/push --requeue` and
   `briefs/gate-in-ci.md` use it.
3. **The drain reports an unarmed pull request.** The `unarmed` cause (`nm_unarmed` in
   `helpers/needs-maintainer.jq`) labels a ready, unheld pull request (`nm_ready_unheld`) that is
   neither armed nor queued for 30 minutes (`NM_MINUTES` in `helpers/merge-drain.sh`), once per
   episode, and arms nothing.

`helpers/push-requeue-selftest.sh`, run by `script/lint`, drives `script/push --requeue` and
`helpers/arm.sh` against a stub `gh` and `git`. It checks dequeue, push and arm in that order, a
no-op re-arm that must fail, every hold refused with its label named and no merge called, and that
`nm_held` in `helpers/needs-maintainer.jq` lists the same three holds.
`helpers/needs-maintainer-selftest.sh`, run by `script/lint-fleet`, covers the `unarmed` cause.

## What remains

1. **The instructions still name the bare command.** The briefs, the developer-lane skill and the
   two texts a person reads at the moment of arming still say `gh pr merge --auto` where they should
   say `helpers/arm.sh` or `script/push --requeue`:
   - `notes/skills/developer-lane/SKILL.md` (the ready-and-arm bullet);
   - `briefs/gate-a-lane.md`, `briefs/run-a-milestone-wide.md` (dispatch and the arming paragraph)
     and `briefs/session-start.md` (re-arming after a flake);
   - the hook's refusal in `helpers/push-queued-check.sh`, which prints dequeue, push and
     `gh pr merge` by hand instead of `script/push --requeue`;
   - the `unarmed` and `ejected` comments `helpers/merge-drain.sh` posts.

   `gh pr merge --auto` by hand stays possible. The header of `helpers/hold-labels.sh` also names
   `helpers/arm-selftest.sh`, which does not exist; the agreement check it describes is case 9 of
   `helpers/push-requeue-selftest.sh`.

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
  for `gh pr merge .*--auto` finds nothing outside `helpers/arm.sh`.

## Follow-on

- **Outstanding.** The instruction texts listed under "What remains" still name `gh pr merge
  --auto`; checked 2026-11-09 (UTC) by a grep of `briefs/`, `notes/skills/` and `helpers/`. The
  re-arming push, the arming entry point and the drain's `unarmed` report are built, and
  `helpers/push-requeue-selftest.sh` passed on that date.

## Index row

`script/push --requeue` re-arms a queued pull request and `helpers/arm.sh` refuses a held one, but the briefs, the skill, the hook's refusal and the drain's comments still name the bare `gh pr merge --auto`. What remains points each of them at the entry point.
