# Leaving the central program table: how a typed word finds a program

[The proposal](../1027-leaving-the-central-program-table.md) holds the end state. This appendix says how
a word typed at the prompt finds a program, and why that stays apart from what the program holds.
A name decides which program runs. Only the program's own `grant` lines decide what it holds.

## What the shell does

Read from `components/src/swish/` and `crates/swish/src/bare.rs`.

1. A builtin wins. `grant_plan::parse` matches `BUILTINS` (`apropos`, `bg`, `bind`, `caps`, `cd`
   and the rest) before any program, and no program may take a builtin's name.
2. A word with a `/` is a path. `run_image` reads the file and sends its bytes.
3. A bare word goes to `capabilities::bare`, which asks `swish::bare::resolve` with three facts:
   whether the image carries the name, the live installed-programs table, and the nearest
   `.jig-versions` file.
   - `name@version` picks that installed version.
   - Otherwise `installed_programs::lookup` follows the `default <program> <digest>` pointer to a
     package row of that name. Vouch rows never answer a name.
   - Installed only: run it by path. Image only: run the image's program. Both: refuse, naming the
     installed path (`write_both`).
4. On the wire an image program travels as its id today and an installed one as its bytes; the
   progenitor knows installed bytes by digest only.

No alias mechanism exists. `bind` maps paths in the namespace, not commands, and the shell reads no
start-up file of commands.

§229 (a bare name reaches an installed program) governs the rest. A bare word the image carries
runs the image's program. Install refuses a package whose program takes an image's name
(`installed_programs::refuse_entry`, `Error::ImageName`) or another package's (`Error::Taken`). An
installed program with an image's name could only come from a later image adding the name; the
prompt then refuses the word and names the installed path.

## Several providers of one function

Each provider runs under its own name. An owner who installs `acme-reboot` from a repository they
added, and grants it the reboot object, types `acme-reboot` to run it. The image's `reboot` keeps
the word `reboot`, and keeps its own grants. A second provider, `other-reboot`, installs beside
both and holds the reboot object too if the owner grants it. Nothing in nife names any of them, and
nothing ranks them.

Example:

```text
$ jig install acme-power        # the owner types acme-reboot to grant it
$ acme-reboot
  acme-reboot: synced 14 blocks; restarting
$ reboot                        # after the restart: the image's program, as before
```

## A grant is independent of the name

A `grant` line binds a capability to a digest (`installed_programs::Grant`). The name a person
types never reaches the progenitor for installed bytes, and the progenitor checks grants by digest
alone. So the same bytes reached by a bare word, by a path or under a second package's name hold
the same grants. Renaming a program grants nothing and revokes nothing. A table mapping words to
programs could be added later without touching the grant records.

## Where the image's names come from

A name the image carries is a name in the measured archive ([decision 7](decisions.md), ruled).
Two checks read it: the install refusal (`Error::ImageName`, through the `image` closure in
`activation.rs`) and the prompt's `bare`. Both ask `Program::from_name` today, which is the central
table. In the end state the progenitor reads its own measurement table, the `<name> <digest>` lines
`measured_boot::verdict` checks every archive load against. The shell reads the base list page the
progenitor publishes from the same archive (migration step 9).

Builtins keep their own rule. `grant_plan::parse` matches them before any program, so a program
under a builtin's name could never run by its bare word. Today
`every_listed_builtin_is_a_builtin_and_no_program` asks `Program::from_name`; without the table, a
check at image build must refuse an archive entry named like a builtin.

## Not verified

- No rc or profile reader exists in `components/src/swish/`; a search for one found none, and
  `main.rs` was not read whole.
- An image program has no path today: `bare` answers `Both` with "the image's is not reachable by
  this name", and no spelling reaches the image's program other than its bare word.
