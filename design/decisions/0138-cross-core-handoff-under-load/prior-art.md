# §138 appendix: the prior art on migration and affinity

An appendix of [§138 (how a saturated workload is made to hand threads across
cores)](../0138-cross-core-handoff-under-load.md). The main page is complete without it. This file
holds what seL4, L4Re, Barrelfish and QNX do, and how Linux authorizes affinity.

Every quote below was fetched and read, and every one was re-checked against the downloaded source
before it went in this file: the seL4, Barrelfish, L4Re and QNX material on 2026-09-01, the Linux
material on 2026-09-02. The seL4 manual quotes are from the PDF's extracted text (the ligature in
"affinity" is why a naive grep misses them); the source quotes are from `raw.githubusercontent.com`.
Every Linux claim in this document names the version it was checked against, because a function
name in that tree is a fact with an expiry date and three of the ones in this argument expired in
v6.10.

## seL4 does not migrate threads at all, and says why

This is the finding that reframes the whole question, because nife's measured behavior is seL4's
deliberate design. From Elphinstone, Zarrabi, Danis, Shen and Heiser, *An Evaluation of
Coarse-Grained Locking for Multicore Microkernels*, https://arxiv.org/pdf/1609.08372, section 3.3:

> This partitioned scheduling implies that threads can only migrate between cores if explicitly
> requested by the user, which is consistent with seL4's general philosophy of having all resource
> management under user control (and also helps reasoning about real-time properties).

Corroborated in the kernel source: `chooseThread` in `src/kernel/thread.c` reads only
`NODE_STATE(ksReadyQueues)`, this core's own queues, and an idle core runs its idle thread rather
than pulling from a neighbor. There is no stealing path and no rebalancer. seL4 does have one
implicit migration, and it follows authority rather than load: the manual's section 6.1.11 says
*"Passive threads will run on the CPU node that the scheduling context was configured with, and will
be migrated on IPC"*, because a scheduling context is donated over `seL4_Call`, so a passive server
runs on its caller's core.

## seL4 has two answers on affinity, and the newer one makes the core a field of a capability

Non-MCS. The manual, section 6.1.2: *"the default thread affinity is the node the thread's TCB
object was created on, and `seL4_TCB_SetAffinity()` can be used to explicitly set the affinity. On
MCS configurations, the affinity is derived from the scheduling context object."* The authority is
the TCB capability and nothing else: `decodeSetAffinity` in `src/object/tcb.c` checks only that the
core number is in range. That is an asymmetry seL4 itself later fixed, because the same kernel
gates *priority* with a second capability (the manual, section 6.1.6: a thread changing another's
priority *"must provide a thread capability from which to use the MCP from"*) and gates *placement*
with nothing.

MCS, and this is the precedent worth having. The manual, section 6.1.10: *"To populate a
scheduling context with parameters, one must invoke the appropriate SchedControl capability, which
provides access to CPU time management on a single node. A scheduling control cap for each node is
provided to the initial task at run time. Threads run on the node that their scheduling context is
configured for."* Under MCS, `seL4_TCB_SetAffinity` does not exist at all; it is compiled out.

The capability's whole content is the core. `include/object/structures_64.bf`, verified by fetching
it:

```
block sched_control_cap {
    field core     64

    field capType  5
    padding        59
}
```

and `src/object/schedcontrol.c` reads the core off the capability rather than off the message
(`cap_sched_control_cap_get_core(cap)`). So there is no message that can name a core. You choose
a core by choosing which capability you invoke, delegating "may run on core 3" is a copy of one
capability, and revoking it is a revoke. That is rung one of `AGENTS.md`'s ladder: the wrong state
is unrepresentable rather than checked.

## L4Re shows the set-shaped version of the same idea, narrowable at mint time

`L4::Scheduler` is a kernel object, and https://l4re.org/doc/classL4_1_1Scheduler.html describes
`run_thread` as launching *"a thread on a CPU determined by the scheduling parameter `sp.affinity`"*.
One invocation carries a CPU set, a priority and a timeslice together. The authority is possession
of the Scheduler capability, and Moe's documentation
(https://l4re.org/doc/l4re_servers_moe.html) shows it being minted narrower: a scheduler proxy is
created with a priority limit, a priority offset and a CPU bitmap, and *"`L4::Scheduler` objects can
be only created through the user factory provided by Moe to the initial application. Other factory
instances cannot create this object."* Hold, narrow, delegate, in the tree's own vocabulary.

Whether Fiasco's kernel ever rebalances on its own was **not established**; the documentation
describes migration only as an explicit act and no statement either way was found. Recorded as
unverified rather than guessed.

## Barrelfish dissolves the authority question by making placement structural

*Barrelfish Architecture Overview*, TN-000, https://barrelfish.org/publications/TN-000-Overview.pdf:
*"Dispatchers do not migrate between cores."* A domain that wants to run on a second core creates a
second dispatcher there, and thread migration is a userspace protocol between the two dispatchers'
schedulers. There is no affinity call because there is nothing to set, and the right to run on a
core is the right to create a dispatcher on it.

## QNX and Linux, for the authority question specifically

QNX (`ThreadCtl`, https://www.qnx.com/developers/docs/8.0/com.qnx.doc.neutrino.lib_ref/topic/t/threadctl.html)
requires no privilege at all and instead makes the dangerous case unreachable: its runmask control
sets the affinity of *the calling thread*, and doing it to a thread in another process is not
supported. The legal masks are further constrained to boot-defined clusters, which is a bounding set
expressed as static configuration.

Linux is the one to argue against, and it is walking away from itself. `sched_setaffinity(2)`
(man-pages 6.9.1, fetched 2026-09-02) needs *"an effective user ID equal to the real user ID or
effective user ID of the thread identified by `pid`, or it must possess the `CAP_SYS_NICE`
capability in the user namespace of the thread `pid`"*: one coarse ambient bit that also gates
`nice` and I/O priority, with nothing bounding which CPUs may be requested. The bounding lives in
a different subsystem, and the manual page is candid that it does not fail loudly:
*"These restrictions on the actual set of CPUs on which the thread will run are silently imposed by
the kernel."* cgroup v2 chose the other way, both in failing loudly on the exclusivity rule and in
making the carve-out right non-delegable: `cpuset.cpus.partition` is
*"owned by the parent cgroup and is not delegatable"*, which is the copy-versus-mint distinction a
capability system gets for free.

Linux's balancer itself is in [`linux.md`](linux.md).
