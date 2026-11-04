---
status: PROPOSED
raised: 2026-11-04
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A push to a queued pull request re-arms it, and arming refuses a held one

COE action item: notes/coes/2026-11-03-merge-queue-backlog.md, action 7.

Raised by lane/coe-merge-queue-2026-11-03 on 2026-11-04 (UTC), from the correction of error for the
Pacific day of 2026-11-03 (`notes/coes/2026-11-03-merge-queue-backlog.md`). The integrator mints
the number at promotion.

Reuse: `helpers/push-queued-check.sh`, `script/push`, and the hold labels `helpers/merge-drain.sh`
already reads (`needs-architect`, `held-by-lane`, `Blocked-by:`).

## Background

The pre-push hook refuses a push to a queued pull request and prints three commands for a person to
run: dequeue, push, re-arm. On 2026-11-03 the session account dequeued 21 entries by hand, 13 of
them within 5 minutes of a push. Re-arming was prompt in that window, but at 06:00 UTC on 11-04 two
clean pull requests (#3120, #3122) sat with auto-merge off after a push. Separately, the session
account enqueued #3186 three times while it carried `needs-architect`, and the drain dequeued it each
time, discarding three group builds.

## What it builds

1. `script/push --requeue` dequeues the pull request, pushes, confirms the remote head as it does
   now, and arms auto-merge at the new head. Without the flag the hook keeps refusing, so a push that
   should not re-arm stays deliberate.
2. One arming entry point, used by `script/push`, the briefs and the maintainer, that refuses a pull
   request carrying a hold label and says which label. `gh pr merge --auto` by hand stays possible,
   and the briefs stop naming it.
3. The drain reports, once per head, a clean pull request with no hold and no auto-merge for more
   than 30 minutes. It does not arm it: arming stays with the account, under the 2026-10-03 rulings.

## Options considered

- Let the drain re-arm again. Refused: it fought the account's own dequeues before, which is why
  those rulings took arming away from it.
- Allow a push to a queued branch and let GitHub drop the entry. Refused: GitHub drops the entry
  silently and cancels auto-merge with it.

## Done when

- `script/push --requeue` on a queued fixture branch leaves it armed at the pushed head.
- Arming a pull request with `needs-architect` fails with the label named.
