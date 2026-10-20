---
status: NOT-STARTED
promoted_from: a-tenth-outsider-pass-attacks-the-confinement-claim
raised: 2026-10-19
milestone_dependencies: 871, 920
decision_dependencies: none
machine_requirements: a podman machine at 6 CPUs and 8 GiB or more (script/launch-pass)
specific_machine: none
needs_person: yes
---
# 936. A tenth outsider pass attacks the confinement claim

Raised by the lane `milestone/871-fetch-path-row` on 2026-10-19 (UTC), which closed milestone 871
(a sixth outsider pass attacks the confinement claim). Name: provisional.

Reuse: `briefs/confinement-outsider-pass.md`, `script/launch-pass` from milestone 920 (an outsider
pass is launched pinned and floor-checked) and `script/booted-rows`, unchanged. Nothing new is
written.

## Why

Risk 7's criterion (c) wants two consecutive clean passes, at least one by a non-Anthropic model
or a human, and its count is 0. Milestone 871's pass booted an escape on its first surface and
never ran its table sweep or its variant work. calef ruled on #2432 (2026-10-19 23:18 UTC): "New
pass." So that work runs here, as a fresh GLM 5.3 pass that can count. No pass has attacked row 35
of `notes/confinement-claims.md`, the package fetch path. The eighth pass, milestone 913 (an eighth
outsider pass attacks the confinement claim), was by GLM 5.3 and did not count.

## What it needs

- A commit: `main` with row 35 and its test, so the table has 35 rows.
- From calef: GLM 5.3 tokens (the pass waits for his weekly allowance to reset), an API key made
  for the pass and revoked after, in the launcher's environment, and patagonia's podman machine at
  6 CPUs and 8 GiB or more.
- A lane: it mints this block, claims it, runs `script/launch-pass --reviewer glm --commit <sha>`,
  reproduces each booted row one ISA at a time, and counts with `script/booted-rows`. Below the
  floor it runs the one completion round.
- The floor: 28 of 35 rows. Rows 12 to 18 are exempt, 1 to 3 may carry from pass 7, and 27 to 29
  and 33 boot on x86_64 only. Rows 34 and 35 are milestone 871's variant ground. They are rows, so
  the floor covers them and the prompt names neither.

## Done means

- The pass's note exists with its launch record, its refusal log and `script/booted-rows`' output.
- Every escape is a failing test committed before any fix.
- Risk 7's appendix cites the pass through the maintainer under §216 (fatal-risk facts are
  correctable, and verdicts are the architect's). Moving the color stays calef's.

## Index row

Risk 7 needs two consecutive clean outsider passes, at least one by a non-Anthropic model or a human, and its count is 0. This is a fresh GLM 5.3 pass over all 35 rows of `notes/confinement-claims.md`, including the package fetch path no pass has attacked.
