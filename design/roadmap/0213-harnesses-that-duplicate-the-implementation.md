---
status: BUILT
raised: 2026-09-01
built: 2026-09-02
---
# 213. A harness that re-implements the code instead of calling it proves nothing about the code

Swept 2026-09-02: **148 harnesses read, one measured blind, rewritten, and carrying a
machine-replayable record of the defect its old phrasing could not see.** Two more have the shape
and are recorded as *not* findings, one of them measured blind on its own and covered by a neighbor.
The result and its method are in notes/falsification.md, under "The sweep for harnesses that
duplicate the implementation".

It never had a gate, for 211's reason: no check tells a deliberate model apart from an accidental
copy. That held. The discriminator the sweep worked out is a question a person answers rather
than a pattern a lint matches: **which side of the assertion did the crate produce?** The crate
must produce the subject; the expectation should come from somewhere the implementation does not
choose, which is why it is so often a recomputation. Both sides written by the harness is the
defect. No check can see that difference, because both look like arithmetic beside a call.

## In brief

Milestone 211 asked whether a harness can see the function under test break. The answer for
`nifefs::the_validation_implies_reads_slice_is_in_bounds` is no, for a reason 211's class does not
cover: **it never calls the function.** Neither `Fs::parse` nor `Fs::read` appears in it. The
harness recomputes both functions' arithmetic inline and proves a property of that recomputation:

```rust
// Exactly parse's acceptance condition, no more.
let start = start_block as usize * BLOCK;
...
// Exactly read's slice arithmetic.
let r_start = start_block as usize * BLOCK;
```

The two comments are honest, which is what makes this a design to reconsider rather than a mistake
to fix quietly. But "exactly" is a claim in prose, and a claim in prose is what DECISIONS §134
exists to stop standing in for evidence. Rewrite `read`'s slice arithmetic and the harness stays
green.

## Why it is worth a milestone rather than a one-line fix

The obvious repair, calling `parse` and `read` from the harness, is what the harness's author
avoided, and the reason is probably tractability: `parse` walks a directory over a symbolic image.
So this is a **restructuring** question rather than a harness question, and it is the shape
DECISIONS §46 (thin primitives or whole subsystems) rule 1 already describes. Factor the acceptance
condition and the slice bounds into
functions small enough for the prover, which both the implementation and the harness then call.
`elf`'s `check_segment_bounds` is the worked example already in the tree: a leaf factored out of a
loop precisely so a bounded model checker can quantify over it, with the real parser calling the
same function.

## The sweep, and what it found

One finding in 148, and the block named it in advance. `nifefs`'s harness was measured blind
in both directions: `read` slicing from `(start_block + DIR_BLOCKS) * BLOCK`, a directory span
past what `parse` accepted, left the old phrasing verifying in 0.04 seconds, and the rewrite goes
red on the same patch.

The repair earned its keep, which this block said to decide rather than assume. It did,
for a reason that is not the proof: the duplication was in the implementation first. `parse`
validated with three lines and `read` sliced with a copy of them, and `read`'s unchecked index
was sound only because a reader could see the two matched. `Fs::entry_bounds` has two ordinary
callers and turns that into one expression, so §46's refusal of machinery-for-tidiness does not
bite. The test for the next case: would this function be worth extracting if there were no
harness?

**The mechanical narrowing this block predicted was not attempted, and should not have been.**
The absence of a call to the crate is easy to grep for and is the wrong question: almost every
harness that recomputes something also calls the crate, and the interesting cases are the mixed
ones. 211 measured its own extractor flagging three findings for the wrong reason and missing a
whole family. Every harness was read.

## BUGS

- Some duplication is correct. `intrusive_fifo` keeps a model queue on purpose and compares
  the real one against it, which is the good version of this and must not be swept up. The
  difference is whether the crate's own function is on the other side of the comparison. That
  sentence turned out to be the whole discriminator, and it survived the sweep unchanged.
- The repair can be worse than the defect, and the check for it is above rather than here now
  that one case has been decided. It did not bite in `nifefs`; it would have if the two conditions
  had not already been duplicated in the source.
- One is a floor, and a lower one than 211's eleven. A harness with the crate on neither side
  of its assertion is the pure form of this defect and there was one. A harness whose subject
  comes from the crate can still rest on a recomputed *assumption*, and telling a safe restatement
  from an unsafe one took reading the code rather than applying a rule.
- A model of a caller cannot be repaired this way.
  `kernel::every_page_between_the_checked_ends_is_itself_a_user_page` restates the guard two
  system call paths apply. Both were read and both still match, so it is faithful; there is nothing to
  extract and call, because what is duplicated is control flow rather than a function, and nothing
  re-reads those call sites when they change.
- The 147 cleared harnesses carry no artifact, the same limit 211 records for its own 135. A
  refactor that inlines a function a harness calls turns that harness into this defect silently.

## Follow-on

- **Refused.** A gate. No check tells a deliberate model apart from an accidental copy, which is
  milestone 211's reason and it held through the sweep. The discriminator the sweep worked out is a
  question a person answers, "which side of the assertion did the crate produce?", and both sides
  look like arithmetic beside a call to any pattern a lint could match.
  This refusal is milestone 470 (design/roadmap/0470-a-gate-on-harness-quality.md), which carries it
  with the condition that would change it.
- **Refused.** The mechanical narrowing this block predicted, greping for harnesses that never call
  the crate. It is easy to run and it is the wrong question. Almost every harness that recomputes
  something also calls the crate, so the interesting cases are the mixed ones, and 211 had already
  measured its own extractor flagging three findings for the wrong reason and missing a family.
  Every one of the 148 was read instead.
- **Recorded.** `notes/falsification.md`. The 147 cleared harnesses carry no artifact, the same
  limit 211 records for its own 135. A refactor that inlines a function a harness calls turns that
  harness into this defect silently, and nothing re-checks the cleared set.
- **Recorded.** `design/roadmap/0213-harnesses-that-duplicate-the-implementation.md`. One finding is
  a floor and a lower one than 211's eleven. A harness with the crate on neither side of its
  assertion is the pure form and there was one. A harness whose subject comes from the crate can
  still rest on a recomputed *assumption*, and telling a safe restatement from an unsafe one took
  reading the code.
- **Recorded.** `design/roadmap/0213-harnesses-that-duplicate-the-implementation.md`. A model of a
  caller cannot be repaired this way.
  `kernel::every_page_between_the_checked_ends_is_itself_a_user_page` restates a guard two system call
  paths apply; both were read and both still match, so it is faithful, but what is duplicated is
  control flow rather than a function and nothing re-reads those call sites when they change.
- **Recorded.** `design/roadmap/0213-harnesses-that-duplicate-the-implementation.md`. Some
  duplication is correct and must not be swept up: `intrusive_fifo` keeps a model queue on purpose
  and compares the real one against it. The difference is whether the crate's own function is on the
  other side of the comparison, and that one sentence is the whole discriminator.

## Index row

Found beside milestone 211's sweep, which was looking for a different defect. `nifefs::the_validation_implies_reads_slice_is_in_bounds` calls neither `parse` nor `read`: it
recomputes both functions' arithmetic inline and proves a property of the copy, so a rewrite of `read` leaves it green. Its own comments say "exactly parse's acceptance condition", which is a
claim in prose standing where evidence should be, and DECISIONS §134 exists to stop that. The
repair is a restructuring rather than a harness edit, along the lines of `elf::check_segment_bounds`. Whether it earns its keep was the first thing decided: it does,
because the duplication was in the implementation before it was in the harness. Swept 2026-09-02:
148 harnesses read, one measured blind, two more recorded as having the shape and not being
findings.
