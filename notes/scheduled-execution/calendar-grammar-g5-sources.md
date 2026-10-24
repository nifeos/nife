# G5's premise, checked against primary sources

*Name: provisional, minted 2026-10-13 (UTC) by lane/prose-cap-notes-c, for G5's "The premise,
checked against primary sources". calef names files.*

Moved verbatim from [calendar-grammar-g5.md](calendar-grammar-g5.md) under §212 (a prose budget) on
2026-10-13 (UTC). That page holds the ruling, the grammar and what the 129 lane builds; this holds
the source reading behind it.

Every source was read on 2026-09-26 (UTC): the RFC and man pages from source text, the vendor
pages live.

| source | what it actually says |
|---|---|
| App Engine `cron.yaml` | `every day 00:00`, `every monday 09:00`, ordinals `1st`..`31st` and `first`..`thirtyfirst`, `of month`, `of sep,oct,nov`. Sub-daily: `every N hours\|mins\|minutes`, optionally `from HH:MM to HH:MM` or `synchronized` (N must divide 24). Time zone is a separate field, UTC by default. |
| RFC 5545 section 3.3.10 | An invalid date such as February 30 "MUST be ignored and MUST NOT be counted". `-1FR` is the last Friday; BYMONTHDAY `-1` is the last day. UNTIL and COUNT "MUST NOT occur in the same recur". BYSETPOS picks the nth of a set, so "the last work day" is `BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1`. WKST changes which days an INTERVAL=2 weekly rule yields. |
| RFC 5545 section 3.8.5.3 | RRULE "SHOULD NOT be specified more than once", and the set from several is "undefined". A DTSTART not synchronized with the rule gives an "undefined" set. |
| systemd.time(7) | `~` counts from the month's end: `Mon *-05~07/1` is the last Monday in May. Shorthands `daily`, `weekly`, `monthly` and five more. No interval anchored to a start date. |
| systemd.timer(5) | Several elapses in one sleep yield "a single service activation". `Persistent=` fires "immediately if it would have been triggered at least once". An active unit is "simply left running". `RandomizedOffsetSec=` and friends spread fleets. |
| Temporal schedules | Spec: calendars, cron, intervals as "epoch + n * interval + phase", inclusive start and end, jitter, zone (UTC default). Policies sit apart: overlap (Skip default, BufferOne, BufferAll, CancelOther, TerminateOther, AllowAll), catch-up window (one year default), pause on failure, remaining actions. "Last day of the month or third Monday are not currently representable." Its cron has no day-field union. |
| Quartz `CronTrigger` | Seven fields with seconds and an optional year. `L` (last), `W` (nearest weekday to a day), `#` (nth weekday), `LW`. One of day-of-month and day-of-week must be `?`. |
| EventBridge Scheduler | `at(yyyy-mm-ddThh:mm:ss)`, `rate(value minutes\|hours\|days)`, `cron(` six fields with year `)`. `?` is required in one day field; `L`, `W`, `#` as in Quartz. A spring-forward time is skipped and a fall-back time runs once. |
| launchd.plist(5) | `StartCalendarInterval` keys Minute, Hour, Day, Weekday, Month; a missing key is a wildcard. Day and Weekday together mean either one. Missed intervals during sleep are "coalesced into one event upon wake". |
| crontab(5) | With both day fields restricted, a job runs "when either field matches": `30 4 1,15 * 5` is the 1st, the 15th and every Friday. |

Five corrections to the maintainer's summary follow from the table.

1. Three of the four example lines are not App Engine syntax. App Engine has no `weekday`, no
   `last`, and no `every 2 weeks`. Its own "every other week" example, `1st,third monday of month`,
   is not every other week in a month with five Mondays.
2. `every 2 weeks on mon 08:00` is not one RRULE as written. INTERVAL=2 counts from DTSTART, which
   the line does not carry, so the fortnight it picks depends on when it was registered. Temporal
   hides this with an epoch phase. An honest line has to carry its anchor.
3. `every 15 minutes from 09:00 to 17:00` is one RRULE only if 17:00 is excluded. BYHOUR and BYMINUTE
   form a product, so 09:00 through 16:45 is one rule and 17:00 on top is a second. The step must
   also divide 60. App Engine's plain `every 5 minutes` is not an RRULE at all: it waits five minutes
   after each run ends.
4. The run-policy words are Temporal's and systemd's, not App Engine's, which builds overlap into
   the interval type. RRULE itself has only UNTIL and COUNT.
5. `every 15 minutes` collides with a stored word. `every 15m` already means a monotonic interval
   from arming, so two lines one space apart would run on different clocks.

One correction to the lane's note, fixed there too: its S2 row said systemd catches up everything
that elapsed, "a stampede". systemd.timer(5) says it fires once. S2 differs from S3 in persisting the
last trigger across power-off, not in running a backlog.
