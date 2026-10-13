# A real hardware entropy source: the appendices to milestone 159

[Milestone 159 (a real hardware entropy source: the JH7110's TRNG)](../0159-jh7110-trng-driver.md)
carries where the driver stands, what was built, the run that served, what it establishes for fatal
risk 6, what is still open and its `BUGS`. This directory carries the history and the bench working
its prose budget has no room for.

- [`wiring.md`](wiring.md): what the first two lanes built, 2026-08-24 and 2026-09-01, and why the
  status did not move after either.
- [`bench-procedure.md`](bench-procedure.md): the ordered bench procedure and the table of what
  each `hw entropy` line means.
- [`bench-runs.md`](bench-runs.md): the first two bench runs of 2026-09-04, the readiness defect
  they found and its fix, and the tour's unreachable success line.
- [`register-sequence.md`](register-sequence.md): the third lane's bring-up changes and their
  sources.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default. calef names
directories.*

- `wiring` Name: provisional. For the parent's "What the second lane built" and its account of why
  the status did not move. calef names files.
- `bench-procedure` Name: provisional. After the parent's "The bench procedure, in order". calef
  names files.
- `bench-runs` Name: provisional. For the parent's "The bench ran it" and "The second bench run".
  calef names files.
- `register-sequence` Name: provisional. For the parent's "What the third lane changed". calef names
  files.
