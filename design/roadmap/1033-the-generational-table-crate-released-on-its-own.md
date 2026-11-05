---
status: NOT-STARTED
raised: 2026-11-05
milestone_dependencies: 887
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1033. `generational_table`: the fixed-capacity, proved slotmap, released on its own

*(Number provisional until the integrator mints it. The title and slug are drafts. Milestone 820
(`extensible_host_controller_interface`: proven xHCI logic, released on its own) is the shape this
follows.)*

`generational_table` is the third crate in the ruled publication order (see the release order of milestone 857 (crates released on their own)).
The architect reviewed it and ruled publish. The crate is the fixed-capacity,
allocation-free, proved member of the slotmap family. Its README states how it differs and lists its
costs under `BUGS`. It points users who need growth to `slotmap` and `thunderdome`.

## What exists

- `crates/generational_table`: `Table<T, N>`, about 550 lines with tests and harnesses, no
  dependencies and no `unsafe`. Its mechanism is in [Generational names](../../notes/generational-names.md).
- Four Kani harnesses, each with a falsification patch under
  `crates/generational_table/falsifications/`: a removed name never resolves again, a name the table
  never minted resolves to nothing, live names are distinct and resolve to their own entry, and a
  bounded walk yields every live entry.
- The release record, [generational_table and the existing crates](../../notes/generational-table-and-existing-crates.md),
  answers whether a published crate already covers an allocation-free, fixed-capacity generational
  table. None does: `slotmap` needs an allocator and lets two `u64` names reach one entry, and
  `coca` is dormant and `unsafe`-heavy.

Reuse: none exists that fits. Searched crates.io and read nine candidates from source (`slotmap`, `thunderdome`, `coca` and six more); the release record gives each verdict.

## What remains

1. The move: the crate leaves this tree for `nifeos/generational_table` on the shared CI template of milestone 887 (crate repositories share one CI template).
   The crate moves and nife keeps no copy. Its README opens on what the crate is for and what it is not for.
2. The soak, which ends on evidence and not a duration, as in milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic, released on its own).
3. The publication of 0.1.0 by trusted publishing, which waits for the architect's approval.
4. nife consuming the release by semantic version, with `Cargo.lock` recording its checksum, the
   kernel's seven tables and `crates/memory_regions` among the consumers.

The crate takes the third slot, after `file_allocation_table` (milestone
813) and `extensible_host_controller_interface` (milestone 820).

## BUGS

- Insert scans first-fit in O(N). The README states it.
- `[Option<T>; N]` spends a tag per slot on any `T` without a niche. The README states it.
- A slot reused 2^32 times resolves an ancient name again. The README states it.
- Whether the public name type stays a bare `u64` or becomes a typed key is open. A newtype is the
  stricter choice, since widening later is compatible and narrowing is not.
- Measured demand for fixed-capacity tables is close to nothing, so the crate may find few users.
  Whether the README says so is open.

## Index row

nife's generational table of named slots, its four Kani proofs and their falsification patches,
published on crates.io as `generational_table` 0.1 from its own `nifeos` repository as the
fixed-capacity, allocation-free member of the slotmap family, then consumed back by nife from that release.
