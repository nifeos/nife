# The process view: the appendices

[`notes/process-view.md`](../process-view.md) is the page to read. It explains the view `ps`,
`pgrep` and `top` read, what each program is and what it costs. These files hold what the page does
not:

- [What is left](remaining-procps-programs.md): the forks the rest of `procps` waited on, with the seven
  questions answered, and which of them have since been ruled.
- [The machine and your share](the-machine-and-your-share.md): how `free`, `vmstat` and `slabtop`
  were built under DECISIONS §225 (`free` sees the machine and your share), and the method number,
  page layout and names calef ratified.
- [`pmap` from the prompt](pmap.md): what the listing costs, and the options calef chooses between.
- [The `SURVEY` method](survey-design.md): the selector, placement, membership and the cursor.
- [`pgrep`](pgrep.md): the filter, its four answers, and the `pkill` that cannot exist.
- [`pmap` and `LIST`](address-space-enumeration.md): the method, the delegation audit, and why no live view can be
  handed over today.
- [`watch`](watch.md): built, then cut, and the test it leaves behind.
- [The CPU-time tests](cpu-time-tests.md): why `cpu_time_tests` compares a spinner with a sleeper.
- [The BUGS list in full](limitations.md): the parent's BUGS section, one line per entry there.

*Name: provisional, for the directory and the stems `what-is-left` and
`the-machine-and-your-share`. Naming is calef's; `script/names --unratified` lists each stem.*

Each stem below is named for its content, and calef names files.

- `survey-design` Name: ratified 2026-10-18 (calef, "survey-design.md") Refused `survey-method`.
- `pgrep` Name: provisional. For the program it describes.
- `address-space-enumeration` Name: ratified 2026-10-18 (calef, "Approve as a group"). Refused `pmap-listing`.
- `watch` Name: provisional. For the program it records.
- `limitations` Name: provisional.
- `remaining-procps-programs` Name: ratified 2026-10-18 (calef, "Approve as a group"). Refused `what-is-left`.
