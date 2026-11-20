---
status: BUILT
raised: 2026-09-21
built: 2026-09-21
promoted_from: map-new-times-a-window-too-short-to-mean-anything
---
# 541. A timed window that excludes preemption

*(Number provisional until the merge queue lands it.)*

Promoted from a proposal on calef's instruction of 2026-09-21, which was to promote the
`map_new` proposal and launch a lane on it. It was written the same day as
`design/roadmap/proposals/map-new-times-a-window-too-short-to-mean-anything.md` by the lane
attributing the x86_64 `map_new` regression (`mapnew/attribute-the-x86-map-new-regression`, pull
request #1081, which this branch is based on). Its gate was `DECISION`, because either shape of
the fix re-saves `map_new` on all three baselines. A baseline save is a statement that a
performance change is intended and understood. That ruling is now given, so the save is in scope.
The block below records what was chosen and why the alternatives lost.

## The defect

| `MAP_ITERS` | `schedule()` spelled one way | spelled the other | delta | reads as |
|---|---|---|---|---|
| 64 (what ships) | 180,604 | 228,356 | 47,752 | **+26.4%, red** |
| 192 | 529,532 | 577,284 | 47,752 | +9.0%, green |

The two spellings are semantically identical and the map path's object code is byte-identical
between them. The marginal cost of one map is 2,726 ticks in both. So `map_new` reported a
quarter-sized regression in a path that had not changed. The 10% tripwire fired or did not
depending only on how many iterations the row happens to run.

**What the window actually is, which the proposal did not have.** `map_new` is roughly 250
microseconds of guest time on every architecture (15,870 ticks at 62.5 MHz on aarch64, 2,410 at
10 MHz on riscv64, 180,537 TSC ticks on x86_64). The scheduler tick period is 10 ms
(`arch::timer::TICK_HZ` is 100 on all three). The window is about 2.5% of a tick period. Whether
a timer interrupt lands inside it is a one-in-forty coin flip on the phase the boot left the timer
in, and that phase moves when any unrelated kernel code changes size. x86_64 is not fragile where
the other two are sound; it is where the coin came up heads.

## What was chosen: option 1, and it is achievable under `-icount`

`kernel/src/bench.rs`'s `map_new` masks interrupts across the timed window
(`arch::interrupts::disable` / `restore`, the contract all three architectures already implement
with `PSTATE.DAIF`, `sstatus.SIE` and `RFLAGS.IF`). **The open question the proposal could not
answer was whether a preemption-free window was possible at all under `-icount`. It is**, and the
measurement is the same perturbation applied to both builds:

| | old chain | the `match` | delta |
|---|---|---|---|
| before, x86_64 | 180,604 | 228,356 | **47,752** |
| after, x86_64 | 180,537 | 180,537 | **0** |

The row is now byte-identical across the perturbation that used to move it by a quarter. The
67-tick difference from the old baseline is the mask instruction pair, not the map path.

**The `-icount` caveat, stated rather than skipped, because it is a real trade.** All vCPUs share
one virtual clock. So a window that masks interrupts is a window that *excludes whatever the rest
of the machine would have done inside it*. That is a different measurement, and here it is the
wanted one. The row's job is the cost of mapping a page, not the cost of being descheduled while
mapping one. Rows that mean to measure scheduling (`yield_switch`, `spawn_reap`) do not mask and
must not. It is available only to a row whose body cannot block. `map_new` retypes from a region,
walks the table and writes a leaf, taking spin locks only.

**The interrupt is deferred, not lost.** It is pending at `restore` and taken there. That is why
`map_new`'s own probe can report one preemption for a window that took none.
`kernel/src/preemption_window_tests.rs` asserts both halves. A masked window takes no preemption on
this core however long it runs, and unmasking delivers the tick that was held rather than dropping
it. The second is the one a reader is right to doubt. A fix that bought a stable number by
starving the scheduler would be worse than the defect.

## Why the other two lost, and one of them lost to a measurement the proposal did not have

Option 2, raise `MAP_ITERS` so the lump is small against the window. It loses because
**the lump is not a constant, so the dilution factor is not predictable.** The proposal's table
showed the same 47,752 at 64 and at 192 iterations and read it as a fixed quantity. But that is
one preemption measured twice. At 4,096 iterations the identical event cost 6,670 ticks on
x86_64, a factor of seven less. The cost of an in-window preemption is bounded below by the trap
and switch. It is bounded above by however much of a quantum another runnable thread takes. So
raising the iteration count buys an unknown number of somethings rather than a smaller fraction of
one known thing. It is also the option that hides the noise instead of removing it. And it costs
run time in proportion: 4,096 iterations is a 62-fold longer window on a row that CI runs on every
push.

Option 3, report the marginal cost `(ticks(2n) - ticks(n)) / n`. It was the most attractive of
the two, because 2,726 was stable across the whole episode. It loses on two counts. It needs two
windows instead of one, each still individually preemptible. The subtraction cancels the lump
only if the same number of preemptions land in both, which nothing arranges. At 4,096 iterations
the unmasked aarch64 window took two preemptions and the riscv64 one took one. So the arithmetic
that was meant to cancel the noise would have been differencing two different amounts of it.
And it is redundant against option 1, which is the honest reason. With the window masked the
total is already stable to 0.04% across the perturbation. The marginal number would buy a second
derivation of a number that no longer moves.

Would option 1 still win if all three cost the same? Yes, and by more. It is the only one of
the three that makes the row measure what its name says. The other two make a row that measures
mapping *and scheduling* easier to live with.

## The other two architectures, which were unmeasured and are not sound

The proposal's second `BUGS` entry was that only x86_64 had been swept. Measured here:

- **The perturbation does not trip them.** The `schedule()` shape change moves aarch64 by 1 tick
  (15,874 to 15,873) and riscv64 by 1 tick (2,412 to 2,411). Both take zero preemptions in the
  window. That is phase, not immunity.
- One preemption would trip aarch64 on its own. Price it at 4,096 iterations, where two
  preemptions land in the unmasked aarch64 window and none in the masked one. That gives ~1,759
  ticks per preemption against a shipping window of 15,870: 11.1%, over the tripwire. riscv64's is
  ~112 ticks against 2,410, 4.6%. That would not fire alone. It would turn any real 6%
  regression into a failure or any real 15% one into a pass.

So the fix ships on all three. The three baselines are saved in their own commit with the
attribution beside the numbers.
Milestone 415 (sub-tripwire drift accumulates across baseline saves) requires that of any save.

## The counter cost 150 bytes of IPC fastpath, and not for the reason anyone guessed

`script/fastpath-footprint` was not on this lane's gate list and caught this after the fact.
riscv64 `ipc_send_recv` grew 5.4% (4,632 to 4,884) against a 5% bound, and `system_call_entry` 6.8%.
That gate exists for Liedtke's 1995 argument that Mach's IPC was slow. A kernel touching a lot of
memory per IPC evicts the *application's* working set, so it is not a style bound.

The attribution, by A/B, because three plausible causes were all wrong:

| what was changed | riscv64 `ipc_send_recv` |
|---|---|
| base | 4,734 |
| the whole milestone as first written | 4,884 |
| ...with the `fetch_add` in `count_preemption` deleted, field kept | **4,884** |
| ...with the field moved to the end of the struct | **4,884** |
| base plus the `cpu::PerCpu` field and **nothing else** | **4,884** |
| the counter in its own array, `PerCpu` untouched | **4,734** |

So it was neither the increment nor the field's position. It was `size_of::<PerCpu>()` going
from 128 to 136. `PERCPU[id]` is an index into an array of these, so the address is
`base + id * size_of`. At 128 that multiply is a shift. `cpu::current()` inlines to a couple of
instructions at each of its many call sites, several on the IPC fastpath. At 136 every one of
them grows. **A field added to `PerCpu` is not free even when the fastpath never reads it**, and
nothing in that file had ever said so.

The counter now lives in `sched::PREEMPTIONS_PER_CPU`, its own array, written on the timer
preemption path which no IPC path reaches. The fastpath is byte-identical to before this
milestone on all three architectures, and `bench/fastpath-*.txt` is untouched.

`cpu.rs` now carries a `const` assertion that `size_of::<PerCpu>()` is a power of two. The
next person meets this as a failing build rather than as a CI gate they did not run. It is
exempt on x86_64, and the exemption is measured rather than assumed. There the struct carries
`x86_trap` and is already 152 bytes, and that architecture's gate is green with room (+1.0%,
+1.4%, +3.9%). Asserting a property the tree does not hold is how a gate teaches people to route
around it.

And one thing this says about the instruments. Between the two shapes, every row of
`script/bench` moved by at most 0.12%. A benchmark tripwire that gates on time could not see a
change that the footprint gate failed on. That is the argument for having both.

## BUGS

- **`map_el0` has the same shape and is not fixed.** It times a mapping loop from EL0. The
  kernel cannot mask interrupts around a window it does not own, so this milestone's mechanism
  does not reach it. It has not been measured for the same fragility. Fixing it needs a different
  instrument (a system call that brackets the window, or reporting the marginal cost after all). That
  is a design fork rather than a lane's call.
- **The masked window is not the benchmark's own claim, and cannot be.** `--features bench`
  compiles the tour out and runs no tests, so nothing in the suite ever executes `map_new`'s
  window. The tests assert the primitive it rests on, one level down. A future change that removed
  the masking from `map_new` while leaving `arch::interrupts` correct would pass every test here.
- `sched::PREEMPTIONS_PER_CPU` is written on every preemption on every core, one relaxed
  increment beside the global one, in a build with no `bench` or `test` feature. Its *code size*
  is now measured and is zero (`script/fastpath-footprint` is byte-identical to base on all three
  ISAs). Its *time* cost is not measured in isolation, and `script/bench --check` bounds it only at
  under 10% of every row.
- The power-of-two assertion is a tripwire, not an explanation. It fires on the size. A future
  field that happens to land on 256 will pass it while still growing every `PerCpu` by 128 bytes
  of mostly padding. Nothing checks that the size is *tight*, only that it is a shift.
- The 2.5% window-to-tick ratio is arithmetic over one measurement of each architecture, not a
  distribution. Nothing here sweeps the phase, so "one in forty" is the shape of the exposure and
  not a measured rate.

## Follow-on

- **Recorded.** `map_el0` has the same short-window shape and this mechanism cannot reach it,
  because the kernel does not own a window that runs in EL0. The limitation is beside the feature in
  this block's `BUGS` and in `notes/benchmarks/preemption-in-the-window.md`'s 2026-09-21 entries,
  which is where a reader meets the row. It is not promoted to a milestone because the instrument is
  undecided. The choice is a design fork (a system call that brackets the window, or reporting the
  marginal cost), not work a lane can pick up.
- **Recorded.** The one relaxed increment this adds to every preemption on every core is
  unmeasured in isolation in *time*. The `BUGS` entry beside it says so and bounds it by the
  tripwire. Its code size is measured and is zero.
- **Recorded.** `script/fastpath-footprint`'s 5% bound is drift from a recorded figure rather than
  distance from a target. The base this branch sits on is already +2.2% and +4.7% on riscv64
  without this milestone contributing anything. That headroom question is not this lane's to
  settle, and another lane is re-shaping that gate. The limitation is recorded in the `BUGS`
  section of `script/fastpath-footprint`, where a reader meets the bound. No recorded footprint
  figure was re-saved.
- **Done.** The proposal's three `BUGS` entries are all closed by this block. The composition of
  the lump is counted rather than inferred (and the inference was wrong). The other two
  architectures are measured, and the three baselines are saved on calef's ruling.

## Index row

A benchmark row whose window is 2.5% of a scheduler tick period does not measure what its name
says. It measures whether a timer interrupt happened to land in it, and reports the answer as a
26.4% regression in a code path that never changed. This milestone establishes that a timed window
*can* exclude preemption under `-icount`, which was an open question, and makes `map_new` do it.
The row is now byte-identical across the perturbation that used to move it by a quarter. It matters
beyond one row because the alternative habit is re-saving a baseline to make red go away, which is
how a tripwire stops being one.
