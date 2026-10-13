---
status: BUILT
raised: 2026-10-12
built: 2026-10-12
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 903. A seventh outsider pass attacks the confinement claim

*(Title, slug and every name below are provisional.)*

Reuse: the counting rule, the report format and the refusal log are milestone 800 (a
non-Anthropic model attacks the confinement claim)'s, unchanged, so the passes compare row for row.
The isolation is milestone 633 (an outside agent attacks the confinement claim)'s, not 800's: this
pass is blind.

## Index row

The seventh outsider pass at risk 7's confinement claim, by a fresh Claude agent briefed blind with
only the published claims table and the source. It counts an attack only when it boots, and keeps
the standing refusal log.

## Why

Risk 7 (the confinement claim is false) is AMBER. Its criterion (c) for green is two consecutive
independent attacks with no escape on a shipped path, at least one by a non-Anthropic model or a
human. The sixth pass (milestone 871) booted DNS rebinding in the package fetch, fixed in #1901, and
the count restarted at zero. calef asked on 2026-10-12 (UTC) for the seventh pass, with the
independence rule as the point: a fresh reviewer, briefed the way milestone 633 allows.

## The attack

Blind, milestone 633's posture. The reviewer is a fresh agent spawned by this lane and briefed with
two things: the claims table in `notes/confinement-claims.md` and the source tree. The brief names
no audit report and no earlier pass note, and asks the reviewer not to open the claims file's
narrative, `design/audit-reports/`, `design/fatal-risks/`, `design/roadmap/`, any earlier outsider
or falsification note, or git history. The lane does not attack and does not hint. It briefs,
collects, reproduces each claimed escape by test or boot, and records.

## Result

BUILT, 2026-10-12 (UTC). No escape. The record is `notes/confinement-outsider-pass-7.md`, which
holds the per-row table and the refusal log.

- Two attempts. The first reviewer read past the claims table into earlier findings, said so,
  and was discounted. A second fresh reviewer, handed the table alone, ran the pass.
- Booted. Two new tests in `system_tests/src/user/` (names provisional), green on aarch64,
  riscv64 and x86_64, reproduced by the lane on each. In one, `SEND_CAP` cannot widen a right
  through its rights register or lend a `GRANT`-less capability (rows 1 and 2). In the other, a
  `WRITE`-only budget narrows a `SPLIT` child but mints `Rights::ALL` on every `RETYPE` and
  `RETYPE_OBJ` (row 3).
- One near miss, already ruled. The retype half of row 3 is the gap milestone 824 (a retype
  mints no right its budget lacks) fixes. New: it reaches every kernel object, not only frames.
- Read. The other 31 rows, about half resting on the row's own cited test.
- Refusal log. Two external safety-system stops during orientation, one per attempt, and the
  first attempt's independence slip. None on a shipped path.

It does not count toward criterion (c). calef ruled on #2010 (2026-10-12 UTC), option (b): with
only 3 of 34 rows booted the count stays 0, and the next pass's brief sets a minimum of booted
rows. That brief is `briefs/confinement-outsider-pass.md`. Its floor is every row a boot can
reach, 27 of 34 (rows 12 to 18 are host proofs and exempt), ruled by calef on #2010 the same day:
"27 seems right". Rows 1 to 3 count when the next reviewer accepts or strengthens this pass's
tests.

## Done means

- Every claim has an attack, or a written reason it was not attacked.
- Every escape is a failing test committed before any fix.
- The pass-7 note exists with its refusal log, and is indexed in `notes/README/` and its area page.
- Risk 7's appendix cites the pass through the maintainer under §216 (fatal-risk facts are
  correctable, and verdicts are the architect's). Moving the color stays calef's.

## Follow-on

- **Done.** The next pass's brief is `briefs/confinement-outsider-pass.md`, with the booted-row
  floor calef's ruling asked for: 27 of 34, ruled on #2010.

- **Milestone 824.** When it lands, `seventh_pass_retype_tests` goes red on purpose; its `GRANT`
  assertions flip with the existing frame-only characterization.
- **Recorded.** Unreached by this pass: The package fetch (no row until milestone 855 (the TLS graph enters the gated
  build)) and hostile clients of userspace servers. A later pass's targets, recorded in the note.

- **Milestone 198.** A human review, or a public bounty once a stranger can install nife, is the
  stronger form and waits on milestone 198 (a package manager, and the trivial install that makes a
  second customer possible).
