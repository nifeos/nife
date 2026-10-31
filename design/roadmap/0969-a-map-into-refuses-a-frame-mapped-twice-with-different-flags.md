---
status: NOT-STARTED
promoted_from: a-map-into-refuses-a-frame-mapped-twice-with-different-flags
raised: 2026-10-27
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 969. A `MAP_INTO` refuses a frame mapped twice with different flags

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

The fix lives in the mapping hold, in `MappingHold::record_mapping` (ruled 2026-10-31 UTC). It refuses a second mapping of a frame the space already maps only when the frame would be
writable through one mapping and executable through another (ruled 2026-10-31 UTC). Other
differences in flags, such as read-only beside read-write, pass. `record_mapping` is
the one function every path reaches: `MAP_INTO`, `PageFrame::MAP`, `MemoryRegion::MAP` and the
kernel's loaders. Every path files its record before the page-table entry goes in, as
`MappingHold::retype_table` already does, and forgets the record if the map fails, so no path
installs a recorded leaf without the check. A second witness maps a frame as code through
`MAP_INTO` and then writable through `PageFrame::MAP`, the order a dispatcher-only check would
miss. The check sees one address space at a time. All three ISAs share the code, so the
riscv64 and x86_64 instances come with it. The witness drops its opt-in and joins the default
suite green on all three architectures.

A check inside one space leaves a second alias open: the same frame writable in the builder's own
space, or in any other space, and executable in the child. Loading already works this way. A loader
writes code through a writable mapping in its scratch, then maps the frame as code into the child.
What is open is whether the writable side may outlive the child's start. The mapping log records
every mapping of a frame in any space, so a check there could see it.

One ruling remains before the lane starts: whether claim 11 covers the cross-space alias, and if
so what the fix refuses there. An architect has ruled the check's home (the mapping hold) and the
refusal's width (the writable-and-executable pair only). Each question, with its background, options, prior art, costs and an example on the
tenth pass's reproduction, is in
[`the-rulings-the-alias-refusal-waits-on`](proposals/the-rulings-the-alias-refusal-waits-on.md).
The status stays NOT-STARTED until an architect rules it.

## Index row

`MAP_INTO` never refuses a frame the space already maps. `MAP_RW` at one `va` and `MAP_CODE` at
another leave every page-table leaf honest while the physical page is both writable and executable.
That defeats W^X by alias on a shipped path. The fix refuses a second mapping that would leave a frame writable through one mapping and
executable through another, in `MappingHold::record_mapping`, which every mapping
path reaches. It also lets the pass's witness
join the default suite on all three architectures.
