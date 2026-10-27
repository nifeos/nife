# A calendar grammar in words: G5, priced against G3 and G4

Status: DECIDED 2026-09-26 (UTC). calef ruled: *"G5 full."* That is the v1 word list below as
proposed, with the refusals as listed. The step rule is S3 plus the `SET` fix under run policy,
adopted by the maintainer as a reversible default and not ruled by calef. Build from
[What the 129 lane builds](#what-the-129-lane-builds).

Written by an agent on the lane `proposal/129-calendar-grammar` for milestone 129 (scheduled
execution). It extends [calendar-and-wall-clock.md](calendar-and-wall-clock.md), which recommended
G3; calef had said *"I don't want to limit us to just nightly and weekly."* The rest of this note is
the proposal as ruled on.

## What is being decided, and which half is irreversible

§122 (the on-disk, per-user schedule store) keeps a user's schedule as `timetable::parse`'s own
text, so the first calendar line a user writes fixes a meaning. The fork is irreversible, and per
`AGENTS.md` it gets options.

The irreversible part is the grammar's shape: the keyword that opens a line, the clause order,
where the command starts, and what an absent clause means. The word list inside the shape is
additive, because every word G5 would add later is a parse error today.

## The premise, checked against primary sources

Every source was read on 2026-09-26 (UTC): RFC 5545, systemd, Temporal, Quartz, EventBridge,
launchd, crontab and App Engine. The table of what each actually says, the five corrections to the
maintainer's summary that follow from it and the one correction to the lane's note are in
[calendar-grammar-g5-sources.md](calendar-grammar-g5-sources.md).

## What the tree already does

- `timetable::parse` dispatches on the first word, `every` or `at-boot`. Any other first word is
  `Error::UnknownSchedule`, so a new one is additive.
- `every <digits><ms|s|m>` is taken. No calendar line exists in the tree, so nobody has acted on a
  calendar grammar yet.
- `crates/calendar` refuses tzdata and refuses month arithmetic, asking "what is one month after
  January 31?". G5's refusal of days 29 to 31 is the same answer.
- Its Kani harnesses prove the day-number conversions inverse over its whole range.
- `crates/periodic_deadline/src/proofs.rs` records that CBMC did not finish a second 64-bit modulo. That is
  the known risk for any next-occurrence proof.
- `clock_contract::state` separates `SET` (an operator) from `SYNCED` (an accepted proposal, bounded
  to an hour forward and a second back). S3 can use that; see the finding under run policy.

## G5's shape: six rules

These are the irreversible part.

1. A calendar line opens with `every`, then a cadence word. `every` followed by a glued interval
   (`15m`) stays the monotonic line; a cadence word (`day`, `week`, a count and a plural) makes it a
   calendar line. Both are one token of lookahead.
2. The schedule ends at the time clause, which is last and ends on a fixed token. `at` takes a time
   list; `from HH:MM to HH:MM by Nm` takes a range and ends on the step. The command is what
   follows, and the parser never reads command words as schedule words. That is what keeps a later
   word from changing an old line whose program happens to share its name.
3. Clauses come in one fixed order, with one spelling per meaning. Lists are one token joined by
   commas (`mon,wed,fri`).
4. Each line is exactly one RFC 5545 RRULE with BYSECOND=0 and WKST=MO. Every field RFC 5545 would
   derive from DTSTART is written in the line. DTSTART therefore matters only as a lower bound, and
   as the phase when a count is above one, where the line must carry it.
5. An absent clause keeps its v1 meaning forever. No zone clause means UTC, even if tzdata ships
   one day. No `starting` means unbounded below. A system-wide zone setting may never reinterpret a
   stored line.
6. Run policy, if any is ever added, goes in a prefix slot before `every` or `at-boot`. It is not
   calendar grammar: overlap and jitter mean the same thing for an interval line.

## The v1 word list and each word's RRULE

All times are UTC. The RRULE column omits `BYSECOND=0;WKST=MO`, which every row carries.

| line | RRULE |
|---|---|
| `every day at 02:00` | `FREQ=DAILY;BYHOUR=2;BYMINUTE=0` |
| `every day at 09:00,21:00` | `FREQ=DAILY;BYHOUR=9,21;BYMINUTE=0` |
| `every weekday at 09:00` | `FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR;BYHOUR=9;BYMINUTE=0` |
| `every weekend at 10:00` | `FREQ=WEEKLY;BYDAY=SA,SU;BYHOUR=10;BYMINUTE=0` |
| `every week on mon,thu at 08:00` | `FREQ=WEEKLY;BYDAY=MO,TH;...` |
| `every month on 1,15 at 00:00` | `FREQ=MONTHLY;BYMONTHDAY=1,15;...` |
| `every month on last day at 23:00` | `FREQ=MONTHLY;BYMONTHDAY=-1;...` |
| `every month on 2nd tue at 09:00` | `FREQ=MONTHLY;BYDAY=2TU;...` |
| `every month on last fri at 17:00` | `FREQ=MONTHLY;BYDAY=-1FR;...` |
| `every month on last weekday at 18:00` | `FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1;...` |
| `every month on 1 in jan,apr,jul,oct at 06:00` | `FREQ=MONTHLY;BYMONTH=1,4,7,10;BYMONTHDAY=1;...` |
| `every 2 weeks on mon starting 2026-10-05 at 08:00` | `DTSTART:20261005T080000Z`, `FREQ=WEEKLY;INTERVAL=2;BYDAY=MO;...` |
| `every 3 days starting 2026-10-01 at 04:00` | `DTSTART:20261001T040000Z`, `FREQ=DAILY;INTERVAL=3;...` |
| `every 3 months on 1 starting 2026-10-01 at 00:00` | `FREQ=MONTHLY;INTERVAL=3;BYMONTHDAY=1;...` |
| `every weekday from 09:00 to 16:45 by 15m` | `FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR;BYHOUR=9,10,11,12,13,14,15,16;BYMINUTE=0,15,30,45` |
| `... through 2027-03-31 at ...` | adds `UNTIL=20270331T235959Z` |

A yearly date is a month rule narrowed by `in`: `every month on 15 in mar at 09:00`.

## The grammar

```ebnf
calendar_line = "every" , ws , cadence ,
                [ ws , "in" , ws , months ] ,
                [ ws , "starting" , ws , date ] ,
                [ ws , "through" , ws , date ] ,
                ws , times , ws , command ;

cadence   = "day" | "weekday" | "weekend"
          | "week" , ws , "on" , ws , day_list
          | "month" , ws , "on" , ws , monthday
          | count , ws , "days"
          | count , ws , "weeks" , ws , "on" , ws , day_list
          | count , ws , "months" , ws , "on" , ws , monthday ;
          (* a count requires "starting" and refuses "in" *)

count     = digit , { digit } ;                  (* 2 to 99 *)
day_list  = dow , { "," , dow } ;                (* one token *)
dow       = "mon" | "tue" | "wed" | "thu" | "fri" | "sat" | "sun" ;
monthday  = dom , { "," , dom }                  (* one token *)
          | "last" , ws , "day"
          | ( "first" | "last" ) , ws , "weekday"
          | ordinal , ws , dow ;
dom       = 1 to 28 ;
ordinal   = "1st" | "2nd" | "3rd" | "4th" | "last" ;
months    = month , { "," , month } ;            (* one token *)
month     = "jan" | "feb" | "mar" | "apr" | "may" | "jun"
          | "jul" | "aug" | "sep" | "oct" | "nov" | "dec" ;
date      = digit4 , "-" , digit2 , "-" , digit2 ;
times     = "at" , ws , hhmm , { "," , hhmm }    (* one token; one shared minute *)
          | "from" , ws , hhmm , ws , "to" , ws , hhmm , ws , "by" , ws , step ;
hhmm      = digit2 , ":" , digit2 ;              (* 00:00 to 23:59 *)
step      = digit , { digit } , "m" ;            (* divides 60, or whole hours *)
```

Two constraints sit outside the EBNF. A `starting` date must itself be an occurrence, because
RFC 5545 leaves an unsynchronized DTSTART undefined. A range must be exact: its start minute below
the step and its end the last step of its hour, so `to 16:45 by 15m` and not `to 17:00`.

## Refused in v1, and why

| refused | why |
|---|---|
| days 29, 30, 31 | RFC 5545 skips a month without them, so `on 31` fires seven months a year and says nothing. `last day` is month-end. |
| `5th <dow>` | The same silent skip, in four or five months a year. |
| a month day and a weekday together | Prior art gives union (crontab, launchd), intersection (RRULE, Temporal) and refusal (Quartz, EventBridge). Two lines give the union. |
| times with different minutes in one list | BYHOUR times BYMINUTE is a product, so `09:00,17:30` would also fire at 09:30 and 17:00. Two lines. |
| an inexact range, or a step not dividing 60 | Not one RRULE. The error names the last reachable time. |
| a count with `in` | A fortnightly rule limited to January can skip years, and the bound on the next fire is lost. |
| a count without `starting`, or a `starting` off the rule | The fortnight would depend on the registration date; RFC 5545 calls an unsynchronized start undefined. |
| zones, `TZ=`, zone names | UTC only. `crates/calendar` refuses tzdata. A fixed-offset clause can be added later and absence still means UTC. |
| COUNT (`times 10`) | Needs a durable counter, and the §122 store holds only the document. `through` needs no state. |
| seconds and sub-minute steps | `every 30s` already exists. |
| Quartz `W`, nearest weekday | Not expressible as an RRULE. |
| full day and month names | One spelling per meaning in a stored format. Aliases can be added later. |
| `every N minutes` spelled out | One space away from `every Nm`, which runs on the other clock. |

## Run policy and the S3 step rule

v1 ships no run-policy words; rule 6's prefix slot is where they go. Overlap today is uniform: an
entry fires beside its running children within the budget, and a `--mem` entry drains first.
Changing that is a timetable decision covering interval lines too. Jitter serves fleets, systemd's
own example. If it comes, it should be a fixed offset like `FixedRandomDelay=`, since random jitter
breaks the proof that each fire is an occurrence.

`starting` and `through` are RRULE bounds, evaluated in wall time, and they meet S3 like this:

- While the clock is `UNKNOWN` a calendar line is dormant. Leaving `UNKNOWN` arms the next
  occurrence strictly after now and fires nothing for the past. That transition is not a step,
  because nothing was armed.
- A forward step fires a line at most once, however many occurrences it covered. A range line
  jumped across three hours fires once, not twelve times. The entry's stamp becomes the latest
  occurrence jumped.
- A forward step past `through` fires the last covered occurrence once, and the line is then spent.
- After a backward step, the next fire is the first occurrence after both now and the stamp. Never
  twice.
- An occurrence before `starting` does not exist, so a backward step to before it leaves the line
  dormant until it.

One change to S3 itself, the maintainer's default since the ruling, reversible because it is
behavior and not a stored format. The stamp rule had a failure the lane's note did not price. An operator `SET` typed as 2030 fires every daily line once and stamps it in 2030. A `SET`
back to 2026 then leaves those lines dormant for four years; the plausibility bound admits up to
2100. The page can tell the cases apart, since `state::SET` and `state::SYNCED` differ. The fix:
a publication in state `SET` is a correction and clears the stamps, because they were taken on
a clock the operator has just called wrong. A `SYNCED` step, bounded to a second backward, keeps
them. The cost is that an operator stepping back an hour may see a line fire again, which the
operator asked for.

## What each option costs

Estimates are Rust code lines without comments or tests. The base is today's parser, about 70
lines, and `next_after`, 11. Neighbors were counted on 2026-09-26 (UTC) without comments.

| | G3: two words | G4: cron fields, no union | G5 v1 |
|---|---|---|---|
| covers | nightly, weekly | anything a crontab can, except "last" and fortnightly | the table above |
| parser, estimate | 40 to 60 | 150 to 200 | 250 to 350 |
| next occurrence, estimate | 30 to 50 | 120 to 180, a search over field bitmasks | 200 to 280, closed form per month |
| measured neighbors | none | cronie `entry.c`, 512 | Temporal `spec.go` and `calendar.go`, 746; systemd `calendarspec.c`, 1,059; dateutil `rrule.py`, 1,453 |
| silent traps left | none in scope | `0 0 31 * *` fires seven times a year unless refused | none: each is a refusal above |

The Kani plan is the same for G4 and G5, and only the predicate differs. `matches(rule, t)` is the
specification, written straight from the RRULE fields over a `Civil` built from fields.
`next(rule, after)` is the implementation. Five harnesses:

1. `next` is strictly after `after`, so a polling pass cannot fire twice.
2. `matches(rule, next)`: every fire is an occurrence.
3. For a symbolic `t` between `after` and `next`, `!matches(rule, t)`: no occurrence is skipped.
4. Totality: for every rule the parser accepts, `next` exists within a bound: the count times the
   period, or twelve months for an `in` line. The refusals make this true, since every month has days 1 to 28, a 4th of each weekday, a last
   day and a last weekday.
5. With `through`, `next` is never after UNTIL.

Time would be a day number and a minute of day, both 32-bit, which keeps the modulo off the 64-bit
path that stalled CBMC. Whether harness 3 finishes is not measured, and it is the one to try first.
Host tests would add a fixture table of occurrences generated once by dateutil's `rrule`, checked in
as data with the command that made it, and no crate dependency.

## How each extends later without changing a stored line

- G3 extends by new first words. Reaching calef's range leaves `daily` as a permanent alias.
- G4 extends by characters it refuses today (`L`, `#`, `W`). Its hazard is familiarity: a pasted
  crontab line means local time and the union, and here means UTC or a refusal.
- G5 extends by tokens that are errors today: a cadence (`year`), an `on` value (`5th`), a clause
  before the time (a fixed offset), a prefix policy word. Rules 2 and 5 keep old meanings.

## The options as presented

| option | shape | cost, estimated | reversibility once stored |
|---|---|---|---|
| G3 | `daily HH:MM`, `weekly <dow> HH:MM` | about 100 lines; Kani trivial | growing past it leaves `daily` as a permanent alias |
| G4 | five cron fields, UTC, day-field union refused | 300 to 350 lines; five harnesses | additive, but carries cron's implied meanings |
| G5 v1 | the rules and table above | 450 to 600 lines; five harnesses | additive by construction |
| G5, smaller first cut | G5's shape, v1 words `day`, `weekday`, `week on` | about G3's cost | additive; month words later |

The evidence favors G5's shape. Nothing has acted on a calendar grammar yet, so choosing it now
undoes nothing. How many words ship first is reversible, and the smaller cut buys G3's cost without
G3's dead end.

Question 7 of `AGENTS.md`. At equal cost we would still choose G5, and not G3: the case for G3
rests partly on its being smaller, which is effort. G4 loses on meaning, promising local time and a
day-field union nife lacks.

## What the 129 lane builds

All of it in `crates/timetable`, host-tested, on all three architectures:

- The parser: [the grammar](#the-grammar) and its two side constraints, a new `Schedule` variant
  holding the parsed rule, and one `Error` per row of [the refusals](#refused-in-v1-and-why), each
  naming its alternative.
- `matches` and `next`, with [the RRULE table](#the-v1-word-list-and-each-words-rrule) as host tests
  and a dateutil fixture table as the oracle.
- The five Kani harnesses under [what each option costs](#what-each-option-costs), in
  `src/proofs.rs`, harness 3 first.
- In `components/src/timetable.rs`: the `clock` grant, the generation check, and S3 with the `SET`
  fix, as listed under run policy.
