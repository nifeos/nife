---
status: DECIDED
raised: 2026-08-24
decided: 2026-08-25
ratified_by: calef
---

# 125. What tells boot-time re-derivation which identities have pending work

calef, 2026-08-25, in conversation, ratifying the recommendation below as
written: *"Yes."*

**The recommended shape is already built, not merely proposed.** `crates/schedule_store`,
`fixtures/src/fs_test_client.rs`'s `ROLE_SCHEDULE_SEED`, and `components/src/session_reviver.rs`
already implement exactly the manifest format below. They landed in the same pull request that
raised this decision (milestone 152's own PR, already merged). Ratifying this closes the gap between
a decision and the code, rather than authorizing new work.

It answers a gap in milestone 152 (durable delegation), pieces 2 and 3: the boot-time re-deriver and
its connection to the schedule store. Neither [§122 (the on-disk, per-user schedule
store)](0122-durable-schedule-store-format.md) nor [§123 (boot-time
re-derivation)](0123-boot-time-rederivation-privilege.md) fully specifies what tells boot-time
re-derivation which identities have a durable session with pending work at all. §122 is about one
identity's own schedule file. §123 assumes "the store names" a set of sessions to re-derive. It does
not say how that set is discovered without falling into milestone 126 (the `procps` package)'s
refusal (enumeration is itself authority).

## The question

`session_reviver` (this lane's own provisional name for §123's boot-only re-deriver) is granted a
construction budget and read access to the schedule store. It re-derives every session the store
names. What object or document is "the store names" here, concretely? How does the re-deriver learn
*which* identities to iterate, given a fixed capability into the principal tree's root and no
permission to enumerate what is under it?

## Is the premise true?

Checked directly. `crates/identity_provisioner` (milestone 155) creates a subtree per identity at
provisioning time. But a provisioned subtree and a subtree with a *pending schedule* are different
facts. Every identity gets a subtree whether or not it ever registers a job. So "walk the identities
`identity_provisioner` created" answers a different, wider question than "which identities have
pending scheduled work right now," and would re-derive sessions for principals who never asked for
one. Confirmed by reading `components/src/identity_provisioner.rs` in full. It performs one `MKDIR`
and one credential `PUT`. It produces no durable list of what it has provisioned anywhere a later
process could read. There is no existing manifest, registry, or enumeration mechanism for "who has a
schedule" anywhere in this tree today.

## What else was considered

**(a) A directory listing (`READDIR`) of the principal tree's root, treating every subtree found
there as a candidate.** Rejected on this tree's own explicit terms. DECISIONS §123 names this exact
shape and refuses it. Boot-time re-derivation must not be granted "anything that would let it
enumerate users rather than iterate a hard-wired set it was constructed to read," citing milestone
126's enumeration-is-authority rule. Beyond the refusal, it also answers the wrong question (every
provisioned identity, not every identity with pending work; see "is the premise true" above). So it
would need a second read per candidate (open that identity's `schedule` file and see if it exists)
to narrow back down to the right set. At that point the enumeration bought nothing the manifest
below does not already give more cheaply and without the refused shape.

**(b) A separate, explicit manifest file** (this decision's recommendation, below): one small
document at a fixed, well-known location. It lists exactly which identities currently have a durable
session with pending scheduled work, and is written whenever that set changes.

**(c) Fold "has pending work" into the schedule file's own existence**, i.e., no manifest at all.
Boot-time re-derivation is handed a hard-wired, compiled-in list of every identity the deployment
ever provisions. For each one it simply attempts to open that identity's `schedule` file, treating
`ENOENT` as "nothing pending" rather than a failure. Considered and rejected. This pushes the "which
identities" question out of the store and into whoever builds the re-deriver's own spawn site (a
literal list baked into `crates/system_initializer` or wherever it is wired). That does not scale
past a fixed, rebuild-to-change roster. It reintroduces exactly the coupling §117 (a principal's
subtree is named by its identity)'s per-identity subtree scheme was built to avoid (a principal's
own resources need no central table to exist). It is also strictly worse than (b) for cost: (b) is
one small read; (c) is one read *per provisioned identity*, most of which will be `ENOENT`. That is
what deployment size actually determines rather than schedule-holder count.

## What this tree already does in the analogous case

**Milestone 152's own reattachment design already answers the adjacent question this way**, and this
decision is that same answer applied one step earlier. The roadmap doc (`design/roadmap/0152-*.md`,
"Reattachment on reconnect, via a scoped lookup, not enumeration") states the identical constraint
("this cannot be built as 'list all sessions and find the match'... it has to be a targeted lookup:
given a proven identity, return the one record for it, never a list"). It answers it with a small
table the credentialer holds, identity to durable-session capability. A manifest is the on-disk
version of the same shape. It is a document naming exactly the identities that matter, read by a
party that already knows where to look, never a directory walk.

`crates/measured_boot::PROGRAM_MEASUREMENTS` is the closer structural precedent. It is exactly this
shape already built and load-bearing: one file at a fixed, well-known name in the initrd archive,
listing names and hashes. `crates/system_initializer::boot` and `login.rs` read it before either
will build anything from an archive entry. Nobody enumerates the archive to discover what exists.
The table says what is vouched for, by name, and the reader consults it. A schedule manifest is the
identical pattern one layer down: a document naming what exists, consulted rather than discovered.

## What is prior art outside the tree

systemd's own answer to "which users have pending timers" is `loginctl list-users` plus a directory
walk of `/etc/systemd/system/` and every user's `~/.config/systemd/user/`, i.e., enumeration. That
walk is gated by ordinary Unix file permissions rather than a capability boundary. That approach is
not available here on this tree's own terms (no ambient identity, DECISIONS §10 (process model), §82
(ambient authority is the problem)). It is the same gap milestone 126 already found when it named
"enumeration is authority" as a refusal rather than an oversight. cron's own
`/var/spool/cron/crontabs/` is the same shape (one file per user, discovered by listing the spool
directory) and has the identical property. It works because Unix treats directory listing as an
ordinary, ungated operation, which this tree deliberately does not.

## What each option costs

Option (b), the manifest, concretely:

- *Format*: a text document, one identity name per line, `#` comments. It matches
  `timetable::parse`'s own dialect for the same reason §122 gives for reusing that crate's document
  shape. A reader who already knows one of this tree's document formats should not have to learn a
  second one for a document this small.
- *Location*: directly under the principal tree's root, a sibling of every identity's own subtree
  rather than nested inside any one of them. The reason is that the manifest is not any one
  identity's own record.
- *Write path*: rewritten (`CREATE`-or-`TRUNCATE` + `WRITE`) whenever the set of identities with
  pending work changes. In this lane's own demonstration, the fixture writer writes it once
  alongside the one identity's `schedule` file. A real registrar (#387) would call the same render
  function every time a schedule is registered or its last pending job clears.
- *Read path*: one `OPEN` by the fixed, compile-time-known name, one `READ`, one parse. No
  `READDIR`, ever. The re-deriver never asks the FS server what exists. It asks only for the one
  document it already knows the name of, and then for each name that document contains.
- *Cost, measured against option (a)*: this is one bounded read regardless of how many identities a
  deployment has ever provisioned. Option (a) is either a refused system call (this tree's own rule) or,
  even if it were not refused, a directory listing followed by a probe read per candidate. The new
  code is a parser and a renderer for an eight-line document. It mirrors `timetable::parse`'s own
  size and shape (`crates/schedule_store`, ~40 lines of parsing logic, host-tested).
- *Against*: a second document format now exists beside `timetable::parse`'s (though it shares the
  same dialect conventions). Also, a write path that forgets to update the manifest when a schedule
  is removed leaves a stale entry naming an identity with no `schedule` file. `session_reviver`'s
  own per-identity read already turns that into an honest, bounded failure rather than a silent
  inconsistency. A missing `schedule` file under a named identity fails that one identity's
  re-derivation rather than the whole pass.

**Option (c)'s cost**, beyond what is stated above under "what else was considered": a compiled or
boot-configured list has no write path at all inside the system. So every new schedule holder is a
rebuild or a redeploy rather than an ordinary write. That is a materially worse fit for "a live
session registers a job" (152's own motivating scenario) than either (a) or (b).

## The recommendation

**Option (b).** A manifest, in `timetable::parse`'s own document dialect, naming exactly which
identities currently have a durable session with pending scheduled work. It is read by one name the
re-deriver already knows rather than discovered by listing anything. This is the cheapest option
that does not fall into milestone 126's refusal. It is not a new mechanism so much as
`measured_boot`'s own already-proven shape (a table of names, consulted by name) applied one layer
down. That is the same way §122's own recommendation was gluing two already-built pieces together
rather than inventing a third.

## How reversible is this, and who has already acted on it

This lane has acted on it. `crates/schedule_store` implements the format this decision proposes,
`fixtures/src/fs_test_client.rs`'s `ROLE_SCHEDULE_SEED` writes it, and
`components/src/session_reviver.rs` reads it. All of these land in the same pull request as this
decision document, per this lane's own brief ("investigate... and if it's a real fork, write it
up... rather than guessing"). That is the same shape §122's own lane took (build the recommended
shape while the decision is still `PROPOSED`), and the same caveat applies. Nobody outside this lane
depends on the manifest's format yet. So it is still cheap to change the file name, the line format,
or the location before a second registrar (#387) is built against it. Once #387 writes and reads
this file for real, the format becomes a wire contract between two programs (this lane's own writer
and `session_reviver`, then #387's real registrar and `session_reviver`). Changing it then needs a
migration rather than an edit. That is exactly AGENTS.md's test for when a decision doc is owed
rather than optional.

## What this does not decide

- Whether the manifest needs anything beyond a bare identity name (a timestamp of last write, a
  count of pending jobs, a checksum). Left to whoever builds #387's real registrar, once there is a
  real write pattern to design it against. This lane's own writer needs none of that (it writes
  exactly one identity, once).
- What happens when an identity's manifest entry has no corresponding `schedule` file (a stale entry
  from an incomplete removal). `session_reviver`'s own per-identity read already turns this into a
  bounded, per-identity failure rather than an unhandled case. But whether a real deployment wants
  that to be silent, logged, or corrective (removing the stale entry) is not decided here.
- The credentialer-held reattachment table milestone 152's own roadmap doc describes for the
  *reconnect* case (a live session finding its own durable session again) is a different mechanism.
  It is for a different question (identity to *session capability*, in memory, established at
  login), and this decision does not touch it. The parallel drawn above ("what this tree already
  does in the analogous case") is structural, not a claim that the two should share code.
