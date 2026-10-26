# The `rfence_self` baseline row

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds the 2026-09-21 and 2026-09-23 readings
of one oscillating riscv64 row, with the dates, tables and corrections behind them. Name: ratified
2026-09-24 (calef); [the naming record](README.md) holds it.*

The current value, checked 2026-09-24: `bench/baseline-riscv64.txt` reads `rfence_self 5991 512`
under `# toolchain: nightly-2026-09-23`, re-recorded by `a66e8c5f9` (2026-09-23). Which cause moved
the row between 5991 and 6476 is still open; the 2026-09-23 section below says why.

## 2026-09-21: `rfence_self` did not get 7.5% faster, its baseline row got 7.5% wrong

*Contested since, and the 2026-09-23 section is the current state.* The row went back to 6476 on
2026-09-22 and measured 5991 again on 2026-09-23. There a second lane read the same movement as
compiler drift, not as a wrong hart count. This section records the checks.

calef asked for the fast benchmark to be chased the way a regression would be. On 2026-09-21
riscv64's `rfence_self` measured 7.49% under its committed baseline while every other row on every
architecture sat inside noise. The icount tripwire is one-sided by design, so nothing would have
raised it.

The tree did not get faster, and the benchmark did not stop measuring what it measures. The
committed number was wrong on the day of its commit.

### The measurement

`rfence_self` measured 5991 ticks, exactly the value the baseline held before commit `28e165e2`
raised it to 6476. `6476 - 5991 = 485`, and `485 / 6476 = 7.489%`: the whole reported improvement
is one row returning to a number it had never left.

Run at the commit that wrote 6476, and at its parent, the tree measures 5991 both times. Every
other riscv64 row at `28e165e2` reproduces what `28e165e2` recorded, several to the digit
(`spawn_reap 34277`, `ctx_switch 504958`, `spawn_el0 203942`). So the file came from a real run of
that tree, and one row in it did not.

Checks follow for each way the number could be fragile:

| checked | result |
|---|---|
| repeat runs, same build | byte-identical full tables, 3 of 3 |
| clean `cargo clean -p kernel` rebuilds | 5991 both times |
| six spinning host processes competing for cores | 5991 (`-icount shift=0,sleep=off` is load-immune, as intended) |
| the commit that recorded 6476, and its parent | 5991 and 5991 |

6476 appears nowhere else in the tree: not in the block for milestone 447 (a thread's vector
registers are its own), which that commit re-measured, not in the note, not in any commit message.

### What did reproduce: sensitivity to the hart count

`rfence_self` is sensitive to how many harts are online, even though it fences a mask naming only
the calling hart:

| harts online | `rfence_self` ticks |
|---:|---:|
| 1 (what `--riscv` boots) | 5991 |
| 2 | 6432 |
| 4 | 7153 |

Each is deterministic across runs. The committed 6476 sits 0.7% above the two-hart value and nowhere
near the one-hart value. That suggests a run made at the wrong hart count and does not prove one.
`xtask/src/bench.rs` sets `NIFE_SMP=1` unconditionally on this leg, so the only way to get there
today is to edit that line, which is how the table was produced.

The mask names the caller, so nothing is IPI'd. But OpenSBI's TLB path is not the same code on a
platform with other harts running. The kernel side is identical in all three rows.

### Does the fence still fence?

Yes, and nothing in the window touched it. `kernel/src/arch/riscv64/mmu.rs` has the two call sites
that matter, both unchanged. `flush_tlb` executes a local `sfence.vma` and *then*
`sbi_remote_sfence_vma` against `online_harts_mask() & !(1 << cpu::id())`. `flush_asid` does the
same with `sbi_remote_sfence_vma_asid`. Each skips the firmware call only when that mask is empty,
which on a one-hart boot is the correct answer.

The limit of the benchmark: `rfence_self` passes `me`, its *own* hart, so the cost is "getting into
firmware and back" rather than a shootdown. On the single-hart boot `--riscv` uses, the production
call sites issue zero RFENCEs, which the suite's own probe prints (`map_new_remote_fences 0 over 64
iters`). So this row prices a call the running kernel never makes in that configuration. It is a
floor for what an RFENCE costs, not the cost of a shootdown, and a change to the real shootdown
path would not move it.

### The tripwire consequence

`--check`'s slack is `base / 10`. A baseline of 6476 gives the row 647 ticks of headroom around a
true value of 5991. So the row could regress by 1132 ticks, 19% of its real cost, before the gate
fired. The corrected row restores ~600 ticks of slack around the number the tree produces.

### What was re-saved, and how

The single line `rfence_self 6476 512` was hand-edited to `rfence_self 5991 512`. Not `--save`,
which rewrites the whole file from one run and would have re-recorded fifteen rows this lane had no
mandate to move; other lanes were working on that question.

A `#` comment was put above the row saying what it is. That comment would not survive the next
`--save`, because `run_bench`'s save path builds the file from a fixed header plus one
`name ticks iters` line per result. That is a foot gun. It was a second argument for item 2 of
milestone 415 (sub-tripwire drift accumulates across baseline saves). That item's §190 (must a
baseline save record why it moved) asks whether a save should record a reason. A row that cannot carry a
reason cannot carry a correction either. *(Correction, 2026-09-24: milestone 302 merged on
2026-09-23 (#1126). `script/bench --save` now requires `--why` and writes a `# why:` line above the
numbers. The three baseline files today carry `# why: unrecorded, predates milestone 302`.)*

### One stale comment found in passing

`xtask/src/bench.rs`'s check loop was preceded by a comment claiming indented `#` lines "are
treated as data" by "a column-0-only check". The filter below it is
`.filter(|l| !l.trim_start().starts_with('#'))`, which handles indented comments correctly. It was
left as a finding, because `xtask/src/bench.rs` was shared scaffolding another lane was in.
*(Fixed 2026-09-24: the comment now says the column-0 hazard is what `trim_start` prevents,
rather than describing it as live.)*

## 2026-09-23: `rfence_self` has now been read two ways, and the file cannot settle it

The 2026-09-21 section concluded that the committed 6476 "was wrong on the day it was committed",
read it as a run at the wrong hart count, and hand-edited the row back to 5991. Two days later the
row was 6476 again. The lane clearing PR #1112 (the `nightly-2026-09-23` bump) measured 5991 to the
tick in three reproducible runs and read the same movement as compiler drift. *(Correction,
2026-09-24: this section first said "four days later"; 2026-09-21 to 2026-09-23 is two.)*

Both readings rest on arguments from outside the file. `bench/baseline-riscv64.txt` records a
number, a pinned nightly and a QEMU version. It does not record why the number holds that value, so
neither lane could check the other's cause without re-deriving it from commit messages and git
history.

### The oscillation, from committed text alone

No emulator ran for this table. It is `git show <commit>:bench/baseline-riscv64.txt` seven
times:

| commit | date | `# toolchain:` | `rfence_self` |
|---|---|---|---:|
| `e6833cd2` | 2026-09-20 | (stamp not yet added) | 5991 |
| `28e165e2` | 2026-09-20 | (stamp not yet added) | 6476 |
| `99f13dad` | 2026-09-21 | nightly-2026-09-20 | 6476 |
| `84e29394` | 2026-09-21 | (hand edit, no re-save) | 5991 |
| `d14ad408` | 2026-09-21 | nightly-2026-09-20 | 6476 |
| `483382c9` | 2026-09-21 | nightly-2026-09-20 | 5991 |
| `c64af73e` | 2026-09-22 | nightly-2026-09-22 | 6476 |

The row alternates between two values across saves under two different pins, with a hand
correction in the middle that a later `--save` rewrote. Which cause is right is open. Deciding it
needs an A/B across two nightlies at a fixed hart count, which nobody has run. Neither reading is a
reason to move the floor: `script/bench --save` commits a performance floor and is an architect's
call. *(Since then: `a66e8c5f9`, 2026-09-23, re-recorded the row at 5991 under
`nightly-2026-09-23`.)*

### `spawn_el0` under `nightly-2026-09-23`

Measured by the same lane: up ~6% on both aarch64 and riscv64, bit-identical across repeats, under
the 10% tripwire. Bit-identical repeats say it is the compiler and not the host. Being under the
tripwire means nothing stops the next floor from absorbing it.

### Why this is recorded here rather than proposed as work

The mechanism that would fix it already existed twice. Milestone 302 (a baseline records what it
was saved against, and a stale one fails loudly) asked `--save` to write a `# why:` line, a format
calef ratified on 2026-09-16. Item 2 of milestone 415 (sub-tripwire drift accumulates across
baseline saves) states the same obligation, with
[§190](../../design/decisions/0190-what-a-baseline-save-must-record.md) (must an icount baseline save
record why it moved) as its decision. Milestone 577 (the icount baselines predate the pinned
nightly) was promoted and closed in one act on 2026-09-23 rather than become a third statement
of it; this section holds its evidence. *(Correction, 2026-09-24: 302 has since merged, per the
2026-09-21 section's correction above.)*

`script/lint`'s baseline-toolchain check, added by `99f13dad`, is the half that landed first, and it
fired on PR #1112 as designed. It proves a floor was saved under the pinned nightly. It cannot prove
the numbers under that stamp are right.
