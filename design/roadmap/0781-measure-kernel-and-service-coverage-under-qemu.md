---
status: NOT-STARTED
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

Ruled 2026-10-27 (UTC) by the architect: option 2. Build the test kernel, and then the services,
with `-C instrument-coverage` and a `no_std` profiler runtime that only test builds depend on, and
dump the counters at QEMU exit through semihosting. The report uses the same regions as the host
`script/coverage` report, so `helpers/error_paths.py` reads it unchanged. The QEMU TCG plugin of
option 1 is not built: blocks give lines and not regions, a `?` error edge would have to be found by
address, and user programs that share virtual addresses would have to be told apart by address space.

Conditions on the build:

- The runtime is a dependency under §46 (thin primitives or whole subsystems). Before it is added,
  the candidate (`minicov` is the usual one) is audited for version, licence, transitive
  dependencies, any C build step and maintenance, and the result is recorded where the tree records
  a dependency. A runtime that is more than a thin one, or that needs a C toolchain the CI lacks,
  stops the work and goes back to an architect.
- The version is pinned exactly and is at least two weeks old.
- The first pull request covers the kernel on one architecture end to end: the instrumented build,
  the counter dump, an lcov or JSON report, and `helpers/error_paths.py` reading it. It reports how
  many of the kernel's error paths the tests reach. The services and the other two architectures
  follow in later pull requests.
- QEMU runs under `helpers/qemu-bounded.sh`. No recurring heavy CI job is added; any schedule goes in
  the Pacific-night window (about 08:00 to 13:00 UTC) and needs a report first.

## Index row

`script/coverage` sees only host crates, so about half the tree's 1,761 error paths, including every cleanup path the count found, are invisible to it. The ruling builds the kernel's unit-test image first, with `-C instrument-coverage` and a `no_std` profiler runtime, dumps the counters through semihosting at exit, and reads them with `helpers/error_paths.py`; the services and the other two architectures follow.
