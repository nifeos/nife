# The instruction clock: why it is opt-in

*Name: provisional. Named after the parent's own heading.*

An appendix to [`notes/instruction-clock.md`](../instruction-clock.md). It records what `-icount`
costs, measured on 2026-08-17 (UTC), and why the instrument is its own boot mode and feature rather
than part of `script/test`.

`-icount` is not a flag that observes. It changes what QEMU is. But the first thing to say is a
correction, because the milestone block states the cost as *"icount is slower and changes what the
suite measures"* and nobody had ever measured the first half of that. On compute it is not
slower.

| identical bench boot, `-smp 1`, three runs each | wall clock |
|---|---|
| `-icount shift=0,sleep=off` | 2.47 s, 2.50 s, 2.61 s |
| no icount | 2.69 s, 2.62 s, 2.80 s |

Measured 2026-08-17, same binary, marker to marker (boot to `bench: done`), on a laptop carrying
other lanes' gates. The instrument was if anything marginally *faster*, because `sleep=off`
fast-forwards virtual time through the idling the bench does between phases, and that gain covers
icount's per-instruction overhead. So "it would slow the suite down" is not the argument, and
repeating it would have been an unmeasured claim inside a milestone about unmeasured claims. The
second half of the block's sentence is exactly right, and is the whole of what follows.

The two real reasons are these.

*One virtual clock for every vCPU.* An idle hart parked in `wfi` jumps that clock forward to the
next event, so multi-hart timing is fiction here. That is why this boot and the bench boot are both
`-smp 1`, and why the placement probe can never move here (a cross-core delivery test cannot run on
a one-core instrument, `notes/benchmarks.md`). A suite run this way would not fail; it would
silently stop proving every cross-core property it exists to prove, which is worse.

*A clock-bound wait stops costing host time and starts costing instructions.* This boot samples 64
tick periods, which is 0.64 s of virtual time and therefore 6.4x10^8 guest instructions, and it
spends about 3 s of wall clock retiring them. A plain TCG guest reaches the same counter value in
0.64 s. So the suite's `spin_for`s and clock-bounded `wait_for`s would cost roughly five times what
they cost today, and the compute-bound parts would cost nothing extra. That is a real bill, and it
is a different bill from the one everyone assumed.

Putting it on `script/test` would therefore change what all ~400 tests measure in order to sharpen
two of them. So this is its own boot mode, its own feature and its own command, on the model
`script/bench` already set. Nothing on the test path changes at all. The `tick_trace` hooks are
`#[cfg(feature = "icount")]`, so the test and shipping builds do not contain them, and what this
boot measures is therefore the handler that ships rather than a handler carrying an instrument.

It is its own feature, which implies `bench` rather than extending it. The two boots park at the
same point, so they leave exactly the same set of functions unreferenced, and `bench` already
carries that set spelled out one `cfg` at a time across five files. Declaring `icount` independent
means writing every one of those conditions again, in `main.rs`, `sched.rs` and `user.rs`, to
describe a fact already described. The other way out is a crate-level `allow(dead_code)`. DECISIONS
§38 (a suppression is scoped to an item) refuses that, correctly, and `script/lint` enforces the
refusal. (It refused this lane's first attempt, which is the mechanism working.)

What that costs is that the icount kernel carries the benchmark code, compiled and never run.
Nothing measures *this* binary against a baseline, so the ±5% codegen drift that makes
`bench --check` a coarse tripwire is not a cost here. The property that mattered is kept:
`script/bench`'s own binary is untouched, so no baseline had to be re-saved to admit an unrelated
instrument, which is a tripwire losing its history for nothing.

What it costs to run: 7.0 s wall for both ISAs including two kernel builds (2026-08-17, same
conditions).
