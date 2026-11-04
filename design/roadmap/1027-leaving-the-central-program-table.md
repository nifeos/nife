---
status: NOT-STARTED
promoted_from: leaving-the-central-program-table
raised: 2026-11-02
milestone_dependencies: 597, 801, 805, 809, 892, 909, 910, 911, 990
decision_dependencies: 158, 195, 197, 219, 220, 229, 241, 251, 270, 272, 316
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1027. Leaving the central program table

Raised 2026-11-02 (UTC) by the lane `maintainer/leave-the-central-program-table`. The architect
asked how `crates/grant_plan` stops holding one central table of the programs the shell can spawn,
what replaces it, and how to get there. On 2026-11-02 (UTC) the architect ruled that no grant binds
to a program's name: an owner's `grant` line may give installed bytes any capability the
progenitor endows, `reboot` and the installer included, and several programs may hold one. The
image's own programs keep their grants through the measured archive and stay the default. This
proposal answers from the code under that ruling, writes no code, and names everything new
provisionally. A file and a symbol stand in for line numbers, which §277 (cite
a heading or a symbol) forbids.

Reuse: the manifest note and its encoder (milestone 597 (a program carries its manifest in an ELF
note)) and the vouch in `grant_plan::image_manifest`. Also the owner's `grant` lines (milestone 892
(a std program at the prompt holds the network and a resolver)), the approval check (milestone 911
(scripted installs grant only what the owner's approval file allows)) and the progenitor's measured
archive. No new dependency.

Appendices: [every row, classified](1027-leaving-the-central-program-table/classification.md),
[every consumer](1027-leaving-the-central-program-table/consumers.md), [the end
state](1027-leaving-the-central-program-table/end-state.md), [what an installed program cannot hold
yet](1027-leaving-the-central-program-table/installed-path-gaps.md), [what each owner grant
risks](1027-leaving-the-central-program-table/security.md), [how a typed word finds a
program](1027-leaving-the-central-program-table/command-names.md), [the
migration](1027-leaving-the-central-program-table/migration.md), [prior
art](1027-leaving-the-central-program-table/prior-art.md), [the
decisions](1027-leaving-the-central-program-table/decisions.md) and [what was not
verified](1027-leaving-the-central-program-table/unverified.md).

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
   `OutputDeclaration::BytesAndDiagnostics`. And the function also refuses every owner-granted field
   (`reboot`, `sync`, `installer`, `catalog`, `indexes`, the activation directory, console input).
   Beyond `image_can_carry`, the frames path has transport limits of its own: an image runs only on
   a plain line (no pipe, no redirection), and the progenitor refuses a held image. And `endowed_image`
   refuses installed bytes with a `VERBS` note. So no package can install `rm` or `wc` today. Nor
   can one install `mdr` (an input) or the eight tools that declare a second stream: the frames
   path blocks 11 of the 14 tools.
3. **`reboot` and `sync` belong to one program, and no image may hold them.** Correct of the code:
   the note encoder refuses them at compile time and `image_can_carry` at spawn, and the same two
   refusals keep `jig`'s five owner's-console fields off every image. The ruling of 2026-11-02 makes
   each an owner's grant; the code does not follow yet.
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

Each program declares its own grants in its own note. The vouch decides what a note earns, and the
owner's `grant` lines decide what installed bytes earn beyond it. Nothing in nife keys on a
program's name. [The end state appendix](1027-leaving-the-central-program-table/end-state.md) has the
details.

1. Each program's source holds its manifest and carries it as a note, `reboot`, `jig` and
   `graphical_terminal` included. 26 of the 28 rows already encode
   (`every_compiled_in_manifest_round_trips`); the other two need the `OWNER_GRANTS` note type.
2. The vouch: the measured archive for the image's own programs, the live installed-programs
   table for the rest. Every owner-granted field needs a `grant` line for the program's digest; the
   composition writes the image's own lines from the base approval file, which lists every grant
   the image makes.
   `Vouch::Vouched` grows from `{ network }` to a set of owner-granted kinds, and `image_manifest`
   refuses a declared kind without its grant, naming it, as it does for the network today.
3. The owner's `grant` lines gain one kind per owner-granted field: `reboot`, `sync`,
   `installer within <approval>`, `catalog`, `indexes r` or `rw`, `activation r`, `console`, and
   `display`. The root zone already has one (`zone .`). The verb table needs none: the `VERBS` note
   narrows within what the owner granted. Each line binds to a digest, and a new version asks
   again unless an approval entry's `update` covers it.
4. `grant_plan` keeps the planner, `spawnproto`, `Manifest`, `Vouch` and the list of owner-granted
   fields. No `Program`, no pinned table and no role list remain.
5. The progenitor publishes a base list page from the archive's notes, and the shell plans from it
   and asks for an archive program by name. Every wire id becomes a hole.
6. A typed word finds a program as it does today, and §229 (a bare name reaches an installed
   program) stands: a bare word the image carries runs the image's program. A replacement runs
   under its own name or by path, and its grants follow its digest, not its name. [The command-name
   appendix](1027-leaving-the-central-program-table/command-names.md) has it.

The grants differ in danger. [The security
appendix](1027-leaving-the-central-program-table/security.md) reads each one's behavior with two holders
from the code and sorts them into three classes. `reboot`, `sync`, the catalog, the activation
directory and zones are shareable. Console input and the display session are exclusive: one
foreground holder at the owner's console. The installer and the index store's write right are
console class, because a holder of either can turn one yes into every grant. An installed holder
gets them only in §270 (a package manager holds an installer endpoint, not the spawn endpoint)'s
delegated form, bounded by an approval. The built-ins stay the default and the fallback: no install
takes the archive's grants away, and rollback or removal drops a replacement's grants with its rows.

Why this shape and not the others. A registry in the installed-programs table would copy each note
outside the bytes the vouch covers, which §197 (a package is one archive file) chose against. A
table generated at build time would still change for every program, and nife would build each
departed package to read its notes. A pinned table for `reboot` and `jig` no longer has a reason:
the ruling binds no grant to a name. And while a row in nife holds `ps`'s manifest, a `ps` release
that needs a new grant needs a nife commit, which is the coupling §316 (a package's programs leave
as one repository, after their crates) means to end.

The trade-offs, plainly. It adds a note type, a page format and a wire bit, all one-way. It
moves the decision about what a base tool may hold from nife's code review to the image's approval
file. It lets an owner hand a reviewed-by-nobody program the power to restart the machine, which is
the ruling's intent and the reason for the confirmation in the security appendix.

## The migration, one line per step

[The migration appendix](1027-leaving-the-central-program-table/migration.md) gives each step's tests,
the wire-id rules and what changed from the first plan.

1. Every row's program except the three owner-granted ones and `mdr` carries a note copied from its
   row; all 28 ids pinned. No ruling.
2. The `Endowment` carries its manifest. No ruling.
3. The frames path carries a second stream and redirection. No ruling.
4. A note type of its own, `OWNER_GRANTS`, spells the owner-granted fields; `image_can_carry`
   still refuses them (ruled as decision 1).
5. The grant lines, the `OwnerGrants` vouch and the confirmation; the blanket refusals go in the
   same pull request; the console class stays delegated (ruled as decision 2).
6. The display session keys on `Manifest::display` and allows one session.
7. The installed path reads the `VERBS` note.
8. `reboot`, `jig` and `graphical_terminal` carry their own notes.
9. The progenitor publishes the base list (ruled as decision 3).
10. Archive programs spawn by name, with grants from generation 1's lines; the image's names are
    the measured archive's (ruled as decision 7).
11. The rows leave, one package per pull request; the last deletes `Program`.
12. The leaf crate, if ruled.

The pinned table keyed by name, and the wire ids it would have kept, are no longer steps. Every
step except the leaf crate has its ruling.

## The decisions, in order

[The decisions appendix](1027-leaving-the-central-program-table/decisions.md) gives each one with its
background, alternatives and example. The ruling dissolved the question of the screen and keyboard.

1. Ruled 2026-11-02 (UTC): the owner-granted asks travel in their own note type, `OWNER_GRANTS`.
2. Ruled 2026-11-02 (UTC): three classes, with the installer and the index store's write right
   bounded by an approval for installed bytes, and none of the sensitive grants for a recorded
   service.
3. Ruled 2026-11-02 (UTC): every program carries its own grant record, and nothing central remains.
4. Ruled 2026-11-02 (UTC): the base approval file lists every grant the image makes to its own
   programs, and nothing comes from the archive implicitly.
5. Ruled 2026-11-02 (UTC): a repository key's ceiling never covers an owner-granted kind.
6. Ruled 2026-11-02 (UTC): no override. A bare word the image carries runs the image's program,
   and an installed program runs under its own name or by path.
7. Ruled 2026-11-02 (UTC): "a name the image carries" means a name in the measured archive, with
   §229 (a bare name reaches an installed program)'s refusals unchanged.
8. Decided by the maintainer, provisional: every new name ships provisional and meets its own name
   review. Reversing costs one pull request.

The leaf crate stays the separate decision already before the architect.

## BUGS

Each of these holds today and matters only once installed bytes may hold the grant; the migration
step named fixes it.

- Console input has no badge check: `SpawnLoop::console_input` places stdin for any `std` job that
  declares it, where the installer checks `req.badge == 0`. Safe today because only `jig` declares
  it. Step 5.
- Nothing keeps the display session to one holder. `Request::graphical` starts `gpu_driver` on the
  lent capabilities with no live-session flag; only the shell's wait prevents a second. Step 6.
- The index store has no lock, and the progenitor installs on its copy's word (milestone 801
  (packages over the internet), ruling I2). Two writers race. Step 5 gives each manager its own
  copy.
- The progenitor believes a holder's `answered console` trailer and writes every ask as granted
  (`activation.rs`, `program_asks::asked_grants`). The owner's yes lives only in `jig`'s prompt.
  Step 5 refuses that trailer from an installed holder.
- The manager column records the holder's program name (`hold_reply(label, reply, p.name())`), and
  `can_edit` compares names. An installed manager needs an identity that is not a program name;
  its package name is the obvious one. Step 5.
- Nothing settles the bench rows' class: milestone 961 (the kernel's special boots become commands
  typed at the production shell) runs them at a production shell, and the release image excludes
  the package that holds them.
- `pmap` ships in every image and nothing can spawn it, as the BUGS of §158 (the shell's table is
  one macro) records. Under the end state it needs only a note and a script line; this proposal
  does not schedule that.

## Index row

How `crates/grant_plan` stops holding the shell's program table, under the ruling that no grant
binds to a program's name. Each program declares its own grants in its own note; the archive
vouches for the image's programs, and the owner's `grant` lines give installed bytes any
capability, `reboot` and the installer included, sorted into three classes by danger. A
replacement runs under its own name. Twelve steps get there, each keeping main shippable.
