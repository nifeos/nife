# Prior art

An appendix to [the milestone](../1101-memory-regions-outside-review.md).

How the crate sits against what a reader of other systems already knows. Rows marked unverified
are from memory and a lane confirms the wording before a decision cites them.

| system | the same problem | how it is solved there | where this crate differs |
|---|---|---|---|
| seL4 `Untyped` | carve objects and sub-untypeds out of a run of memory, reclaim it whole | a free index (watermark) per untyped, children recorded in the capability derivation tree, `Retype` checks every destination slot before it moves the index, `Revoke` tears the children down and resets the index (unverified detail) | the same bump watermark and the same "a refused retype moves nothing" rule, except on a full table (item 1); the parent link lives in the child and a return is LIFO plus coalescing; a claim type instead of a CDT |
| Linux `page_counter`, the `pids` cgroup controller | charge one object against a group and every ancestor, refuse at any ceiling | `page_counter_try_charge` walks the ancestors charging each and uncharges back down on the first failure (unverified detail); `pids.max` is set per group by its parent | two walks under one borrow, decide then commit, so a refusal writes nothing; a child's ceiling is a fixed sixteenth of the root's rather than a value its parent sets (Decision 4) |
| Zircon | a job tree with per-job policies and kernel-object limits | policies are per job and inherited; resources are ranges with exclusive children (unverified detail) | no policy object; the ceiling is a count per kind, carried in the region |
| Barrelfish, Composite | retype memory capabilities, hierarchical revocation | capability trees with explicit descendants (unverified detail) | no descendant list: `return_to_parent` scans the table for the parent's live children |
| `generational_table` (the dependency), `slotmap`, `thunderdome` | a name that dies with what it named | a typed key the table checks on every use | this crate takes and returns the name as a bare `u64` at its own boundary (Decision 2) |
| `bumpalo`, `typed-arena` | a bump allocator with a reset | one arena, reset all at once, no children | a tree of arenas, each a child carved from a parent's run, returned LIFO or coalesced |

Where this crate differs from all of them. The decision over the table is a pure function Kani
proves. The single-winner claim is a value that a caller cannot clone or build. Loom searches the
racing protocol with a falsification witness that must still find the old double free. Every harness
ships with the patch that turns it red. What it costs: a `u64` name at the boundary, a scan of the
table on every return and on every `spent`, kernel-specific object kinds, and a hole under a live
child that is not reused while the child lives.
