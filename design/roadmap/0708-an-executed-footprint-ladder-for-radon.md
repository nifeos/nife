---
status: NOT-STARTED
raised: 2026-10-03
promoted_from: an-executed-footprint-ladder-for-radon
milestone_dependencies: 370, 74
decision_dependencies: none
machine_requirements: riscv64 silicon; PMU cycle counter
specific_machine: radon (the 32 KiB L1i the 4 KiB target is derived from, and E3's baseline readings)
needs_person: yes
---
# 708. An executed footprint ladder for radon

*Title and slug are drafts.*
Title and stem provisional.

Whether the 4 KiB fastpath target is right waits on an experiment nobody can run yet. E3's padding
is never executed, so even under milestone 370 (a layout control) it tests counted bytes and code
displacement, not Liedtke's executed footprint. The experiment that does is E5, planned in
[`notes/footprint-perturbation/executed-footprint.md`](../../notes/footprint-perturbation/executed-footprint.md),
which says what each outcome means before any number exists.

The build, all of it in that note's "What exists, and what does not":

1. An executed jump chain in a fixed region pinned first in `.text`, sized by two build-time
   variables, with a dense twin per rung so instruction count is held equal.
2. A code working set beside E4's data one in `kernel/src/bench.rs`.
3. `script/fastpath-footprint --layout` checking the region across images.
4. Optional: an L1I miss event in `kernel/src/arch/riscv64/pmu.rs`, behind a probe line.

On both ISAs, per DECISIONS §19 (architectural parity). The person is needed only for the evening
that follows, about 100 minutes on radon.

## Index row

Whether the 4 KiB fastpath target is right waits on an experiment that executes its padding, which the existing layout control does not. Proposed: build E5, an executed jump chain and a code working set, as planned in `notes/footprint-perturbation/executed-footprint.md`.
