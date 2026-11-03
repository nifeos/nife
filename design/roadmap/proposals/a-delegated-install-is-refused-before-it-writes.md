---
status: PROPOSED
raised: 2026-11-03
milestone_dependencies: 911
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A delegated install is refused before it writes a byte

Raised 2026-11-03 (UTC) by the security audit
[2026-11-03-from-the-package-to-the-grant.md](../../audit-reports/2026-11-03-from-the-package-to-the-grant.md),
finding 1. Severity: medium. Every name below is provisional.

Reuse: `approval_gate` and `place_program` in `crates/system_initializer/src/activation.rs` as
they are. The work is their order, and one check in `package_archive`.

## The failure

Milestone 911 (scripted installs grant only what an approval file allows) bounds what a holder of
a delegated installer badge may do. The comment on the delegated check says it runs "before anything
is placed or written". It does not. `install` calls `place_program` for every program in the
package first, and `approval_gate` only after. A package the approval does not cover, or one marked
`update = false` that is already installed, is refused, but its bytes are already under
`packages/<name>/<version>/`, written or replaced in place.

No authority widens. No table row or grant is written, and spawn hashes the bytes it runs. What
is lost is the bound the approval claims on the store: a delegated holder can fill the disk, or
replace the bytes at a path a person runs by name, with any package the catalog vouches for,
whether or not the owner approved it.

## The work

1. Run the delegated check before placement, and keep the comment that says so true. A test drives
   a delegated install of an unapproved package and asserts that nothing new is under `packages/`.
2. `package_archive::check_programs` refuses a program name that is empty or holds `/`. It also
   accepts `.` and `..`. The file server refuses both components today (`redoxfs_server`'s
   directory tests), so this is depth and not a hole. The archive check refuses them as well, so
   the install path does not depend on the server for it.

## What this does not do

It does not revisit what a delegated holder may install. That is milestone 911's ruling.
