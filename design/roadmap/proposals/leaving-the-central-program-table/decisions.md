# Leaving the central program table: the decisions, in order

[The proposal](../leaving-the-central-program-table.md) holds the end state and the order. These
are the questions only the architect can answer, one at a time, in the order they block
[the migration](migration.md). Each gives its background, the recommendation, the alternatives,
an example and what was not verified, and ends in one question. Prior art here is from memory and
not re-checked. Steps 1 to 9 and 11 wait on none of the open ones, decisions 6 to 8.

## What the ruling of 2026-11-02 settled

Of the five decisions the first version of this proposal asked, the ruling dissolves one and changes
two.

- Dissolved: whether the system fixes the screen and keyboard or an owner grants them. The ruling made
  it an owner's grant. What remains is mechanism: a `display` field and a live-session guard
  (migration steps 4 and 6), which [the end state](end-state.md) and [the security
  appendix](security.md) cover.
- Changed: "every program carries its own record, with a central table kept only for what the
  system fixes" loses its second half. No table remains (decision 3, ruled).
- Changed: the base image's approval file lists the owner-granted kinds too (decision 4, ruled).
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

## 2. Ruled: three classes of owner grant

Ruled 2026-11-02 (UTC): shareable, exclusive and console class, as [the security
appendix](security.md) sets out, with no recorded service holding `reboot` or either of the other
two classes.

## 3. Ruled: every program carries its own record, and nothing central remains

Ruled 2026-11-02 (UTC). [The end state](end-state.md) says what `grant_plan` keeps.

## 4. Ruled: the base approval file lists every grant the image makes

Ruled 2026-11-02 (UTC). Nothing comes from the measured archive implicitly; [the end
state](end-state.md) says how the composition turns the file into `grant` lines.

## 5. Ruled: a repository key never grants an owner-granted kind

Ruled 2026-11-02 (UTC): option A. [The end state](end-state.md) says what that means for upgrades
and for the key-line parser of milestone 666 (a signed build installs up to its key's ceiling).

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
`OwnerGrants`, `NotGranted`, the `display` field, each grant line's spelling, the
`word` line and `jig use` need names. Recommendation: ship each provisional and bring them in one
naming pass once steps 5 and 12 exist. Question: may they ship provisional? Yes or no.

## The leaf crate, already before calef

The split of `Manifest`, its declaration types and the slot constants into a leaf crate (provisional
name `program_manifest`) stays a separate open decision. The end state works without it.
