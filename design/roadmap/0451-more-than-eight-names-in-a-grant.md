---
status: REFUSED
raised: 2026-09-20
refused_by: 109, 448
---
# 451. More than eight names in a single grant

Refused by milestone 109 (design/roadmap/0109-xargs-at-the-grant-bound.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '109. `xargs`: batching a grant too large to hand over', under `## Follow-on`:

> Raising `MAX_NAMES` above eight. Eight is measured rather than chosen: a name set travels by
> value through the expander, the `Expansion`, `designate`'s return and the `Endowment`, and at
> sixteen the shell ran off the bottom of its stack planning a single grant, twice. Lifting the
> number means giving the shell an allocator or the grant a different carrier, which is its own
> decision with its own argument, and `xargs` is still wanted afterwards because the ceiling moves
> rather than disappearing.
>
> -- design/roadmap/0109-xargs-at-the-grant-bound.md

## Why it is here rather than only there

`MAX_NAMES` is eight because a name set travels by value through the expander, the `Expansion`,
`designate`'s return and the `Endowment`, and the shell's stack is what pays for each copy. At
sixteen it ran off the bottom of that stack planning a single grant, twice, which is a measurement
rather than a preference.

## Revisit

- **Condition.** Either an allocator for the shell or a different carrier for a grant, which the
  refusal names as its own decision with its own argument. The bell is any change that stops a name
  set traveling by value: the ceiling moves rather than disappearing, and `xargs` is still wanted
  afterwards.

## Index row

The eight-name ceiling is a consequence of a name set being copied by value through four stages on a
shell stack with no allocator, measured by the shell overflowing at sixteen. Raising it is real work
that needs a different carrier first, and this records it as a door rather than a wall.
