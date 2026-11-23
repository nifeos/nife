---
status: BUILT
raised: 2026-10-04
built: 2026-10-04
---
# 740. The survivors a merged pull request adds are checked against a triage record

*(Number provisional until the merge queue lands it.)* This is calef's ruling of 2026-10-04 UTC on fatal
risk 3: "Yes, build the inflow check." The spec is the recommendation in
`nifeos/nife:notes/mutation-testing/inflow-2026-10-03.md`.
Fatal risk 3's green condition is that the survivors each merged pull request adds on its own lines
are triaged, and until this nothing could tell whether that held. Verdicts and colors stay calef's.

Built with no mutation sweep: from the artifacts of runs 37108924347 (2026-10-03) and 35589550926
(2026-09-21) and from the triage ledgers.

## What was built

- `helpers/mutation_inflow.py` (provisional name). `check` diffs a census's `missed.txt` shards
  against the previous committed survivor list, blames each new survivor's line to the commit and
  merged pull request that wrote it, and fails listing any with no triage row. `--selftest` runs a
  fixture (old census, new census, triage CSV) covering triaged, untriaged, unattributed, renamed
  and duplicate-key cases, wired into `script/lint`.
- `notes/project-metrics/mutation-triage.csv` (provisional name): `crate,function,mutation,
  disposition,reason`, one row per triaged survivor, dispositions `killed`, `equivalent`, `gap`. The
  key is how cargo-mutants names a mutant without its line number, so a row survives line drift.
- `notes/project-metrics/mutation-survivors/DATE.csv.gz` (provisional name): the committed
  survivor list per census, key and count only, because the Actions artifacts expire in about 90
  days. Two are committed (2026-09-21, 2026-10-03), 4.7 KB and 6.7 KB gzipped.
- Two jobs in `.github/workflows/mutation.yml`. `inflow` runs the check and goes red on an
  untriaged inflow survivor; it runs on the weekly schedule and never on a merge. The check that
  does sit on a merge is milestone 924 (a survivor on a pull request's own lines cannot merge
  untriaged)'s diff gate. `record` opens a pull request through the App
  token and the `automation` environment (as `metrics.yml` does) adding the census's rows to
  `mutation-census.csv` and its survivor list. It records only a complete census, all eight shards.
- The 2026-10-03 census row in `mutation-census.csv`, which capture had left out.
- A session-start check (`briefs/session-start.md`) and a ledger convention
  (`notes/mutation-testing.md`): a triage lane writes a row per survivor in addition to the prose.

## What the backfill recovered

248 rows, covering 327 of the 1,004 survivors of 2026-10-03, from the ledgers' per-crate sections.
A ledger says "N missed, K killed, E equivalent, the sweep afterwards reports E". Where the
equivalents could be named and the count matched the ledger exactly, every other mutant in the
crate is `killed`. Where it could not (`documentation`, `filesystem_protocol`, `video_terminal`,
`machine_discovery`, `uefi_loader`'s `output_len`, and the ledgers that give only a count), no row
was written. Of the 472 new-survivor keys the check attributes to merged pull requests since
2026-09-21, 162 are triaged and 310 are not. That backlog is what the next triage lane starts from.

## BUGS

- A row covers its whole key. Two mutants with the same operator swap in one function share a
  row, so a later pull request adding a third to a function whose first two are triaged is covered
  by the old row. The check cannot see that; the row's reason is where to say it.
- Two keys are mixed and have no row (`timetable`'s `range` and `next_time`, `<` to `<=`): one
  mutant of each key is equivalent and another is killed, which one row cannot say.
- New survivors on lines older than the previous census are reported, not failed. No pull
  request wrote them. There were 2 in the 2026-09-21 to 2026-10-03 window.
- The first scheduled run proves the jobs only on a real census. The check was exercised
  against the real artifacts locally, the workflow passes `actionlint`, but `record` needs the App
  secrets and has not run.

## Follow-on

- **Recorded.** In `briefs/session-start.md` (step 2): the 310 untriaged inflow keys of the 2026-09-21 to 2026-10-03 window are the next triage lane's input, which writes a row per survivor.

## Index row

The check that makes "the survivors a pull request adds are triaged" something a scheduled job can
fail, with the record it reads and the weekly capture it needed.
