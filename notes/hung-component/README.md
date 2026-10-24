# The hung component: the appendices to notes/hung-component.md

[`notes/hung-component.md`](../hung-component.md) carries the problem, the three hang shapes, the
answers to the four questions, the case as built, EXAMPLES and BUGS. These files carry the argument
and evidence behind those answers, which its prose budget has no room for.

- [`the-armed-kill.md`](the-armed-kill.md): why case (c) could not be torn down when the note was
  written, in `reap_region_objects`'s words and step by step.
- [`liveness-watch.md`](liveness-watch.md): why a watcher that holds only `ENUMERATE` beats a
  watchdog that can kill, and the provisional name `liveness_watch`.
- [`what-counts-as-hung.md`](what-counts-as-hung.md): question 2 argued owner by owner, the progress
  counter, its two costs, what is missing, and a citation corrected on the way.
- [`stranded-caller.md`](stranded-caller.md): question 3's path through the code, why the operator
  cannot answer for the component, and how destroying the component answers the caller `Gone`.
- [`the-wedge-test.md`](the-wedge-test.md): the six results of the `ROLE_HUNG` case, why its
  wedge is silent, and why the quiesce has a stand-in.
- [`what-is-left-of-milestone-23.md`](what-is-left-of-milestone-23.md): what the lane found about the two other
  residuals of milestone 23 (a capability-routed component OS with live replacement).
- [`examples.md`](examples.md): the Rust examples, a watcher's `SURVEY` loop, its one-right
  endowment, and restoring a service under a hung component.
- [`terminal-supervisor-bound.md`](terminal-supervisor-bound.md): how `terminal_supervisor`'s
  quiesce bound was measured, and why its instances and stand-ins are built in bays.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

Each stem below is named for its content, and calef names files.

- `the-armed-kill` Name: provisional.
- `liveness-watch` Name: provisional. Named after the provisional program name the note proposes.
- `what-counts-as-hung` Name: provisional. Named after the parent's question 2.
- `stranded-caller` Name: provisional.
- `the-wedge-test` Name: provisional.
- `what-is-left-of-milestone-23` Name: ratified 2026-10-18 (calef: "Approve both").
- `examples` Name: provisional. For the parent's Rust EXAMPLES. calef names files.
- `terminal-supervisor-bound` Name: provisional. After the program and the bound it measures.
Each stem here is named for its content, and calef names files.
