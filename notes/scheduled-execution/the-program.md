# The timetable program: what it holds and how it runs

An appendix of [scheduled execution](../scheduled-execution.md). It carries the design that
`components/src/timetable.rs` links from its doc comments. The crate's registry and grammar are in
the parent page; this file is about the program that runs them.

## What it holds

The program holds `timetable::contract`'s table and nothing else:

- the plan page, and an untyped budget that pays for every instance's region, the loader's scratch
  mappings, and the well a `--mem` grant nests in;
- the supervision and reaping endpoints;
- in `a1`, an archive of exactly the programs the plan builds;
- in `a2`, the registration page of §222 (who holds a user's schedule), or zero.

Without a registration page the document is the compiled-in `timetable.conf`. `timetable::Audit`
prints how wide the archive was, next to the plan, so the spawn site's narrowing can be checked from
inside. The audit measures and does not enforce, because a process cannot narrow its own endowment.
`system_tests/src/user/timetable_tests.rs` builds the sub-archive from exactly `Registry::programs`.
Why the set is a written list is in [archive-narrowing.md](archive-narrowing.md).

It holds no clock page unless one is granted. It holds no directory, console, network or device,
which is why a scheduled `date` is refused at registration. A plan holds a kilobyte-sized
`grant_plan::Endowment` per entry, so the program needs a big stack. An overflow reads like a wild
pointer, not like a stack fault.

`_start` states what it holds once, as a `timetable::Held`, in the vocabulary registration checks
against. Every field but `mem_pages` is false, and each is a fact a reader can check against the
slot list. `timetable::SHIPPED_HELD` is the one number that has widened since the first stratum of
milestone 129 (scheduled execution). The crate's host test uses the same constant, so the two cannot
drift apart, and widening it again shows in the printed plan. The report endpoint is left out when
the spawn site placed none. A durable session never places one (Fork 6 C of milestone 152 (durable
delegation)): a durable job writes through what its entry grants.

## Probes before anything is allocated

`_start` reads four slots before it allocates anything: the activation slot, which is what makes
it store mode, the run-unvouched slot, the clock slot and the report slot. Until then an occupied slot can only be what the spawn site placed there. A
region split later could land in the slot and read as held.

A scheduled job never holds the run-unvouched capability, gate D2 of §219 (how the shell names an
installed program to the spawner). That is how §220 (trusting a key is scoped) keeps its reach.
Dropping trust in a key has to reach every program that could run an unvouched image, and a job
firing long after its session's key was dropped is the one it would miss.

The check runs once, at `_start`, and once suffices. A job's authority is built in `fire` and
`fire_with_grant` from two sources only: the report endpoint, and a region split from the budget.
Neither can be the capability unless this process holds it. It can hold it only if a spawn site
put it there, because the program makes no `RECEIVE_CAP` after `_start`. A timetable that does not
hold it at `_start` can never endow a job with it, and one that does runs nothing.

## Store mode

Store mode is a timetable holding `activation/`, milestone 152's Fork 8 D. It holds read-only views
of the store's `activation/` and `packages/` instead of an archive, and refuses to run without a
registration page. Each entry's program is resolved as the prompt resolves a bare name, through
the live generation's default row (`installed_programs::lookup`), never an owner's vouch. The bytes
are staged from `packages/<name>/<version>/<program>` and used only if they hash to the entry's
digest. They get what their manifest declares, as the progenitor endows an installed program run
at the prompt, less any network: a timetable endows none (`timetable::schedulable`).

At every fire the name is resolved again and the line planned again (`Registry::plan_at_fire`),
so an upgrade fires the new version and an uninstall stops the job at the next beat. When the name
no longer resolves or the plan no longer fires, the beat is skipped and never retried.

## Registration while running

With a registration page, the loop polls for a replacement document once per pass. Everything
that can refuse happens before anything changes: the length, the text, the parse, the
registration and the program lookup. Only then is the registry swapped, armed with
`Registry::arm_after` so unchanged lines keep their beat. This is §222's all-or-nothing
sub-ruling.

The document is copied out of the page first, because the reply overwrites the page with the plan
and a registrar may stage the next document as soon as it reads the reply. The program keeps two
buffers and alternates: the replacement is registered and resolved in one while the registry in
force still borrows the other. The reply goes into the page before anything goes down `OUT`, and
the reply word is stored last with release ordering. An empty document ends the process once its
running jobs finish.

A registration page also makes the program silent on `OUT`. Its registrar is a session blocked on
supervision that cannot drain a stream, and a `SEND` nobody takes would stop the loop. The exit
code goes into the page's exit word instead, which the registrar reads after reaping it.

## Running

The plan is printed before anything fires: what every scheduled child will hold, or why it will
never run. Then every admitted entry's program is resolved, so a plan naming a missing program
fails at start-up rather than as a fire that quietly does not happen. After that point nothing
looks anything else up, which is what makes `Registry::programs` checkable. The line
`timetable: armed` ends the plan, because the plan and the running share one stream.
`system_tests/src/user/timetable_tests.rs` reads to exactly that line.

Each pass reads the wall clock once, when the clock page is held, before anything is due. Calendar
rows are dormant while the time is unknown and re-armed when its generation moves
(`Registry::observe`). Without the page every calendar line is `Unbacked::WallClock`.

A fire builds one instance in its own region of 48 pages, the number `components/src/spawner.rs`
uses for the same job. One region per instance is what lets a single `MemoryRegion::DESTROY` reclaim
a whole dead child. The instance is endowed at most what the printed plan shows. Slot 0 holds the
report endpoint when this timetable hands one out. Slot 1 holds the clock page, `READ` only and
mapped read-only at `contract::CLOCK_VA`. A `--mem` grant, narrowed to `WRITE`, comes next. That is
the progenitor's order, and `system_initializer` narrows a shell's `--mem` delegation the same way.
Its supervision endpoint goes in the reserved fault slot, which `START` reads and clears, so the
child holds no authority on its own death channel. Nothing is kept afterwards. The TCB capability is
not the thread, and since §32 (collect a corpse without being able to build one) the region
capability is not the reap either.

When the budget cannot back another instance, the loop blocks until a corpse comes back, then tries
once more. A second failure, or a failure with nothing outstanding, is a budget too small for one
instance, which is a wiring error rather than congestion. A `--mem` entry runs alone; why is in
[mem-entries.md](mem-entries.md).

Exits and faults are counted apart, because §26 (the fault endpoint) delivers both. A crash is a
fact worth reporting about a scheduled job, and a total would hide it. The kernel is the only
sender on the deaths endpoint, so its tid needs no badge. A reap refused with `NotPermitted` is
retried up to 1,024 times, as `components/src/job_undertaker.rs` does: the refusal is about the
region's other residents, one preemption is enough, and running out traps, because a corpse that
never becomes collectable is a leak.

## BUGS, in full

- The program has not adopted the `Timer` of milestone 106 (a wait that ends on either the
  interrupt or the deadline). It yields between fires, polls for registrations and reaps lazily.
  Adopting it is milestone 674 (the last yield-spinners sleep on a timer), whose timetable section
  lists the steps. `Registry::next_deadline` is already the deadline it would sleep until.
- The archive holds every image the plan builds. That is code, not authority, because a job holds
  only what `fire` endows; [one-image-per-entry.md](one-image-per-entry.md) has the argument.
- `--mem` entries run one at a time, alone, and interval entries due meanwhile run late, once
  ([mem-entries.md](mem-entries.md)).
- A hung child stops the whole timetable once the budget runs out. The loop then blocks on the
  supervision endpoint, and nothing kills a livelocked child.
- In store mode a program over `contract::STAGING_BYTES` is refused without saying which refusal
  it was, and a fire failure is reported only as the page's exit word, `contract::E_BUDGET`.

*Name: provisional, minted by the lane for milestone 929 (comments in the Linux style across
first-party Rust). calef names files.*
