---
status: DECIDED
raised: 2026-09-23
decided: 2026-09-23
ratified_by: calef
---

# 212. A prose budget: 3,000 words of main body, with appendices under the same cap

The cap is 3,000 words of main body, ratified by calef on 2026-09-23
(UTC). That answers question 1 below and nothing else. Raised by him the same day, after a
maintainer session spent most of a context window reading one document, `design/fatal-risks/README.md`, at
17,742 words.

Appendix siting is ratified too, by calef on 2026-09-23 (UTC), answering question 3. The default
is a parent-named sibling directory: `notes/soak.md` beside `notes/soak/*.md`, with
`notes/soak/README.md` as the appendices' index and the directory's naming record (§75 (directories under `design/` and `notes/` carry provenance in their own README)).
The page keeps its path when it crosses the cap and is split, so a note has one shape whatever its size.

*Corrected 2026-10-11 (UTC), calef on pull request #1918, who chose this layout over the page
moving inside as `X/README.md`: this paragraph used to give `design/fatal-risks/README.md` beside
`design/fatal-risks/*.md` as the default's example. That contradicted the orphan rule below, which
checks `X.md`, and the lane that split the ten longest notes had to pick a reading.*

This governs a page whose detail was moved out of it. A directory of records made one at a time,
such as `design/decisions/`, `design/roadmap/`, `design/journeys/`, `notes/coes/` or
`design/fatal-risks/`, has no page to keep: its `README.md` is the index, and nothing here changes
it. `design/fatal-risks/` holds nine numbered risk records, and calef moved its summary inside on
2026-09-29 for exactly this reason: "the directory should look like the archive and the file like a
summary of it, the same shape as design/decisions/ and design/roadmap/".

The rule has no exceptions. calef ruled on pull request #1921 (2026-10-11, UTC) that fatal-risks is
a records directory rather than a marked exception. The one page that lived inside its own
directory moves out to `notes/delegated-review.md`, beside its transcripts. It is the study of
milestone 521 (does an AI review of a pull request catch anything the gates and the maintainer do
not).

A thematic directory is a permitted exception when the appendices are independently citable,
which is why `AGENTS.md`'s tenets live in `design/tenets/` rather than in `AGENTS/`. Content that is
a document in its own right is neither, and stays a peer document in the same directory, which is
what `notes/register-of-measures.md` is beside `notes/project-metrics.md`.

The reason the default is parent-named is the orphan rule, which is the one new failure this
convention manufactures. A parent-named directory makes the check a path rule: every file under `X/`
must be linked from `X.md`. A thematic directory can only be checked by walking links, so an
exception costs more gate than the default does. An exception is therefore marked where a reader
meets it, in the appendix directory's own `README.md`, with its reason.

One premise that was checked rather than assumed, because it was the stated reason for a lane's
choice: `script/lint`'s notes-index check is `glob.glob('notes/*.md')` and does not recurse, so
appendices under `notes/<stem>/` never needed index rows. `design/tenets/` still earns its exception,
on the ground that a tenet is cited on its own, not on the index ground.

Enforcement is ratified, by calef on 2026-09-23 (UTC), answering question 4: the ratchet, plus a
graph in the metrics deck.

The ratchet is what the recommendation below describes. A document already over the cap may not
grow. A document under it may not cross. A new document, or one rewritten wholesale, meets the cap
outright. An exception is marked in the document itself and carries its reason, which is the rung the
ladder permits when the higher one costs more than the failure does. No document carries a
prose-budget exception now. Roadmap blocks as a class stay under the cap, so question 2's refusal
of a class exemption stands. No migration sweep: the 174 documents over the cap are worked
worst-first by words times readers, and the ones nobody reads are left alone or archived.

The graph is the half that is not a gate, and it is there because a ratchet is invisible. A gate
fires on the change in front of it and says nothing about the trend, so the debt can sit flat for
months and nobody notices either the stall or the progress. `notes/project-metrics.md` is where this
tree already plots what it wants to stay honest about, so the prose budget is plotted beside the
unsafe count and the harness count rather than tracked in a file somebody has to remember to open.

The series to plot, recommended and not yet ratified: the excess above the cap in words,
which is the debt itself, and the count of documents over the cap. The first says whether the
tree is paying the debt down, the second says whether the ratchet is holding. Both were measured at
569,775 words and 174 documents on 2026-09-23.

The gate and the graph count words the same way, through one function: every whitespace-separated
word in the file, less a generated table. §213's sentence and bold measures read prose alone. calef
ruled this on 2026-10-13 (UTC), choosing the option "Gate counts every word".

What this section does not yet decide: whether the cap applies to every document or to a class of
them (question 2). The recommendation below argues for every document. The two marked exceptions
to date are the evidence either way: `AGENTS.md` at 5,873 words of imperatives and
`design/fatal-risks/README.md` at 4,235 (it was 4,176 when this section was first written, and
grew during the density pass). *(Section number 212 was minted at merge on 2026-09-24.)*
calef retired `AGENTS.md`'s exception on 2026-10-06 (UTC): "Retire the word-cap exception".

calef's framing:

> At Amazon we would have a one pager or a six pager to manage human context. The same seems right
> here for our documents.

A maintainer session answered with a two-class rule: cap the documents read in order to decide,
leave reference documents (`notes/`, roadmap blocks) uncapped with a structural requirement
instead. calef refused that split, and the refusal is the load-bearing part of this section:

> Amazon would allow for appendixes that were not part of the six pages. That allows for deeper
> discovery and in effect capped every document at six pages with documents referencing others. And
> trust me, the long documents were rare and didn't get used. Its too much cognitive load for a
> human to page through a book to get what they need.

And on mechanism: *"We can make generous use of hyperlinks. They're awesome."* Read that as license
to link rather than restate, here and everywhere.

**A sibling section, [§213 (writing standards)](0213-writing-standards.md), minted 2026-09-24.** It bounds how densely a document must be written, where this one bounds
how much of it there may be. They are two decisions, and the pairing is why: a cap on length with no
density standard is satisfiable by terse vagueness, and a density standard with no cap still permits
a book. Two sections linking to each other instead of merging into one is this section's own
convention demonstrating itself.

## What is being decided

1. What the cap is, in words.
2. What it applies to: every document, or a class of them.
3. What an appendix is, where it lives, and what may be moved into one.
4. What happens to the documents already over the cap.

## The evidence

Measured on 2026-09-23 at base `29fa47181`, counting whitespace-separated words in every `.md`
directly under `design/`, `design/decisions/`, `design/roadmap/`, `notes/` and `briefs/`.

1,008 documents, 2,073,706 words, roughly 4,100 pages. Medians and p90 by directory:
`design/decisions/` 935 / 2,031 (212 files); `design/roadmap/` 1,060 / 3,049 (570); `notes/`
2,933 / 8,454 (201); `briefs/` 1,302 / 1,379 (8).

At a 3,000-word cap, 174 files (17.3%) are over, and they hold 1,091,775 words, 52.6% of all
prose measured. The excess above the cap is 569,775 words, about 190 six-pagers' worth of
splitting.

The longest documents, with the number of other files in the tree that mention the basename
(`git grep -l`, excluding the file itself), which is the best available proxy for a reader arriving
at it:

| document | words | citing files |
|---|---|---|
| [`notes/benchmarks.md`](../../notes/benchmarks.md) | 46,810 | 111 |
| [`notes/mutation-testing.md`](../../notes/mutation-testing.md) | 31,136 | 41 |
| [`notes/load-sensitive-assertions.md`](../../notes/load-sensitive-assertions.md) | 26,699 | 52 |
| [`notes/README.md`](../../notes/README.md) | 22,489 | 138 |
| [`design/naming.md`](../naming.md) | 19,947 | 127 |
| [`notes/stranger-test.md`](../../notes/stranger-test.md) | 18,775 | 36 |
| [`design/fatal-risks/README.md`](../fatal-risks/README.md) | 17,742 | 120 |
| [`design/roadmap/0047-navigation-and-naming.md`](../roadmap/0047-navigation-and-naming.md) | 17,315 | 6 |
| [`notes/x86-port.md`](../../notes/x86-port.md) | 14,938 | 67 |
| `nifeos/nife:notes/pipes.md` | 14,372 | 119 |
| [`design/roadmap/0139-drive-down-unsafe.md`](../roadmap/0139-drive-down-unsafe.md) | 13,775 | 1 |

Documents over 6,000 words are cited from a median of 40 other files; documents at or under 1,500
words, from a median of 2. (n=54 and n=601.)

That figure cuts both ways, and it is the argument. It contradicts the naive reading of calef's
"the long documents didn't get used": ours are used, heavily. It supports his design anyway. A
94-page document reached from 111 other files is being used as a database, and each of those
arrivals is a reader who wanted one fact and had to load a book. High traffic times high length is
the definition of a document that should be a main page plus appendices. The caveat, stated where
the claim is made: nobody here can measure whether a document was read, only that something links
to it, so this is an inference from links and not an observation of readers. An earlier census the
same day produced different citation counts for the same files (142 for `notes/benchmarks.md`
against 111 here) because it counted matching lines rather than distinct files; the method above is
the one these numbers came from.

## The options

(a) No cap, structure only. Require headings, a summary, a table of contents. Refused: every
long document here already has structure, and 17,742 words with good headings is still 17,742 words
a session loads. Structure helps a human skim and does nothing for an agent's context window, which
is the cost that prompted this.

(b) Per-class caps. Cap decision sections and briefs, the documents read in order to decide;
leave `notes/` and roadmap blocks uncapped as reference. Refused by calef, quoted above. Two
reasons in his refusal. An exempt class is where every long document ends up, and the exemption
here would cover `notes/`, which is exactly the directory holding the worst offenders. And the
reference case is the one that needs the cap most, because paging through a book for one fact is
the cognitive load he names.

(c) A uniform cap with recursive appendices. One number, every document, no exempt class; depth
lives in appendix files that are themselves documents under the same cap. Recommended. It is
the shape that makes (b) unnecessary rather than overruling it: a reference document keeps all its
depth, reorganized so that arriving costs a page instead of a book.

(d) Ratchet versus tree-wide migration. A cliff turns 174 files red on the day it lands and
buys nothing, because the gate cannot split them. Recommended: a ratchet, the shape already
used for the unsafe count and the icount tripwire. An over-cap file may not grow; a new or
under-cap file may not cross.

## The recommendation

(c) plus (d), with these specifics.

3,000 words of main body per document, every document in the tree, no exempt class. The
one-pager is worth keeping as a **softer convention rather than a second cap**. `briefs/` sits at a
1,302-word median and is already there in practice, so the convention describes what the tree does
instead of imposing a second number to enforce.

**Appendices are files under the same cap, sited beside their parent**: `notes/benchmarks.md` plus
`notes/benchmarks/<topic>.md`. The tree already has that shape at
[`notes/project-metrics.md`](../../notes/project-metrics.md) beside `notes/project-metrics/`,
though there the directory holds generated charts and data rather than prose, so it is a precedent
for the siting and not for the content. **These paths are provisional names** and calef ratifies
them, per `AGENTS.md`.

**The completeness rule, which no gate can check.** A reader must be able to read the main document
and act on it without opening any appendix. An appendix exists to verify or challenge the main
document, never to carry its argument. That is what separates a real split from stuffing the
overflow out of sight, and it is a review question.

**Hyperlinks are the mechanism**, per calef. A named claim plus a link beats a paragraph
reproducing what another document already says. This only works because links here are cheap and
`script/citations` already watches them, so a split does not silently produce dangling references.

## The gate, built

It is [milestone 586 (a prose ratchet in lint)](../roadmap/0586-a-prose-ratchet-in-lint.md), minted 2026-09-24 and shared with §213's gate.
#1230 built it on 2026-09-24: `helpers/prose_ratchet.py`, run by `script/lint`, with its baseline in
`design/prose-baseline.tsv`. The milestone is PARTIAL. One week of baseline-churn measurement
remains, and that number decides whether the ratchet stays.

- A word count per file in `script/lint`, main body only.
- A ratchet on growth, per (d) above, so the 174 do not all go red on day one.
- A mechanical rename is not growth (calef, 2026-09-27T05:23Z, ruling on PR #1289). A rename
  adding no new sentence does not count as touching a document here. `RENAME_PAIRS` and
  `rename_masked` in `helpers/prose_ratchet.py` read such a substitution as the form it replaced
  before measuring, so it adds no word under this cap; design/roadmap/586 and §213 record the same
  ruling.
- A marked exception in the document itself, carrying its reason. `AGENTS.md`'s ladder permits
  an exception and requires it to say out loud that it is one, because an unmarked exception reads
  as a design and the next person extends it.
- **An orphan check.** An appendix nothing links to is a lost document, which is the failure this
  convention manufactures if nothing watches for it.

## Migration, ranked by words times citing files

First cut, in order: [`notes/benchmarks.md`](../../notes/benchmarks.md),
[`notes/README.md`](../../notes/README.md), [`design/fatal-risks/README.md`](../fatal-risks/README.md),
[`design/naming.md`](../naming.md).

The ranking has a tail, and it is half the point. Splitting all 190 six-pagers of excess is
months of lane work, and most of it would be spent where nobody arrives, so a document few files
cite is worked last or archived.

## What is blocked until this is answered

`design/fatal-risks/README.md` is being condensed on branch `maintainer/fatal-risks-condensed` against a
3,000-word body plus appendices, so that lane is this convention's first instance and a different
ratified number means redoing it. Nothing else is blocked.

## BUGS

- A word cap rewards moving prose rather than cutting it. Every file can pass while the tree's
  total gets worse, and a reader who needs the whole argument now opens four files instead of one.
  The completeness rule and the review question are the only defense, and neither is a check: no
  gate can tell a good split from a hidden one. The pair with §213 is what actually reduces reading
  cost; either alone is evadable, and this one is evadable in exactly the direction §213 watches.
- The traffic argument rests on links, not readers, as said above. If the long documents are in
  fact read end to end by the people who cite them, the cap costs those readers a worse experience
  to buy a better one for everyone else. This section has no way to find out which.
- **3,000 is a judgment, not a measurement.** It is roughly six pages, it is above `notes/`' own
  median of 2,933, and those two facts are the whole case for that number rather than 2,500 or
  4,000.
