---
status: BUILT
raised: 2026-08-31
built: 2026-08-31
---
# 210. No kernel test can be run by name, so one falsification costs a whole suite

Built. `cargo xtask test --test <substring>` filters the kernel suite; the filter is baked into the
test binary by `kernel/build.rs` and read by `kernel/src/testing.rs`'s runner. See notes/scripts.md.

The boot was measured and the premise below is wrong, in the direction that makes this worth
more rather than less. QEMU start to `running 312 tests` is 0.50 s; the 312 tests are 53.1
s. Boot is about 1% of the QEMU leg, not "most of the four minutes". A warm filtered run of one
test is 8.6 s end to end, and what remains is fixture building (the archive, the `std` exerciser,
five disk images), not the boot.

In brief. `kernel/src/testing.rs`'s runner takes no filter, `cargo xtask test` parses only
`--arch`, `--cpu` and `--hvf`, and arguments after `--` go to QEMU. So there is no way to run one
kernel test.

Every host crate has this for free: `cargo test <name>` and `cargo kani --harness <name> --exact`
both work, which is why DECISIONS §134's falsification sweep is thirty seconds over twenty-five
records. **The kernel half of that table is not affordable at all**. One falsification costs a full
suite run of about four minutes, and milestone 202 measured 234 of those seconds as a single hung
test whose break manifested as a watchdog timeout.

## Why it is worth its own block

It is not only 202's problem. Every lane that has waited out a whole suite to see one test has paid
this, and the tree records the shape without naming the cause. The load-sensitive assertions, the
scanout checks, the two-core crash hunt, and this session's own `ci-build` runs that failed on timing
under load and told us nothing about the change under test.

It also gates something specific. `kernel/falsifications/` exists on `main` as of milestone 202 and
nothing sweeps it, because a sweep that runs the whole suite per record is not a sweep. Six
kernel confinement claims have no mechanism for that reason.

## What it needs

A filter reaching `kernel/src/testing.rs`'s runner from the command line, and an `xtask` argument to
carry it. The runner already has the test names; nothing has to be discovered, only passed through.

The interesting constraint is the boot. A kernel test is not a function a harness calls; it runs
inside a booted kernel under QEMU, so "run one test" means "boot, run one, exit", and the boot is
most of the four minutes. A filter makes the *selection* cheap and does not by itself make the *run*
cheap, which is worth knowing before anybody promises a fast inner loop.

## BUGS

- The boot costs 0.50 s against 53.1 s of tests. What bounds a filtered run instead is fixture
  building, which nothing here filters and nothing records which test needs.
- A filtered run cannot fail the whole-suite instruments: the frame ledger's kept-frames
  ceiling, the thread peak and the stack high-water are totals over 312 tests. And tests are not
  independent, so one that only passes after an earlier test wired a service fails alone. Both are
  in notes/scripts.md's BUGS beside the flag.
- `--test` selects tests and never architectures, so all three legs still run and `--arch` remains
  the explicit narrowing. The cost is that filtering an architecture-specific test without `--arch`
  fails on the other two legs, which the failure message names.
- **`kernel/falsifications/` is a provisional path** that milestone 202 created and nothing reads.
  Whoever builds the sweep decides whether it stays there or moves. **Not built here**, on purpose:
  the runner filter is the missing capability and it now exists, but a kernel sweep still needs
  three things this lane did not touch, and each is a decision rather than plumbing. The
  `Falsification:` comment convention (§134) has to extend from `#[kani::proof]` to `#[test_case]`,
  which is where the annotation would live. `script/falsifications` has to learn a second replay
  verb: today `--sweep` runs `cargo kani --harness <name> --exact`, and a kernel record needs
  `cargo xtask test --arch <a> --test <name>` instead, per architecture. And the patch path has to
  be decided against §134's per-crate rule, which `kernel/falsifications/` already reads as a
  refusal of.

## Follow-on

- **Recorded.** In `notes/scripts.md`'s BUGS section, beside the flag: a filtered run cannot fail
  the whole-suite instruments, because the frame ledger's kept-frames ceiling, the thread peak and
  the stack high-water are totals over 312 tests. Tests are also not independent, so one that only
  passes after an earlier test wired a service fails alone.
- **Recorded.** In `design/roadmap/0210-run-one-kernel-test.md`, which carries the measurement: what
  bounds a filtered run is fixture building rather than the boot, which is the archive, the standard
  library exerciser and five disk images. Nothing filters those and nothing records which test needs
  which, so a one-test run is 8.6 seconds rather than the sub-second the filter suggests.
- **Recorded.** In `notes/scripts.md`, beside the flag: the filter selects tests and never
  architectures, so filtering an architecture-specific test without an architecture fails on the
  other two legs. The failure message names the cause.
- **Milestone 323.** Build the sweep that
  replays kernel falsification records. Three pieces, each a decision rather than plumbing. Extend
  §134's `Falsification:` comment convention from `#[kani::proof]` to `#[test_case]`, teach
  `script/falsifications` a second replay verb (`cargo xtask test --arch <a> --test <name>` per
  architecture, beside today's `cargo kani --harness <name> --exact`), and settle where kernel
  records live against §134's per-crate rule, which `kernel/falsifications/` currently reads as a
  refusal of. Milestone 212 handed this back here, and six kernel confinement claims from milestone
  202 have no replay mechanism until it is done.

## Index row

Minted from milestone 202's lane, which paid the cost twenty-five times. `kernel/src/testing.rs`'s
runner takes no filter and `cargo xtask test` parses only `--arch`, `--cpu` and `--hvf`, so one
kernel test costs a full suite run of about four minutes. Every host crate has this for free. It
also gates a real thing: `kernel/falsifications/` exists on `main` and nothing sweeps it,
because a sweep that runs the whole suite per record is not a sweep.
