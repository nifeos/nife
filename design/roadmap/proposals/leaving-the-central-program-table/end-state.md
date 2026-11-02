# Leaving the central program table: the end state

[The proposal](../leaving-the-central-program-table.md) holds the summary and the order. This
appendix describes what exists instead of one central table, what the owner's `grant` lines say,
how the vouch grows, and where a grant lives. [The security appendix](security.md) weighs each
grant's danger, and [the command-name appendix](command-names.md) says how a word finds a program.
Every name it introduces is provisional.

The ruling of 2026-11-02 (UTC) shapes all of it. No grant binds to a program's name. An owner's
`grant` line may give installed bytes any capability the progenitor endows, `reboot` and the
installer included, and several programs may hold the same one. The image's own programs keep
their grants through the measured archive and stay the default.

## What exists instead

Five parts, and none of them is a table of programs.

1. Each program declares its own manifest. A program's source carries a manifest constant and
   places it with `capability_manifest_note::carry!`, as `least_authority_demo`, eight programs in
   `fixtures/` and three in `std_exerciser/` do today. `reboot`, `jig` and `graphical_terminal`
   do the same. The constant travels with the program when its package leaves for its own
   repository.
2. The vouch decides what a declaration earns. For a program the image carries, the vouch is the
   archive's measurement (`measure_spawnable` refuses an archive program whose digest the measured
   table does not hold), bounded by the image's approval file. For an installed program, the
   vouch is its row in the live installed-programs table plus the owner's `grant` lines.
   `grant_plan::image_manifest` stays the one function the shell and the progenitor both call.
3. A fixed list of owner-granted kinds, keyed by manifest field. `grant_plan` holds the set of
   fields a vouch alone does not earn. Today that is the network; after the ruling it adds the reboot object,
   the sync-only capability, the installer, the catalog, the index store, the activation
   directory, console input and the display session. It names no program. It changes only when a
   new field joins `Manifest`.
4. The progenitor publishes the base list. At boot it already parses each archive program in
   `measure_spawnable`. It also reads each one's note and writes a read-only page of (name, note)
   pairs for the shell. The shell plans a bare word against that page, and `caps` previews from
   it. The progenitor reads its own copy of the note again at spawn and refuses a request that
   does not fit.
5. The wire names an archive program by name. A new request bit says "the archive entry named on
   the argument page", and the progenitor looks the name up in the archive it measured. Every
   wire id becomes a hole once its row leaves.

Fixtures register nowhere, as before: a fixture carries a note, test images pack every `[[bin]]`
(§158 (the shell's table is one macro), item 2), and release images omit the `fixtures` package.

## What is left in `grant_plan`

The planner, `spawnproto`, `Manifest` and its declaration types, the slot constants,
`image_manifest`, `Vouch`, and the list of owner-granted fields in part 3. No `Program` enum, no
`programs!` macro, no `Program::verbs` and no `Program::resolver_zone`. No role list either: which
program answers the word `reboot` is the owner's record, not nife's
([the command-name appendix](command-names.md)). Nothing in the crate keys on a program's name,
and the planner treats no program as special: it plans every line against the manifest the note
declares and the vouch admits. Ruled 2026-11-02 (UTC).

Every program carries its own record. Its `MANIFEST` note, with its `OWNER_GRANTS` note where it
has one, says what it asks for. An owner's `grant` line, or the image's approval file for the
image's own programs, says what it receives, bound to its digest.

## The owner's `grant` lines

`installed_programs::Granted` has six kinds today (`Network`, `Zone`, `Service`, `Installer`,
`Directory`, and the root zone as `Zone("")`), each one line after the rows:
`grant <digest> network`, `grant <digest> zone .`, and so on. The parser refuses the whole table
for a grant with no row, a grant out of order, a repeat or a misspelling (`installed_programs::grants`).

The end state adds one kind per owner-granted field, each spelled after its `Manifest` field. All
spellings are provisional.

| capability | `Manifest` field | line | operand |
|---|---|---|---|
| the reboot object | `reboot` | `grant <digest> reboot` | none |
| the sync-only capability | `sync` | `grant <digest> sync` | none |
| the installer endpoint | `installer` | `grant <digest> installer within <approval>` | the approval that bounds it ([the security appendix](security.md)) |
| the image's catalog | `catalog` | `grant <digest> catalog` | none |
| the index store | `indexes` | `grant <digest> indexes r`, or `indexes rw within <approval>` | the rights; writing needs an approval ([the security appendix](security.md)) |
| the activation directory | `activation_directory` | `grant <digest> activation r` | read only, the one right it has |
| console input | `console_input` | `grant <digest> console` | none |
| the display session | `display` (new) | `grant <digest> display` | none |
| the root resolver zone | the note's zones | `grant <digest> zone .`, already built | none |

The verb table needs no line. A program's `VERBS` note narrows what it holds per verb, and the
narrowing only removes. So the owner grants the union, and each line holds at most that. The
installed path reads the note once `endowed_image` stops refusing it.

The display session needs a `Manifest` field because nothing declares it today: the shell and the
progenitor test `Program::GraphicalTerminal` (`components/src/swish/running.rs`, the request
builder; `crates/system_initializer/src/spawn_loop.rs`, `Request::graphical`). Every new field also
needs a note encoding. None of the fields above has a byte in versions 1 to 3 of the descriptor
(`crates/capability_manifest_note/src/lib.rs`, the layout tables), and `decode` sets each to false
("Never from a note"). So they travel in a note type of their own, ruled 2026-11-02 (UTC).

- The type is `OWNER_GRANTS = 5` (provisional name), under the same owner as `MANIFEST` 1,
  `SUBTREE_GRANTS` 2, `VERBS` 3 and `interfaces_note::INTERFACES` 4. Like `SUBTREE_GRANTS` and
  `VERBS`, it carries its own version constant (`OWNER_GRANTS_VERSION`), encoder, decoder and
  length.
- Version 1 of its descriptor is a version word, one flag byte and one rights byte. The flag byte
  holds a bit each for `reboot`, `sync`, `installer`, `catalog`, `activation_directory`,
  `console_input` and `display`. The rights byte says the index store is forbidden (0), read only
  (1) or read and write (2). The bit order and the rights values are provisional.
- One manifest keeps one encoding. A program that asks for none of these carries no `OWNER_GRANTS`
  note, so `decode` refuses a descriptor whose flags and rights are all zero. Descriptor versions 1
  to 3 of `MANIFEST` stay as they are, and its version 4 stays free.
- A later flag bumps only `OWNER_GRANTS_VERSION`. The manifest's own layout never changes for it.
- A program with no `OWNER_GRANTS` note asks for none of these. `elf::Elf::note` answers `Ok(None)`
  for a missing note and an error for a malformed or doubled one, and the readers of `VERBS` and
  `INTERFACES` already treat `Ok(None)` as no declaration (`endowed_image`,
  `serves_declared_abi`). The shell and the progenitor read the note beside `MANIFEST` and merge it
  into one `Manifest` before `image_manifest` sees it.

The precedent is the interface-versions proposal
(design/roadmap/proposals/the-manifest-note-declares-interface-versions.md), which put the
interface list in a fourth note type rather than a descriptor version. A descriptor version is a
whole layout, and each new version must say how it composes with the zones and the service ask; a
separate note type composes with all of them by construction.

## How `Vouch` grows

Today `grant_plan::Vouch` is `Unvouched` or `Vouched { network: bool }`. The progenitor's `vouched`
in `crates/system_initializer/src/image_request.rs` sets `network` from
`installed_programs::network_granted`, and `granted_zones` reads zones apart. The spawn path reads
no other `Granted` kind.

In the end state `Vouched` carries a set: `Vouched { granted: OwnerGrants }`, one bit per
owner-granted field, with the index store's rights beside it. Zones stay apart, as today, because
they are a list. `image_manifest` then does for every field what it does for the network now. A
declared field with its grant passes. A declared field without its grant refuses the spawn and
names the missing grant (`ImageRefusal::NotGranted(kind)`, provisional; today
`NetworkNotGranted`). A grant never gives what the note did not ask for. The blanket refusals in
`image_can_carry` for `reboot`, `sync`, `installer`, `catalog`, `indexes`, `activation_directory`
and `console_input` go, and so do the matching compile-time refusals in
`capability_manifest_note::encode`.

For the image's own programs the vouch is the archive: `Vouch::Archive`, provisional, earns every
owner-granted field the note asks, because the composition already checked the note against the
image's approval file ([decision 4](decisions.md)).

Refusing rather than narrowing is the restrictive choice, and the tree already makes it for the
network. A program that asked for `reboot` and silently ran without it would fail later and further
from the cause.

## Where a grant lives, and what binds it to bytes

A grant line lives in the generation, beside the rows, and binds to the digest alone
(`installed_programs::Grant` holds a digest and a kind, no package or version). Rollback restores
an older generation whole, grants included. Removal drops a grant with the last row of its digest
(`installed_programs::without_package`). The progenitor checks only the digest at spawn.

Package identity enters in one place: the approval file of milestone 911 (scripted installs grant
only what the owner's approval file allows). Its entries are by package name, and its `update`
answer covers new versions of that package (`installed_programs::covers` compares no digest). So
"per package identity where the vouch is by package" already has a home: an approval entry names
the package, and every install under it writes per-digest lines.

The end state keeps both halves. The progenitor never resolves a package name at spawn, which keeps
§197 (a package is one archive file)'s rule that the vouch covers the hashed bytes. A grant on a
name would follow whatever bytes later took the name; a grant on a digest cannot.

## Upgrades and §195 sources

A new version has new digests, and nothing carries an old digest's grants to a new one. Today
`installed_programs::with_entries_granting` writes the new digest's grants from its own note, and
`jig install` asks the owner again for the whole package before it sends anything (`say_asks`,
`owner_says_yes`). The old version's lines stay with its rows.

The end state keeps that rule: every new digest asks again, at the console, unless an approval entry
with `update` covers it. Under an approval, a new version may hold only what the entry allows, and
[the security appendix](security.md) keeps the console-class grants out of every delegated install. So a
repository the owner opted into (§195 (a reviewed recipe vouches for a package)) can ship a new
`acme-reboot`, and the owner's scripted update grants it `reboot` again only if the entry says
`reboot`. A source vouches for bytes; it never grants. A key's ceiling under §220 (signed builds: a
vendor signs, a developer self-signs) covers `domain entropy network` and none of these, and
[decision 5](decisions.md) asks to keep it that way.

## Several holders, and the built-in default

The ruling lets several programs hold one capability. The code already allows it for most:
`Wants::of` in `spawn_loop.rs` places the reboot object for every job that declares it, and four
installer holders may run at once (`INSTALLER_HOLDERS`). [The security appendix](security.md) gives
each grant's behavior with two holders and where it must stay exclusive.

The image's `reboot`, `jig` and `graphical_terminal` keep their grants through the archive. Removing
or rolling back an installed replacement removes its grants with its rows, and the built-in answers
again. Nothing installed can take a capability away from a built-in.

## The dependency graph

Unchanged in shape: the leaf crate question (provisional name `program_manifest`) is still open,
and the design works either way. Without it, each tool links `grant_plan` for `Manifest` and the
slot constants, and `mdr` gains an edge to the planner to carry its note. With it, no tool links
the planner, and `process-tools` and `core-tools` stop waiting for `grant_plan` in the
contract-crate order. The ruling adds no edge: the grant kinds live in `installed_programs`, which
`grant_plan` does not link, and `OwnerGrants` lives in `grant_plan`.

## Checked against the rulings

| ruling | how the end state meets it |
|---|---|
| §158 (the shell's table is one macro) | item 3's macro goes away whole, which amends the section; item 4 (ids never reused) holds as holes |
| §197 (a package is one archive file) | unchanged: the manifest lives in the hashed bytes, and grants bind to the digest |
| §219 (how the shell names an installed program to the spawner) | unchanged: unvouched bytes earn the clock and configuration pages and nothing else |
| §220 (signed builds: a vendor signs, a developer self-signs) | unchanged: a key's ceiling covers no owner-granted kind beyond the network ([decision 5](decisions.md)) |
| §229 (a bare name reaches an installed program) | unchanged by the grant; the word table in [the command-name appendix](command-names.md) needs its own ruling ([decision 6](decisions.md)) |
| §251 (restarting the machine is a kernel object) | met: clause 2 endows any job whose declaration the archive or an owner's line vouches for |
| §270 (a package manager holds an installer endpoint) | met, with the restriction in [the security appendix](security.md) that an installed holder is always bounded by an approval |
| §316 (a package's programs leave as one repository) | required: no manifest stays behind in nife |
| milestone 892 (a std program at the prompt holds the network and a resolver) | met: the root zone is already a grant line; `Program::resolver_zone` goes |
| milestone 990 (the base image's first installed-programs table) | gives each base program a row; the end state vouches base programs by the measurement, which test images also have |
