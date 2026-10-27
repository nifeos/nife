---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-27
ratified_by: calef
---

# 174. Which caller each of the three uncalled instruments gets

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice B, which read milestone 341 (give the three instruments nothing runs a caller)'s
`DECISION` gate and found it naming no section. The finding is milestone 232's audit, and the
2026-09-03 proposal sweep carried it forward. Ruled 2026-10-27 (UTC).

## What was decided

Three instruments rendered a verdict and nothing called them. Each needs a caller: a `local` row in
`script/ci-build` (a developer waits for it before pushing), a `ci` row (only a runner waits), or an
entry in `script/cadence-check`'s schedule. The stake was never tidiness. `design/fatal-risks.md`'s
first risk stood GREEN on a hand-run instrument, which is a claim about whether this project should
continue resting on somebody having typed a command once.

## The ruling

| instrument | caller | state |
|---|---|---|
| `script/interleaving-check` | a `ci` job (`interleavings` in `ci.yml`) | settled in fact: in CI since b43a6c93b (milestone 746 (Loom runs in CI)) |
| `script/crate-probes` | a `ci`-tier row in `script/ci-build`, run on every pull request | owed: fix the failing crates first, then wire it |
| `script/rule-violations --check` | a `local` row in `script/ci-build` | owed: after a lane times it |

`crate-probes` comes first as repair. Every crate that fails is fixed so it builds alone. A crate
that is by design not meant to build outside the kernel is taken off the probe list, with the reason
written beside its entry, and its real dependencies are never deleted to make it pass. Only a green
instrument becomes a gate. It then runs on every pull request, not on a cadence: the cadence option
was an argument about effort, and the gate exists so that a regression is stopped, not noticed later.

`rule-violations --check` takes the `local` tier when a lane has timed it and the time is seconds.
If the time is not seconds, the lane reports the time and the placement returns to an architect.

Whether any of these checks is a required status check in the merge queue's ruleset is a different
question, [§97 (six gates run on every pull request)](0097-advisory-checks.md)'s, and a repository
ruleset change is the maintainer's to make.

## Costs, as milestone 232 (audit every check against two questions) measured them

| instrument | cost | verdict then |
|---|---|---|
| `script/interleaving-check` | 12.4 seconds, 26 loom harnesses | green |
| `script/crate-probes` | about 3 minutes, builds fifty crates | 43 of 50 passing |
| `script/rule-violations --check` | unpriced | green, 2 open strikes against a threshold of 3 |

## The precedent the table follows

A check is `ci` because of what it builds, not because of what it asserts. `fastpath-footprint`
builds two release kernels, `stack-frame-check` builds the kernel test binary twice, `coverage`
re-runs the whole host suite and `cpu-matrix` is five QEMU boots. `crate-probes` builds fifty crates,
which is the same argument. `interleaving-check` is cheaper than rows in the `local` tier but already
has its CI job. `rule-violations --check` reads text and counts strikes, which is why it is `local`
if the timing agrees.

## What this unblocks

Milestone 341, which carries this section as its decision dependency. The measuring and the wiring
are a lane's work.

## What this does not decide

The neighboring question of how a tier is spelled and what `script/ci-build` with no arguments
means, which is milestone 440 (what `script/ci-build` with no arguments should mean)'s. Nor does it decide required status checks (see above).
