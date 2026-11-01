---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: 151
machine_requirements: none
specific_machine: none
needs_person: yes
---
# Section 151 states the end state

Raised 2026-11-01 (UTC) by the lane `lane/section-151-is-amended-by-the-end-state`. It carries the
exact replacement text for four statements in §151 (the goal of the repository split is
independent release and third-party programs), rewritten in place so that §151 states the end state
ruled on 2026-11-01 (UTC). A lane may not write `design/decisions/`. The architect reads this text
before anyone edits §151.

## Background

§151 was decided on 2026-09-15. It names what the split is for: independent release and
third-party programs. It also says how a piece earns a repository of its own, and it leaves the
order of the split to a later ruling.

The ruling of 2026-11-01 (UTC) sets the end state. This repository breaks up into single-purpose
repositories and remains, as `nifeos/nife`, only for its history. The kernel, programs, tooling and
crates all leave. What leaves references nothing back into this repository, so moves go bottom-up:
the crates a program links leave before it or with it. The kernel leaves last among the code, to
`nife-kernel`, a name that stays provisional until the architect ratifies it. Pull request #3027
recorded this in milestone 857 (crates released on their own) and journey 5, and listed four
statements in §151 that the end state contradicts or strains.

A document here states what is in force now (§298 (a document states what is in force now)), and
git keeps what it said before. So each replacement below reads as the current rule, with no dated
note and no marker of change. What §151 said before stays in git and in this pull request.

## The replacements, ready to paste

Each pair below quotes the current text of
`design/decisions/0151-repository-goal-is-independent-release.md` at `nifeos/nife@b68664cb1`, then
the text that takes its place. Everything not shown stays as it is.

### 1. The independent-release bullet in "What this settles"

Current:

```markdown
- Independent release. Each shippable piece can be versioned and released on its own cadence,
  behind an interface that does not change under its consumers. This is the property that decides the
  grain of the split (see BUGS): a thing earns separation when it can ship on its own schedule, not
  because it is a separate package.
```

Replacement, two bullets in its place:

```markdown
- Independent release. Each piece carries its own version and ships on its own cadence, behind an
  interface that does not change under its consumers' version requirement.
- One purpose per repository. Every piece leaves this repository for a repository of its own with
  one purpose: the kernel, the programs, the tooling and the crates. No piece has to pass a test to
  leave. The order below decides only when it leaves.
```

### 2. The first BUGS item

Current:

```markdown
- **This decides the goal, not the grain.** "One repository per package" was calef's opening framing
  in the same conversation. The maintainer argued for splitting on release cadence instead, since a
  piece earns a repository when it can ship independently and almost nothing can yet while the
  protocols move. This section records that independent release is the *test* for the grain; which
  pieces pass it is measured when the split is scheduled, not asserted here.
```

Replacement: none. The item comes out of BUGS whole. The grain is now a rule, and the second bullet
of replacement 1 states it. The integration-gate item stays as the only BUGS entry.

### 3. "What this does not settle"

Current, the whole section:

```markdown
## What this does not settle, and it is deliberately separate

The order. *When* the split happens, against what preconditions, is not decided here and should be
ruled on its own. The candidate sequencing, from the records rather than invented: the wire protocols
carry real version numbers and have stopped churning weekly (milestone 39, §14), and a package format
exists so `basalt` has something to assemble (milestone 198). Until then the single-command,
all-architecture `script/test` is the credibility mechanism milestone 39 says a premature split would
quietly break, so the strain has to justify the trade. That trade is a separate ruling.
```

Replacement, the whole section:

```markdown
## The order: bottom-up

Nothing that leaves this repository refers back into it. So the split goes bottom-up in the
dependency graph: the crates a program links leave before it or with it, and nothing that has left
depends on a path or a pin into this repository. This repository may depend on what has left. The
leaf pilot `jh7110_entropy` (milestone 818) leaves first, then the contract crates, then the
programs and tooling once the crates they link have left. The kernel leaves last among the code, to
`nife-kernel` (provisional). Once everything has left, `nifeos/nife` holds only the history.

Churn does not hold a move. A contract crate leaves and carries a version while its interface
still changes, and each consumer checks that version (milestone 956). The single-command,
all-architecture `script/test` proves the whole system today, and the split has to keep that proof
running (see BUGS).
```

### 4. Two sentences on the near-term path

The tension #3027 named sits in the model section. The same idea appears once more, in the paragraph
after the "What this settles" bullets, so both are shown.

Current, in the paragraph after the bullets:

```markdown
That recommendation reaches the split when the seams exist rather than by directory
surgery up front.
```

Replacement:

```markdown
Each piece leaves that repository whole, in the order below, rather than by directory surgery up
front.
```

Current, in the model section:

```markdown
**What this does not change** is the near-term path. Develop in one repository now (milestone 39's
option B) and reach the split when the seams exist; the *target model* being a distribution rather
than a base system does not force the split earlier. It corrects the mental model of the endpoint,
and with it the maintainer's earlier BSD-leaning comparison, not the sequencing.
```

Replacement:

```markdown
**The model does not set the order.** The bottom-up rule above does. A piece lives in this
repository until its move, and the moves start at the leaves. The *target model* being a
distribution rather than a base system neither hastens nor holds a move. It corrects the mental
model of the endpoint, and with it the maintainer's earlier BSD-leaning comparison.
```

### The frontmatter

`status: DECIDED` becomes `status: AMENDED`. `raised`, `decided` and `ratified_by` stay. The
decisions README defines `AMENDED` as a section that holds while part of it changed, with the file
stating the decision as it now stands, which is this case. It is a status value, not prose, so no
reader meets it as a dated note.

## What stays unchanged

- The title, the opening quotation, the "Third-party programs" bullet and the first sentence of the
  paragraph after the bullets.
- The Linux-distribution model and its three consequences. A kernel that leaves to a repository of
  its own makes the first consequence literal.
- "What is unblocked", and the integration-gate item in BUGS. The end state makes that gate more
  pressing, and nothing ruled on 2026-11-01 builds it.

## Why each replacement reads as it does

- Replacement 1 keeps independent release as the purpose and drops it as a gate. The first
  sentence changes only to say whose version requirement holds the interface steady: the same
  day's correction on #3027 says interfaces may break, provided consumers can check versions.
- Replacement 2 deletes rather than rewords. A BUGS item records a limitation, and the grain is no
  longer one.
- Replacement 3 drops the package-format precondition along with the rest. Milestone 198 built the
  package format on 2026-09-26, so the precondition already holds, and no ruling makes it a gate on
  a crate's move. `jh7110_entropy` moved without depending on it.
- Replacement 4 keeps the sentences' real point: the target model does not by itself move the
  split earlier.

I applied all five pairs to a scratch copy of §151, and each current text matched exactly. The
result has 956 words against 971 today. The prose ratchet passes it with 1 passive against a
baseline row of 5, and the history ratchet finds nothing in it. The mid-paragraph replacement in 4
is rewrapped to 100 columns with its paragraph when pasted.

§235's amendment and milestone 607 also conflict with the end state, as #3027 lists. They are a
separate question, and this proposal does not touch them.

## Found while writing this

- Milestone 857's 2026-11-01 ruling bullet calls the design record's move "likely, as a direction
  and not a ruling". Journey 5 and §201 (one roadmap until a citation has to cross) state it as
  ruled, to a repository named `nifeos.org`. The replacements above say nothing about the design
  record, so they are correct either way. Milestone 857's bullet wants a record fix.
- Two records ask whether "§151's preconditions" are met: `notes/the-first-split.md` section 1 and
  milestone 756 (process-tools, core-tools and disk-tools leave first). Once replacement 3 lands,
  §151 names no preconditions, and both want a sentence pointing at the bottom-up order.

## What I need from you

The text above is §151 rewritten in place to state the 2026-11-01 end state. Nothing in
`design/decisions/` changes until you approve it.

Recommended: A. Approve the four replacements and the status change as written. A lane then
pastes them into §151 under `script/record-ruling`, the one way a lane may write
`design/decisions/`.

- A. Approve as written.
- B. Approve with changes. Name them, and the lane makes them before pasting.
- C. Refuse. §151 keeps its present text, and its grain test and open order go on reading as rules
  in force beside milestone 857's ruling. Nothing else waits on this.

This blocks only the edit to §151. No move, crate or milestone waits on it.
