# Leaving the central program table: the decisions, in order

[The proposal](../leaving-the-central-program-table.md) holds the end state and the order. These
are the questions the migration needed answered, in the order they block
[the migration](migration.md), each with its ruling. Decisions 1 to 7 are the architect's, and
decision 8 is the maintainer's. Only the leaf crate stays open.

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
- Unchanged: §229 (a bare name reaches an installed program)'s refusals, now read against the
  measured archive (decision 7, ruled).

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

## 6. Ruled: no override of a word the image carries

Ruled 2026-11-02 (UTC): §229 (a bare name reaches an installed program) stands. A bare word the
image carries runs the image's program, and an installed program runs under its own name or by
path. [The command-name appendix](command-names.md) says how a word finds a program.

## 7. Ruled: the image's names are the measured archive's

Ruled 2026-11-02 (UTC): option A. "A name the image carries" means a name in the measured archive.
That is one source, the archive the boot already measures, and it covers every image program
whether or not it holds a grant. It does not depend on the grant records. Install refuses a package
that takes such a name, and the prompt runs the image's program for such a bare word.
[The command-name appendix](command-names.md) says how each side learns the names.

## 8. Decided: the new names ship provisional

Decided by the maintainer, provisional; reversing costs one pull request. Every name this proposal
introduces ships provisional, and its own name review ratifies it. The names are `OWNER_GRANTS`,
the new `Granted` kinds and their grant-line spellings, and the note's flag bits and rights
encoding. So are `OwnerGrants`, `NotGranted`, the `display` field, the base list page and the
by-name request bit.

## The leaf crate, already before calef

The split of `Manifest`, its declaration types and the slot constants into a leaf crate (provisional
name `program_manifest`) stays a separate open decision. The end state works without it.
