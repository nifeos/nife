# §138 appendix: how Linux v6.12 balances

An appendix of [§138 (how a saturated workload is made to hand threads across
cores)](../0138-cross-core-handoff-under-load.md). The main page is complete without it. This file
holds how Linux balances load, the push for running tasks, the domain hierarchy, and the control
that turns it off.

## Linux balances continuously, at four moments, because nothing ever tells it anything

Version, because "Linux does X" without one rots. Everything below is Linux v6.12
(`Makefile`: `VERSION = 6`, `PATCHLEVEL = 12`), fetched from
`raw.githubusercontent.com/torvalds/linux/v6.12` on 2026-09-02 and read. Three of the names a
reader may have in their head were changed in v6.10 and are wrong for any tree newer than that:
`rebalance_domains` became `sched_balance_domains`, `newidle_balance` became
`sched_balance_newidle`, and `scheduler_tick` became `sched_tick`. All three carry the old names in
v6.9 and the new ones in v6.10, checked in both. The rot is not hypothetical either: the comment
above `sched_balance_softirq` in v6.12 still says `scheduler_tick()`, two releases after that
function took its new name.

**EEVDF did not change this.** `Documentation/scheduler/sched-eevdf.rst`: *"The Linux kernel began
transitioning to EEVDF in version 6.6 (as a new option in 2024), moving away from the earlier
Completely Fair Scheduler (CFS)"*. Every function named below is still in `kernel/sched/fair.c` in
v6.12 under `fair_sched_class`. That EEVDF replaced the *pick* rather than the *balancer* is an
inference from the file rather than a quoted claim, and is marked as one.

1. Periodically, off the tick. `sched_tick()` in `kernel/sched/core.c` ends, under
`CONFIG_SMP`, with `rq->idle_balance = idle_cpu(cpu); sched_balance_trigger(rq);`, and
`sched_balance_trigger` in `kernel/sched/fair.c` raises the softirq only when the deadline has
arrived (`if (time_after_eq(jiffies, rq->next_balance)) raise_softirq(SCHED_SOFTIRQ);`).
`Documentation/scheduler/sched-domains.rst` states the whole chain:

> In kernel/sched/core.c, sched_balance_trigger() is run periodically on each CPU through
> sched_tick(). It raises a softirq after the next regularly scheduled rebalancing event for the
> current runqueue has arrived. The actual load balancing workhorse,
> sched_balance_softirq()->sched_balance_domains(), is then run in softirq context (SCHED_SOFTIRQ).

2. When a CPU is about to go idle. The comment on `sched_balance_newidle` in
`kernel/sched/fair.c`: *"sched_balance_newidle is called by schedule() if this_cpu is about to
become idle. Attempts to pull tasks from other CPUs."* Its two callers are `balance_fair` and the
`idle:` arm of `pick_next_task_fair`, both in `kernel/sched/fair.c`.

3. On behalf of a tickless idle CPU, by a different CPU. A CPU with its tick stopped cannot
reach moment 1, so someone else reaches it for them. The comment on `sched_balance_softirq`:

> This softirq handler is triggered via SCHED_SOFTIRQ from two places:
>
> - directly from the local scheduler_tick() for periodic load balancing
>
> - indirectly from a remote scheduler_tick() for NOHZ idle balancing through the SMP cross-call
>   nohz_csd_func()

and inside it, *"If this CPU has a pending NOHZ_BALANCE_KICK, then do the balancing on behalf of
the other idle CPUs whose ticks are stopped."*

4. At wake-up, fork and exec. `select_task_rq_fair`'s comment: *"Select target runqueue for the
waking task in domains that have the relevant SD flag set. In practice, this is SD_BALANCE_WAKE,
SD_BALANCE_FORK, or SD_BALANCE_EXEC. Balances load by selecting the idlest CPU in the idlest group,
or under certain conditions an idle sibling CPU if the domain has SD_WAKE_AFFINE set."* The
affine heuristic is deliberately cheap, and says so: *"The purpose of wake_affine() is to quickly
determine on which CPU we can run soonest. For the purpose of speed we only consider the waking and
previous CPU."*

And a push, for the thread a pull cannot reach. A running task is not on a run queue to be
stolen from, so `sched_balance_rq` asks `need_active_balance` and, if it says yes, queues work on
the busiest CPU with `stop_one_cpu_nowait`: *"active_load_balance_cpu_stop is run by the CPU
stopper. It pushes running tasks off the busiest CPU onto idle CPUs."* The stopper thread really is
the migration thread of the folklore; `kernel/stop_machine.c` names it `"migration/%u"`.

All of it hangs off a per-CPU domain hierarchy, and every level carries its own constants.
`sched-domains.rst`: *"Balancing within a sched domain occurs between groups"*, and
`sched_balance_domains` *"checks each scheduling domain to see if it is due to be balanced, and
initiates a balancing operation if so"*, walking `for_each_domain(cpu, sd)` up the parent chain.
The tuning surface is four fields per level in `include/linux/sched/topology.h`:

```
	unsigned long min_interval;	/* Minimum balance interval ms */
	unsigned long max_interval;	/* Maximum balance interval ms */
	unsigned int imbalance_pct;	/* No balance until over watermark */
	unsigned int balance_interval;	/* initialize to 1. units in ms. */
```

One correction to the shape of the hierarchy, which a maintainer stated from memory as "SMT,
then cache-sharing cores, then NUMA nodes". `kernel/sched/topology.c` names three default levels,
not two, and NUMA is not one of them:

> By default (default_topology[]) these include:
>
>  - Simultaneous multithreading (SMT)
>  - Multi-Core Cache (MC)
>  - Package (PKG)
>
> Where the last one more or less denotes everything up to a NUMA node.

NUMA levels are built above `PKG` from the distance table, and an architecture may replace the
array entirely with `set_sched_topology()`.

What this is evidence of, and the re-cut of the maintainer's three-moment summary, stay on [the main
page](../0138-cross-core-handoff-under-load.md#the-prior-art-read-rather-than-recalled).

**And Linux itself treats "do not balance this" as a supported request rather than an escape
hatch**, which is the half of the comparison that argues against reading its balancer as
unconditional. `cpuset.cpus.partition` accepts `"isolated"`, documented in
`Documentation/admin-guide/cgroup-v2.rst` as *"Partition root without load balancing"*, and cgroup
v1's `cpuset.sched_load_balance` is the older spelling of the same control.
