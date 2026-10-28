# The mutation census record: one row per crate, per census

*Names: `notes/mutation-census.md`, `script/mutation-census` and the two CSV files below are
**provisional**. Naming is an architect's (AGENTS.md); a lane ships a provisional name and says so.
Milestone 518 (a census that cannot be attributed is a number nobody can act on), 2026-09-20.*

[`notes/mutation-testing.md`](mutation-testing.md) is the ledger of triage: what a mutant is, what
each survivor means, and the three-way rule for what to do about one. This page is the time
series. It says what the score was, per crate, on each day the whole corpus was measured, and how
to ask which crates moved between two of those days.

The data is in the tree, versioned with the code it describes:

- [`notes/project-metrics/mutation-census.csv`](project-metrics/mutation-census.csv), one row per
  crate per census: `census,run,crate,tested,caught,missed,timeout,unviable,real_holes`. `real_holes`
  counts the crate's survivors that no algebraic argument answers, in helpers/mutation_real_holes.py's
  sense. `--add` computes it from the census's own shard output against the triage file of the
  recording day. A blank cell means the census predates the column; the live count for the newest
  census lives in
  [`notes/mutation-testing/survivor-classification.md`](mutation-testing/survivor-classification.md).
- [`notes/project-metrics/mutation-census-renames.csv`](project-metrics/mutation-census-renames.csv),
  the crate renames a comparison has to resolve before it can join two censuses.

`script/mutation-census` writes both and reads them back. Its header carries the usage, the worked
examples and its own `BUGS`.

## Why this exists, which is a failure rather than a plan

`script/mutation --report` prints a per-crate table from a finished run, and then the run output is
thrown away. So until this page was written the tree had kept one per-crate record in its
life, `.cargo/mutants-baseline.txt`, dated 2026-08-03, and every comparison anybody wanted to make
had to be made against it whether or not it was the right comparand.

That cost two wrong readings in one week, both on the same paragraph of `design/fatal-risks/README.md`:

1. A crate was reported as having regressed in two days when the column it was read out of was
   six weeks old. Milestone 438 (would a diff-scoped mutation check have caught the 55) measured it against historical trees and the arithmetic
   closed to the unit: 73 survivors already present, 4 added, 77 found.
2. The corpus rate was reported as having fallen between 2026-09-14 and 2026-09-19. It had not;
   see below.

Risk 3 said the gap plainly and this is it closed: *"Which crates caused it is unknown, because the
2026-09-14 census's per-crate numbers were never written into the tree."*

## The record, and its provenance

| census | source | crates | mutants | viable | survivors | killed |
|---|---|---|---|---|---|---|
| 2026-08-03 | `.cargo/mutants-baseline.txt` at 3d0ab7f61 | 38 | 5,551 | 5,141 | 391 | 92.4% |
| 2026-09-14 | [run 34833498873](https://github.com/nifeos/nife/actions/runs/34833498873) | 64 | 10,012 | 9,277 | 771 | 91.7% |
| 2026-09-16 | [run 35163453633](https://github.com/nifeos/nife/actions/runs/35163453633) | 62 | 9,626 | 8,903 | 687 | 92.3% |
| 2026-09-19 | [run 35421192143](https://github.com/nifeos/nife/actions/runs/35421192143) | 62 | 9,656 | 8,925 | 563 | 93.7% |
| 2026-09-21 | [run 35589550926](https://github.com/nifeos/nife/actions/runs/35589550926) | 66 | 10,988 | 10,178 | 771 | 92.4% |

Five censuses, and there have never been more than five. The 2026-09-21 row was captured on
2026-09-24, three days after the run went green, by a lane that happened to need the number. The baseline and the 2026-09-14 rows
reproduce `notes/mutation-testing.md`'s published totals to the unit, which is the check that the
ingest derives what a person derived by hand.

**`killed` counts a timeout as a kill and excludes unviable mutants from the denominator**, which is
[`mutation-testing.md`](mutation-testing.md)'s definition: a timeout is a detected hang, and an
unviable mutant does not compile, which says nothing about the tests. It is stated here rather than
assumed because the one time two censuses were compared by hand the two rows used two different
definitions.

**What is missing is missing because it never existed.** The scheduled runs from 2026-08-10 to
2026-08-31 produced no result at all; `mutation.yml`'s own `BUGS` section has why. The 2026-09-03
run produced one shard of eight, which is a sample and is deliberately not a row here. A sample
cannot tell an absent mutant from a killed one, so a per-crate row taken from one reads as a crate
that got worse.

The backfill worked on luck, and the next one will not. GitHub keeps a workflow artifact for 90
days. Every shard of all three green runs was still there on 2026-09-20; the same backfill attempted
in December would have recovered nothing. Capture the census the day it goes green:

```console
$ script/mutation-census --add-run 35421192143
```

## Renames, and why the table is derived from `lib.rs`

Between 2026-09-14 and 2026-09-19, milestone 265 (`_proto` is a truncation)'s sweep renamed sixteen crates (`ntp_proto` to
`network_time_protocol` and its siblings) and three more followed on 2026-09-18 (`dtb`, `ipc`,
`gpt`). **A join on the bare crate name does not give a smaller answer, it gives a wrong one**. The
renamed crates vanish from the intersection and take their mutants with them. So the surviving rate
is over a different corpus than the one it is being compared to.

`script/mutation-census --renames` derives the table from `git log --diff-filter=R` and commits it.
**It follows `crates/*/src/lib.rs` and not `Cargo.toml`, and that is a scar.** Git pairs renames by
content similarity, and a commit that renames several crates at once hands it a pile of four-line
`Cargo.toml` files nearly identical to one another. Asked about `Cargo.toml` it answers confidently
and wrongly, `crates/uheap -> crates/line_editor` and `crates/canary_gate -> crates/work_steal_slot`,
cross-matched pairs out of one sweep. A crate's `lib.rs` is distinctive, and the same query over it
returns `canary_gate -> memory_corruption_canary_gate` and `steal_request -> work_steal_slot`, which
is what happened. The table is committed rather than recomputed on every run so that a wrong pairing
can be corrected by hand and stays corrected.

## Which crates moved: the comparison is an attribution

```console
$ script/mutation-census --compare 2026-09-14 2026-09-19
62 crates in both, 2 only in 2026-09-14, 0 only in 2026-09-19
killed 92.3% -> 93.7% over the crates in both (+1.42 points)
survivors 687 -> 563, viable 8,893 -> 8,925

crate                                    viable          killed         survivors   points
timetable                           182 ->   146   73.6% -> 100.0%    48 ->    0      0.51
filesystem_contract                 598 ->   598   90.1% ->  93.3%    59 ->   40      0.21
swish                               188 ->   188   89.4% ->  97.9%    20 ->    4      0.18
device_tree_blob                    411 ->   411   96.6% ->  99.8%    14 ->    1      0.15
work_steal_slot                      24 ->    13   54.2% -> 100.0%    11 ->    0      0.11
memory_regions                       72 ->    70   88.9% -> 100.0%     8 ->    0      0.09
elf                                 104 ->   104   94.2% -> 100.0%     6 ->    0      0.07
capability                           68 ->    68   88.2% ->  95.6%     8 ->    3      0.06
machine_discovery                   615 ->   622   88.1% ->  87.6%    73 ->   77     -0.04
```

`points` is the crate's exact contribution to the corpus-level move,
`((k_b - k_a) - rate_a * (v_b - v_a)) / v_b`, and the column sums to the move rather than merely
ranking it. That matters because a crate can shift the corpus rate without its own rate shifting, by
growing or shrinking against the average; `machine_discovery` is that case and the decomposition
says so instead of leaving a reader to infer it. The table also carries a `real holes` column per
crate, read from the record's own column; a census recorded before the column shows `-`, and so do
its totals.

## The first finding: the fall did not happen

`design/fatal-risks/README.md`'s risk 3 stands AMBER on a rate that fell a full point like-for-like between
two censuses five days apart. Recomputed from the artifacts of the same two runs:

| | crates | viable | as risk 3 reports it | recomputed |
|---|---|---|---|---|
| like-for-like, 2026-09-14 | 38 | 6,552 | 93.6% | 93.6% |
| like-for-like, 2026-09-19 | 37 | 6,472 | 92.6% | **94.7%**, over 38 crates and 6,604 viable |
| whole corpus, 2026-09-14 | 64 | 9,277 | 91.7% | 91.7% |
| whole corpus, 2026-09-19 | 62 | 8,925 | 91.4% | **93.7%** |

Two things separate the columns, each worth about a point:

- A timeout is a kill, or it is not, and the two rows disagree. The 2026-08-03 and 2026-09-14
  figures count it as a kill, per the definition above. The 2026-09-19 figures are `caught / viable`,
  scoring the 205 timeouts as survivors. 8,157 of 8,925 is 91.4%, the published number; 8,362 of
  8,925 is 93.7%.
- **`cred` became `credentialer`, and the join dropped it.** Reproducing exactly 37 crates and 6,472
  viable mutants takes resolving `dtb` and `ipc` by hand and missing `credentialer`, which is 103
  viable mutants at 100%.

Read consistently, the direction reverses under either definition. With timeouts as kills, the
whole corpus went 91.7% to 93.7%; with timeouts as survivors, 89.5% to 91.4%. Survivors fell 771 to
563 over the same window.

This does not make risk 3 green, and milestone 518's lane did not edit that file. The entry's
stronger argument is untouched by the arithmetic. The rule of milestone 85 (mutation testing over
the host crates) is that every survivor is triaged into a test, an exclusion with a reason or a
recorded gap. 563 of them are not.

One caveat on the definition this page prefers. The hand-check that justifies scoring a timeout
as a kill was done on the baseline's 96. There are 205 now and nobody has looked at them. That is a
reason to check them, not a reason to change definitions between two rows of one table.

## BUGS

- Nothing gates capture. A census can still be run and thrown away, exactly as four of them
  were; this makes it one command not to, which is rung three of AGENTS.md's ladder and honest about
  being rung three. The mechanism would be a step in `mutation.yml`, which has `contents: read`
  today and would need write permission on the tree, a decision rather than a patch.
- The 2026-08-03 row's counts are derived, not observed. The file it came from said so: the run was resumed twice, `caught` is total-minus-the-rest, and three of 5,551 mutants
  could not be matched to any pass. A comparison against it is a comparison against a
  reconstruction.
- The 2026-08-03 row's crate names are half-swept: it spells `device_tree_blob` for a run that
  recorded `dtb`, and `cred` for `credentialer`. The rename table resolves both.
- A deleted crate and a crate renamed to something git could not follow look identical, and both
  print as "only in A". `multicast_dns_protocol` and `multicast_dns_config` are that case between
  2026-09-14 and 2026-09-19.
- Nothing checks that a row was ingested from the run it names. `--add` believes its caller
  about the date and the run id, the same hole `script/metrics`' `--coverage-for` has and for the
  same reason. `--add-run` does not have it, and is the reason to prefer it.
