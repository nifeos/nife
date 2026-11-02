# Leaving the central program table: the decisions, in order

[The proposal](../leaving-the-central-program-table.md) holds the end state and the order. These
are the questions only the architect can answer, one at a time, in the order they block
[the migration](migration.md). Each gives its background, the recommendation, the alternatives,
an example and what was not verified, and ends in one question. Prior art here is from memory and
not re-checked. Steps 1 to 3, 7 and 8 wait on none of them.

## What the ruling of 2026-11-02 settled

Of the five decisions the first version of this proposal asked, the ruling dissolves one and changes
two.

- Dissolved: whether taking the screen and keyboard is pinned or an owner's grant. The ruling made
  it an owner's grant. What remains is mechanism: a `display` field and a live-session guard
  (migration steps 4 and 6), which decision 1 and [the security appendix](security.md) cover.
- Changed: "every program carries its own record, with a central table kept only for what the
  system fixes" loses its second half. No table remains (decision 3).
- Changed: the base image's approval file must now list the owner-granted kinds too (decision 4).
- Unchanged: what §229 means by a name the image carries (decision 7), and provisional names
  (decision 8).

Two-way doors the maintainer may decide, each provisional: the order rows leave in, a generic
sentence for a register answer, `SPAWN_NOT_IN_IMAGE` for a removed id, pinning all 28 ids, and
keeping the upgrade rule as it is (every new digest asks again unless an approval's `update`
covers it). That last one is today's behavior, and it is the restrictive choice.

## 1. May the manifest note carry the owner-granted fields?

One-way: a wire format programs carry in their bytes.

A program states what it needs in a small record inside its executable, the note. The format has
three versions, and none has room for the reboot object, the sync-only capability, the installer,
the catalog, the index store, the activation directory, console input or the display. They live
today only in the central table, which only nife edits. The ruling lets an owner grant them to an
installed program, but an installed program cannot yet ask for them.

Recommendation: A, version 4, the 56 bytes of version 1 and the zones of version 2, plus one flag
byte (a bit each for `reboot`, `sync`, `installer`, `catalog`, `activation_directory`,
`console_input` and `display`) and one byte for the index store's rights. One manifest keeps one
encoding: a manifest that asks for none of these stays version 1, 2 or 3.

Alternatives: B, one byte per field, as version 1 does; clearer to read, eight bytes instead of two,
and the tree's own precedent. C, a separate note type for owner-granted asks; it would split one
manifest across two notes the progenitor must agree on.

Example: `acme-reboot` declares `reboot` and `sync`; its note is version 4 with two bits set. A
program that asks for the network only is still version 1.

Prior art: ELF notes in general version their descriptor and leave old readers to refuse new
versions; this tree does the same.

Not verified: how version 4 composes with version 3's service ask; the layout must carry both.

Question: approve a version 4 note with a flag byte (A), or one byte per field (B)?

## 2. Which grants may an installed program hold only within limits?

One-way: it sets what an owner's yes can hand to bytes nobody reviewed.

Two of the newly grantable capabilities can make more grants. The installer writes the owner's
grants for whatever it installs, on its own word that the owner said yes. The index store's write
right decides which bytes the installer accepts by name. A holder of either can turn one yes into
every grant. Console input and the display can read what the owner types.

Recommendation: A, three classes, as [the security appendix](security.md) sets out. Shareable
grants (`reboot`, `sync`, catalog, activation directory, zones) go to any installed bytes on an
owner's line. Exclusive grants (console input, display) go only to a foreground job at the
owner's console, one holder at a time. Console-class grants (installer, the index store's write
right) go to an installed program only in the delegated form of §270 (a package manager holds an
installer endpoint, not the spawn endpoint): bounded by an approval, no `answered console`, no
`Approve`, `Revoke` or `Vouch`, its own index directory, and no installing of an exclusive or
console-class ask. No recorded service may hold `reboot` or either class.

Alternatives: B, every grant on an owner's line with no class limits; the ruling's widest reading,
and it makes a replacement manager a second owner. C, refuse the console class to installed bytes
outright; it fails the ruling's goal of a third-party package manager.

Example: under A, `acme-pkg` with `grant <digest> installer within acme` installs and upgrades the
packages the `acme` approval names, and `jig install` refuses its attempt to install a package that
asks for `display`.

Prior art: Android grants `INSTALL_PACKAGES` to no third-party app and lets others ask the system
installer, which may stop for the user. Nix `trusted-users` shows the danger of B: a trusted user
can add a substituter, and with it, trust.

Not verified: whether a per-manager index directory breaks `jig list`'s reading of the one copy.

Question: A (three classes), B (no limits) or C (no console class for installed bytes)?

## 3. Does every program carry its own grant record, with no central table at all?

One-way: it changes who vouches for a base tool, adds a page format and a wire bit.

Today the shell starts 28 programs by name from a table inside the planner crate, and that table
decides what each may hold. The first version of this question kept a small table for `reboot`,
`jig` and perhaps the terminal. After the ruling no grant belongs to a name, so nothing needs to stay
central.

Recommendation: yes. Every program, the image's own included, carries its own record. The system
builds a one-page list of the image's programs from those records at boot, and the shell asks for
a program by name. It costs a page the shell reads, a request bit, and amending item 3 of §158 (the
shell's table is one macro).

Alternatives, from [the end state](end-state.md): a registry in the installed-programs table (a
second copy of each record outside the trusted bytes), a table generated at build time (nife would
build every package to produce it), or the image composer writing the list into the image.

Example: a person types `reboot`. The shell finds it on the boot list, sees the record asks for the
reboot object, and asks for the program named `reboot`. The system finds it in the measured image
and endows the object because the measurement vouches.

Not verified: the list's size for a full base image (estimated under a page) and the boot cost of
reading every record.

Question: every program carries its own record, and nothing central remains. Yes or no?

## 4. Must the image's approval file list every grant the system hands out?

One-way: it sets what vouches for a base tool.

When the composer builds an image, it checks each package's records against an approval file. The
file lists the network, resolver zones, the service, named directories and the installer today. A
record can also ask for the process view, randomness, the machine counters, and after decision 1
the reboot object and the rest. Under decision 3 the measured archive would vouch for whatever a
base package's record asks.

Recommendation: yes. Each base package's approval entry names every grant its programs ask,
`domain`, `entropy`, `machine`, `share` and the owner-granted kinds included. A `process-tools`
update that suddenly asks for `reboot` fails composition until a reviewed line allows it.

Alternative: trust the record, as for the progenitor-endowed grants of an installed program today.

Example: basalt's approval file says `init` may hold `reboot` and `sync`, `jig` the installer,
catalog, index store, activation directory and console input, and `display` the display session.

Not verified: whether the interactive install should list `domain`, `entropy`, `machine` and `share`
too; a separate question.

Question: should the base approval list every grant the system endows? Yes or no.

## 5. May a trusted repository key ever grant these without the owner?

One-way once a key's ceiling ships in an image.

§220 (signed builds: a vendor signs, a developer self-signs) lets an owner trust a key with a
ceiling, a subset of `domain entropy network`, and an install under that key earns up to the
ceiling with no prompt. §195 (a reviewed recipe vouches for a package) leaves open how a ceiling
covers grants added since.

Recommendation: never. A key's ceiling stays within `domain entropy network`, and every
owner-granted kind beyond the network needs the owner's own line, at the console or in an approval.
A source vouches for bytes; only the owner grants.

Alternative: let a ceiling name any owner-granted kind, so a trusted vendor can ship a package
manager that installs with no prompt.

Example: a trust line for acme's key with ceiling `network` lets `acme-power` install with the
network and no prompt; its `reboot` ask still stops for the owner's yes.

Not verified: no key trust exists in code yet (`installed_programs` has no `trust` line).

Question: may a key's ceiling include owner-granted kinds? Recommended no.

## 6. May the owner point a word the image carries at an installed program?

One-way once scripts rely on what `reboot` means.

§229 (a bare name reaches an installed program) gives every bare word one owner and keeps the
image's program on the word, so a local program runs by path. It refused installs and vouches that
claim a name. [The command-name appendix](command-names.md) proposes an owner's word table: lines in
the generation, written only at the owner's console, mapping a word to an installed program.

Recommendation: yes. An explicit line the owner writes is a third case §229 did not consider, and it
is the only way typing `reboot` reaches a replacement. Rollback or `jig use --default` restores the
image's program.

Alternative: the word table serves only words the image does not carry; `acme-reboot` answers to
its own name and to any new word, never to `reboot`.

Example: under yes, `jig use reboot acme-reboot`, then `reboot` runs the replacement. Under no,
`jig use restart acme-reboot` works and `jig use reboot acme-reboot` is refused.

Prior art: Debian's `update-alternatives` lets the administrator repoint `editor` at any provider.

Not verified: no syntax reaches an image program by path while a word overrides it.

Question: may an owner's word line override an image name? Yes or no.

## 7. Which bare names belong to the image, once the table no longer says?

One-way: it conflicts with §229's present definition.

Install refuses a package whose program has a name the image carries, and the prompt refuses a name
that is both. Today "a name the image carries" means a row of the central table
(`installed_programs::refuse_entry`'s caller, `Program::from_name`). §229's amendment narrows the
install refusal to what a boot slot holds, and that narrowing is unbuilt.

Recommendation: A. The measured archive's names, with both refusals unchanged until the narrowing
is built. Nothing a person sees changes.

Alternatives: B, build the narrowing now, so a newer `core-tools` package may install `date`. C,
keep the table's names, which keeps a fragment of the table alive.

Example: under A, `jig install core-tools` still refuses while the image carries `date`.

Question: A (the archive's names) or B (build the narrowing now)?

## 8. The new names

Two-way in effect: each ships provisional. The base list page, the by-name request bit,
`OwnerGrants`, `Vouch::Archive`, `NotGranted`, the `display` field, each grant line's spelling, the
`word` line and `jig use` need names. Recommendation: ship each provisional and bring them in one
naming pass once steps 5 and 12 exist. Question: may they ship provisional? Yes or no.

## The leaf crate, already before calef

The split of `Manifest`, its declaration types and the slot constants into a leaf crate (provisional
name `program_manifest`) stays a separate open decision. The end state works without it.
