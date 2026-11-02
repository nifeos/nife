# Leaving the central program table: the decisions, in order

[The proposal](../leaving-the-central-program-table.md) holds the end state and the order. These
are the questions only the architect can answer, one at a time, in the order they block work. Each
opens with its background, then the recommendation, the alternatives, an example and what was not
verified. Each ends in one question. Steps 1 to 3 of [the migration](migration.md) wait on none of
them.

Two-way doors the maintainer may decide, each recorded as provisional: the order rows leave in
(fixtures first), the generic sentence for a register answer, answering `SPAWN_NOT_IN_IMAGE` for an
id whose row has left, and extending the wire-id pin to all 28 ids.

## 1. Does each program declare its own grants, with only the system's own grants kept central?

One-way: it changes who vouches for a base tool, adds a page format and adds a wire bit.

Today the shell can start 28 programs by name, and a table inside the planner crate decides what
each may hold. Adding a tool, or changing what one needs, edits that table. A package that leaves
for its own repository would still need a commit in nife whenever its programs' needs change.

Installed programs already work another way. Each carries a small record of its needs inside its
own executable, and the system grants that record only when the bytes are trusted: listed by the
owner at install, or measured as part of the image. 26 of the 28 table rows can already be written
as such a record; the two that cannot are `reboot` and `jig`, whose grants no installed program may
hold.

So the question is whether every program, the image's own included, should carry its own record,
with the central table kept only for those two (or three; see question 4). The shell would learn
the image's programs from a one-page list the system builds at boot from those records, and would
ask for a program by name. What it costs: a new page the shell reads, a new way to ask for a
program, and an amendment to §158 (a program is declared once)'s item 3, which made the table one
macro.

Recommendation: yes. Alternatives, each in [the end state](end-state.md): (b) a registry kept in
the installed-programs table, refused because it copies each record outside the trusted bytes; (c)
a table generated at build time, refused because nife would have to build every package to produce
it; (d) a test-only table for fixtures, unnecessary once fixtures carry records. For the list
itself: the system builds it at boot (recommended), the image composer writes it into the image,
the shell reads the whole image archive itself, or the shell asks per command.

Example: a person types `date`. The shell finds `date` on the boot list, sees it asks for the clock
and a second output stream, binds the line, and asks for the program named `date`. The system finds
`date` in the measured image, reads its record again, and grants the clock.

Not verified: the size of the list for a full base image (estimated under a page), the boot cost of
reading every record, and whether any program outside the tree spawns by id.

Question: should every program carry its own grant record, with the central table kept only for
grants the system fixes itself? Yes or no.

## 2. Which bare names belong to the image, once the table no longer says?

One-way: it conflicts with an existing ruling, and it decides which names a package may claim.

§229 (a bare name reaches an installed program) gives every bare word one owner. Install refuses a
package whose program has a name the image carries, and the prompt refuses a name that is both.
Today "a name the image carries" means "a row of the central table"
(`crates/system_initializer/src/activation.rs`, the `image` check). §229's amendment narrows the
install refusal to the programs a boot slot still holds, the progenitor and the root supervisor,
and that narrowing is unbuilt. §241 (a threadbare base) says every other program is a package.

Recommendation: A. "The image carries a name" means the measured archive holds that name, and both
refusals keep their present force until the amendment's narrowing is built as its own work. This
changes nothing a person sees today and keeps a package from silently claiming `date`.

Alternatives: B, build the amendment now, so a newer `core-tools` package may install `date` and
the bare word runs the installed one; C, keep the table's names as the definition, which leaves a
fragment of the table alive for this alone.

Example: under A, `jig install core-tools` on a running image is refused while the image carries
`date`, exactly as today.

Not verified: how B would order an installed `date` against the image's, which the amendment does
not say.

Question: A (the archive's names, refusals unchanged) or B (build the narrowing now)?

## 3. Must the image's approval file list every grant the system hands out, not only the owner's?

One-way: it sets what vouches for a base tool.

When an image is composed, each package's records are checked against an approval file. The file
lists only four kinds of grant today: the network, name-resolution zones, running as a service, and
named directories. The other grants a record can ask for, the process view, randomness and the
machine counters, come with trust alone. Today the central table holds those, so a reviewed nife
commit decides them. Under question 1 a base package's own record would decide them.

Recommendation: yes. Restrictive first: an approval entry also names `domain`, `entropy`, `machine`
and `share` for each base package that asks, so a package update cannot widen what `ps` sees without
a reviewed line. Loosening later stays compatible; tightening later would not be.

Alternative: no, trust the record as installed programs are trusted today. This matches the
interactive install, which asks the owner only about the four.

Example: a later `process-tools` whose `pgrep` asks for `entropy` fails composition until its
approval entry says `entropy`.

Not verified: whether the interactive install should follow suit; it is a separate question.

Question: should the base approval list every grant the system endows? Yes or no.

## 4. Is taking the screen and keyboard a grant the system fixes, or one an owner may give?

One-way: it is a security boundary.

`graphical_terminal` declares nothing a record cannot carry. Its power comes from its name: the
shell and the system build a display session with the keyboard for it because of which program it
is. Holding the keyboard is close to reading the owner's console, which only the owner's console
grants.

Recommendation: A, fixed by the system, a third pinned row, until a second program needs a
display. Alternative: B, a new ask an owner grants at install, like the network, which needs a
record field and a wire change.

Example: under A, an installed terminal program cannot take the screen however it is vouched.

Question: A (pinned) or B (an owner's grant)?

## 5. The new names

Two-way in effect: each name ships provisional and can be ratified later. The base list page, the
by-name request bit, the pinned table, and whether the shrunken enum keeps the name `Program` all
need names. The recommendation is to ship each as provisional and bring them in one naming pass
once steps 4 to 7 are built. Question: may they ship provisional? Yes or no.

## 6. The leaf crate, already before calef

The split of `Manifest`, its declaration types and the slot constants into a leaf crate
(provisional name `program_manifest`) is a separate open decision; this proposal does not decide
it. The end state works without it: the table still leaves, but the tools keep linking
`grant_plan`, and `mdr` must take a new edge to the planner to carry its record. With it, the tools
shed the planner and `process-tools` and `core-tools` stop waiting for `grant_plan` in the
contract-crate order. [The end state](end-state.md) draws both graphs.
