---
status: PROPOSED
raised: 2026-09-19
---

# 174. Which caller each of the three uncalled instruments gets

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice B, which read milestone 341 (give the three instruments nothing runs a caller)'s
`DECISION` gate and found it naming no section. The finding is milestone 232's audit, and the
2026-09-03 proposal sweep carried it forward. *(Section number provisional until the merge queue
lands it.)*

## State of the first instrument

`script/interleaving-check` has run on every pull request since commit `b43a6c93b` (2026-10-03,
milestone 746 (Loom runs in CI)): the `interleavings` job in `.github/workflows/ci.yml`. That settles its caller. The
other two instruments, `script/crate-probes` and `script/rule-violations --check`, are still
uncalled and this section stays open for them.

## What is being decided

Three instruments in this tree render a verdict and nothing calls them. Each needs a caller, and the
caller is one of three: a `local` row in `script/ci-build` (a developer waits for it before
pushing), a `ci` row (only a runner waits), or an entry in `script/cadence-check`'s schedule.

**The stake is not tidiness.** `design/fatal-risks.md`'s first risk stands GREEN on a hand-run
instrument, which is a claim about whether this project should continue resting on somebody having
typed a command once.

## The tree as it stands, read rather than recalled

`script/ci-build` is the single enumeration since milestone 286 retired `script/gates`. It carries
17 rows, 9 `local` and 8 `ci`, `name|tier|command`, with the reason each `ci` row is not `local`
written beside the tier it explains. None of the three instruments is in it, in any workflow under
`.github/workflows/`, or in `script/cadence-check`'s schedule, so all three still answer only when a
person types them. `notes/check-inventory.md` lists each with `nothing` in its caller column.

The costs, as milestone 232 measured them:

| instrument | cost | last verdict |
|---|---|---|
| `script/interleaving-check` | **12.4 seconds**, 26 loom harnesses | green |
| `script/crate-probes` | **about 3 minutes**, builds fifty crates | 43 of 50 passing |
| `script/rule-violations --check` | **unpriced**; `notes/check-inventory.md` records no timing | green, 2 open strikes against a threshold of 3 |

The third is the one nobody measured, and that is worth saying out loud, because this decision is
about cost and one of its three inputs is missing.

## What this tree already does in the analogous case

The `ci` rows' own comments are the precedent and they are all the same shape: a check is `ci`
because of what it builds, not because of what it asserts. `fastpath-footprint` builds two
release kernels *"so there is nothing warm to share and a developer would wait for a cold build"*;
`stack-frame-check` builds the kernel test binary twice; `coverage` instruments and re-runs the whole
host suite; `cpu-matrix` is five QEMU boots.

Read against that, the three sort themselves:

- `interleaving-check` is twelve seconds of host compilation and asserts on `loom`. It is cheaper
  than rows already in the `local` tier.
- `crate-probes` builds fifty crates, which is the `coverage` and `cpu-matrix` argument exactly.
- `rule-violations --check` reads text and counts strikes, and nothing about it suggests a build.

## The neighboring decision, which is open in the same sweep

`script/ci-build`'s own header says, in capitals, that **the tier tags and the no-argument meaning
are PROVISIONAL pending calef**, and points at milestone 440 (*What `script/ci-build` with no
arguments should mean*). That milestone is also one of milestone 435's 45 blocks and is held by
another slice. 400 decides how a tier is *spelled* and what no arguments *defaults to*; this
decides which tier three specific checks take. They are separable and they are cheaper to answer
together, since both are read off the same table.

## The options, per instrument

| instrument | A | B | C |
|---|---|---|---|
| `interleaving-check` | **`local`** | `ci` | cadence |
| `crate-probes` | `local` (adds ~3 min to every pre-push run) | `ci` (adds a job to every pull request) | **cadence** |
| `rule-violations --check` | **`local`** | `ci` | cadence |

Recommendation: `local`, cadence, `local`, and the reason is the `ci` rows' own rule rather than
a preference. `interleaving-check` is cheaper than checks already in the local tier and guards a
fatal risk, so it earns the strongest caller available. `crate-probes` is a fifty-crate build whose
answer changes slowly, which is what a cadence entry is for and what milestone 232 declined to
assume CI was. `rule-violations --check` is a text scan; if measuring it shows otherwise, it moves
to the row above.

This is a reversible fork and the recommendation is offered on that basis. A tier is a word in a
table and a cadence entry is a line in a schedule; either can be changed in one commit by whoever
finds it wrong. What is not reversible is leaving a fatal risk resting on somebody's memory, which
is the standing cost of not answering.

And question 7, answered out loud: yes, this recommendation would stand if all three options
cost the same to implement. That is because the argument is about what a developer should have to
wait for rather than about what is easier to wire. All three placements are a single line.

## What is blocked until this is answered

Milestone 341. The measuring and the wiring are a lane's work; the placement is not, because two
of the three are expensive enough that "run it in CI" changes what a pull request costs.

## What this does not decide

Nothing about whether these checks should be required in the merge queue's ruleset, which is [§97
(six gates run on every pull request)](0097-advisory-checks.md)'s question and a different one. §97
asks whether a red check may stop a merge, and this asks whether anything runs the check at all.
