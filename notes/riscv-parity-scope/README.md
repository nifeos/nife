# RISC-V parity: the appendices to notes/riscv-parity-scope.md

[`notes/riscv-parity-scope.md`](../riscv-parity-scope.md) carries each parity workstream as built,
the gaps in the shared suite, and the post-parity correction. These files carry the records its
prose budget has no room for.

- [`user-tests-port.md`](user-tests-port.md): how `kernel::sched::tests` and `kernel::user::tests`
  came to run on both ISAs, the defects that surfaced, and what stays aarch64-only.
- [`suite-gaps.md`](suite-gaps.md): the ASID gap, the 24-test overlap still open, the leak police,
  and two test failures first misread as host load.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/`
beside `notes/X.md`. calef names directories.*

Each stem here is named for its content, and calef names files.
