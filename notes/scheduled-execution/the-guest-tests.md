# The timetable's guest tests

An appendix of [scheduled execution](../scheduled-execution.md), for
`system_tests/src/user/timetable_tests.rs`. The parent says what the timetable does. This file says
how each kernel test proves it, and why each one is shaped the way it is.

## Every test counts and never times

No test here asserts that a fire happened within any interval, and none compares a wall clock to
anything. A test asks for a number of fires, or for the identity of the next report, and waits. A
slow host makes a test slower and never makes it red
([load-sensitive assertions](../load-sensitive-assertions.md)). The harness's per-test ceiling is
the only backstop.

Waiting is also why `report` watches two endpoints. A plain `RECEIVE` on the report endpoint would
turn every fire failure into the 60-second watchdog hang. The timetable says "the budget cannot back
one instance" on its output endpoint and stops, while the test waits on a report that never comes.
So `report` fails with the timetable's own sentence if the timetable speaks first.

## The shipped document

`a_scheduled_entry_holds_what_the_plan_said_and_a_refused_one_never_runs` is one test because its
three parts are one claim. A scheduler that fires proves nothing about authority. A printed plan
proves nothing if what fires disagrees with it. A refusal proves nothing unless something else in
the same document did fire, since a scheduler that fired nothing would satisfy it trivially.

The shipped document exercises the parent's four answers:

- `least_authority_demo 7`, `least_authority_demo 3` and `memory_grant_depleter --mem 4` are
  planned, backed and fired. Their answers come back on the endpoint the plan said they would
  hold. The third is milestone 129 (scheduled execution)'s `--mem` grant: this timetable holds
  `timetable::SHIPPED_HELD.mem_pages` pages it may split off, and `memory_grant_depleter` maps some
  of them and reports how many.
- `memory_grant_depleter` with no `--mem`, and `wc`, are refused by the prompt's own check. The same
  lines typed at a shell get the same sentences, because it is the same function.
- `date` and `ps` are refused because this timetable holds nothing to back them. On Unix a clock and
  a process listing are ambient, so those lines would run.

Reading the plan is half the claim. The timetable is blocked on its plan's sends until the test
takes them, so nothing can have fired before the plan was complete. The summary's clean-exit count
is the supervision half. Each child was born with the timetable's supervision endpoint in its
reserved fault slot, died, reported its death and had its region reclaimed through
`Rendezvous::REAP`. A scheduler that leaked a region per fire would still print the fire count, and
would run out of budget instead of finishing.

The fired values are the negative control. Every refused entry is a program that would write
something else on the report endpoint (`date` and `wc` write `byte_sink_contract` bytes), so a
leaked refusal fails on the value rather than on a count.

## Replacing the document

`a_registrar_replaces_the_document_whole_and_a_failed_replacement_changes_nothing` stands in for the
registrar, which will be a user's durable session once milestone 152 (durable delegation) rebuilds
one. It sends four replacements, each the control for the others:

1. The stored schedule's shape, `STORED_SCHEDULE_CONTROL`, with the program renamed. §222 (who holds
   a user's schedule)'s fifth sub-ruling is that the session writes the store and then replaces, so
   what the store holds must be exactly what a replacement accepts, unedited. Its `at-boot` line
   fires (9).
2. A document that does not parse. It is refused whole, with its line number in the page, and
   nothing fires: the schedule in force is untouched.
3. An edit. The `at-boot` line is resent byte for byte and keeps its beat, so it does not fire a
   second time. The `every 30s` line is removed. A new `at-boot` line fires once (16), a scheduled
   `date` is unbacked, and a new hourly line arms and never comes round.
4. An empty document, which ends the timetable. The run was asked for no fire count, so nothing else
   could end it. A timetable holding nothing would still hold its session up, under §16 (object
   revocation)'s live-children rule.

One tolerance is stated rather than hidden. The stored schedule's `every 30s` line is in force from
the first replacement until the third, and a host stalled for thirty seconds in between would let
it fire. A 49 before the 16 is therefore accepted and counted, and nowhere else. Everything the test
claims is about 9, 16 and the page.

## The calendar test, and why its publications are safe

`a_calendar_entry_keeps_time_by_its_granted_clock_and_a_step_moves_it_by_s3` is the timetable's
registrar and its clock. The timetable is granted the clock page starting UNKNOWN. Every assertion
is the identity of the next report, so a line that fired while the clock was unknown, or fired twice
after a backward step, puts a report where the next assertion expects another. Its steps:

1. Registered while the clock is unknown: two calendar lines, admitted and dormant.
2. The clock becomes known a second before 02:00 and runs on. The 02:00 line fires when it gets
   there, and not before.
3. A `SYNCED` step back to 01:30 fires nothing, then forward to 03:05 fires only the 03:00 line,
   once. That line is `date`, which gets the clock page too and prints the day the page says.
4. An operator `SET` back to a second before 02:00 clears the stamps, so 02:00 fires again.
5. An empty document ends the timetable.

Nothing tells the test when the timetable has read the page, so no assertion may depend on it. Each
step publishes at most once before waiting on a report, with two exceptions. Step 3's pair has the
same answer whether or not the timetable saw the first of the two, because step 2's stamp keeps
02:00 from firing across either generation. The other exception is `fires_at_two`'s retries.

`fires_at_two`'s lower bound is what makes the wait mean something. Without it, a timetable that
fired a calendar line the moment it saw the clock, a minute early, met no assertion and ended in a
60-second watchdog hang (measured 2026-10-06). A minute is the grammar's resolution, so that is the
earliest an early fire can be.

A missed publication is published again, never failed. A timetable whose first pass after the
publication came at 02:00 or later armed tomorrow's occurrence, and only a later generation
re-arms it. The retry is always `SYNCED`, which is safe whatever the timetable had done:

- It never read the first publication, or read it late and armed tomorrow. The retry is a new
  generation before 02:00, so 02:00 arms and fires once.
- It armed 02:00 and has not fired yet. The retry re-arms the same occurrence, once.
- It fired and the report is slow. The stamp it took at 02:00 keeps the retry from firing again. A
  `SET` retry would clear that stamp and fire twice, which is why a retry is never one.

So a step-4 `SET` the timetable never read is not retried into existence. The `SYNCED` retry fires
only if the stamp was cleared, which is the property step 4 claims. That case needs a pass within
`LEAD` plus `MISSED`, five seconds. Step 2 has no stamp to protect and is covered by the retry
whatever the delay.

`LEAD` is not what the test proves. It only has to outlast the gap between a publication and the
timetable's next pass, and a pass is one yield of a loop that never blocks here, so one second
covers it with room.

## The spawn site's own choices

- The stack size is stated, not inherited, as `supervision_protocol`'s `CHILD_STACK_PAGES` rule
  asks. The timetable's working set is the plan: a `grant_plan::Endowment` is a kilobyte, and a
  `timetable::Registry` holds one per entry. Too small a stack dies with a data abort whose faulting
  address is the stack pointer. That is what a stack overflow looks like from the kernel, and it
  reads like a wild pointer when it is not one.
- `spawn_timetable_with` does not use the suite's shared progenitor-shaped spawn. Why is in
  [milestone 794 (one progenitor-shaped spawn for the system tests)](../../design/roadmap/0794-one-progenitor-shaped-spawn-for-the-system-tests.md).
- The archive is copied into fresh pages rather than shared with the initial ramdisk's, so it costs
  the address space a page each. That is the price of narrowing: no capability in this system names
  "these blocks of that archive", so a narrower endowment is a smaller archive, and a smaller
  archive is bytes somebody has to own.
- Each test gives back the budget, the TCB region and the endpoint region. The kernel's region table
  is finite, and a later test's fire once failed with "the budget cannot back one instance" at 252
  of 256 live regions. `reclaim_region` refuses a region with a child still carved out of it, so the
  budget's reclaim also proves every scheduled instance was collected.