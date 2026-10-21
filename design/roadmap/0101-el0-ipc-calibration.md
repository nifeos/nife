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

What remains is one step, reading cycles from a PMU instead of
deriving them from nanoseconds and an assumed clock. That is milestone 74's deliverable by name, and
it rides the silicon of milestone 16 (real hardware and IOMMU-backed driver isolation). This line
read `NONE` until 2026-08-16. That was true while a
paragraph was still waiting to be rewritten, and stopped being true when it was. Nothing in this
milestone is startable today, and a gate of `NONE` was advertising the opposite to
`script/roadmap --ready`.

## What has landed, and what it cost us to say

The measurement and the paragraph are done (PR #104, commit `b4178513`). One release-build run of
both IPC benchmarks on one machine, both planes from the same boot, five boots back to back:

| bench | plane | ns/iter, median of 5 | what one iteration includes |
|---|---|---|---|
| `ipc_rtt` | kernel-side | 46 | two rendezvous, one address space, no trap |
| `ipc_rtt_el0` | EL0 to EL0 | 350 | two rendezvous, two address spaces, four `svc`s |

On one core, against seL4 itself, nife's EL0 round trip costs about 4.6 times seL4's: seL4's
`Call` plus `ReplyRecv` between two address spaces 96 ns, `ipc_rtt_el0` 442 ns, both guests of the
same HVF machine (2026-10-21, eight boots each, nife at `f87c9b5c4`;
[arm-plan-under-hvf.md](../../notes/arm-plan-under-hvf.md)). That is the ratio in force. Where it is
not apples to apples: both kernels pay stage-2 switches seL4's published figures do not; both are
loop averages against the generic timer, not sel4bench's per-call PMU reads, because the guest's PMU
is synthesized; seL4 carries one patched line for the guest's 8-bit ASIDs; and seL4 takes two
fastpath syscalls where `ipc_rtt_el0` takes four. The table above is an earlier build: the same
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

**Cycles are still arithmetic here, and no cycle ratio in this file should be quoted tighter than "same
order".** Every cycle figure inherits a ~1.5x uncertainty from not knowing which core type the vCPU
thread ran on. That is milestone 74's job, and it is why this milestone is not BUILT.

**The remaining step is milestone 74's, not a second lane on this one.** Folding it into
milestone 25 was the alternative this block named, and the reason for keeping it separate is now on
the record rather than open. The paragraph correction is what should not have waited for a board,
and it did not.
## Follow-on

- **Milestone 74.** The one remaining step, reading cycles from a PMU rather than deriving them
  from nanoseconds and an assumed clock, is that block, PARTIAL (both ISA halves built, neither run on its board). No bench row in
  `kernel/src/bench/` reads a cycle counter today. Only the `cycles_per_tick` probe in
  `bench/cycle_probe.rs` does, and it converts ticks rather than timing a path.
- **Milestone 16.** The silicon that makes a PMU readable at all is 16a, which has booted the full
  tour on three harts and has not yet run the benches. HVF on the development machine passes
  through no PMU.
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
  What remains is pointing the benches at the grant on hardware.
  Correcting milestone 75 (who may read the cycle counter, and by what authority)'s `NOT-STARTED`
  row and milestone 74's dependency line is the architect's, and this block touches neither.

## Index row

On one Apple core under HVF, `ipc_rtt_el0` costs 442 ns against seL4's 96 ns for `Call` plus
`ReplyRecv`, about 4.6x (2026-10-21), not the 12-24x this block predicted. The 1.1x to 1.7x
from seL4's published cycles crosses silicon on an assumed clock. Three errors found
where one was expected, and the prediction is left standing as the record rather than quietly
fixed. Cycles are still arithmetic on an assumed clock; the PMU reading waits on milestone 16's
silicon
