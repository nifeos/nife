---
status: BUILT
raised: 2026-09-24
built: 2026-10-07
promoted_from: triage-the-crates-the-2026-09-21-census-measured-first
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 637. Triage the crates the 2026-09-21 mutation census measured for the first time

Promoted 2026-10-03 (UTC); the number is provisional until the integrator confirms it at merge, and the lane is `milestone/637-survivor-triage`. *(Title and slug are drafts.)* Fatal risk 3 (the tests do not test anything) is what this serves.

Newest census: mutation run 37108924347 (2026-10-03), 85 crates, 14,853 mutants, 1,004 missed, 92.7% killed. Its missed list is the work list; the 2026-09-21 table below is the order.

Seventh and last batch (`milestone/637-survivor-triage-7`) done 2026-10-07 (UTC). Final tally of the 1,004 (killed by a new test, recorded equivalent, recorded gap, remaining): 621, 359, 24, 0. The last 201 sat in 39 crates; 114 already had rows from milestone 326's inflow lanes, and the other 87 are in part five (`nifeos/nife:notes/mutation-testing/census-2026-10-03-triage-part-5.md`). The accounting is in `nifeos/nife:notes/mutation-testing/census-2026-10-03-triage.md`.

Raised by the lane that condensed `notes/mutation-testing.md`
(#1209), while recording the 2026-09-21 census, whose run no one had captured.

The census is recorded in `notes/project-metrics/mutation-census.csv`, and the work
is milestone 326 (nobody has been assigned to turn a mutation score upward)'s method applied to new
crates.

## What is owed

Four crates appear for the first time in the 2026-09-21 census (run 35589550926). Two of them carry
most of its new survivors:

| crate | viable | killed | missed | timeout |
|---|---|---|---|---|
| `stick_maker` | 288 | 64.9% | 101 | 2 |
| `portable_executable` | 277 | 72.6% | 76 | 0 |
| `firmware_configuration` | 32 | 84.4% | 5 | 0 |
| `sealed_pair` | 63 | 95.2% | 3 | 0 |

Every survivor becomes a test, an exclusion carrying its reason, or a recorded gap, per the triage
rule in `notes/mutation-testing.md`. The accounting goes in a new appendix under
`notes/mutation-testing/`, in the shape the 2026-09-20 appendices use.

## Update 2026-09-24: two of the four are done, and the rest of the census joins them

Milestone 326's classification lane (`milestone/326-survivor-classification`) took `stick_maker`
and `portable_executable` and sorted all 771 of the census's missed survivors by kind
(`notes/mutation-testing/census-2026-09-21-triage.md`). `stick_maker`'s answer to the question below
was yes: 61 of its 101 were `src/host/` and `main.rs`, `#[cfg]`-selected per host OS, now excluded.
`portable_executable` went 76 to 5, all equivalent.

**What that classification left untriaged is this proposal's natural scope**, so it is widened here
rather than filed twice. 89 survivors, plus 14 that arrived after the census:

| crate | missed at 2026-09-21 | note |
|---|---|---|
| `documentation` | 47 | a lane of its own; see its appendix's feature-gate history |
| `component_plan` | 11 | new since the baseline, never triaged |
| `ps`, `pgrep`, `pmap` | 6, 6, 5 | the same |
| `firmware_configuration`, `sealed_pair` | 5, 3 | this proposal's original two |
| `loaded_image_check`, `uptime` | 2, 1 | new since the baseline |
| `globally_unique_identifier_partition_table`, `entropy_protocol`, `socket_protocol` | 1 each | above the August triaged count |
| `uefi_loader/src/device_tree_from_acpi.rs` | 14 (current tree) | merged 2026-09-23, after the census: inflow |

## What to check first

Re-derive each crate with `script/mutation -p <crate>`; do not trust the census rows. Before
triaging, rule out the instrument. Three crates have read low because of it: `uefi_loader`'s unbuilt
`[[bin]]` modules (#1232), `documentation`'s feature-gated tests, and `timetable`'s proof file. Ask
whether any of `stick_maker`'s or `portable_executable`'s files are compiled only behind a feature or
a target, with `cargo mutants --list -p <crate>` and the crate's `Cargo.toml`.

## Done when

Every crate in the update's table is triaged, with its accounting in an appendix the main page's
table links.

## Follow-on

- **Done.** The 201 left after batch 6, on 2026-10-07 (UTC): 10 killed by new tests, 70 equivalent and 7 gaps among the 87 with no row, and the 114 rows the inflow lanes wrote re-checked against a per-crate re-run.
- **Recorded.** The census host is Linux and lanes re-run on macOS, so a `cfg!(target_os)` branch can read caught locally and missed weekly (`board_console::dial_in_warning`, three mutants). Part five says so where the next lane reads.
- **Recorded.** 35 survivors in code merged after the census (`board_console/src/exposure.rs`, `jh7110_clock_and_reset`, `screen_console`) have no triage row; part five lists them, and the inflow check (#1582) and the next census are their route.
- **Milestone 771.** Milestone 771 (the inline scanner reads from a slice that ends with its range). `design/roadmap/0771-the-inline-scanner-reads-from-a-slice-that-ends-with-its-range.md` makes the eight `documentation` over-read equivalents unrepresentable.
- **Recorded.** The `output_len` and `CMDLINE_LEN` bounds are generous by design, so 12 survivors in `uefi_loader` stay as gaps, in `notes/mutation-testing/census-2026-10-03-triage.md`.

## Index row

Every survivor in the weekly mutation census becomes a test that was seen to fail under the mutation, or a recorded equivalence with its reason, so the 92.7% killed figure stops being a number nobody has read behind. It serves fatal risk 3.
