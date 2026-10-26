# Global baselines and the drift law: the first verdicts, 2026-08-03

*(An appendix of [notes/load-sensitive-assertions.md](../load-sensitive-assertions.md), which holds
the register and the rules. Moved here on 2026-09-24 and tightened; pull request #1211's first
commit has the text verbatim.)*

*(Milestone 78 (the load-sensitive assertions). `kernel/src/sched.rs`, `system_tests/src/user/tests.rs`,
the timer drift twins in `kernel/src/arch/riscv64/timer.rs` and `kernel/src/arch/aarch64/timer.rs`,
`kernel/src/smp.rs`, and the frame-hygiene assertion already removed from
`system_tests/src/user/live_swap_tests.rs`.)*

On 2026-08-03 five distinct assertions had failed pull requests that changed no executable code.
The roadmap block holds the evidence table
([design/roadmap/0078-load-sensitive-assertions.md](../../design/roadmap/0078-load-sensitive-assertions.md);
the old text said `design/roadmap.md`, corrected 2026-09-24). This appendix records what was done
about each and why. The verdicts are per assertion, and the arguments are the deliverable.

## The diagnostic that sorts the family

A slow machine produces a deficit, never a surplus. Host contention deschedules the guest's vCPUs,
so work the test is waiting on happens late or not yet. That can only make a count lower than a
wait expected: fewer ticks delivered, a thread not yet run, frames not yet returned. So the
direction of a failure is the diagnosis.

- A failure in the positive direction ("not yet") is honest load sensitivity. The fix, if any, is to
  wait on the property itself with the watchdog as backstop. That is `smp.rs`'s documented
  `wait_for` argument.
- A failure in the negative direction (fewer threads than the baseline, more free frames than at the
  start) is not a timeout at all. The assertion was written against something wider than the
  property under test. State arriving from outside the measured window tripped it: a neighboring
  test's teardown landing late. `notes/riscv-parity-scope.md` named this shape ("a wait written
  against something wider than the property"). The BUGS section of `notes/live-replacement.md` is
  the completed analysis of one instance.

Margins cannot fix the second kind. Widening a bound that fires on a negative discrepancy only hides
the defect. That is the reasoning of §61 (a lint is adopted on evidence) about the dropped lints,
applied to assertions.

## The verdicts

### Reaper count (`sched.rs`, `a_finished_thread_is_reaped_and_its_memory_returned`): rescoped

It failed as "finished threads were never reaped, left: 5, right: 6". The count was below its
baseline. Eight reaped threads cannot produce that, but one baseline-counted thread exiting mid-test
does. The test sampled `thread_count()` at the top and asserted the table returned to it, so its
baseline was a number the rest of the system moves on its own.

Now each batch keeps the eight `ThreadId`s it spawned and waits for `is_thread_present` to go false on
each. That is the property the test is responsible for ("the threads this batch created were
reaped"). It is immune to neighbors by construction: a generational `ThreadId` resolving to nothing
means this thread is gone, whatever else the table is doing. This is the third appearance of this
exact fix. `reclaim_frees_a_started_then_exited_childs_regions` got it first (see
riscv-parity-scope.md), and `is_thread_present`'s doc comment already argued it.

The frame half of the test also changed direction. The second batch's cost is asserted as
`used() <= before` (waited on, clock-bounded) rather than `==`. A leak, the milestone 6 (threads,
the context switch, and preemption) bug the test guards, leaves `used` above `before` forever, so
the wait times out and fails as before. Equality also demanded that no other test free a frame
during the window. That is the neighbor exposure again, in the frame allocator instead of the
thread table.

### Address-space frames (`user/tests.rs`, `a_dead_user_thread_frees_its_whole_address_space`): rescoped

The recorded failure is "-19 frames did not come back". It was seen on CI (the lane for milestone 71 (the thread-start fault), and
again on PR #50's build+test job on 2026-08-03) and once on a quiet aarch64 dev machine. It is
negative: `used()` settled 19 frames below the baseline, so the wait for equality could never
succeed. The frames arrived from outside the measured window. The test's own settle loop (two
agreeing samples before taking the baseline) already rules out its own in-flight frees, which is how
we know the source is a neighbor.

It took the same two changes as the reaper test. The reap waits are per-`ThreadId` (`is_thread_present`
on the outlaw just spawned, replacing `thread_count() <= baseline`). The final assertion waits for
`used() <= before`. Leak sensitivity is unchanged: every frame an outlaw's address space keeps holds
`used()` above `before` forever.

### Frame hygiene (`user/live_swap_tests.rs`): already removed, nothing to do

It was removed on 2026-08-03 in PR #46, with the completed analysis in `notes/live-replacement.md`'s
BUGS section. The tree confirms it gone, replaced by a comment saying why. The removal is not a
counterexample to "deletion is not the fix". The property the test is responsible for (the budget
reclaim returns exactly `SWAPPER_BUDGET_PAGES`) was already asserted twelve lines above, so the
global count added only the neighbor exposure. Milestone 78's postscript records the same.

### Timer drift (`ticks_arrive_at_the_configured_rate`, both ISAs): re-aimed at the re-arm law

The old assertion compared delivered ticks to elapsed counter time, with one period of slack each
way. `script/test` passes no `-icount`, so the guest counter follows host time. A host that
deschedules the vCPU for a few periods coalesces ticks into exactly the deficit the
re-arm-from-`now` defect produced. It failed under load on `rv64`, the control model. No margin
separates "our re-arm is late" from "the emulator was not running". Widening changes how often you
notice, not what is measured.

The milestone named only the riscv64 site. The aarch64 twin supplied its own evidence during this
lane's first gate run (2026-08-03): "timer drift: 22 ticks in 25 periods" at
`arch/aarch64/timer.rs`, on an otherwise green suite, while the host compiled the std farm beside
QEMU. That is a deficit, the coalescing signature, on the ISA the milestone had no drift evidence
for. Both twins got the same fix, which parity requires anyway. A test that measures the wrong thing
on one ISA measures it on both.

The test exists for one property: re-arming relative to `now` compounds lateness (TVAL on aarch64,
`now() + interval` on riscv64). The property is the grid law. The grid is state the kernel owns on
both ISAs. On aarch64 it is `CNTV_CVAL_EL0`, readable back out of the hardware. On riscv64 it is the
software `DEADLINE` array, kept because SBI's `set_timer` is write-only. So both tests now assert the
law directly. Over a window in which `MISSED_TICKS` did not move, the deadline advanced by exactly
one interval per delivered tick.

The defect fails this on the first tick, because each re-arm overshoots the grid by the handler
latency. A descheduled emulator cannot fail it. A deschedule long enough to slip the grid increments
`MISSED_TICKS` (the re-anchor safety valve, and correct behavior), and the window is retried. A
small `deadline()` accessor was added beside `missed_ticks()` on each ISA. One wall-clock bound
survives because contention cannot falsify it. Descheduling only drops ticks, so more ticks than
elapsed periods still fails; that is `rearm`'s spin-forever failure mode.

One claim moved out of scope rather than being weakened: that SBI actually fired at the software
grid's deadlines. Under `-icount shift=0,sleep=off` virtual time is a deterministic function of
instructions executed. So the icount instrument (`script/bench`) is where that claim is checkable
without the host as a confound. It was recommended and not built here; see the milestone report.
Until then the residual gap is an implementation that maintains `DEADLINE` correctly but arms SBI
with something else. No wall-clock margin could distinguish that from load either. (It was built on
2026-08-17: see [the icount claims](the-icount-claims.md).)

### Placement probe (`smp.rs`, `work_can_be_placed_on_every_cpu`): left alone, and that was wrong

*Withdrawn on 2026-08-04. The argument below is kept because it is the argument that failed, and
the way it failed is instructive. The verdict that replaced it is in
[the second round](measurement-windows-and-the-load-recipe.md).*

It was checked against the same question and appeared to pass. The wait (`wait_for(done)`, 60 s,
subordinate to the 90 s watchdog) is on exactly the property under test, not a proxy. `done` is
"each core's probe has marked its own core". Its failure direction is purely positive ("has not
happened yet"). So load produces late passes and honest timeouts, never a wrong measurement, and the
file's own comment block carries the argument for the budget.

Moving it to the icount instrument is not an option even in principle. The subject is genuinely
cross-core wall clock. The icount bench boots `-smp 1` because icount's shared virtual clock makes
multi-hart timing fictional (notes/benchmarks.md). A cross-core delivery test cannot run on a
one-core instrument.

### Round-robin fairness (`sched.rs`, `threads_round_robin`): rescoped

It failed once ("thread {i} never ran") and passed on re-run. The window was 300 yields, and a yield
count is not a duration. §28 (SMP placement) scatters the three threads across cores. On a contended
host the test core burns 300 cheap yields before a starved vCPU has run its thread at all. That is
the exact defect `smp.rs` documented and fixed in its own waits on 2026-07-30.

The test now waits on the property (every counter above zero), clock-bounded by the module's
`wait_for`, and asserts the wait succeeded. It also waits for its three threads to be reaped before
returning. Its own teardown is then not the late-landing state a later test's accounting finds in
flight. This test was a candidate supplier for the reaper test's baseline drift, three tests
upstream of it in the same file.

## Postscript: a fast machine finds the same family (milestone 81, 2026-08-04)

The day after this landed, the aarch64 suite ran on the physical Apple Silicon core under
Hypervisor.framework for the first time (milestone 81 (an HVF leg); notes/hvf-leg.md). Every failure
it produced was this family, found from the opposite direction. There were five, one per run: in
`sched.rs` (three), `user/reap_tests.rs` and `user/supervision_tests.rs`. Four were yield counts
standing in for a wait (one `yield_now()`, then 100, then 4000, then 2000). The fifth,
`a_thread_that_never_yields_is_preempted_anyway`, was a vacuity guard racing a scheduling order.

The diagnostic above needs one addition to stay right. "A slow machine produces a deficit" is true,
but the deficit's cause is that a yield count is not a duration, and that is symmetric. A loaded
host burns cheap yields while another vCPU is descheduled. A native host burns them in nanoseconds
while another core has not been dispatched at all. Both arrive as "the thing I was waiting for has
not happened yet". Both are positive-direction failures, and both take the same fix: wait on the
property with the clock as the bound. So the checklist for reading any of the remaining 34 sites
does not change; only the reason to expect a hit does.

The timer verdicts came through untouched. None of the re-aimed assertions failed on a machine where
guest time is host time and there is no icount instrument. Aiming a test at the re-arm law instead
of at elapsed wall clock made it accelerator-independent, which this note could not have claimed the
day it was written.
