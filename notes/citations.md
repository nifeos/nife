# Citations that name what they cite

A citation in this tree points into one of two numbered records: a decision (`§N`, in
`design/decisions/`) or a milestone (`milestone N`, in `design/roadmap/`). There are about 2,150 of
the first and 2,860 of the second, roughly a fifth of them in code comments. They are load-bearing:
the documentation is part of this project's deliverable, so a footnote that lands in the wrong place
is a defect in the product, not an untidiness.

Two gates already check that a citation **resolves**. `script/decisions --check` proves a cited `§N`
has an index row; `script/roadmap --check` proves a cited `milestone N` has a block under
`design/roadmap/` (it checked for an index row until milestone 294 generated that index from the
blocks). Neither can prove
it resolves to the thing its author meant, and both say so in their own headers.

`script/citations` is the third gate, and it reads the target.

## Why this needed a gate at all

Twenty-eight comments across `crates/`, `kernel/`, `notes/` and `user/` credited the two-tier `^C`
design to "milestone 24". Milestone 24 is *A second aarch64 board: Virtualization.framework*. The
`^C` design is DECISIONS §24.

It was never a renumbering. The first of them was written on 2026-07-28 and the roadmap already said
Virtualization.framework that day, so it was wrong at birth and spread by copy-paste. Both existing
gates reported clean the entire time, because both numbers exist.

The tell, once you know to look for it, is that several sites read `milestone 24, DECISIONS §24`:
one thing cited twice under two schemes. `milestone N` and `§N` are two numbering schemes over the
same integers, and they agree by accident for the early numbers (milestone 12 (Call/Reply IPC) and
§12 (Call/Reply IPC) really are the same thing) and diverge after. That is exactly the trap. Reading
them as one scheme is right often enough to survive.

## The convention

A citation may carry a gloss, and a gloss is checked.

```
milestone 24 (a second aarch64 board)
DECISIONS §24 (interrupting the foreground process)
```

Both are self-checking: the parenthetical can be compared against the record's own title, which is
the comparison neither existing gate can make. Write one and it binds.

An existing citation without a gloss changes nothing. On a line a commit adds, the gloss is
required: see the ratchet below.

### The tolerance rule

A title is long (*"Interrupting the foreground process: two-tier, shell-held, no new kernel
surface"*) and a citation should be able to name a distinctive fragment rather than the whole thing.
A gloss passes on either of two tiers.

Tier 1, the title. Every content word of the gloss appears in the record's H1, order-free and
ignoring a short stopword list. A gloss abbreviates and reorders, so `§31 (the foreign-language
seam)` matches *"The foreign-language seam: C holds no capabilities and makes no policy"*, and
`milestone 55 (Time Machine over SMB3 with Apple's extensions)` matches a title that differs from it
by one preposition. 76 of the tree's 106 glosses pass here.

Tier 2, the quotation. The gloss appears as a contiguous phrase in the record's body, matched on
normalized whitespace. This is a different act rather than a weaker tier 1: `§4 ("a driver never
reaches into a kernel global")` quotes what a decision *says* rather than what it is *called*, and
no title check can accept it. Contiguous is deliberate. "Every word appears somewhere in a
5,000-word document" is not a test, it is a coincidence generator.

Three shapes are not glosses and are recognized rather than judged: a repo path (`milestone 15
(design/roadmap/0015-asids.md)`), a cross-reference to the other record (`milestone 12 (§12)`), and
a bare date. A path is held to a *harder* standard than a gloss, because it is exact: it must exist,
and a path into a numbered record must carry the number that cited it.

### The diagnostic that matters

When a gloss fails, the script asks whether it matches the same number under the other scheme,
and says so when it does. That turns the symptom into the fix, and it is the defect this whole thing
was built for:

```
citations: notes/credentials.md:346: §49's gloss (users, login, and attribution) is not
    grounded in 0049-removal-and-recursion.md
      §49 is: Removal is a directory operation, and `-r` widens the grant rather than...
      but it matches milestone 49: Users, login, and attribution: what identity is for...
      Two schemes, one number. This is the milestone-24 defect.
```

## The ratchet: a citation a commit adds says what it cites

`script/citations --ratchet`, in `script/lint` since 2026-09-19, on calef's ruling that day: *"add
the gloss ratchet."* It reads the **diff** against the merge base rather than the tree, so nothing
existing is touched and no lane is asked about text it did not write.

**The rule is per number per file, not per occurrence.** A `milestone N` or `§N` on an added line
passes if that number, under that scheme, is glossed *somewhere* in the same file: by title, by
quotation, by a path into the record, or by a `GLOSS_ALLOW` entry. A paragraph that names §151 four
times explains it once, and a gate asking for four glosses would be answered by deleting three
mentions.

A bare date and a bare cross-reference do not count as that one gloss, though `--check` recognizes
both and is right to. They are well-formed citations that do not say what the target *is*, which is
the only property this ratchet is about.

The rest of the ratchet's record is in [citations/ratchet.md](citations/ratchet.md). It says why a
ratchet on new text does not contradict this page's refusal of a sweep, what it cannot catch, and
its measured cost: a median of 16 asks per pull request over 42 merges. The limit worth knowing
before running it: it reads `HEAD`, not the working tree, so commit a fix before expecting it to go
quiet.

## The census: the population nothing had ever counted

`script/citations --census` counts the citations that carry no gloss at all, which `--check` cannot
see. Its first run, on 2026-09-19, found 18,517 citations in 1,447 files, and 505 of the 9,483
(file, scheme, number) pairs named: 5.3%, where the glossed subset alone had looked 83% split-proof.
Not one cited number failed to resolve. The unit is the pair, because one gloss answers every
mention in a file, and `--census --list` prints the worklist. The table, and how the zero was
re-checked, are in [citations/census-and-backfill.md](citations/census-and-backfill.md).

## The backfill, and the rule that chose it

Milestone 444 (a citation says what it cites) glossed 25 pairs across `README.md`, `CONTRIBUTING.md`
and `SECURITY.md`, chosen by one rule: gloss where the citation is the reader's only route to what
is meant. Do not gloss where the surrounding text already says what the target is, and do not gloss
a dated account, a quotation, or a block's own history. The remaining pairs are milestone 444's
recorded work, regenerated by `--census --list`. The worked cases are in
[citations/census-and-backfill.md](citations/census-and-backfill.md).

## --moved: when a status flips, who was citing that milestone

Milestone 385 (when a milestone's status flips, tell the lane which notes cite it) is the fourth
mode and the only one that cannot fail. The other three say something is wrong. This one says
something might be, and it cannot tell which, so it prints a worklist and returns zero.

The failure it answers has a measured shape. Six "is there a milestone for X" questions in one
evening turned up four things wrong on `main` rather than four things missing, all the same: prose
that was true when written and went silently false when a milestone landed.
`notes/why-not-general-purpose.md` told newcomers there was no networking, no writable filesystem,
no display and no SMP, five of six rows false. Milestone 66 (Vaultwarden: somebody else's real
application, running here)'s gap table said TCP listen and accept were absent from the contract a
month after milestone 107 (the socket contract learns to accept) shipped them. Every one stayed
green through every gate.

```
$ script/citations --moved 5f850c367~1..5f850c367
citations: milestone 525 moved NEW -> BUILT: A bad upgrade cannot brick the machine: two boot slots, tries an
citations:   design/decisions/0207-the-roadmap-is-a-graph-and-says-so.md:50
citations:   design/roadmap/0554-a-good-upgrade-sticks.md:10
```

Two files, and the second is the real one: milestone 554 (a good upgrade sticks: what marks a trial
boot successful)'s block still described 525 as the proposal it had been.

It tells rather than obliges because DECISIONS §207 (the roadmap is a graph, and the block says so
in fields a script can walk) retired an obligation to edit on a status flip. A report asks for no
edit and turns no other branch red. Four measurements set its shape: a worklist is about three
files, it fires per branch, it keys on the bare `milestone N`, and it reads the block file alone.
Over 150 merges the median flip has one citing file. The reasoning, the measurements and the tail
are in [citations/moved.md](citations/moved.md).

## Attributed quotations

The second half. A block quote may name the file it came from, and the passage must still be there:

```
> Kernel memory is never demand-paged. Kernel pages are mapped eagerly.
>
> -- design/decisions/0009-irq-safe-locking.md
```

Matching is on normalized whitespace, which is the only reason this is affordable. A note wraps at
100 columns and a block quote of it wraps at 96 after the `> ` prefix. So byte equality would fail
on every correct quote in the tree and the check would be thrown away inside a week. An ellipsis
(`...`) splits the quote into segments that must appear in order, so a quote may skip the middle of
a paragraph.

### Why it is worth the false-positive cost, argued rather than assumed

The obvious objection is reflow: rewrap the source and every quote of it breaks. That objection is
answered by the normalization, and what remains is the case where the source's **words** changed,
which is precisely what the check is for.

The defect it answers is real and recent. `design/roadmap/0074-cycle-counters.md` carried a block
quote attributed to `notes/benchmarks.md` (*"At ~3.2 GHz, 705 ns is ~2,200 cycles round trip... we
are 4 to 7 times heavier"*). Milestone 101 re-measured and retracted that arithmetic; the paragraph
no longer exists in the note. The roadmap was quoting a retraction as the current record, and
nothing could see it, because a prose block quote attributed to another file is a citation that no
gate resolves.

So: yes, verify it. The cost is bounded by the normalization and the payoff is a class of rot that
is otherwise invisible.

## Line numbers: cite a heading or a symbol

§277 (cite a heading or a symbol, never a line number), calef, 2026-10-11 (UTC). Cite a document by
path and heading, as `notes/net.md#the-inbound-half` or `notes/net.md`'s "The inbound half". Cite
code by path and symbol, as `kernel/src/sched.rs` `threads_round_robin`. Never follow a path with
`:` and a line number, or a link with `#L` and one: a line number moves with every edit above it,
and a heading or a symbol moves with its text.

`helpers/line_citations.py` (provisional name) is the gate, in `script/lint` and its `--no-cargo`
pre-push mode. Citations into Markdown are held at zero. Citations into code are held to a ceiling
that only falls, 835 when the gate landed, and a branch may not add one net. Quoted tool output is
not a citation: a `path:LINE:COLUMN` diagnostic, and anything in a fenced or indented code block.
Anything else that must keep a line number carries `line-citation: exception. Reason: ...` on its
line or the line above.

    $ python3 helpers/line_citations.py --list notes/x.md   # what the gate counts, and where
    $ python3 helpers/line_citations.py --bank              # after a sweep, lower the ceiling

## EXAMPLES

Report every citation, classified:

```
$ script/citations
73 decisions, 115 milestones

GLOSSED CITATIONS (106)
  cross-reference  6
  date             1
  exempt           1
  path             13
  quotation        9
  title            76

ATTRIBUTED QUOTATIONS (1)
```

Gate it, which is what `script/lint` runs:

```
$ script/citations --check
citations: 106 glossed citations, all grounded (76 by title, 9 by quotation, 13 by path,
    6 cross-references, 1 dates, 1 exempt)
citations: 1 attributed quotations, all still present in their sources
```

The breakdown sums to the total on purpose. A summary line whose parts do not add up is a number
nobody can check, which is the defect `script/lint`'s shellcheck line was fixed for.

Ask what this branch adds, which is the second thing `script/lint` runs:

```
$ script/citations --ratchet
citations: every citation on the 121 lines this branch adds says what it cites
```

The proof that it fires, run on 2026-09-19 by planting one line in this very file and committing
it. The planted line cited three records, and the interesting part is which two were reported:

```
$ script/citations --ratchet
citations: notes/citations.md:296: milestone 444 is cited with no gloss anywhere in this file
      milestone 444 is: A citation says what it cites, so a number stops being the identity
citations: notes/citations.md:296: §46 is cited with no gloss anywhere in this file
      §46 is: Thin primitives or whole subsystems; we write everything in between
```

The third, milestone 55, was not reported, because this page already carries
`milestone 55 (Time Machine over SMB3 with Apple's extensions)` in its tolerance-rule section. That
is the per-number-per-file rule working, and it is the whole difference between a ratchet and a tax.
The line was then removed and the same command went quiet.

Judge a range that is not this checkout's HEAD, which is how the cost above was measured:

```
$ script/citations --ratchet "$(git merge-base $pr^1 $pr^2)..$pr^2"
```

Citing the decision rather than the milestone, in a code comment:

```rust
/// Granted a per-job interrupt channel so `^C` can reach it (DECISIONS §24). A
```

Quoting a decision's own words, which tier 1 cannot accept and tier 2 can:

```markdown
DECISIONS §19 (parity is a gate, not an aspiration) applied to the toolchain.
```

Note that §19's *title* says parity is a **tenet**. The body says it is a gate. Two sites in the
tree glossed it as "architectural parity is a gate", which is neither, and both now quote the body:

> **Parity is a gate, not an aspiration.** A kernel capability ships on every supported
>
> -- design/decisions/0019-architectural-parity.md

That block is not an illustration. It is the tree's one live attributed quotation, and `script/lint`
re-resolves it against `0019-architectural-parity.md` on every run; edit that sentence in the
decision and this page fails the build until it is brought back into agreement.

## BUGS

Each limit in one sentence. The full entries, with how each was found and what was measured, are in
[citations/bugs.md](citations/bugs.md).

- The scanner sees a citation across one line break and only one; two are invisible.
- A lettered citation is glossed against its own block if it has one and against its parent
  otherwise, and the ratchet's per-file key drops the letter.
- The candidate pre-filter is deliberately looser than the scanner, and tightening it for speed is
  how the gate would go blind again.
- A possessive gloss, the number followed by `'s` and then its parenthesis, is not recognized; put
  the parenthesis directly after the number.
- `--moved` cannot tell staleness from correctness, and its output is only a prompt to look.
- `--moved` misses negative claims, such as a page saying "there is no networking", which cite
  nothing; milestone 259 (sweep `notes/` for claims that stopped being true) is that sweep.
- `--moved` reads `notes/` and `design/`, not code comments, and does not report a block deleted
  or renumbered, which `script/roadmap --check` already fails.
- An untracked file is invisible, so stage before you check.
- A gloss is optional, so an unglossed citation is checked by nothing: the rule is "what you write
  is checked", not "you must write it". The ratchet stops that backlog growing; it does not close
  it.
- A `--ratchet` replay of an old commit judges it against today's records.
- A wrong citation whose gloss is wrong in the same direction passes, and tier 2 is weaker the
  longer the target document.
- Fenced code blocks in markdown are skipped, so a citation inside one is unchecked.
- The ratchet reads the committed tip; an uncommitted edit that adds or removes a citing line
  fails the run with "Commit first".
- A tool that reads only stdout can see this gate as clean while it fails; check the exit code.

A file whose only glossed citations are lettered is scanned, and a gloss may wrap onto the line
after its number: milestone 583 (`script/citations` could not see a citation a line break split, or
a lettered milestone).

## What it found on the first run

28 comments credited the `^C` work to milestone 24 (a second aarch64 board) rather than §24
(interrupting the foreground process). 31 glosses could not be grounded, 6 of them the wrong record.
There was also one stale `design/roadmap.md` path, and two claims a decision does not make. Each is listed in [citations/first-run.md](citations/first-run.md).

## The same shape in another medium

Milestone 112 (the SAFETY comments that bind nobody) found the same failure in a `// SAFETY:`
comment that described a different operation and passed its lint for as long as the file existed. A
comment that names its subject can be compared against it; one that does not is unverifiable by
construction. The case is in [citations/another-medium.md](citations/another-medium.md).

## See also

- [Naming things](../design/naming.md): why `§N` and `milestone N` are different numbers over the same
  integers.
- [The `script/` entry points](scripts.md): where this sits among the gates.
- `script/decisions` and `script/roadmap`: the two gates that check a citation resolves, and whose
  headers name this blind spot.
