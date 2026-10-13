---
status: REFUSED
raised: 2026-09-20
refused_by: 203, 448
---
# 469. Automating the re-apply of the vendored divergence patch

Refused by milestone 203 (design/roadmap/0203-vendored-engine-upgrades.md), which records it in its
Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names
work a number, a status and a condition that would change it.

## The refusal, in its own words

From '203. Nothing will ever tell us RedoxFS moved', under `## Follow-on`:

> The expensive half of the workflow, which would re-apply the divergence patch and report which
> of the five carried over. Automating a rebase before anyone has performed one by hand is
> guessing at the shape of a job nobody has done; the cheap version raises the pin and lets
> `script/vendor-verify` go red, which makes the upgrade a visible object.
>
> -- design/roadmap/0203-vendored-engine-upgrades.md

## Why it is here rather than only there

The cheap half of the upgrade workflow shipped: raise the pin and let `script/vendor-verify` go red,
which turns an upgrade into a visible object somebody has to deal with. The expensive half would
re-apply the divergence patch and report which of the five hunks carried over, and that is a job
nobody in this tree has yet performed by hand.

## Revisit

- **Condition.** One rebase performed by hand. The refusal's argument is that automating a job
  before anyone has done it is guessing at its shape, so the first real upgrade is both the trigger
  and the specification.

## Index row

Automation refused on the ground that nobody has done the manual job it would automate, which is a
reversible refusal with a cheap and obvious trigger. The visible-failure half shipped instead, so
the upgrade cannot happen silently in the meantime.
