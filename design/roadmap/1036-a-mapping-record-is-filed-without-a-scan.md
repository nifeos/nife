---
status: NOT-STARTED
promoted_from: a-mapping-record-is-filed-without-a-scan
raised: 2026-11-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1036. A mapping record is filed without scanning its log page for a tombstone

Raised 2026-11-03 (UTC) by lane `lane/riscv64-benchmark-drift`, while finding what raised riscv64
`map_el0` on `main` ([the note](../../notes/benchmarks/riscv64-unexplained-drift.md)). Every
name below is provisional.

Reuse: the log page's own layout. `LogPage` already carries a 64-bit `used` word, and a
tombstone count fits in half of it without taking an entry.

## The cost

`MappingHold::file` in `kernel/src/revoke.rs` records every user mapping. Before it appends to the
head log page, it scans that page's used slots for a tombstone (a record whose `phys` is 0) to
reuse. The scan runs on every map, whether or not the page holds a tombstone, and a fresh space's
pages hold none until something unmaps. [notes/mapping-log.md](../../notes/mapping-log.md)
states the cost's shape: half of `LOG_ENTRIES` per record on average, linear in that constant.

Measured on riscv64 icount, the -O0 bench kernel, `main` at `7f54880c4` with `nightly-2026-11-02`
and QEMU 11.1.2. Deleting the scan and nothing else, so every record appends:

| row | `main` | no scan | change |
|---|---:|---:|---:|
| `map_el0` | 47,891 | 30,785 | -35.7% |
| `spawn_el0` | 226,377 | 225,825 | -0.24% |
| every other row | | | within one tick |

About 3,400 guest instructions of a 9,600-instruction map are the scan. Milestone 969 (no frame is
writable and executable in one address space) changed the scan in #2965 from `iter_mut().find`
to `iter().position` to return the slot's index. That made each scanned record about 9% dearer at
-O0, part of the +2.89% #2965 put on `map_el0`. The scan predates #2965. Nobody measured a release
build, where the loop optimizes to a compare and a branch per record and costs less.

## What to build

Each log page counts its tombstones, and `file` scans a page only when that count is above zero.

- `LogPage::used` becomes two 32-bit halves: the high-water mark it is now, and a tombstone count.
  `LOG_ENTRIES` stays 170 and `size_of::<LogPage>()` stays one frame, which its `const` assertion
  already checks.
- Every write of `phys = 0` in `revoke.rs` today (seven, in `forget_mappings_within`,
  `forget_recorded`, `forget_mapping`, `forget_mapping_at`, `unmap_matching` and
  `revoke_region`) goes through one `LogPage::tombstone(index)` that raises the count. `file`
  lowers it when it reuses a slot. One method is the mechanism that keeps the count right; seven
  call sites that each remember to count are not.
- A debug assertion in `file` checks, on a scan that finds nothing, that the count was zero.

Refused: a free-slot index per page. It costs a second word per page, and reusing the slot it names
means finding the next free one, which is the scan again. Refused: dropping tombstone reuse and always
appending. A space that maps and unmaps in a loop would then grow its log without bound, paid from
its own region.

## Done when

`map_el0` on riscv64 icount falls by about a third against the floor in force, every other row
holds within one tick, the baseline saves on all three architectures carry the step with a
`# why:` line, and `bench/attributions.tsv` names the save. A map into a space whose log holds a
tombstone still reuses it (a test).

## Index row

Every user mapping scans its log page for a tombstone, about a third of a mapping call on riscv64. Proposed: each log page counts its tombstones and the scan runs only above zero.
