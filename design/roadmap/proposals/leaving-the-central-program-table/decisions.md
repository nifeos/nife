# Leaving the central program table: the decisions, in order

[The proposal](../leaving-the-central-program-table.md) holds the end state and the order. These
are the questions only the architect can answer, one at a time, in the order they block
[the migration](migration.md). Each gives its background, the recommendation, the alternatives,
an example and what was not verified, and ends in one question. Prior art here is from memory and
not re-checked. Steps 1 to 4, 7 and 8 wait on none of the open ones.

## What the ruling of 2026-11-02 settled

Of the five decisions the first version of this proposal asked, the ruling dissolves one and changes
two.

- Dissolved: whether the system fixes the screen and keyboard or an owner grants them. The ruling made
  it an owner's grant. What remains is mechanism: a `display` field and a live-session guard
  (migration steps 4 and 6), which [the end state](end-state.md) and [the security
  appendix](security.md) cover.
- Changed: "every program carries its own record, with a central table kept only for what the
  system fixes" loses its second half. No table remains (decision 3).
- Changed: the base image's approval file must now list the owner-granted kinds too (decision 4).
- Unchanged: what §229 means by a name the image carries (decision 7), and provisional names
  (decision 8).

Five two-way doors the maintainer may decide, each provisional. They are the order rows leave in,
a generic sentence for a register answer, `SPAWN_NOT_IN_IMAGE` for a removed id, and pinning all
28 ids. The fifth keeps the upgrade rule as it is: every new digest asks again unless an
approval's `update` covers it. That last one is today's behavior, and it is the restrictive choice.

## 1. Ruled: the owner-granted asks travel in a note type of their own

Ruled 2026-11-02 (UTC): a fifth note type, `OWNER_GRANTS = 5`, with its own version, encoder,
decoder and length; [the end state](end-state.md) has its layout. Descriptor versions 1 to 3 stay
as they are.

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
installer endpoint, not the spawn endpoint). That form holds to an approval, with no `answered console`, no
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

Alternatives, from [the end state](end-state.md). A registry in the installed-programs table keeps
a second copy of each record outside the trusted bytes. A table generated at build time makes nife
build every package. The image composer could also write the list into the image.

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
record can also ask for the process view, randomness, the machine counters, and through the
`OWNER_GRANTS` note
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
`jig use restart acme-reboot` works and the progenitor refuses `jig use reboot acme-reboot`.

Prior art: Debian's `update-alternatives` lets the administrator repoint `editor` at any provider.

Not verified: no syntax reaches an image program by path while a word overrides it.

Question: may an owner's word line override an image name? Yes or no.

## 7. Which bare names belong to the image, once the table no longer says?

One-way: it conflicts with §229's present definition.

Install refuses a package whose program has a name the image carries, and the prompt refuses a name
that is both. Today "a name the image carries" means a row of the central table
(`installed_programs::refuse_entry`'s caller, `Program::from_name`). §229's amendment narrows the
install refusal to what a boot slot holds, and that narrowing is unbuilt.

Recommendation: A. The measured archive's names, with both refusals unchanged until a lane builds
the narrowing. Nothing a person sees changes.

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
