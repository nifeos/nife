---
status: BUILT
raised: 2026-09-19
built: 2026-09-19
---
# 443. Lanes wait on each other for three reasons, and none of them is the work

Built 2026-09-19. Minted 2026-09-19 by the maintainer, on calef's question: *"Is there
any way to decouple all of this so that we don't have to sequence?"* *(Number provisional until the
merge queue lands it.)*

## What happened on 2026-09-19, which is the evidence

Eleven pull requests were in flight across two sessions. Four of them (#988, #993, #995, #997) were
blocked at once, and none of the blocks was about the code in them:

1. `design/roadmap/README.md` is a generated index of 441 rows, committed. Every pull request
   that mints or finishes a milestone rewrites rows in it, so unrelated lanes conflict in one file.
   The stick-program lane and the layout-control lane each reported, independently and without
   being asked, that they deliberately did not regenerate it.
2. `script/fatal-risks` reads that index (`built_dates` in `script/fatal-risks`) rather than the milestone
   blocks. So a stale derived file made a truthful record look wrong: risk 9's citation of milestone
   177 "looked current only while 177's row was not BUILT", which is how a lane put it. This is the
   coupling that actually jammed the four.
3. Numbers are minted by hand and must be contiguous. Two sessions collided on `§156 (what)`, `§158 (program)`
   and milestone 439 in one day, and `script/lint` fails on a gap, so whoever lands second
   renumbers. The tree's own rule (the integrator mints) assumes one integrator, and there were two.

A fourth cause was the maintainer's, not the tree's, and is recorded here so the fix is not
mistaken for a whole answer. Lanes were cut from other lanes' branches (#987 on #985, #993 on #990,
#997 on #996), which turns one delay into four. The merge queue already lands groups of five, so
lanes cut from `main` would have been independent. That is a rule for whoever briefs, not code.

## What was built

### Every gate reads the record; nothing reads the rendering

Milestone 294 (design) had already made the index derived and, deliberately, made a stale row a report
rather than a failure, because failing would force every lane back into the file it had just got
them out of. What 294 did not do was stop other scripts opening the committed table, and two of
them did. The lag was therefore not harmless; it was an input.

- `script/fatal-risks` took its Built dates from `design/roadmap/README.md`, one `open()` at
  line 409. It reads `script/roadmap --index` now, which renders the same five columns from the
  per-milestone blocks. Same regular expression, same column, same meaning; the source moved one
  step upstream to the thing it was derived from.
- `script/audits` counted `BUILT` rows in the same file for its milestones-built cadence
  trigger. On the day it read 213 against a tree with 214, which is the one direction an
  audit signal must not err in. Same fix.
- `script/lint` gained a check that fails if any other `script/` entry point reads the committed
  index as data. This is the rung-two mechanism the milestone needed: the coupling reads as
  ordinary code at the call site, so a note would not have held.

`script/roadmap`, `script/metrics` and `script/catch-up` still read it, and only the first is the
generator. The other two read the committed file at historical revisions, through `git ls-tree`
and `git show`, to chart what the roadmap said in a past week and what changed between two commits.
A block-derived answer does not exist for a revision before 294 landed and cannot be manufactured.
They are on the lint check's allow-list for that reason, which is a different question rather than
an exception, and it is also the reason the file stays committed at all.

What this can no longer catch. Nothing that was being caught. The old reading did not assert
anything the new one does not; it asserted the same things about a table that could be behind. The
lint check is a grep and will miss a script that shells out to `git show HEAD:design/roadmap/README.md`
or assembles the path from pieces; it catches the shape both real defects had.

One finding was being hidden, and it is real. With the Built date taken from the blocks,
`script/fatal-risks --check` reports that risk 9 was last dated 2026-09-17 and reasons from
milestone 177, which turned `BUILT` on 2026-09-19. That is not a finding this milestone created: the
old check produces it byte for byte the moment `script/roadmap --write` is run on today's `main`,
which is what the integrator is supposed to do at every merge. It was latent, and it stayed latent
only because nobody regenerated. Risk 9 owes a dated correction paragraph, and that is the
architect's to write rather than a lane's.

### The index stays, and the choice was not close

The block offered two shapes: drop the committed file and print it on demand, or keep it and
regenerate after merge. Keep it, for a reason found by enumerating the consumers rather than by
preference.

| consumer | what it takes from the index | after 443 |
|---|---|---|
| `script/roadmap` | owns it; `--write` renders it, `--check` validates the rows | generator, unchanged |
| `script/fatal-risks` | Built dates for the "as of" check | reads `--index` |
| `script/audits` | `BUILT` row count, a cadence trigger | reads `--index` |
| `script/metrics` | milestone counts per status, at eight historical revisions | unchanged, allow-listed |
| `script/catch-up` | status transitions between two revisions | unchanged, allow-listed |
| `script/lint` | nothing; now forbids new readers | new check |
| `script/names`, `script/citations` | nothing | unchanged |
| `xtask` | nothing | unchanged |
| `crates/documentation` | nothing; two comments cite it as an example of a wide table | unchanged |
| `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `design/naming.md`, roadmap blocks | a link a reader follows | unchanged |

The two history readers decide it. `script/metrics` charts milestones-by-status per week from
blobs at revisions nobody has checked out, and `script/catch-up` diffs the roadmap between two
commits. Deleting the file would end both series from the day it landed, permanently, to avoid a
chore. The browsable roadmap on GitHub is the second reason and the one the block named; it is real
and it is not the load-bearing one.

What is not done, and it is deliberate. Nothing yet makes the regeneration happen. It is the
integrator's at merge, written in `AGENTS.md` and in `script/roadmap`'s header, which is rung four,
and on 2026-09-19 `main` carried three stale rows because rung four is what it is. A post-merge
workflow is a bot writing to `main`, which this tree has never done, and it needs milestone 128 (automation)'s
identity plus a rule about what else such a commit may touch. That is an architect's call and is
written up rather than built: `design/roadmap/0510-nothing-regenerates-the-roadmap-index.md`.

The point stands without it. After this milestone a stale index costs a reader an out-of-date
page and costs a gate nothing, which is what "purely a rendering" was supposed to mean.

### A duplicate is the defect; a gap is not

`script/decisions --check` failed on a hole in `design/decisions/` numbering. It reports one now.

**The roadmap already worked this way**, which is the tree's own answer in the analogous case: 441
and 442 are unused today and nothing complains. Only the decision index demanded density.

**Failing on a gap is what forced the renumber, and the renumber is the expensive edit.**
[§194](../decisions/0194-sessions-interleave-rather-than-serialize.md) records the hazard in its own
words: a citation rewritten by number can be silently wrong and still pass every gate, because the
section it now names exists. On 2026-09-19 one branch was renumbered **four times in two and a half
hours**, §156-§189 up to §160-§194, because a second session was minting from the same range and
contiguity made every collision displace the whole run. With gaps allowed, only the colliding
sections move: four files rather than thirty-four, four times over.

**Duplicates stay fatal in both directions**, unchanged: two index rows for one number, and two
files claiming one number, each its own failure.

**What this can no longer catch**, stated rather than discovered: a decision file deleted outright,
with its index row deleted in the same commit, and cited nowhere in the tree. Every partial shape
still fails. A file with no row, a row with no file, a row pointing at a file that is gone, and a
`§N` anywhere in the tree that resolves to nothing are all still problems. Those are the shapes
a half-finished deletion actually takes. What is left is a decision nobody ever referred to, removed
completely and on purpose, which is not the accident the check was written for.

**`design/naming.md` gained the rule it never stated.** It is the authority for both numbering
schemes (§155) and said nothing about density in either; it now says duplicates are fatal, gaps are
reported, and that the later lander takes the next free numbers instead of displacing a run.

## What this does not do

- It does not make two lanes safe in the same source file. That is milestone 365
  (`xtask/src/main.rs` with no module structure) and the hotspot rule in AGENTS.md.
- It does not remove the merge queue's serialization, which is not the problem: the queue lands
  groups of five, and today's jam was conflicts and false gate failures, not throughput.
- **It does not touch `design/decisions/README.md`**, which is hand-maintained and is the same
  kind of hotspot as a hand-kept roadmap index: every lane that lands a decision edits that one sorted
  table. Deriving it from the decision files is 294's shape applied one directory over, and it is
  proposed rather than done here.

## BUGS

- The index is also how a reader browses the roadmap on GitHub. Kept, for that and for the two
  history readers. The trade named in the original block was real and was decided rather than
  dodged.
- A post-merge regeneration commit is a bot writing to `main`, which this tree has never done.
  It needs an identity (milestone 128) and a rule about what else such a commit may touch. Not
  built; proposed.
- Nothing bounds how stale the committed index may get. `script/roadmap` prints the number on
  every run and whoever is looking reads it. Three rows was invisible; so is thirty.
- **The lint check is a grep.** A script that reads the index through `git show` or builds the path
  from pieces passes it. It catches the shape the two real defects had, which is an `open()` of a
  literal path.

## Follow-on

- **Milestone 510.** Post-merge regeneration of `design/roadmap/README.md`, with the bot-identity
  question it depends on, in milestone 510 (regenerating the index is nobody's job), `design/roadmap/0510-nothing-regenerates-the-roadmap-index.md`.
  It is an architect's call, not a lane's, which is why this milestone stopped at removing the
  gates' dependence on the file rather than at keeping the file current.
- **Milestone 513.** `design/decisions/README.md` is hand-maintained and is now the tree's remaining
  index hotspot, in milestone 513 (the decision index is still hand-maintained), `design/roadmap/0513-the-decision-index-is-still-hand-maintained.md`.
- **Recorded.** `script/metrics` and `script/catch-up` still read the committed index, at historical
  revisions where nothing else can answer, and so they read `main`'s lag at HEAD too. The allow-list
  and the reason are in `script/lint` where the next person meets the check.
- **Recorded.** Deleting a decision file, its index row and every citation to it in one commit is
  now invisible to `script/decisions`. Beside the check, and in `design/naming.md`.
- **Decision.** Risk 9 in `design/fatal-risks/README.md` is dated 2026-09-17 and reasons from milestone
  177, which turned BUILT on 2026-09-19. `script/fatal-risks --check` reports it now, and the
  pre-443 check produces the same finding byte for byte on a freshly regenerated index, so it was
  latent rather than new. Its home is the gate, which is red until it is answered, and the answer is
  an architect's: what 177's completion does to the risk is a judgment about the risk, not about
  the record. The standing rule it falls under is
  `design/decisions/0194-sessions-interleave-rather-than-serialize.md`, that anything global stays
  provisional until the queue lands it; this one is held under the `needs-architect` label with the
  ask written on the pull request.

## Index row

Eleven pull requests in flight, four blocked at once, and none of the blocks about the code in them.
The causes were a committed 441-row generated index that every milestone rewrites, a gate reading that derived file
rather than the blocks it derives from, and hand-minted contiguous numbers that two sessions
collide on. `script/fatal-risks` and `script/audits` now read `script/roadmap --index`, a lint check
stops a third reader appearing, and a gap in `design/decisions/` numbering is reported instead of
failed, so the later lander takes the next free number instead of renumbering a run. The index stays
committed because `script/metrics` and `script/catch-up` read it at revisions where no block-derived
answer exists.
