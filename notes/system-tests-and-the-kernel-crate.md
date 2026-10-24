# The system tests and the kernel crate

*Milestone 609 (the system tests leave the kernel crate). Measured 2026-09-27 (UTC) against
`dcf0735a5`. Name provisional.*

The kernel crate is three things at once: the kernel, the integrator that brings the base services
up, and the harness for the whole-system test suite. calef ruled on 2026-09-27 (UTC) that the system
tests leave it, so that a kernel can be built and released without building the system, and a
base-service change does not need a kernel change. This note records what couples them, what this
milestone has already cut, and the one question that is calef's.

## What the kernel linked, and why

Pull request #1389's packages note counted twelve edges from the kernel into services and fixtures.
Reading each use site gives three kinds, and a fourth turned up when the new gate first ran.

| crate | used by | now |
|---|---|---|
| `ps`, `pgrep`, `pmap` | `survey_tests`, `pmap_tests` only | dev-dependency |
| `network_time_protocol` | `ntp_tests` only (the service file names it in a comment) | dev-dependency |
| `package_archive` | `tests.rs` and `riscv_virtio_tests.rs` only; not one of the twelve, found by the gate | dev-dependency |
| `coremark` | `bench.rs` (feature `bench`) and one suite test | optional behind `bench`, and a dev-dependency |
| `job_mix` | `job_mix.rs` (feature `job_mix`) | optional behind `job_mix` |
| `soak_page` | `soak.rs` (`soak_test`), `ipc_stack_depth.rs` (`ipc_stack_depth`, and tests) | optional behind both, and a dev-dependency |
| `calendar` | `arch/x86_64/rtc.rs`: the RTC's civil date to Unix seconds | still linked |
| `jh7110_entropy` | `memory.rs`: find the TRNG in the device tree at boot | still linked |
| `video_terminal` | the tour's display bring-up reads `status::TERM_UP` and the grid size | still linked |
| `block_roster` | `disk_service` writes the roster page a program reads | still linked |
| `non_volatile_memory_express` | the kernel's own NVMe module, dead outside tests | still linked |

So `cargo build -p kernel` now compiles eight fewer crates on all three targets. `cargo tree -p
kernel -e normal,build` lists five of the twelve, and all five are the production uses in the last
rows.

The premise needed one correction. "No kernel build can happen without building the whole
system" is true, but not because of these edges. All twelve are small pure-logic libraries with no
dependencies of their own; none of them builds a program. What does build the system first is
`kernel/build.rs`, which compiles the progenitor's digest into `TRUST_ROOT` and so needs the archive
packed before the kernel links. calef's same-day ruling moves that trust root out of the kernel
binary, as a separate milestone. The initrd itself is loaded at run time (`-initrd`), not embedded.

The five that remain are a division question, not a test question. Each is sans-IO logic the
kernel uses for real, the way it uses `device_tree_blob`, which the packages note already calls a
leaf library rather than the kernel. Whether `calendar` is a service, a contract or a leaf library
is #1389's classification to settle; the answer decides whether each one stays, moves, or has its
shared part (for `video_terminal`, the `status` codes) split into a contract. This milestone does not
guess at it.

## The gate

`script/lint` check 13 (number provisional) reads `cargo metadata` and fails when a non-optional
kernel dependency is named only by test files (`*_tests.rs`, `tests.rs`), named by no source file,
or is one of the three fixture crates. It is a property rather than a list, because the kernel gains
about thirty dependencies a month and a list would fire on every legitimate one. Planting `ps` back
into `[dependencies]` fails it. Making a fixture non-optional fails earlier, in Cargo, because a
`dep:` feature cannot name a non-optional dependency. Its blind spots are written beside it.

## What the tests are, measured

`kernel/src/user/` is 82 files. By the attribute on each module's declaration in `user.rs`:

| kind | files | lines |
|---|---|---|
| `#[cfg(test)]` only: the system tests and the services only they spawn | 61 | 20,584 |
| compiled into every kernel, `allow(dead_code)` outside tests (bring-up) | 17 | 6,724 |
| production: console, entropy, install, the x86 program table | 4 | 1,480 |
| `user.rs` itself, mixed | 1 | 4,009 |

The kernel has 483 `#[test_case]`s: 278 in `user/`, 205 beside the kernel code they test.

The 61 test-only files name 147 distinct kernel items, 1,414 times (`sched` 552, `testing` 203,
`cap` 164, `memory_region` 151). 109 of those items are already `pub`, two are `pub(crate)` and one
is private; 35 resolve through re-exports and arch facades. Two exist only under `cfg(test)`:
`sched::rendezvous_waiting_senders` and `testing::skip`. The kernel's 35 top-level modules are all
private `mod`s of a binary crate.

## The fork: where the tests go

*Decided 2026-09-27T06:40Z: calef chose A, below. What was built is in the next section; the
options are kept as they were put to him.*


This changes what `script/test` runs and needs a crate name that sticks, so it is calef's.

**A. A system-test image crate** (provisional name `system_tests`, beside `kernel/`). The kernel
becomes a library plus a thin binary. The new crate is a second `no_main` binary that links the
kernel library, the service and fixture crates, and the 61 test-only files, moved unchanged so git
tracks them, with `crate::` rewritten to `kernel::`. The kernel's own 205 tests stay where they are.
The cost follows from the counts above. The 35 modules become `pub mod` (probably
`#[doc(hidden)]`) and three items widen. The two `cfg(test)` items move behind a kernel feature,
because `cfg(test)` is not set on a dependency. The new crate needs its own `build.rs` to pass the
linker script, since `rustc-link-arg` does not reach a dependent. Every arch leg in `xtask` runs two binaries instead of
one; the aarch64 suite spends about 53 s in its tests and a boot is a few seconds of that. Two foot
guns, to be written where a reader meets them. The kernel's public surface becomes an internal API
the tests pin. And a test-hook feature can be unified into the kernel binary by a `cargo build
--workspace` that selects both crates. `xtask` builds `-p kernel`, so the release path is safe, but
nothing gates it.

**B. Stop at the edges.** What this milestone has already built: the tests stay in the kernel crate
under `cfg(test)`, every crate only they use is a dev-dependency, and check 13 holds it. The shipped
kernel no longer compiles test-only code. But a base-service change that moves its test still edits
`kernel/`, and if packages are versioned per crate, that is a kernel version for a change the kernel
binary never sees. It does not meet the ruling.

**C. The tests become userspace programs.** This is seL4's shape: sel4test is a separate project
whose `sel4test-driver` is the root task and runs each test in its own process, reporting over IPC
(read, docs.sel4.systems). Hubris keeps a `test/` tree with `test-runner` and `test-suite` tasks and
per-board test apps (directory listing read; that it builds them as separate images from the same
kernel is recalled, not read). It proves the syscall surface rather than kernel internals, which is
the better test. It loses here because 1,414 references into `sched`, `testing` and `memory_region`
observe state no syscall exposes. Porting means rewriting 20,000 lines and either dropping those
observations or widening the syscall surface (§10 (process model: capability-based, microkernel)) to reach them. Right for new tests, wrong as the
move.

**D. A feature on the kernel instead of dev-dependencies**, `system_tests = [ "dep:ps", ... ]`.
Refused: a feature can be switched on in a release build by accident and a dev-dependency cannot,
and Cargo already has the mechanism for exactly this.

**Recommendation: A**, with B (built) as its first step. It is the only option that meets the
ruling, and it is mostly moves: the files go unchanged and the rewrite is mechanical. Would it still
win at equal cost to B? Yes; B's advantage is only that it is done. It is reversible (code and a
provisional crate name), and nobody outside the tree has acted on the kernel being a binary. The
6,724 lines of bring-up stay in the kernel under A. They are the integrator, and leave with the
trust-root and progenitor work, not with the tests.

## What was built (option A)

Seven commits on `milestone/609-system-tests-crate`, each with one purpose.

1. The kernel became a library plus a thin binary. `src/main.rs` is a link line; `_start` is still
   the library's, and the linked kernel has the same symbols at the same entry address.
2. The 66 `cfg(test)`-only files moved to `system_tests/src/user/` byte for byte, in their own
   commit, so `git blame` follows every line. That is 61 plus five that landed on `main` while
   this was built (`notification_tests`, `timer_tests`, `scratch_window_tests`,
   `terminal_quiesce_tests`, `terminal_swap_tests`): 22,824 lines and 306 `#[test_case]`s.
3. The wiring. A kernel `system_tests` feature makes the boot a test boot and calls
   `system_tests_main`, which the new crate defines. The kernel's modules stay private; under the
   feature only, `system_test_access` re-exports the fifteen the suite names, and the new crate
   glob-imports it so the moved files' `crate::sched::...` paths resolve unchanged. Twelve items
   widened to `pub`. Items that only one of the two suites calls say which, per item, because §38
   (a suppression is scoped to an item and carries a reason) forbids a blanket.
4. The fourteen falsification records whose tests moved went with them.
5. `script/test` runs both images on each leg, and so do the HVF leg and `uefi-test`. Under
   `--test` the selection is counted across the two images. The host passes, clippy, drift, both
   stack checks, rustdoc and the falsification sweep all learned about the second image.
6. Living path references were repointed.
7. The citation ratchet stopped counting a line moved verbatim as a new citation.

The kernel binary built together with `system_tests` fails to link, on purpose: the feature would
otherwise turn a shippable kernel into one that runs tests instead of booting.

## The rule for new tests, and the migration backlog

calef's ruling carries two rules beyond the move. A new service test is a userspace program unless
it has to observe kernel internals, and a test that does says which internals and why; this is
written where `script/test` is documented, in [scripts.md](scripts.md). And existing tests that
touch no kernel internals migrate to userspace over time.

That backlog is small, measured over the 66 files on 2026-09-27 (UTC):

| cut | files | `#[test_case]`s |
|---|---|---|
| names none of `sched`, `testing`, `memory_region` | 2 (`pipeline_tests`, `measured_boot_tests`) | 10 |
| names no kernel module but the test harness (`testing`, for `skip!`) | 3 (`language_tests`, `pipeline_tests`, `redirection_tests`) | 17 |
| names no kernel module at all | 1 (`pipeline_tests`) | 5 |

`measured_boot_tests` avoids the three named modules but reads `cap` and `trust`, so it observes
internals after all. The honest backlog is the three harness-only files: the shell language,
pipeline and redirection tests, which drive `swish` and read what it printed. Every other file
observes kernel state no syscall exposes, which is the case rule 2 allows.

## What each test cost, and its budget

`crates/test_times` holds the grammar of the `time per test` block the kernel's runner prints and
`xtask` reads. The console is the record's channel because it is the only one aarch64, riscv64 and
x86_64 share. x86_64 exits through `isa-debug-exit`, which has no file I/O, so semihosting a file
out would break §19 (architectural parity is a tenet).

The block's heading is not in §254 (a gate prints what each item cost)'s wording, which rules on the
line. It is there so a reader can tell "this image printed no record" from "this image ran no
tests", and so a test whose own output happens to begin with `time ` is never read as a record line.
The figures are milliseconds because whole seconds round most of this suite to zero (§254). A `u64`
of milliseconds outlasts any suite.

Every test gets `DEFAULT_BUDGET_SECS`, 90 s, unless it declares a longer one in `SLOW_TESTS`, with
its reason. A budget is roughly twice the measured time: enough headroom that host load or a debug
build does not produce a flaky failure, and tight enough to still catch a hang. A single global limit
large enough for every test would protect nothing. `SLOW_TESTS` has been empty since 2026-10-11
(UTC). Its one entry gave `std_net_runs_over_the_socket_contract` 700 s for a measured 300 to 344 s.
That test now takes 1.3 to 2.5 s, and the slowest test in about 27,000 per-test records takes
20.5 s.

## BUGS

- Five service crates are still kernel dependencies, pending #1389's division of them.
- Check 13 counts a crate used only inside an inline `#[cfg(test)] mod` in a non-test file as a
  production use, and does not skip a crate name that appears inside a string.
- Dead code is checked per item in both test images, but an item's `allow` names only which suite
  calls it on the architecture that reported it. An x86_64 test build is not gated, so its
  allowances were added only where a gated build found them.
- Five notes still say `kernel/src/main.rs` for code that is now in `lib.rs`: board-console.md,
  boot-ladder.md, nifefs.md, pipes/second-stream.md and visionfive2.md. A new `main.rs` keeps the
  old name, so git does not call the move a rename, and the prose ratchet would hold each note to
  today's bold limit for a one-word repoint. Code and data pointers were repointed.
