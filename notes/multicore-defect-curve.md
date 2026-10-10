# The multicore defect-discovery curve: its data, and how a soak appends to it

*Owned by milestone 201 (is multicore reliability converging), which scopes
[`design/fatal-risks/README.md`](../design/fatal-risks/README.md)'s risk 5. Name provisional (this file's stem),
minted 2026-09-24 by the lane that re-derived 201's seeds; naming is an architect's.*

Risk 5 says the concurrency is wrong in ways QEMU cannot show, arriving one at a time, forever.
Milestone 201 turns that into a measurement: count the multicore defects found per unit of stress on
real silicon, per architecture, and watch the slope. A flattening curve is a confidence. A linear one
is the red result. This page is where the points live, so that milestone 225 (run the soak on radon,
argon and xenon) and every later run can append rather than re-argue.

## Two tables, because a curve is a numerator over a denominator

**Exposure** is the denominator: one row per boot that ran a stress workload, whether or not it
found anything. A boot that found nothing is the most important row there is, because it is the
flat part of the curve, and it is the row most likely to go unrecorded.

**Defects** is the numerator: one row per multicore defect, with the exposure row that found it, or
`none` if nothing was measuring stress when it turned up.

The curve for one architecture is then: walk that architecture's exposure rows in date order, keep a
running total of crossings, and plot the running count of defects of class `race` or `multicore`
against it. Hours and boots plot the same way when a second view is wanted. Never pool architectures (aarch64, riscv64 and x86_64 have different memory
models; see 201's `BUGS`). Never plot a defect whose exposure is `none`: it has no x-coordinate.

### Which denominator

calef ruled on 2026-09-25 (UTC) that the curve is judged on crossings, and 201's block
records it. Hours and boots are still recorded in every row, so a later question can be answered
without re-running anything:

- **hours**, which is what milestone 201 names;
- **crossings**, the judged unit: the soak's `crossings=` counter at the last beat (cross-core
  migrations, from `sched::migrations()`), which [`soak.md`](soak.md) argues is the honest unit
  because clock time on a saturated machine mostly repeats one interleaving. A board's rows count
  toward risk 5's verdict once the architecture reaches 10 million crossings across at least 3
  boots, stated before the run (§259 (a multicore soak counts toward risk 5 at ten million
  crossings over three boots)); a defect found does not reset the total;
- **boots**, since each boot is a fresh draw of the placement lottery and PCT's model says
  independent starts multiply the chance of finding a shallow bug where a long run does not.

## Exposure rows

| Column | What goes in it |
|---|---|
| `id` | `E` plus a number, never reused |
| `start (UTC)` | date and time of power-on, UTC; the zone of any source time is converted, and a guess is marked `~` |
| `machine` | a board name (`radon`, `argon`, `xenon`), or `hvf` for patagonia's physical cores. QEMU and loom rows do not belong here: they are not the silicon this risk is about |
| `arch` | `aarch64`, `riscv64`, `x86_64` |
| `cores` | cores online, from the boot's own `smp:` line |
| `build` | the commit SHA the image was built from |
| `workload` | `soak-test`, `soak-test-reboot`, or a named alternative; a workload that does not cross cores is recorded as such and never counted (201's `BUGS`: idle hours flatten the curve for free) |
| `hours` | wall time from the first beat to the last, from the last beat's `t=` |
| `crossings` | the last beat's `crossings=` |
| `beats` | the last beat's `beat=` |
| `result` | `clean` if the last beat has `refused=0 mismatch=0 stalled=0` and no `soak-test: FAILED` line; otherwise the defect ids it opened |
| `source` | the log's path in the tree, or the note section that records it |

### How a `soak-test` log becomes a row

`script/board-console --exposure <log> --machine <name> --build <sha> --start '<utc>'` does
everything below and prints the row (milestone 225, 2026-10-05). The rules are kept here because
they are what it implements, and a reader checking a row should not have to read Rust.

Every beat is one line, printed by `kernel/src/soak.rs`:

```
soak-test: t=<s>s beat=<n> rounds=<n> rate=<n>/s wakes=<n> wakerate=<n>/s workers=<n> refused=<n> mismatch=<n> stalled=<n> drifted=<n> crossings=<n> remote=<n> steals=<n> deferred=<n>
```

Take the last such line of the boot. `hours` is its `t=` divided by 3,600, `crossings` and
`beats` are read off it, and `result` is `clean` when its three failure counters are all zero and no
`soak-test: FAILED` line appears anywhere in the log. Milestone 297 (`soak` becomes `soak-test`)
landed on 2026-09-14; logs from before it spell the prefix `soak:`, which `crates/board_console`
already normalizes.

A non-clean boot opens a defect row with class `unclassified`, and it stays unclassified until
somebody has read the dump. That is the step the retracted VisionFive 2 reading skipped in the other
direction: it was classified as a defect before it was understood.

Check the first beat before recording anything: `wakerate` about `100 * harts` and `crossings`
rising between beats ([`soak.md`](soak.md), "On radon at a bench"). A boot that fails that check is
recorded with workload `soak-test (not crossing)` and excluded from the curve.

### The rows

| id | start (UTC) | machine | arch | cores | build | workload | hours | crossings | beats | result | source |
|---|---|---|---|---|---|---|---|---|---|---|---|
| E1 | 2026-09-03 ~20:04 | radon | riscv64 | 4 | not recorded | soak-test | ~0.33 | ~3,000 at beat 12, final not recorded | not recorded | clean as far as recorded | [`soak.md`](soak.md), "radon, on real silicon" |
| E2 | 2026-09-03 ~20:24 | radon | riscv64 | 4 | not recorded | soak-test | 2.97 | 5,507 | 2,137 | clean | [`soak.md`](soak.md), "The three-hour run" |
| E3 | 2026-09-04 ~00:06 | radon | riscv64 | 4 | not recorded | soak-test, census build | ~0.4, end not recorded | not recorded | not recorded | clean as far as recorded | [`soak.md`](soak.md), "The three-hour run" |
| E4 | 2026-09-25 ~01:02 | radon | riscv64 | 4 | `9e879f1e7` | soak-test | 8.16 | 4,108,581 | 5,818 | clean | `bench/radon-2026-09-25/soak-8h.log`; [`visionfive2.md`](visionfive2.md), "The eight-hour soak, 2026-09-25" |
| E8 | 2026-10-10 ~18:12 | radon | riscv64 | 4 | `23f0c8d87` | soak-test-reboot | 0.03 | 104 | 24 | clean | `bench/radon-2026-10-10/592-optionb-four-cycles.log` |
| E9 | 2026-10-10 ~18:15 | radon | riscv64 | 4 | `23f0c8d87` | soak-test-reboot | 0.03 | 25,223 | 24 | clean | `bench/radon-2026-10-10/592-optionb-four-cycles.log` |
| E10 | 2026-10-10 ~18:18 | radon | riscv64 | 4 | `23f0c8d87` | soak-test-reboot | 0.03 | 22,116 | 24 | clean | `bench/radon-2026-10-10/592-optionb-four-cycles.log` |
| E11 | 2026-10-10 ~18:21 | radon | riscv64 | 4 | `23f0c8d87` | soak-test-reboot | 0.03 | 3,493 | 24 | clean | `bench/radon-2026-10-10/592-optionb-four-cycles.log` |
| E12 | 2026-10-10 ~18:24 | radon | riscv64 | 4 | `23f0c8d87` | soak-test-reboot, ended by power-off | 0.03 | 16,462 | 22 | clean | `bench/radon-2026-10-10/592-optionb-four-cycles.log` |

E8 through E12 are milestone 592's option-B evening, `23f0c8d87`, one capture: four 120-second draws each ended by the direct AXP15060 write (the reset milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write) built, proven twice-deterministic in the same log), and a fifth boot ended when calef powered the board off. The crossings spread across five identical 120-second builds, 104 to 25,223, is the boot-lottery distribution the series milestone 249 (the boot lottery is sampled by a person walking to the board) wanted and had never sampled: a 243x range on identical silicon and build, zero defects throughout. E5 through E7, the same evening's plain-soak boots, are recorded by the 225 lane's pull request; this block leaves their numbers to it to keep one home per fact.

The start times are Pacific local times from `notes/soak.md` converted to UTC; the note does not
state its zone, and the conversion is inferred from the commits that recorded them (2026-09-03
20:45 UTC and 2026-09-04 00:11 UTC), which only a Pacific reading fits. None of the three logs is
in the tree and none records its build, which is why their columns are thin. So radon's curve
starts at roughly three and a half hours and 5,500-plus crossings with zero defects, which is a
confidence about one architecture and one workload, not a verdict about anything.

E4 is the first row with a log in the tree and a build recorded, and on the crossings axis it
dwarfs the three before it: 4.1 million against about 8,500 recorded in total, from one boot of 8.16
hours that drew the fast arrangement. So radon's curve now stands at about 4.12 million crossings
over four boots and 12 hours with zero defects. It is one draw of the placement lottery, and on
the boots axis it is still four points.

## Defect rows

| Column | What goes in it |
|---|---|
| `id` | `D` plus a number, never reused; a retracted row keeps its id |
| `found (UTC)` | the date of the first commit recording it, UTC |
| `arch` | as above, or `model` for a loom finding, which runs C11 and no architecture |
| `instrument` | `silicon` (with the board), `hvf`, `qemu-tcg`, `loom`, `audit` |
| `exposure` | the exposure row that found it, or `none` |
| `class` | one of the five below |
| `silicon-only` | `yes` only if it has been seen on silicon and has not reproduced under QEMU; `no` if QEMU or loom showed it; `unknown` otherwise |
| `fixed` | date (UTC) and milestone, commit or pull request; or `open` |
| `source` | where the defect is recorded at the code |

Classes: `race`, a defect that depends on the interleaving of two or more cores; `multicore`,
one that exists only with more than one core but is deterministic once placement is fixed;
**`instrument`**, a defect in the stress workload or its hook rather than in the kernel;
**`retracted`**, a reading since overturned, kept so it is never counted again by someone who read
the old version; `unclassified`, not yet understood. Test-only defects (an assertion wrong under
load) do not belong here; [`load-sensitive-assertions.md`](load-sensitive-assertions.md) is their
register.

### The history before the curve

Everything below was found before any exposure was measured, so every row's exposure is `none` and
none of them is a point on the curve. They are here because risk 5's second clause ("the bugs appear
only on silicon") is a claim about this column, and the `silicon-only` column is the evidence.

| id | found (UTC) | arch | instrument | class | silicon-only | fixed | source |
|---|---|---|---|---|---|---|---|
| D1 | 2026-07-23 | aarch64 (the only port then) | qemu-tcg, a suite flake (8 of 10) | race | no | 2026-07-23, the `on_cpu` deferral | [`intrusive-queues.md`](intrusive-queues.md), the wake-before-switch-out race |
| D2 | 2026-07-27 | riscv64 | qemu-tcg | multicore | no | 2026-07-27 | [`riscv-parity-scope.md`](riscv-parity-scope.md), the PLIC boot-hart-lottery bug |
| D3 | 2026-08-01 | riscv64 | qemu-tcg (`-cpu thead-c906`, 2 of 4 runs) | race | no | 2026-08-15, `crates/memory_corruption_canary_gate` | [`interleaving.md`](interleaving.md), the canary's serialization |
| D4 | 2026-08-04 | model | loom | race | no | 2026-08-04, milestone 80 | [`interleaving.md`](interleaving.md), "What loom found" (the seqlock fence) |
| D5 | 2026-08-14 (loom's date; first seen earlier) | not recorded | not recorded | race | unknown | before 2026-08-14 | [`scheduler.md`](scheduler.md), the steal edge of the switch-out window |
| D6 | 2026-08-14 | riscv64 | silicon (radon, boot 8) | retracted | n/a | retracted 2026-08-15 | [`visionfive2.md`](visionfive2.md), fifth bench stop |
| D7 | 2026-08-18 | riscv64 | qemu-tcg, 1 of 45 loaded runs, two cores | race | no | 2026-08-18, pull request #316 | [`object-revocation.md`](object-revocation.md), the double free |
| D8 | 2026-08-26 | x86_64 | qemu-tcg (10 of 10) | race | no | 2026-08-26 | `arch::x86_64::ap_boot`'s `BUGS` #2 (the TLB shootdown) |
| D9 | 2026-08-26 | x86_64 | qemu-tcg (about half of runs) | multicore | no | 2026-09-18, milestone 316 | `arch::x86_64::ap_boot`'s `BUGS` #3 (which core booted) |
| D10 | 2026-08-26 | x86_64 | qemu-tcg (26 of 40 at `-smp 4`) | race | no | 2026-09-19, milestone 161 | `arch::x86_64::ap_boot`'s `BUGS` #1 (the lost check-in) |
| D11 | 2026-09-02 | riscv64, and a loaded host | qemu-tcg | instrument | no | 2026-09-02, milestone 221 | [`soak.md`](soak.md), "Two bugs this mechanism had" (both) |
| D12 | 2026-09-17 | x86_64 | audit (milestone 313), then qemu-tcg (7 of 12) | race | no | 2026-09-23, milestone 315 | `sched::delete_port_range_caps_impl`, the per-core port revoke |
| D13 | 2026-09-19 | aarch64 | hvf, 5 of 9 recorded full runs | unclassified | unknown | open | [`hvf-leg.md`](hvf-leg.md), `a_std_program_serves_a_granted_listening_port` |

The milestones in the `fixed` column: milestone 80 (Loom: the hand-rolled atomic protocols,
model-checked), milestone 161 (the x86_64 kernel port), milestone 221 (the soak never crosses cores,
so build the hook that makes it), milestone 313 (the security audit that was due since August),
milestone 315 (a port revoke that reaches every core) and milestone 316 (which core booted: making
`NIFE_SMP=2` mean something on x86_64).

**What the table says about risk 5.** Eleven rows are real or instrument defects, and every one
whose instrument is recorded was shown by QEMU, loom or an audit; D5's is not recorded. The one silicon reading (D6) is retracted. The only row that has appeared on
physical cores and not under TCG is D13, and the evidence there (a host prober's connection landing
in a gap between two listeners, seen four times) points at the test harness rather than the kernel.
It stays `unclassified` until someone settles it, because an unclassified row is a candidate for
exactly the class risk 5 names and should not be quietly argued away.

## BUGS

- The history table is a lane's sweep, not a census. It was assembled on 2026-09-24 from
  `notes/`, the `BUGS` sections cited, and `git log`, by searching for the words these defects were
  recorded under. A multicore defect recorded under different words, or fixed in a commit whose
  message never said so, is missing. The rows found are the ones the tree already told a reader
  about, which biases the table towards defects somebody thought were interesting.
- D5's date is when loom modeled it, not when it was found. `notes/scheduler.md` records it as
  a race "observed on the machine" without a date or an instrument; `notes/interleaving.md` says the
  protocol's races "were found by flakes and bench boots first". Which it was is not recorded, and
  a bench boot would make it the one row that bears on risk 5's premise.
- E1 to E3 lack builds and final beats. Their logs were read at the bench and summarized into
  `notes/soak.md`, never committed. Milestone 225 should commit each boot's log under `bench/` so a
  row can cite a file rather than a paragraph.
- ~~Each row is copied off the last beat by hand.~~ Since 2026-10-05 (milestone 225),
  `script/board-console --exposure <log> --machine <name> --build <sha> --start '<utc>'` prints
  the row, the four figures and the two bench checks, and its test reads E4's log back into E4.
  The id stays `E?` on purpose: the next free one is read here, not guessed.
- Nothing computes the curve. It is a table a reader sums by hand. That is adequate at three
  rows and will not be at thirty; a script reading these two tables is small work, but it earns its
  keep only once 225 has appended enough rows to plot.
