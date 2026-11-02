# Leaving the central program table: how a typed word finds a program

[The proposal](../leaving-the-central-program-table.md) holds the end state. This appendix answers
one question the ruling of 2026-11-02 (UTC) raises. An owner installs `acme-reboot` from a
repository they added, and grants it the reboot object. How does typing `reboot` at the prompt run
it, when several programs may offer the same function and nife names none of them? The answer
stays apart from the grant: a word decides which program runs, and only the program's own grant
lines decide what it holds.

## What the shell does today

Read from `components/src/swish/` and `crates/swish/src/bare.rs`.

1. A builtin wins. `grant_plan::parse` matches `BUILTINS` (`apropos`, `bg`, `bind`, `caps`, `cd`
   and the rest) before any program, and `Program::from_name` must never learn a builtin's name.
2. A word with a `/` is a path. `run_image` reads the file and sends its bytes.
3. A bare word goes to `capabilities::bare`, which asks `swish::bare::resolve` with three facts:
   whether the image carries the name (`Program::from_name`), the live installed-programs table,
   and the nearest `.jig-versions` file.
   - `name@version` picks that installed version.
   - Otherwise `installed_programs::lookup` follows the `default <program> <digest>` pointer to a
     package row of that name. Vouch rows never answer a name.
   - Installed only: run it by path. Image only: plan it from the table. Both: refuse, naming the
     installed path (`write_both`), as §229 (a bare name reaches an installed program) requires.
4. On the wire an image program travels as its id and an installed one as its bytes; the
   progenitor knows installed bytes by digest only.

No alias mechanism exists. `bind` maps paths in the namespace, not commands, and the shell reads no
start-up file of commands. Install refuses a package whose program has the image's name
(`installed_programs::refuse_entry`, `Error::ImageName`) or another package's (`Error::Taken`).

So under today's rules a third-party `acme-reboot` installs and runs as `acme-reboot`. The word
`reboot` keeps meaning the image's program, and a package named `reboot` cannot install.

## The proposal: the owner's word table

A word table the owner writes, in the generation, read by the shell before the image's names.

- The record. One line per word in the generation, after the pointers:
  `word <word> <program>` (provisional spelling), meaning "the bare word runs the installed program
  this name's default pointer selects". It names a program, not a digest, so an upgrade of
  `acme-reboot` keeps the word. The parser holds it to the table's rules: one line per word, sorted,
  and a target that names no package row makes the table unreadable, as a dangling `default` does.
- Who writes it. Only the owner's console, through a new activation verb on the console's own
  installer endpoint (`jig use reboot acme-reboot`, provisional). A delegated holder may not send
  it, so no package manager and no package can repoint a word. Removing a package drops every
  word line that targets it, as removal drops its grants.
- How the shell reads it. `bare::resolve` checks the word table first. A hit resolves the target
  through its default pointer and runs it by path, with no `Both` refusal, because the owner chose
  it by name. A miss continues exactly as today.
- Several providers. One word has one target. Every provider stays reachable by its own name, and
  by path. The owner moves the word with another `jig use`, and `jig use --default reboot`
  (provisional) removes the line, so the image's program answers again.
- The fallback. A word whose target the owner removed has no line, because removal took it. A
  rollback restores the generation whole, so the word and the grant it relied on return or leave
  together.

The table grants nothing. If `reboot` points at a program the owner never granted the reboot
object, the shell runs it, and the progenitor refuses the spawn naming the missing grant
(`ImageRefusal::NotGranted`, provisional; see [the end state](end-state.md)). The two records stay
independent: the grant binds a capability to a digest, and the word binds a name to a program.

Example:

```text
$ jig install acme-power        # the owner types acme-reboot to grant it
$ jig use reboot acme-reboot
  reboot now runs acme-reboot (from acme-power 2.1.0); jig use --default reboot restores the image's
$ reboot
  acme-reboot: synced 14 blocks; restarting
```

## Why this shape

- In the generation, not in a file of its own. Rollback already restores a generation whole, the
  parser is already strict, and the shell already reads the live generation for every bare word.
  A separate file would need its own rollback and its own reader.
- Written by the owner's console only. A word is a claim on what the owner types, which is the same
  authority §229 refused to installs and to vouches. Only the owner may make it.
- A program, not a digest. The owner chooses a provider, not a build; the provider's upgrades keep
  the word, and its grants still need the owner's yes per digest.
- Nothing in nife. The image carries no list of roles and no word names a capability. A role such
  as "the reboot program" exists only as the owner's line.

## What was considered

| option | why it lost |
|---|---|
| A role list in `grant_plan` (`reboot`, `package-manager`, `terminal`) | nife would name the functions a program may serve, which the ruling refuses |
| Let `default <program> <digest>` point across names | the pointer's meaning, "which version of this program", would also become "which program", and §229's `Both` refusal reads it today |
| An `alias` builtin and a start-up file per session | right for a person's shortcuts, wrong for the machine's `reboot`: each session would decide, and a login session's file is not the owner's |
| Let a package install a program named `reboot` | the package would claim the word by installing, which §229 refuses for good reason; the owner did not choose it |
| Debian `update-alternatives` priorities | a priority is a search order, which §229 (a bare name reaches an installed program) refused as B3 |

Prior art, from memory and not re-checked: Debian's `update-alternatives` binds a generic name
through `/etc/alternatives/` to one provider, chosen by the administrator or by priority. Android's
`RoleManager` lets the user choose which app holds an exclusive role such as the default dialer,
and grants the role's permissions with it. This proposal takes the administrator's explicit choice
from Debian and the owner as chooser from Android, and refuses both the priority and the coupling
of role and permission. [The prior-art appendix](prior-art.md) has the table.

## What needs a ruling

The word table lets the owner's chosen program answer a word the image also carries. §229 ruled
that the system's program keeps a bare name and a local one runs by path (its B2), and refused
installs and vouches that claim one. An owner's explicit word line is a third case §229 did not
consider. [Decision 6](decisions.md) asks whether it may override an image name. Without it, the
word table still serves every word the image does not carry, and `acme-reboot` answers to its own
name.

## Not verified

- An image program has no path today: `bare` answers `Both` with "the image's is not reachable by
  this name". So while a word line overrides an image name, the image's program runs only after
  `jig use --default`. A spelling that always reaches the image's program (an archive path, or a
  prefix) would close that, and nothing proposes one yet.

- No rc or profile reader exists in `components/src/swish/`; the search grepped for one and did
  not read `main.rs` whole.
- Whether tab completion (`components/src/swish/prompt.rs`) should offer word lines; it offers
  builtins, `Program::ALL` and installed defaults today.
- The cost of one more lookup per bare word; `bare` already reads the live table, so it should be
  a scan of lines it already holds.
