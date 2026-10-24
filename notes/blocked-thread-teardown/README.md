# Ending a blocked thread: the appendices to notes/blocked-thread-teardown.md

[`notes/blocked-thread-teardown.md`](../blocked-thread-teardown.md) carries the problem, what was
decided and built, the stale reply capability hazard, the survey table, the authorization question
in brief, the four proposals side by side, EXAMPLES and BUGS. These files carry the research its
prose budget has no room for.

- [`what-the-machine-already-has.md`](what-the-machine-already-has.md): the three places a blocked
  thread can be, the abort pair that already existed, the one missing primitive, and the precedent
  for reaching outside the region.
- [`prior-art.md`](prior-art.md): seL4, Mach, L4Re, Zircon, QNX Neutrino, Linux and Windows NT, each
  with the source quoted.
- [`mapping-and-authority.md`](mapping-and-authority.md): what each shape would break in this
  kernel, and the four candidate authorities to end a blocked thread.
- [`the-four-proposals.md`](the-four-proposals.md): proposals A to D in full, and the questions the
  note left open.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

Each stem here is named for its content, and calef names files.
