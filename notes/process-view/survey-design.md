# SURVEY: the selector, placement, membership and the cursor

*Name: ratified 2026-10-18 (calef, "survey-design.md") Refused `survey-method`.*

An appendix to [notes/process-view.md](../process-view.md), which gives the method's signature, its
rights and its states, and summarizes this page. These are the design arguments behind the rest of
the method.

calef ruled on 2026-09-21 that observing another thread is a selector. The argument to `SURVEY`
that was always a zero now names which per-thread record the caller wants, and that record's word
comes back in x2. A new fact becomes a new `abi::survey::record` value; it never becomes a fourth
return register.

The deciding input was a forecast rather than an argument about elegance. A register row holds a
fourth word with no new mechanism at all, and appending one is the fewest moving parts *if the row
stops*. calef's answer was that he expects a third and a fourth fact, which settles it: a mechanism
that has to be redesigned at the sixth field is the wrong mechanism at the fourth. The systems that
already went through this agree, and were read rather than recalled: Linux reached 52 fields in
`/proc/[pid]/task/[tid]/stat` behind a pseudo-file, and Zircon reached 41 topics behind
`zx_object_get_info(handle, topic, buffer, size)`, which is this selector. Nobody grows a register
row.

x0 and x1 are the frame; only x2 belongs to the record. The cursor walk and the tid are the same
whichever record is asked for. So a caller that wants two facts walks the domain twice and joins on
the tid, and a caller that wants one is unaffected by every record it does not ask for. That is what
makes adding a record cost an existing reader nothing. It is asserted rather than assumed
(`system_tests/src/user/survey_record_tests.rs`). The cheap way to build a selector is to let each
record drive its own walk, and that version makes the tids unjoinable while passing everything else.

`record::STATE` is 0, which is the backward-compatibility claim and not a coincidence. Every
caller written before the selector existed passed a zero into an argument it believed was padding,
so it selects the record it was already reading, and not one of them changes. That is a claim about
a wire rather than a hope, so it is pinned by a test on the constant itself.

An unknown record is `BadMethod`, refused before the walk begins. The selector is part of the
method's name, so an unrecognized one gets the refusal an unrecognized method word gets, and no new
error code was needed. The check is before the walk rather than at the point the record is extracted
because of one case. Against an empty domain, a check at extraction never runs, and the walk falls
off the end. The caller is handed `DONE` and prints "no threads", when what really happened is
that it asked a question this kernel does not understand. That is the same failure this method
already refuses for an unauthorized viewer, one axis over.

The rights check still runs first, so a holder without `ENUMERATE` gets `NotPermitted` whatever
record it names. A selector is never a way to probe which records exist.

## The records, and what milestone 282 adds

| record | value | x2 |
|---|---|---|
| `record::STATE` | 0 | one of `abi::survey`'s four state codes |
| `record::PLACEMENT` | 1 | the cpu the thread was **placed on** when it started, or `record::NO_CPU` |

Milestone 282 (a thread's CPU time, and the `top` it makes possible) is NOT-STARTED and is the next
consumer. Under §150 (how does a thread's CPU time reach userspace?)'s original ruling it was a
fourth return word; under the selector it is one
new constant, one arm in `sched::survey_supervised`, and one line in `record::is_known`. Nothing
that reads `STATE` or `PLACEMENT` is touched. §150's substance is untouched too: tick-sampled,
per-thread, scheduled on-CPU time is still what that figure means.

## Placement, and why it is not "where it is running now"

`record::PLACEMENT` reports the core `pick_spawn_target` chose when the thread started. The fact
already existed in the kernel: `sched::spawn_reporting_placement` has handed it to the in-kernel
job-mix supervisor since milestone 240 (the soak reports what happened and not where), as a
return value, which is only available to whoever
did the spawning. A userspace supervisor did not do the spawning, and that missing path is what kept
that supervisor in the kernel.

It is placement rather than location, and the reason is measured rather than tidy. Tracking the
core a thread is on *now* means a store in the context switch, which is the hottest line of the
hottest function. The kernel has exactly that field, `Thread::last_cpu`, and it is behind a
soak-build feature gate. Shipping it unconditionally cost 5.7% of `ipc_fastpath`'s
footprint on aarch64 (5788 -> 6120 bytes). That is over the 5% bound of milestone 132 (the fast
path's footprint, and a gate so Mach's mistake cannot happen quietly), with riscv64 and x86_64
growing 4.7% and 4.6% behind it. A placement is one store per thread creation, on a path that is
cold by definition, so the same information that was too expensive to keep continuously is free to
keep once. `Thread::placement` is therefore unconditional where `last_cpu` is not.

The cost of that choice is stated where a reader meets the record: a thread stolen onto another
core, or woken onto its waker's (DECISIONS §28 (SMP placement: local wakes), sub-point 2), still reports where it was placed. A reader
wanting "now" does not have it here.

The authority argument, and it is deliberately short. §150 already weighed that a viewer holding
`ENUMERATE` learns an aggregate about threads it cannot otherwise name, and accepted it for a CPU
time counter. A placement is strictly less than what was accepted: one bounded value out of at most
64, written once and never again, against a counter that moves continuously and can therefore be
differenced into a timing channel. A viewer that can already see a tid and a run state learns which
of a handful of cores that thread was put on, and learns nothing whatever about a thread outside the
domain it was handed.

## The cpu id is a name, not an index

The one way to misuse this record, and it is the bug `crates/cpu_set` exists to kill. The online
set is not `0..n`. On the VisionFive 2 it is `{1, 2, 3}`, because slot 0 is an M-mode monitor core
with no MMU. Treating the count as an index put `init` into a parked core's inbox, and took three
boots to diagnose on first silicon (notes/visionfive2.md).

So: key a per-core tally by the id this record returns, and never iterate a range. A census built
that way needs no online mask at all, because the ids it observes are by construction a subset of
the online set, and that subset relation is asserted rather than asserted-in-prose
(`the_cpus_a_census_observes_are_a_subset_of_the_online_set`). That is the whole reason this record
can be handed to a userspace supervisor on its own.

What a census cannot do is tell an online core with nothing on it from a core that is not online,
because it never saw either. That is a question about the machine rather than about a domain, and
nothing in this tree answers it for userspace today; see [limitations.md](limitations.md).

## Membership is the relationship, and it is proved

The kernel walks its thread table and reports an entry when `capability::survey_includes(fault_ep,
invoked_ep)` says so, which is `matches!(fault_ep, Some(ep) if ep == invoked_ep)` and nothing else.
Three Kani harnesses in `crates/capability`:

- inclusion holds if and only if the thread's recorded endpoint is the invoked one. Both
  directions matter here, unlike for the reap: one is confinement (a stranger is never shown) and the
  other is truthfulness (a member is never hidden, so a missing row means gone rather than
  concealed);
- the view and the reap have the same scope, for every liveness, so the set a monitor sees and
  the set a supervisor may collect from cannot diverge;
- plus §32's two existing reap harnesses, which this reuses rather than restates.

## Why one entry per call

`SCHED` is given back between entries. A survey that held it for a whole domain would let a
userspace program decide how long the scheduler is locked, which is a latency hole a program could
open on purpose.

The cost is that a survey is a sequence of snapshots rather than one. The cursor is a *slot
index*, not a position in a filtered sequence (`generational_table::Table::iter_from`). That is what
makes the resume safe. The entry that was at slot `k` is the only thing that can be at slot `k`, and
if it died, `k` is empty rather than somebody else's thread. So a resumed walk never reports a member twice
and never resolves a cursor to the wrong thread. It can miss a member born into an already-passed
slot, and it can list a member that dies before the table prints. That is `readdir`'s bargain, taken
knowingly.
