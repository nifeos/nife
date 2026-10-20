# Counted claims: a number in the prose is a claim, and a gate can keep it

*The name `counted-claims.md`, and the phrase "counted claim" it introduces, are provisional.
A lane ships a provisional name and says so; naming is an architect's (AGENTS.md). The evidence
this page has no room for is in [`counted-claims/`](counted-claims/README.md).*

DECISIONS §39 says a name is a claim, made before a reader sees a line of code. So is a number, and
a number is the kind of claim a machine can check. That difference is the whole of this convention:
names need a person, counts do not.

## The evidence

Three claimed counts were tested against the tree on 2026-08-14. All three were wrong, and two
of them disagreed with each other as well as with the tree.

| the claim | where it lived | the tree, that day |
|---|---|---|
| "the 23 `#!/bin/sh` scripts" | `script/lint`, `.github/workflows/ci.yml` | 36 |
| "107 harnesses across 19 crates" | `notes/verification.md` | 110 across 19 |
| "112 Kani proof harnesses" | `CLAUDE.md` | 110 |

Every one of them had been right when it was written. That is the finding rather than a detail:
each number is a snapshot of whenever somebody last counted, and nobody re-derives one. The `23`
had drifted by thirteen and sat in two files, neither of which knew about the other.

Two days later, when this convention was built, the harness count had moved again, to 119 across 21.
It moves whenever anyone lands a proof, which is what makes a hand-maintained number hopeless and a
derived one free.

## The convention

Put an HTML comment after the number, naming the count it vouches for:

```markdown
**119 harnesses** <!--count:kani-harnesses--> across 21 crates <!--count:harness-crates-->
```

`script/lint` finds every marker, re-derives that count from the tree, and fails on a mismatch,
naming both values and the file and line. The marker is plain HTML-comment syntax, so it is invisible
in rendered markdown and legal as ordinary text inside a YAML or shell `#` comment, which is why the
same spelling works in `.github/workflows/ci.yml` as in a note.

The number a marker vouches for is the last integer on the line before it, commas allowed, so
`1,303 commits <!--count:...-->` works. Last rather than first, so that a line carrying two counts
binds each marker to the number just in front of it. `notes/unsafe-obligations.md` is the live case
and it writes crates before harnesses, the opposite order to everywhere else, which is exactly why
the rule is positional rather than clever.

A marker inside a fenced block or a backtick span is ignored. The block above is an example of
the convention, not an assertion about the tree, and so is the one in the roadmap; a marker written
as inline code is being named rather than used. Fenced blocks only, though: an indented code block
is not recognized (see BUGS).

## The registry

The other half lives in `script/lint`, one entry per name. Each entry carries the question it
answers, in the prose's own words, and a derivation.

| name | the question it answers | derived from |
|---|---|---|
| `kani-harnesses` | how many Kani proof harnesses the tree carries, which is what `script/verify` proves | `#[kani::proof…]` alone on its line, in any workspace package |
| `harness-crates` | how many workspace packages carry at least one Kani proof harness | distinct packages among those files |
| `sh-scripts` | how many `#!/bin/sh` scripts there are under `script/` and `helpers/`, which is the set shellcheck gates | files whose first line is exactly `#!/bin/sh` |
| `syscalls` | how many syscall numbers the ABI defines, which is the whole width of the trap | `pub const SYS_*: u64` in `crates/abi/src/lib.rs` |
| `rights-bits` | how many named single-bit rights a capability can carry | `pub const NAME: Rights = Rights(1 << N)` in `crates/capability` |
| `loom-harnesses` | how many loom harnesses the tree carries, which is what `script/interleaving-check` runs | `loom::model(` calls in `crates//*.rs` |
| `loom-crates` | how many crates carry at least one loom harness | distinct crate directories among those files |

The last four arrived with the 2026-08-17 documentation sweep, whose lens was the ABI. `syscalls`
exists because the ABI crate's own front page, the kernel's syscall module, an `Error` variant's
description and two notes all said the surface was three calls; `SYS_CAP_DELETE` had landed
three weeks earlier and moved none of them. `rights-bits` exists because `ENUMERATE` made the rights
four and `notes/capability-lifecycle.md` still said three the next day. The `loom-*` pair is the one
this note asked for by name: it cited `notes/interleaving.md` as the claim a marker could never
reach, and measuring it in order to mark it found it wrong in both halves.

**The question is not decoration, it is the entry's most important field.** "How many `#!/bin/sh`
scripts" has at least three defensible answers: `ls script/` gives 37, the shellcheck glob
`script/* helpers/*.sh` gives 40, and "files that literally begin `#!/bin/sh`" gives 40 today and
could give fewer tomorrow. A gate that answers a subtly different question than a human would is
worse than no gate, because it fails a document that is right, and the way it gets fixed is by
somebody deleting the marker.

## EXAMPLES

### Adding a new marked count

The worked example, the tree's harness count from nothing to gated, is in
[`adding-a-count.md`](counted-claims/adding-a-count.md). In short:

1. Decide the question, and write it as a sentence. If you cannot write it without an "or", you
   have two counts.
2. Write the derivation and check it against the real shapes, not the ones you expect. The harness
   grep found 123 where the answer was 119: two hits are in the vendored RedoxFS, which this suite
   has never proved, and two are doc comments in `helpers/kani-lint-shim/`.
3. Add the registry entry to the `==> counted claims` block in `script/lint`, with a docstring
   saying what shape it assumes.
4. Mark the sites: the marker after the number, on the same line.
5. Run the gate. It prints one line on success, and on failure names the file, the line and both
   values, with the question the derivation answers.

### What it found on its first runs

Two things, recorded in [`first-runs.md`](counted-claims/first-runs.md). The `harness-crates`
derivation disagreed with `script/verify`'s hand-kept table by one: `multicast_dns_protocol` had
never been added, so its three harnesses were never proved. And documenting the check overflowed
the markdown renderer's line buffer.

## What this is not

**It is a ratchet, not a sweep.** An unmarked number stays unchecked. The population grows as people
touch numbers, not in one pass, and there is deliberately no attempt at tree-wide markup.

The posture is `script/names --check`'s, which is the closest relative in the tree. That gate insists
a name carry a provenance *state* and never that the state be `ratified`, because a gate demanding
ratification would block every unrelated merge behind a review queue. The same restraint here.
**Insist a marked number be right; never insist that every number be marked.**

Two neighboring classes were measured and deliberately excluded, so nobody builds them here by
mistake:

- TODOs that name a milestone. Mechanically checkable and not worth a mechanism: the whole tree
  has two, and one of them is a retrospective *about* a TODO that was already removed. A gate with
  one live subject is a gate nobody remembers exists.
- Prose asserting the system lacks something it now has, such as a note saying "we don't have that"
  about guard pages that milestones 4 and 90 built. Real, common, and **not checkable**: it needs a
  reader who knows the system, which is milestone 117 (the stranger test).

## Three relations, and equality is the wrong one for anything that moves

```markdown
**124 harnesses** <!--count:kani-harnesses-->                  a census: must EQUAL the tree
**over 100 harnesses** <!--count-at-least:kani-harnesses-->    a floor: fires when the count DROPS
**at most 60** <!--count-at-most:unsafe-density-outside-arch--> a ceiling: fires when it RISES
```

Why the second exists, measured 2026-08-17. `kani-harnesses` was marked at four claim sites and
the count moved three times in one day. So every lane that added a harness had to edit four files,
one of them `README.md`, which nearly every other lane touches as well. **The convention had
manufactured a merge hotspot that the tree itself did not have**, and it did it to the file with the
most traffic in the repository.

Reading what each site actually claimed split them cleanly, and that is the reusable part:

| Site | What it claims | Relation |
|---|---|---|
| `notes/verification.md` | the canonical census, with its own history | equality |
| `notes/unsafe-obligations.md` | "exactly five Kani items, `any` (287), `proof` (124)" | equality |
| `README.md` | coverage and scale, in a table row about `script/verify` | **floor** |
| `notes/fuzzing.md` | comparative context for a different technique | **floor** |

For a monotonically growing quantity, a floor is the true claim and an exact equality is a
maintenance tax that buys nothing. "Over 100 harnesses" is what those two sentences were really
asserting. The exact figure was incidental to the point they were making, and it cost a four-file
edit every time somebody proved something new.

The ratchet is intact, which is the only test that matters. A floor that goes false still fails
the build, and going false means harnesses were deleted, which is precisely the event worth
catching and the one an equality check was drowning in noise. Rounded to 100 it will not move for
months, and when it does, moving it is a deliberate act rather than bookkeeping.

### The ceiling, which is the floor read upside down

The floor suits a quantity where more is better. Unsafe is the other way up: the bad event is an
upward drift no single commit looks responsible for, and only a ceiling says "and it should not
grow". A ceiling is written above the tree, with headroom measured rather than guessed, unless the
population is small and consequential enough that every addition deserves the stop. Choosing the
quantity matters as much as the relation: a count of unsafe blocks would fire on almost every lane,
while a density holds a trend that is already falling. An auto-fix that rewrote marked numbers was
refused, because it biases every disagreement toward "fix the number" when the real bugs are where
the derivation is wrong. [`the-ceiling.md`](counted-claims/the-ceiling.md) has the measurements,
the `agents-md-lines` ceiling that became a two-way ratchet and was retired on 2026-10-06, and the
refusal in full.

## BUGS

- An unmarked number is unchecked, by construction. The gate reports nothing about it, so "lint
  passed" never means "every number in the tree is right". It means every *marked* number is. This is
  the ratchet working as designed and it is the first thing to know about it.

- A floor says less than it looks like it says. `over 100` beside a tree of 124 is true and
  uninformative, and a reader who wants the number has to go to `notes/verification.md`. That is the
  trade: two files stop being a collision point and one indirection appears. If a floor ever drifts
  so far below the truth that it misleads (say the tree reaches 400), raise it, because nothing warns you,
  because a floor with slack is exactly what the relation is for.

- A marker can lie about what it counts. Nothing checks that `<!--count:kani-harnesses-->` sits
  beside a sentence about harnesses rather than about crates; the gate matches the number, not the
  noun. Same limit `script/names --check` records: presence is checkable, meaning is prose, and prose
  is checked by reading.

- A derivation can quietly answer a narrower question than its own name asks, which is the same
  defect one level in, and it is not hypothetical. `kani-harnesses` and `harness-crates` both walked
  `crates/` until milestone 212 (`script/falsifications` walks `crates/` only, so the ratio it prints
  is not the tree's), with a docstring giving the reason as "the kernel, the user programs and xtask
  are not packages it compiles". Milestones 193 (put `kernel/src` within reach of the prover) and 197
  (`user/` and `xtask` are out of reach of the prover) made that false a milestone later. Every
  marker stayed green throughout, because a derivation that is wrong in the same way everywhere
  agrees with itself. The question sentence in the registry is the only thing that catches this,
  and only when somebody reads it against the tree.

- `harness-crates` counts packages and is still called `harness-crates`. Milestone 212 rescoped
  the derivation and left the name, because a marker name is a name and so calef's, and renaming it
  means editing every site that carries it. It is a live example of the bug above: the name asks a
  narrower question than the derivation answers, in the one place that is supposed to keep them
  honest.

- **A marker works in a `.rs` doc comment, and the fence exemption does not follow it there.** The
  2026-08-17 sweep put four in `crates/abi/src/lib.rs` and `kernel/src/syscall.rs`, because the
  claims it was gating live in the boundary artifact rather than in a note, and an HTML comment is
  invisible in rustdoc. The gate finds them: it greps every tracked file. But the fenced-block skip
  is implemented for `.md` only, so a marker inside a ```` ```text ```` block in Rust source is
  checked as a live claim. Nothing in the tree does that today. Keep markers in the prose of a doc
  comment, never inside its fences, and note that the file's own examples cannot demonstrate the
  convention the way a note's can.

- Only fenced blocks and backtick spans are exempt. A marker inside a fence, or inside inline
  code, is being shown rather than asserted, and the gate skips it. Prose explaining the convention
  has to be able to spell it, the same exemption `script/lint`'s rejected-vocabulary check makes for the
  documents that argue about the word. A marker inside a four-space indented code block is *not*
  recognized and will be checked as a live claim. Write examples in fences.

- One number per marker, one marker per number, on one line. A marker on the line after its
  number fails with "no number before it on this line", which is a confusing message for what is
  really a line-wrapping accident. Reflow the paragraph so the two sit together.

- A count spelled in words is read, but only a small one, and never a composed one. The marker
  took digits only until the 2026-08-17 documentation sweep, which needed words. The claim it went to
  gate was "the surface is three calls". A small count in prose about a design is written as a
  word far more often than as a numeral. Cardinals up to `twenty`, plus the round tens and
  `hundred`, are understood; **`twenty-one` is not**, and a marker on one fails with "no number
  before it" rather than passing. That is deliberate rather than unfinished. A count large enough to
  want composing is one a person writes in digits anyway, and a parser that accepted "one hundred
  and twenty" would be guessing at prose. This changed nothing about the ratchet, because only a
  *marked* number is ever read: admitting words cannot make the gate fire on an ordinary sentence
  that says "the three of them".

- **An untracked file is not scanned.** The gate runs `git grep`, so a marker in a file that has not
  been `git add`ed yet is silently unchecked until it is staged. It starts being checked at the moment
  it could reach anyone else, which is the right moment, but it does mean a local run can pass on a
  marker that CI will reject.

- Some counts are too expensive to derive here. `script/lint` runs on every build, so anything
  needing a compile (the frame sizes from `script/stack-frame-check`, the benchmark numbers) does not
  belong in the registry. Refuse the entry rather than quietly making the gate slow; a hated gate gets
  routed around, and a routed-around gate protects nothing.

- **A wall clock is not a count.** `notes/verification.md`'s timing table stays a dated measurement,
  because re-deriving it means running the suite. Dating a measurement is the honest alternative to
  gating it, and the two should not be confused.

- Counts that span the tree are still the integrator's at merge (AGENTS.md). Two lanes can each
  mark a number correctly for their own branch and disagree after the merge. The gate turns that from
  a silent wrong number into a failing group build, which is the improvement rather than a cure.

- `AGENTS.md`'s method figures are unmarked, including a Kani harness count that was wrong on the
  day it was tested. A lane may not edit that file, so the first tranche could not reach it. Marking
  that paragraph is a judgment about how much machinery a piece of rhetoric should carry, and it is
  the obvious next tranche.
