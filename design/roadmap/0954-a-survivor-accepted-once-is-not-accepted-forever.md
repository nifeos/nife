---
status: IN-PROGRESS
branch: milestone/954-a-survivor-accepted-once-is-not-accepted-forever
raised: 2026-10-27
promoted_from: a-survivor-accepted-once-is-not-accepted-forever
milestone_dependencies: 740, 924
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 954. A survivor accepted once is not accepted forever, and lanes work the accepted pile down

*(Title and slug are drafts.)*

Raised 2026-10-27 (UTC) by `lane/mutation-gaps-plan`. calef ruled that day that three gaps in
mutation testing close, and this is the first: accepted is not killed. The weekly census, the inflow
check and the per-pull-request gate stop new untriaged survivors. Nothing works the existing ones
down, and nothing asks again whether an old verdict still holds.

This extends milestone 326 (nobody has been assigned to turn a mutation score upward), which is
BUILT. 326 gave every survivor of the 2026-10-19 census a row and made that census the baseline. Its
own `BUGS` section says a sweep needs repeating and leaves open whether that becomes a gate. This
proposal answers that question. Milestone 250 (an unviable mutant is a hole in the
measurement) stays separate: an unviable mutant never ran, and every survivor here did.

## What the census holds today

The 2026-10-26 census (run 40311212712) has 712 missed mutants. Matched against
`notes/project-metrics/mutation-triage.csv` through `helpers/mutation_inflow.py`'s key and rename
table, they split like this:

| a census survivor's row | mutants |
|---|---|
| `equivalent` | 612 |
| `gap` | 6 |
| `killed`, which the census has just contradicted | 3 |
| no row at all | 91 |

The file holds 1,525 rows: 1,074 `killed`, 406 `equivalent` and 45 `gap`. Most rows record closed
work. The live accepted pile is the 618 that an `equivalent` or `gap` row answers.

The 91 with no row sit in 50 keys. Most are drivers: `designware_ethernet` 26, `e1000e` 26,
`designware_mobile_storage` 22, `non_volatile_memory_express` 7, `driver_process_contract` 5. Of
the 50 keys, 36 already survived on 2026-10-19. The inflow check passes them because it fails only
on a survivor it blames to a pull request merged since the last census. A survivor on an older
line with no row fails nothing. The three stale `killed` rows are `grant_plan`'s `Windows::take`,
`serial_console`'s `dial_in_warning` and one in `system_log_contract`.

The 612 equivalents cluster by operator:

| mutation | mutants |
|---|---|
| a bitwise swap, almost all OR becoming XOR over flags that share no bit | 378 |
| a comparison, such as less-than becoming less-or-equal | 89 |
| a shift, mostly one shifted left by zero becoming one shifted right by zero | 67 |
| arithmetic | 41 |
| a return value, a deletion or other | 37 |

The top crates by missed plus timeout, from the same census: `designware_ethernet` 78, `e1000e`
76, `designware_mobile_storage` 75, `paging` 73, `extensible_host_controller_interface` 50,
`grant_plan` 44, `video_terminal` 42 and `filesystem_contract` 33. Timeouts belong to the
companion proposal, `a-mutant-that-hangs-is-its-own-verdict.md`.

## The four holes

1. A row never expires. The gate clears a survivor on a changed line with an `equivalent` row. That
   row may describe the function as it stood before the pull request rewrote it.
2. Nothing checks the whole census. The inflow check reads only blamed survivors, so 91 mutants
   sit with no row and nothing fails.
3. `equivalent` holds three different claims. Some are algebra and hold in every caller. Some hold
   only for today's callers, like a bound no caller reaches. Some hold only under this harness,
   like a release-only guard behind a `debug_assert!`, or a register no host test can observe.
   The third kind is a gap with a better name. Another instrument could kill it.
4. A `gap` row names no owner. AGENTS.md's rule for identified work is a milestone or a `BUGS`
   entry, and a `gap` row is neither.

## Most of the pile is noise, so measure it first and remove it at the source

Two shapes make up 445 of the 612 equivalents: a bitwise swap over operands that share no bit,
and a shift of or by zero. The same two shapes cover 71 of the 91 survivors with no row, so 516 of
the 712 missed, 72%, look like noise. No test can ever kill either shape. `x << 0` equals
`x >> 0` and `0 << n` equals `0 >> n` for every value. A `|` over disjoint bits equals a `^` over
them. The `paging` lane of 2026-10-27 (#2804) confirmed it crate by crate: all 73 `paging`
survivors are equivalent. 64 are disjoint-bits swaps and 9 are shifts of `1 << 0` and
`0b00 << 6`. Every one already had a row, and 21 of those rows carry a generic reason copied from
another crate. The shape count here comes from the mutation text alone. Shape is evidence, not
proof, and part 1 checks each one against its code.

A census that counts this noise as missed hides the real holes inside it. Three ways to remove it:

- A row per mutant, as today. It keeps every mutant visible and costs a reader per row, forever, and
  the copied reasons show what that reading becomes at volume.
- Rewrite the code so the mutant cannot arise: a typed flags builder, or one shared helper that
  composes every field. It moves the noise into one function, but the helper's own `|` stays
  equivalent under its contract, and code shaped for the tool is not better code.
- A scoped exclusion in `.cargo/mutants.toml`, one `exclude_re` per function and shape, with its
  reason, as milestone 85 (mutation testing over the host crates) allows. For a disjoint-bits swap,
  the reason names a `const` assertion beside the code that the operands share no bit, so the
  compiler checks the argument. For a shift, the reason is the identity. `script/lint` fails an
  entry whose function no longer exists or whose named assertion is gone.

The recommendation is the scoped exclusion, and it holds at equal cost: the product code stays as
written, and the proof of equivalence compiles. Its limit belongs in the exclusion's `BUGS`. An
excluded function that later gains a `|` over overlapping bits generates no mutant for it. The
`const` assertion narrows that risk without closing it, so an exclusion names one function, never
a crate or a shape tree-wide.

## What done means

The headline is real holes: census survivors that no algebraic argument answers. After this work
that means `caller` rows, `gap` rows and anything unrowed. Today the number is unknown and at most
196, the 712 less the two noise shapes. Measured on the first census after the last part lands:

- No census survivor has a `killed` row or no row at all. Today there are 94.
- The census holds no disjoint-bits or zero-shift survivor. Each one either leaves through a
  scoped exclusion with a compiled argument, or turns out not to be equivalent and gets a test.
- Every remaining `equivalent` row carries a kind, `algebraic` or `caller`. A harness-only claim
  becomes a `gap` row.
- Every `gap` row names its home, a milestone number or a `BUGS` location.
- In each of the eight crates above, every survivor is `algebraic`, `caller` with the caller named,
  or a `gap` with a home. Its triage note and the census report record
  its real-hole count.

That is not a target score, for milestone 326's reason: exclusions can buy a percentage. Every
exclusion here carries an argument a tool checks, and the weekly job fails on a survivor with no
reason.

## How the work splits

Each part is one lane. Each crate lane re-derives its survivors with `script/mutation -p <crate>` on
its own worktree. The largest crate here, `grant_plan`, has 1,312 mutants, about 50 minutes at the
census's measured 2.35 seconds per mutant. A crate lane changes one crate per pull request, so the
gate runs only that diff's mutants. The gate of milestone 924 (a survivor on a pull request's own
lines cannot merge untriaged) ran 217 of them in three minutes on #2797, well under its 20-minute
cap.

1. The measurement. Classify every live survivor by shape and by the reason its row gives, flag
   copied reasons, and publish the real-hole count per crate as a census report column. No code
   changes, so this lane runs first and decides how large parts 4 and 5 are.
2. The mechanism. The weekly job gains a census-wide check. Every survivor needs an
   `equivalent` or `gap` row, and a `killed` row for a live survivor fails, the gate's own rule.
   The triage file gains `kind` and `verified` columns, and existing rows backfill as
   `unclassified`. `script/lint` holds the `unclassified` count to a baseline that only falls, the
   prose ratchet's shape, and fails a `gap` row with no home. It also checks each scoped exclusion
   as above. The gate stops accepting a row its own pull request did not add or re-verify for a
   survivor on a line that pull request changed. Before switching that rule on, the lane replays
   it over the last 100 merged pull requests and reports how many it would have stopped.
3. The 94 with no row or a stale one, one lane per driver crate: `designware_ethernet`, `e1000e`,
   `designware_mobile_storage`, then one lane for the remaining five crates together. The first
   question in each is whether rows exist under a key the helper does not match.
4. The noise, one pull request per crate, largest first. Each adds the `const` assertions and the
   scoped exclusions and deletes the rows they replace. `paging` is the first, from #2804's
   reading.
5. The eight crates' remaining survivors, one lane each, in the order above. Each classifies its
   `equivalent` rows, writes the test that kills any a test can, and moves harness-only claims to
   `gap` with a home. The Kani results of milestone 741 (does a standing proof notice a regression)
   come first, since its table names 17 mutants a
   proof kills and every test misses. A row for one of those is wrong.
6. The remaining crates, in batches ordered by real holes. One lane takes as many crates as fit
   one day of per-crate runs, so a census does not wait on a lane that never ends.

## What holds the gain

- The census-wide check fails the weekly job on any survivor without a live row.
- The gate's re-verify rule makes an accepted verdict expire when its code changes, which is the
  moment it can become false.
- The `unclassified` ratchet, the home rule and the exclusion check fail `script/lint`.
- The census report prints real holes per crate beside missed. `script/mutation-census --compare`
  shows the trend. The checks above hold the verdicts, and the count is the lagging reading.

## Cost

Six parts and roughly fourteen lanes, most of them a crate's afternoon. The weekly census gets
slightly cheaper, since excluded mutants stop running, and the new check reads the census's own
output. The re-verify rule costs authors work. A pull request that rewrites a function carrying
accepted survivors re-reads those rows. Part 2 measures that before it switches the rule on.

Reuse: `helpers/mutation_inflow.py`'s key, rename chain and ledger reader, which the gate and
the weekly job already share; `.cargo/mutants.toml`'s `exclude_re`, which cargo-mutants already
reads; and the prose ratchet's baseline shape for the `unclassified` count. Nothing new is written
where one of these fits.

## Names

The `kind` column, its values `algebraic`, `caller` and `unclassified`, and the `verified` column.
Name: provisional, minted by this proposal on 2026-10-27 (UTC). An architect names things.

## Index row

The 2026-10-26 census has 712 missed: 612 answered by an `equivalent` row, 6 by a `gap`, 3 by a
false `killed` row and 91 by nothing, and 516 of them are two shapes no test can kill. Proposed:
measure the real holes, remove the noise through scoped exclusions whose argument compiles, add a
census-wide check and rows that expire when their code changes, and work the rest down per crate.
