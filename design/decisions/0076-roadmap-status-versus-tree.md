---
status: DECIDED
raised: 2026-08-04
decided: 2026-08-25
ratified_by: calef
---

# 76. What catches a milestone status that is wrong in both places?

calef, 2026-08-25, in conversation, ratifying the recommendation below as
written: *"Ratify it."* (raised 2026-08-04 after nine milestones were found misrecorded in one
sweep.)

What. `script/roadmap --check` verifies that `design/roadmap/README.md` and the milestone's own file
agree. It catches the split-brain defect. **It cannot see a status that is wrong in both places**,
because the tree is a third record and nothing compares against it.

The cost, measured. On 2026-08-04 nine milestones (81, 82, 85, 94, 96, 100, 101, 109, 110, 113)
had merged implementation PRs while both records read `NOT-STARTED`. `script/roadmap --ready` is the
list work is picked from, so it offered finished work: a developer spent its entire budget re-running
mutation tests for milestone 85, which had merged hours earlier. The gate was green throughout.

Two candidate signals, both measured rather than assumed.

- A `milestone N:` commit subject on `main`. Purely local, no network, cheap. Recall is poor:
  23 milestones have such a commit and 35 of 54 BUILT milestones do not, because the work often lands
  under a descriptive subject and the merge commit carries the PR title. It would have caught none of
  the nine.
- A merged PR titled `milestone N...`. Caught all nine. Needs `gh` and the network, so it
  cannot live in `script/lint`; it would be a CI-only check, or a periodic one the steward runs.

The recommendation. A CI-only check, because the signal that works needs the network and CI has
it: fail (or warn) when a milestone reads `NOT-STARTED` and a merged PR names it in the
implementation form. Pair it with the convention that the integrator sets status in the merge,
but do not rely on the convention alone, since milestone 92's own argument is that a practice living
in memory gets skipped exactly when it matters, and this one was skipped nine times in one day.

The argument against. Both signals key on a title convention that nothing enforces, so the
check is only as good as how people write PR titles, and a lane that titles its PR differently is
invisible to it. That is a real weakness and it is the same class of weakness as
`script/decisions --check`'s: it verifies a citation resolves to *some* section, never the right one.
A check that is right most of the time is still worth more than the nothing that exists now, but it
should be honest in its own output about what it cannot see.

**Reframed 2026-08-25: a backstop now, not the primary defense.** A practice has emerged in this
project since 2026-08-04, discussed with calef before ratification: a lane updates its own
milestone's roadmap status as part of the same PR that does the work, rather than as a separate
step landing later. That is closer to `AGENTS.md`'s own rung 1 ("make the wrong state
unrepresentable") than to this decision's own rung-2 check ("a gate that fails loudly"), because the
status update is bundled into the very commit it describes rather than a fact someone has to
remember to go add. It directly attacks the shape of the original failure: nine milestones sat
`NOT-STARTED` because the update was a forgettable separate step, and a step that cannot be separated
cannot be forgotten the same way.

That closes most of the acute failure mode this decision was written against, but not all of it, and
three real leaks survive the newer practice: a lane can still be briefed and simply not do the status
update, or update the wrong milestone's file; work landing outside the "brief a lane" shape (a direct
maintainer fix, an outside contributor, a dependabot-generated pull request) never goes through this
treatment at all; and the check's own already-named weakness stands regardless of who lands the
work, since it keys on a PR-title convention nothing enforces.

So the CI-only check stays the right call, ratified as designed above, but its job changes: it is no
longer the thing standing between this tree and a repeat of nine-in-one-day, because the newer
practice is already doing most of that work at a stronger rung. It is now the cheap backstop behind
that practice, catching the residual cases where a lane skips the bundled update or the work never
went through a lane at all. **Not yet built.** Ratifying the decision does not itself land the check;
whoever implements it should read this section before writing the check's own output message, since
the honest framing for a user hitting it now is "this slipped past the practice that usually catches
it," not "nothing else is watching."

Not otherwise blocked. The nine original records are corrected; nothing else waits on this.
