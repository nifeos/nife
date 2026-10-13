---
status: BUILT
built: 2026-10-03
raised: 2026-09-26
promoted_from: the-confinement-table-lists-the-unvouched-child
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 673. The confinement table lists the unvouched child

*Title and slug are drafts.*
Raised by milestone 198 (a package manager) rung 3a's D2 lane,
which built and falsified the claim milestone 202 (every confinement test is a ritual until
somebody breaks the confinement) was owed and could not add its row. Name provisional.

An editing pass on one note, reversible, and a maintainer's to run.

The claim: an unvouched child holds no capability its caller did not delegate, beyond the clock
and configuration pages (§219 (how the shell names an installed program to the spawner), gate D2).
It is tested by `script/swish-check`'s `installed/unvouched` line and was falsified three times by
hand on aarch64, once per authority. The account is notes/packages/running-unvouched.md.

It belongs as row 31 of notes/confinement-claims.md's table. Touching that note costs three things
the lane did not take on:

- The bold touch rule. The note carries 99 bold spans in about 9,000 words; a touched document
  must end at 4 per 1,000, so about 65 go.
- The citation gate glosses every citation on a changed line, and stripping bold changes about
  seventeen cited lines.
- The note is over §212 (a prose budget)'s word cap, so the row and the glosses have to be paid for by cuts.

Done means the row is in the table, the note passes `script/lint`, and milestone 202's Follow-on
entry for this proposal is marked done.

## Built

Row 31 is in the table, tested by the `installed/unvouched` line of `script/swish-check` (all three
legs) and falsified by `xtask/falsifications/swish_check.boot.swish_check_boot.patch`, replayed on aarch64
on 2026-10-03 (UTC): exit 0 without it, exit 1 with it, the child reporting `domain: REACHED` and
`slots held: 0 1 2 7`. The row counts are now 31 (`design/fatal-risks/README.md` and
`design/fatal-risks/the-confinement-claims.md`). Milestone 202's Follow-on entry is marked done.

Limits, recorded where a reader meets them: the record was replayed by hand, because the sweep
could not replay a swish-check line until milestone 742 (every test is falsified as routine). Only
the domain defect has a patch, and only aarch64 was replayed. The proposal's cost estimate (about 65 bold spans) was not paid: the
prose ratchet judged the row against its baseline, and the lane cut three restating sentences from
the note to keep its word count at or under main's.

## Follow-on

- **Done.** Row 31 and the counts, carried by this lane; milestone 202's Follow-on entry is marked done.
- **Recorded.** The sweep cannot replay a swish-check line, the record covers the domain defect only
  and only aarch64 was replayed: the limits sit beside the record, in
  `xtask/falsifications/swish_check.boot.swish_check_boot.patch`.

## Index row

The claim that an unvouched child holds nothing its caller did not delegate is tested and falsified by hand but has no row in `notes/confinement-claims.md`. Proposed: add it as row 31, an editing pass that touches the note's bold touch rule.
