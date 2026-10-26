# The +6.5% drift, decomposed (milestone 300 (decompose the icount baseline drift))

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds the 2x2x2 grid that attributed the
2026-09-15 drift to one un-gated switch-tuple residual, with the dates, tables and corrections
behind them. Name: ratified 2026-09-24 (calef); [the naming record](README.md) holds it.*

## 2026-09-15: the baselines drifted +6.5%, and it was one feature's un-gated switch-tuple residual, not the toolchain (milestone 300)

The aarch64 and riscv64 baselines were last saved 2026-08-27 (commit `a79fdb95`). By 2026-09-15
`main` sat ~+6.5% over them on the switch-heavy benchmarks, ~19 nightly bumps and ~150 commits
later, each step under the 10% tripwire. Milestone 299's lane surfaced the gap and proposed
re-baselining (PR #883). calef's constraint: prove what moved each number before blessing it,
because "a silent regression laundered into the floor is exactly what this milestone exists to
prevent."

### The method: a 2x2x2 grid, not a single before/after

A naive `current - baseline` conflates three variables that all changed in the window. They are the
code, the pinned nightly (`nightly-2026-08-27` -> `nightly-2026-09-15`), and the dev Mac's QEMU
(`11.0.2` -> `11.1.1`, upgraded 2026-08-28, after the baseline save). Since icount counts guest
instructions, in principle the emulator version is part of what a number means. That is why
`.qemu-version` is pinned and `script/qemu-check` warns on a mismatch. Only 11.1.1 is installable
locally now, so the QEMU term had to be measured out rather than assumed away.

Four measurements per arch, QEMU held at 11.1.1 throughout, isolate each term:

| Point | Code | Nightly | What it isolates against the point above |
|---|---|---|---|
| **A** | `a79fdb95` | 2026-08-27 | the committed baseline (recorded on QEMU **11.0.2**) |
| **B** | `a79fdb95` | 2026-08-27 | **B - A = QEMU** (same code, same nightly; only 11.0.2 -> 11.1.1 differs) |
| **C** | `HEAD` (`1e9a8a14`) | 2026-08-27 | **C - B = code** (a79fdb95 -> HEAD, same nightly) |
| **D** | `HEAD` | 2026-09-15 | **D - C = toolchain** (08-27 -> 09-15 nightly, same code) |

icount is deterministic, so each point is one run, exact. D is the re-saved baseline.

### The result: QEMU ~0, toolchain ~0, code is the whole move

aarch64 (ticks; components per the grid above):

| benchmark | baseline A | QEMU (B-A) | code (C-B) | toolchain (D-C) | new baseline D | total |
|---|---:|---:|---:|---:|---:|---:|
| yield_switch | 1096445 | -1 | **+71205** | 0 | 1167649 | +6.5% |
| ctx_switch | 2909256 | +1 | **+179446** | +617 | 3089320 | +6.2% |
| spawn_el0 | 1223065 | -2334 | **+63485** | -69 | 1284147 | +5.0% |
| broker_rtt | 2076750 | 0 | **+77145** | 0 | 2153895 | +3.7% |
| call_reply | 1040221 | 0 | **+38456** | -1 | 1078676 | +3.7% |
| relay_rtt | 2027957 | 0 | **+72006** | 0 | 2099963 | +3.6% |
| ipc_rtt_el0 | 10739771 | -3667 | **+346421** | +16329 | 11098854 | +3.3% |
| ipc_rtt | 1026533 | +1 | **+25393** | 0 | 1051927 | +2.5% |
| spawn_reap | 205713 | 0 | +5908 | 0 | 211621 | +2.9% |
| sink_throughput | 4413308 | +286 | +89039 | +287 | 4502920 | +2.0% |
| null_syscall | 405003 | 0 | +225 | 0 | 405228 | +0.06% |
| coremark | 20917371 | 0 | +417 | -1904 | 20915884 | -0.01% |
| map_new | 15743 | 0 | 0 | +1 | 15744 | +0.01% |
| map_el0 | 388673 | 0 | +2 | -1 | 388674 | +0.00% |

riscv64:

| benchmark | baseline A | QEMU (B-A) | code (C-B) | toolchain (D-C) | new baseline D | total |
|---|---:|---:|---:|---:|---:|---:|
| yield_switch | 183768 | 0 | **+12239** | -97 | 195910 | +6.6% |
| ctx_switch | 492350 | +1 | **+30318** | -104 | 522565 | +6.1% |
| spawn_el0 | 192808 | 0 | **+10614** | +209 | 203631 | +5.6% |
| broker_rtt | 349322 | 0 | **+12995** | +16 | 362333 | +3.7% |
| call_reply | 174952 | 0 | **+6490** | -16 | 181426 | +3.7% |
| relay_rtt | 339292 | 0 | **+12191** | -68 | 351415 | +3.6% |
| ipc_rtt | 169463 | -1 | **+5511** | +179 | 175152 | +3.4% |
| ipc_rtt_el0 | 1821100 | +2230 | **+55867** | -808 | 1878389 | +3.1% |
| spawn_reap | 32975 | 0 | +1004 | 0 | 33979 | +3.0% |
| sink_throughput | 747166 | 0 | +15176 | 0 | 762342 | +2.0% |
| null_syscall | 72229 | 0 | +1 | 0 | 72230 | +0.00% |
| coremark | 3654773 | 0 | -394 | +1 | 3654380 | -0.01% |
| map_new | 2396 | 0 | 0 | 0 | 2396 | 0 |
| map_el0 | 62308 | +1 | 0 | -1 | 62308 | 0 |
| rfence_self | 5991 | 0 | 0 | 0 | 5991 | 0 |

The QEMU column is noise, every entry sub-0.2% and of both signs. The 11.0.2 -> 11.1.1 upgrade does
not move icount, which is now measured rather than assumed. The toolchain column is also noise:
`nightly-2026-08-27` and `nightly-2026-09-15` emit byte-identical instruction counts on the same
code (`yield_switch` 1167649 on both aarch64 nightlies). This refutes the proposal's framing, which
read the drift as the nightly bump's codegen. The entire move is code, and it is the same code on
both ISAs: near-identical percentages, which codegen noise would not produce.

### The code component is one commit: the cycle-counter grant of milestone 139 (drive the unsafe count down), at the switch

`git bisect` on aarch64 `yield_switch` over `a79fdb95..HEAD` (toolchain fixed at 09-15) landed on
`57399c34` (2026-09-02), *"sched: write the cycle-counter grant at the context switch"*, DECISIONS
139 option 4. Benching the full suite at that commit and its parent `61c6a780` isolates the switch
write:

| benchmark | parent 61c6a780 | at 57399c34 | delta | per-iter |
|---|---:|---:|---:|---:|
| yield_switch | 1096444 | 1182988 | +86544 | +43.3/switch |
| ctx_switch | 2909838 | 3126218 | +216380 | +43.3/switch |
| relay_rtt | 2027957 | 2115135 | +87178 | +87/iter |
| broker_rtt | 2076749 | 2163016 | +86267 | +86/iter |
| call_reply | 1040221 | 1083471 | +43250 | +43/iter |
| ipc_rtt | 1026533 | 1069550 | +43017 | +43/iter |
| ipc_rtt_el0 | 10757288 | 11137698 | +380410 | +76/iter |
| coremark | 20915467 | 20912818 | -2649 | ~0 (compute) |
| map_new / map_el0 | 15744 / 388673 | 15744 / 388450 | ~0 | ~0 |

`yield_switch` and `ctx_switch` move by the identical per-switch amount (+43.3 ticks). Every IPC
benchmark moves in proportion to how many switches it does. Pure compute (`coremark`) and the
no-switch map primitives do not move. That is a single per-context-switch cost, not a scatter of
regressions. A later commit, milestone 299's `44890a8a` (keeping the port grant off the non-x86
switch path), trimmed the aarch64/riscv peak from ~+43 to the net ~+35.7 ticks/switch at HEAD.

### Classification, corrected: a removable regression, not an intended feature cost (milestone 300)

PR #885's first pass classified this as intended feature work, reasoning that 139 is a decided
feature whose own text places the cost at the switch. It re-saved the baselines to absorb it, and
flagged that the delivered +35.7 ticks overran 139's estimate of ~2-3 by an order of magnitude,
guessing a non-inlined arch function. This section's reading is the one in force; the pass's text is in
`git log -p notes/benchmarks.md`.

The reason is milestone 237 (the cycle-counter grant), which made the cycle-counter grant a measurement-only feature that
ships OFF. `set_cycle_counter_grant` is `#[cfg(any(test, feature = "cycle_counter_grant"))]`, and in
a feature-off binary the arch write, `pmuserenr`/`scounteren`, and the grant field do not appear at
all. So the +35.7 ticks/switch on `main` were not paying for a shipping feature. They were a
residual 237 left behind. 139 threaded the grant through the shared context-switch tuple as an extra
element. When 237 gated the feature off, it turned that element into a constant `false` and trusted
the optimizer to fold it away (a `#[cfg]` is not allowed on a tuple element, which is why 237
reached for the fold). The fold works in the release build but not in the debug build the icount
gate measures. So the const-`false` element, its read, and a gated-off
`install_cycle_counter_grant` call all stayed in the shipping switch and cost the +35.7.

This is the class milestone 299 (the x86 port-range capability) found and fixed for the x86 port grant in the same function
(`44890a8a`): a per-switch value threaded through the shared tuple and trusted to fold, which it did
not. Milestone 300 applies 299's fix to the cycle-counter grant. The grant is read and installed
behind a `#[cfg(any(test, feature = "cycle_counter_grant"))]` at the switch site, into a gated local
rather than the tuple. The shipping switch tuple is back to its pre-139 width
`(prev_slot, next_ctx, next_root)`.

Recovery, measured on both ISAs (QEMU 11.1.1, `nightly-2026-09-15`, debug icount; #885 floor ->
milestone 300 fix, with the pre-139 parent `61c6a780` from the bisect table for reference):

| benchmark | #885 floor | 300 fix | recovered | pre-139 | residual over pre-139 |
|---|---:|---:|---:|---:|---:|
| aarch64 yield_switch | 1167649 | 1101149 | -66500 (33.3/sw) | 1096444 | +4705 (+0.43%) |
| aarch64 ctx_switch | 3089320 | 2922971 | -166349 (33.3/sw) | 2909838 | +13133 (+0.45%) |
| riscv64 yield_switch | 195910 | 184875 | -11035 (5.5/sw) | 183768 | +1107 (+0.60%) |
| riscv64 ctx_switch | 522565 | 495050 | -27515 (5.5/sw) | 492350 | +2700 (+0.55%) |

`coremark`, the pure-compute control, stayed flat across the fix (aarch64 20915884 -> 20915599,
riscv64 3654380 -> 3654349, both ~0.001%). The fix recovers ~91-93% of the drift and lands within
~0.5% of the pre-139 numbers. The sub-1% residual is the codegen noise floor, not a remaining
threaded cost. The tuple is provably 3-wide and no cycle-counter code compiles with the feature off,
so there is nothing left to gate. Untouched benchmarks shift by comparable sub-tenth-percent amounts
whenever switch-path code changes, because the compiler remakes whole-crate inlining decisions. The
baselines were re-saved on `nightly-2026-09-15` against these recovered numbers, replacing #885's
re-baseline.

*(Since then, 2026-09-24: the milestone 300 floors have themselves been re-recorded. The current
floors carry `# toolchain: nightly-2026-09-23`, from commit `a66e8c5f9` on 2026-09-23.)*

139's estimate was right after all. It estimated *"the compare"* `switch_user_root` already pays
(~2-3 ticks when nothing is granted), and #885's guess of a non-inlined arch function on the hot path
was wrong: the arch function ships off and is absent from a feature-off binary. Once the tuple
residual is gone the delivered cost matches the estimate, to within the noise floor. The 139
feature, when built (`--features cycle_counter_grant`), still costs ~136 bytes in the release switch
(see `kernel/Cargo.toml`). That is its legitimate on-cost.

### A baseline is saved only from the shipping feature set (calef, 2026-09-15)

`bench --save` must run with the shipping feature set: every measurement-only feature
(`cycle_counter_grant`, `soak_test`, `job_mix`, and any future one of that kind) OFF, and every
shipping feature ON. A baseline saved from an opted-in build bakes a cost that never ships into the
tripwire floor. Both directions fail. A shipping feature turned off for a bench understates
production, and a measurement feature turned on overstates it. The one legitimate bench-only feature
is `icount` itself, and it draws the line. It instruments observation: it changes how `timer::now()`
reads the clock, not the switch logic under measurement. A bench feature may change how you observe,
never what you measure.

### x86_64 recovered too: the residual was the *shared* tuple, not an aarch64/riscv path

Milestone 299's lane re-saved `bench/baseline-x86_64.txt` on 2026-09-15. This milestone's brief
carried #885's assumption that x86_64 was unaffected because "the grant is a no-op on x86".
Measured, that is wrong. The `next_cycle_counter` element sat in the switch tuple on every ISA; it
was never `target_arch`-gated. So the x86 debug icount build carried the const-`false` element and
its gated-off install like the other two, and the milestone 300 fix recovers it:

| benchmark | #885 floor | 300 fix | recovered |
|---|---:|---:|---:|
| yield_switch | 20080160 | 18903108 | -1177052 (-5.9%) |
| tss_iomap_switch | 24818161 | 23661444 | -1156717 (-4.7%) |
| tss_iomap_lazy_switch | 24487313 | 23309946 | -1177367 (-4.8%) |
| tss_iomap_lazy_nop | 20490158 | 19323946 | -1166212 (-5.7%) |
| coremark | 306262395 | 306261408 | -987 (~0, control) |

`bench --x86 --check` passed against the #885 floor only because ~5.9% is under the 10% tripwire.
Leaving that floor in place would bake the removable regression into the x86 tripwire. So
`bench/baseline-x86_64.txt` is re-saved against the recovered numbers alongside the other two. The
IPC benchmarks moved in proportion to their switch counts (`relay_rtt` 35208246 -> 34051949,
`broker_rtt` 36208747 -> 35054295). The no-switch primitives (`map_new`, `map_el0`,
`null_syscall`) stayed byte-identical, the same signature as the other two ISAs.
