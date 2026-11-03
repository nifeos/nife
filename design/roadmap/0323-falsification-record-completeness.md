---
status: BUILT
raised: 2026-09-03
built: 2026-10-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 323. The falsification record is incomplete in five ways, and each was found by a different lane

Five proposals from five lanes were about the same record, and this block carries them as one.
Parts 4 and 5 were already answered by the tree, which is the finding at the bottom of this block
rather than a footnote to it. All five parts are built, and the aarch64-only records for
confinement rows 22, 23, 24 and 30 are replayed.

Part 4 was decided on 2026-09-18, and it was a ratification rather than a minting. calef ratified
`Expected to fail:` as the line a record carries, in
[§134](../decisions/0134-harness-falsification-record.md)'s own spellings section. This block filed it
as *"what to call a new field"* on the proposal's observation that four patches already did it in
prose; counted on the day, all 66 falsification records carried the line and 65 spelled it exactly
that way. The one outlier (`Expected red`, in `crates/nifefs`) was corrected in the same change, so
the convention the gate will read is uniform across all 66.

Part 5 was decided on 2026-09-19, and the answer was neither of the two offered. It asked whether
`kernel/falsifications/` is an exception to §134's per-crate rule or a mistake. Both halves of the
premise were false, and both had been closed before this block promoted the proposal:

- The path was settled on 2026-09-01. §134 (a harness carries a machine-replayable falsification record) carries a paragraph headed *"Clarified 2026-09-01: the
  path is beside the package, not under `crates/`"*, which names `kernel/falsifications/` as
  conforming. There is no per-crate rule to be an exception to. Counted on 2026-09-19, twelve
  packages carry a `falsifications/` directory (ten under `crates/`, plus `kernel/` and
  `components/`) and every one obeys `<package>/falsifications/<module.path>.<harness_fn>.patch` with
  no special case.
- The replay was built on 2026-09-16. The part says six kernel claims have records nothing can
  replay. Milestone 305 added a `KernelTest` class to `script/falsifications` on top of milestone
  210's `cargo xtask test --test <substring>`, and `notes/confinement-claims.md` struck the `BUGS`
  bullet that said the mechanism did not exist. Measured on 2026-09-19: 14 kernel tests carry a
  record and 13 are replayable. The fourteenth is `unfalsified` on purpose, because a real escape
  there hangs the run instead of failing the test.

calef widened §134's wording on 2026-09-19 rather than leaving the one thing part 5 did surface,
which was vocabulary rather than design: the section said "harness" throughout while the mechanism
had covered kernel `#[test_case]`s since 305. `script/falsifications` had flagged that gap against
itself and handed it over; §134's *"Harness" covers a kernel test too* section is the answer.

Why a cluster and not five proposals. Each was filed by the lane that tripped over it, from a
different direction: milestone 313's security audit, 307's sweep of all 26 confinement rows, 318's
NVMe work, 319's device-tree argument, and 247's original sweep. Read together they are one finding
with five faces: `script/falsifications` is the instrument this project uses to tell a test that
can fail from a test that cannot, and the instrument has holes. That is `design/fatal-risks/README.md`
risk 3's own subject, and risk 3 is the entry whose verdict is still calef's.

Promoting them one at a time would produce five briefs, five lanes and five partial answers to a
question that wants one.

## The five parts, each with the proposal that found it and what it claims

1. The device-tree parser's four harnesses are unfalsified. `crates/device_tree_blob` carries `be32_is_total`,
   `be64_is_total`, `be32_reads_big_endian_when_in_bounds` and `align4_rounds_up_to_a_multiple_of_four`,
   and none has a record. Four patches against code that already exists. Found by milestone 319.

   Closed 2026-09-23. Still true when re-checked: all four carried `Falsification: unfalsified`.
   Four patches now exist, one per harness (`crates/device_tree_blob/falsifications/`), each
   falsified by hand before being recorded. `be32_is_total` and `be64_is_total` revert their
   `checked_add` to a bare add, reopening the near-`usize::MAX` overflow panic the hardening
   closed. `be32_reads_big_endian_when_in_bounds` swaps `from_be_bytes` for `from_le_bytes`.
   `align4_rounds_up_to_a_multiple_of_four` floors instead of ceiling-rounds, which keeps the
   multiple-of-four property and breaks `a >= n`. `script/falsifications --sweep device_tree_blob`:
   4 swept, 0 survivors, 0 stale.

2. The NVMe end-to-end test has none either, and milestone 318 (the NVMe boot test on real
   geometry) falsified it by hand with nowhere to put the evidence. What is owed is a cost
   judgment, not a decision: a fifteenth kernel record lengthens the sweep for every lane, and
   this one needs an NVMe controller attached. Found by that milestone's lane.

   Closed 2026-10-03 (UTC). `cargo xtask test` attaches the NVMe controller on every test leg, so
   no extra setup was needed to replay. The record is
   `system_tests/falsifications/user.non_volatile_memory_express_tests.a_confined_el0_process_serves_the_block_interface_end_to_end.patch`.
   Its defect is in the code under test, which the two hand falsifications of milestone 318 (the NVMe
   boot test on real geometry) were not. `Handoff::transfer_command` sends a filesystem block
   number as an LBA (`let slba = block;`), the factor-of-8 confusion the test's block 37 exists to
   catch. Replayed on aarch64 with `cargo xtask test --arch aarch64 --test
   a_confined_el0_process_serves_the_block_interface_end_to_end`: red in
   `system_tests/src/user/non_volatile_memory_express_tests.rs`, "byte 512 of block 37 came back wrong", exactly the
   assertion the patch predicts. `script/falsifications --sweep system_tests` then swept it red in
   21.8 s.

3. A record names one architecture, so a portable claim is evidenced on one leg. Found by milestone 313 (the security audit that was due since August: userspace confinement, read adversarially), as its finding 5. Two spellings fit the existing convention and either would do.

   Closed 2026-10-03 (UTC). The aarch64 defect has an exact `x86_64` twin: `arch/x86_64/mmu.rs`
   maps `.rodata` with `Flags::kernel_rodata()`, and `Flags::user_rodata()` there writes the `U/S`
   bit while `CR4.SMAP` stays off. One record cannot name two files, so `script/falsifications`
   now accepts `Architecture: aarch64, x86_64` and replays the patch on each leg, requiring red on
   every one; a hunk for an ISA that is not booted is dead text to that boot. Row 21's patch carries
   both hunks. Replayed: red in `a_user_program_cannot_read_a_kernel_address` in `system_tests/src/user/el0_boundary_tests.rs` on aarch64 and on `x86_64`
   ("the user program read a kernel address and was NOT stopped"). riscv64 stays out because the
   defect cannot be booted there (a `U` kernel page faults the kernel, see the row 21 section of
   `notes/confinement-claims.md`), and its evidence is the software walk.

   The same sweep covered the other rows reported as aarch64-only. Rows 22 (both tests), 23, 24 and 30
   have portable hunks, so their records now name all three ISAs, each replayed red on riscv64 and
   `x86_64` at the assertion the record predicts. Row 19 was reported the same way and is not: its
   two Kani harnesses are ISA-neutral, and `a_grandchild_is_bounded_by_the_root` was `unfalsified`
   while its sibling's patch already described the defect, so it now has a `replayable` record (3
   swept, 0 survivors). Its four `dir_capability_tests` kernel tests carry no record; see `## Follow-on`.

   Two things the replays found. The `x86_64` leg boots twice (direct, then real firmware) and
   the sweep summed the selections, so a record red only under firmware read as stale; it now counts
   distinct tests. And `port_holder_transmits_then_a_non_holder_faults` is exactly that record: its defect stays
   green on the direct boot and goes red only under real firmware, in the non-holder assertion of `system_tests/src/user/x86_port_tests.rs`. The
   patch now says so. The cause, measured 2026-10-03 by lane `x86-port-falsification-split`: the
   direct boot runs two cores, the per-core TSS bitmap kept the holder's grant on cpu 0, and the
   non-holder ran on cpu 1. The test now puts its non-holder's `out` on the holder's core and the
   record is red on both boots (the same assertion in `system_tests/src/user/x86_port_tests.rs`).

4. A record does not say which assertion it expects to fire, so a patch that turns a test red for
   the wrong reason is indistinguishable from one that works. Milestone 307 swept all 26 rows of
   `notes/confinement-claims.md` asking exactly this. Found by milestone 307.

   Already closed, and stayed closed. calef ratified `Expected to fail:` on 2026-09-18 (see
   `## What it is` above). Re-checked 2026-09-23 against the tree as it now stands. 73 of 74
   falsification patches carry the line (up from the 66 counted at ratification, as later lanes
   added records). The 74th
   (`kernel/falsifications/user.shell_navigation_tests.two_shells_with_different_roots_cannot_name_each_others_files.patch`)
   carries the equivalent sentence in a different word order ("Expected to fail, and it does, at
   ..."). That satisfies `script/falsifications --check`'s actual rule (a non-empty prose head)
   without literally opening `Expected to fail:`. Nothing to do.

5. Prose that still describes the mechanism as it was before milestone 305 (the six kernel confinement rows get a falsification a machine can replay) built the kernel
   replay. The part as filed said six kernel claims had records nothing could replay. The sweep of milestone 247 (follow-on work named by a finished milestone goes nowhere, and this is the third time)
   found it. It came from the block of milestone 210 (no kernel test can be run by name, so one falsification costs a whole suite). It was closed on 2026-09-16; what survives is rot around it, and it is a
   lane's afternoon rather than a design question. Two instances are known and a sweep should look
   for more. The patch
   `components/falsifications/proofs.push_never_writes_past_the_buffer_it_was_given.patch` tells its
   reader that no script will apply it and that the sweep *"walks `crates/` only"*; both were true
   when milestone 197 wrote them and neither is true now, and the record reports `replayable` today.
   Milestone 197's own `BUGS` carries the `crates/`-only claim while its `## Follow-on` two screens
   below already records milestone 212 closing it, which is the shape worth grepping for: a `BUGS`
   entry nobody struck when the work landed. The sweep earns its keep because
   `notes/confinement-claims.md` went a fortnight saying a primitive did not exist that had already
   shipped.

   Closed 2026-09-23, and the grep found two more of the same shape. The named patch's stale
   paragraph is struck and corrected in place. Milestone 212 (script/falsifications walks crates/
   only, so the ratio it prints is not the tree's), closed 2026-09-01, moved the scope to `cargo
   metadata`. Milestone 175 (split user/: components/ for services, fixtures/ for test and
   benchmark programs) moved `printenv` itself between packages (`user` to `components`), so even
   the *hand* instructions the old paragraph gave named the wrong package.
   `script/falsifications --sweep components`: 1 swept, 0 survivors, 0 stale. A grep for the exact
   phrase turned up two more unstruck instances of the same shape, both now corrected. They are
   milestone
   197's own `BUGS` entry (the one this part named as the example to grep for), and the matching
   entry in `notes/user-proofs.md`. A third, smaller staleness found in the same block: milestone
   197 said its falsification patch lived at `user/falsifications/`, true when written and stale
   since milestone 175, above; corrected.

## What it is not

Not a rewrite of `script/falsifications`. Parts 1, 2 and 3 are records the tree does not have,
part 4 adds one line the sweep may read, and part 5 is stale prose. Nothing here changes how the
sweep works.

Not a claim that the instrument is broken. It works: milestone 202 (every confinement test is a ritual until somebody breaks it) enumerated 26 confinement
claims with 25 replayable falsifications, and milestone 305 used it to find a confinement test that
could not fail. This is about the edges it does not reach yet, which is why every part was found by
a lane doing something else.

## What the two decisions cost, which is this block's second finding

Both parts filed as calef's were already answered by the tree, and neither lane could have known.

Part 5's proposal was written on 2026-09-03. The path question it raised had been settled on
2026-09-01, two days earlier, in a paragraph its author had no reason to re-read. The replay it
asked for was built on 2026-09-16 by milestone 305, thirteen days later. The proposal was
promoted into this block on 2026-09-18, two days after that, by a maintainer who did not re-check
its premise against the tree. Part 4 has the same shape with a smaller gap: it reported *"four
patches in the tree already solve this in prose"*, which was a sample of 66.

A proposal's premise decays while the proposal sits, and promotion is where that is cheapest to
catch. Filing is the wrong moment, since the lane is looking at one thing and the tree moves
underneath it afterwards. Reading the brief is too late, because by then a lane has been spent. The
promotion step already reads every proposal in the cluster and already costs the maintainer's
attention, so re-running the proposal's own claims there is the smallest addition that would have
caught both of these.

This is deliberately not a gate, for the reason §134's own `BUGS` gives about prose: no check can
tell a stale premise from a live one. It is rung three, written where the next promotion happens.

## Follow-on

- **Done.** Part 1. `crates/device_tree_blob`'s four harnesses each carry a `replayable` record,
  falsified by hand before being written down. `script/falsifications --sweep device_tree_blob`:
  4 swept, 0 survivors, 0 stale.
- **Done.** Part 2, in `system_tests/falsifications/` on branch `milestone/323-falsification-gaps`:
  the NVMe end-to-end test carries a `replayable` record, red on aarch64 in
  `system_tests/src/user/non_volatile_memory_express_tests.rs`.
- **Done.** Part 3, same branch: `script/falsifications` takes a comma list in `Architecture:`,
  row 21 is red on aarch64 and `x86_64`, and rows 22, 23, 24 and 30 are red on all three ISAs.
  A sweep of `system_tests` on 2026-10-03 reported 18 swept, 0 survivors.
- **Done.** Part 4. `Expected to fail:` was ratified by calef on 2026-09-18, before the first lane
  started. It held on re-check: 73 of 74 records carry the exact line, and the 74th carries the
  same sentence in different words. That is what the gate (a non-empty prose head) asks for.
- **Done.** Part 5. The named patch's stale "no script will apply it" paragraph is corrected in
  place, and a grep for its phrasing found two more unstruck instances: one in
  `design/roadmap/0197-user-and-xtask-proofs.md`, one in `notes/user-proofs.md`. A third, in the same
  roadmap block (a patch location stale since milestone 175), is also corrected.
- **Recorded.** Row 19's four `kernel::user::dir_capability_tests` tests carry no falsification
  record. The `attenuate` defect turns the read-only one red on aarch64 only through a vacuity
  guard at line 974 of `kernel/src/user/fs_service.rs`, so it would show the test is wired and not that a
  widened capability is refused. Written where a reader meets it, in `notes/confinement-claims.md`.
- **Recorded.** The unfalsified-harness backlog (56 of 228 on 2026-10-05) has no milestone of its
  own; a provisional one is proposed in the lane report ("drive the unfalsified Kani harness count
  to zero"). Batch 1 (`lane/falsify-backlog-1`, 2026-10-05) took 17: 56 became 39. One finding:
  `cq_pop_stays_in_bounds_and_flips_only_at_the_wrap` stayed green under an early-wrap pop, so its
  harness was strengthened before being falsified.
- **Recorded.** Batch 2 (`lane/falsify-backlog-2`, 2026-10-05, stacked on batch 1) took 36: 39
  became 3, the three left being the two recorded as unfalsifiable and the x86_64 `fp` harness,
  which needs an x86 host. Three findings, each with a strengthening commit or a note. (1) A
  `kani::cover!` cannot turn a Kani 0.67.0 harness red: an unsatisfiable cover prints "0 of 1 cover
  properties satisfied" and `VERIFICATION:- SUCCESSFUL`, so `printenv`'s
  `the_buffer_can_be_filled_exactly` stayed green under a `push` that reserved its last byte; it now
  asserts the boundary. (2) `usb`'s configuration walk was bounded at 24 bytes and the shortest
  configuration naming an endpoint is 25, so its three endpoint assertions were never evaluated;
  the bound is 25 now, with a cover that a keyboard is accepted. (3) `calendar`'s length guard in
  `number` is reached by no input (every caller is bounded first), so removing it leaves
  `parse_is_total_on_hostile_bytes` green; that harness is falsified by removing the offset-designator
  check instead, and the guard is defense in depth.
- **Recorded.** Batch 3 (`lane/falsify-backlog-3`, 2026-10-05, stacked on batch 2) took 3 unfalsified to
  2 (the two recorded as unfalsifiable stay). (1) Two harnesses had `cover!` and no assertion:
  `calendar`'s `parse_is_total_on_hostile_bytes` (now asserts a successful parse implies 20 bytes)
  and `machine_discovery`'s `no_override_writes_outside_the_sixteen_legacy_irqs` (now asserts an IRQ
  no ISA override names keeps its default entry); both replay red. `--check` now fails a cover-only
  harness. Kani 0.67.0 has no flag that fails on an unsatisfied cover (`cargo kani --help` and the
  verification-results page show none). (2) The x86_64 `fp` harness is `attested` with its patch
  written down, since the sweep and re-falsify jobs run on arm64. A new `sweep-x86_64` job
  runs `script/falsifications --sweep-attested`; first replay: run 37368707083 (red on a survivor).

## Index row

`script/falsifications` is how this project distinguishes a test that can come back red from one
that cannot, which `design/fatal-risks/README.md` risk 3 calls the difference between quality and the
illusion of it. Five lanes each tripped over a different hole in it between 2026-09-03 and
2026-09-17. Two sets of harnesses with no record at all. A record that names one architecture for a
portable claim. A record that does not say which assertion it expects. And six kernel claims nothing
could replay. The last two were filed as calef's and both turned out already answered by the tree,
which is this block's second finding: a proposal's premise decays while it sits in the pile.
Taken 2026-09-23: three of five closed. `device_tree_blob`'s four harnesses are falsified and
recorded (part 1); the stale mechanism prose is corrected (part 5); `Expected to fail:` was already
closed by calef (part 4). On 2026-10-03 (UTC) the NVMe test got a replayable record red on aarch64
(part 2), and confinement row 21 got an `x86_64` leg, with rows 22, 23, 24 and 30 replayed red on
riscv64 and `x86_64` (part 3). All five are built.
