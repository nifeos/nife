---
status: DECIDED
raised: 2026-09-24
decided: 2026-10-07
ratified_by: calef
---

# 257. A required review that the App would give is refused

*Raised for milestone 588 (a merge needs a review no fork can supply).*

## The question

Should the `main` ruleset set `required_approving_review_count: 1`, with `nife-smelter[bot]`
approving each lane's pull request before the merge drain arms it? This was option 1 of the
proposal milestone 588 was promoted from, raised 2026-09-24 after a security audit found that a
merge needs no review and a merge-group build runs a pull request's own workflow edits with the
repository's secrets.

## The ruling

Refused. calef, 2026-10-07 (UTC): *"Refuse the App-given required review and close 588 as BUILT."*

## Why

- The two protections that matter are already live. Option 2 (workflow approval for every outside
  contributor) and option 3 (the App's secrets behind a `main`-only environment) were adopted
  2026-09-24. Between them a fork's code cannot run with a secret, whatever the drain does.
- Option 1's review would be a rubber stamp by construction. The App approves whatever passes the
  admission predicate, so the approval carries no reading.
- The record would lie. Every pull request page would say "approved by nife-smelter" on work
  nobody read, and a reader would take that for a review. A false record is worse than an empty
  field, because the empty field is honest.

## When to revisit

When a second human holds merge rights, which is likely as the repository split brings
contributors. A person's review then means something, and a required count of one stops being a
stamp. Until then the count stays 0.
