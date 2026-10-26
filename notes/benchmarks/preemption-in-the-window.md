# Preemption inside a timed window

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds the x86_64 `map_new` +26.4% lump and
milestone 541's masked window, with the dates, tables and corrections behind them. Name: ratified
2026-09-24 (calef); [the naming record](README.md) holds it.*

## 2026-09-21: the x86_64 `map_new` "+26.4%" is a fixed 47,752-tick lump, not a cost per map

`script/bench --x86 --check` was reported failing on `map_new` (228,380 against a baseline of
180,604, +26.4%). The report came from the lane building a thread's own CPU page, which A/B'd it and
concluded it was somebody else's and predated them. Both halves of that conclusion are wrong. This is
the attribution.

### The regression is not on `main`, and never was

On `origin/main` at `8ea8e2f3`, `map_new` measures 180,604, the committed baseline to the digit, and
`script/bench --x86 --check` exits 0. It also measures 180,604 at `8cda8b53`, the merge base that
branch forked from. Every number quoted in that pull request is a number from its own branch.

### One commit, and one hunk inside it

Walking the branch commit by commit:

| commit | `map_new` |
|---|---|
| `99875a7b` the crate alone | 180,604 |
| `d5b28887` the kernel writes each thread's core | **228,380** |
| `32ac6bdd`, `d2489e26` | 228,380 |
| `31061717` the page moves into the gigabyte the process already maps | 224,332 |

So `228,380` and `224,332` are two commits of one branch, not a with-and-without pair. The A/B that
reported them "identical with the change compiled out" had compared the branch against itself.

### It is not the page, and it is not `cpu::id()`

Three A/Bs on the branch tip, each rebuilt and re-measured:

| what was removed | `map_new` |
|---|---|
| nothing (the tip) | 224,332 |
| `attach_current_cpu_page` in `AddressSpace::new` | 228,380 |
| `attach_current_cpu_page` made a no-op everywhere, so no space has a page at all | 228,380 |
| `cpu::id()` replaced by the constant `0` at the call site | 224,332 |
| `schedule()`'s `next_root` restored to its old expression | **176,556** |

The whole of it is one hunk in `schedule()`, and that hunk's semantic content is not what costs.
Applying only the shape change to a clean `origin/main`, with no current-CPU page, no `cpu::id()`
call and no new crate, reproduces the failure exactly:

```
let next_root = match sched.threads.get(next).unwrap().space.as_ref() {
    Some(space) => space.ttbr0(),
    None => crate::arch::mmu::reserved_root(),
};
```
in place of the `.map(|s| s.ttbr0()).unwrap_or_else(...)` chain gives `map_new` 228,356. Two
spellings of the same value, and a 26.4% benchmark failure between them.

The map path's object code is byte-identical in both builds: `AddressSpace::map_at` at `0x8e`,
`AddressSpace::map_new` at `0x148`, the monomorphised `Mapper::map` at `0x331`,
`memory_region::retype_page` at `0x144`. Nothing on the path that `map_new` times got larger.

### What it actually is: a fixed lump inside a window that is too short

Sweeping the iteration count against both spellings settles it.

| `MAP_ITERS` | old chain | the `match` | delta | reads as |
|---|---|---|---|---|
| 64 (what ships) | 180,604 | 228,356 | **47,752** | **+26.4%** |
| 192 | 529,532 | 577,284 | **47,752** | +9.0% |

The delta is the same 47,752 ticks at both sizes, so it is not work per map. The marginal cost of
one map is `(529,532 - 180,604) / 128 = 2,726` ticks in the old build and
`(577,284 - 228,356) / 128 = 2,726` in the new one, identical. It is a fixed lump that lands inside
the timed window. `47,752 / 9,594` is 4.98 `yield_switch` iterations, so the reading that fits the
arithmetic was roughly five context switches landing in the window in one build and not the other.
The periodic timer's phase moves against a `schedule()` of a different length. *(Corrected the same
day: the lump is one preemption, not five. See milestone 541's entry below.)*

Two consequences:

- Every other row moves 0.3% to 0.6%, and in the opposite direction; `coremark` moves 0.01%. The
  `match` form is marginally cheaper on the switch path. `map_new` is the lone outlier by two orders
  of magnitude.
- At 192 iterations the identical lump reads as +9.0% and passes the 10% tripwire. The row is only
  180,604 ticks, an order of magnitude smaller than every other non-`spawn` row, so a handful of
  preemptions is a quarter of it. `map_new` as it ships cannot tell a regression in the map path
  from the scheduler's code moving by a few hundred bytes.

### The x86_64 leg is in CI, and the comment saying otherwise is stale

The pull request's premise, quoted from `.github/workflows/ci.yml` itself, was that this had sat
undetected because nothing pulls the x86_64 tripwire. That stopped being true on 2026-09-15, when
`ba99c835` added `script/bench --x86 --check` to the `bench` row of `script/ci-build`. That row is
what the `bench (icount regression tripwire)` job runs. It caught this on the first push, in 5m10s,
which is why that job is red. The `BUGS` comment beside the step is what was out of date, and it
was corrected in the same commit as this entry.

### BUGS

All three were closed the same day by milestone 541 (a timed window that excludes preemption),
whose entry is directly below. They stay as written because the second one was wrong.

- This entry attributes the number and does not fix the benchmark. `map_new`'s window is short
  enough that whether a preemption lands in it dominates it, which is a defect in the benchmark
  rather than in either spelling of `schedule()`. See
  [design/roadmap/541](../../design/roadmap/0541-a-timed-window-that-excludes-preemption.md) (a timed
  window that excludes preemption), which is this proposal promoted and built.
- The five-context-switches reading is arithmetic that fits, not an instrumented count. The lump is
  a measurement; its composition is an inference from `47,752 / 9,594`. A lane that fixes the
  benchmark should count the preemptions directly. It did, and the count is one.
- No baseline was re-saved. `map_new`'s baseline of 180,604 is the old spelling's number and still
  reproduced on `main` exactly. 541 re-saved all three on calef's ruling; the x86_64 row reads
  176,491 in `bench/baseline-x86_64.txt` today (re-recorded for `nightly-2026-09-23`).

### Withdrawn the same day: "the x86_64 tripwire is 26% off, and nothing has ever pulled it"

An earlier 2026-09-21 entry, written by the lane that found the failure, said `map_new` failed at
228,380 against 180,604 and failed identically with its own change compiled out (224,332). It read
that as somebody else's regression, predating it and never caught because the x86_64 leg was not in
CI. It deliberately left the baseline red rather than bless an unattributed 26%. The attribution
above refutes both claims: the regression was never on `main`, and the x86_64 leg has been in CI
since 2026-09-15 (`ba99c835`). Condensed here on 2026-09-24; the original text is in
`git log -p notes/benchmarks.md`.

## 2026-09-21: one preemption, not five, and a window that can refuse it

Milestone 541, the entry above promoted and built. Two findings, and the first corrects the entry
above.

### The lump is one preemption, and a preemption does not cost what a yield costs

Counting directly, with a probe reading a per-core preemption counter across exactly the timed
window:

| build | preemptions in the window | `map_new` |
|---|---|---|
| old chain | **0** | 180,604 |
| the `match` | **1** | 228,356 |

So the whole 47,752-tick lump is one timer preemption. The "about five" reading divided it by
`yield_switch`'s 9,594 ticks per iteration, and that denominator is the wrong event. A `yield_switch`
iteration is a voluntary round trip between two kernel threads that are both immediately runnable. A
timer preemption is a trap, a dispatch, an EOI and a `schedule()`. It then lasts however long it
takes for this thread to run again, which nothing in the benchmark bounds.

The spread on that last term is the substantive half of the correction. The same single preemption
measured at 4,096 iterations costs 6,670 ticks, against 47,752 at 64. That is a factor of seven for
the same event, depending only on what else was runnable when it landed. So raising `MAP_ITERS` is
not the fix it looks like: the disturbance is an unknown number of unknown-sized events, not a
constant divided by a larger window.

### Preemption can be excluded under `-icount`, and this is the measurement

The open question the proposal could not answer. Masking interrupts across the window
(`arch::interrupts::disable` / `restore`, which all three architectures already implement):

| | old chain | the `match` | delta |
|---|---|---|---|
| unmasked | 180,604 | 228,356 | **47,752** |
| masked | 180,537 | 180,537 | **0** |

The row is byte-identical across the perturbation that used to move it by a quarter. The `-icount`
trade is real and taken deliberately. All vCPUs share one virtual clock, so a masked window excludes
what the rest of the machine would have done inside it. A row named `map_new` should exclude that;
`yield_switch` and `spawn_reap` must not.

### The window is ~2.5% of a tick period on every architecture

This says x86_64 was not special. It is arithmetic over one measurement of each:

| arch | `map_new` window | counter | window in real time | one preemption, priced | as a fraction |
|---|---|---|---|---|---|
| aarch64 | 15,870 ticks | 62.5 MHz | 254 us | ~1,759 ticks | **11.1%** |
| riscv64 | 2,410 ticks | 10 MHz | 241 us | ~112 ticks | **4.6%** |
| x86_64 | 180,537 ticks | TSC | ~181 us | 6,670 to 47,752 | **3.7% to 26.4%** |

`TICK_HZ` is 100 on all three, so the scheduler tick period is 10 ms and the window covers about 2.5%
of one. Whether a tick lands inside it is a one-in-forty coin flip on the phase the boot left the
timer in, and unrelated code-size changes move that phase. On aarch64 the row would fail the 10% tripwire on a
single preemption; the coin had not come up heads there yet. The per-preemption prices for aarch64
and riscv64 are lower bounds, measured at 4,096 iterations where the preempted thread resumed
promptly.

### BUGS

- `map_el0` has the same shape and is not fixed. It times a mapping loop from EL0, and a kernel
  cannot mask interrupts around a window it does not own. It has not been measured for this.
- The per-architecture preemption prices come from one build each, at a different iteration count
  than the one that ships. The x86_64 row shows the same event varying by a factor of seven, so read
  them as orders of magnitude.
- Nothing sweeps the phase. "One in forty" is the ratio of two measured durations, not a measured
  failure rate.

### The counter cost 150 bytes of IPC fastpath, and the increment was not why

`script/fastpath-footprint` was not on this lane's gate list and caught this after the fact: riscv64
`ipc_send_receive` at 5.4% over a 5% bound, `syscall_entry` at 6.8%. Three plausible causes were all
wrong.

| what was changed | riscv64 `ipc_send_receive` |
|---|---|
| base | 4,734 |
| the milestone as first written | 4,884 |
| ...with the `fetch_add` deleted, the field kept | **4,884** |
| ...with the field moved to the end of the struct | **4,884** |
| base plus the `cpu::PerCpu` field and nothing else | **4,884** |
| the counter in its own array, `PerCpu` untouched | **4,734** |

It was `size_of::<PerCpu>()` going from 128 to 136. `PERCPU[id]` indexes an array of that struct, so
the address is `base + id * size_of`. At 128 that is a shift, and `cpu::current()` inlines to a
couple of instructions at each of its many call sites, several on the IPC fastpath. At 136 every one
of them grows. The counter moved to its own array, and the fastpath is byte-identical to base on all
three architectures.

The instrument lesson: between the two shapes every row of `script/bench` moved by at most 0.12%. A
tripwire on time could not see a change that a tripwire on size failed on. Neither substitutes for
the other. `map_new`'s own defect was the mirror image: a row that moved 26.4% while the code was
byte-identical. [The fastpath footprint appendix](fastpath-footprint-gate.md) describes the size
gate.

`kernel/src/cpu.rs` now asserts the size is a power of two. x86_64 is exempt: the struct there
already carries `x86_trap`, is already 152 bytes, and its gate is green with room (+1.0%, +1.4%,
+3.9%). The exemption rests on a measurement, because asserting a property the tree does not hold
teaches people to route around the gate.
