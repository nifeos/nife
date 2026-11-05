---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 351. Every `std` test grants the mount root, so a walk that over-asks for rights still passes

Premise re-checked 2026-09-19 and it holds. `kernel/src/user/std_tests.rs` names no caretaker: every std program it spawns is handed
the mount root, and the one rights-shaped assertion in the file is about `/../motd` asking for the
level above the only root it has. `fs_subtree_caretaker` is built and packed
(`components/Cargo.toml`), so both halves of the fixture exist and nothing has joined them.

Both halves exist. `fs_subtree_caretaker` narrows a grant today, and the `std`
platform abstraction layer (PAL) runs under the test runner on every architecture. Nothing is owed
by another milestone and no decision is waiting.

**In brief.** Spawn a `std` program on an `fs_subtree_caretaker` endpoint rather than on the mount
root, and run the existing PAL tests against it. That exercises the rights discipline under a
narrowed grant, which nothing does now. Every `std` test today is handed the root, so a path
operation that asks for more rights than it needs is indistinguishable from one that asks for
exactly what it needs. Both succeed.

## Why this matters

This is a test that would have caught a real defect, and the block says so: `readdir` nearly
shipped asking for `dir::ALL`. Under the root grant that code passes every test in the tree, and it
fails the first time a program is confined, which is the case the whole capability model exists to
serve. A test suite that cannot tell a confined program from an unconfined one is not testing the
property this system claims.

It also compounds. The PAL is being extended a piece at a time, and each new operation picks its
rights by whatever the author thought was needed. Under the current fixture nobody finds out. Under
a narrowed one, over-asking is a failing test at the moment it is written, which is rung two of
AGENTS.md's ladder rather than rung four. Rung four is where `readdir` was caught by somebody
happening to read the diff.

A held directory in the PAL (milestone 122 (a directory handle `std` can hold)) asks for
`dir::PARENT_RIGHTS`, the sentinel §98 (`OPENDIR` cannot be asked to attenuate) ruled (`design/decisions/0098-opendir-cannot-attenuate.md`),
so it reaches the child in one message under any grant. No test under a narrowed grant exercises
that path, for the same reason, and this fixture shows that a held directory gets exactly the
parent's rights.

## Index row

Every `std` test is handed the mount root, so a path operation that asks for more rights than it
needs is indistinguishable from one that asks for exactly what it needs. Both succeed. The fix is to spawn a
`std` program on an `fs_subtree_caretaker` endpoint. Running the platform abstraction layer tests
against it exercises the rights discipline under a narrowed grant. It would have caught a real
defect. `readdir` nearly shipped asking for `dir::ALL`. That passes every test under the root grant and
fails the first time a program is confined. A held directory asks for `dir::PARENT_RIGHTS`
(§98), and no `std` fixture exercises that path under a narrowed grant either.
