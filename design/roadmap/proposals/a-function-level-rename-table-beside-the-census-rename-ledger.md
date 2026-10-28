---
status: PROPOSED
raised: 2026-10-28
milestone_dependencies: 740
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A function-level rename table beside the census rename ledger

Raised 2026-10-28 (UTC) by `lane/954-rename-table-proposal`, cut to write down what the first two
part-3 lanes of milestone 954 (a survivor accepted once is not accepted forever) found. Each lane
asked its driver crate part 3's question, whether the census's unrowed survivors lacked rows. Each
found that the rows already existed. `designware_ethernet` (#2847): all 26 unrowed survivors
resolved to 11 keys. Ten name functions the crate renamed after the 2026-10-26 census ran. The
eleventh, `Handoff::pack`, moved whole into `driver_process_contract`, whose ledger holds its row
under that crate's composite key. `designware_mobile_storage` (#2849): all 12 rowless keys, 22
survivors, the same cause. Of the crate's 12 measured real holes, 9 are these artifacts and 3 are
honest `caller` rows. The two lanes resolved 23 of the carried list's 50 keys, and no row was
missing from any of them; the matcher could not see the rows under the names the renames left
behind. This is identified work 954's lanes uncovered, not a part of 954.

## What the matcher resolves today

A survivor's key is `(crate, function, mutation)`, and the key resolves through
`notes/project-metrics/mutation-census-renames.csv`, the committed ledger of crate renames that
`helpers/mutation_inflow.py`'s `resolve()` walks as a chain on both sides of every comparison.
Every instrument resolves through that one function, so a second table there reaches them all.

- The weekly inflow check of milestone 740 (the survivors a merged pull request adds are checked
  against a triage record).
- The census-wide check that milestone 954 (a survivor accepted once is not accepted forever)
  shipped in its part 2.
- The per-pull-request gate of milestone 924 (a survivor on a pull request's own lines cannot
  merge untriaged) and its replay.
- The classification page (`helpers/mutation_real_holes.py`), the carried list
  (`helpers/mutation_survivor_rows.py`) and the shape-sites file (`helpers/mutation_shape_sites.py`).

`script/mutation-census` writes the ledger itself, deriving it from `git log --diff-filter=R` over
`crates/*/src/lib.rs`, because git pairs whole files well and four-line `Cargo.toml`s badly.

The ledger covers crate renames only. A function renamed inside a crate strands its rows the same
way, and the derive cannot grow a function-level table: git does not see a function renamed inside
a file, so there is nothing to derive. The crate ledger exists because its own class of gap already
bit once: the 2026-10-18 `_contract` rename stranded 75 survivors, and the `orphans` check now
fails a triage row whose crate names no package. Function renames have no ledger and no check.

## What an unrecorded rename costs

The census reports survivors under the new name, the triage rows sit under the old one, and the
matcher files the keys as unrowed. The carried list carries them, the classification page counts
them as unrowed candidates and inflates the crate's real-hole count, and the census-wide check
fails the weekly job on every one. The remedy is archaeology. A lane reads pull requests until it
can say which rename happened, then reads every row against its code to confirm what the rename
already explained. Both part-3 lanes paid that, for 23 keys that one table row each would have
answered. The cost repeats at every rename sweep, because nothing records the mapping anywhere the
matcher reads.

## The mechanism

The function-level table sits beside the crate-level one:
`notes/project-metrics/mutation-census-function-renames.csv`, the same four columns
(`old,new,commit,date`), with `old` and `new` as full keys, `crate::function`. Full keys keep two
crates' same-named functions apart, scope a rename to its crate, and express the move the evidence
already holds, `designware_ethernet::Handoff::pack` into
`driver_process_contract::designware_ethernet::Handoff::pack`, which a function-only column cannot.
The rows are append-only and hand-written. `resolve()` walks them as a second chain: the crate
component resolves through the crate ledger first, then the full key resolves through this one.
Both sides of every comparison resolve the same way, so chained renames compose.

The lane doing the rename writes the row, at rename time, in the pull request that renames the
function. Nothing can derive the row, so the pointer sits where the renamer already looks:
[notes/skills/naming-authority/SKILL.md](../../../notes/skills/naming-authority/SKILL.md) is what a
lane reads before any rename, and this work adds the rule there. Two checks hold it up. The
`orphans` command that `script/lint` already runs fails a triage row whose crate names no package.
It gains the function component: a row whose function the crate no longer defines fails, unless a
rename row maps its key forward. The census-wide check gains the same walk, so when it fails an
unrowed key it says whether the key's function has vanished from the tree, the rename signature,
and names the likely rows. What fails when a rename goes unrecorded: the renaming pull request
strands its own rows, the lint lists them, and the weekly job's failure carries the rename
signature. Whoever renames writes the record; the checks make forgetting expensive instead of
invisible, which is rungs two and three of AGENTS.md's ladder, not rung one.

## What done means

- The 23 keys both lanes reconstructed by hand replay in the helper's selftest: the 11
  `designware_ethernet` keys and the 12 `designware_mobile_storage` keys of the 2026-10-26 census
  resolve to their rows, chains included.
- The carried-list generator resolves through the table, and the list regenerates with neither
  crate on it.
- The classification generator resolves through the table, and `designware_mobile_storage`'s line
  against the 2026-10-26 census reads 3 real holes, its three `caller` rows, where it reads 12
  today.
- `script/lint` fails a triage row whose function its crate no longer defines and no rename row
  maps forward, with selftest cases for the rename, the deletion and the move-between-crates
  shapes.
- The naming-authority skill names the table and the write, where a lane reads it before a rename.

## Cost

One small lane. The backfill is nearly free: both lanes already did the archaeology, and #2847 and
#2849 name both sides of every row, so the 23 rows transcribe from their records. The matcher
change is one more chain in one function whose selftest already holds the rename shape. The lint
extension reuses the `orphans` walk. The standing cost is one row per recorded rename, written by a
lane at a moment it is already editing names, plus the tree walk the lint already pays for the
crate component. Whether `e1000e`'s 26 unrowed survivors are this class is that crate's own part-3
question; the table gives its lane a place to write the answer once, where every instrument reads
it.

Reuse: `notes/project-metrics/mutation-census-renames.csv`, whose columns and chain walk this table
copies; `helpers/mutation_inflow.py`'s `resolve()` and key, which every reader already shares; and
the census record, `notes/mutation-census.md`, that anchors both sides of every comparison. The work
writes nothing new beside them but the function-level rows.

## Names

The file `notes/project-metrics/mutation-census-function-renames.csv`. Name: provisional, minted by
this proposal on 2026-10-28 (UTC). An architect names things.

## Index row

Both part-3 lanes of milestone 954 (a survivor accepted once is not accepted forever) found their
crates' missing rows under names a rename sweep left behind. 26 survivors sat in 11 keys in one
crate and 22 in 12 keys in the other; 9 of one crate's 12 measured real holes were these artifacts.
Proposed: a function-level rename table beside the crate-level ledger. The lane renaming writes a
row at rename time, every instrument reads it through the resolver it already shares, and a lint
fails a row whose function has gone missing without a rename row.
