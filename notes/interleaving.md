# Interleavings, model-checked (loom)

The fourth leg of the analysis surface, after Kani ([verification.md](verification.md)), the fuzzers
([fuzzing.md](fuzzing.md)) and Miri ([undefined-behavior.md](undefined-behavior.md)). Milestone 80.

CLAUDE.md's fourth rule is *assume weak memory ordering*, and before this milestone nothing in the
tree could falsify a violation of it. We had a rule, careful comments about acquire
and release, and no instrument. The instrument found a real bug in
the first protocol it was pointed at that had not been designed with it in mind.

## Why the other three tools cannot see this

| Tool | What it checks | Why it misses an ordering bug |
|---|---|---|
| Kani | properties for every *input* | every harness in the tree is single-threaded, and the note that carries them says so: "concurrency is the sharpest edge of this limit" |
| The fuzzers | crashes on hostile bytes | one thread, one input at a time |
| Miri | aliasing, provenance, uninitialized reads | it runs *one* interleaving of a program, not the space of them |
| `script/test` under QEMU TCG | that the kernel boots and passes | TCG executes guest atomics conservatively and round-robins vCPUs; it explores almost none of the orderings aarch64 and riscv64 permit |
| `script/test --hvf` (milestone 81) | the real ISA on four physical cores | genuine orderings, **unsearched**: it samples what the silicon happened to do on that run |

Loom is the one that searches. It runs a concurrent test on the host and enumerates every thread
interleaving *and* every reordering the C11 memory model permits, including relaxed-ordering
surprises that no machine you own would produce today and some machine will produce tomorrow.

And loom models C11, not ARM and not RISC-V. That is the caveat to repeat rather than bury: it
narrows the gap, it does not close it. Litmus-level confidence about either ISA's own model would
need herd7-style tooling and is not this milestone. A tear loom finds is real; a clean loom run is
not a proof about the silicon.

## The survey: where the hand-rolled protocols actually are

The roadmap named three candidates (the per-CPU run-queue handoff, the reaper handoff, the IPC
sender queue) and the brief added two more (`crates/intrusive_fifo`, `crates/generational_table`). Four of those five
have no atomic protocol at all, and finding that out is most of what the survey was for.

| Candidate | What it actually is | Reachable by loom |
|---|---|---|
| The IPC sender queue (`crates/inter_process_communication`) | zero atomics. `Rendezvous` is plain data under the `SCHED` `IrqSafeMutex` | nothing to explore |
| `crates/intrusive_fifo` (the run queues) | zero atomics. Single-owner with interrupts masked, plus an `UnsafeCell` | nothing to explore |
| `crates/generational_table` (the thread table) | zero atomics. Under `SCHED` | nothing to explore **as atomics**, and that reading was too narrow: `crates/memory_regions` is a `generational_table` table under a lock, and its protocol had a real double free in it. See the `thread_wake_handshake` and `memory_regions` sections of [what-was-modeled.md](interleaving/what-was-modeled.md) |
| The reaper handoff (`PerCpu::switched_from`) | one `AtomicU64`, both accesses `Relaxed`, written and read **by the same core** with interrupts masked. The atomic is interior mutability, not synchronization | nothing to explore |
| The run-queue handoff | the migration inbox is an `IrqSafeMutex`; the *steal request slot* is the lock-free part | **yes**, and it is the pilot |

So the population is smaller than it looked, and that is a fact about the design rather than a gap in
the search: this kernel puts almost everything behind a ranked interrupt-safe lock on purpose
(DECISIONS §9, [locking.md](locking.md)). What is left, from a grep for every compare-exchange, swap
and fetch-op outside test code:

- `crates/work_steal_slot` (new, milestone 80 (loom)): the work-steal request slot. The pilot.
- `crates/clock_contract`: the clock page's seqlock. Cross-*address-space*, hand-rolled, with an
  explicit fence in the reader. This one the roadmap did not name, and it is where the bug was.
- `kernel/src/smp.rs`: the boot roster. `HWID`/`STARTABLE` written relaxed, then `ROSTER` stored
  with a release; readers acquire `ROSTER` and then read the arrays. A textbook array publication,
  correct as written, and single-shot at boot.
- `kernel/src/arch/*/irq.rs`: the interrupt-routing lottery, a compare-exchange per IRQ line.
  Rule 1 keeps it under `arch/`, so lifting it is a bigger question than this milestone.
- `crates/user_mode_runtime/src/heap.rs`: a hand-rolled userspace spin lock. `user_mode_runtime` is aarch64 inline
  `asm!` and does not compile for the host at all, so reaching it needs the lock lifted out first.
- Everything else is a counter: `fetch_add` on a statistic that a reader compares against zero or
  against its own earlier reading. Relaxed is right and there is no protocol.

## What was modeled

25 harnesses <!--count:loom-harnesses--> across six crates <!--count:loom-crates-->, run by
`script/interleaving-check`.

The harness number is written in digits because `script/lint`'s marker reads small cardinals and no
compositions, so "twenty-four" would be read as four; the crate number stays a word because six is
in range. That asymmetry is the gate's, not the prose's.

Both numbers said sixteen and three until the 2026-08-17 documentation sweep. This line is the
reason the sweep's gate work happened: `notes/counted-claims.md` cited it as the case a
`<!--count:NAME-->` marker could never help, because milestone 125's marker read digits and nothing
else. It reads a cardinal spelled as a word now, so the sentence is re-derived on every lint. The
count was wrong in both halves, and the note contradicted itself a few hundred lines down, where the
timing table already said "all four crates".

| Crate | Protocol | What the model holds |
|---|---|---|
| `crates/work_steal_slot` | the work-steal request slot, a compare-exchange mailbox; the pilot | one steal per victim per round, each request served exactly once, release/acquire publication, and a relaxed pairing that must fail |
| `crates/clock_contract` | the clock page's seqlock, across address spaces | no torn reading, a generation that matches its pair, serialized writers; this is where the bug was |
| `crates/thread_wake_handshake` | block/wake, in the gaps between `SCHED` critical sections (2026-08-14) | a wake racing a switch-out resumes on a saved context; three historical races rebuilt as `#[should_panic]` reconstructions |
| `crates/memory_corruption_canary_gate` | the corruption canary's arm/check/disarm (2026-08-15) | armer and checker exclusion, disarm's quiescence, single-flight checking; the old two-flag protocol failed on the first run |
| `crates/memory_regions` | the untyped region claim (2026-08-18) | exactly one destroyer reclaims; a witness harness asserts the pre-#316 protocol double-frees |
| `crates/spin_lock` | the lock under `IrqSafeMutex` (2026-10-25) | exclusion and the happens-before edge for two and three takers, and a Relaxed release that must fail |

Two of the six are lock-based, so what loom searches there is the interleaving of critical
sections, not memory orderings. That is the standing correction to the survey: a protocol is a
candidate when its steps span more than one critical section, whatever its fields are. Every
harness runs to exhaustion, with no preemption or branch bound, except `spin_lock`'s three-taker
model, which loom cannot finish past one preemption. Each crate's harness table, how
each was falsified, and its bounds are in
[interleaving/what-was-modeled.md](interleaving/what-was-modeled.md).

## What loom found

A real weak-memory bug in the clock page's seqlock, on the first run.

The writer claimed the sequence (a compare-exchange to an odd value) and then wrote the state and
the offset, with nothing ordering the claim ahead of them. Three of the four harnesses failed
immediately, all with the same shape: a reader observing the *new* offset beside the *old* state,
revalidating the sequence successfully because the odd value had not reached it either, and
returning the pair. A wrong wall clock, silently, from an API whose whole job is to make a torn read
impossible.

```
a torn reading: (1, 2000) is neither publish
a reader saw a recognized page with garbage in it: (0, 1000)
the generation disagrees with the reading it came with: Reading { state: 1, offset_nanos: 0, generation: 0 }
```

The fix is one line, a `fence(Release)` between the claim and the data stores, which is exactly the
`smp_wmb()` Linux puts in `write_seqcount_begin`.

The part worth keeping is which fixes do not work. The obvious reflex is to strengthen the
compare-exchange, and it was already `Acquire` on success with a comment saying that is what stops
the stores being hoisted above the claim. That comment is true and irrelevant to this bug:

| Attempt | Result |
|---|---|
| claim as `Acquire` (as shipped) | 3 of 4 harnesses fail |
| claim as `AcqRel` | 3 of 4 fail |
| claim as `SeqCst` | 3 of 4 fail |
| `fence(Release)` after the claim, claim left `Acquire` | all pass |

An acquire or release RMW orders accesses around *itself*. What a seqlock writer needs is its own
store ordered ahead of the plain stores that follow, and that is a store-store barrier between
the two, which no ordering on the RMW expresses. This is the kind of thing that is obvious once
stated and was not obvious to anyone who read the code, including the person who wrote the comment.

The reader's existing `fence(Acquire)` was checked the same way: removing it fails the same three
harnesses, with the writer's fence in place. So both halves of the pair are now checked rather than
argued.

Why nothing else caught it. It is unreachable on x86 (total store order gives the missing
barrier for free). QEMU's TCG explores almost none of the orderings that produce it. The ten host
tests in `clock_contract`, the kernel's clock tests on both ISAs, `script/verify`, `script/fuzz` and
`script/undefined-behavior-check` all passed before the fix and all pass after it: none of them asks
a question this could answer. The failure mode it would have produced on the VisionFive 2 is a
timestamp that is wrong by however far the clock last stepped, at a rate too low to reproduce and
with no instrument pointed at it.

### And the pilot found nothing, which is its own result

All six `work_steal_slot` harnesses passed on the first run, which is worth having for three
reasons.

It converts three comments into checked facts (the herd collapsing to one claim, the release/acquire
pairing, the accepted staleness of the load reading). It leaves a regression test on a protocol
that is about to matter more. The scheduler partitioning of milestone 17 (multikernel-leaning scheduler) is explicitly sequenced behind
this one, and [ipc-tables-lock-inventory.md](ipc-tables-lock-inventory.md) says any design that replaces the
`SCHED` lock with messages wants its protocol born loom-checked. And the negative result itself is
informative: the steal slot is simple *because* the design pushed everything else behind a lock, so
"loom found nothing here" is evidence for DECISIONS §9's discipline rather than evidence against
loom.

The sharpest thing the pilot did produce came from breaking it on purpose:

| Break | Result |
|---|---|
| `claim` as a load then a store instead of a compare-exchange | `two_thieves_race_and_exactly_one_claim_is_granted` fails: "both thieves were granted the slot" |
| `take` as a load then a store instead of a swap | **all six still pass.** The atomicity of the swap is not load-bearing under the kernel's single-victim discipline: `serve_steal_request` runs on the owning core from a masked-interrupt handler and cannot re-enter itself, so there is never a second taker |
| ... and the same break, against a two-victim harness | fails: "one request was served to two victims: Some(4) and Some(4)" |

That is why `a_second_victim_cannot_serve_the_same_request` is in the suite: it is the one harness
guarding a property the running system does not currently need. The swap stays because it is one
instruction instead of two and because the discipline that makes the weaker version safe is written
nowhere the compiler can see it.

## EXAMPLES

Run everything:

```
$ script/interleaving-check
==> loom: the hand-rolled atomic protocols, every interleaving
running 6 tests
test interleavings::two_thieves_race_and_exactly_one_claim_is_granted ... ok
...
test result: ok. 6 passed; 0 failed
running 4 tests
test interleavings::a_reader_never_sees_half_a_publish ... ok
...
test result: ok. 4 passed; 0 failed
running 6 tests
test interleavings::a_wake_racing_a_switch_out_resumes_on_a_saved_context ... ok
...
test result: ok. 6 passed; 0 failed
running 7 tests
test table::interleavings::two_destroyers_race_and_exactly_one_reclaims ... ok
test table::interleavings::the_pre_fix_protocol_double_frees_and_this_model_finds_it ... ok
...
test result: ok. 7 passed; 0 failed
```

Watch the region claim fail, which is cheaper than trusting a green run. Delete the
`self.table.remove(name)` from `RegionTable::claim_for_destroy` and:

```
$ script/interleaving-check -p memory_regions two_destroyers
thread '...two_destroyers_race_and_exactly_one_reclaims' panicked at crates/memory_regions/src/table.rs:
assertion `left == right` failed: exactly one caller may free a region's pages, and this execution had 2
  left: 2
 right: 1
```

Run one harness, which is what you do while iterating on a counterexample:

```
$ script/interleaving-check a_reader_never_sees_half_a_publish
```

Reproduce the clock bug, to see what a loom failure looks like before trusting a green run. Delete
the `fence(Ordering::Release)` from `ClockPage::publish` and:

```
$ script/interleaving-check -p clock_contract
thread '...a_reader_never_sees_half_a_publish' panicked at crates/clock_contract/src/lib.rs:738:26:
a torn reading: (1, 2000) is neither publish
```

Add a protocol of your own. Four steps, and the third is the one that is easy to get wrong:

1. Put it in a crate that compiles for the host. A protocol inside a `no_std` binary is unreachable,
   which is rule 7 pushing in the same direction it already pushes for Kani.
2. Swap the atomics behind the cfg:
   ```rust
   #[cfg(loom)]
   use loom::sync::atomic::{AtomicU32, Ordering};
   #[cfg(not(loom))]
   use core::sync::atomic::{AtomicU32, Ordering};
   ```
   and add `[target.'cfg(loom)'.dependencies] loom = "0.7"` to the crate's manifest.
3. Give every spin loop a yield. Loom's scheduler is cooperative, so a thread spinning on
   `core::hint::spin_loop()` can starve the writer whose progress it is waiting for and the model
   never terminates. `clock_contract` has a `spin_hint()` helper that is `loom::thread::yield_now()`
   under the cfg and the hint otherwise; copy that shape.
4. Add the crate to `script/interleaving-check`'s package list, write the harnesses, and falsify
   each one before believing it.
5. Gate that the caller still calls it, which is the step this note did not have until milestone
   136 and the one whose absence is silent. See the section directly below.

## A lift is only worth what its caller does

Every retrofit here takes a protocol out of the code that runs it, puts it in a crate a model
checker can reach, and has the original call the crate. The entire value is that last clause. A
model that searches code the kernel no longer runs reports success forever, with no symptom.
Milestone 136 (one decision path, and a gate that keeps it that way) gated this for `crates/memory_regions`, where #316's double free could be rebuilt
from the public API alone. `script/lint`'s "the region claim protocol has one decision path" check
pins the crate's public surface and requires each free loop to make its entitling call first. The
other loom crates have the same exposure and are not gated, except that `IrqSafeMutex`'s field type
is `spin_lock::SpinLock`, so the compiler holds that caller. The rebuilt gap, the two halves of
the check and the `compile_fail` doctests are in
[interleaving/lift-is-only-worth-what-its-caller-does.md](interleaving/lift-is-only-worth-what-its-caller-does.md).

## The cost, measured

| | |
|---|---|
| Runtime, all five crates, warm | **under a second** wall on an M-series laptop (0.2 s measured 2026-08-14, with `thread_wake_handshake`'s six harnesses adding ~10 ms of search; `memory_corruption_canary_gate`'s three, measured 2026-08-15, add under 10 ms; `memory_regions`' five, measured 2026-08-18, add ~50 ms over 1,364 executions) |
| Runtime, cold (compiling loom and its 28 transitive crates) | ~6.5 s |
| Crates added to `Cargo.lock` | **28** (loom plus `generator`, `scoped-tls`, `tracing`, `tracing-subscriber` and their trees) |
| Crates compiled by an ordinary `cargo build`, `cargo test`, `cargo clippy` or `script/test` | **zero** |

That runtime is why it could be a gate and is not one yet; see BUGS below.

### Where the dependency is gated, exactly

`loom` is declared under `[target.'cfg(loom)'.dependencies]`, which is tokio's own pattern. Cargo
evaluates `cfg(loom)` as false for every real target, so:

- `cargo build`, `cargo test`, `cargo clippy`, `script/test`, `script/lint` and every CI job never
  resolve it, never download it and never compile a line of it.
- `cargo tree` does not show it. `cargo deny` does not see it either: `deny.toml` narrows the graph
  to the five targets this project builds for, and `cfg(loom)` is true for none of them.
- The one place it *is* visible is `Cargo.lock`, which records every possible dependency regardless
  of activation. Twenty-eight lines' worth. That is the honest cost of the decision, and it is the
  cheap end of DECISIONS §46: nothing ships it, nothing links it, and removing it is deleting two
  manifest stanzas and two cfg blocks.

`--cfg loom` is set by `script/interleaving-check` and by nothing else in the tree.

## BUGS

Each entry is in full in [interleaving/limitations.md](interleaving/limitations.md).

- Loom models C11, not aarch64 and not riscv64. A failure it reports is real; a clean run is not a
  proof about the silicon.
- It is not in `script/test`. The `interleavings` job in `ci.yml` runs it per pull request, with a
  20-minute budget tripwire.
- It cannot see the reschedule interrupt that `work_steal_slot`'s liveness depends on.
- The harnesses are small on purpose, and small is a bound. `Reached` flags make a bound that
  empties the interesting branch fail loudly.
- The region model checks the claim, not the free loop, and its lock is not the kernel's lock.
- The gate on `untyped.rs` is narrower than the property it protects.
- `user_mode_runtime`'s spin lock and the interrupt-routing lottery are unmodelled.
- The `#[cfg(loom)]` code is invisible to `script/lint`; `script/interleaving-check` lints it with
  `-D warnings`.
