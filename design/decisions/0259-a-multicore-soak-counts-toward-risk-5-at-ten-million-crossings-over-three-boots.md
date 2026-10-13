---
status: DECIDED
raised: 2026-10-07
decided: 2026-10-07
ratified_by: calef
---

# 259. A multicore soak counts toward risk 5 at ten million crossings over three boots

## The ruling

calef answered "B" on 2026-10-07 (UTC) to the question `design/fatal-risks/multicore-reliability.md`
asks in its item 4 and milestone 201 (is multicore reliability converging)'s Follow-on asks again: how long must a multicore soak run
before a board's rows count toward risk 5's verdict.

A board's exposure rows count toward the verdict for an architecture once that architecture has
**at least 10 million crossings across at least 3 boots**, stated before the run. Crossings are the
`crossings=` counter at the last beat, the unit calef ruled the curve is judged on on 2026-09-25.

## Why

Rule of three: zero defects in N trials bounds the rate below 3/N at 95% confidence. Ten million
crossings with zero defects rules out a rate above about 1 in 3.3 million. The boot minimum is there
because each boot is one draw of the placement lottery, and radon's 4.1 million crossings are almost
all one boot (E4).

## What it means for each board

- radon needs about 6 million more crossings over at least 2 more boots.
- xenon needs the full 10 million, over at least 3 boots.
- argon needs the same, and sits behind its own delivery.

## Defects do not reset the count

A defect found during a soak stays on the curve and the count carries on. The verdict reads the
slope per architecture, not a clean streak. The bar says when there is enough exposure to read a
slope, not when the answer is green; a flat curve is still a confidence and never a proof.

## Refused

- 4 million crossings over any number of boots. One draw already meets it, so it is not a bar.
- 30 million crossings over 5 boots. It costs about 52 more radon hours for a bar nothing yet
  needs. A later ruling can raise it if the curve asks for more.
