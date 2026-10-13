---
status: BUILT
raised: 2026-07-13
built: 2026-07-14
---
# 10. A shell at EL0, and processes spawned on command

Built in `61ed8c2` (2026-07-14). The original table called this rung "proof the whole stack
works," and the commit claims exactly that: four processes and the channels between them, an input driver
owning UART RX, the shell, a console server, and a worker spawned on demand through a process
service. "Everything the user sees is a conversation between processes, and the kernel is a message
router that touches none of it."

The revised row (`491f23d`) reads "A process server, and a shell that spawns binaries," which
absorbed the original milestone 9 ("Processes: spawn, exit, wait") when the ladder was rewritten
around §10; the spawn half of that plan lives here, and teardown beyond `exit` was deliberately
left for later (revocation is milestone 13's subject and reaping a whole process is milestone
26's).

The shell itself was three commands (`help`, `echo`, `run <n>`). Interactive niceties began the
next day (`3f0de79` boots straight to a shell, `588c206` echoes keystrokes), and everything the
shell has since become is milestones 31, 47, and 67's story.

## Follow-on

- **Milestone 13.** Teardown beyond `exit` was deliberately left out here, and revocation is where
  it went: this block says so in its own second paragraph. Spawning a process was this milestone's
  half of the old milestone 9; taking one apart safely was not.
- **Milestone 26.** Reaping a whole process, the other half of the teardown this block deferred. A
  thread that calls `exit` was enough for a shell that runs a worker on command; an owner that wants
  the memory back was not.
- **Milestone 31.** The shell itself stopped at three commands (`help`, `echo`, `run <n>`), and the
  capability shell is where it became a program that hands out authority rather than a demo of one.
- **Milestone 47.** Navigation and naming, which is what the three-command shell had none of:
  nothing here could say where it was or what it was holding.
- **Milestone 67.** The language the shell speaks, which this block leaves as the interactive
  niceties that started the next day (echoing keystrokes) rather than a grammar anyone designed.

## Index row

proof the whole stack works: every keystroke is a conversation between processes
