---
status: NOT-STARTED
promoted_from: a-new-kani-harness-must-arrive-falsified
raised: 2026-11-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1037. A new Kani harness must arrive falsified

Raised by the lane that drove the unfalsified harness count back to 0 on 2026-11-03 (UTC). The
count had climbed from 0 to 2 to 4 over two weeks (2026W43 to 2026W45).

Reuse: none exists to take. The proposal extends the new-test ratchet of milestone 749 (a new
confinement test carries a falsification record), which lives in `script/falsifications`.

## The gap

`script/falsifications --check` gates on presence. A harness must carry a `Falsification:` block,
and `unfalsified` satisfies it. The script argues for that on purpose: a gate that demanded an
empty queue would block every unrelated merge. But the ratchet of milestone 749 covers one kind of
test only. It refuses a new `#[test_case]` under `system_tests/src/user/` with no block, and it
also accepts `unfalsified`. No ratchet of any kind looks at a new Kani harness.

So a lane can add a harness, write `Falsification: unfalsified`, and pass every gate. The metric
then counts it, and the next weekly record shows the rise. Both pairs arrived that way:

- Two `interfaces_note` harnesses came with the pull request that moved the INTERFACES note to a
  leaf crate (#3043). Their blocks read "Kani does not run in the lane that wrote this; the gate
  does", so the author knew nobody had seen either harness fail.
- Two `block_roster` harnesses came with the pull request that made the crate stand alone and
  proved its reader (#3062).

The authors did not break the rule. The rule lets a lane stop one step short, and the cheapest
line that passes is `unfalsified`.

## What is proposed

Hold a Kani harness that is new by name to a stricter floor than an old one. Its block must say
`replayable`, `attested` or `unfalsifiable` with a reason. `unfalsified` stays legal for the
harnesses that exist today, so the ratchet blocks no unrelated merge. The ratchet decides new by
name against the merge base, as milestone 749 does, so a moved harness does not count as new.

The milestone needs to settle two things:

- Whether `attested` is enough for a new harness, or only `replayable`. Recommendation:
  `replayable`. The lane that writes a harness knows best which defect should turn it red, and
  `--affected-since` already replays a record on the pull request that adds it.
- Whether the per-pull-request sweep must run the new record before merge. It does so today for a
  changed crate. A lane without Kani learns the verdict from the gate, which is the cost the
  `interfaces_note` author named.

Restrictive first, relax later: a lane can loosen a stricter floor if it proves too costly, and
nobody can tighten a loose floor without first clearing a backlog.

## Index row

A new Kani harness may carry `unfalsified` and pass every gate, and the count of such harnesses has risen. Proposed: a harness new by name must record a replayable, attested or unfalsifiable falsification.
