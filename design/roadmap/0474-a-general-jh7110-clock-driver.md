---
status: REFUSED
raised: 2026-09-20
refused_by: 220, 448
---
# 474. A general JH7110 clock driver, covering all five domains

Refused by milestone 220 (design/roadmap/0220-jh7110-clock-and-reset.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '220. This kernel drives no clock or reset controller, and the first real device will need
one', under `## Follow-on`:

> A general JH7110 clock driver covering all five domains and every clock. The milestone's own
> `BUGS` named unbounded scope as its main risk and the two ends differ by an order of magnitude;
> the arithmetic here is general enough that a second domain is a table entry rather than a
> rewrite, so building the other four before anything needs them would be work with no reader.
>
> -- design/roadmap/0220-jh7110-clock-and-reset.md

## Why it is here rather than only there

The JH7110 has five clock domains and the two ends of them differ by an order of magnitude in
complexity. The milestone's own `BUGS` named unbounded scope as its main risk, and what shipped is
arithmetic general enough that a second domain is a table entry rather than a rewrite.

## Revisit

- **Condition.** A driver that needs a second clock domain. The refusal's own argument supplies the
  trigger: building the other four before anything needs them would be work with no reader, and the
  shape that shipped means the fifth arrives as data rather than as code.

## Index row

Breadth refused on the ground that it would be work with no reader, with the existing implementation
shaped so that each additional domain is a table entry. The condition is a first consumer, which is
the same test this tree applies to mechanisms generally.
