---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-28
ratified_by: calef
---

# 167. What a profiling session's grant names

Ruled 2026-10-28 (UTC): C, a profiling session's grant names a supervision subtree (the SURVEY
relation) with a named counter set attached; the counter set is held from day one; cross-thread
counter reads are accepted as a deliberate widening over §139. Unblocks milestone 147.

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote
down)'s lane. That lane found milestone 147 (a profiler that holds exactly the counters it was
granted) gated on `MILESTONE 75, DECISION` naming no decision, when the prior question its gate
defers to has been answered since 2026-09-02. [§139 (who may read the cycle
counter)](0139-cycle-counter-authority.md) (who may read the cycle counter, and by what authority)
took it; 147's gate never said so, and §139's own text says it does **not** cover what 147 needs.
*(Section number provisional until the merge queue lands it.)*

## What §139 settled, and why 147 is still owed a decision

§139 chose a per-thread grant enforced at the context switch, with the grant a field in the
spawn manifest rather than a method on a live thread. It considered milestone 147 explicitly, and
declined to let it argue for the live form:

> 147 wants *this profiler may read that subtree's counters*, which is cross-thread authority with a
> named target, and neither shape here provides it.
>
> -- design/decisions/0139-cycle-counter-authority.md

So the prior question is closed and the second consumer's question is untouched: §139 grants a
thread the right to read its own counter, and a profiler wants to read somebody else's.

## What is being decided

What a profiling session's grant names, and whether cross-thread counter reads exist at all.
Three parts:

1. The target. A subtree, a single thread, or nothing (the profiler can only read itself).
2. The counter set. One counter, or a set named at grant time.
3. Whether the set is discovered rather than assumed, since some silicon does not report the
   events a session would want to name.

## Why this is a stronger claim than "disabled by default"

Every HPC profiler surveyed reads counters through host-OS ambient authority. `perf_event_open`
targets any pid the caller's privilege reaches, gated by `perf_event_paranoid` (a **global sysctl,
not a per-target grant**) or `CAP_PERFMON`, which is a capability in name and "root, or nothing" in
practice. A profiling tool with that access on a shared node can in principle sample a neighboring
tenant's job, which is why `perf_event_paranoid` exists and why HPC centres frequently run it at
`-1` for exactly these tools.

**The independent confirmation matters because it is not ours.** Brown, *RISC-V for High Performance
Computing* (CUG '25, ACM 3757348.3757367) names immature profiling tooling as a high-priority action
item for the whole RISC-V and HPC community, and attributes it partly to hardware: *"the hardware
events that are made available by RISC-V CPUs."* That clause is why part 3 above is in this decision
rather than in the implementation.

## What this tree already does in the analogous case

Narrowing by subtree is built and is the model. `rendezvous::SURVEY` (milestone 126) walks
exactly the supervision subtree whose fault endpoint this is, needs no second bookkeeping, and is
authorized by the same relationship `REAP` is ([§32](0032-reap-without-build.md), a supervisor may
collect a corpse without being able to build one). A `ps` launched from a shell sees the shell's
children and nothing else. A profiler is the same shape with a different verb, which is the
strongest argument available here: the kernel already maintains the relation the grant would name.

And discovery-rather-than-assumption already has a pattern. Milestone 74's `Isa` is built at
boot by probing what is present rather than assuming a fixed catalog. A counter-set capability
should name only the events the running silicon reports, and a probe for an unsupported event should
refuse cleanly rather than read zero.

## The options

| | what the grant names | cost |
|---|---|---|
| **A** | **Nothing new.** A profiler reads its own counters under §139's per-thread grant and no more. | Free, already built, and refuses the feature: a profiler cannot profile a job it did not become. |
| **B** | **A single target thread.** | Smallest cross-thread authority. A profiler of a multi-threaded job needs one grant per thread and has no way to follow a thread it did not know about. |
| **C** | **A supervision subtree**, the `SURVEY` relation with a counter set attached. | Matches what a profiler actually wants, reuses a relation the kernel keeps, and `caps <profiler>` can print exactly which processes it may read. Adds cross-thread reads, which §139 deliberately did not open. |

Recommendation: C, with the counter set held from day one. Milestone 74's scope note refused to
become a profiling framework *"for a second consumer"*, and this is that second consumer arriving on
schedule. Holding a set rather than one counter from the start is the restraint 74 was saving:
it costs nothing now and avoids redesigning the type the day a second event is asked for. The set is
a capability's contents, never an ambient enable bit.

What C costs, stated rather than implied: cross-thread counter reads are a larger side-channel
surface than one thread reading its own, and §139 closed that door on purpose. Opening it for a
profiler is a real widening and should be ruled as one rather than inherited.

## How reversible it is

**The grant shape is surface and is not reversible.** What *is* reversible is the ordering: A is
the current state and costs nothing to keep while the answer waits.

## What is blocked until this is answered

Milestone 147, and its demonstration: two confined workloads on one board, a profiler holding a
capability over one of them, and a negative control where it is refused at the type level on the
other.

Not blocked, and worth separating so they do not get tangled in: the sample-reporting wire format,
whether sampling or counting is the first mode, and multi-node aggregation, which this system has in
no form. Sampling needs an overflow interrupt path nothing here opens.

And the side-channel argument is from the literature, not from this board. Whether a confined
nife process can distinguish a neighbor's cache behavior through any channel this kernel leaves
open is a claim milestone 43's audit lens should aim at, and neither 147 nor this decision attempts
it.
