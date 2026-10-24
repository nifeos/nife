# The durable schedule store's manifest

An appendix to [notes/scheduled-execution.md](../scheduled-execution.md), which explains the
scheduler and its grants. This file holds how boot-time re-derivation finds the schedules it must
restore, which `crates/timetable_store` defines (milestone 152).

## One file per identity, and a manifest that lists them

§122 (the on-disk, per-user schedule store) decided the schedule itself: one file per identity,
inside that identity's own subtree, in `timetable::parse`'s format. What tells boot-time
re-derivation which identities have one is a manifest, as §125 (what tells boot-time re-derivation
which identities have pending work) recorded. It sits at a fixed, well-known location and lists the
identities with pending scheduled work, one name per line.

## Why a manifest rather than a directory listing

`READDIR` of the principal tree's root would be enumeration, which §123 (boot-time re-derivation)
refuses the re-deriver. It would also answer a different question, because every provisioned
identity gets a subtree whether or not it ever registers a schedule. Opening one file at a name known
at compile time is a targeted lookup, and what the manifest's contents name is data, not a directory
the reader walked.

## What the crate is not

It performs no IO and makes no syscalls. Both sides are `components/src/login.rs`. It writes an
identity's line when a schedule opens at a login, and reads the manifest at start-up to re-derive
each session. The parser and the renderer stay one function each in the crate. It does not touch
`timetable::Schedule` or `timetable::parse`, and depends on nothing.

## The shared fixture

The fixture schedule the kernel's tests share is modeled on `components/timetable.conf`. Its
commands name the suite's installed fixture program, which `login_tests`' `install_scheduled_program`
installs. A durable session's timetable resolves commands in the installed-programs table, where
§229 (how a bare name at the prompt reaches an installed program) forbids a package from taking a
name the image carries. `timetable_tests`' registrar control copies the document's shape with an
image program's name back in. An archive-mode timetable plans against `grant_plan`'s static
manifests and knows only image programs; see its `STORED_SCHEDULE_CONTROL`.
