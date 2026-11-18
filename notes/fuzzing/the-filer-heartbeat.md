# What the fuzz heartbeat proves about the filer

*Name: provisional.*

An appendix to [`continuous-host.md`](continuous-host.md). The filer, `fuzz-filer` in
[nifeos/fuzz-host](https://github.com/nifeos/fuzz-host), turns each held cause into a private
advisory. It files only while `fuzz-filer.timer` runs on the host, so a person must enable the timer.
A release and an ok window prove that the supervisor runs and say nothing about the filer. The
heartbeat therefore reads the filer's own line in the daily report.

## The `Filer:` line

`helpers/fuzz-daily-publish.sh` writes one line into `report.md` on the host, from host-local facts
only. It gives:

- the held logs, causes, filed causes and parked causes.
- the unfiled causes: those the rules do not park and no advisory covers yet. A budget or hang cause
  that the rules park does not count.
- the newest timestamp in the filer's ledger.
- whether `fuzz-filer.timer` has its enable switch on, and when it last ran.

The counts come from the closing tally of `fuzz-filer run --dry-run`, which reads no token and sends
nothing. The script keeps that tally and copies no cause line. When the host cannot read a value, the
line says `unknown`. The line holds counts and timestamps only, and nothing reads a held log.

## What fails

`helpers/fuzz-heartbeat.py` opens the `fuzz-loop-broken` issue, naming the check and the day, when:

- a report from 2026-11-19 on has no `Filer:` line. A release from before that day has none and passes.
- the tally says `unknown`.
- a cause waits for an advisory.
- `fuzz-filer.timer` has its enable switch off.
- the timer last ran more than two UTC days before the report's day, or never ran.

A green heartbeat therefore proves a recent release, an ok window, a bounded infrastructure share, an
enabled filer timer that fired within two days, and no held cause waiting for an advisory. It does not
prove that an advisory is correct. The maintainer chose the two-day bound and the start day,
provisionally.

`python3 helpers/fuzz-heartbeat.py --selftest` and `helpers/fuzz-daily-publish-selftest.sh` test these
rules, and `script/lint` runs both.
