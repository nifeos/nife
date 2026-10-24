# The clock grant: the four links, and what delegation would need

*Name: provisional.*

An appendix to [notes/grant-expression.md](../grant-expression.md), moved there on 2026-10-11
(UTC) under §212 (a prose budget). It records how `date` got a clock at the prompt against
what this note predicted.

## The four links this note said were missing, and the one it got wrong

The assessment above listed four, in different subsystems, and three of them were right:

1. The interactive boot started no clock service. Correct, and it was the bulk of the work: the
   kernel starts it before the progenitor exists on both boot paths.
2. Init had no way to receive the page. Correct: it is a read-only frame capability now, granted
   ahead of the filesystem pair so its slot number does not depend on whether a disk was attached.
3. "The spawn protocol carries no clock. A clock is a third position and a new flag word, in both
   inits." Wrong, and the reason is the interesting part. A clock is not designated on the
   command line, so there is nothing for the shell to *send*: the progenitor already decodes the program id, so
   it can read that program's manifest itself and decide. The wire did not change at all. The general
   rule that falls out: a flag word carries what the sender chose, and an authority the sender
   could not choose does not belong on it.
4. The child needs the page mapped and the cap inserted. Correct, at `CLOCK_VA` and slot 1,
   because `date` probes the slot before touching the address.

The old paragraph said all of it "would ship unexercised, because nothing in the test suite
boots the interactive shell". That stopped being true when milestone 50 (pipes and redirection: one sink protocol) wrote
`script/swish-check`,
which is the gate this landed against.

## What a delegable clock would still need

Nothing here lets the *shell* hand a clock to anything. That is a real difference and not a
formality. A shell holding the page could grant it to any child. The progenitor granting it per manifest
means the set of processes that can read the time is decided by declarations rather than by a prompt.
Making it delegable would mean the progenitor keeping a copy for the shell, a `Holdings` field, and a clock
position on the wire after all. There is no program asking for it, so it is recorded and not built.

Recorded-accepted by milestone 94's sweep (2026-08-04). "No program is asking" is the whole
argument and it is a good one: building the wire position first would be a mechanism without a
requirement. Milestone 86 then narrowed it rather than opening it, giving the shell a clock it holds
with `READ` and no `GRANT`, and notes/time-command.md rejects clock delegation for `time`
specifically rather than deferring it. An audit may pass over this. See
notes/untracked-work-sweep.md.
