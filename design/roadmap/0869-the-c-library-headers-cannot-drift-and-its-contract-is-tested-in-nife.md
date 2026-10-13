---
status: NOT-STARTED
raised: 2026-10-10
milestone_dependencies: 835
decision_dependencies: 46, 265
machine_requirements: none
specific_machine: none
needs_person: no
---
# 869. The C library's headers cannot drift, and its contract is tested in nife

*(The title and slug are provisional.)*

calef ruled Q2 on #1896 (2026-10-10 UTC) as option B. The cbindgen headers stay committed, and CI
regenerates them and fails on any difference, the shape rustls-ffi uses. On Q3 he ruled the same
day: *"We can't add testing of every program
that nife will ever run into this tree. We need packages working."*

## What this milestone is, and is not

It is the C library's own contract, tested where the library lives. It is not the foreign programs.
Each foreign program's tests (SQLite, ioping, and later `ripgrep`) run in basalt's package build
against the pinned nife. That is milestone 810 (`ripgrep` is packaged in basalt and installed with
`jig`)'s shape. For SQLite it is the proposal `design/roadmap/0897-sqlite-is-packaged-in-basalt.md`.

## What it builds

- A drift check. CI installs cbindgen 0.29.0 and runs `helpers/c-library-headers.sh`. Any
  `git diff` under `vendor/relibc/include/` fails it.
- The C library's contract tests in nife: small C programs written for the purpose, in the suite on
  every architecture with no fetch. They pin what the platform layer promises (open, read, write,
  seek, stat and unlink through a grant; the clock; `malloc`; a `double` through `printf`'s
  varargs; `errno` on a refused grant), so a regression in the library fails nife's own CI.

Reuse: the header script as it is, and `c_library`'s build path for the test programs.

## What it leaves where it is

Milestone 835 (a C library, stage 1: files, clock and memory)'s local SQLite and ioping tests (`system_tests/src/user/c_program_tests.rs`) stay in
nife until basalt's build runs them. Then they move out, with `helpers/build-speedtest1.sh` and
`helpers/build-ioping.sh`.

## Done when

CI fails a pull request that changes a seeded module's exported types without regenerating its
header. It also fails one that breaks any of the library's contract tests.

## Index row

The C library's own contract in nife's CI: a check that regenerates its headers and fails on drift,
and contract tests written for it. Foreign programs are tested in basalt's package build.
