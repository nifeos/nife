# A second lost wakeup: the missed reschedule poke (closed)

*Name: provisional.*

An appendix to [notes/scheduler.md](../scheduler.md), moved there on 2026-10-11 (UTC) under §212
(a prose budget). It is the full record of the missed-SGI bug of 2026-08-28: the dump, the fix and the proof.

**`spawn_on` asked "is this target my own core?" twice, on either side of a window in which the
asking thread can be stolen onto a different core.** When the two answers disagreed the placement
went into a remote core's inbox and the reschedule SGI that makes an inbox get drained was skipped,
so the thread sat `Ready` forever beside four idle cores. Found 2026-08-28 on the generic `rv64`
CPU-matrix leg of PR #574's run 33147291785, whose diff is a shell script, a baseline file and a
note, and so could not have caused it.

## It shares a watchdog message with the section above and is a different bug

That is the whole reason this subsection leads with the comparison. The message
(`WATCHDOG: no progress for ~60 s. Every core idle, every thread blocked: a lost-wakeup hang.`) is
the *instrument*, not the diagnosis, and the first reading of this failure in the wild was "that
known riscv64 class recurring". The dump refutes it in four fields, against the one recorded by
milestone 72, "A lost wakeup that a hundred leaked threads may be causing":

| | Milestone 72's dump | This one |
|---|---|---|
| Threads | 101 | **120** |
| Endpoints | 109 | **119** |
| Thread states | every one `Blocked` | **one `Ready`**, 115 `Blocked`, 4 idle threads `Running` |
| `wake_pending` | false everywhere | false everywhere |
| Inboxes | **all four empty** | **`core 2: inbox_len=1`**, the other three empty |
| Hung in | `reclaim_frees_a_started_then_exited_childs_regions` | `a_frame_capability_shares_a_page_and_a_read_only_view_cannot_write_it` |

A `Ready` thread is the tell, and the watchdog's own wording hides it. "Every thread blocked" is
what the heartbeat concluded from making no progress; it is not something it checked. One thread was
runnable the entire minute. Read the dump, not the banner.

## What it was

`place_on` decides local-or-remote by comparing `target` against `cpu::id()`, under `IPC_TABLES`,
which masks interrupts. Both of its callers then made the *same* comparison a second time to decide
whether to send the SGI, and both made it with interrupts enabled:

- `spawn_on` read `let remote = target != cpu::id()` before taking the lock at all.
- `start_thread_control_block` re-read `target != cpu::id()` after `drop(guard)`, which unmasks
  interrupts before the comparison runs.

Between the two reads the calling thread can be preempted, land back on its own core's run queue,
and be handed to an idle core by a work steal (§28.3). It resumes somewhere else holding a stale
answer. In the direction that hurts, the first read said "local" and the placement went remote: the
thread lands in a core's inbox and nothing pokes that core.

Nothing else drains an inbox. `drain_inbox` has exactly one caller per architecture, the
reschedule-SGI handler. `schedule()` does not look at the inbox, and neither does `run_idle`, which
is `try_initiate_steal(); wait_for_interrupt(); yield_now()` forever. So a missed SGI is permanent
rather than late.

That `run_idle` does not drain is a decision, not an oversight (DECISIONS §133, calef,
2026-08-28). Draining before parking would make any future missed poke self-healing. It was
refused because it trades a loud failure for a quiet one. This bug announced itself as a watchdog
dump precise enough to name the culprit from the trace rings. The same defect under an idle
drain would have read as threads occasionally starting late. The window is a few instructions wide,
so that dump was the only instrument that could have found it. The section carries the case for
draining, which is real, and what would reopen it.

And the target then refuses to rescue itself. `try_initiate_steal` returns early when
`runnable() > 0`, and `runnable()` counts the inbox, on purpose: an idle core with work in transit
should wait for its own work rather than steal more. Here the "work in transit" is never arriving,
so the guard that normally prevents a redundant steal instead pins the core in idle. That is why
one lost SGI wedges the whole machine and not one thread.

## How it was diagnosed, from the dump alone

The per-core trace rings (`--- thread dump ---`, added for the VisionFive 2 first-silicon work)
caught the migration in the act, which is why this needed no reproduction to name:

```text
core 2: ... switch:0x0 ... switch:0x1000000076 steal:0x0/3 block:0x1000000076/176 switch:0x3
core 3: ... switch:0xf00000076 switch:0x4 drain:0x1 switch:0x0 place:0x500000077/2 block:0x0/177 switch:0x4
```

Read in order: core 2 was running tid `0x0` (the test's main thread), switched to the frame
producer, then served a steal of tid `0x0` to core 3 (`steal:0x0/3`). Core 3 drained one thread
(`drain:0x1`), ran it (`switch:0x0`), and *that* is where the `place:0x500000077/2` happens: main
finished, on core 3, a `spawn_on` whose "is it local" answer it had computed on core 2. The push
went to core 2. No SGI followed it, because the stale answer said local.

The rest of the dump is the consequence, and every field agrees:

```text
tid=0x500000077 state=Ready  on_cpu=false address_space=0x00000000 wait=-        the consumer, never run
tid=0x1000000076 state=Blocked wait=0x1b0/Sender                                 the producer, no receiver
tid=0x0000       state=Blocked wait=0x1b1/Receiver                               main, waiting for the verdict
core 2: current=0x0003 idle=0x0003 need_resched=false inbox_len=1                the consumer, undrained
```

`page_frame_service::wire` spawns a producer and a consumer and then `ipc_receive`s their verdict. The
consumer never ran, so the producer blocked sending to it, so main blocked receiving from them.

The trace ring is what made this a two-hour bug instead of a four-day one. Milestone 72's entry
above says careful reasoning pointed at RISC-V for four days; the difference here is not better
reasoning, it is an instrument that records the transition rather than only the end state.

## The fix

`place_on` returns `Option<usize>`: the core that owes an SGI, or `None` for a local push. It is
`#[must_use]`, and its callers send the poke off that value. There is now one comparison against
`cpu::id()` in the whole path and its result is the only thing that can drive the poke, so the two
answers cannot disagree, because there is no second answer to disagree with. That is rung one of
CLAUDE.md's ladder rather than a comment asking the next caller to be careful.

`irq_notify` already worked this way and was the model: it takes `wake_load_aware`'s `Option<usize>`
out of the critical section and pokes whatever it names. `wake_load_aware` now calls `place_on` for
both sides instead of open-coding the local push beside it, which deletes the last duplicate of the
comparison.

## Why a stranded thread usually gets away with it, which is the whole story of the rate

A strand is not immediately fatal, and that is why this took six weeks to surface. The inbox is
drained wholesale: the *next* SGI anyone sends to that core, for any reason, sweeps up whatever is
sitting there. So a missed poke is normally repaired within milliseconds by the next unrelated
placement, steal or device wake aimed at the same core, and nothing is ever seen.

It wedges only when the strand is the last thing that happens. The stranding placement is the
work the rest of the machine was about to wait on, so no further SGI is generated. The system
goes quiet holding one runnable thread nobody will look at. That is exactly the CI dump. Main
spawned the consumer, blocked on its verdict, the producer blocked sending to a consumer that never
ran, and there was no other traffic left to poke core 2.

So the frequency of the *bug* is the frequency of the strand, and the frequency of the *hang* is the
much smaller frequency of a strand landing on the last placement before the machine idles.

## How it was proved, since one CI dump is not an experiment

Milestone 72 (lost)'s method again, and with its correction: a call-free delay loop, not a yield loop.
An early attempt held the window open with `yield_now()` and only produced a livelock. A
yielding thread goes back on its own core's run queue and `schedule()` hands it straight back; a
steal needs the thread to be *queued while something else runs*. A plain spin does that and the
first attempt at the plain spin worked.

With a 300,000-iteration spin between `spawn_on`'s two reads, on riscv64 under `-cpu rv64`, the
window is crossed roughly 8 times in 100 `spawn_on` calls rather than never, and both directions
show up in the log:

```text
[WINDOW] spawn_on tid=0x300000007  migrated cpu 0 -> 3 target=2: benign  (crossing 3 of 70 calls)
[WINDOW] spawn_on tid=0x3d00000005 migrated cpu 1 -> 0 target=1: STRANDS (crossing 17 of 133 calls)
```

`benign` is the harmless direction (a spurious SGI to a core that has nothing waiting). `STRANDS` is
the fatal one: the spawner read the target as its own core, then moved, so the push went remote and
the stale answer skipped the poke.

The A/B, same widened build, one line of logic apart:

| | Pre-fix (`remote` from the stale read) | Fixed (`remote` from `place_on`) |
|---|---|---|
| `STRANDS` crossings | 2 | 2 |
| `smp::a_batch_of_cpu_bound_work_reaches_every_core` | **FAILED**: `migration workers never drained (46/48 done)` | **ok** |

Two strands, two missing workers, out of forty-eight. The count of lost work equals the count of
stranded placements, which is the strongest form this evidence comes in: not "the symptom went
away" but "the arithmetic matches". (Both arms later die on `no initrd region`, which is an artifact
of driving `cargo test` directly instead of `cargo xtask test`, and hits both arms equally.)

Without the widener the local rate is zero and the null result has a denominator. On a quiet
8-core host, and again under ten host burners, the instrumented suite crossed the window 0 times
in more than 1,600 `spawn_on` calls and 400 `start_thread_control_block` calls across five runs,
and every run passed. The window is a handful of instructions of wall clock, so what opens it is a
host descheduling a vCPU inside it. CI's small shared runners emulating four harts are where it is
widest, which is where the sighting arrived; a laptop with cores to spare is close to the worst
place to look for it.

Do not read "only `rv64` failed, four other models passed" as an `rv64` property. Each model
gets exactly one run per matrix, so a race at any rate below about 20% produces a one-model failure
most of the time it appears at all. Milestone 72's entry makes the same point from the other side,
counting six riscv64 rolls of the dice to one aarch64 roll per pull request. Nothing in this bug is
architecture-specific: `sched.rs` is portable code and both callers were wrong on all three targets.

## The general lesson, and it is the same shape as the one above

**A decision made under a lock and re-derived outside it is two decisions.** The first entry's
lesson is that a call returning `Err` may still have acted; this one's is that a *value* read inside
a critical section and recomputed outside it is not the same value, however identical the expression
looks. When a critical section decides something the code after it needs, carry the decision out as
a value. `Option<usize>` and `#[must_use]` cost nothing and make the stale re-read unspellable.

## BUGS

- The `#[must_use]` that makes the stale re-read unspellable is on `spawning::spawn_on`, not on
  `placement::place_on`, whose `Option<usize>` is the answer that must not be dropped. Its message
  is about the returned SGI, which `spawn_on` does not return. The attribute stayed in
  `spawning.rs` when `place_on` moved to `placement.rs`, so a new caller of `place_on` that drops
  the answer compiles silently. Moving the attribute is a one-line code change.
