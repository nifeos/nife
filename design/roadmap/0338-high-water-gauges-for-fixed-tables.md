---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 338. Every fixed-size table but one is raised only after it has failed silently

Filed 2026-09-03 as an unnumbered proposal by the milestone 247 sweep,
from milestone 231's block; numbered 2026-09-19 by milestone 433. **Premise re-checked 2026-09-19 and
it holds.** `kernel/src/cap.rs` still carries the only gauge of its kind
(`CAPABILITY_TABLE_PEAK_MEASURED`, `PEAK_REPORTED`, `PEAK_STABLE_PASSES`),
`kernel/src/sched/thread_table.rs` still carries `PEAK_THREADS` as a separate mechanism reported in a
different place, and `kernel/src/memory.rs`'s `MAX_REGIONS` and `crates/nifefs`'s `NAME_LEN` still
have no gauge at all. `CAPABILITY_TABLE_SLOTS` in `kernel/src/capability_object.rs` names those two as the tree's own precedent for the failure being
described.

The pattern exists and works. Milestone 231 built the gauge for
`CAPABILITY_TABLE_SLOTS`, `sched::MAX_THREADS` has an older separate one, and the remaining tables
need either the same treatment or a decision that one mechanism serves all of them. Either route is
startable today.

**In brief.** `CAPABILITY_TABLE_SLOTS` now reports `capability slots: 21 of 24 at peak` on every
boot, and `script/swish-check` fails if the kernel flagged a boot as having gone past the peak
recorded beside the constant. No other fixed-size table has that. `MAX_REGIONS` and
`nifefs::NAME_LEN` have no gauge at all. `sched::MAX_THREADS` has its own `PEAK_THREADS`, built
separately and reported differently. So the shape is being solved once per table by hand.

## Why this matters

The argument for the first gauge applies unchanged to the rest, and milestone 231's block makes it
in the strongest available form: `CAPABILITY_TABLE_SLOTS` was raised three times and **every raise
was reactive, after a silent failure that named something else**. Milestone 230 is the worked
example, and it cost a bisect: a virtio-rng attached, init trapping at `user_mode_runtime::trap` with no
message, and the cause was slot exhaustion in `crates/system_initializer`. Nothing about the failure
said "table full".

Every ungauged constant still has that property. A table that fills produces a failure somewhere
downstream, in whatever code first cannot get a slot, and the distance between the cause and the
symptom is the whole cost. The gauge closes it by making the approach visible before the wall is
hit, which is the difference between a number on a boot line and a bisect.

The second reason is the one that makes this a proposal rather than three copies of a patch.
Solving the same shape once per table by hand is how `sched::MAX_THREADS` ended up with a mechanism
that does the same job under a different name and prints in a different place. A reader now has to
know both. One more hand-rolled gauge makes three.

## The fork inside it, which is small

Whether to generalize or to repeat is worth deciding rather than defaulting into. A shared mechanism
means one place to read, one output format, and one check in `script/swish-check`. It costs an
abstraction over constants that differ in kind, since `nifefs::NAME_LEN` is a length bound on a name
and not a count of live objects. Forcing those into one gauge may be the wrong shape. Repeating
costs a reader a new name per table. A lane can settle this by looking at what the three remaining
constants actually bound, and should say which it chose and why.

## Where it came from

Milestone 231 (nothing counts how many capability slots a boot actually uses) named it: *"Build the
same high-water gauge for the other fixed-size tables, or decide that one mechanism should serve all
of them. `MAX_REGIONS` and `nifefs::NAME_LEN` have no gauge at all and `sched::MAX_THREADS` has its
own separate `PEAK_THREADS`, so the shape is being solved once per table by hand and every ungauged
constant is still raised only after it has failed silently."*

## Index row

`CAPABILITY_TABLE_SLOTS` reports `capability slots: 21 of 24 at peak` on every boot and
`script/swish-check` fails if the kernel flagged a boot as having gone past the peak recorded beside
the constant. No other fixed-size table has that. The argument for the first gauge applies unchanged.
`CAPABILITY_TABLE_SLOTS` was raised three times and every raise was reactive, after a silent failure
that named something else. Milestone 230 (`script/shell-check` is red on `main`, on both architectures) is the worked example that cost a bisect over a
virtio-rng attaching and init trapping with no message. Every ungauged constant still has that
property, because a table that fills produces a failure downstream in whatever code first cannot get
a slot, and the distance between cause and symptom is the whole cost. The second reason makes this
one piece of work rather than three copies of a patch. Solving the shape by hand per table is how
`sched::MAX_THREADS` ended up with a mechanism doing the same job under a different name, printed
somewhere else. One more makes three. The fork inside it is small and a lane can settle it by
looking at what the remaining constants actually bound.
