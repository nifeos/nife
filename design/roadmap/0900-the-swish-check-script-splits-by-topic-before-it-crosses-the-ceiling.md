---
status: NOT-STARTED
raised: 2026-10-12
promoted_from: the-swish-check-script-splits-by-topic-before-it-crosses-the-ceiling
milestone_dependencies: 847
decision_dependencies: 266, 280
machine_requirements: none
specific_machine: none
needs_person: no
---
# 900. The swish-check script splits by topic before it crosses the ceiling

*(The title and slug are provisional.)*

Milestone 847 (the swish-check harness is split along its seams) gave the script table its own
module.

`xtask/src/swish_check/script.rs` holds the rows `script/swish-check` types. It starts at 1,287
lines. `SWISH_CHECK_SCRIPT` alone was 405 lines on 2026-09-23. It was 1,004 on 2026-10-04 and 1,099
on 2026-10-11. At last week's rate the file reaches the ceiling in about eight weeks. At the 18-day
rate it takes three. The ceiling is §266 (a Rust source file stays under 2,000 lines).

The work is to cut the table into topic files under `script/`. Candidates are pipes and files,
navigation, packages, the network and `std` programs. The leg still types one slice, assembled
from them in the same order, so no transcript changes. §280 (the swish-check harness splits into
submodules, with its script in one) gave the table its own module for this. The split stays
inside `script/`.

Assembling one `&[Line]` from several `const` slices is the only new code. How to do it is this
milestone's choice.

Reuse: not applicable; this moves rows and adds only the assembly.

Done when every file under `script/` is under 1,500 lines. `script/swish-check` must pass on
aarch64, riscv64 and x86_64, typing the same lines in the same order.

## Index row

The swish-check script table is cut into topic files before it reaches the 2,000-line ceiling,
assembled into the same slice so no transcript changes.
