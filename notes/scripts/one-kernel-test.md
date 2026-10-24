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

## A filter across the two test images

Each leg boots two test images, the kernel's own and `system_tests`', so a filter naming a system
test selects nothing in the kernel's image. `cargo xtask test --test` sets
`NIFE_TEST_FILTER_ACROSS_IMAGES` (provisional), and the harness then leaves the count to xtask,
which checks the sum over both images. Left unset, as for a bare `cargo test`, an empty selection
still fails. The 2.3 s relink above buys back the 53 s of suite a filtered run skips, and
`rerun-if-env-changed` is what makes a changed filter rebuild at all.

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

## Repeating it (`--repeat N`)

`--repeat N` runs the filtered tests N times in one boot, for a failure seen once in many boots
(milestone 321 (a corpse that did not park, on four real cores)). It needs `--test`, and reaches
the kernel as `NIFE_TEST_REPEAT` (provisional), the filter's compile-time channel.
[`kernel/src/testing.rs`](../../kernel/src/testing.rs)'s `TEST_REPEAT` holds the semantics.

```
$ NIFE_SMP=4 script/test --arch x86_64 --test a_dead_child_is_still_in_the_domain_until_it_is_reaped --repeat 200
repeat: each selected test runs 200 times (NIFE_TEST_REPEAT)
repeat run 1 of 200: 59022 frames free, longest free run 53165, 0 of 64 capability slots held, 5 threads
test system_tests::user::survey_tests::a_dead_child_is_still_in_the_domain_until_it_is_reaped ...
  user thread 4294967301 killed: vector 14 (page fault)
...
ok
repeat run 2 of 200: 59022 frames free (+0 since the last run), longest free run 53165, ...
...
repeat result: ok. 200 runs, 200 passed, no failure; 59022 frames free after the last run, 0 capability slots held
```

The first failure ends the boot with its own message and dump, preceded by
`repeat result: FAILED. K runs of N, K-1 passed, first failure at run K`. A test that loses frames
or capability slots each run is refused (`REPEAT REFUSED`) as soon as the loss projects past what
is left, so running dry cannot read as the test failing. For a bench stick, `cargo xtask uefi-test
--test <name> --repeat N --stage-only` builds the `system_tests` image into `target/esp-test` and
boots nothing.

### BUGS

- The leak check covers frames and the runner's capability slots. A test leaking a thread or a
  rendezvous per run fails in whichever run exhausts the table; the per-run thread count shows the
  trend, and nothing refuses it.
- The per-test time record holds the sum over every run, so a repeated run's record is not a
  per-test cost.
