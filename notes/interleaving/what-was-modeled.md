# Interleavings: what was modeled, crate by crate

*Name: provisional.*

An appendix to [notes/interleaving.md](../interleaving.md), which summarizes it. The parent has the
survey, the counts and a table of all five crates. This page has each crate's harnesses, how each
was falsified, and its bounds.

## `crates/work_steal_slot`, the pilot

An idle core cannot reach into a loaded core's run queue, deliberately (DECISIONS §11), so stealing
is a message. The thief claims a one-slot mailbox in the victim with a compare-exchange from zero,
and pokes it with a reschedule interrupt. The victim swaps the slot back to zero at its next
scheduler entry, and hands one thread into the thief's inbox. The slot was an `AtomicU32` field on
`PerCpu` with the compare-exchange written inline in `sched.rs`. It is now a crate, and the kernel
calls it rather than keeping a copy of the protocol. That is the same Phase-2 move
`memory_regions`, `inter_process_communication` and `direct_memory_access_validator` made for Kani.

| Harness | Property |
|---|---|
| `two_thieves_race_and_exactly_one_claim_is_granted` | "a thundering herd of idle cores collapses to one steal per victim per round", which is a claim about a compare-exchange under concurrency and therefore a sentence no single-threaded test could check |
| `a_granted_claim_is_served_exactly_once` | conservation: a granted request is in the victim's hand or still in the slot, never both and never neither, with the victim polling while the claim is in flight |
| `a_second_victim_cannot_serve_the_same_request` | the read and the clear are one step, so one request cannot be handed to two cores |
| `a_take_sees_everything_the_thief_wrote_before_claiming` | the release/acquire pairing publishes what preceded the claim (loom's `UnsafeCell` turns "visible" into a checked fact rather than an argument) |
| `a_relaxed_pairing_publishes_nothing` | **the falsification**, `#[should_panic]`: the same handshake with relaxed orderings must fail, and if it ever stops failing we want to hear about it |
| `a_stale_load_reading_costs_a_round_and_nothing_more` | §28's gossip claim, that a thief reads its victim's load relaxed and possibly stale on purpose: the interleaving where the victim drains between the load and the claim costs a wasted round and nothing else |

Everything else on the steal path is under a lock. The compare-exchange and the swap are lifted
into the crate because they are the kind of protocol that is right on total store order and wrong
on the weakly ordered ISAs this project runs on.

## `crates/clock_contract`, the second protocol

A seqlock over a shared page. The clock service writes, and every process holding a read mapping
reads, with no lock available between them because they are in different address spaces. Its own
documentation says the memory ordering is the point rather than decoration.

| Harness | Property |
|---|---|
| `a_reader_never_sees_half_a_publish` | the state and the offset are a matched pair; a reader that catches the writer mid-publish retries rather than blending |
| `the_generation_a_reader_sees_matches_the_pair_it_read` | `Reading::generation` is a value callers depend on (did the clock step under me), not a diagnostic, so it must agree with the pair it arrived with |
| `two_writers_serialise_rather_than_corrupt_the_page` | the crate says several processes may hold the page read/write and the compare-exchange serializes them; "would corrupt silently" is a claim about interleavings |
| `a_racing_reader_sees_an_unrecognised_page_or_a_whole_one` | `init` writes the magic last with a release, so a reader racing the first publish gets `UNKNOWN` or a whole page, never a recognized page with garbage in it |

## `crates/thread_wake_handshake`, the block/wake protocol (2026-08-14)

The retrofit the fourth bench stop's audit asked for. notes/visionfive2.md said plainly that the
block/wake protocol had no loom coverage, and that covering it meant an extraction. The protocol
behind `Thread`'s `on_cpu`/`wake_pending`/`wait_on`/`ipc_served`/`ipc_aborted` fields is now a
crate. The kernel calls its transitions at every wake, park, switch and finish-switch site, and loom
searches it on the host.

This one extends the method. The parent's survey counts atomic protocols, and by that count the
block/wake path had nothing to explore: every field is written under `SCHED`. What the fourth bench
stop demonstrated is that a lock-based protocol still has an interleaving space, in the *gaps
between critical sections*. A thread that parks itself releases `SCHED` and keeps running until its
core saves its context. A waker can take the lock inside that window. All three of this protocol's
recorded races live in that gap. (The third was a bench reading since overturned, and is held as
hardening; see the table below.)

So the model puts `SCHED` behind a `loom::sync::Mutex`. The thread's saved context goes in a
`loom::cell::UnsafeCell`, written outside the lock (where `switch_to` writes the real one). Each
core's critical sections run in a loom thread in the kernel's program order, and loom orders the
sections. What it checks is therefore interleaving logic, plus the context handoff's ordering
through the lock, not memory orderings. There are none to check, and saying otherwise would be
claiming coverage the model does not have.

| Harness | Property |
|---|---|
| `a_wake_racing_a_switch_out_resumes_on_a_saved_context` | the wake-before-switch-out race (notes/intrusive-queues.md): whichever side of the race queues the thread (a direct wake after the switch-out, or a deferred wake completed by `finish_switch`), exactly one does, and the resume reads the context the victim's core saved |
| `a_wake_that_ignores_on_cpu_switches_into_an_unsaved_context` | **the reconstruction**, `#[should_panic]`: the pre-fix wake with the deferral removed, and loom must find the interleaving where the resumer takes a thread whose core is still saving its context |
| `a_stolen_thread_resumes_on_its_saved_context` | the steal edge of the same window: a preempted thread sits `Ready` with `on_cpu` still set, and the single-owner queue discipline (serve only on the owning core, after `finish_switch`) is what orders the thief's resume after the context save |
| `a_thief_that_pops_a_foreign_queue_steals_an_unsaved_context` | **the reconstruction**, `#[should_panic]`: break the single-owner rule and loom must find the thief taking a mid-switch-out thread |
| `an_undelivered_wake_racing_a_park_strands_nobody` | boot 8's gate under race: a spurious wake with nothing delivered is `Refused` in every interleaving, before or after the switch-out completes; the receiver stays parked and waiting, the real sender still completes the rendezvous, and the resume sees a delivery |
| `without_the_gate_a_spurious_wake_completes_an_empty_rendezvous` | **the reconstruction**, `#[should_panic("resumed with nothing delivered")]`: the pre-boot-8 wake (deferral kept, gate absent) strands the receiver in every interleaving. This is what boot 8's dump was read as, and notes/visionfive2.md's fifth stop (2026-08-15) overturned that reading; the gate stays as hardening |

Two mechanics worth copying. The model's invariants are real `assert!`s, not `debug_assert!`s.
This script compiles `--release`, and a should-panic reconstruction with its tripwire compiled out
reports success while checking nothing. And each of the three reconstructions is the *historical*
semantics rebuilt locally in the harness (the fields are public). So the shipped methods never carry
a broken variant. Reconstructing all three was cheap, because each was a deletion.

Bounds, honestly. Every harness runs to exhaustion: no `LOOM_MAX_PREEMPTIONS`, no branch bound.
That is affordable (the whole crate's search is ~10 ms), because the models are three or four
threads with two or three critical sections each. That size is not modesty. One waker, one victim
core, one thief is the entire cast of every recorded race in this protocol. What the small model
cannot represent is stated in the crate's BUGS. The model checks the protocol *under* the kernel's
locking discipline: every site holds `SCHED`, run queues are single-owner, and `finish_switch` comes
before the next scheduler entry. That the kernel keeps the discipline is established by reading
`sched.rs`, not by loom.

What it found: nothing in the current protocol, on the first run and after falsifying every
harness. Deleting the deferral fails the first harness, deleting the gate fails the fifth, and the
three reconstructions fail by construction. Like the pilot, the negative result has a reading. All
three of this protocol's races were found by flakes and bench boots first, and fixed before this
model existed. The model now holds the fixes in place, where the next edit to `wake` or
`finish_switch` cannot silently undo them.

## `crates/memory_corruption_canary_gate`, the corruption canary's serialization (2026-08-15)

The fourth extraction, and the first whose bug was in the extracted protocol itself rather than
held in place around it. The registry canary (first-silicon diagnostics, `kernel/src/sched.rs`)
serialized arm/check/disarm with two hand-written flags, `ARMED` and `IN_CHECK`, and the pair had
two holes.

The observed one: a `check()` that lost the single-flight compare-exchange returned silently. So the
kernel test's decisive check could be swallowed by a timer tick's pass that had read the watched
byte before the flip. That is the `thead-c906` flake in [cpu-models.md](../cpu-models.md)'s BUGS,
two failures in four runs of one tree.

The latent one: a checker between its `ARMED` load and its `IN_CHECK` win was invisible to
`arm()`'s drain loop. So a re-arm could rewrite the watch table and the shadow while a pass read
them. That is a data race whose torn `(base, len)` is a wild read in kernel space. No failure was
ever traced to the second hole: every boot arms at most once today, over ranges that never change.
But a corruption instrument that can itself read wild is not an instrument.

The replacement is one state word (`DISARMED`/`ARMED`/`ARMING`/`CHECKING`, every transition a
single compare-exchange) behind RAII guards. The kernel touches the plan and the shadow only while
holding one. Three harnesses:

- armer/checker exclusion over a shared `UnsafeCell`, where loom's access tracking makes a torn plan
  an error in itself, not a value someone must notice;
- disarm's quiescence promise: after `disarm()` returns, the caller writes the cell bare-handed;
- single-flight for two checkers.

A fourth, scratch-only harness encoded the OLD two-flag spelling against the exclusion model, and
loom falsified it on the first run. That is the falsification discipline the
`thread_wake_handshake` section above describes, applied to a protocol that was actually broken.

The state word's transitions, as the crate's guards make them:

```text
           arm()                    drop(ArmGuard)
DISARMED ─────────► ARMING ──────────────────────► ARMED ◄──┐
   ▲                  ▲                              │      │ drop(CheckGuard)
   │ disarm()         │ arm()                        │      │
   │                  │              try_check()     ▼      │
   └───────────────── ARMED ◄──────────────────── CHECKING ─┘
```

`try_check` answers `None` when it cannot take the gate, and the caller knows which it got: the
timer tick shrugs, and a caller that needs a completed pass loops. What a divergence means is the
kernel's business, not the gate's.

## `crates/memory_regions`, the untyped region claim (2026-08-18)

The fifth extraction, milestone 135 (the region claim), and the first on the memory-reclamation
path. It exists because a real double free landed there and its fix could not be gated.
`untyped::destroy` checked the region under the `REGIONS` lock, released it, revoked, freed every
page, and removed the table slot last. So two callers holding a name for one region could each pass
the refusal check inside that gap, and each run the free loop over the same pages. It happened once
in 45 loaded runs on riscv64, and it needed two cores. Pull request #316 fixed it by removing the
slot under the same hold that decided to destroy it. It said plainly in
[object-revocation.md](../object-revocation.md)'s BUGS that the single-winner claim was argued from
lock discipline and gated by nothing.

What moved. The region table and every decision taken over it left `kernel/src/memory_region.rs`
for `crates/memory_regions`, where the arithmetic it calls (`split_new_watermark`,
`destroy_outcome`) was already Kani-proved. `RegionTable::claim_for_destroy` takes `&mut self` and
does both halves. That is rung one of CLAUDE.md's ladder rather than a tidier spelling. The pre-fix
shape is not expressible against that signature, because there is no intermediate state in which a
caller holds a decision about a table it no longer holds. The kernel keeps what a crate a model
checker can run must not have, which is the I/O: the frame allocator, the direct map, the revoke,
and the lock.

This is the second lock-based protocol here, after `thread_wake_handshake`, and the same caveat
applies with the same force. What loom searches is the interleaving of *critical sections*, not
memory orderings, because there are no hand-rolled orderings to search. The parent's survey table
counts atomic protocols, and by that count this had nothing in it. Two of the five protocols now
modeled are in the population that survey called empty. That is the standing correction to it: a
protocol is a candidate when its steps span more than one critical section, whatever its fields are.

| Harness | Property |
|---|---|
| `two_destroyers_race_and_exactly_one_reclaims` | the property the whole reclamation path rests on. Two threads claim the same region and the winner runs the free loop; loom fails any execution where the loop runs twice. Both `Reached` flags fire, so both winners were really explored rather than one order run twice |
| `the_pre_fix_protocol_double_frees_and_this_model_finds_it` | **the falsification witness**, and it is the one harness in this script that asserts a protocol is BROKEN. It reconstructs `untyped::destroy` as it stood before #316, against this module's private internals, and passes only when loom **finds** an execution in which both callers free the same run |
| `a_retype_never_hands_out_a_page_the_reclaim_is_about_to_free` | the other half of why the slot comes out first: before the fix, a `retype_page` landing in the gap resolved the name and returned a page already on its way back to the allocator. Exactly one of the two succeeds, and loom explores both winners |
| `a_split_and_a_claim_on_one_parent_cannot_both_succeed` | the same exclusion one level along, and it matters more: a child carved from a parent that was concurrently reclaimed would hold a name for pages already back in the allocator |
| `a_parent_is_never_reclaimed_while_its_child_is_returning` | the child's slot comes out at its claim but the parent's child count drops only in `return_to_parent`, after the caller has revoked the run. That ordering is what keeps the parent refusing across the window in which the pages are neither the child's nor yet the parent's |

The witness is worth copying, and it is not the same mechanism as the `#[should_panic]`
reconstructions above. A should-panic harness records that *something* failed. This one records
*which* execution failed: it counts the double-free outcome in a real atomic outside the model, and
asserts on the count after `loom::model` returns. That is the `Reached` non-vacuity shape pointed at
a negative property instead of a positive one. It converts "we broke it by hand once and watched it
fail" into a standing gate that no one has to remember.

What it found: nothing in the current protocol, and the negative result has the same reading as the
pilot's. The bug was found by a flake and fixed before this model existed. The model holds the fix
in place, where the next edit to `untyped.rs` cannot silently undo it. What it did produce came from
breaking it on purpose, twice:

| Break | Result |
|---|---|
| `claim_for_destroy` stops removing the slot (the single-winner property deleted) | three of the five fail. `two_destroyers` reports "exactly one caller may free a region's pages, and this execution had 2", which is the original bug's shape in the original bug's units |
| the parent's child count decremented at claim time rather than in `return_to_parent` | `a_parent_is_never_reclaimed_while_its_child_is_returning` fails, and **only** that one, which is what a targeted harness earning its place looks like |

Bounds: 1,364 executions across the five harnesses, ~50 ms, no `LOOM_MAX_PREEMPTIONS` and no branch
bound. The harnesses use `RegionTable<2>` or `<4>` rather than the kernel's 256, because the search
is exponential and nothing in the protocol depends on capacity. Two threads is the entire cast of
the recorded bug.

## `crates/spin_lock`, the kernel's lock (2026-10-25)

`IrqSafeMutex` (`kernel/src/sync.rs`) masks interrupts and checks lock ranks, and under that it
takes `spin_lock::SpinLock`: a flag set with an Acquire compare-exchange and cleared with a Release
store. Most of the kernel's happens-before edges come from that pair
([memory-ordering.md](../memory-ordering.md)). The crate is the protocol and nothing else, so the
model searches exactly what the kernel compiles. The data sits in a loom cell that the guard's
`deref` and `deref_mut` read and write, so two guards whose accesses no edge orders fail the run.

Five harnesses:

| Harness | What it holds | Search |
|---|---|---|
| `two_takers_are_exclusive_and_ordered` | two threads each increment under the lock; none goes missing, and the search reaches each order | exhaustive, 105 executions |
| `three_takers_are_exclusive_and_ordered` | the same for three, and the search reaches each of the three counts | preemption bound 1, 66 executions |
| `try_lock_refuses_while_another_guard_holds` | a held lock refuses `try_lock`, a refusal leaves the holder holding, and the search reaches both outcomes | exhaustive |
| `a_release_publishes_what_the_holder_wrote_before_it` | a write outside the lock, made before the release, is visible to the next holder: the IPC rendezvous's edge | exhaustive |
| `a_relaxed_release_publishes_nothing` | the previous model with the crate's own unlock passed `Relaxed`; passes only on loom's causality violation | exhaustive |

Falsifying by hand: the guard's release store weakened to `Relaxed` fails the four positive models,
and so does the lock's compare-exchange weakened to `Relaxed` on success. The fifth harness keeps
the first of those falsifications standing without anyone breaking the code.

The three-taker bound is loom's, not the lock's. Each waiter yields while the flag holds, and with
two waiters loom's scheduler can hand the processor from one to the other without end while the
holder never runs. At a bound of two it reaches such an execution and stops past 20,000 scheduling
branches with "Model exceeded maximum number of branches", the limit its message names for spin
locks. One preemption still covers a holder preempted inside its critical section with two waiters
spinning behind it.
