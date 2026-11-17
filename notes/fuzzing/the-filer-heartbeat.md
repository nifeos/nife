# What the fuzz heartbeat proves about the filer

*Name: provisional.*

An appendix to [`continuous-host.md`](continuous-host.md). The filer, `fuzz-filer` in
[nifeos/fuzz-host](https://github.com/nifeos/fuzz-host), turns each held cause into a private
advisory, and it files only while `fuzz-filer.timer` is enabled on the host. The timer must be
enabled. A release and an ok window prove that the supervisor runs; they say nothing about the
filer, so the heartbeat reads the filer's own line in the daily report.

## The `Filer:` line

`helpers/fuzz-daily-publish.sh` writes one line into `report.md` on the host, from host-local facts
only. It gives:

- held logs, causes, filed and parked, and unfiled: causes that are not parked and have no advisory
  yet. A budget or hang cause that the rules leave parked is not unfiled.
- the newest timestamp in the filer's ledger.
- whether `fuzz-filer.timer` is enabled, and when it last ran.

The counts are the closing tally of `fuzz-filer run --dry-run`, which reads no token and sends
nothing. The script keeps that tally and copies no cause line. A value the host cannot read is
written `unknown`. The line holds counts and timestamps only, and no held log is read.

## What fails

`helpers/fuzz-heartbeat.py` opens the `fuzz-loop-broken` issue, naming the check and the day, when:

- a report from 2026-11-19 on has no `Filer:` line. A release from before that day has none and passes.
- the tally is `unknown`.
- any cause is unfiled.
- `fuzz-filer.timer` is not enabled.
- the timer last ran more than two UTC days before the report's day, or has never run.

A green heartbeat therefore proves a recent release, an ok window, a bounded infrastructure share, an
enabled filer timer that fired within two days, and no held cause waiting for an advisory. It does not
prove that an advisory is correct. The two-day bound and the start day are the maintainer's, and
provisional.

The selftests are `python3 helpers/fuzz-heartbeat.py --selftest` and
`helpers/fuzz-daily-publish-selftest.sh`, both run by `script/lint`.
