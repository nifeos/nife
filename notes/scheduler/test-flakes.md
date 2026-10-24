# Two test flakes the scheduler's migration exposed

*Name: provisional.*

An appendix to [notes/scheduler.md](../scheduler.md), moved there on 2026-10-11 (UTC) under §212
(a prose budget). Each was a test asserting something the scheduler never promised, and each is recorded so the
next person who sees it does not debug the branch in front of them.

## A found flake, fixed: a per-CPU test was asserting an affinity nothing promised

Found while running the gates for milestone 35 (prove the DMA-confinement boundary), on a tree whose diff cannot touch scheduling.
The diff held `#[cfg(kani)]` harnesses, the DMA validator's layout constants, and the IOMMU domain
builder. That is what made it clearly pre-existing rather than a regression.

`kernel::cpu::tests::boot_cpu_percpu_is_reachable` opened with `assert_eq!(id(), arch::boot_cpu_id())`.
So it asserted the test case is executing on the boot core. On aarch64 `boot_cpu_id()` is the
constant 0. `id()` is derived from `TPIDR_EL1`, which each core sets once at boot and which no
context switch saves or restores. So `id() == 1` means the code was running on core 1, not that
a pointer was stale. Nothing promises otherwise. With four cores online and §28 (SMP placement)'s stealing, a
secondary core may pull the test thread. Then the assertion fails on an affinity the scheduler
never offered.

It was observed once in four consecutive full-suite runs on an unchanged tree (`left: 1, right: 0`).
So it was roughly a one-in-four flake on this machine. When it fired, it failed the aarch64 half of
`script/test`.

It was resolved by weakening the test to the property its own doc comment always described. It is
now `cpu::tests::percpu_is_self_consistent_on_whatever_core_we_run`. `current()` points at
`PERCPU[id()]` and no other block. And `of(boot)` reaches the boot core's block by index, which is
what the cross-core paths (IPI, stealing) rely on. That is true on every core, so it is stronger
coverage rather than weaker. Under §28 placement the suite scatters, and the property gets exercised
on several cores over a run instead of only the boot core.

The rejected alternative was giving kernel test cases boot-core affinity to keep the original
assertion. It reads like the more rigorous option and is the worse trade. It buys one assertion back
at the price of running the entire suite on one core. That is exactly where the placement bugs §28
introduced would hide. A harness that avoids the scheduler it is meant to test is not a harness. The
general rule is this. When a test fails because the system legitimately does something the test did
not expect, check whether the *test's* claim was ever promised. Only then treat the system's
behavior as the defect.

## The bounded-yield tests fail under host CPU contention, and the control says so (2026-07-30)

Recorded so the next person who sees it does not spend the afternoon reading a diff. Several
`kernel::sched::tests` cases wait for something with a fixed number of `yield_now()` calls. Others
assert a count has settled: `threads_round_robin` ("thread 2 never ran"),
`an_interrupt_that_arrives_before_the_wait_is_not_lost`, `other_threads_run_while_one_is_blocked`,
`a_finished_thread_is_reaped_and_its_memory_returned`. Under TCG the guest's four cores are host
threads, so a fixed yield budget is a wall-clock budget in disguise. When the host is busy the
budget runs out before the work does.

It was observed during milestone 37 (prove), under DECISIONS §34 (RedoxFS primary) condition 1.
Three different tests from that list failed across four full-suite runs. Every one of them was in a
module that executes before any of that milestone's code exists. That already ruled the diff out
structurally. The confirmation was cheaper than the reasoning: unmodified `origin/main`, same
machine, same minute, failed too (a fourth test, the reaper's count). Meanwhile a QEMU from another worktree had been holding 200% of the host for 43 minutes.

It was measured again on 2026-08-03, because milestone 72 (a lost wakeup)'s confirmation loop ran
the recipe that provokes them. Four host burners ran riscv64 twenty times, with three failures, all
from this list (`a_thread_that_never_yields_is_preempted_anyway`,
`a_blocked_waiter_wakes_with_an_error_when_its_endpoint_is_revoked`, and the sibling
`threads_round_robin`). That test's "thread {i} never ran" was `sched.rs:2709` then; 128d9492a made
it a clock-bounded wait on 2026-08-03, and the assertion is `kernel/src/sched.rs:7686` as of
2026-10-11. 15% under load, 0% quiet. They are distinguishable from a real hang at a glance: these
fail in about 23 s with a named assertion, a lost wakeup fails at 60 s with a thread dump.

Two things follow. A run that fails one of these is not evidence about the branch until it has
been seen on a quiet machine or contradicted by a control run. A control run costs ten minutes and
settles it. And the standing fix is the one the main page argues for. These bounds should be
progress-based or wall-clock with slack, not a yield count. A yield count measures the host's spare
capacity and calls it the scheduler's behavior.
