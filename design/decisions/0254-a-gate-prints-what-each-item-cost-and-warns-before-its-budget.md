---
status: DECIDED
raised: 2026-10-06
decided: 2026-10-06
ratified_by: calef
---

# 254. A gate prints what each item cost, and a job near its budget warns rather than fails

calef ruled four forks on PR #1778 on 2026-10-06 (UTC), one at a time, each recorded verbatim in a
maintainer comment on that pull request the same day. They settle the two proposals promoted there:
milestone 807 (the kernel suite reports what each test cost) and milestone 808 (every gate accounts
for its time), which depends on 807. *(Section and milestone numbers provisional until the merge
queue lands them.)* Recorded by the maintainer session the same day.

Status, 2026-10-07 (UTC): milestone 807 is built on PR #1819, PARTIAL until merge-group runs
accumulate; milestone 808 is partly built on PR #1825. Neither is merged yet.

Every name here is provisional: the shared crate that holds the record's grammar, the per-job
breakdown file, the tracking issue's label and the daily record's CSV. Naming them is a separate
ratification under design/naming.md.

## The rulings

| Fork | calef's words | What it decides |
|---|---|---|
| 1. The kernel record's format | "Yes on Fork 1, the printed block" | After the suite, the kernel prints one `time <ms> <test path>` line per test. `xtask` parses it. The grammar lives in a shared crate (rule 7), name provisional. |
| 2. What gates drift | "Yes, soft warning with a label" | No per-suite budget. A soft warning, failing nothing, when a job on main passes about 85% of its milestone 721 (each merge-group CI job has a 20-minute budget) budget, raised as one tracking issue per job carrying a label. |
| 3. Where the per-run breakdown shows | "Step summary plus artifact, with no PR comment" | Each run's per-test breakdown goes to the CI step summary and to a downloadable artifact holding the full records. |
| 4. The daily record and chart | "Yes on Fork 4, daily record and weekly chart" | A scheduled workflow appends each day's per-test and per-job times on main to a CSV under `notes/project-metrics/`; `script/metrics` charts job times against their budgets weekly. |

## Fork 1: the record travels in the transcript

The console is the only channel aarch64, riscv64 and x86_64 share. The block comes after the suite's
existing reports and before `test result:`, so every line `xtask`, `script/falsifications` and the
HVF leg already parse is unchanged. Milliseconds, because whole seconds round most of this suite to
zero. The grammar is something two programs agree on, the kernel and `xtask`, so it lives in a crate
both depend on and never in a `#[path]` module.

## Fork 2: milestone 721's budget stays the only hard limit

The proposal recommended a per-suite, per-architecture seconds budget in a ratchet file, failing the
job over budget. calef refused it. A second hard limit fails whichever pull request happens to tip
an aggregate over the line, which is what cpu-matrix's 20-minute budget did to #1755 and #1766. That
pull request is rarely the cause; the drift is spread across many.

What replaces it:

1. When a job's run on main passes about 85% of its budget in `.github/ci-job-budgets` (17 of 20
   minutes today), the job emits a warning and stays green.
2. That run's step summary shows the job's time against its budget and its slowest suites and tests,
   from Fork 1's records.
3. The signal is a label, and a label needs an object to sit on; a commit on main has none. So a routine opens one tracking issue per job when the job
   first crosses the line, updates that issue on later runs, and closes it when the job drops back
   under. The issue carries the label (name provisional). Milestone 723 (a stopped merge watcher
   is reported within three of its own intervals) is the precedent for a routine that opens an issue.
4. The per-test records are for diagnosis. Nothing gates on them.

### How the issue reaches a session

calef, 2026-10-06 (UTC), on PR #1778: *"Yes, amend 808 and §254 that way."* The first wording said
the label put the issue where sessions already look. It did not. A session's queue is
`needs-maintainer`, which `helpers/needs-maintainer.jq` computes for pull requests only, and nobody
opens the step summary of a green run on main. A label nothing queries is rung zero of the ladder,
"somebody will notice". So:

1. An open budget issue joins the `needs-maintainer` listing as an eighth cause, `budget` (name
   provisional), in `helpers/needs-maintainer.jq`, its selftest and fixtures, and the drain's query.
   The session reads pull requests and issues in one call to the REST issues endpoint, because a
   second command beside `gh pr list` is one more thing to remember.
2. While a budget issue is open, the weekly job-time chart in `notes/project-metrics.md` carries a
   bullet under it naming the job, its percentage of budget and the issue number. It goes through
   `week-notes.csv` and ages out by milestone 623 (bullet under the chart explains a cliff)'s rule.

Milestone 808 builds both; its exit criteria name them.

## Fork 3: no pull request comment

Step summary plus artifact, every run. A comment on every pull request was estimated (not measured)
at about 60,000 tokens a day of agent context, for a breakdown that is unchanged on most pull
requests.

## Fork 4: a daily record, a weekly chart

The daily collector follows `helpers/merge_queue_share.py`, which writes
`notes/project-metrics/merge-queue-daily.csv` for milestone 724 (the merge queue reports its
ejection share and its time to merge). Per-step times come from GitHub's jobs API, so no job changes to provide them. The weekly
chart plots each job's time against its milestone 721 budget, which is the trend the warning reads
one run at a time.

## What was refused

| Option | Why it lost |
|---|---|
| Writing the record to a host file through semihosting | x86_64 exits through `isa-debug-exit`, which has no file I/O, so it breaks §19 (architectural parity is a tenet). |
| Lowering `SLOW_REPORT_SECS` to zero | Changes a line three consumers already parse, in whole seconds. |
| A per-suite, per-architecture budget ratchet (the proposal's recommendation) | A second hard limit ejects whichever pull request tips the sum, as cpu-matrix's budget did to #1755 and #1766. |
| A tighter per-test ceiling | The drift on record was spread over about a hundred tests; it would have seen about a third. The ceiling stays a hang detector. |
| Report only, with no signal | The floor of the mechanisms ladder, and close to what the tree had while the cpu-matrix drift ran for two weeks. |
| A comment on every pull request | Token cost for nothing new on most pull requests. |
| Artifact only | Drops the step summary, which costs nothing and is where a reader of the run already looks. |

## Open

- Whether the 85% line reads a single run or a median of recent runs is left to milestone 808's
  lane. A single run near the line will open and close the issue repeatedly. If it does, the lane
  damps it and amends this section in the same pull request.

  **Amendment, 2026-10-07 (UTC), recorded by the maintainer from the lane's proposal on PR #1825
  (milestone 808's lane; label name still provisional).** One merge-group run at or past the line
  opens the job's `near-budget` issue, and three runs in a row under it close it. Opening on one
  run keeps the warning as early as the ruling wants, and it fails nothing. Closing on one run
  would flap, since one job on one commit varies by about 30% across hosted runners
  (`helpers/verify_times.py`, measured 2026-10-06). `helpers/ci_job_times.py` holds the rule
  (`CLOSE_AFTER`).
- `helpers/job-budget.py check` prints its warning at 85% of the budget on every run, the ruling's
  line, so there is one warning, not two; if calef wants both, this section is amended.
- The label's name and the shared crate's name wait on ratification.
