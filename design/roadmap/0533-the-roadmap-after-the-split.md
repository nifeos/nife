---
status: NOT-STARTED
raised: 2026-09-19
promoted_from: the-roadmap-after-the-split
milestone_dependencies: none
decision_dependencies: 201
machine_requirements: none
specific_machine: none
needs_person: no
---
# 533. Whether the roadmap should become issues, and what a citation means after the repository split

Promoted from the proposal `the-roadmap-after-the-split`, filed 2026-09-19, on calef's instruction
of 2026-09-20 to give every proposal on `main` a number. The text below is the proposal's own,
unedited except for this paragraph: the argument is its author's and promotion is not the moment to
improve it. Written by `maintainer/roadmap-after-the-split`, on calef's question of 2026-09-19: *"We
have a lot of machinery now for managing milestones. We could use issues. Are we sure our current
approach is still the right approach? How will we manage this when we break up the repo and build
packages?"* Shaped for the integrator to mint as a `design/decisions/` section.

The record's *location* is reversible (a `git mv`). The **citation convention is
not**: 3,148 `milestone N` sites in 288 Rust files follow it, and it is in a reader's head. Options
below, with no winner on the identifier. Blocked until answered: nothing today, and that is the
point of raising it now rather than during the split.

**Answered 2026-09-20 by DECISIONS §201 (one roadmap until a citation has to cross), which rules on all three asks. The citation identity is deferred under option E with two conditions, a cross-repository dependency is declared by the blocked side alone, and issues carry a stranger's bug and non-dependency coordination. This file stays as the priced analysis behind that ruling.**

## What is being asked, split into the three questions hiding in it

1. Is the in-tree roadmap still the right medium?
2. Would an issue tracker be better, and at what?
3. What happens to all of it when the split of §151 (the goal of the repository split is
   independent release) arrives?

They have different answers, so they are answered separately. The short version: **(1) yes, for the
layer that gates read and the layer a newcomer reads. (2) yes, at exactly two things, neither of
which is the roadmap. (3) the number breaks and the gloss does not, which is why the identifier is
the only part worth deciding now.**

## The roadmap is three layers, and only one of them is in `design/roadmap/`

Treating it as one thing is what makes the question look hard. Measured in this worktree at
`bfe6a7ae`:

| Layer | What it is | Where it lives today | Size | Can a tracker hold it? |
|---|---|---|---|---|
| **Argument** | why, what was refused, what was measured, the `BUGS` | `design/roadmap/<n>-<slug>.md`, `design/decisions/<n>-<slug>.md` | 441 blocks, 66,304 lines; 194 sections | No. It is co-versioned with the code it describes (see below) |
| **Status** | one word per block, read by gates | the same files | 11 distinct words, 215 `BUILT` | No. A gate cannot read a tracker offline (measured below) |
| **Scheduling** | who is working on what, what is next | **already outside the tree**: `gh pr list --draft` | one draft per lane | It already is one. §90 (the claim is a draft pull request) decided this on 2026-08-16 |

**The third row is the answer to most of the question.** DECISIONS §90 separated claiming from
closing precisely because *"the two want opposite properties. A claim is ephemeral coordination
state, true for two hours and then false forever, and it must be instant. A status is a durable
record, and it should be slow, versioned and gated exactly like every other record here."* The
scheduling layer moved out three years of commits ago in project time, and it moved to GitHub, just
not to Issues. So "we could use issues" is not a proposal to change the roadmap. It is a proposal to
change either the record (row 1) or the gate input (row 2). Those are the two rows a tracker cannot
hold.

**Status words are 11, and 3 blocks carry none.** `**Status: BUILT**` 215, `NOT-STARTED` 173,
`PARTIAL` 35, `SUPERSEDED` 7, `RECORDED` 6, `REMOVED` 3, `PROPOSED` 3, `OPTIONAL` 2, `IN-PROGRESS`
1, `DECIDED` 1, and 3 with an empty status word. A tracker gives you open/closed plus labels, which
is fewer distinctions than the tree already makes and no cheaper to keep accurate.

## What a tracker can and cannot carry, checked rather than asserted

The proposal checked four things about a tracker by command, and listed what a tracker does better.
Both are in [`tracker-checks.md`](0533-the-roadmap-after-the-split/tracker-checks.md): whether a gate
can read it offline, whether it versions with the code, whether a citation survives an old tag, and
what a host move or split does to issue numbers.

### The correction this proposal owes, loudly

**§90 says issues are disabled on this repository. They are not, and they are empty.**

```
$ gh repo view nifeos/nife --json hasIssuesEnabled
{"hasIssuesEnabled":true}
$ gh issue list --limit 5
(no output)
```

That sentence was true when §90 was written on 2026-08-16 and is false today; nobody noticed because
nobody looked. It changes the shape of the question in a useful direction: **the experiment is
already available and has zero uptake.** Nothing has to be enabled, configured or decided to start
using issues for the two things above. That is evidence about demand, not about design, and it is
the cheapest evidence on this page.

## The split is the forcing function, and the number is what breaks

DECISIONS §151 rules the goal is independent release and third-party programs, on the Linux
distribution model: *"The kernel is one independently-released component, not a privileged base."*
A milestone number is one global namespace over one tree. After the split, a number means nothing
outside the repository that holds the block.

### The measurement that decides this, and it rules out the obvious option

Citations are not local to a subsystem. Distinct milestone numbers cited from Rust and Markdown
sources, by directory group:

| Cited from | distinct milestones | distinct `§` sections |
|---|---|---|
| `kernel/` | 168 | 69 |
| `crates/` | 150 | 72 |
| `components/` | 78 | 52 |
| **union** | **207** | **85** |
| **cited from two or more of the three** | **128 (62%)** | **65 (76%)** |
| cited from all three | 61 | 44 |

Occurrence counts, for the size of any sweep: 3,148 `milestone N` in 288 `.rs` files under
`kernel/`, `crates/` and `components/`; 1,796 more in `notes/`; 2,939 inside `design/roadmap/`
itself.

So 62% of cited milestones are cited from more than one of the three groups most likely to become
separate repositories. Per-repository numbering does not decompose the citation graph; it cuts
across it. Whatever the grain of the split turns out to be, the majority of citations are
cross-repository citations the day it happens.

### The second measurement, which is the one with good news in it

`script/citations` classifies every glossed citation by how it resolves:

```
$ script/citations
194 decisions, 440 milestones

GLOSSED CITATIONS (560)
  cross-reference  12
  date             29
  exempt            1
  path             24
  quotation        29
  title           465
```

465 of 560, or 83%, resolve against the target's own title. Nothing requires a gloss;
`notes/citations.md` is explicit that *"the rule is 'what you write is checked', not 'you must write
it'"*. Authors wrote titles anyway, because a number does not say what it names and they wanted the
sentence to be readable.

That is the finding underneath every option below. **A citation of the form `milestone N (lanes
wait on each other for three reasons)` already resolves without the number**, by `grep` in any
repository, under any renumbering, after any move. The tree is 83% of the way to a
split-proof citation convention by habit, and nobody decided to do it.

### Options for what a citation means after the split, priced. No winner.

A. Records travel with their code; each repository numbers its own.
Blocks move to the repository they describe; `nife-kernel` milestone 1 (boot to Rust on QEMU
`virt`) and `basalt` milestone 1 both
exist. *Cost:* the 62% figure. Every cross-group citation needs a repository prefix added by hand or
becomes ambiguous, and `script/citations`' path mode (24 citations) breaks outright since paths stop
resolving in the local checkout. Its title mode (465) survives only if the sibling repository is
checked out. *Kills the strongest gate this tree has for cross-repository citations.*

B. Per-repository prefixes on a single scheme, `nife-kernel#443`, `basalt#12`.
*Cost:* a new thing every reader learns, doing real work in 62% of citations rather than the rare
case. Add a mechanical rewrite of 3,148 sites, whose failure mode §194 (a renumber is the price) names: silently wrong and
still passing. *Buys:* nothing that C does not buy, unless numbers stay authoritative.

C. The slug becomes the identity; the number becomes a nickname.
The record is `design/roadmap/decouple-the-lanes.md`; the number is a display convenience that may
be dropped, duplicated across repositories, or left with gaps. Citations resolve by the gloss, which
83% of them already do. *Cost:* the tree must stop treating the number as authoritative, which
`script/roadmap --check`, `script/lint` check 4b and the `milestone/N-*` branch convention all do
today. *Buys:* every existing well-formed citation survives the split untouched, and no sweep is
required, because the gloss is already there in the cases that matter.

D. A citation resolver that knows several repositories.
`script/citations` grows a manifest of sibling checkouts and resolves across them. *Cost:* a gate
whose answer depends on what the operator has cloned, which is the opposite of the offline property
measured in [`tracker-checks.md`](0533-the-roadmap-after-the-split/tracker-checks.md), and which degrades to a warning in CI. *Buys:* it is the only option
that
keeps a machine-checked cross-repository citation at all, so it is probably a companion to C
rather than an alternative to it.

E. One roadmap forever, in `basalt`; code repositories cite it and hold none of it.
*Cost:* it contradicts "records travel with their code" and re-creates a single global namespace
that every independent release depends on, which is the coupling §151 exists to remove. *Buys:*
nothing changes for 441 blocks or 3,148 citations, and the split touches code only.

### The mechanism the prior art hands us for free

FreeBSD's ports tree has run exactly this problem for thirty years and solved it with a flat file.
From the Porter's Handbook: each line of `MOVED` is *"the name of the port, where the port was
moved, when, and why,"* in the form `old name|new name (blank for deleted)|date of move|reason`,
and *"if the port was removed, the section detailing where it was moved can be left blank."*

A `design/roadmap/MOVED` of that shape, written at the split, turns every option above from
lossy into merely indirect: a number that no longer resolves locally resolves to a line saying where
it went and why. It is rung three of the ladder (a written record at the thing itself), it costs one
file, and it is the only part of this page that is cheap in every option.

## Prior art, read rather than recalled

The proposal's reading of eight other projects (Rust, Go, Kubernetes, Fuchsia, Python, Debian,
FreeBSD ports, Homebrew), with the LLVM move and a withdrawn claim from this lane's first pass, is in
[`prior-art.md`](0533-the-roadmap-after-the-split/prior-art.md).

## Recommendation, with the test from §92 (a caretaker is supervised by the client it serves)

**Keep the argument and the status in the tree. Do not move the roadmap to issues. Enable nothing
and decide nothing about the identifier today, and instead spend one ratchet on the gloss, which is
what makes the identifier decision cheap whenever it is taken.**

Concretely, three items, smallest first:

1. A ratchet on new citations: a `milestone N` or `§N` on a line this commit adds must carry a
   gloss. Answered and built on 2026-09-20: calef approved it the day this was filed, and
   milestone 444 (a citation says what it cites) shipped it as `script/citations --ratchet`, wired
   into `script/lint`. Its census also corrected this file's own number, and the correction is worth
   carrying. The 83% below is 83% *of the citations that already carry a gloss*. That is 505 of
   9,483 (file, scheme, number) pairs, or 5.3% of the tree. The ratchet's rate argument is
   unaffected and the starting point is far lower than this file assumed. Two limits the ratchet
   turned out to have are in `notes/citations.md`'s BUGS. `script/lint` already has this shape at line 1995 (`git merge-base HEAD origin/main`),
   and the word "ratchet" appears in it eleven times. This answers `notes/citations.md`' standing
   objection exactly, which is about a *sweep*: *"Requiring a gloss on the first mention of each
   number in each file means 2,911 sites, every one of which has to be read to know what its author
   meant... So the rule is 'what you write is checked', not 'you must write it'."* A ratchet reads
   nothing retroactively and writes no gloss mechanically. It moves the tree from 83% split-proof to
   asymptotically 100%, at the rate the tree changes, starting now.
2. Write `design/roadmap/MOVED` when the split happens, in FreeBSD's four-field form. One file,
   and it makes every option in section 3 survivable.
3. Use issues for the two things a tree cannot do, as soon as either one is real: a stranger's
   bug report on a released artifact, and a work item spanning two repositories. Not for milestones,
   not for status, not for claims. §90's reason against a second place for truth is intact; that
   reason is about the *same* fact in two places, and neither of these facts is in the tree at all.

The §92 test. *Would I still recommend this if all the options cost the same?* Yes, and the
reason is not effort. A slug says what it names and a number does not; that is why 465 of 560 glosses
are titles although nothing required them. An in-tree record answers a question about the revision
you are standing on and a tracker cannot; that is a property, not a saving. The one place cost is
genuinely deciding is the refusal to renumber, and that is said plainly rather than dressed up.
Renumbering 441 blocks is cheap in agent-hours and expensive in §194's terms. The recommendation
is to not do it because it is dangerous, not because it is long.

Reversibility, per the *move fast on what can be undone* test, which is "who else has already
acted on this". The gloss ratchet is reversible: delete the check, keep the glosses. Which script
reads what is reversible; the decouple-the-lanes lane on #999 is proving it this week by moving two
gates off the generated index. `MOVED` is append-only and additive. The identifier convention is the only
irreversible thing on this page. That is why it is presented as five options with no winner, and
why item 1 exists. A gloss ratchet makes options A, B, C and E converge, so the decision gets cheaper
every week it is deferred rather than more expensive. That is unusual enough to be worth stating,
because the normal shape of a deferred decision here is the opposite.

## What it costs to do nothing, after the decouple-the-lanes work lands

What stays painful once #999 lands, and the proposal's direct answer to calef's question, are in
[`cost-of-nothing.md`](0533-the-roadmap-after-the-split/cost-of-nothing.md).

## BUGS

- **This page does not price letting GitHub mint the number**, although the prior art points
  straight at it and it would dissolve the decouple-the-lanes block's third cause. It is left as an
  option in [`prior-art.md`](0533-the-roadmap-after-the-split/prior-art.md) for three reasons. It
  has option B's post-split problem. 441 existing blocks cannot be reconciled with a PR counter past
  999 without §194's dangerous edit. And it would make `script/lint` and `script/roadmap` depend on
  numbers minted by a host this project might leave. A lane could price it
  properly; nobody has.
- The 62% cross-group figure assumes the split's grain is `kernel/` / `crates/` / `components/`.
  §151 explicitly does not decide the grain (*"This decides the goal, not the grain"*), so the figure
  is an indication rather than a prediction. The direction is robust: any grain finer than one
  repository makes some citations cross-repository, and the citation graph was never built to
  respect a boundary that did not exist.
- **The gloss ratchet is not free of false positives and this page has not measured them.** A line
  that mentions a number in a sentence that is not a citation would
  trip it, and `git grep -w TODO`'s 82% false-positive rate is this tree's standing warning about
  greps that judge prose. Whoever builds it should measure the rate against the last two hundred
  commits before turning it into a failure rather than a report.
- Nothing here measures how often a citation is actually followed. The whole argument that
  citations must resolve rests on the assumption that readers follow them. That is plausible, it is
  what `script/citations` exists for, and it is unmeasured.

## Index row

Answered 2026-09-20 by DECISIONS §201 (one roadmap until a citation has to cross), which rules on
all three asks. The citation identity is deferred under option E with two conditions, a cross-
repository dependency is declared by the...
