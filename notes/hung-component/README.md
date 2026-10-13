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
  cannot answer for the component, and what the test shows.
- [`the-wedge-test.md`](the-wedge-test.md): the four results of the `ROLE_HUNG` case and why its
  wedge is a `CALL`.
- [`milestone-23-residuals.md`](milestone-23-residuals.md): what the lane found about the two other
  residuals of milestone 23 (a capability-routed component OS with live replacement).
- [`examples.md`](examples.md): the Rust examples, a watcher's `SURVEY` loop, its one-right
  endowment, and restoring a service under a hung component.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

Each stem below is named for its content, and calef names files.

- `the-armed-kill` Name: provisional.
- `liveness-watch` Name: provisional. Named after the provisional program name the note proposes.
- `what-counts-as-hung` Name: provisional. Named after the parent's question 2.
- `stranded-caller` Name: provisional.
- `the-wedge-test` Name: provisional.
- `milestone-23-residuals` Name: provisional. Named after the parent's own heading.
- `examples` Name: provisional. For the parent's Rust EXAMPLES. calef names files.
