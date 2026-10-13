# What the provenance record cannot see

*An appendix to [`design/naming.md`](../naming.md), which is the rule. This file holds the known
limits of `Name:` blocks and `script/names`, including the kinds of name the worklist does not
cover and why `helpers/` helpers are left out. It exists to verify or challenge the main page. A
reader who only needs to name, ratify or rename something should not have to open it. The directory
`design/naming/` and this file's stem are provisional names; naming is an architect's.*

## BUGS

- It checks that a name carries a reason, never that the reason is still true. A block whose
  argument was overtaken looks exactly like one whose argument holds. `script/decisions --check`
  records the same limit for `§N` citations. It is not closeable by a script, because a reason is
  prose and prose is checked by reading. Milestone 97 (citations that name what they cite) is the
  neighbouring case.
- It cannot tell an honest `unrecorded` from a lazy one. The 44 in
  [provenance.md](provenance.md) were each researched against the git history, and nothing stops
  the forty-fifth from being a shrug. The only defence is that an `unrecorded` entry cites the
  commit that introduced the name. The next reader starts where this one stopped rather than from
  nothing.
- `recorded` is the state easiest to claim and hardest to check, which is the price of splitting it
  out. The citation is checked for being present and never followed. So `recorded (milestone 46)` on
  a name milestone 46 (rename the components for what they are) never mentions passes the gate
  exactly as well as a true one. Read the citation before trusting the state. A wrong one costs less
  than the alternatives: it demotes the entry from "research owed" to "signature owed" in a
  worklist, rather than putting a false claim in the tree.
- Seven of the ten `recorded` lean on a rule that was derived from the names it now explains.
  `*_proto` won in milestone 46 partly *because* "it is what the actual crates already were". So
  saying `fs_proto` is recorded by that rule is not fully independent of `fs_proto`. It is not
  circular either: the decision adjudicated four live spellings, and `script/lint` has enforced the
  winner since. But a reader weighing the state should know it leans on one decision, and that the
  decision partly ratified the status quo.

### The tree has more kinds of name than the table covers

This entry is the one place that says which. Everything else that states the worklist's scope
(`script/names`' own comment, `helpers/name_provenance.py`, `helpers/roadmap_proposals.py`) cites
this entry rather than restating it, on purpose. The last two copies of this claim went stale for a
month after the coverage grew, and nothing compared them against the tool.

Crates, programs, `script/` entry points and Cargo packages carry blocks. Directories, types and
`helpers/` helpers do not, and at least one ratified name had no home as a result:
`design/audit-reports/` (calef, 2026-08-04). There `audit-trail` was refused because
`design/decisions/0035-scanner-findings.md` already uses that phrase for a chronological record of
dismissals and it is also what an operating system means by it (`auditd`). Bare `audits` was
passed over because every file in the directory is a report. Recorded here rather than stretched
into a schema that does not fit it.

- Closed for directories by §75 (carry provenance in their own README). A directory under
  `design/` or `notes/` carries its provenance in its own `README.md`, as `design/decisions/`,
  `design/roadmap/`, `notes/` and `design/audit-reports/` (`Name: ratified`, with the
  `audit-trail` refusal) do.
- Closed for Cargo packages by the `package` kind. `kernel`, `xtask`, `redoxfs_server` and
  `tools/redoxfs_host` carry blocks in their manifests.
- Types are covered only when marked. `script/names` reads four more kinds by marker. An `item` is any function, constant, type, module, macro, field or variant whose `///` doc
  holds a `Name:` paragraph. A `module` is a `//!` block outside a crate root or program. A
  `directory` is the README block §75 asked for, now gated, and each `.md` stem inside is a
  `document`. Only marked items are tracked, so an unmarked type is still invisible; that is the
  price of not failing thousands of names nobody recorded.
- `script/metrics` still counts the original four kinds only, so the dashboard's provenance series
  does not see the kinds above. Its enumeration reads `git ls-tree` at past revisions, and teaching
  it the item parse is its own piece of work.
- `helpers/` is uncovered on purpose, priced and refused on 2026-09-20 by milestone 446 (the naming
  worklist says what it covers, and stops saying what it used to). Its own entry is below, because
  it is a decision rather than a gap.

That blind spot has a live casualty, found while triaging. `disk_partitioner`'s introducing commit
(2026-08-03) named two provisional things: itself, and `fs_maker`. The first is on a covered
surface, so it is in the worklist with the word "provisional" quoted in its block. The second is at
`redoxfs_server/src/bin/mkfs.rs`, where nothing looks. So it was resolved to `mkfs` by whoever was
mid-task, and no record anywhere says a decision was owed. That is the exact failure this milestone
exists to prevent, still happening one directory over.

### `helpers/` helpers are deliberately outside the worklist, and the numbers are why

There were 17 files in the helper drawer when this was priced, of which 9 already carried a `Name:` paragraph that nobody asked
them for, written by the lane that added the file. So the question is not whether a helper may argue
its own name. It may, and more than half did. The question is whether the worklist should enumerate
them. Enumerating cost about 15 rows on a worklist 76 deep that day: a fifth again of the only queue
in this tree whose sole consumer is an architect's attention. Three things decided it against.

1. The worklist is ordered by exposure, and its own header says so. A program is typed at the
   prompt, a crate is what a newcomer greps, a `script/` entry point is typed by whoever works on
   the tree. The Scripts section of
   [programs-scripts-and-directories.md](programs-scripts-and-directories.md#scripts) defines
   `helpers/` as the drawer that is *not* typed by people. Enumerating it would add a tier below
   the bottom tier of a list whose whole ordering is exposure.
2. Nothing is being lost. Not one of those 9 paragraphs records a refusal, so milestone 115's "the
   refusals are the valuable half" claim gives up nothing by leaving them out. That is the number
   to re-measure if this is ever revisited.
3. The 8 without a paragraph are the machine-invoked ones (`qemu-runner-*.sh`, `qemu-bounded.sh`,
   `memory-bounded-runner.sh`, `build-ripgrep.sh`, `rust_source.py`). A gate demanding blocks would
   mostly manufacture rulings on names nobody types.

If it is ever built, order it last, after `script/`, for the same exposure reason. What the refusal
did buy: `script/names <helper>` no longer answers "neither a name in the tree nor a recorded
refusal" about a file that argues its name at length. It says the name is out of scope and points
at the file.

Re-measured 2026-09-24: `scripts/` holds 27 helpers, 12 with a `Name:` paragraph. A grep of those
12 paragraphs for a refusal finds none, so the second reason still holds. The 15 without one now
include `open-lane.sh`, `open-lane-gateway.sh` and three `review-*.sh` helpers alongside the
machine-invoked set, so the third reason is weaker than it was.

### Smaller limits

- A type's name is a naming decision the mechanism does not see. `BootEndowment` was ratified on
  2026-08-04 (replacing `Grants`) and is mentioned inside `system_initializer`'s block only because
  its crate happens to export it. `ChildEndowment` was ruled by calef on 2026-08-15 in §69
  (`Endow` is `ChildEndowment`), and that ruling lives in a decision, not in any `Name:` block.
- The `Name:` marker is a string in a comment. A header that never had one is caught by the gate.
  A header that loses one to an edit is caught only if the edit removes the whole line.

### A correctly formatted `ratified` is never checked against calef

On 2026-08-14 two lanes contradicted each other about the same name on the same day.
`crates/cpu_set` was introduced twice in one stack of pull requests. The first, #176, carried `Name:
ratified (calef, 2026-08-14, the same day the first-silicon online-set sweep introduced it)`. The
second, #178, one branch later in the same stack, carried `Name: unrecorded, provisional (introduced
2026-08-14 by the first-silicon online-set sweep; calef has not seen it)`. The second is the true
one, and the two sat in the queue together asserting opposite things about whether a ruling had
happened. (Both headers said `Chris` when they were written. They are quoted here in the referent
this tree adopted on 2026-08-15, which is also how the surviving one now reads in `crates/cpu_set`.)

Corrected 2026-09-24, and not resolved here: `crates/cpu_set` now reads `Name: ratified 2026-08-14
(calef, ...)`. Commit `3be741f62` landed it on 2026-08-15, and its message says calef ratified the
name on 2026-08-14 with the `cpu_set_t` argument. So the surviving header does not read as the
paragraph above says. Whether #178's "calef has not seen it" was true when it was written, this
record cannot settle.

The gate caught the false one, and caught it for the wrong reason. `script/names --check` rejected
#176 because the date sits inside the parenthetical, where `ratified (\d{4}-\d{2}-\d{2})` cannot
reach it. Written as `Name: ratified 2026-08-14 (calef, …)`, the identical false claim would have
passed every check in CI and landed on `main`. What stopped it was punctuation.

This follows from the design and is not a defect in it. The gate checks that a block names one of
the states and never that the state is `ratified`. A gate keyed on ratification holds every
unrelated merge behind a queue only one person can drain, the wall milestone 115 (the names that
were ratified) was written not to build. The cost of that choice is what this entry records:
claiming calef's ruling is as cheap as claiming anything else, and nothing downstream disagrees. The
`recorded` entry above says the citation is never followed. This is the same hole one state up,
where the claim is not a citation anybody could follow but an assertion about a person.

A lane that does not know this will write `ratified` meaning "this name seems settled". Write
`unrecorded, provisional` instead and say so in the report. It costs nothing: `script/names
--unratified` is a worklist rather than a wall, and an unratified name has never failed a build.

What would help without building the wall, if this recurs: surface newly added `ratified` blocks in
a diff for the integrator rather than blocking on them. `ratified` is the one state a lane
structurally cannot be entitled to assert. So a new one appearing in a pull request is worth a human
glance, even though it must not be worth a red check. That is rung two of AGENTS.md's ladder applied
to the one state that currently sits on rung zero. Not built, and not obviously worth building for a
hazard observed once.
