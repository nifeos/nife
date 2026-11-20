---
status: BUILT
raised: 2026-10-05
built: 2026-10-05
milestone_dependencies: 762
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 763. A destroyed region cannot take a live space's page tables with it

Approved by calef on 2026-10-05 (UTC) as a follow-on of milestone 762 (a mapping cannot outlive its
frame's revoke), whose `revoke::revoke_region` `BUGS` recorded the hole. Built in #1665. *(Title
and slug are drafts.)*

## Why

`PageFrame::MAP` and `MemoryRegion::MAP` build the intermediate page tables a mapping needs out of a
region the caller names, which need not be the space's own, and nothing recorded them.
`MemoryRegion::DESTROY` unmapped every leaf the mapping log held in the region and handed every page
back, tables included, while the space still linked them. The next owner of such a page writes that
space's translations: fatal risk 7's confinement claim, one level up the page table from 762.

## Reuse

This is kernel code on the confinement path, where §46 (thin primitives or whole
subsystems) says write it. Inside the tree, the mapping log, 762's hold and `flush_asid` were reused
rather than given a second registry, lock or flush; the cut is the one new walk, in `crates/page_tables`
beside `unmap`.

## What was built

- The premise, driven. `system_tests::user::page_table_region_tests` maps at 128 GiB with tables
  from region R, destroys R through the `DESTROY` system call, and looks three ways. Before the fix, on
  aarch64, a `PageFrame::MAP` page still translated through R's freed table. A `MemoryRegion::MAP`
  leaf is R's own and was always unmapped, but a second map beside it built no tables and wrote its
  leaf into the freed one.
- Tables are recorded. A table a caller-named region pays for is a `TABLE` record in the space's
  mapping log, filed by `MappingHold::retype_table` under 762's hold before the mapper links it.
  `LIST`, the leaf sweeps and rollback skip table records.
- `DESTROY` cuts them. After its leaf pass, `revoke_region` takes each table record in range and
  cuts that table out of the walk that reaches it (`page_tables::Mapper::unlink_table`, one entry
  cleared). It flushes the space's ASID (`mmu::cut_user_table`, on all three ISAs) and tombstones
  the mapping records that hung beneath it. The space keeps running and loses that span, which is what
  already happens to a leaf the region paid for. `DESTROY`'s answers are unchanged.
- The registry carries the ASID, so `register_space` takes it and both space constructors now
  take the ASID before registering.
- `MemoryRegion::MAP` retypes its own leaf and calls `map_current_user_page_frame`, and the three
  `map_current_user_page` functions it alone used are gone.
- Names, all provisional: `TABLE`, `LogEntry::is_table`/`is_mapping`, `MappingHold::retype_table`,
  `Mapper::unlink_table`, `mmu::cut_user_table`, `forget_mappings_within`, `page_table_region_tests`.

## Falsification

`system_tests/falsifications/user.page_table_region_tests.a_destroyed_region_takes_no_live_spaces_page_tables_with_it.patch`
leaves the records and cuts nothing. Replayed red on aarch64 at the predicted assertion. 762's
`MemoryRegion::MAP` patch no longer applied to the new `memory_region_map` and was re-cut; it
replayed red on aarch64 at its own headline.

## Cost

aarch64 icount (`script/bench`), against 762's tip: `spawn_el0` 1,426,806 to 1,425,693 ticks
(-0.1%), `map_el0` 353,554 to 354,085 (+0.15%, about one tick a map), every other row within a few
hundred ticks of noise or lower. riscv64 and x86_64 pass `script/bench --check`. One scan finds
both kinds: a second scan of every log after the leaf pass costs `spawn_el0` 3,072 ticks a spawn
(+21.5%) and fails CI's tripwire.

## Gates

- `script/test --test page_table_region` passes on aarch64, riscv64 and x86_64 locally, and the
  host test `unlinking_a_table_cuts_its_whole_span_and_nothing_else` in `crates/page_tables`.
- The whole aarch64 suite passes locally with no frame-ledger failure (300 passed, 4 skipped).
- `script/stack-frame-check` and `script/fastpath-footprint` pass against main's baselines. On
  762's tip x86_64 `ipc_send_receive` briefly shrank 5.2% (an inlining shift) and was re-recorded;
  on main it came back to 5,339 B, so that re-record was dropped.

## BUGS

- A space's root is not covered. It cannot be cut, and a thread bound to a space whose root is
  in the destroyed region keeps running on the freed root (driven once on aarch64 with a scratch
  test). calef ruled on 2026-10-05 (UTC) that `DESTROY` kills such a thread as a resident as recorded in
  §16 (object revocation), amended that date; milestone 765 (a destroyed region cannot free the root a running
  thread walks) builds it. Recorded at `revoke::revoke_region`.
- Cutting a table takes everything beneath it, including leaves and tables other regions paid
  for. A program that mixes budgets under one table loses all of it when the region holding the
  table goes. That is the honest price of a table being an object of the region that paid for it;
  `abi::page_frame::MAP` says so where a program's author meets it.
- A table record whose table sits below one already cut stays until its own region is
  destroyed, when its cut answers `None` and it is dropped. It costs a log slot meanwhile.

## Follow-on

- **Milestone 765.** The root of a space, which no cut can reach: ruled option A by calef on
  2026-10-05 (UTC), not yet built.
- **Recorded.** Cutting a table takes what other regions paid for beneath it, at `abi::page_frame::MAP`
  and in this block's `BUGS`.

## Index row

A region's `DESTROY` no longer hands back page tables a live space still walks. Tables that
`PageFrame::MAP` and `MemoryRegion::MAP` build from a caller-named region are recorded in the space's
mapping log, and the destroy cuts each one out of the walk before the page goes back. Driven red on
aarch64 first; the root of a space is milestone 765.
