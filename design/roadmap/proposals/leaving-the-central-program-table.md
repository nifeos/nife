---
status: PROPOSED
raised: 2026-11-02
milestone_dependencies: 597, 805, 892, 909, 911, 990
decision_dependencies: 158, 197, 219, 220, 229, 241, 251, 272, 316
machine_requirements: none
specific_machine: none
needs_person: yes
---
# Leaving the central program table

Raised 2026-11-02 (UTC) by the lane `maintainer/leave-the-central-program-table`. The architect
asked how `crates/grant_plan` stops holding one central table of the programs the shell can spawn,
what replaces it, and how to get there. This proposal answers from the code, writes no code, and
names everything new provisionally. A file and a symbol stand in for line numbers, which §277 (cite
a heading or a symbol) forbids.

Reuse: the manifest note and its encoder (milestone 597 (a program carries its manifest in an ELF
note)) and the vouch in `grant_plan::image_manifest`. Also the owner's `grant` lines (milestone 892
(a std program at the prompt holds the network and a resolver)), the approval check (milestone 911
(scripted installs grant only what the owner's approval file allows)) and the progenitor's measured
archive. No new dependency.

Appendices: [every row, classified](leaving-the-central-program-table/classification.md),
[every consumer](leaving-the-central-program-table/consumers.md), [the end
state](leaving-the-central-program-table/end-state.md), [what an installed program cannot hold
yet](leaving-the-central-program-table/installed-path-gaps.md), [the
migration](leaving-the-central-program-table/migration.md), [prior
art](leaving-the-central-program-table/prior-art.md), [the
decisions](leaving-the-central-program-table/decisions.md) and [what was not
verified](leaving-the-central-program-table/unverified.md).

## What the tree does today

`crates/grant_plan/src/program/catalog.rs` declares 28 programs through the `programs!` macro, wire
ids 0 to 27. `program/manifests.rs` gives each its manifest in `Program::manifest`. The shell
resolves a bare word with `Program::from_name`, plans the line against that manifest, and sends the
id. The progenitor turns the id back into a program with `Program::from_id`, takes the ELF it
loaded at boot from the archive by the same name, and endows the same manifest.

A second path already exists. A program may carry its manifest in an ELF note
(`.note.nife.manifest`). When the shell runs a file by path, or a bare word that names an installed
program, it reads the note and sends the file's bytes. The progenitor hashes its own copy, looks
the digest up in the live installed-programs table, and calls `grant_plan::image_manifest` with the
note and the `Vouch`. `endowed_image` in `crates/system_initializer/src/image_request.rs` does this.

The table is not the main source of the crate's churn. Over the 30 days before this branch, 148
non-merge commits touched `crates/grant_plan`. Of those, 36 touched `program.rs` or `program/`, 55
touched `spawnproto.rs` and 43 touched `lib.rs`. About a third of the 36 add or change a program.

## What the maintainer believed, checked

Each belief the brief carried, against the code.

1. **Two paths endow a child.** Correct. One refinement: the `Vouch` the progenitor builds carries
   only the network grant (`vouched` calls `installed_programs::network_granted`), and zones come
   separately through `granted_zones`. `installed_programs::Granted` has five kinds (`Network`,
   `Zone`, `Service`, `Directory`, `Installer`), but the spawn path reads only the first two. The
   service has no start yet (milestone 910 (a package declares a service) is not started), and the
   directory kind has no asking manifest yet.
2. **What `image_can_carry` refuses.** Correct, with two corrections of wording. No field says
   "supervised job" or "second stream": the first is `interruptible`, the second is
   `OutputDeclaration::BytesAndDiagnostics`. And the function also refuses every pinned field
   (`reboot`, `sync`, `installer`, `catalog`, `indexes`, the activation directory, console input).
   Beyond `image_can_carry`, the frames path has transport limits of its own: an image runs only on
   a plain line (no pipe, no redirection), and the progenitor refuses a held image. And `endowed_image`
   refuses installed bytes with a `VERBS` note. So no package can install `rm` or `wc` today. Nor
   can one install `mdr` (an input) or the eight tools that declare a second stream: the frames
   path blocks 11 of the 14 tools.
3. **`reboot` and `sync` belong to one program, and no image may hold them.** Correct. The note
   encoder refuses them at compile time and `image_can_carry` at spawn. The same two refusals pin
   `jig`'s five owner's-console fields.
4. **About 28 rows.** Exactly 28, ids 0 to 27, no hole. `Program::manifest` has 27 arms because
   `NullSyscall | SinkProducer` share one, and 26 manifest functions serve them. The wire-id pin
   covers ids 0 to 12 only. Fixtures sit in the table because it is the only way to make a program
   spawnable by bare name on a boot with no disk. With a disk a fixture could run by path, but most
   declare grants the frames path cannot carry.
5. **`Program::resolver_zone` and `Program::verbs` key on the row.** Correct. Each answers
   something only for `jig`. A `VERBS` note type exists (`capability_manifest_note::encode_verbs`),
   and nothing on the installed path reads it.
6. **The leaf crate waits on a ruling.** Correct; the design works with and without it.

Findings the brief did not carry:

- The timetable is a fourth spawner. `components/src/timetable.rs` builds jobs from its own archive
  by `e.program.name()` and endows `registered.program.manifest()`; in store mode it already reads
  installed programs' notes.
- `pmap`, `disk_surveyor` and `disk_partitioner` ship in packages and have no row, so package
  membership and a row are already independent.
- `mdr/Cargo.toml` links neither `grant_plan` nor the note crate. A note for `mdr` costs its package
  a new edge to the planner unless the leaf crate exists.
- Milestone 990 (the base image's first installed-programs table) already writes a vouch row for
  every program a composed image carries.

## The end state recommended

Each program declares its own grants in its own note, and the system pins only what it must fix
itself. Six parts; [the end state appendix](leaving-the-central-program-table/end-state.md) has the
graphs, the owners and the check against each ruling.

1. Each program's source holds its manifest and carries it as a note. 26 of the 28 rows already
   encode (`every_compiled_in_manifest_round_trips`).
2. The vouch decides what a note earns: the measured archive for the image's own programs, and the
   installed-programs table with the owner's `grant` lines for the rest. Unvouched bytes keep
   §219 (how the shell names an installed program to the spawner)'s clock and configuration pages
   and nothing else.
3. A pinned table in `grant_plan`, keyed by name, holds `reboot` and `jig`, and `graphical_terminal`
   unless question 4 rules otherwise. It is the only place a pinned field is true.
4. The progenitor builds a read-only page at boot listing each archive program's name and note. The
   shell plans, completes and previews from it. The progenitor never trusts the page over its own
   reading.
5. A request names an archive program by name. Wire ids survive only for pinned rows, and every
   removed id stays a hole.
6. Fixtures register nowhere. They carry notes; test images pack them and release images do not.

Why this shape and not the others. A registry in the installed-programs table would copy each note
outside the bytes the vouch covers, which §197 (a package is one archive file) chose against. A
table generated at build time would still change for every program, and would make nife build each
departed package to read its notes. A test-only registration is unnecessary once fixtures carry
notes. And while a row in nife holds `ps`'s manifest, a `ps` release that needs a new grant needs a
nife commit, which is the coupling §316 (a package's programs leave as one repository, after their
crates) means to end.

The trade-offs, plainly. It adds a page format and a wire bit, both one-way. It moves the decision
about what a base tool may hold from nife's code review to the image's approval file, which today
checks only four kinds of grant; question 3 asks to widen it. And it conflicts with §229 (a bare
name reaches an installed program), whose install refusal this table defines today; question
2 asks how to redefine it.

## The migration, one line per step

[The migration appendix](leaving-the-central-program-table/migration.md) gives each step's proof,
the wire-id rules and the test strategy.

1. Every non-pinned row's program except `mdr` carries a note copied from its row; an on-target
   check holds note and row equal; the id pin covers all 28. No ruling needed.
2. The `Endowment` carries its manifest, so nothing reads a row to plan. No ruling needed.
3. The frames path carries a second stream and redirection, so eight tools can install from a disk.
   No ruling needed.
4. The progenitor publishes the base list and the shell plans from it (question 1).
5. Archive programs spawn by name, and §229's refusals switch to the archive's names (questions 1
   and 2).
6. The rows leave, one package per pull request: fixtures, `process-tools`, `core-tools`, `demos`,
   then `mdr`.
7. The pinned table keys on names, and the progenitor's id array shrinks to it.
8. The leaf crate, if ruled, lets the tools shed the planner and moves `process-tools` and
   `core-tools` ahead of `grant_plan` in the contract-crate order.

Only step 8 moves a package earlier in that order. Steps 1 to 7 remove the per-program edits from
`grant_plan`, about a quarter of its recent commits.

## The decisions, in order

[The decisions appendix](leaving-the-central-program-table/decisions.md) gives each one with its
background, alternatives and example.

1. Does every program carry its own grant record, with the central table kept only for grants the
   system fixes? Recommended yes. One-way.
2. Which bare names belong to the image once the table no longer says? Recommended: the measured
   archive's names, with §229's refusals unchanged. One-way; it conflicts with §229's present
   definition.
3. Must the base approval file list every grant the system endows (`domain`, `entropy`, `machine`,
   `share`)? Recommended yes. One-way.
4. Is taking the screen and keyboard pinned, or an owner's grant? Recommended pinned. One-way.
5. May the new names ship provisional? Recommended yes. Two-way in effect.
6. The leaf crate stays the separate decision already before the architect.

## BUGS

- Nothing settles the bench rows' class: milestone 961 (the kernel's special boots become commands
  typed at the production shell) runs them at a production shell, and the release image excludes
  the package that holds them.
- `pmap` ships in every image and nothing can spawn it, as the BUGS of §158 (the shell's table is one macro)
  records. Under the end state it needs only a note and a script line; this proposal does not
  schedule that.

## Index row

How `crates/grant_plan` stops holding the shell's program table. Each program declares its own
grants in its own note, and the vouch decides what the note earns. A pinned table keeps only
`reboot` and `jig`, and archive programs spawn by name. Eight steps get there, each keeping main
green.
