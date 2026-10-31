---
status: BUILT
raised: 2026-10-27
built: 2026-10-31
promoted_from: a-map-into-refuses-a-frame-mapped-twice-with-different-flags
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 969. No frame is writable and executable in one address space

Branch: `milestone/969-no-frame-is-writable-and-executable-in-one-space`.

The tenth outsider pass (milestone 936 (a tenth outsider pass attacks the confinement claim))
mapped one `READ | WRITE` frame `MAP_RW` at one address and `MAP_CODE` at the next in one space.
Each page-table leaf was honest and the page was writable and executable, which defeated claim 11
by alias. An architect ruled on 2026-10-27 (UTC) that the escape sits on a shipped path and that the
fix is mandatory.

Reuse: the mapping log every space already keeps (`kernel/src/revoke.rs`), the leaf walk
`mmu::translate_at`, and the pass's witness. No new dependency and no vendor.

## What the kernel does

`MappingHold::record_mapping` refuses a mapping that would leave a page writable through one
mapping and executable through another in the same address space. That one function serves every
path that installs a recorded user leaf: `AddressSpace::MAP_INTO`, `PageFrame::MAP`,
`MemoryRegion::MAP` and the kernel's loaders. Each path files its record before it writes the leaf,
as `MappingHold::retype_table` does for tables. It hands the record back through
`MappingHold::forget_recorded` if the leaf then fails. So no path installs a recorded leaf without
the check, and no other core can fetch through a leaf the check would refuse. The record now goes
in before the leaf (ruled 2026-10-31 UTC).

The refusal is narrow (ruled 2026-10-31 UTC). A new writable mapping fails when the space maps the
frame executable anywhere, and a new executable mapping fails when it maps the frame writable
anywhere. Read-only beside read-write passes, and so do read-only beside code and two mappings with
the same flags. The pure predicate is `paging::Flags::makes_writable_executable_pair`. The log
stores no permissions, so the check walks the leaf of each record that names the same frame. A
read-only mapping cannot complete the pair, so it skips the scan, which leaves `map_el0`'s
read-only loop out of the cost. A caller sees `NotPermitted`, the error's second case, "the object
refuses the operation whatever the rights", with no new error code and no wire-format change
(ruled 2026-10-31 UTC, #2965). Inside the kernel the refusal is
`paging::MapError::WritableAndExecutable`.

The check sees one address space at a time (ruled 2026-10-31 UTC). The same frame writable in one
space and code in another maps, and loading relies on that: `process_builder::fill_and_map` writes a
segment through its own writable view and then maps the frame as code into the child. Row 11 of
`notes/confinement-claims.md` states the claim per space with that limit beside it. The shared code
covers all three ISAs. Each ruling's background, options, prior art and cost are in
[`the-rulings-the-alias-refusal-waits-on`](proposals/the-rulings-the-alias-refusal-waits-on.md).

The tests, in `system_tests/src/user/tenth_pass_map_gate_tests.rs`, run in the default suite on
every architecture:

- `one_physical_page_is_never_writable_here_and_executable_there`, the pass's witness: `MAP_RW`
  then `MAP_CODE` through `MAP_INTO`, refused with `NotPermitted`.
- `the_pair_is_refused_through_page_frame_map_in_either_order`: code through `MAP_INTO` then
  writable through `PageFrame::MAP`, and the reverse, from a thread holding `WRITE` on its own
  space. A check in `MAP_INTO`'s dispatcher alone would admit the first order.
- `the_narrow_rule_admits_every_other_pair_and_every_other_space`: read-only twice, read-only
  beside writable and beside code, writable twice, and the frame writable in one space and code in
  another, all admitted.
- `a_refused_overlay_leaves_the_code_page_exactly_executable`: a writable overlay on a code page is
  refused as the pair, and a refused duplicate leaves no record behind.

Each test carries a replayable falsification under `system_tests/falsifications/`; the witness's
patch and the reverse-order test's remove the check from `record_mapping`. The Kani harness
`paging::verification::no_admitted_pair_is_writable_here_and_executable_there` proves the predicate
over every access word, with its own patch.

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
- A holder of `WRITE` on its own space and a `READ | WRITE` frame can write code without any alias:
  map the frame writable, write, unmap, then map it as code. Claim 11 is about one instant and holds;
  the holder still runs bytes it wrote. `net_stack` and `domain_name_resolver` hold that authority.
- `AddressSpace::map_new` maps a fresh page without a record, so the check never sees it. No
  capability names such a page, so no path can map it a second time.
- `map_el0` and `spawn_el0` are not re-measured under the check. A writable or executable mapping
  now scans its space's log once; `map_el0` maps read-only and skips the scan.

## Follow-on

- **Proposed.** What a global rule would cost, measured, before an architect decides it.
  `design/roadmap/proposals/price-the-global-writable-and-executable-rule.md` has it.
- **Proposed.** Whether introducing code becomes a right of its own, which would also close the
  first two `BUGS` entries. `design/roadmap/proposals/a-confined-component-holding-its-own-space-can-load-code.md`
  has it.
- **Recorded.** The unmeasured cost on `spawn_el0`, in `BUGS`.

## Index row

One physical page mapped writable at one address and as code at another, in one address space,
defeated W^X by alias on a shipped path. `MappingHold::record_mapping`, which every mapping path
reaches before its leaf goes in, now refuses that pair with `NotPermitted` and admits every other.
The pass's witness joins the default suite on all three architectures, beside a reverse-order
witness through `PageFrame::MAP`.
