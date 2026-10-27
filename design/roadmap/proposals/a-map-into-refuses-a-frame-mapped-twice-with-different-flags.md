---
status: PROPOSED
raised: 2026-10-27
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# A `MAP_INTO` refuses a frame mapped twice with different flags

The tenth outsider pass's lane raised this on 2026-10-27 (UTC) while it recorded milestone 936
(a tenth outsider pass attacks the confinement claim). calef ruled the same day that the escape
below sits on a shipped path and that the fix milestone is mandatory.

Reuse: the pass's witness test and the `MAP_INTO` checks that exist; no new dependency and no
vendor. Searched `design/roadmap/` for a prior alias fix and found none.

`AddressSpace::MAP_INTO` never refuses a frame the space already maps. `MAP_RW` at one `va` and
`MAP_CODE` at another leave every page-table leaf honest while the two aliases make the physical
page both writable and executable; this defeats W^X by alias (claim 11). A builder holding
`WRITE` on a space and a `READ | WRITE` frame reaches it with ordinary capabilities. The per-leaf
Kani proofs the row cites survive; the claim as published does not. The pass's note records the
falsification, and the replayable patch sits beside
`system_tests/src/user/tenth_pass_map_gate_tests.rs`.

The fix: `address_space_map_into`, or the mapping hold beneath it, refuses a second mapping of a
frame the space already maps when the flags would differ. One check covers every path that
installs a leaf, so the riscv64 and x86_64 instances come free with the shared code. The witness
drops its opt-out and joins the default suite green on all three architectures.

An architect rules the refusal's exact shape before the lane starts: any flag difference, or only
the writable-and-executable pair; and the check's home, the dispatcher or the hold.
