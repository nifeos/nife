# The CI log baseline: per-check attribution mined before the logs expire

*Names: `notes/ci-log-baseline.md`, `script/ci-log-baseline` and the CSV below are provisional.
Naming is an architect's (AGENTS.md); a lane ships a provisional name and says so.*

This tree cannot currently answer "has this gate ever fired in CI?" GitHub Actions keeps a failed
run's metadata forever. But a job conclusion alone says only "the job failed", not which of its
sub-checks did. The one place that answer lives is the job's own log, and GitHub deletes those on a
retention window. **There turn out to be two such windows, one per log endpoint, and they do not
expire in step** (see "Two clocks" below). `design/roadmap/0531-no-gate-records-when-it-fires.md` (a
research lane's proposal, an architect's to rule on, unedited by this record) mined that answer for
the `clippy` job on 2026-09-21. It found the window closing around 2026-10-21. This page and
`nifeos/nife:notes/project-metrics/ci-log-baseline.csv` absorb that
mining into a committed, re-derivable record. They extend it to every other CI job whose log names a
sub-check the same way, before more of it is gone.

This is not the gate-firing ledger proposed alongside it. That ledger (still an architect's to rule
on) would capture every future, local firing. Those never reach Actions, because a lane ran
`script/lint`, saw red, fixed it, and pushed green. This page captures only the past. It captures
only the slice of the past that escaped local gating and reached GitHub. It is not a ranking of
which gates are worth keeping; see the caveat below.

## The record, and its provenance

`nifeos/nife:notes/project-metrics/ci-log-baseline.csv`, one row per
failed CI job GitHub still names. It covers the five job types this script knows wrap more than one
distinguishable check: `run_id,job_id,date,workflow,job_name,attributed_check,branch`.

`script/ci-log-baseline --update` writes it and `--report` reads it back. Its own header carries the
usage, the `EXAMPLES` and its `BUGS`.

Reused rather than refetched, where it was safe to. A prior research lane (the gate-audit proposal
above) had already fetched all 309 `clippy` job logs and all 99 `bench (icount regression tripwire)`
job logs while writing its proposal. Per its own report, it left the mined output in its session
scratchpad rather than the repo. This lane found it there, and re-verified it rather than adopting
it on trust. Five `clippy` rows spread across the full date range were re-fetched live and matched
the cached marker exactly. They were the oldest failure, one from 2026-08-17, one from 2026-08-25,
one from 2026-09-16, and the newest distinct-check firing. Two `bench` rows (one `TRIPWIRE`, one
generic failure) were re-fetched and matched too, down to the exact benchmark name and the exact
`##[error]` line. This lane fetched everything else in this record fresh: `supply chain`, `cpu
matrix`, `verify (Kani proofs)`, and the run-to-branch mapping.

**Which job types, and why only these five.** `script/lint` runs 47 named checks inside one `clippy`
job and exits on the first failure. So the log's last `==>` marker names the check that was running.
Three more job types share that convention closely enough to use the same technique:

- **`supply chain (advisories, licences, vendored integrity)`** runs `script/supply-chain`: one
  `cargo-deny` pass per manifest (six of them), then `script/vendor-verify`'s own per-package pass.
  Each is announced with its own `==>` line, and `set -e` stops the script at the first one that
  fails. The last marker names it exactly. 12 of 14 failures are the main workspace's own manifest,
  and one is `script/vendor-verify` failing on the `redoxfs` pin specifically. One has no marker at
  all because `taiki-e/install-action` failed to download its own tool before `script/supply-chain`
  ever ran (recorded as unattributable, not silently dropped).
- **`verify (Kani proofs)`** runs `script/verify`'s per-crate Kani loop the same way. There is one
  `==> kani: <crate> (<what it proves>)` line per harness and `set -e`, and the last marker names
  the harness. Only five of these have ever failed.
- **`cpu matrix (riscv64 across QEMU CPU models)`** runs `script/cpu-matrix`, and its last `==>`
  marker is not useful on its own. The script prints `==> matrix` as a section header after every
  model has run, whether any failed or not. So that marker is identical on all 213 failures and
  would attribute nothing. What does distinguish them is the line the script prints to stderr,
  `cpu-matrix: failed on: <models>`. The rarer case is the probe that confirms `-cpu` is actually
  narrowing the machine failing before the model loop starts. There it is the `==> preflight: ...`
  marker that precedes it (nothing follows a preflight exit, so the ordinary "last marker" rule
  works there). 209 of 213 name one or more specific CPU models (`rva22s64` 157 times, `sifive-u54`
  155, `thead-c906` 154, `rva23s64` 150, `rv64` 148; these overlap, since a single failed job can
  name several models). Zero are preflight failures, and four failed during QEMU's own build/install
  step, before `script/cpu-matrix` printed anything of its own.
- **`bench (icount regression tripwire)`** runs `script/icount` via `xtask bench`, which has no
  `==>` convention at all. It prints `bench: CHECK FAIL <name>: <cur> vs baseline <base> (...)` per
  regressed benchmark when the tripwire itself fires. This record greps for that directly, the
  technique the gate-audit proposal already used by hand for this one job type. 43 of 99 carry that
  line (`spawn_el` 21, `map_new` 16, `yield_switch` and `ctx_switch` 4 each, plus a handful of other
  benchmarks once or twice each). The other 56 are recorded as `unattributable: not the tripwire`.
  Their log carries nothing more specific than GitHub's own generic `##[error]Process completed with
  exit code 1`-style line, which this record captures too (truncated) rather than reducing to a bare
  "unattributable".

Every other failed job type is a single check, and GitHub's job name already is the finest
attribution available. Ruled out for that reason: `build + test (host + QEMU)` (327 failures),
`coverage (host crates)` (104), `rustfmt` (55), `fastpath footprint (the IPC path must stay
L1i-sized)` (33). Also `stack frames (no frame over a third of a thread stack)` (14),
`undefined-behavior check (host crates, sampled paths)` (5), `build against the latest nightly`
(19). Also `architect hold (needs-architect label)` (67), which is a status check reflecting a label
rather than a script running a test. So "which check fired" does not apply to it at all. Also
`mutants (shard N/M)` (varying shard counts across two mutation-testing eras), `re-falsify the
harnesses this change can reach` (4), `report against the baseline` (3), `propose a pin bump` (3),
`is an audit due` (5), and CodeQL's `Analyze (javascript-typescript)` (2). One of these, `coverage
(host crates)`, DOES carry finer detail in its log. `script/coverage`'s floor check names the
offending file on a `coverage: <file>: N% ... under the floor` line. But that line carries no `==>`,
and the marker that follows it, `coverage floor FAILED`, is identical on all 104 failures. Capturing
the file-level detail would need a different extraction technique than this record uses everywhere
else. It is left as a `BUGS` entry rather than folded in here.

## The honest accounting, 2026-09-21

| | |
|---|---:|
| Failed CI runs on record (metadata; never expires) | 909 |
| Failed CI jobs on record, all types (metadata; never expires) | ~1,310 |
| Failed jobs among the five attributable job types | 640 |
| ...attributed to a specific sub-check | 583 |
| ...recorded as unattributable, with a reason | 57 |
| ...of which "log expired" | 0 |
| ...of which "log unavailable" for some other reason | 1 (a tool-install failure before the job's own script ran) |
| ...of which "ran, but not the check this job type is watched for" | 56 (bench, non-tripwire failures) |
| Oldest date reached | 2026-07-23 (this repository's very first CI run; there is nothing older to reach) |
| Newest date reached | 2026-09-21 |
| Roughly how many GitHub API calls this lane made | ~275: 1 total-run-count probe, ~10 paginated pages to re-list all 909 failed runs with their branch, ~10 probe fetches to check log survival across the date range by hand, 232 fresh log fetches (14 supply chain + 213 cpu matrix + 5 verify), 7 re-verification fetches (5 clippy, 2 bench), ~10 fetches re-probing the three job ids calef flagged as expired (three attempts each, one with response headers, plus the run-level archive for the same run), plus the calls the discovery process above made while locating and reading the reused cache. The 309 clippy and 99 bench log fetches were **not** repeated; they were reused from a prior lane's cache and spot-checked live instead. |

Zero rows in this record are unattributable because of expiry, via the run-level archive. That is
the endpoint `script/ci-log-baseline` now prefers to fall back to. It has not begun expiring as far
as either measurement below can tell. That qualification is new and load-bearing. Read the next
section before trusting the unqualified version of this sentence anywhere else.

### Two clocks, and they disagreed with each other

The brief that opened this work stated the oldest failed run's log already read as expired. This
lane's first pass checked that directly against `/actions/jobs/{id}/logs` (the per-job endpoint, the
only one this script used at the time). It could not reproduce it. Job 89101327085 returned its full
log. That is the run at 2026-07-23T02:13:08Z, the first CI run this repository ever recorded,
confirmed by paging to the end of the unfiltered run list. So did every job sampled from 2026-07-24
through 2026-08-01, and the specific `clippy` job the earlier proposal named as its oldest. That
reading was written up as a correction to the brief and to the gate-audit proposal's own same-day
measurement, which projects deletion starting around 2026-10-21. Nothing had expired yet, full stop.

calef then measured the same three job ids directly (89101327085, 89101327099, 89101327127, all from
run 29973830663). He got `expired` at the job endpoint for all three, while the *run-level* archive
(`/actions/runs/29973830663/logs`) for the same run still served its logs. Told this, this lane
re-probed the same three ids immediately afterward, three times each with response headers. It got
`200 OK` every time, with `Last-Modified` matching the run's own creation time (`Thu, 23 Jul 2026
02:13:47 GMT`) and a stable `ETag` across repeated requests. Both readings are real; neither side
mis-typed an id or misread a response. The honest conclusion is that GitHub's per-job log endpoint
is, at minimum, inconsistent near wherever its retention boundary actually sits. Two probes of the
identical id, close together in time, returned `200` and `410` respectively. Plausible mechanisms
include a deletion sweep that has started but not finished, or a storage tier/CDN replica that has
not caught up with one that has. Nothing here can distinguish those or re-derive which one it is.

What is not in dispute, and is the more important fact operationally: the run-level archive
(`/actions/runs/{id}/logs`) returned every job's log intact. That held for every run and every job
this lane tried, including the three the job endpoint failed on for calef. `script/ci-log-baseline`
was rewritten to use this as a fallback rather than a primary. It is one larger fetch (a zip of
every job in the run, `2_build + test (host + QEMU).txt`-style names) rather than one small one. So
`fetch_log` now tries the per-job endpoint first and, on ANY failure there, pulls the matching entry
out of the run zip. So: **the accounting above ("of which 'log expired': 0") is true of what this
script can now reach, via whichever endpoint answers.** It is not a claim that the job endpoint
alone has never returned expired. It plainly has, for calef, for the very ids this lane also
checked.

This changes the date to watch. The gate-audit proposal's 2026-10-21 estimate was for *a* retention
window. That is not necessarily the one that matters, now that the job endpoint has apparently
already started misbehaving for the oldest logs. The date worth watching is whether the run-level
archive starts failing. That is the endpoint this script now depends on, and the one with no
evidence of expiry yet. If it holds past 2026-10-21, the proposal's estimate was about the wrong
clock. If it starts failing before or at that date, the two clocks were closer together than either
measurement suggested. Either way, do not read "the window closes 2026-10-21" as settled for the
endpoint this record actually uses.

## What this preserves from the gate-audit proposal, and what this record's own data says about it

This record's own 309 `clippy` rows reproduce the proposal's clippy table almost exactly. They were
re-derived independently by grouping this CSV's own `attributed_check` column rather than copied
from the proposal's prose. The counts are 78 roadmap status, 50 markdown links and the notes index,
37 naming conventions, 27 em-dashes, 27 decisions, and 21 the host+xtask clippy pass. Then 18
counted claims, 14 citations, 8 new citations say what they cite, 4 spelling, 2 each for
TODO-cites-a-milestone/script docs/every fence names its counterpart, 1 each for rustdoc/the
separate-workspace clippy pass/the riscv64 shell-feature pass. So the proposal's derived finding is
preserved and cross-checked here rather than merely repeated: **32 of `script/lint`'s 47 named
checks have never failed a CI job**. They include several with the best-documented real defect
catches in the tree (`conflict markers`, `sh -n`, `unsafe fn contracts`).

One number is corrected rather than repeated. The proposal states 13 firings for "the aarch64
clippy pass, under its three successive names". Grouping this record's own three aarch64 marker
texts gives 16: `kernel + user + user_rt (aarch64)` 10 and `kernel + user + user_mode_runtime
(aarch64)` 3. The oldest form, from before the runtime crate existed as its own name, is `kernel +
user (aarch64)` 3. 10 + 3 + 3 = 16, not 13, and the arithmetic on the row above it agrees. This
record's 19 distinct markers sum to exactly 309 only under 16, not 13. Recorded here rather than
silently fixed in place. This tree's convention is to correct the record on purpose when the machine
and the document disagree.

**Restated in this record's own voice, because it is worth restating rather than only citing: this
is not a ranking of which gates are worth keeping.** A check that fires locally, gets fixed, and is
pushed green never reaches GitHub Actions at all. So this record and its CSV systematically
undercount exactly the gates that work earliest and cheapest. That is a different thing from the
gates that matter least. A count of zero in this record means "never failed a CI job", never "never
caught anything."

## BUGS

- **This record cannot tell a retry from a new event.** A CPU model or a benchmark that fails on
  three consecutive re-runs of the same push produces three rows here. They are indistinguishable
  from three separate regressions on three separate days. The gate-audit proposal noted the same
  limit for `bench`'s clustering (13 failures on one day, 10 the next, read as "one condition
  re-failing" rather than ten events). It applies to every job type in this record.
- **`coverage (host crates)`'s per-file detail is not captured**, see above. A future extension
  would need to grep the `coverage: <file>:` lines specifically, not the `==>` convention this
  record otherwise relies on.
- The four `cpu matrix` failures attributed to a QEMU build/install step are a real category this
  record does not further distinguish. A failure there means the runner's QEMU cache was cold or
  stale, not that any CPU model actually ran and failed. `script/ci-log-baseline` records the
  install-log marker verbatim rather than inventing a category for it.
- `architect hold` and `is an audit due` are gates over process state, not over code, and were ruled
  out of this record for that reason. A "which check fired" question does not apply to a label or a
  cadence date the way it does to a script with named sub-checks.
- Which endpoint `script/ci-log-baseline` uses, stated plainly because a re-runner in November
  needs it more than anything else here. It tries `/actions/jobs/{id}/logs` first (smaller fetch).
  On any failure there it falls back to pulling the matching entry out of
  `/actions/runs/{id}/logs`'s zip. The entry is matched by the job's own name against the archive's
  `<index>_<job name>.txt` filenames (safe only because no run in this record has ever carried two
  failed jobs of the same attributable-type name; checked before relying on it, not assumed). A
  future run of this script that reports `log expired at both the job endpoint and the run archive`
  means the run archive itself has started expiring. That is the deadline actually worth watching
  (see "Two clocks" above). A job-endpoint-only failure it now recovers from silently via the
  fallback, and never surfaces as expiry at all.
