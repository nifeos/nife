---
status: BUILT
raised: 2026-09-26
built: 2026-09-26
---
# 601. The region table prints its peak, and a refused split's leak is recorded

Built 2026-09-26. *(Number provisional: 596 to 600 are held by open pull requests,
so the lane took the next free; the integrator confirms it at merge. Title a draft.)* Promoted by
the maintainer from two findings in the lane of milestone 152 (durable delegation), both about the
kernel's region table (`memory_region::MAX_REGIONS`, 256).

## Why

On aarch64 `main` the suite reached `timetable_tests` with 252 of 256 regions live. The lane of
milestone 152 measured that with a temporary print. The cause was `login_test_client` leaking a
four-page scratch region per run, which #1347 fixes. One more login test made it 253, and the
timetable's `--mem` split failed. It failed as a 60-second hang, in a test unrelated to logins.
The thread table had the same failure shape before its raise to 256 (`sched::MAX_THREADS`). The
answer there was a `threads:` line on every run. Nothing printed the region peak.

## What is built

- A `regions:` line in every suite's closing summary, beside `threads:`, on all three
  architectures. The runner is shared, so one change covers all three. It prints the peak, the
  ceiling, the spare, and the test during which the peak was last raised.
  `memory_region::PEAK_REGIONS` is kept under the `REGIONS` lock on the two paths that grow the
  table. `RegionTable::len` is the reading, and its name is provisional.
- The ledger beside `MAX_REGIONS`: per-module residue at the peak. With #1347 the peaks are
  aarch64 225, riscv64 224 and `x86_64` 125, all set in `timetable_tests`. Without it, this lane's
  CI printed 256 of 256 on aarch64 and 255 on riscv64, and the suite still passed.
- A frame low-water beside it. The maintainer widened the milestone to cover it after the lane of
  milestone 198 (a package manager) hit the same failure shape on frames. `memory::FREE_LOW_WATER`
  sees every allocation. The summary also prints any refused allocation with the test it
  happened in. A per-test contiguity reading was built and dropped: it never moved, and its
  bitmap scans timed out the `x86_64` 17 GiB leg. The ledger and that measurement are at
  `memory::FREE_LOW_WATER`.
- A rendezvous peak beside both, the third ceiling the same day. The lane of milestone 152 met
  the 512-rendezvous registry in `timetable_tests`. The `rendezvous:` line prints the peak and how
  many sit on kernel chunks, which are never freed. With #1347 aarch64 ends at 505 of 512 (7
  spare), riscv64 at 491 and `x86_64` at 315. The ledger is at `sched::PEAK_RENDEZVOUS`.
- No gate on headroom, deliberately. The thread peak's reason holds. A second reason is written at
  `MAX_REGIONS`: nothing yet says which regions are meant to be permanent.
- A split refused for a full table moves nothing. `RegionTable::split` makes every check, then
  takes the child's table slot, and only then advances the parent's watermark and raises its child
  count, so a refusal leaves no child count without a child and the parent stays reclaimable.
- A sibling leak, fixed rather than recorded. `syscall::memory_region_split` minted the child,
  then returned `OutOfMemory` when the caller's capability table was full. The orphaned child had
  the same permanent consequence. It now destroys the child.
  `split_refused_for_a_full_capability_table_orphans_no_child` proves the parent comes back
  reclaimable with its budget whole, and it passes on all three architectures.

## A refused split moves nothing, a full table included

The rule is option B of `notes/region-split-on-a-full-table.md`, decide before mutating, a
provisional ruling by the maintainer that calef may overrule. The split runs in one `&mut self`
borrow (milestone 135 (the region claim, under loom)), so reserving the child's slot before moving
the parent races nothing. The host test `a_split_refused_for_a_full_table_leaves_the_parent_unchanged`
and the Kani harness `a_split_refused_for_a_full_table_moves_nothing` hold it.

## Follow-on

- **Milestone 676.** Milestone 676 (the NTP and login tests give their regions back, or say why they keep them). `design/roadmap/0676-the-ntp-and-login-tests-give-their-regions-back.md`:
  those two modules hold a third of the region residue, and only the login service is on the held
  list.
- **Milestone 671.** Milestone 671 (tests retype their rendezvous from their own region, so the registry stops filling). `design/roadmap/0671-tests-retype-their-rendezvous-from-their-own-region.md`:
  466 of aarch64's 505 live rendezvous sit on kernel chunks that are never freed.
- **Done.** A split refused for a full table moves nothing: `RegionTable::split` in
  `crates/memory_regions/src/table.rs`, under the provisional ruling in
  `notes/region-split-on-a-full-table.md`.

## Index row

The suite now prints how close the boot came to three ceilings: the region table, page frames and
the rendezvous registry. It names the test that pushed it there, so the next full table is read off a
transcript instead of found by a hang in an unrelated test. `MAX_REGIONS` carries the ledger of
what holds regions at the peak. A split refused for a full table moves nothing, and a split
refused for a full capability table destroys the child it made, so neither leaves its parent
unreclaimable.
