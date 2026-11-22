---
status: NOT-STARTED
raised: 2026-08-04
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 103. `^C` stops spinning: the shell's interrupt watch, blocking

Raised 2026-08-04 as "`^C` is decided and ready to schedule and nobody
built it", which is what `notes/session-handoff.md`'s "Wave-3: what's next, roughly in order" says. The note is stale and `^C` is built.
What is genuinely unbuilt is one line of §24's own implementation record, and this block is that
line.

Milestone 106 (a wait that ends on either the interrupt or the deadline) was the
gate and is BUILT (2026-09-26): a `Timer` object that signals a notification, and the binding of
milestone 151 (notification objects). The watch now sleeps a tick per look; what is left is making the job's end and `^C` signal
the notification the shell waits on. See 106's block.

The correction first, because the stale claim is the more useful finding.
`notes/session-handoff.md`'s "Wave-3: what's next" list has five items. Items 2 and 3 were struck
through and marked DONE when they landed. Item 1, `^C`, was never struck, and it still reads
"(§24 decided, not built) ... Ready to schedule". The evidence that it shipped is not subtle:

- `DECISIONS` §24 (interrupting the foreground process) carries a section headed "Implementation amendment (built): two primitives
  forced the shape", describing both tiers as running on both ISAs.
- `line_editor` implements `OPERATION_INTRCOUNT`; `crates/grant_plan` holds `jobframe` (the per-job
  shared interrupt frame) and `Escalation` (the host-tested escalation policy); `interruptible` is a
  manifest field the shell sets per program.
- §24 records that "a pure `loop {}` spinner is now torn down on the second `^C` on both ISAs",
  which required the §16 (object revocation) amendment teaching `MemoryRegion::DESTROY` to force-kill a live resident thread.

A handoff note is a snapshot of one session and nothing updates it when the world moves. Milestone
93 (documentation audits as a mechanism) is the general answer; this is one more measured instance
for its evidence pile.

What is actually not started. §24's amendment names its own interim in plain words: "The shell
learns of `^C` by polling, deliberately (wait A). The shell must watch the job and the `^C` at
once, and with only blocking primitives it cannot block on both." So `swish` busy-polls
`line_editor`'s `OPERATION_INTRCOUNT` with a `yield` between calls, for as long as a foreground job runs.
§24 finishes the thought: "the shared flag and the poll are the honest interim, not the
destination."

That costs a runnable thread for the entire lifetime of every foreground job, which is the whole
time a user is waiting, and it is scheduler work proportional to how long the command takes.
`line_editor`'s own `OPERATION_INTRCOUNT` doc says it is waiting for "the blocking notification primitive",
so the consumer is already annotated with the thing it needs.

The work. Convert the shell's watch loop to whatever milestone 106 (a wait that ends on either
the interrupt or the deadline) settles on, retire the poll, and keep the escalation policy exactly
where it is. `grant_plan::Escalation` is host-tested and does not change: the timing source changes,
not the decision. The acceptance evidence is the one thing the poll cannot give, a foreground job
that runs for a second with the shell consuming no scheduler turns, plus the existing `^C` tests
passing unchanged on both ISAs.

<!-- writing-standards: exception. Marked 2026-09-26 (UTC) by the lane for milestone 106 (a wait
that ends on either the interrupt or the deadline). Reason: this change touches the block only to
retire its gate, which 106 met. Bringing the block to 4 bold spans per 1,000 words is a rewrite for
whoever builds 103, not something to hide inside a gate change. Remove this marker then. -->

## Scope note

Strictly downstream of milestone 106. There is no timed wait anywhere in the kernel (milestone
51 (wall-clock time) records the fork and the three candidate shapes), and this milestone adds no
kernel surface of its own. If 106 lands the deadline on `Rendezvous::RECEIVE`/`CALL`, this is a small
change in one file.

**Not `ThreadControlBlock::SUSPEND`.** Pausing a job resumably is `SUSPEND` and `RESUME`, from §300 (the paused state, a TCB verb, silent), which milestone 48
(job control) uses for `fg`, `bg` and a stopped state. The two are adjacent
and separate: 48 adds a state a job can be in, this one changes how the shell waits.

**Not the shared-flag design.** §24 chose control by shared memory over control by message, with its
reasons recorded (a running computation cannot watch an endpoint). A notification primitive does not
reopen that; it removes the spin from the *shell's* side, where the shell is not the party doing the
work.

## Index row

Raised as "decided and never built" from a handoff note whose other items were struck when they
landed; `^C` shipped, both tiers, both ISAs (§24's built amendment, `OPERATION_INTRCOUNT`, `jobframe`, `Escalation`). What is unbuilt is §24's own named interim: the shell busy-polls for the whole life
of every foreground job because there is nothing to block on. Downstream of 106
