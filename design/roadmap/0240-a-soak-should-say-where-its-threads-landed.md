---
status: BUILT
raised: 2026-09-03
built: 2026-09-03
---
# 240. The soak reports what happened and not where, so an eightfold difference cannot be explained

Built 2026-09-03. Minted the same day by calef, during the first bench session that ran
the soak on real silicon. *(Number provisional until the merge queue lands it.)*

In brief. Two soak runs on radon, from the same card and the same build, twenty minutes apart:

| | first beat | rate | crossings at beat 12 |
|---|---|---|---|
| 13:04 | `rounds=918313` | **183,662/s** | ~3,000 |
| 13:24 | `rounds=112960` | **22,592/s** | 47 |

Eightfold, and the machine was identical. The boot tour's own pure-compute check ran 6,904,828
and 7,271,375 iterations in the first run against 6,831,327 and 7,288,574 in the second, over the
same fixed window, with 82 preemptions both times. Same clock, same four cores online, same timer,
same firmware. So the CPU is not throttled and the difference is in the workload, not the machine.

The explanation is almost certainly placement, and milestone 221 (the soak never crosses cores,
so build the hook that makes it) already predicted its shape. Its `BUGS` records that the crossing
count varies by more than 2x between identical runs and names the boot-time placement lottery as the
cause. Twenty-four threads (four groups of a responder, three callers, a grinder and a tick waiter)
are placed across four cores at spawn, and nothing rebalances. That is the central finding of
milestone 219 (the boot tour ends and the kernel halts, so there is nothing to soak) and a
deliberate design
following DECISIONS 138 (how a saturated workload is made to hand threads across cores). A core that
draws two grinders starves its IPC threads, because a grinder is pure compute and never yields.

**Almost certainly is not good enough**, and that is this milestone. The soak prints round trips,
wakes, crossings, remote placements, steals and deferrals. **It does not print where any thread is.**

## What it needs

A per-core census at soak start, printed once: which roles landed on which core. Enough that a
reader of a log alone can say whether that boot drew two grinders on one core, without having the
board.

That turns a series of boots from *"the rate varies"* into *"the rate varies with this placement"*,
which is the difference between an observation and a measurement.

## Why it is worth building before the next bench session rather than after

Tonight's plan is a series of boots rather than one long run, on two independent arguments that
arrived the same afternoon. The eightfold spread, and the literature that
`notes/soak.md` now records, where stress coverage saturates and independent runs multiply while one
long run does not.

Without the census those boots produce a distribution nobody can attribute. With it, every log
explains itself, and the question of whether thread affinity is worth its system call surface becomes
answerable from data rather than from argument.

## What this is not

**It is not a rebalancer.** DECISIONS 138 declined one, on prior art that no capability microkernel
read for it rebalances at all, and nothing here reopens that.

**It is not an argument for thread affinity yet.** Two boots are two boots. The census is what would
let a series of them make that argument, or refuse it.

## What was built, and the two things it changed about this block

The end-of-run question is settled, and not the way this block framed it. It asked whether a
census should *also* be reported at the end. The first run answered something stronger: nine to
eleven of the twenty non-waiter threads are off their spawn core by the first beat, on every run,
with the steal count in the low single figures. So it is not work stealing. It is the local
rendezvous wake, and DECISIONS 138 (how a saturated workload is made to hand threads across cores)
already said in words that a communicating set converges onto one core within a few exchanges and
stays. A start-only census would have misattributed every boot tonight. The soak now prints one at
start, one whenever the arrangement changes, and one before the thread dump on a failure, with a
`drifted=` field on every beat saying whether the block above the reader is still current.

The starvation shape named above is not the one the first four runs support. This block named *a
core drawing two grinders*; the QEMU run that did exactly that was the fastest of the four, and what
tracks the rate in that small sample is how many IPC groups share a core. Four runs on an emulator
settle nothing, which is why this milestone built an instrument rather than an argument. The table is
in notes/soak.md.

## BUGS

- The placement explanation is still an inference on silicon. The census makes the placement
  readable; it does not make two boots into a series. Only a run of boots on radon can say whether
  the eightfold spread follows the arrangement, and that is the measurement this was built for
  rather than one it performed.
- The census is `last_cpu`, so it reports where a thread last ran rather than where it is
  queued to run next. A thread waiting in another core's inbox still reads its old core. That is the
  honest answer to the question the field can answer, and it is not the whole placement story.
- It adds output to a run whose output is already dense. Four or five lines at start, and a
  fresh block only when the arrangement changes, which after the first beat is almost never. A
  machine that genuinely thrashes would print one per beat and roughly double the log.

## Follow-on

- **Milestone 225.** The measurement this instrument was built for. The census makes placement
  readable; it does not make two boots into a series, and only a run of boots on radon can say
  whether the eightfold spread follows the arrangement. Four runs on an emulator settle nothing, and
  whether thread affinity is worth its system call surface is the question that series would answer or
  refuse.
- **Recorded.** In `design/roadmap/0240-a-soak-should-say-where-its-threads-landed.md`: the census is
  `last_cpu`, so it reports where a thread last ran rather than where it is queued to run next, and
  a thread waiting in another core's inbox still reads its old core. That is the honest answer to
  the question the field can answer, and it is not the whole placement story.
- **Recorded.** In `design/roadmap/0240-a-soak-should-say-where-its-threads-landed.md`: it adds
  output to a run whose output is already dense. Four or five lines at start and a fresh block only
  when the arrangement changes, which after the first beat is almost never, though a machine that
  genuinely thrashes would print one per beat and roughly double the log.
- **Refused.** Not a rebalancer. DECISIONS 138 declined one on prior art, since no capability
  microkernel read for it rebalances at all, and nothing here reopens that question.

## Index row

two runs from one card differed 8x in IPC rate with the CPU proven identical, and nothing prints
which core a thread landed on
