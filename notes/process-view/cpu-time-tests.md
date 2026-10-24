# The CPU-time tests

An appendix to [notes/process-view.md](../process-view.md). The kernel tests in
`system_tests/src/user/cpu_time_tests.rs` check that the `CPU_TIME` record of `SURVEY` means what it
says. This page holds the reasoning behind their shape.

*Name: provisional, for the stem `cpu-time-tests`, after the test file it explains. calef names files.*

## Why a mechanism test is not enough

`survey_record_tests` proves the selector mechanism. A record can be asked for, an unknown one is
refused, and the walk is the same whichever record it is. A record that returned a constant zero,
or that charged every tick to the wrong thread, would pass all of it. These tests check the
number. They have their own file because `tests.rs` is the merge hotspot every lane collides in.

## A spinner against a sleeper

CPU time is only meaningful as a comparison. An absolute figure could be wrong by any factor and
still look plausible. So every assertion compares two threads in one domain over one interval. One
is a runaway that never yields, and the other is a child parked in a send that nobody receives. The
runaway must gain and the sleeper must not.

That pair is also the two ends of the accounting choice in DECISIONS §150 (how does a thread's CPU
time reach userspace?). §150 refused `spawn_instant`, because a thread alive for five minutes and a
thread that ran for five minutes would read the same. The two threads in the headline test are the
same age to within the microseconds it took to build the second. An age-based implementation
reports them equal, and the test fails it.

The headline asks for 50 ms, five of the fifty ticks a half-second window holds. The runaway
shares the machine with the test thread's yield loop and with whatever earlier tests left running.
A bound that assumed a core to itself would fail on a busy suite rather than on a bug.

## The window

A window is half a second, fifty timer ticks at the 100 Hz all three architectures run. That is
long enough that a runaway collects the handful of ticks the assertions need, even on a shared
core. It is short against a suite that already spends a second in `force_kill_tests`. It is counted
in timer counts rather than loop iterations, because a yield loop's rate belongs to the host.

## Parity

The counter lives in `sched`, and the increment is in `sched::on_tick`. The only
architecture-specific code in reach is the one-instruction spin loop, which `force_kill_tests`
already keeps for all three. So all three architectures run the same assertions, under DECISIONS
§19 (architectural parity is a tenet). The spin stub is copied rather than shared with
`force_kill_tests`. There it is a teardown fixture and here it is a workload, and a shared item
would make each file's edit the other's problem.

## What the frame check covers

The headline test asks whether the two runs it allocated came back, not whether the machine's free
count did. A force-killed runaway becomes a corpse on a tick on whichever core holds it, so its last
pages can land after the test returns. A neighbor bracketing the global count would then see them.
[Two unowned reds](../load-sensitive-assertions/unowned-reds.md) records that failure.

The check does not cover the child's own pages outside any region, including its current-cpu page.
No region names them once the space is gone. `current_cpu_tests.rs` asserts on that frame directly,
and the whole-boot frame ledger in `testing.rs` would catch a drift.
