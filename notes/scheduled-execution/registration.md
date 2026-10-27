# Runtime registration: a proposal

Status: PROPOSED 2026-09-26. Written by the lane for milestone 129 (scheduled execution) for calef's decision. It asks for a
wire format, which two programs agree on, and for where a user's schedule lives. Both are an
architect's call under AGENTS.md, so nothing here is built.

## What is being decided

Today `components/timetable.conf` is compiled into the scheduler. The right to register an entry is
the right to rebuild the image. A person who logs in should be able to schedule a job on their own
authority, change it, and remove one line of it while their session lives. Three questions follow:
which process holds a user's schedule, what crosses the wire, and how one entry is removed.

## What waits on the answer

- A `REPLACE` handler in `components/src/timetable.rs`, and its constant in `crates/timetable`.
- Wiring `crates/timetable_store` into a running scheduler rather than only into `session_reviver`.
- Milestone 129's removal item, which has had no answer since #387.

All three also wait on milestone 152 (durable delegation), whatever the answer. The registrar is a user's durable
session, and that type was deleted with `smb_server` on 2026-08-30. 152's lane claimed the rebuild
on 2026-09-26.

## The options

### A. One system timetable, one document per registrar

One scheduler for the machine. Each session is handed its own registration endpoint, and the
scheduler keeps a `Held` and a document per endpoint. It lost on authority. The scheduler must back
every entry it fires, so it holds the union of every user's delegated authority. Compromising it
yields everyone's, which is the sum this milestone exists to bound, now taken across users. Revoking
one user under §108 also becomes a message the scheduler must act on, rather than a subtree death.

### B. One system timetable, one entry per call

`ADD` returns a handle and `REMOVE` takes one. It has A's union problem, and two of its own. A
capability handle does not survive a reboot, so after `session_reviver` re-derives a schedule the
user's old handles name nothing. A numeric handle is an ambient name the scheduler must check
against its caller. And §122 stores a whole document per identity, so every per-entry call has to be
folded back into a document before it can be written.

### C. One timetable per durable session, whole-document replace (recommended)

The session spawns its own timetable from its own budget and supervises it. The `Held` it passes is
exactly the authority it lets its jobs hold. The endpoint to that timetable is the registration
right, so there is no badge and no table. There is one operation, `REPLACE`, which carries a whole
document; the reply carries the plan. Removing an entry is a `REPLACE` without that line. Removing
all of them is an empty document, or the session destroying its timetable.

## Why C

- The compromise radius is one user's delegated authority. The milestone's claim holds per user.
- The cascade in §108 (disabling credentials kills the durable session) costs nothing. Revoking
  credentials kills the session, and §40 (a supervisor's death is its subtree's death) takes its
  timetable and every job with it.
- §109 (attribution is a property of a channel) already decided on a fresh object per principal rather than a shared view.
  `components/src/login.rs` builds a fresh budget and directory for every login. A timetable per
  session applies the same rule once more.
- `Registry::register(doc, held)` is unchanged. `held` is simply what the session gave it.
- The store is already this shape. §122 (the on-disk, per-user schedule store) keeps one whole document per identity. Registration becomes
  "write the file, then `REPLACE`", and boot becomes "spawn a timetable with the file".

Prior art, read on 2026-09-26: crontab(1) is whole-file. Installing replaces the table, `-e` edits
and reinstalls it, and `-r` removes it. systemd-run(1) creates one transient timer unit per call.
Stopping one by unit name through `systemctl` is recalled rather than read. B is the systemd shape,
and it pays for named units, which here would be ambient names.

## Five smaller choices inside C

Each has a recommendation, and each is cheap to reverse until a second program speaks the protocol.

1. Transport. The document goes in a shared page, the shape `filesystem_contract` already uses
   for names and file bytes. A document holds at most `MAX_ENTRIES` (8) entries, and the shipped
   one is 200 bytes once comments are stripped, so one 4096-byte page is ample. The reply is one
   word of verdicts, eight bits per entry, with the printed plan in the page.
2. Atomicity. The new document is parsed and planned whole before it replaces the old one. A
   parse error leaves the old schedule running and returns the line number.
3. Phase across a replace. An entry whose text is byte-identical keeps its beat, and a new or
   changed one arms fresh. Otherwise editing one line would shift every other schedule.
4. In-flight children. A job from a removed entry runs to completion and is reaped normally. An
   edit should not destroy work already started.
5. Who writes the store. The session writes the file, since it holds the directory, then sends
   `REPLACE`. The timetable keeps holding no directory. Its four-capability list is the
   milestone's demonstration and should not grow for persistence.

## What each costs

- Memory. C runs one timetable per scheduling user. The kernel test sizes one at a 768-page
  budget and a 32-page stack (`system_tests/src/user/timetable_tests.rs`). A real session chooses its own
  budget, and a one-entry document needs one 48-page instance at a time.
- The yield loop. This is the larger cost, and the best argument for A. There is no timed wait
  here. Milestone 106 (a wait that ends on either the interrupt or the deadline) is gated on
  milestone 263 (can a userspace process hold a timer). Until then every running timetable spends
  a core's worth of yields. N scheduling users means N such loops, where A pays once. The cost is
  temporary: `Registry::next_deadline` already computes what a timed wait would block on. It is not
  measured here, because no boot runs two timetables.
- Code. C needs one operation and its handler. A needs that plus a per-registrar table and a way
  to mint endpoints. B needs handles and document folding. That is effort, and it is not the reason.

## Is the premise true

Checked on 2026-09-26 against `256815e56`:

- `Registry::register` takes `held` as a parameter. True.
- A durable session type exists. False: it went with `smb_server`, and `session_reviver.rs` cites it
  only in comments. The registrar needs 152's rebuild.
- The timetable runs in the interactive boot. False: only `system_tests/src/user/timetable_tests.rs`
  spawns it, and `crates/system_initializer` never names it. C adds no process to a real boot until
  a session schedules something.
- `crates/timetable_store` has no removal verb. True, and under C it needs none.

## Reversibility, and whether we would still choose C

The operation number and page layout are a wire format. They get expensive once a second program
speaks them, and today nothing does. Which process holds the schedule is cheaper to change while no
user has registered anything. The on-disk store does not depend on that choice, because every
option reads §122's document.

At equal cost, C still wins. It is also the smaller build, but the reasons are the compromise radius
and the free cascade. It narrows the one-image-per-entry gap as well, since a compromised timetable
then reaches one user's plan rather than the machine's.

## What a yes unblocks, and what a no means

- Yes to C, with the five choices. A lane builds `REPLACE` in `components/src/timetable.rs` on
  all three architectures, with the kernel test harness standing in as registrar. It does not have
  to wait for 152. Connecting a real session does.
- A instead. The same handler plus a per-registrar table, and a `BUGS` entry recording the
  cross-user union where the scheduler's authority list is written.
- No to all three. The document stays compiled in and milestone 129 stays PARTIAL on this item.
