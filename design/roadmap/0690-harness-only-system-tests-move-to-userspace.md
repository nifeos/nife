---
status: NOT-STARTED
raised: 2026-09-27
promoted_from: harness-only-system-tests-move-to-userspace
milestone_dependencies: 609
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 690. The harness-only system tests move to userspace

*Title and slug are drafts.*
calef's third rule of 2026-09-27: existing tests that touch no kernel internals migrate to userspace
over time. Milestone 609 (the system tests leave the kernel crate) measured which do. Of the 66 files
in `system_tests/src/user/`, three name nothing in the kernel but the test harness (`skip!`):
`language_tests`, `pipeline_tests` and `redirection_tests`, 17 tests in 717 lines. They drive the
shell and read what it printed, which a userspace test program can do through the spawner and a
byte sink.

Everything else in that directory observes kernel state no syscall exposes, which is the case the
second rule allows to stay. The table is in notes/system-tests-and-the-kernel-crate.md.

What this needs first is a userspace harness that reports into the same transcript (`running`,
`test result:`), so `script/test`, `--test` and the falsification sweep read it unchanged.

## Index row

Three system test files, 17 tests in 717 lines, name nothing in the kernel but the harness. Proposed: move them to userspace behind a harness that reports into the same transcript.
