---
status: PROPOSED
raised: 2026-11-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A new Kani harness must arrive falsified

Raised by the lane that drove the unfalsified harness count back to 0 on 2026-11-03 (UTC). The
count had risen from 0 to 2 to 4 over two weeks (2026W43 to 2026W45).

## The gap

`script/falsifications --check` gates on presence: a harness must carry a `Falsification:` block,
and `unfalsified` satisfies it. That is deliberate and the script says why. A gate that demanded
the queue be drained would block every unrelated merge. But the rule has a second half that was
only built for one kind of test. The new-test ratchet (milestone 749) refuses a new
`#[test_case]` under `system_tests/src/user/` that has no block, and it also accepts
`unfalsified`. No ratchet of any kind looks at a new Kani harness.

So a lane can add a harness, write `Falsification: unfalsified`, and pass every gate. The metric
then counts it and the next weekly record shows the rise. Both harness pairs were added that way:

- `interfaces_note` (two harnesses), in the pull request that moved the INTERFACES note to a leaf
  crate (#3043). The block even reads "Kani does not run in the lane that wrote this; the gate
  does", so the author knew the harness had never been seen to fail.
- `block_roster` (two harnesses), in the pull request that made the crate stand alone and proved
  the reader (#3062).

The authors were not careless about the rule. The rule lets them stop one step short, and the
cheapest line that passes is `unfalsified`.

## What is proposed

Hold a Kani harness that is new by name to a stricter floor than an old one: its block must be
`replayable`, `attested` or `unfalsifiable` with a reason. `unfalsified` stays legal for the
existing ones, so the ratchet blocks no unrelated merge. New is decided by name against the merge
base, exactly as the milestone 749 ratchet does, so a moved harness is not new.

Two things the milestone needs to settle:

- Whether `attested` is enough for a new harness, or whether only `replayable` is. Recommendation:
  `replayable`, since the lane that writes a harness is the best placed to write the defect, and
  `--affected-since` already replays a record on the pull request that adds it.
- Whether the per-pull-request sweep must run the new record before merge. It does today for a
  changed crate. A lane that cannot run Kani locally would learn from the gate, which is the
  cost the `interfaces_note` author named.

Restrictive first, relax later: a stricter floor for new harnesses can be loosened if it proves
too costly, and a loose floor cannot be tightened without a backlog.
