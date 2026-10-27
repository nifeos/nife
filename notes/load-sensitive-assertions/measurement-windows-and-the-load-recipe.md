# Measurement windows and the load recipe: the second round, 2026-08-04

*(An appendix of [notes/load-sensitive-assertions.md](../load-sensitive-assertions.md), which holds
the register and the rules. Moved here on 2026-09-24 and tightened; pull request #1211's first
commit has the text verbatim.)*

## The second round, 2026-08-04: three more, and a diagnostic the first round did not have

The `cpu matrix` job became a merge blocker. Three sites failed across four models in a handful of
runs, on pull requests whose diffs could not reach them (an `xargs` change failed a timer
assertion). One of the three was the probe [the first round](global-baselines-and-the-drift-law.md)
had deliberately left alone.

| site | model | what it said |
|---|---|---|
| `arch/riscv64/timer.rs`, `holding_a_lock_masks_the_timer` | `rv64`, the control | `left: 41, right: 40` |
| `smp.rs`, `work_can_be_placed_on_every_cpu` | `rva23s64`, `thead-c906` | "work placed on a core never ran there" |
| `sched.rs`, `a_thread_that_never_yields_is_preempted_anyway` | `sifive-u54` | "the spinner never ran at all" |

The diagnostic that sorts this round is the window, not the direction. The first round sorted the
family by the sign of the discrepancy, which found three assertions written against something wider
than the property. These three are all positive failures, and the sign says nothing useful about
them. Each measures across instructions that are not part of the thing it measures. One counts a
tick between the read and the mask. One asserts a probe's execution that placement never promised.
One samples a spinner before it was ever given a turn. Host contention causes none of them. It
stretches the window in wall-clock terms, and that turns a race that never lost on a quiet machine
into one that loses on a shared runner.

### `holding_a_lock_masks_the_timer` (both ISAs): the window moved inside the lock

The claim is "no timer interrupt lands while an `InterruptSafeMutex` is held". The measurement was
`before = ticks()` outside the critical section, then `ticks()` inside it. So the window included the
handful of instructions between the read and `M.lock()`. A tick landing there is charged to the
lock, and the run goes red with a message accusing `InterruptSafeMutex` of not masking.

That window is where a descheduled vCPU resumes. A resuming vCPU has a deadline already in the past,
so it takes the interrupt at the first instruction it executes. Hence "left: 41, right: 40", one
surplus tick, on the control model. The same window also straddled a preemption point, and `TICKS`
is per core (§11 (per-CPU run queues)). A steal (§28.3) moving the thread between the two reads
compares two unrelated counters. On a machine whose cores started within a tick of each other, that
also looks like an off-by-one.

Both reads now happen inside the critical section. There interrupts are masked and the thread can
neither switch nor migrate, so `cpu::id()` is fixed across the block and the window is exactly the
property. Nothing about the assertion was weakened. A real masking failure lands a tick inside that
window and still fails, now without a competing explanation.

The post-release half ("and the moment we let go, the pending interrupt is delivered") kept its
claim and lost its fixed two-period spin. It waits, bounded in tick periods, and reads the counter of
the core it was on, by index. Dropping the guard is a preemption point.

A new accessor per ISA generalizes half of this note: `ticks_on(core)` and `missed_ticks_on(core)`
beside `ticks()` and `missed_ticks()`. A per-core counter read either side of a wait must name its
core, or a migration silently changes the subject. `ticks_arrive_at_the_configured_rate` had already
found this in the first round and solved it locally, by bracketing the hart id into its snapshot. The
accessor makes it available to the other four tests in those files, all of which had the same hole:
`the_timer_is_ticking`, `the_handler_keeps_up_when_no_lock_is_held`,
`a_long_critical_section_costs_a_tick`, and the masking test itself.

### `work_can_be_placed_on_every_cpu` (`smp.rs`): the first round's verdict was wrong

The first round left it alone, arguing that its wait is on the property itself and can only fail in
the "not yet" direction. The premise is false, and the machine said so. The failure is not slow, it
is wedged, and no budget fixes it:

1. the probe for core A is placed on A's run queue while the test thread runs on A;
2. an idle core B, which has no probe of its own yet, steals it (a queued thread is fair game);
3. the rest of the probes are placed, every core is now busy, and nothing is idle;
4. A holds only the test thread, which never yields into idleness (`schedule()`: "a thread yielding
   into an empty run queue simply carries on"), so A never asks for work back.

Stealing is pull-based from an idle core (§28.3). Once no core is idle, nothing rebalances. So
`SPREAD[A]` stays zero for the full 60 s and then reports a timeout. The one-persistent-probe-per-core
trick was meant to prevent exactly this, by keeping every core busy with its own. It has a hole: it
only holds if every probe is placed before any core starts stealing. A contended host stretches the
placement loop across a tick, which is all it takes.

This is the third time §28 (SMP placement) has invalidated a placement assumption in this one file.
The comment in `secondary_main` step 6 is the second, and it stated the rule this verdict rests on:
*a deadline cannot fix an unreachable condition*. That comment also records that widening the wait
from 10 s to 60 s changed nothing, which is what finally separated the two cases there. The first
round read the same file and reached the opposite conclusion about the test next door.

So the test now asserts what `spawn_on` actually promises: arrival at the named core. A new per-core
counter, `PerCpu::adopted`, counts threads this core has taken out of its own inbox. That is the one
point where a thread crosses from a remote core's hands into this core's queue. `inbox_len` cannot
serve, because it is a depth: a push and a drain between two reads leave it where it was. The test
makes placements one at a time and follows each to a reap. So the adoption the counter shows can
only be the thread the test just placed.

The test deliberately no longer asserts that the target then ran it. That is not a property
`spawn_on` has: it is a placement hint, not a pin, and a steal moving the thread first is correct
behavior. The claim is decomposed, and each half is now stable.
`every_secondary_runs_scheduled_work` proves every core runs what is on its own queue.
`a_batch_of_cpu_bound_work_reaches_every_cpu` proves placement plus stealing fills the machine.
Delivery here, execution there.

The test also got a case it never had: this core as a target. `place_on` puts a local target
straight onto our own run queue, with no inbox and no IPI. So the old loop's `target == here`
iteration was not exercising the cross-core path at all. A placer thread on another core now makes
that placement. That is sound because a thread changes core only by a steal, and only an idle core
steals. The core running the test thread never goes idle, so the placer cannot land on it.

### `a_thread_that_never_yields_is_preempted_anyway` (`sched.rs`): a race the test built for itself

`assert!(SPINNING > 0, "the spinner never ran at all")` is a sample, taken after `STOP` is set. The
order was: spawn the spinner, spawn the polite thread, wait for the polite thread, set `STOP`, then
check that the spinner had run. If the polite thread got its turn first, `STOP` was already true when
the spinner was finally scheduled. It left its loop without incrementing anything, and the run went
red while the kernel did nothing wrong.

The spinner running is a precondition of the claim, not the claim, so the test waits for it rather
than sampling it. Waiting for it before the polite thread exists also makes the rest stronger. The
polite thread's turn can then only have come from preempting a thread that was genuinely running.
And the test baselines `preemptions()` after that point, so the preemptions the test claims are the
ones that gave the polite thread its turn.

The one-second deadline went with it. The budget is now 200 delivered ticks on this core, not a
wall-clock interval. A tick is when a preemption can happen, so preemption opportunities are the unit
the claim is counted in. It is also the one budget a contended host cannot inflate. Descheduling the
emulator delivers fewer ticks over a stretch of wall clock, while a `timer::now()` deadline keeps
running whether the guest executes an instruction or not. It is the move the first round made on the
drift twins (assert the law, not the rate), applied to the scheduler instead of the timer.

The helper (`within_ticks`) does not yield, because this test must not. It re-anchors its budget if
the thread changes core. The tick counter is per core, and a migration means we were preempted,
which is the news the test is waiting for anyway. If ticks stop entirely it does not return, and the
harness's 90 s per-test ceiling is the backstop. A timer that is not delivering at all is the arch
timer tests' failure to report, not this one's.

### The instrument that was missing: run the matrix under deliberate load

Both rounds so far worked from CI failures. That means waiting for the family to bite someone else's
pull request and then reasoning backwards. There is a cheaper way, and it should be the first thing
anyone reaches for here:

```sh
# one spinner per host core, then the matrix
n=$(sysctl -n hw.ncpu); i=0
while [ "$i" -lt "$n" ]; do ( while :; do :; done ) & i=$(( i + 1 )); done
script/cpu-matrix; kill %1 %2 %3 %4 %5 %6 %7 %8
```

On an eight-core machine that takes the load average to about 22 and reproduces this family in one
run. The first time it was tried (2026-08-04, immediately after the three fixes above) it failed
three models at two sites never seen before. Neither was one of the three just fixed:

| site | model | what it said |
|---|---|---|
| `sched.rs`, `a_sender_blocks_until_a_receiver_arrives` | `rv64` | "the sender never woke after its message was taken" |
| `sched.rs`, `other_threads_run_while_one_is_blocked` | `sifive-u54`, `rva22s64` | "a worker made no progress while another thread was blocked on IPC" |

Both are a yield count used as a duration. `wait_for`'s own doc comment describes that defect, and
round one had already fixed it once, in `threads_round_robin`. Since §28 scattered work across cores,
fifty yields on a core with an empty run queue are microseconds. They elapse before the thread being
waited on has been scheduled at all. Five such waits were converted to `wait_for`. Five other yield
loops in the same file were left: a cleanup drain asserts nothing, and a negative assertion ("it must
NOT have woken yet") only gets safer when the machine is slow.

Every matrix run this round produced, in order, because reporting the best one would be the
dishonesty this milestone exists to remove:

| run | conditions (peak 1-minute load average) | result |
|---|---|---|
| 1 | shared dev machine, no induced load (LA ~3) | 5/5 pass |
| 2 | same (LA ~3) | 5/5 pass |
| 3 | 8 spinners on 8 cores (LA 22.8) | **3 fail**: `rv64`, `sifive-u54`, `rva22s64`, at the two yield-count sites above |
| 4 | 8 spinners, after converting those waits (LA 15.5) | 5/5 pass |
| 5 | 8 spinners (LA 36.5, the heaviest of the five) | 5/5 pass |

Runs 1 and 2 are what a green CI run would have said, and they said it before the yield-count sites
were touched. Two clean matrices in a row proved nothing about them.

The lesson is about method, not about those two tests. This family is reproducible on demand, and it
had been diagnosed from CI logs three times instead. A red matrix under load proves nothing about a
model (notes/cpu-models.md is emphatic about that, and it is right). It is the best available prover
of an assertion: it is the condition under which a wait that measures the wrong thing gives the wrong
answer.

### The handler-latency twins: still not fixable with a wall clock, and now said plainly

`the_handler_keeps_up_when_no_lock_is_held` on both ISAs got the `missed_ticks_on` core-scoping and
nothing else. The rest of it cannot be re-aimed on this instrument. A miss means a whole tick period
elapsed before the handler re-armed, and from inside the guest a 30 ms handler and a 30 ms deschedule
are the same observation. `miss_detail` (aarch64) reports how late the re-arm was, which
distinguishes them for a human reading the panic. But "excuse the misses whose lateness has the
deschedule signature" is a weaker claim, not a re-aimed one: a handler slow by more than two periods
would be excused by it.

The honest alternative is the instrument, not the assertion. Under `-icount shift=0,sleep=off`
virtual time is a function of instructions executed. A deschedule cannot advance it, so "the handler
took fewer than N instructions" is a claim a contended runner cannot falsify. Unlike the placement
probe, this one has no reason to need more than one core, so the icount bench's `-smp 1` is not an
obstacle. Recommended here, not built here. (It was built on 2026-08-17, as
[the icount claims](the-icount-claims.md) records, and the assertion was deleted on 2026-08-18 by
[the timer disposition](timer-assertion-disposition.md).)
