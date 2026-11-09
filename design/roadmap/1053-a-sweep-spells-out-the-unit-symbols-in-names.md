---
status: NOT-STARTED
promoted_from: a-sweep-spells-out-the-unit-symbols-in-names
raised: 2026-11-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1053. A sweep spells out the unit symbols in names, and the unit ratchet counts every unit

Raised by lane/record-follow-on-items.

Reuse: `helpers/vendored_name_gates.py`'s unit ratchet and its `--unclear` listing, and the
template's `[units]` table. No new tool.

## Background

design/naming.md spells every unit out (`_HERTZ`, `_MILLISECONDS`, `_NANOSECONDS`) and says a
separate sweep renames the existing names that use a symbol, about 95 of them. The unit ratchet
counts only the units in `UNIT_COUNTED` (`nanos`, `millis`, `micros`, `us`, `sec` and their
plurals), whose count is 0. The template's table also refuses `ns`, `ms`, `hz` and the byte and
bit-rate symbols, which the ratchet does not count yet: `vendored_name_gates.py unit --unclear`
lists 162 distinct identifiers that carry one, uses included.

## The work

1. Rename the names that carry `ns`, `ms`, `us` or `hz` in a symbol's place, by family, with a
   `#[doc(alias)]` only for a specification's term. A ratified name keeps its spelling until its own
   name review, and the lane lists those for an architect rather than renaming them.
2. Add those units to `UNIT_COUNTED` in the change that brings their count to 0, so the ratchet
   holds the result.
3. Fix `respell`, which still writes `nanos` as `ns` and `millis` as `ms`.
4. Do the same for the byte and bit-rate symbols (`kib`, `mib`, `gib`, `mbps`).

## Done when

- `vendored_name_gates.py unit --unclear` lists only ratified names and the kept identifiers, each
  with its reason.
- `UNIT_COUNTED` holds every unit of the template's table, at a count of 0.
- design/naming.md no longer says a sweep remains.

## Index row

Names that use a unit symbol survive because the unit ratchet counts only some units. This sweeps the symbols out of existing names and has the ratchet count every unit the template refuses.
