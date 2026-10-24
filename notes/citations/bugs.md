# Citations: the full BUGS entries

*Name: provisional.*

An appendix to [`notes/citations.md`](../citations.md), whose `BUGS` section states each limit in
one sentence. These are the entries in full, with how each was found.

## The scanner sees across one line break, and only one

Until milestone 583 (`script/citations` could not see a citation a line break split, or a lettered
milestone) the gap between a number and its `(` had to be a single literal space. So reflowing a
paragraph until the wrap fell between a citation of milestone 326 (nobody has been assigned to turn
a mutation score upward) and its `(` removed the citation from the gate's view without failing
anything. Three lanes hit that in one week. A closing emphasis marker is part of the gap too, for
the same reason. This tree bolds the number and leaves the gloss plain:

```
**milestone 41** (dead code: triage the suppressions)
```

so the `**` sat where nothing was allowed to be, hiding thirteen sites across ten files. A citation
split across *two* line breaks is still invisible. The one-newline cap is what stops a stray `(` on
line 40 pairing with a `)` on line 900, and nothing in the tree wraps that way today.
`script/citations --selftest` pins both the shapes that must be seen and the near-misses that must
stay quiet. It runs in `script/lint` ahead of `--check`, because a green check looks identical
whether the scanner works or has quietly stopped seeing a shape.

The same limit, stated from the gloss's side: a gloss may span at most one line break. Long glosses
in Rust doc comments wrap, and the scanner joins one continuation. Two are not read, and a
three-line gloss is invisible. This is a real limit, not a theoretical one: the line-based first
draft of this script read zero wrapped glosses and missed one of the milestone-24 defects for
exactly that reason.

## Lettered citations

A lettered citation is glossed against its parent block, which is where the sub-part is described:
7a, 9a, 16a, 16b, and milestone 19 (run a real workload)'s 19a through 19f. The roadmap refuses a
lettered block file, so no letter has a title of its own to be read against.
The ratchet's per-file key drops the letter either way. So a file that glosses milestone 19 has
answered for milestone 19d as well. That matches the per-number rule the ratchet is built on, and it
means a sub-part can ride on its parent's gloss.

So `milestone 19e` is checked against milestone 19's file, because there is no `19e` file. A gloss
describing what 19e specifically did will not be found in 19's text, and has to be phrased as
commentary instead.

## The pre-filter is looser than the scanner, on purpose

The candidate pre-filter accepts a line that *ends* in a citation, because that is what a wrapped
one looks like to a line-based `git grep`. That widens the file list from 649 to 853 and costs about
a second. Tightening it for speed is how the gate goes blind again, and the selftest is what would
notice. It also accepts `[a-z]?` after the number (`PREFILTER` in `script/citations`), so a lettered
citation, a number with a letter after it, puts its file on the list. The
general shape: when a checker chooses its inputs with one pattern and reads them with another, the
gap between the two is unreachable by any test that only runs the checker.

## A possessive gloss is not recognized, although the tree is full of them

The gate accepts `milestone 128 (the automation gets its own identity, and the agents get their own
voice)` and rejects `milestone 128's (the automation gets its own identity, ...)`. It reports the
glossed line itself as "cited with no gloss anywhere in this file". The possessive form is ordinary
English and already appears in the tree. Milestone 218 (every boot of the VisionFive 2 needs a human
typing four commands into U-Boot) is cited in exactly that possessive shape in `design/roadmap/`,
among others, and those files pass only because they predate the ratchet. This very entry tripped
the gate while being written, on the line above, which is as good a demonstration as the entry could
ask for.

Found 2026-09-23 by the `maintainer/what-the-machinery-did` lane, which spent three commits on it.
The failure names the right line and gives no hint that the apostrophe is what broke it. So the
obvious response is to reword the gloss rather than to move it. The workaround is to rewrite the
sentence so the number is followed directly by its parenthesis, which is what that lane did. Either
the matcher should allow `'s` before the gloss, or the message should say which form it wants.

## `--moved` cannot tell staleness from correctness, and it never will

A note saying that milestone 30 (the network stack as a confined component) built the net stack is
right forever, and it is listed every time 30's block is touched. The output is a prompt to look. If
looking is usually wasted the mode will be skipped, and that, rather than a false negative, is the
failure mode to watch for.

## `--moved` misses the worse half of the class it was raised for

The miss is inherent. A page saying "there is no networking" cites nothing, because a negative claim
has no anchor to hang a check on. That is the shape of the worst of the four instances that
prompted milestone 385 (when a milestone's status flips, tell the lane which notes cite it). It is
not detectable without telling an assertion from an observation in prose. This tree has already
priced that: `git grep -w TODO` ran at an 82% false-positive rate. The other half wants a sweep
rather than a gate, and milestone 259 (sweep `notes/` for claims that stopped being true) is that
sweep.

## `--moved` sees a citation in prose and not one in code

It walks `notes/` and `design/` only. A `milestone N` in a Rust comment is usually provenance for the
code beneath it, rather than a claim about what the tree can do today. Including `*.rs` would
multiply the worklist by the population `--census` counts without adding a case of the defect.
Untested rather than proved: nobody has measured how often a code comment makes a present-tense
claim about a milestone.

## A block deleted or renumbered is not reported by `--moved`

The mode reads the status at the tip and skips a file that is gone, so a number retired out from
under its citers prints nothing here. That case is already a hard failure elsewhere:
`script/roadmap --check` fails a `milestone N` that resolves to no block at all.

## An untracked file is invisible to this check

A green run says nothing about an untracked file. The walk is `git ls-files "*.md"`, so a file that
has been written but not yet `git add`ed is not in the corpus. The check reads the tracked tree,
reports honestly about it, and exits 0. Found on 2026-09-19, the expensive way. A maintainer wrote a
new roadmap block containing a block quote that ended mid-sentence, ran `script/citations --check`,
read exit 0, committed and pushed. The quote had never been read. The same run after the commit
failed on it, and a lane that had branched from the pushed commit reported the gate red on its own
base.

It is the pattern this tree keeps meeting: an absent failure signal read as a pass. It is the same
shape as `script/decisions` silently skipping a binary file, and `script/roadmap`'s merged-branch
check spending a day unable to fail. Nothing here is wrong: a check over the tracked tree is the
right corpus, since that is what a reader clones. What is wrong is reading its exit code as a
statement about the working directory.

The habit that fixes it costs nothing: stage before you check. `git add -A` and then run the gate,
or run it again after committing and before pushing. `script/lint` does not have this hole for the
same files, because it is invoked on a committed tree in CI. That is why the defect survives locally
and not on a pull request.

## A gloss is optional, so an unglossed citation is checked by nothing

This is the honest limit, and it was a deliberate choice, measured rather than assumed. Requiring a
gloss on the first mention of each number in each file means 2,911 sites. Every one has to be read
to know what its author meant; that is the whole premise, since a pattern-applied gloss would be a
confident falsehood next to every wrong number. That is not a sweep this project can do correctly in
one pass. A mechanically inserted gloss would make the 28 milestone-24 defects *look* verified. So
the rule is "what you write is checked", not "you must write it". The number is recorded here so
that whoever revisits it is arguing against a measurement.

That measurement was itself too small, and the census replaced it on 2026-09-19. 2,911 was first
mentions counted one way. The census counts 9,483 (file, scheme, number) pairs, of which 8,978
carry no gloss, over 18,517 citations. The conclusion does not move; it gets stronger. A sweep of
that size cannot be done correctly in one pass, and it is why milestone 444 (a citation says what it
cites) shipped a ratchet plus 25 hand-read glosses rather than a rewrite.

The ratchet does not close this entry; it stops it growing. An unglossed citation already in the
tree is still checked by nothing. And the escape in the per-file rule means a file that glosses a
number once satisfies the ratchet for every other mention in it forever.

## A `--ratchet` replay of an old commit judges it against today's records

The decision and milestone tables are loaded from the working tree, not from the revision under
test. So a range replayed over history can report a number that had no block at the time, or ground
a gloss in a title that has since been rewritten. That is harmless for the live check, where the
tree and HEAD are the same thing. It is worth knowing before quoting a replayed number as history.

## A wrong citation whose gloss is also wrong in the same direction passes

The first of these grounds in nothing and fails. The second grounds fine, and is still in the wrong
place if it sits next to interrupt code:

```
milestone 24 (the ^C work)
milestone 24 (a second aarch64 board)
```

The check proves the gloss and the number agree, never that either matches the surrounding prose.
(Writing this paragraph is what proved the fence exemption below is load-bearing: the first line
failed the gate until it went inside a fence.)

Tier 2 is only as strong as the target document is short. A long decision file contains many
phrases, so a wrong-but-plausible quotation of one has more room to land in another. Nothing in the
tree does this today; it is the direction the rule is weakest in.

## Fenced code blocks in markdown are skipped entirely

A page documenting this convention has to be able to show a wrong citation, and this page does. The
cost is that a genuine citation inside a fence is unchecked. `script/lint`'s TODO gate exempts *all*
of markdown for the same reason, so this is the narrower version of a carve-out the tree already
makes.

## Nothing checks `design/decisions/`'s index rows' own titles

That is `script/decisions`' stated non-goal (a row may abbreviate), so a row that abbreviates
*wrongly* is still invisible. The roadmap left this class on 2026-09-14: milestone 294 generates its
index from the blocks, so a roadmap row's title is its block's H1 by construction, and the migration
found five rows that had drifted.

Resolved since: milestone 582 (a decision's status becomes a field, and the index becomes
generated) generated the decisions table from each file's H1, and calef retired
the table on 2026-10-12 (UTC). There are no rows left to drift.

## The ratchet reads the committed tip, not the working tree, and refuses to pretend otherwise

`--ratchet` diffs against the branch's base and reads each file with `git show`, so a gloss you have
just typed and not committed is invisible to it. Found on 2026-09-20 by a maintainer who fixed three
unglossed citations, re-ran the ratchet, and read the same three failures back. A one-line "Commit
first." note was added. On 2026-09-24 it was missed beside a failure; the uncommitted fix was right,
and CI failed on the committed line.

So since 2026-09-24 a file whose uncommitted edits add or remove a line citing a real `milestone N`
or `§N` (or an untracked file containing one) fails the run. It names the file and checks nothing
else, because any report would be about text you are no longer looking at. Commit, then run it.
Dirty files whose edits cite nothing still get the one-line note, and the verdict stands, which is
what keeps `script/lint` usable on a work-in-progress tree. The scan is looser than the ratchet (it
reads raw diff lines, fences and all), so it can ask for a commit that would not have changed the
verdict; that is the cheap direction to be wrong in. Checking the working tree instead was refused,
because it would make a local pass mean something different from a CI pass, which only ever sees
commits.

## A tool that reads only stdout can report this gate as clean while it is failing

`script/citations` writes some findings to stderr. So a script that captures `stdout` alone and
looks for `cited with no gloss` sees nothing, and concludes the tree is clean. A maintainer's
gloss-fixing loop did exactly that on 2026-09-20. It printed `clean`, and `script/lint` failed on the
same tree seconds later. The fix is one flag: capture both streams, or check the exit code rather
than the text. The exit code is right; it was the transcript that lied.
