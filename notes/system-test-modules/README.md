# The system-test modules: the appendices to notes/system-test-modules.md

[`notes/system-test-modules.md`](../system-test-modules.md) carries what the suite's modules prove
and why each is shaped as it is. These files carry the same for single test files whose design does
not fit a doc comment.

- [`address-space-tests.md`](address-space-tests.md): ASID tagging, the cross-core shootdown, a
  space installed across a tick, and exact frame accounting.
- [`compositor-tests.md`](compositor-tests.md): the display stand-in, claim 25 part by part, input
  and focus, and the four witnesses to one scanout.
- [`confined-fuzzer-tests.md`](confined-fuzzer-tests.md): the conductor's oracles, how it serves a
  blocked call, and what it cannot reach.

*Name: provisional. The directory follows the siting default of §212 (a prose budget), `notes/X/`
beside `notes/X.md`. calef names directories.*

Each stem here is named for the test file it explains, and calef names files.
