---
status: REFUSED
raised: 2026-09-20
refused_by: 112, 448
---
# 453. A lint for a SAFETY comment on a safe fn

Refused by milestone 112 (design/roadmap/0112-safety-comments-that-bind.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '112. The SAFETY comments that bind nobody', under `## Follow-on`:

> A lint for "a SAFETY comment on a safe fn". Neither unsafe lint can read a comment, and a check
> for this shape would fire on the legitimate uses the block separates out, where "caller" means
> the calling thread or process rather than a soundness obligation. If the distinction ever turns
> out to be mechanical, `design/decisions/0061-lints-on-evidence.md` is the ledger that adopts a
> lint on evidence from this tree.
>
> -- design/roadmap/0112-safety-comments-that-bind.md

## Why it is here rather than only there

Neither of the unsafe lints can read a comment, so the check would have to be written by hand, and
the shape it would fire on is not the shape that is wrong: a `SAFETY` note on a safe function is
sometimes a soundness obligation nobody holds and sometimes an ordinary caveat where "caller" means
the calling thread or process.

## Revisit

- **Condition.** The distinction turning out to be mechanical, which is the refusal's own wording.
  `design/decisions/0061-lints-on-evidence.md` is named there as the ledger that adopts a lint on
  evidence from this tree, so the route is already written and what is missing is the evidence.

## Index row

A lint here would fire on the legitimate uses as well as the defective ones, because the difference
is what the word "caller" means in a sentence. The refusal names the ledger that would adopt it if
the difference ever becomes mechanical, which makes it a door with a bell rather than a dead end.
