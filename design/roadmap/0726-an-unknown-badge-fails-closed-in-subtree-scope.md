---
status: BUILT
built: 2026-10-03
raised: 2026-10-03
promoted_from: an-unknown-badge-fails-closed-in-subtree-scope
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 726. An unknown badge fails closed in subtree_scope

Promoted and built 2026-10-03 (UTC) by the lane for this milestone, on calef's ruling of the same day: option 1, fail closed in the crate. The number 726 is provisional until the queue lands it (725 is taken by a running lane). *(Title and slug are drafts.)*

Raised 2026-10-03 (UTC) by the lane for milestone 633 (an outside agent attacks the confinement
claim), fatal risk 7's first outsider pass. It is calef's call because it changes what §230 (badged
endpoint capabilities) says a server does with a badge it has no window for.

## The finding

`crates/subtree_scope` is the one implementation of a subtree grant that an eligible file server
links, under milestone 606 (a directory walk costs what it does on Linux)'s ruling D. Its
`Bindings::<B>::of(badge)` maps a badge to what it may reach. Badge 0, the unbadged value, is
`Binding::Unbound`, the whole endpoint's authority, and that is load-bearing: caretakers hold such
badges. A nonzero badge at or past `B` is also mapped to `Binding::Unbound`, by `index` folding it to
0. The crate's own comment says this is deliberate, "what the server already does with such a
badge's window (§230)".

So a nonzero badge the server has no window for does not reach nothing. It reaches everything. That
is a fail-open default at a confinement boundary, and the only thing between it and an escape is
the spawner's badge allocation staying inside `B`. The kernel's `BADGE` method puts no ceiling on a
badge: any nonzero `u64` is accepted, and nothing in the kernel knows a server's `B`.

It is not reachable by a confined client today. `BADGE` refuses an already-badged source, so a
client handed a badged endpoint cannot mint a badge of its own choosing. The pass recorded it as a
hardening candidate, not an escape. It becomes an escape the day a spawner mints one badge past a
server's pool, or two servers with different pool sizes are handed badges from one allocator.

## The options

1. Fail closed in the crate. `Bindings::of` returns `Binding::Revoked` for a nonzero badge at
   or past `B`; badge 0 stays `Open`. Cost: one arm in `of`, one line in the Kani harness
   `a_badge_once_bound_is_never_open_again` (assert a badge past `B` is never `Open`), one host
   test. Behavior change: a request carrying a badge the server has no window for is refused
   `EBADF` rather than served with full authority. That is the point. Premise to check before
   building: no server deliberately sends a badge past its own `B` and expects `Open`; the crate
   doc says caretakers hold badge 0, so none should.
2. A kernel ceiling on `BADGE`. Refuse a `new_badge` above some limit. Lost on inspection: §230
   makes the badge a `u64` the holder chooses, servers have different pool sizes, and the kernel
   cannot know any of them. A single ceiling is the wrong knob in the wrong place.
3. Keep fail-open and record it. Add the sentence to the crate's `BUGS`. Lost: it leaves a
   confinement boundary at rung three of the mechanism ladder when rung one costs one arm.

## Recommendation

Option 1. It is one crate, one proof and one test, fully reversible, and nobody outside the crate
has acted on the fail-open: `Bindings::of` has one consumer, the server that links the crate. If
both options cost the same, option 1 is still the choice, because a boundary that does not know a
name should refuse it.

What waits on the answer: nothing in flight. The hardening is small enough for a lane the day it is
ruled.

## Follow-on

- **Done.** The crate arm, the Kani harness `a_badge_with_no_window_is_never_open` and its falsification, and the host test `a_badge_with_no_window_is_refused` landed in this milestone's pull request, with the amendment to DECISIONS §230 (badged endpoint capabilities) and the dated fact in `design/fatal-risks/README.md`.
- **Recorded.** The kernel's `BADGE` still has no ceiling, deliberately; `design/decisions/0230-badged-endpoints-name-a-callers-frame.md` carries the amendment.

## Index row

`subtree_scope` mapped a nonzero badge at or past its table size to the whole endpoint's authority. calef ruled 2026-10-03 (UTC) that it fails closed: such a badge is refused, and badge 0 stays the caretaker's.
