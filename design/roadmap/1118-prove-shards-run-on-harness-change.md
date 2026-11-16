---
status: BUILT
built: 2026-11-16
raised: 2026-11-16
---
# 1118. The prove shards run on a pull request that changes a harness

The rule in force since 2026-11-16 (UTC): verify.yml's Kani jobs run on a pull request whose diff
adds or changes a Kani harness, and every other heavy gate stays in the merge queue. The branch is
`milestone/1118-prove-shards-run-on-harness-change`; the number is provisional.

What drives it: a new harness in #3567 needed more memory than the hosted runner has. The proofs
ran only in the queue, so its failure there cost the entries behind it about two days. On its pull
request it would have failed in about a minute.

Reuse: `helpers/full_ci.py` has the shape (one decision in the draft gate, read by each job's
condition, failing toward running), and `helpers/rust_source.py`'s lexer already tells a comment
edit from a code edit for `script/verify --affected-since`.

## What is in force

- `helpers/harness_change.py decide` runs in verify.yml's draft gate and writes `harness=true` when
  the diff from the merge base adds, removes or edits a `#[kani::proof]` or
  `#[kani::proof_for_contract]` function (its attributes and body, compared as tokens without
  comments), changes a file under a `falsifications/` directory, or changes `script/verify`. A base
  that does not resolve, a failed git command or a file that does not lex answers true. Its
  `--selftest` runs in `script/lint`, and `since <base>` gives the same answer by hand.
- `verify scope`, the five `prove` shards, the two kernel rows, `re-falsify` and
  `verify (Kani proofs)` each read `heavy != 'false' || harness != 'false'`, so a missing answer
  runs them. A pull request that changes no harness skips them, and each reports Success under its
  required name as before.
- notes/merge-queue.md, "The heavy gates run only in the queue, and the proofs also on a harness
  change", states the rule beside the queue-only set.

## Follow-on

- **Recorded.** A change to a function a harness calls, and not to the harness, still proves first
  in the queue: `helpers/harness_change.py`, `BUGS`.

## Index row

A pull request that adds or changes a Kani harness, a falsification patch or script/verify runs verify.yml's Kani jobs before it queues, so a harness too large for the hosted runner fails on its own pull request and not in the merge queue.
