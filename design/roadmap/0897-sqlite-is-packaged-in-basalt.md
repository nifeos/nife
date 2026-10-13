---
status: NOT-STARTED
raised: 2026-10-10
promoted_from: sqlite-is-packaged-in-basalt
milestone_dependencies: 835, 810
decision_dependencies: 265
machine_requirements: none
specific_machine: none
needs_person: no
---
# 897. SQLite is packaged in basalt, and its tests run in basalt's package build

*(The title and slug are provisional.)*

calef, 2026-10-10 (UTC): *"We can't add testing of every program that nife will ever run into
this tree. We need packages working."*

The sibling of milestone 810 (`ripgrep` is packaged in basalt and installed with `jig`), for SQLite.
Milestone 835 (a C library, stage 1: files, clock and memory) runs SQLite's `speedtest1` unmodified
on all three architectures. Its tests live in nife today (`system_tests/src/user/c_program_tests.rs`)
and skip unless `helpers/build-speedtest1.sh` ran. That is a foreign program tested inside the
kernel's tree, which calef ruled is the wrong place.

## The work

- basalt builds SQLite 3.50.4 and `speedtest1` from their pinned sources against the C library of
  the nife commit basalt pins, with the flags `helpers/c-library-cflags.sh` gives.
- basalt's package build runs `speedtest1 --verify` in memory and on a file, on each architecture,
  and fails unless both print the hash nife's tests check today.
- ioping is packaged the same way by a sibling of its own. That is milestone 906 (ioping is
  packaged in basalt, and its test installs it with `jig`), under §297 (software reaches a nife system as a package `jig` installs from basalt's index, the
  kernel included).
- When basalt's build runs them, the C program tests leave nife, and so do the two helpers.

Reuse: milestone 810's packaging and its basalt build, the helpers' pinned fetches, and the
assertions of `c_program_tests.rs`, moved rather than rewritten.

## Index row

SQLite (and ioping) built and tested in basalt's package build against the pinned nife, as `rg` is
by milestone 810, so foreign programs' tests leave nife's tree.
