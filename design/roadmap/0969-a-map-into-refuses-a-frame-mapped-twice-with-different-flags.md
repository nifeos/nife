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

Claim 11 covers one address space (ruled 2026-10-31 UTC). The lane rewrites row 11 of
`notes/confinement-claims.md` to say so, with its limit beside it: a holder of `WRITE` on a frame
can change code that another space runs, and the capability decides who that is. Loading relies on
exactly that: a builder writes code through its own writable view, then maps the frame as code into
the child. Claim 11's falsification stays the same-space witness and its replayable patch, re-cut to
remove the check. Whether a global rule replaces the limit waits on a price, proposed below.

An architect ruled all three questions on 2026-10-31 (UTC): the check's home (the mapping hold),
its width (the writable-and-executable pair only) and its scope (one address space). Each
question, with its background, options, prior art, costs and an example on the tenth pass's
reproduction, is in
[`the-rulings-the-alias-refusal-waits-on`](proposals/the-rulings-the-alias-refusal-waits-on.md).
Nothing blocks the lane now.

## BUGS

- The per-space limit has a cross-space route: a component holding `READ` on a frame a less trusted
  peer writes, and `WRITE` on its own address space, can map that frame as code and run the peer's
  bytes. `MAP_CODE` needs only `READ` on the frame. Read on 2026-10-31 (UTC), not booted, these hold
  both:
  - `net_stack`, built by `system_initializer::children` with its own space at
    `socket_contract::stack_slots::OWN_SPACE`, and by the kernel's `run_with_own_space` spawns in
    `virtio_service`, `e1000e_service` and `designware_ethernet_service`. `socket_table::attach`
    receives each client's socket page as a frame capability and holds it until it maps it and
    deletes it.
  - `domain_name_resolver`, own space at `endowment::OWN_SPACE_SLOT`. `attach_client` keeps each
    client's frame capability in `state.attached` for the life of the grant.
  - The progenitor (`system_initializer`), own space at slot 28. It holds frames its children write,
    such as the network DMA page and the shell's output page.
  - No fixture. `scratch_release_witness` holds its own space but no peer's frame, and the two
    fixtures that alias a writable code page into children (`os_primitives_benchmarker`,
    `job_mix_task`) hold no capability to their own space.

## Follow-on

- **Proposed.** What a global rule would cost, measured, before an architect decides it.
  `design/roadmap/proposals/price-the-global-writable-and-executable-rule.md` has it.
- **Proposed.** Whether introducing code becomes a right of its own, which would also close the
  route in `BUGS`. `design/roadmap/proposals/a-confined-component-holding-its-own-space-can-load-code.md`
  has it.

## Index row

`MAP_INTO` never refuses a frame the space already maps. `MAP_RW` at one `va` and `MAP_CODE` at
another leave every page-table leaf honest while the physical page is both writable and executable.
That defeats W^X by alias on a shipped path. The fix refuses a second mapping that would leave a frame writable through one mapping and
executable through another, in `MappingHold::record_mapping`, which every mapping
path reaches. It also lets the pass's witness
join the default suite on all three architectures.
