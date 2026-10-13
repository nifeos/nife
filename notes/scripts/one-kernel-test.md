# Running one kernel test: how the filter works

*An appendix to [`notes/scripts.md`](../scripts.md), the front door to `script/`. It holds how
`script/test --test` reaches the kernel and what it switches off. A reader who only needs to run a
command should not have to open it. The directory `notes/scripts/` and this file's stem are
provisional names; naming is calef's.*

## How the filter reaches the kernel, and why it is compile-time

`kernel/build.rs` bakes `NIFE_TEST_FILTER` into the test binary as a `rustc-env`, and `runner`
reads it as a `const`. That is not the obvious design (a boot argument is), and the reason is
parity: a runtime channel means the boot protocol, and there are three of them. aarch64 and riscv64
arrive with a device tree whose `/chosen/bootargs` this kernel does not parse; x86_64 arrives
through PVH with no device tree at all. One `env!` is identical on all three and needs no parsing.
The price is a kernel relink, measured at about 2.3 s, when the filter *changes*.

## What the flag turns off, and why

A filtered run is not the suite, so three things that assert what unselected tests would have
written are suppressed rather than allowed to fail for an unrelated reason:

- the host-logic crates do not run at all (they already have `cargo test <name>`, and running
  their 72 s to reach one kernel test would keep most of the cost the flag removes);
- the post-run RedoxFS, crash and blank image checks are skipped, the same guard `--arch x86_64`
  already has. They would open a stale image from a previous run and report a true fact about a
  leftover file as a false one about this run;
- the scanout and inbound referees still run (the scanout referee is also what
  presses keys over QEMU's monitor, which the keyboard test needs) but their verdicts become
  advisory, and the run says so on a line of its own.

## BUGS

- The last item above is out of date. `filtered_leg` in `xtask/src/suite.rs` starts neither
  referee, so a filter that selects `a_host_process_connects_to_the_guest_and_is_answered` hangs
  until the 60 s watchdog: no host process ever connects. Seen 2026-10-13 (UTC) on riscv64 with
  `--test riscv_virtio_tests`, twice, with and without that day's `while_installed` change.
