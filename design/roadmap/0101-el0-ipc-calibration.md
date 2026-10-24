---
status: PARTIAL
raised: 2026-08-04
milestone_dependencies: 74
decision_dependencies: none
machine_requirements: riscv64 silicon; PMU cycle counter
specific_machine: none
needs_person: yes
---
# 101. The L4 calibration, read from the IPC number that pays for the trap

Partial as of since 2026-08-04 (PR #104). Raised 2026-08-04 as "build the EL0-to-EL0 IPC
benchmark nobody owns", and that is not what the tree says. The benchmark exists and has been
published for days. The milestone survived because the comparison built on it did not follow, and
the corrected comparison is considerably worse for us than the one that was on the page.

Everything that can be built without a board is built. The `ipc_rtt_el0` client reads the cycle
counter across its own timed loop when the bench boot is built with `cycle_counter_grant`, and
prints the round trip in cycles, read rather than converted (below). What remains is a person at
a board running it, and an architect's answer to which board closes this block.

Outstanding: three things. A radon bench boot built with `cycle_counter_grant` reads the round
trip directly on riscv64 silicon. The same probe on argon, the TX1 milestone 127 is still waiting
for, reads it on the only machine whose cycles are the unit of seL4's 413 and 426. And calef
chooses, under "Needs calef", which of those two closes this block. Checked 2026-10-24 (UTC)
against the tree: no transcript under `bench/` carries an `ipc_rtt_el0_cycles` line, and 127 is
`NOT-STARTED`.

## Needs calef

Which reading closes milestone 101? Both are a person at a board; the question is which one.

- (B, recommended) radon's direct read. The block's own `machine_requirements` names riscv64
  silicon, and the step this block was waiting for, cycles read from a PMU instead of an assumed
  clock, is exactly what the probe gives there. The same-silicon comparison against seL4's TX1
  figures is milestone 25's (cross-OS comparison) and already gated on argon, so nothing is lost.
  It closes on one evening at a board that already boots the bench.
- (A) argon's direct read. The calibration's referent is seL4's TX1 pair, so only a TX1 cycle
  count makes the ratio a measurement. Honest, but it ties this block to a delivery with no date.
- (C) both. B now as a datum, A to close. It keeps the block open for the same reason as A.

## Cycles on silicon: what the tree reads today

radon measured `cycles_per_tick 250.00` on 2026-09-16, and `ipc_rtt_el0` in the same boot was
124,433 ticks over 5,000 iterations (`bench/radon-2026-09-16/bench-134300.log`). That is about
6,222 cycles per EL0 round trip on a U74 at a measured 1.0 GHz, single hart, release build,
and milestone 74's [`riscv64-half.md`](0074-cycle-counters/riscv64-half.md) tabulates it beside
the other rows. The clock there is measured, not assumed, which removes this block's 1.5x
uncertainty for that machine. It is still two windows multiplied together: it assumes the core did
not change frequency between the probe and the row.

It is not an L4 calibration. A U74 is an in-order RISC-V core and seL4's 413 and 426 are A57
cycles, so the two counts are of different machines doing different instruction streams. Quote it
as a riscv64 figure and nothing more.

### The direct read

With `cycle_counter_grant` beside `bench`, `ipc_rtt_el0` grants its client the counter (DECISIONS
§139's per-thread grant, via `sched::grant_cycle_counter_to_current`). The client reads
`user_mode_runtime::cycle_reading()` inside its tick window. The bench boot then prints
`bench-probe: ipc_rtt_el0_cycles` with cycles per round trip and cycles per tick across the same
window, and a second line with what the client said it counted. The row is unchanged, so no baseline moves.
`notes/benchmarks/kernel-bench-harness.md` has the lines, the refusals and the commands. Under
aarch64 TCG on 2026-10-24 (UTC) it printed 15.99 cycles per tick against the probe's 16.00, which
proves the plumbing and measures nothing. No riscv64 or x86_64 run exists: this machine has no
`qemu-system-riscv64`, and the x86_64 bench boot runs no EL0 rows.

## What has landed, and what it cost us to say

The measurement and the paragraph are done (PR #104, commit `b4178513`). One release-build run of
both IPC benchmarks on one machine, both planes from the same boot, five boots back to back:

| bench | plane | ns/iter, median of 5 | what one iteration includes |
|---|---|---|---|
| `ipc_rtt` | kernel-side | 46 | two rendezvous, one address space, no trap |
| `ipc_rtt_el0` | EL0 to EL0 | 350 | two rendezvous, two address spaces, four `svc`s |

On one core, against seL4 itself, nife's EL0 round trip costs about 4.6 times seL4's. seL4's
`Call` plus `ReplyRecv` between two address spaces takes 96 ns and `ipc_rtt_el0` 442 ns, both
guests of the same HVF machine (2026-10-21, eight boots each, nife at `f87c9b5c4`;
[arm-plan-under-hvf.md](../../notes/arm-plan-under-hvf.md)). That is the ratio in force. Where it is
not apples to apples: both kernels pay stage-2 switches seL4's published figures do not. Both are
loop averages against the generic timer, not sel4bench's per-call PMU reads, because the guest's PMU
is synthesized. seL4 carries one patched line for the guest's 8-bit ASIDs, and takes two fastpath
syscalls where `ipc_rtt_el0` takes four. The table above is an earlier build: the same
bench read 395 to 444 ns on 2026-10-21, a rise nothing has explained.

Against seL4's published pair, 413 cycles for the call and 426 for the reply, one-way each, ~839
for a round trip, the 350 ns above converts to roughly 1.1x to 1.7x. That is not a measurement of
the two kernels. It sets bare-metal PMU cycles on a Jetson TX1 (Cortex-A57, 2015) against
nanoseconds under HVF on an Apple M3, times a clock nothing pinned (2.75 GHz E-core to 4.05 GHz
P-core). The range is that clock.

Three errors were found where one was expected, and they are the reason the original "4 to 7
times heavier" read as sober: wrong plane (kernel-side, no trap), wrong build (debug against their
release), and wrong convention (our round trip against their one-way figure). Two inflate the ratio
and one deflates it, so the number landed somewhere plausible by cancellation. notes/benchmarks.md
carries all of it, with the four caveats that matter more than the ratio does: 2015 silicon against
2023, their fastpath on and ours nonexistent, their two syscalls against our four, and an assumed
clock. None of that makes this a win, and the note says so in a section headed that way.

## The prediction of record, left standing

Everything under this heading is the block as it was raised on 2026-08-04, before anything was
measured. It is wrong, and it is kept rather than quietly fixed. notes/benchmarks.md keeps the
plausible first reading of the RISC-V `map_new` movement for the same reason. The record of having
been confidently wrong is the useful artifact. A page that only ever shows corrected
numbers teaches a reader nothing about how much to trust the next one.

It forecast 12-24x. Measured on one core against seL4 itself, the answer is about 4.6x. It was
written to catch an overstatement and produced a larger one.

<!-- line-citation: exception. Reason: the block as raised, kept wrong on purpose (above). -->
> What the source actually says. notes/benchmarks.md:66 names a true EL0-to-EL0 benchmark as
> "the right follow-up" and says it "needs one `CNTKCTL_EL1` bit so EL0 can read the counter". Both
> halves are stale. ...
>
> The numbers, from the note.
>
> | what | ns/iter (HVF, debug) | what it includes |
> |---|---|---|
> | `ipc_rtt` (kernel-side, milestone 21) | ~951 | the rendezvous, no trap |
> | `ipc_rtt_el0` (EL0, the primitive suite) | ~2272 | two rendezvous and four `svc`s |
>
> And then the L4 section compares the kernel-side one anyway. It converts ~705 ns at ~3.2 GHz
> to ~2,200 cycles and reports us "4 to 7 times heavier" than an L4-lineage fastpath's 300 to 600.
> Run the same arithmetic on the number that includes the trap: ~2272 ns at ~3.2 GHz is roughly
> **7,300 cycles**, which is 12 to 24 times an L4 fastpath, not 4 to 7. The two nanosecond
> figures come from different runs ... so the ratio wants one clean run rather than a subtraction
> across sessions, and it is not going to move the conclusion by a factor of three.

Two elisions above, both marked: the block's list of what the note already contained (answered in
the table below instead) and its aside on where the 705 and 951 figures came from. Everything else
is its own wording, kept because paraphrasing a retracted claim is how this same file's sibling went
wrong twice (design/roadmap/0074-cycle-counters/corrections.md).

**Why it was wrong, since that is the part worth carrying forward.** The arithmetic is fine: 2272 ns
at 3.2 GHz really is ~7,300 cycles, and ~7,300 against 300-600 really is 12 to 24. Every step is
checkable and every step is right. It fixed the plane error and inherited the build error without
noticing it (2272 ns is a debug figure, and the debug-to-release tax on this path is ~6.7x,
measured). It inherited the convention error without noticing it either (their 300-600 is one-way).
And it inherited the 3.2 GHz clock from the paragraph it was correcting. A correction that reuses
its target's assumptions is not a correction. The last clause, "it is not going to move the
conclusion by a factor of three", is the tell. It forecloses the result before measuring, which is
the one thing a milestone whose entire deliverable is a measurement must not do.

## The two stale claims, verified against the code

The block above asserted two things about the tree. Both were checked line by line on 2026-08-16,
and both had been false for longer than the block was.

| the claim | the tree |
|---|---|
| a true EL0-to-EL0 benchmark is "the right follow-up" | `ipc_rtt_el0` in `kernel/src/bench/el0.rs`: two EL0 processes, two endpoints, a client self-timing `SEND`-then-`RECEIVE` against a server process, reported by the same function's `bench: ipc_rtt_el0` line |
| it "needs one `CNTKCTL_EL1` bit so EL0 can read the counter" | the bit is set in `init` in `kernel/src/arch/aarch64/timer.rs` (`EL0VCTEN`, bit 1), with the RISC-V twin `scounteren.TM` set in `init` in `kernel/src/arch/riscv64/timer.rs`. The RISC-V write is now a whole-register write (milestone 228 (the cycle counters are closed by assumption)), not a bit set |

Citations here name a symbol, not a line (§277 (cite a heading or a symbol, never a line number)),
because the four line numbers this table first carried drifted until they landed on unrelated code.
Re-verified against `main` on 2026-10-20 (UTC): both claims still hold as stated above.

**The sentence those halves came from no longer exists**, and that is the correction this block was
still describing as pending. Commit `b4178513` deleted it from notes/benchmarks.md in the same
change that rewrote the calibration, so from 2026-08-04 this file was quoting a sentence that was
already gone and calling it "the source". The line-number citation it used now lands in the
middle of the corrected section, which is the well-formed-but-wrong failure mode `script/roadmap`
records as invisible to it.

**One caveat on the RISC-V twin, because "it was opened by milestone 19e" is too clean.** The
aarch64 bit was genuinely set then. The RISC-V side was *documented* as set and, before 2026-07-30, was not: U-mode `rdtime` worked
only because
QEMU's OpenSBI leaves the bit permitted. Firmware default, not a choice this project made, and it
would have failed on a board whose firmware clears it.

## Scope note

**Do not delete or demote the kernel-side numbers.** They are the gating instrument: icount
regression tripwires run against them, and notes/benchmarks.md explains why a kernel-internal path
length is the right thing for a gate and the wrong thing for a comparison. Both planes stay; only
the comparison moved.

**Cycles against seL4 are still arithmetic here, and no cycle ratio in this file should be quoted
tighter than "same order".** Every cycle figure from HVF inherits a ~1.5x uncertainty from not
knowing which core type the vCPU thread ran on. radon's figure has no such uncertainty and no seL4
counterpart. That is why this milestone is not BUILT.

**The remaining step is milestone 74's, not a second lane on this one.** Folding it into
milestone 25 was the alternative this block named, and the reason for keeping it separate is now on
the record rather than open. The paragraph correction is what should not have waited for a board,
and it did not.
## Follow-on

- **Outstanding.** A radon bench boot with `script/board-image --bench --extra-features
  cycle_counter_grant`, read for its `ipc_rtt_el0_cycles` lines. Checked 2026-10-24 (UTC): no
  transcript under `bench/` has one, and the probe in `kernel/src/bench/el0.rs` is new.
- **Outstanding.** The same read on argon, the TX1. Checked 2026-10-24 (UTC): milestone 127 is
  `NOT-STARTED` and the TX1 has not arrived, and `script/board-image` builds riscv64 only.
- **Outstanding.** calef's answer under "Needs calef" (A, B or C), which decides whether the radon
  read alone closes this block. Checked 2026-10-24 (UTC): no ruling in this block or in
  `design/decisions/`.
- **Done.** The direct read is built: `ipc_rtt_el0`'s client reads the granted cycle counter across
  its timed loop under `cycle_counter_grant`, in `kernel/src/bench/el0.rs` and
  `fixtures/src/os_primitives_benchmarker.rs`, and `script/lint` compiles the pair on two ISAs.
- **Milestone 74.** Cycles from a PMU on each ISA, PARTIAL (both halves built; radon has run the
  riscv64 half, argon has run nothing). Its `cycles_per_tick` probe gave this block radon's
  converted figure.
- **Milestone 374.** A cycle column on every bench row. This block added one probe line for one
  row and left the row format, which 374 calls a decision about a recorded artifact, alone.
- **Milestone 16.** radon is 16a's board. It ran the bench suite on 2026-09-16
  (`bench/radon-2026-09-16/bench-134300.log`). HVF on the development machine passes through no
  PMU.
- **Recorded.** The kernel-side numbers stay. The IPC round trip is the gating instrument the
  icount tripwires run against, and `notes/benchmarks.md` explains why a kernel-internal path
  length is right for a gate and wrong for a comparison.
- **Recorded.** No cycle ratio in this block may be quoted tighter than "same order": every cycle figure
  inherits roughly 1.5x uncertainty from not knowing whether the vCPU thread ran on an M3 E-core or
  P-core.
- **Recorded.** The four caveats that outweigh the ratio still hold, including that seL4's fastpath
  is on and this kernel has none, recorded in `notes/benchmarks.md`. Milestone 132 measured the
  general path's footprint and did not build a fastpath.
- **Recorded.** U-mode `rdtime` worked on RISC-V because QEMU's OpenSBI left the counter permitted
  rather than because this project set it, until 2026-07-30. Milestone 228 has since made it a
  whole-register write so the code says what the comment says.
- **Refused.** Folding the remaining step into milestone 25 was considered and declined: the
  paragraph correction is what should not have waited for a board, and keeping this separate is
  what let it not wait.
- **Recorded.** The authority half of milestone 74 (cycle counters: SBI PMU on RISC-V,
  `PMCCNTR_EL0` on aarch64)'s gate is narrower than its dependency line (`75`) reads. §139 (who may
  read the cycle counter, and by what authority) is decided. Milestone 229 (build the cycle-counter
  grant DECISIONS 139 decided) and milestone 237 (the cycle-counter grant costs 136 bytes of IPC
  fastpath) are BUILT. A per-thread grant therefore rides the context switch, in
  `install_cycle_counter_grant` behind the `cycle_counter_grant` feature.
  A granted thread reads the counter at EL0 in
  `system_tests::user::cycle_counter_tests::a_granted_thread_reads_the_cycle_counter_and_an_ungranted_one_faults`.
  This block's own gate names only milestone 74, so it does not overstate the authority half.
  The benches now use the grant; what remains is running them on hardware.
  Correcting milestone 75 (who may read the cycle counter, and by what authority)'s `NOT-STARTED`
  row and milestone 74's dependency line is the architect's, and this block touches neither.

## Index row

On one Apple core under HVF, `ipc_rtt_el0` costs 442 ns against seL4's 96 ns for `Call` plus
`ReplyRecv`, about 4.6x (2026-10-21), not the 12-24x this block predicted. The 1.1x to 1.7x
from seL4's published cycles crosses silicon on an assumed clock. Three errors found
where one was expected, and the prediction is left standing as the record rather than quietly
fixed. radon's measured clock makes `ipc_rtt_el0` about 6,222 cycles on a U74, a riscv64 figure
with no seL4 counterpart. The bench boot now reads the round trip in cycles directly under
`cycle_counter_grant`; no board has run it, and which board closes the block is calef's call
