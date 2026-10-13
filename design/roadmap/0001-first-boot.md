---
status: BUILT
raised: 2026-07-12
built: 2026-07-12
---
# 1. Boot to Rust on QEMU `virt`, and print to the PL011 UART

This block is a record of what the commits show, not a reconstruction of what anyone remembers.

The original plan is the first commit, `b7f10e7` (2026-07-12), whose `## Milestones` table in
DECISIONS.md carried this as "Boot to Rust on QEMU `virt`, print to UART. What it teaches:
freestanding binaries, linker scripts." The outcome matches the plan exactly.

What landed, all on 2026-07-12:

- `1ca902a` set up the cargo workspace, the pinned nightly toolchain, and the QEMU runner.
- `a7d4821` added xtask for build, run, test, gdb, and objdump.
- `78efc3e` booted to Rust on QEMU `virt` and printed to the PL011 UART: `boot.s`, the linker
  script, `.bss` zeroed by hand, the stack pointer set before the first `bl`.
- `f703caa` added the QEMU test harness and the four boot invariant tests, "set up on day one on
  purpose; the alternative is debugging by println! for a year." The four tests are still the
  model CLAUDE.md points at: the harness runs, `.bss` was zeroed, `sp` is 16-byte aligned, we are
  at the exception level we think we are.

An honest gap in the record: these commits predate the "Milestone N:" title convention, which
starts at `543d390` (milestone 2), so no commit names milestone 1. The boot commit and the plan
row are the evidence.

## Follow-on

- **Recorded.** `design/roadmap/0001-first-boot.md` carries the one gap this block found: no commit
  names milestone 1, because these commits predate the "Milestone N:" title convention that starts
  at `543d390`. The boot commit and the plan row are the evidence, and nothing can be done about it
  now without rewriting history.

## Index row

the first instruction; freestanding Rust, the linker script, and a test harness on day one
