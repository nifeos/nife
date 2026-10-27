---
status: PARTIAL
promoted_from: measure-kernel-and-service-coverage-under-qemu
raised: 2026-10-04
milestone_dependencies: none
decision_dependencies: 46
machine_requirements: none
specific_machine: none
needs_person: no
---
# 781. Measure kernel and service coverage under QEMU

Raised 2026-10-04 (UTC) by the lane for milestone 745 (count the error paths no test reaches),
provisional. *(Title and slug are drafts.)*

## Why

`script/coverage` measures host crates only. The kernel, the services and the syscall-wrapping
crates hold 1,159 Result-family error paths and 602 Option-family ones that no coverage run sees,
about half of the tree's total ([`notes/untested-error-paths.md`](../../notes/untested-error-paths.md)).
Every cleanup path the count found is among them.

## Ruling

Ruled 2026-10-27 (UTC) by the architect. Build the test kernel, and then the services, with `-C
instrument-coverage` and a `no_std` profiler runtime that only test builds depend on. Dump the
counters at QEMU exit. The report uses the same regions as the host `script/coverage` report, so
`helpers/error_paths.py` reads it unchanged. The alternative is a QEMU TCG plugin that records executed blocks, and it is not built. Blocks give
lines and not regions. A `?` error edge needs a search by address. User programs that share virtual
addresses need telling apart by address space.

The runtime is a dependency under §46 (thin primitives or whole subsystems), so it carries a
dependency audit and the kernel's bar. [`notes/kernel-coverage.md`](../../notes/kernel-coverage.md)
holds both, the numbers and the method.

## What the first pull request builds

The kernel's own unit-test image on aarch64:

- `cargo xtask coverage` builds it with `-C instrument-coverage` in a target directory of its own,
  behind the kernel's `coverage` feature, which is the only thing that links `minicov`.
- The image writes its counters to a file on the host through ARM semihosting as it exits, on the
  failing paths as well as the passing one.
- The same command renders an HTML report and an lcov file with `llvm-cov`, and runs
  `helpers/error_paths.py` on the HTML.

## What remains

Each is its own pull request:

1. The whole-system image (`system_tests/`), which boots the kernel as a library with the services
   running, merged with the unit-test profile.
2. The services' own programs at EL0, each with its own dump, since no kernel semihosting call
   reaches a user address space.
3. riscv64 and x86_64: the same feature, with each architecture's exit and file channel.
4. A scheduled run in the Pacific-night window (about 08:00 to 13:00 UTC), after a report on what
   one run costs. No recurring job exists today.

## Follow-on

- **Outstanding.** The four numbered items under "What remains". Checked 2026-10-27 (UTC): `kernel/src/coverage.rs`
  exists for aarch64 only. `kernel/src/lib.rs` refuses the `coverage` feature on the other two
  targets. `xtask/src/coverage.rs` boots the kernel's unit-test image and nothing else.

## Reuse

`minicov` 0.3.9 (MIT or Apache-2.0), Amanieu d'Antras's libc-free copy of LLVM's profiling runtime.
`cargo-llvm-cov` and `grcov` need a host process and a filesystem, which a kernel image has
neither of. Writing a serializer for the raw profile format would be a fork of LLVM's format with
nothing to track it.

## BUGS

- The report covers the kernel's unit-test image, on aarch64. The whole-system image, the services
  and the other two architectures are the list above; until they land the report undercounts what
  the full suite reaches.
- The instrumented image is slower and its stack frames are larger. A test that measures time or
  stack depth runs against different numbers than the plain image, so the report says what the
  tests reach and never whether they pass.
- Counters are plain integers, bumped without atomics on every core. A count can lose an increment
  when two cores bump one counter together; whether a region ran stays the same.

## Index row

`script/coverage` sees only host crates, so about half the tree's 1,761 error paths, including every cleanup path the count found, are invisible to it. The block builds the kernel's unit-test image with `-C instrument-coverage` and a `no_std` profiler runtime, dumps the counters through semihosting at exit, and reads them with `helpers/error_paths.py`; the whole-system image, the services and the other two architectures follow.
