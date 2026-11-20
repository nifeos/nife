---
status: BUILT
raised: 2026-09-21
built: 2026-09-21
---
# 527. The `SURVEY` selector, and a thread's placement

*(Number provisional until the merge queue lands it.* **Expect
renumbering**: 524 onward are contested between several in-flight branches, and this lane picked 527
by looking at what was on `main` and at nothing another session can see. The integrator mints the
real number at merge, like every other global name.*)*

Builds the selector calef ruled on 2026-09-21 in the decisions section provisionally numbered 204
(how userspace asks where a thread runs), which amends §150 (how does a thread's CPU time reach
userspace?) inside the window §150 itself named. That section is not on `main` as this block is
written, so it is described here rather than cited, and the citation lands when it does.

**In brief.** `abi::rendezvous::SURVEY` took a cursor and returned three fixed words. It now takes a
**record selector** in the argument that was always a zero, and the third word is the record's. Two
records ship: the run state the method already returned, numbered 0 so that no existing caller
changes, and a thread's **placement**, the cpu it was put on when it started. The placement fact
already existed in the kernel and had no path to userspace, which is what kept the job-mix
supervisor in the kernel.

## The new semantics of the method, which is a system-call-surface change

```text
invoke(cap, SURVEY, cursor, record, 0) -> (next_cursor, tid, word)
```

- No new system call number and no new method number. `SURVEY` is still method 6 on a rendezvous
  capability, and still takes `ENUMERATE` and pointedly not `READ`. What changed is that its second
  argument now means something.
- x0 and x1 are the frame; only x2 belongs to the record. The cursor walk and the tid do not
  depend on the selector, so a caller wanting two facts walks the domain twice and joins on the tid.
  A caller wanting one is unaffected by every record it does not ask for. That is what makes a
  new record cost an existing reader nothing. It is asserted rather than assumed, because the
  cheap way to build a selector is to let each record drive its own walk and that version makes the
  tids unjoinable while passing everything else.
- `abi::survey::record::STATE` is 0. Every caller written before the selector existed passed a
  zero into an argument it believed was padding, so it selects the record it was already reading.
  Backward compatibility on a wire is a claim rather than a hope, so it is pinned by a host test on
  the constant itself and by a kernel test that walks one domain both ways and compares word for
  word.
- `abi::survey::record::PLACEMENT` is 1, and answers a cpu id or `record::NO_CPU`.
- An unknown record is `abi::Error::BadMethod`, refused before the walk begins. The selector is
  part of the method's name, so an unrecognized one gets the refusal an unrecognized method word
  gets; no new error code was needed, and none was added. It is checked before the walk rather than
  at the point the record is extracted because of exactly one case. Against an empty domain a
  check at extraction never runs, the walk falls off the end, and the caller is handed `DONE` and
  prints "no threads" when it actually asked a question this kernel does not understand. A plausible
  wrong answer is worse than an error, which is the ruling this tree already made about a counter it
  could not trust.
- The rights check still runs first. A holder without `ENUMERATE` gets `NotPermitted` whatever
  record it names, so the selector is never a way to probe which records a kernel answers.

## Why a selector rather than a fourth word

§150 ruled that CPU time would arrive as a fourth return register and flagged that sub-choice as the
irreversible one. It was to be overturned before milestone 282 (a thread's CPU time, and the `top` it makes
possible) ships or not at all. 282 is NOT-STARTED, so 2026-09-21 was that moment, used as designed.

The deciding input was a forecast rather than an argument about elegance. A register row holds a
fourth word with no new mechanism, and appending one is the fewest moving parts *if the row stops*.
calef expects a third and a fourth fact, which settles it: a mechanism that must be redesigned at
the sixth field is the wrong mechanism at the fourth. The systems that already went through this
agree, and were read rather than recalled: Linux reached 52 fields in
`/proc/[pid]/task/[tid]/stat` behind a pseudo-file, and Zircon reached 41 topics behind
`zx_object_get_info(handle, topic, buffer, size)`, which is this selector. Nobody grows a register
row.

What §150 still rules is untouched. Tick-sampled, per-thread, scheduled on-CPU time is still
what milestone 282's figure means. Only how it reaches a reader moved.

## Placement, and why it is not "where it is running now"

`record::PLACEMENT` reports the core `sched::pick_spawn_target` chose when the thread started.
`sched::spawn_reporting_placement` has handed that fact to the in-kernel job-mix supervisor since
milestone 240 (the soak reports what happened and not where) as a return value, which is only
available to whoever did the spawning. A userspace supervisor did not do the spawning, and that
missing path is what kept that supervisor in the kernel.

It is placement rather than location, and the reason is measured. Keeping the core a thread is
on *now* means a store in the context switch, which is the hottest line of the hottest function. The
kernel has exactly that field, `Thread::last_cpu`, and it is behind a soak-build feature gate.
Shipping it unconditionally cost 5.7% of `ipc_fastpath`'s footprint on aarch64 (5788 ->
6120 bytes), over milestone 132 (the fast path's footprint, and a gate)'s 5% bound, with
riscv64 and x86_64 growing 4.7% and 4.6% behind it. A placement is one store per thread
creation, on a path that is cold by definition, so the same information that was too expensive to
keep continuously is free to keep once. `Thread::placement` is therefore unconditional where
`last_cpu` is not, and it is written at `sched::spawn_on` and `sched::start_thread_control_block`
rather than inside `place_on`, which is also the wake path.

The cost is recorded where a reader meets the record and in `notes/process-view.md`'s `BUGS`: a
thread stolen onto another core, or woken onto its waker's, goes on reporting where it was placed.

## The online set, which is the one way to misuse this

**The cpu id is a name, not an index.** The online set is not `0..n`. On the VisionFive 2 it is
`{1, 2, 3}`, because slot 0 is an M-mode monitor core with no MMU. Treating the count as an
index put `init` into a parked core's inbox and took three boots to diagnose on first silicon. That
is why `crates/cpu_set` exists, and a record handing userspace a core id is a fresh invitation to
the same bug.

So the answer, stated rather than left implied: a placement census needs no online mask at all.
Key a tally by the id the record returns and never iterate a range. The ids a census observes are by
construction a subset of the online set, and that subset relation is asserted in the suite rather
than asserted in prose. A census built that way is correct on both machines.

What it cannot do is show an online core with nothing on it, because such a core appears in no
thread's record, and it cannot tell that case from a parked core. Nothing in this tree gives
userspace the online mask, and this record deliberately does not smuggle it out: the mask is a fact
about the machine and a survey answers questions about a domain. That gap is a `BUGS` entry in
`notes/process-view.md` and a proposed milestone below.

## Authority, written where the reader meets the method

§150 already weighed that a viewer holding `ENUMERATE` learns an aggregate about threads it cannot
otherwise name, and accepted it for a continuous CPU-time counter. A placement is strictly less than
what was accepted: **one bounded value out of at most 64, written once and never again**, against a
counter that moves continuously and can therefore be differenced into a timing channel. A viewer
that can already see a tid and a run state learns which of a handful of cores that thread was put
on, and learns nothing whatever about a thread outside the domain it was handed.

That reasoning is in the dispatcher's own comment on the `SURVEY` arm rather than only in a
decisions file two hops away. That is rung three of the ladder applied to an argument a reader
needs at the point they are reading the code.

## What milestone 282 adds, which is the point of building it this way

One constant (`record::CPU_TIME`), one line in `abi::survey::record::is_known`, and one arm in
`sched::survey_supervised`. Nothing that reads `STATE` or `PLACEMENT` is touched, and no register
moves. 282's own block still describes step 2 as *"a fourth word on `abi::rendezvous::SURVEY`'s
return"*; **that sentence is now stale and this lane deliberately did not edit it**, because a
developer does not touch another milestone's block. It is named in this lane's report as an edit the
integrator owes at merge.

## Parity

aarch64, riscv64 and x86_64, proven by the same suite with no scope note, and that was confirmed
rather than assumed. Placement is `sched`'s: `pick_spawn_target` samples two online cpus' runnable
counters and `place_on` enqueues onto the winner, with no line of either under `kernel/src/arch/`.
The seven tests in `kernel/src/user/survey_record_tests.rs` run literally unchanged on all three,
plus the x86_64 UEFI-firmware leg.

## BUGS

- This moves the growth problem from registers to records rather than abolishing it. Each record
  layout is still a wire format, and a new record value is as expensive to un-ship as a fourth word
  would have been. What is bought is that adding one requires no change to what an existing reader
  already parses.
- A reader that wants two facts pays two walks, and the two walks are separate snapshots of a
  domain that may change between them. The join on tid is safe (a tid that vanished is simply absent
  from the second walk), but a thread born between the walks appears in one and not the other. The
  same `readdir` bargain `SURVEY` already took, one axis over.
- **The record names are provisional**, `record` and `STATE` and `PLACEMENT` and `NO_CPU` alike, as
  is `Thread::placement` and `user_mode_runtime::survey_record`. Zircon calls these *topics*;
  *record* was chosen for being the noun for "the fields you get back" rather than for the thing you
  ask about, but calef names public items.
- `ps` and `pgrep` still ask only for the state record, so nothing at the prompt displays a
  placement yet. `ps::collect` is hard-wired to that record, which is why this milestone's tests
  drive the cursor walk directly instead of through the real program's loop, and that is a departure
  from `survey_tests`' discipline worth knowing about before copying it.

## Follow-on

- **Milestone 564.** The missing half is milestone 564 (a userspace program cannot learn which
  cpus are online),
  `design/roadmap/0564-a-userspace-program-cannot-learn-which-cpus-are-online.md`. A placement census
  cannot show an idle core, and the only way a program can currently guess at the online set is the
  range that is wrong on real silicon. The shape is a machine-description question rather than a
  survey one, so it is not a record and does not belong behind this selector.
- **Recorded.** That `ps` and `pgrep` still ask only for the state record, in this block's `BUGS`.
  Whether a placement column appears, and what it is called, is a naming call as much as a build
  one. It belongs with milestone 282 (a thread's CPU time, and the `top` it makes possible)'s
  `top` question rather than ahead of it.
- **Recorded.** The per-thread page for the self case, in `notes/process-view.md`'s `BUGS`. The same
  2026-09-21 ruling gave a thread observing *itself* an `rseq`-shaped page rather than a selector,
  because the consumer there is an allocator reading its own cpu on every allocation. Not built and
  not this milestone, and recorded so a reader does not mistake the selector's scope for it.

## Index row

 `abi::rendezvous::SURVEY` took a cursor and returned three fixed words. It now
takes a record selector in the argument that was always a zero, and only the third word belongs
to the record. So the cursor and the tid are the same for every record and a new fact costs an
existing reader nothing. `record::STATE` is 0 precisely so that every pre-selector caller, which
passed a zero as padding, keeps reading what it read. An unknown record is `BadMethod`, refused
before the walk so that a bad selector against an empty domain is a refusal rather than a `DONE` a
reader prints as "nothing here". The one record shipped is `record::PLACEMENT`, the cpu a thread was
put on at start, a fact `spawn_reporting_placement` has held since milestone 240 with no path out.
It is placement rather than live location because the live answer needs a store in `schedule()`'s
switch, which cost 5.7% of `ipc_fastpath`'s footprint and is why `Thread::last_cpu` is soak-gated. A
census keyed by the returned id needs no online mask, which is asserted as a subset relation rather
than promised, and the id is a name rather than an index because the online set is `{1, 2, 3}` on
real silicon. Milestone 282 now adds one constant and one match arm instead of a fourth register.
