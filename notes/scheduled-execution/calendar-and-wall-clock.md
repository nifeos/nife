# Calendar and wall-clock entries: a proposal

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

Status: settled 2026-09-26 (UTC) in [calendar-grammar-g5.md](calendar-grammar-g5.md): calef ruled "G5 full", and the step rule is S3 with a `SET` fix, the maintainer's default. Originally PROPOSED 2026-09-26. Written by the lane for milestone 129 (scheduled execution) for
calef's decision. The block has called each of these "its own decision" since 2026-08-15, and they
turn out to be one: a calendar entry is a wall-clock entry with a vocabulary. What is asked is the
grammar, which is a format two programs agree on, and the rule for a clock that steps.

## What is being decided

A timetable today knows two schedules, `every <interval>` and `at-boot`, both on the monotonic
counter, which needs no capability. The housekeeping milestone 55 (Time Machine) wants is
"thin snapshots at 02:00", which is a time of day. Three questions follow:

1. What an entry saying a time of day is written as.
2. Which time zone that time is in.
3. What it does when the wall clock steps under it.

## What the tree already settles

- The wall clock is a capability. §43 (reading the clock is a page, setting it is a page you may
  write, proposing is an endpoint) makes reading it a read-only page. `timetable::Held` already has
  a `clock` field, so a session that wants wall-clock entries grants its timetable the page, and
  one that does not gets `every 1s date`'s refusal for every calendar line. No new authority is
  needed and none is invented.
- A step is visible. The clock page publishes wall time as an offset from the monotonic counter,
  with a `generation` that counts publications (`crates/clock_contract`). A timetable can hold
  each calendar entry as a monotonic deadline and recompute it whenever the generation moves.
- Steps are bounded. An accepted NTP proposal moves a clock that knows the time at most an hour
  forward or a second back (`clock_contract::policy`). An operator's `SET` is unbounded.
- There are no time zones with rules. `crates/calendar` supports a fixed UTC offset and refuses
  tzdata on purpose. Every date in this tree is UTC by rule. So daylight saving cannot arise unless
  somebody ships tzdata, and that is a separate decision the calendar crate already declined.

## The options for the grammar

| | shape | why |
|---|---|---|
| G1 | cron's five fields, `0 2 * * *` | Familiar, and a whole language: ranges, lists, steps, day-of-month versus day-of-week union. Most of it is unused by housekeeping, and every part is a parser to prove. |
| G2 | systemd's `OnCalendar=` | More expressive than cron, larger again. Read, not built: systemd.timer(5). |
| G3 (recommended) | two words, `daily HH:MM` and `weekly <mon..sun> HH:MM`, in UTC | Matches the two words this document already has. Covers nightly and weekly housekeeping. Adding `monthly` later is an additive line, not a new language. |

## The options for a step

| | rule | cost |
|---|---|---|
| S1 | Vixie cron's: a forward jump under three hours runs the skipped jobs soon after; a backward jump under three hours does not re-run the repeated ones; three hours or more is a correction and the new time is used as is (cron(8), read 2026-09-26). | A magic number, chosen for daylight saving, which this tree does not have. |
| S2 | systemd's: `Persistent=` stores the last trigger on disk and fires once for whatever elapsed while off (systemd.timer(5), read 2026-09-26). | Corrected 2026-09-26 (UTC): not a stampede, since systemd fires once. It differs from S3 by persisting a stamp across power-off. See [calendar-grammar-g5.md](calendar-grammar-g5.md). |
| S3 (recommended) | Recompute every deadline when the generation moves. An occurrence whose time a forward step jumped past fires once. An occurrence already fired is never fired again after a backward step, because each entry remembers the last wall-clock occurrence it fired. | One word of state per entry. It is the interval rule (skip, do not catch up, never twice) applied to wall time. |

And while the clock's state is not known (`state::UNKNOWN`), a calendar entry is dormant: armed, not
due. A machine that does not know the date should not guess it at 02:00.

## Costs, measured where they can be

- Grammar: G3 adds two schedule words to `timetable::parse` and one variant to `Schedule`, which
  the store of §122 (the on-disk, per-user schedule store) carries unchanged because the store's format is the timetable's document.
- Runtime: one clock-page read per pass, a seqlock load, beside the counter read the loop already
  makes. Not measured; there is no calendar entry to measure.
- Proof: the deadline arithmetic belongs in `crates/timetable`'s Kani harnesses next to
  `next_after`, and wall time converts through `crates/calendar`, already host-tested.

## Reversibility

The grammar is stored on disk per identity (§122), so once a user writes `daily 02:00` it is a
format somebody has acted on. That makes G3 the expensive half. The step rule is behavior, cheap
to change until a housekeeping job depends on it.

## Would we still choose G3 and S3 at equal cost

Yes. G3 is smaller and matches the existing document; S3 is the rule this timetable already
follows for intervals, so a reader learns one rule rather than two.

## What a yes unblocks

A lane adds `daily` and `weekly` to the grammar, a `clock` grant to a registrar-mode timetable's
spawn, the generation check to the loop, and the Kani harnesses, on all three architectures. A no
leaves the grammar at two words and milestone 129 PARTIAL on this item.
