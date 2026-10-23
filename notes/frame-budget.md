# The suite's frame budget

*Name provisional. Until 2026-10-09 (UTC) this account lived in `kernel/src/testing.rs`'s doc
comment on `SUITE_PAGE_FRAME_BUDGET`, accumulating a dated entry per raise. Milestone 862
(comments state the constraint as it is now) moved it here. The constraint stays at the constant;
the conventions and the findings are here, and the constant's doc comment links this page. The
measurement narratives went to that move's commit message, where `git blame` finds each entry's own
commit too.*

`SUITE_PAGE_FRAME_BUDGET` is what the whole system-test suite may leave unreturned, in frames,
checked once at the end of the run by `report_frame_ledger`. The gate's other half,
`SUITE_MIN_FREE_RUN`, is documented at that constant. The objects themselves are
[notes/frames.md](frames.md)'s subject; this note prices the ones the suite keeps.

## What the suite legitimately keeps

Measured conclusions, undated; each row's number is the reading that established it.

- `root_supervisor`'s construction budget: `SPLIT` from a parent with live children, refused at
  reclaim by construction.
- One page per kernel endpoint, never freed by design.
- One current-CPU page per live `AddressSpace` (about 103 spaces at the 2026-09-21 reading): freed
  by `Drop`, and `current_cpu_tests::the_page_is_returned_when_the_space_is_dropped` proves it.
- The memoized `login`: its construction budget is never returned on purpose, plus a 32-page
  channel untyped. The start-up re-derive test's second `login` carries a durable budget: 832
  pages plus two store caretakers and the timetable's staging buffer.
- Two `credential_service` instances (1,552 and 1,606 frames): nothing in this tree tears one down.
- The file servers' client staging windows (112 frames) and working-set heaps, the latter seen as
  §13 (capability revocation and untyped reclamation) mapping records.
- Terminals, compositors and raw-mode scaffolding the tests leave running the way a session leaves
  its own: five compositors about 1,900 frames, three terminals about 267 after `Cell` grew to 16
  bytes, four scratch pages per spawned client role.
- The `e1000e` DMA region: 18 pages and its domain's four page tables, held for the boot and
  reused by every later wiring.

## The conventions every change to the number follows

- Measure the merged tree's real run. Never sum two branches' numbers; per-test frame cost is not
  additive across unrelated changes.
- The tighter architecture of the pair carries the headroom.
- `+32` headroom, set 2026-08-27 and kept since, covers a measured two-frame local-against-CI
  divergence and the `caretaker_teardown` local flake's run-to-run spread.
- Raising or lowering it is a decision, not a formality: read the `[that test kept N frames]` lines
  the run prints, find who grew or shrank, and be able to say why.

## Image size moves the number too

Part of the residue is program image size, not any test's memory. Each of `c_seam_tests`' three
confiner spawns keeps a space sized by `c_confiner`'s loadable segments, 15 pages in a debug build.
Spaces that map the whole initial ramdisk keep revocation-log pages sized by it, so a larger archive
moves this number too. A change that grows a program can therefore move it by a few frames with no
leak behind it; the `[that test kept N frames]` lines and the programs' segment pages tell the two
apart.

## Every number it has been

| date (UTC) | measured | set to | cause |
|---|---|---|---|
| 2026-08-16 | 14,031 / 13,787 | baseline pair | first measured pair, milestones 54 and 55 merged |
| 2026-08-22 | 15,624 | | milestone 49 (users, login, and attribution): the memoized `login`, 640-frame budget |
| 2026-08-22 | | 16,968 | `login` capability-table fix, `CONSTRUCTION_PAGES` 640 to 1,408 |
| 2026-08-23 | | 18,627 | milestone 155 (a provisioning tool): a second credential service, 1,659 |
| 2026-08-25 | 18,621 to 18,626 (CI) | 18,632 | `MappedWindow` pulls `core::fmt` in, +54 KiB text on three programs |
| 2026-08-26 | 19,054 | 19,060 | milestone 49 channel-per-client: +320 budget, +104 client scratch |
| 2026-08-26 | 19,142 | 19,157 | milestone 47 (navigation and naming) `printenv`, measured with the above together |
| 2026-08-26 | 26,943 | 26,958 | milestone 142 (a text display good enough that people use it): scanout 128x64 to 1280x720 |
| 2026-08-27 | 22,075 / 21,853 | 22,090 | scanout retarget to 924x344: the first lowering |
| 2026-08-27 | 19,158 | 19,190 | milestone 49 terminal test scaffolding, margin doubled to +32 (branch) |
| 2026-08-27 | 22,107 / 21,868 | 22,139 | the two above reconciled on the merge |
| 2026-08-28 | 22,217 / 21,941 | 22,249 | milestone 169 (`kilo`, the smallest real text editor): raw-keystroke tests on a merged trunk |
| 2026-09-21 | 22,352 | 22,384 | one frame per live address space, the current-CPU page |
| 2026-09-26 | 22,482 | 22,514 | milestone 599 (a frame per filesystem client channel): seven staging windows, 112 frames |
| 2026-09-26 | 22,640 | 22,672 | `Cell` 8 to 16 bytes, three terminals outlive their tests (branch) |
| 2026-09-26 | 22,754 / 22,613 | 22,786 | the two above reconciled on the merge |
| 2026-09-26 | +14 (CI) | 22,800 | milestone 606 (a directory walk costs what it does on Linux): FS working set |
| 2026-09-27 | 23,450 | 23,482 | milestone 152 (durable delegation) fork 7: a second `login` proving start-up re-derive |
| 2026-09-27 | | 23,626 | fork 8: the durable budget +144, arithmetic |
| 2026-09-30 | | 24,829 | first full measured durable run, re-derive alone 5,426 |
| 2026-10-02 | 24,715 | 24,861 | timetable staging buffer 64 to 128 KiB |
| 2026-10-03 | 26,636 / 26,598 | 26,668 | milestone 719 (compositor confinement claim 25): five tests, each keeps its own |
| 2026-10-04 | 26,673 / 26,648 | 26,705 | overflow checks in the release builds |
| 2026-10-04 | 26,713 | 26,745 | milestone 494 (a driver for the network card a PC actually has): the `e1000e` DMA region |
| 2026-10-06 | 23,732 / 23,674 | 23,764 | lowered: tests stopped keeping endpoints and scanouts |
| 2026-10-10 | 22,822 / 22,763 | 23,764, kept | milestone 801 (packages over the internet): every std spawn hands back its clock service, configuration page and stack; x86_64 measured 9,344 (q35) and 5,954 (OVMF) (branch) |
| 2026-10-19 | 23,765 / 23,685 | 23,797 | milestone 139 (drive the unsafe count down) round 11, +5 from image size: `c_confiner` 14 to 15 loadable pages in a debug build, kept once by each of three spawns; the other two not isolated, with the archive about four frames larger. Base and branch measured at one path (riscv64 is main's; the branch run stopped at aarch64) |
| 2026-10-19 | 24,197 | 24,229 | milestone 142 (a text display good enough that people use it): scanout 924x344 to 1120x624 (311 to 683 frames). The four tests that hold the kernel's display DMA region while they run each keep 372 to 383 more than main's CI run (the surface's growth); compositor tests about 4 more each (four-page windows), the focus test 22. Net +432 over 23,765. aarch64 only; riscv64 on the next run |
| 2026-10-19 | 24,251 / 24,248 | 24,283 | a process's region now pays for recording the `PageFrame` runs it is granted (`user::entry::run_with`): spaces that outlive their tests and hold a run keep its log pages, about 5 more for each compositor test and 30 to 40 for the login and credential services. Both paths measured |
| 2026-10-20 | 24,267 / 24,290 | 24,322 | milestone 142 increment 5: `display_terminal` draws three more faces, so its image loads 16 more pages, and every terminal the suite keeps holds one. Against main's merge-group run (24,221 / 24,238) on riscv64: `display_tests`' terminal +15, `compositor_tests`' focus test's two window terminals +32, the rest one-frame noise and a 23-frame swap between `date_tests`. CI measured, both paths; riscv64 is the tighter |
