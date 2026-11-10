---
status: BUILT
promoted_from: where-the-daily-fuzz-output-goes
raised: 2026-11-01
built: 2026-11-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1026. Where the daily fuzz output goes

Raised by lane/continuous-fuzzing-host on 2026-11-01 (UTC). An architect ruled option A on 2026-11-01 (UTC); "What is in force" below states the ruling.

Reuse: `gh release create` and `git push` already do the publishing; the proposal writes no new
transport.

The continuous fuzzing host writes a daily report with counter totals, the clean seed ranges, and the
count and age of held findings (`helpers/fuzz-daily-publish.sh`). The report has to land where others
can read it. The corpus and the findings are public after the fix, which needs a stable address. The
heartbeat needs a newest report with a date. Held findings never appear in it, and a finding starts as
a private security advisory.

Two places fit, and each needs a wider credential than the host holds today.

## What is in force

Ruled 2026-11-01 (UTC): option A. The continuous fuzzing host publishes its daily output as one GitHub
release per day on nifeos/nife, tagged `fuzz-YYYY-MM-DD`, with `report.md` and `clean-ranges.tsv` as
assets. It does not publish a branch of artifacts. The reasons:

1. A release is deletable without touching history.
2. A release adds nothing to any clone.
3. The newest release's date is the heartbeat.
4. Moving from a release to a branch later is compatible; moving from a branch to a release is not.

The ruling rejects option B for the reason under it: every clone that fetches all branches pays for
it, and dropping data needs a force-push, which this project forbids. The implementation is in
`helpers/fuzz-daily-publish.sh` and [the host note](../../notes/fuzzing/continuous-host.md).

## Options

- A. A release per day, tagged `fuzz-YYYY-MM-DD`, with the report and the clean-range table as
  assets. Maintainers can delete an old release without touching history. The heartbeat is the newest
  release's date. Nothing enters the git tree or any clone.
- B. A branch of artifacts, one commit per day on a branch of nifeos/nife that holds the same files.
  Plain `git` reaches the files and shows each day's diff. Every clone that fetches all branches pays
  for it, and dropping data needs a force-push, which this project forbids.

Prior art: OSS-Fuzz publishes coverage reports and corpora to a bucket, and syzbot serves a public web
page. Neither writes to the source repository (from memory, not re-read). Option A is the nearest
analogue here.

## Recommendation

Option A. Both options cost the same to build, so effort plays no part. A release is deletable, which
matters for a place that must never carry a held input by mistake, and it keeps daily data out of every
clone. Moving from a release to a branch later stays compatible; moving back does not.

## What this blocked

The `--publish` path in `helpers/fuzz-daily-publish.sh` refused until the ruling. It now calls
`gh release create` and still refuses unless the host sets the enable variable. The credential is an
installation token of the `nife-fuzz` App, which holds Contents write and advisories write on
nifeos/nife alone (ruled 2026-11-01 UTC,
[fuzz-findings-without-a-person](1015-fuzz-findings-without-a-person.md)).

## Done when

The code builds every claim of the ruling, and `helpers/fuzz-daily-publish-selftest.sh` checks each
one. `script/lint` runs it with stand-ins and never calls GitHub.

- The tag is `fuzz-YYYY-MM-DD` (UTC), and the one `gh release create` call names `nifeos/nife`.
- The assets are `report.md` and `clean-ranges.tsv`, and no others. The script never calls `git push`.
- `--publish` refuses unless `NIFE_FUZZ_PUBLISH=1`, and then refuses by name a missing, unreadable,
  expired or nearly expired token, all before any `gh` call.
- The report holds counters and the count and age of held findings. It and the table name no held
  seed, and the table leaves out every range of a slot from its first held window onward.
- The heartbeat (`helpers/fuzz-heartbeat.py`) reads the date of the newest `fuzz-*` release, and is
  quiet until the first one exists.
- The credential is an installation token of the `nife-fuzz` App, minted by `fuzz-token` and limited
  to Contents write and Repository security advisories write on nifeos/nife.

What remains needs a person, which is why `needs_person` stays `yes`. No code remains
to build.

- Create the `nife-fuzz` App and place its key on the fuzz host root-only
  ([the runbook](../../notes/fuzzing/the-fuzz-app.md), steps P5 and P6).
- Set `NIFE_FUZZ_PUBLISH=1` in the publish service's drop-in on the host (step P7). The first release
  then switches the heartbeat checks on by itself.

## Follow-on

- **Recorded.** The person-only steps above: the App key (P5, P6) and the enable variable (P7). They
  stay a person's, and [the setup list](../../notes/fuzzing/finishing-the-setup.md) holds their
  order and checks.

## Index row

The daily fuzz report is published as one GitHub release per day, with the clean ranges and the counters and no held finding. The corpus has a stable address and the heartbeat has a dated report to read.
