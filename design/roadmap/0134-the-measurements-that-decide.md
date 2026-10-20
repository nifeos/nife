---
status: BUILT
raised: 2026-08-18
built: 2026-10-20
---
# 134. The register of measures: every number this kernel owes itself

The register exists and holds the tier A and tier B measures below. It landed 2026-08-18 as
notes/register-of-measures.md, with the test for what belongs in it and the three states a measure
can be in, plus the unsafe census calef folded in and its `count-at-most` ceiling. Tier A (E1
through E4) ran 2026-08-22, and E4's own follow-up (a background load near E1's knee) ran
2026-08-23. E1, E3 and E4 were re-taken on radon on 2026-09-04: the small-cache board all three were
designed against, where the dev Mac's large L1 had been muting them. Tier B remains gated on the
counters of milestone 74 (cycle counters) and the silicon of milestone 127 (the seL4 machine),
measure by measure as "What is built, and what is not" says.

Raised 2026-08-18 by calef, in one question: *"So what data would enable us to make these
decisions?"* Both open kernel decisions ended in the same place, that the deciding number does not
exist, and §96 (process kernel or event kernel) said the instrument "wants a roadmap block of
its own". This is it.

E3's question is retired, by calef's ruling of 2026-10-20 (UTC). Milestone 796 (pin the hot trap
path's placement) holds the hot path still, so no pad or shift moves a byte that is fetched. The
footprint question milestone 188 (the IPC fastpath) leans on is the executed one, E5. "Follow-on"
has the ruling.

The detail lives in two appendices, and this page is complete without them:

- [`measures.md`](0134-the-measurements-that-decide/measures.md): each measure's full definition,
  with its mechanism, prediction, instrument and what each outcome settles.
- [`build-record.md`](0134-the-measurements-that-decide/build-record.md): the dated build narrative
  from 2026-08-18 to 2026-09-19, every result in full, and the corrections made on the way.

## Why a register, and why two tiers

The first draft listed only the experiments runnable today, on the reasoning that a measure we
cannot take is not yet useful. calef's correction, the same day: *"I'm fine defining measures that
we cannot capture until the hardware. I want to capture the measures as a milestone."* He is right,
and the reason is this tree's own ladder. A measure that exists only as an intention is at rung
zero. Written down with its instrument and its blocker, the next person can find it and take it when
the hardware arrives. Defining it costs an hour; rediscovering the need for it costs the milestone
that goes without it.

So this is a register, not a work queue. Two tiers, defined to the same standard, and the tier is
a property of the instrument rather than of the measure's importance. Several of the most valuable
numbers here are in tier B.

What this block is not. It does not build the counters, which is milestone 74. It does not decide
who may read them, which is milestone 75 (who may read the cycle counter, and by what authority). It
does not run the cross-OS comparison, which is milestone 25 on milestone 127's silicon. It says what
to measure and why, so those three have a customer.

## What the two decisions are waiting on

| Decision | The claim it cannot check | Why our instruments miss it |
|---|---|---|
| §95, an IPC fastpath | 5.6 KiB of hot path costs something | icount models no cache; nothing measures I-cache |
| §96, process against event kernel | per-thread kernel stacks cost something | every bench in the tree is two threads |

The two share one root: every instrument here is a micro-benchmark, and both claims are about cache
behavior under load. Warton's event-kernel result is the warning written out, at "generally within
1% on micro-benchmarks" and 20% on a multi-tasking workload.

## The measures

Tier A is available today, on the dev machine under Hypervisor.framework. Tier B is gated on the
counters, the silicon, or both: QEMU-TCG models no cache, no TLB and no cycle, and HVF passes
through no PMU. [`measures.md`](0134-the-measurements-that-decide/measures.md) defines each one.

| Measure | What it is | What it settles | Where it stands |
|---|---|---|---|
| E1 | IPC latency against thread count | whether per-thread stacks cost anything (§96) | knee at 16 threads on radon |
| E2 | thread census on the customer path | whether §96 matters at all | 4 new threads |
| E3 | footprint perturbation: pad the fastpath, time it | §95's premise, with no PMU | retired by calef, 2026-10-20; E5 (milestone 708) asks it of executed code |
| E4 | application displacement under IPC load | Liedtke's claim, as lost throughput | 5 to 8% at 96 threads on radon |
| M5 | cycles per IPC round trip | milestone 101 (the L4 calibration)'s ±1.5x; the seL4 comparison | instrument on all three ISAs |
| M6 | I-cache misses per IPC | whether the footprint proxy is right | no event counter yet |
| M7 | D-cache misses in the stack region | §96's cause, not its symptom | no event counter yet |
| M8 | TLB misses per IPC | stack stride cost; the ASID claim | no event counter yet |
| M9 | per-phase cycles across one IPC | where fastpath effort should go | counter, no phase stamps |
| M10 | lines of app working set one IPC evicts | Liedtke's claim, directly | needs silicon |
| M11 | interrupt latency, observed and bounded | the counterweight to §95 | needs silicon |
| M12 | M5 to M10 on seL4, same board | an honest cross-OS ratio | milestone 25 (cross-OS performance comparison) |

E1's prediction, so it can be wrong: stacks 28 KiB apart, at 1 to 2 KiB touched per IPC, fill a
32 KB L1d at 16 to 32 threads, so the knee should be in the low tens. E1 must not run under icount,
which models no caches and would report a flat line that looks like an answer. E3's power is
asymmetric: a positive result on a large-cache machine is conclusive, and a negative one needs a
small-cache board.

## The correction this milestone makes to the decisions that raised it

§95 (an IPC fastpath) recommends holding the fastpath for "milestone 74's cycle counters on
milestone 127's TX1". §96 recommends re-opening "when that number exists". Both read as though
silicon were the blocker, and neither is true as stated. E1, E2 and E3 run on the existing dev
machine, and E2 needs no new benchmark at all. The honest gate on both decisions is these
experiments, not the hardware, and the hardware improves the answer rather than enabling it. The
experiments that produce a *verdict* need no silicon; the PMU produces *mechanism*.

Recorded here rather than by editing those sections, because a recommendation that was wrong is
worth more standing next to its correction than quietly fixed.

## What is built, and what is not

[`build-record.md`](0134-the-measurements-that-decide/build-record.md) has each step in full.

- 2026-08-18: notes/register-of-measures.md, with nine gated numbers, four dated ones and a
  deliberate-exclusions table. Its test: a number belongs if something depends on its value and it
  can move without anybody editing it. The unsafe census (notes/unsafe-obligations.md) came with it.
- 2026-08-22, on the dev Mac: E2 found 4 new threads at the SMB/FS customer-path topology. E1 is
  flat at about 1,270 to 1,310 ns/iter through 16 threads, rising 8 to 11% by 64 to 96. E3 roughly
  doubles footprint for a 2 to 3% effect, within noise. E4 reads 0 to 3%.
- 2026-08-23: E4 at E1's knee (48 pairs, 96 threads) reads 2 to 9% against 0 to 5% at low load,
  consistently higher but overlapping.
- 2026-09-04, on radon, six interleaved boots (`bench/radon-2026-09-04/`, read in
  notes/footprint-perturbation.md): E1's knee is at 16 threads, 68% up from 2 and then flat to 96.
  E4 is 5 to 8% under 96-thread load against 0 to 1%, peaking at a 32 KiB working set, radon's L1d
  exactly. E3 is 1.49% slower on `call_reply` and 3.01% *faster* on `ipc_rtt_el0`, which dead code
  cannot cause. So E3 cannot separate footprint from code layout.
- 2026-09-04: E3's padding was found reachable only from `sched::ipc_send`, a shape nothing runs.
  `sched::ipc_call` now pads too, to roughly 1.85x on both ISAs.
- 2026-09-19: the layout control is built as milestone 370. `NIFE_FASTPATH_PAD` and
  `NIFE_FASTPATH_SHIFT` give a dose-response over four pad sizes against four un-padded layouts, and
  `script/fastpath-footprint --layout` proves all eight share one instruction stream. Booted under
  QEMU only.
- 2026-09-19: the per-IPC kernel stack depth is measured (`kernel/src/ipc_stack_depth.rs`), and
  BUGS says what it does to E1's prediction. Capacity does not explain a knee at 8 to 16;
  page-aligned stack tops sharing set indices in radon's 4-way L1D would, and so would page-aligned
  TCBs.
- 2026-09-19, tier B's instruments: M5 has one on all three ISAs; M9 has the counter but not the
  stamps; M6 to M8 have none. `PMCCFILTR_EL0` is provisional pending calef's decision A in
  `design/roadmap/proposals/the-aarch64-half-of-74.md`, so no aarch64 cycle figure is published yet.
- 2026-10-20, on radon: E3's step 0 fails under milestone 796's pin
  (notes/footprint-perturbation/step-0-under-the-pin.md), and calef retired E3's question. The
  optional stack-depth boot ran at the same commit as a QEMU control. Kernel-thread CALL/REPLY
  matches to the byte; every EL0 series reads 16 bytes deeper on the board, because `single_hart`
  grows `riscv_trap_body`'s frame from 160 to 176 bytes (notes/stack-high-water/per-ipc-depth.md).

## Scope note

Instrumentation and measurement. No syscall surface, no wire format, no dependency. It builds
benchmarks and produces numbers; it decides nothing by itself, which is the point.

Sequencing: within tier A, E2, then E1, then E3, then E4. E2 first because it is nearly free and
can retire §96 on its own; E4 last because it needs a design rather than a harness. Tier B is
ordered by its blockers rather than by us. M5 goes the moment milestones 74 and 75 land, M6 through
M9 as soon as there is silicon, M10 and M11 after, and M12 last because it needs the other kernel
built and booted.

A tier B measure is not a promise to take it. Some may be answered by a tier A result, and a
measure this register carries and nobody ever needs is a cheap thing to have been wrong about.

## BUGS

- The dev Mac is the wrong machine for a null result. Its caches are large enough to hide both
  effects, so E1 and E3 are conclusive when they show something and weak when they do not. Both want
  a re-run on a 32 KB L1 board before a negative is believed. notes/portability.md already calls HVF
  a rehearsal rather than a verdict, and that applies exactly here.
- E1 measures the process kernel's penalty, not the event kernel's benefit. It shows whether the
  cost exists and how it scales; it cannot show what a continuation-based kernel would actually
  recover, because that requires building one. A knee makes the case worth taking seriously rather
  than proving the alternative wins.
- ~~The 1 to 2 KiB per-IPC stack figure in E1's prediction is an estimate, not a
  measurement.~~ Measured 2026-09-19 on all three ISAs, debug and release. It is about 600 bytes
  per kernel thread per round trip in the release build E1 runs, 0.5 to 1 KiB for an EL0 thread,
  and 2 to 4 KiB in the debug build (notes/stack-high-water.md, "Per-IPC depth"). Redone with the
  measured figure, the prediction puts a pure capacity knee near 54 threads. The observed knee is
  at 8 to 16, and milestone 506 is the test of why.
- The per-IPC depth table is QEMU's, for a build radon boots with two features QEMU cannot
  (`board`, `single_hart`). One of them moves the trap path: `single_hart` saves three more
  registers in `riscv_trap_body`, so every EL0 series reads 16 bytes deeper on radon than in QEMU
  at the same commit. Kernel-thread series match to the byte
  (notes/stack-high-water/per-ipc-depth.md).
- None of this measures the verification argument, and nothing can. §96's claim that explicit
  continuation state suits a model checker better than an implicit stack is a design argument, and
  it will still be a design argument after every number here exists.
- Tier B is specified from the outside, against instruments nobody here has used yet: the cache
  and TLB events a board's PMU exposes. Real PMUs do not implement every architected event, and some
  implement them wrongly, so a measure may turn out unavailable or untrustworthy on the specific
  silicon. Expect M6 through M8 to need adjusting to what the TX1 and the U74 count, and treat the
  event names as intent rather than as a plan that will survive contact.
- M9 could refute the fastpath as sketched, and that is a feature of listing it rather than a risk
  of taking it. §95 reasons from `syscall::dispatch` being the largest symbol to it being the thing
  worth skipping. If per-phase attribution says dispatch is cheap and the trap entry dominates, the
  fastpath work should change shape. Better to find that with a counter than after building it.

## Follow-on

- **Refused.** E3 under a layout control, retired by calef's ruling on #2472, recorded by the
  maintainer as *calef, 2026-10-20 (UTC): "2". Retire E3's question* (19:58 UTC). Step 0 failed
  by construction at `68647cfc5`: eight images, one hot-path layout, `the sled precedes 0 of 16 hot
  symbols`. A byte nothing fetches can reach a clock only by moving code that is fetched, and
  milestone 796's pin moves none of the hot path. Whether moving kernel text costs time is
  796's own sweep. The footprint question milestone 188 leans on is the executed one, E5,
  milestone 708 (an executed footprint ladder for radon). The evidence stays in
  `bench/radon-2026-10-20/` and notes/footprint-perturbation/step-0-under-the-pin.md. Liedtke's
  executed-footprint claim is M6 and E5, never E3.
- **Milestone 506.** milestone 506 (color the kernel stacks and take E1),
  `design/roadmap/0506-colour-the-kernel-stacks-and-take-e1-again.md`: start each thread's stack a
  per-slot color below its top in a feature build and take E1 again on radon. A knee that moves
  right says the stacks caused it, and a process kernel buys it back with coloring. One that stays
  at 8 says the TCBs (page-aligned) did, which an event kernel would not remove either. Either is a
  sharper input to §96 than it has. Can ride on any radon bench evening.
- **Milestone 503.** milestone 503 (an event-counter driver),
  `design/roadmap/0503-an-event-counter-driver-for-m6-to-m8.md`: one cache-refill and one
  TLB-refill event per ISA, kernel-internal, read like 74's cycle counter. Milestone 74's scope note
  held generic events back until a second consumer; M6 to M8 are that consumer. Its first step is
  finding which events radon's OpenSBI and argon's A57 count.
- **Milestone 229.** Tier B's authority blocker is gone.
  `design/decisions/0139-cycle-counter-authority.md` is DECIDED (calef, 2026-09-02, a per-thread
  grant in the spawn manifest) and `design/roadmap/0229-the-counter-grant.md` is BUILT the same day;
  `kernel/src/arch/aarch64/timer.rs` opens and closes the counter per thread at the switch.
- **Milestone 25.** M12, the same measures on seL4 on the same board, is that block, itself PARTIAL
  and gated on hardware and milestone 74, which this one already cites.
- **Done.** Milestone 74's counter driver landed (riscv64 2026-09-03, aarch64 2026-09-19 as PR
  #972), and milestone 309 (unhalted core cycles) added x86_64; "What is built, and what is not" has
  where each tier B measure stands.
- **Done.** The silicon this register waits on arrived: `notes/target-hardware.md` lists argon as
  awaited (2026-10-06) and radon as booting nife and wired as a bench target, which is the
  small-cache board this block names as milestone 127's alternative.
- **Done.** Milestone 370 (the layout control) is BUILT, and the per-IPC stack depth is measured,
  both 2026-09-19.
- **Recorded.** Two findings filed elsewhere, in
  [`build-record.md`](0134-the-measurements-that-decide/build-record.md) with the corrections: the
  x86_64 QEMU runner escapes `helpers/qemu-bounded.sh` (notes/qemu.md), and the instrument copies
  role numbers (notes/stack-high-water.md's BUGS). The limitations this list used to repeat are the
  BUGS above.

## Index row

Both open kernel decisions ended at the same place, that the deciding number does not exist. A
register rather than a work queue, in two tiers defined to the same standard. A measure that exists
only as an intention is at rung zero, and the tier is a property of the instrument rather than of
the measure's importance. Tier A needs no silicon, which corrects §95 and §96
for over-gating on the TX1: a thread census that can retire §96 alone, IPC latency against thread
count, and a footprint perturbation that tests Liedtke with no PMU. The latency run carries a
falsifiable prediction: stacks 28 KiB apart fill a 32 KB L1d at 16 to 32 threads, so the knee
should be in the low tens. It must not run under icount, which would report a flat line that looks
like an answer. Tier B is cycles, I-cache, D-cache attributed to the stack region, TLB, per-phase
attribution that could refute the fastpath premise, application working-set displacement, and
interrupt latency as the counterweight nobody has priced. BUILT 2026-10-20: the register
(notes/register-of-measures.md) carries the unsafe census and its `count-at-most` ceiling. Tier
A is read on radon: E1's knee at 16 threads, E4's 5 to 8% under 96-thread load, 4 new threads at
the customer-path topology. E3's question is retired by calef in favor of E5, milestone 708. Tier
B stays owed on milestone 74's counters and milestone 127's silicon
