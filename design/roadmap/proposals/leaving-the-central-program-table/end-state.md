# Leaving the central program table: the end state

[The proposal](../leaving-the-central-program-table.md) holds the summary and the order. This
appendix describes what exists instead of one central table, who owns each part, how the crates
depend on each other, and how the design meets the rulings that constrain it. Every name it
introduces is provisional.

## What exists instead

Six parts, and only one of them is a table.

1. **Each program declares its own manifest.** A program's source carries a manifest constant and
   places it with `capability_manifest_note::carry!`, as `least_authority_demo`, `jig`, eight
   programs in `fixtures/` and three in `std_exerciser/` do today. The constant lives beside the code that uses its slots, and it travels with
   the program when its package leaves for its own repository. The encoder refuses a pinned field
   at compile time, so no program can declare one.
2. **The vouch decides what a declaration earns.** For a program the image carries, the vouch is
   the archive's measurement: `measure_spawnable` already refuses an archive program whose digest
   the measured table does not hold. For an installed program, the vouch is a row of the live
   installed-programs table plus the owner's `grant` lines, exactly as `endowed_image` applies it
   today. `grant_plan::image_manifest` stays the one function both the shell and the progenitor
   call.
3. **A pinned table, keyed by name, holds what no note may carry.** Its rows are `reboot`
   (`reboot`, `sync`) and `jig` (installer, catalog, indexes, activation directory, console input,
   its verb table and the root resolver zone). `graphical_terminal` joins them unless
   [decision 4](decisions.md) makes the display session an ask. The table lives in `grant_plan`,
   where both callers already look, and changes only when a ruling adds a pinned grant.
4. **The progenitor publishes the base list.** At boot it already parses each archive program in
   `measure_spawnable`. It also reads each one's note and writes a read-only page of (name, note
   descriptor) pairs, which it hands the shell with the rest of its endowment. The shell plans a
   bare word against that page, and `caps` previews from it. The page is a cache of the notes, not
   a source: the progenitor reads its own copy of the note again at spawn and refuses a request
   that does not fit, as it does for an image today.
5. **The wire names an archive program by name.** A new request bit says "the archive entry named
   on the argument page", and the progenitor looks the name up in the archive it measured. Wire ids
   remain only for the pinned rows. A row that leaves the table leaves a hole, and no later program
   takes its id.
6. **Fixtures register nowhere.** A fixture carries a note like any program. The test images pack
   every `[[bin]]` (§158 (the shell's table is one macro), item 2), and a release image packs what
   `packages/base-image.toml` lists, which excludes the `fixtures` package. So the same mechanism
   that serves `date` serves `interrupt_heeder`, and nothing marks a program as a test.

Who owns each part:

| part | owner | changes when |
|---|---|---|
| a program's manifest | the program's package, in its own repository after §316 (a package's programs leave as one repository, after their crates) | the program changes what it needs |
| the vouch for a base program | whoever composes the image: `cargo xtask base-image` and basalt, against the approval file (milestone 909 (basalt composes the bootable image from its packages)) | an image is composed |
| the vouch for an installed program | the owner, at `jig install`, against an approval file for a script (milestone 911 (scripted installs grant only what the owner's approval file allows)) | a package is installed |
| the pinned table | nife's `grant_plan`, by ruling | a pinned grant is ruled |
| the base list page | the progenitor (`init` package) | the page format changes, which is a wire change |
| the planner | `grant_plan` | the grant language changes |

## The four shapes the question named, compared

### (a) Every program carries its own note, and the system pins only what it fixes

Recommended. The note format, the vouch and the image path all exist, in milestones marked BUILT (milestone 597
(a program carries its manifest in an ELF note), milestone 892 (a std program at the prompt holds
the network and a resolver)). `every_compiled_in_manifest_round_trips` proves that 26 of the 28 rows
encode as notes today; the two exceptions are the pinned rows. The precedent in the tree points the
same way. Milestone 956 (the linked interfaces carry checkable versions) put each program's
interface list in its own source as an `INTERFACES` note. A host check holds the list to the link
graph. It built no central list.

Its costs, stated plainly. It changes the wire (part 5) and adds a page format (part 4), and both
are one-way. It moves the decision about what a base tool may hold from code review inside nife to
the image's approval file, which today checks only the network, zones, the service, directories and
the installer (`program_asks::asked_grants`). And the compiler no longer forces a manifest for every
spawnable name; `carry!` forces a well-formed one per program instead, and a program with no note
runs with `NO_NOTE_MANIFEST`.

### (b) A registry the shell reads at start from the installed-programs table

Refused as a source of truth. The table holds a name, a digest and grant lines per program; it
holds no manifest. A registry that also held manifests would be a second copy of each note, outside
the hashed bytes, which can disagree with them. §197 (a package is one archive file) chose the note
inside the hashed bytes (its option M2) to avoid exactly that. What survives of (b) is part 4: a
page derived from the notes at boot, which the progenitor never trusts over its own reading.

### (c) Manifests assembled at build time, like the interfaces note

Refused as the end state. A generated table in `grant_plan` or the shell would still be one
artifact rebuilt for every program added, which is the churn this proposal exists to remove. Once
`core-tools` lives in its own repository, nife would have to build that repository's programs to
read their notes, or keep a copy. The interfaces note is not a counterexample: milestone 956
assembles nothing centrally, and its check only reads each program's own declaration.

### (d) Test-only registration for fixtures

Refused, because (a) makes it unnecessary. A `#[cfg(test)]` or feature-gated row would keep
fixtures inside `grant_plan` and keep a second registration path alive. Under (a) a fixture is an
ordinary program in a package of kind `test`.

## The dependency graph

The graph differs with and without the leaf crate the contract-crate order proposes (provisional
name `program_manifest`: `Manifest`, its declaration types and the slot constants). That decision
is still open; the design works either way.

Without the leaf crate:

```
glob, program_argument_contract
        ^
grant_plan  (planner, spawnproto, Manifest, slot constants, pinned table)
        ^
capability_manifest_note
        ^
process-tools, core-tools, mdr, fixtures   (notes and slot constants)
shell (swish), progenitor (system_initializer), jig, timetable
```

The table leaves, so adding or changing a tool no longer edits `grant_plan`. But each tool still
links `grant_plan` for `Manifest` (to build its note) and for the slot constants, so
`process-tools` and `core-tools` still wait for step 6 of the contract-crate order. `disk-tools`
should not gain a note in this shape: its programs have no row and no note today, and a note would
pull `grant_plan` into a package that otherwise never reaches it. `mdr` is the same case with a
row: `mdr/Cargo.toml` links neither `grant_plan` nor the note crate today, so in this shape its
note costs the `mdr` package a new edge to the planner.

With the leaf crate:

```
program_manifest  (Manifest, declarations, slot constants)
        ^                         ^
capability_manifest_note     grant_plan  (planner, spawnproto, pinned table)
        ^                         ^
process-tools, core-tools,   shell, progenitor, jig, timetable
mdr, disk-tools, fixtures
```

Now no tool links the planner. `program_manifest` and `capability_manifest_note` can leave before
`grant_plan`, so `process-tools` and `core-tools` stop waiting for step 6. `rm`'s option bits
(`grant_plan::rmopt`) need no shared home in either shape: the note declares the letters in bit
order, so `rm` can hold its own constants and a host test can check them against its note.

The leaf crate is what moves the packages earlier. Leaving the table is what makes their departure
coherent: while a row in nife's `grant_plan` holds `ps`'s manifest, a `ps` release that needs a new
grant needs a nife commit, which is the cross-repository coupling §316 sets out to end.

## Security: what a note claims, what vouches for it, what the system pins

A note is a claim the program makes about itself. It earns nothing on its own.

- Unvouched bytes get `UNVOUCHED_MANIFEST` whatever their note says: the clock and configuration
  pages and nothing from the progenitor's own table (§219 (how the shell names an installed program
  to the spawner), gate D2). Running them at all takes the run-unvouched capability.
- Vouched bytes get what their note declares, because the digest that vouched covers the note
  (§197's M2). Four asks also need the owner's grant: the network, a resolver zone, the service and
  a directory, each recorded as a `grant` line (`installed_programs::Granted`). §220 caps a key-trusted install at `domain entropy network`
  (§220 (signed builds: a vendor signs, a developer self-signs)).
- The pinned grants no note can carry: `reboot`, `sync`, `installer` and `catalog`, `indexes`, the
  activation directory and console input. The encoder refuses each at compile time, and
  `image_can_carry` refuses each again at spawn. The end state keeps both refusals, and the pinned
  table is the only place these fields are true.

What changes is who vouches for a base tool. Today a base tool's grants are whatever a reviewed
commit to `grant_plan` wrote. In the end state they are whatever its package's note asks, admitted
by the image's composition. The composition today checks the four owner-granted asks and nothing
else. Restrictive first: [decision 3](decisions.md) asks that the base approval also list the
progenitor-endowed asks (`domain`, `entropy`, `machine`, `share`) for each base package, so a
base-package update cannot widen what `ps` sees without a reviewed line.

## Checked against the rulings

| ruling | what it says | how the end state meets it |
|---|---|---|
| §158 (the shell's table is one macro) | the table is one macro (item 3); ids are written and never reused (item 4); every spawnable row has a binary and a script line (item 5) | item 3 shrinks to the pinned rows, which amends the section; item 4 holds; item 5's check moves to "every `[[bin]]` with a note has a script line" |
| §197 (a package is one archive file) | the manifest lives in the hashed bytes (M2) | unchanged; the base list page is a cache the progenitor never trusts |
| §219 (how the shell names an installed program to the spawner) | option D, gate D2, unvouched bytes get clock and configuration only | unchanged |
| §220 (signed builds: a vendor signs, a developer self-signs) | a key's ceiling is a subset of `domain entropy network` | unchanged; it governs key-trusted installs, not the image's own programs |
| §229 (a bare name reaches an installed program) | B2: a bare name resolves through the live set; install refuses a name the image carries; the amendment narrows that refusal to what a slot holds | **conflict**: today "a name the image carries" means a row of this table (`activation.rs`'s `image`). The end state must say what it means instead; [decision 2](decisions.md) asks |
| §241 (a threadbare base) | the slot holds the kernel, the progenitor and the root supervisor; every other program is a package | supports the end state: the table is what keeps base tools from updating as packages |
| §251 (restarting the machine is a kernel object) | the progenitor endows the reboot object only to a program whose manifest declares it, "the way `entropy` is endowed" | met by the pinned row. Milestone 805 (`reboot` at the prompt) rules that no note can spell `reboot`; §251's comparison with `entropy` describes the endowment, not a note. No conflict, but the wording invites one |
| §270 (a package manager holds an installer endpoint) | only the owner's console grants it | met by the pinned `jig` row; milestone 911's delegated installer badge is an owner's grant and stays one |
| §272 (the grant-plan crate root splits into private modules), fork 2 | the table is expected to shrink once packages install through `jig` | this is that shrinking |
| §309 (a supervised job's `ThreadControlBlock` goes to the shell that asked for it) | an interruptible job is built from the shell's job budget | the by-name path builds archive programs the way rows build today, so it inherits this; the frames path does not ([gaps appendix](installed-path-gaps.md)) |
| §316 (a package's programs leave as one repository) | programs leave with their package | requires the end state: a manifest in nife cannot follow its program out |
| milestone 892 | the owner grants the network at install; whether a zone is the manifest's word or the owner's is open, except `jig`'s root zone | unchanged; `jig`'s zone stays pinned |
| milestone 990 (the base image's first installed-programs table) | a composed image writes a row per base program, with no pointer, claiming no bare word | gives the composed image an installed-programs vouch for each base program beside the measurement; the end state relies on the measurement, which the test images also have |
