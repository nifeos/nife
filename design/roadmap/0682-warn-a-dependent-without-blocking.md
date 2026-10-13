---
status: BUILT
raised: 2026-09-26
built: 2026-09-27
promoted_from: warn-a-dependent-without-blocking
---
# 682. Warn a dependent without blocking the supervisor

Status BUILT 2026-09-27: built on PR #1382. *(Title and slug are drafts.)*

Raised by the lane for milestone 23 (a capability-routed component
OS with live replacement), answering the block's open question of what a supervisor does when a
dependent it must warn before a swap does not answer. notes/non-cooperative-fallback.md has the
options, the prior art and the recommendation.

Then milestone 151 (notification objects: async multiplexing without
wait-any), which is what the build needs. The decision is whether a dependent's warning is advisory. The measurement that says it
can be is built: `swapper`'s `ROLE_UNWARNED` swaps a backend without ever warning `broker`, and the
producer loses nothing.

## Ruled

calef, 2026-09-26: "Make the warning advisory." Build it once milestone 151 lands (#1351).

## Built

2026-09-27, on PR #1382: the page, the bound signal, the late-warning test, and the stranded-operator
`BUGS` entry closed in notes/non-cooperative-fallback.md, where it was recorded.

## What to build

- Replace `broker`'s `BOP_DOWN`/`BOP_UP` `CALL`s with a read-only state page the supervisor writes
  and a notification bound to `broker`'s thread that it signals. The supervisor never waits.
- Keep `ROLE_UNWARNED`'s test, and add one where the signal lands after the swap has finished.
- Close notes/dependency-orchestration.md's `BUGS` entry: a dependent that does not answer no longer
  hangs `swapper`.

## What it unblocks

One of milestone 23's three remaining `Outstanding` lines.

## Follow-on

- **Done.** Closed one of milestone 23's three remaining `Outstanding` lines and the stranded-operator `BUGS` entry in `notes/non-cooperative-fallback.md`, on PR #1382.

## Index row

What a supervisor does when a dependent it must warn before a swap does not answer. Ruled by calef on 2026-09-26 (the warning is advisory) and built on PR #1382.
