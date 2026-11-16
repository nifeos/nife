---
status: BUILT
built: 2026-11-16
raised: 2026-11-16
---
# 1120. The harness-crates claims are floors, so two lanes adding proofs do not race

The rule in force since 2026-11-16 (UTC): two prose sentences say how many workspace packages carry
a Kani proof harness, in `notes/verification.md` and `notes/unsafe-obligations/kani-gate.md`. Both
claim a floor of 30 (`count-at-least:harness-crates`), not an exact count. The branch is
`milestone/1116-harness-crates-floor`.

Both sentences convey scale, and the exact figure was incidental to each. Two correct pull requests
that each added a harness crate made it wrong for whichever merged second. The counted-claims
section of `script/lint` ejected 16 of 97 merge-queue attempts on this one marker, measured on
2026-11-15 (UTC). A floor still fails when the count drops below 30. That is the event worth
catching: a derivation that loses packages, or a run of extractions that empties the prover's reach.

Reuse: the floor relation of [the counted-claims convention](../../notes/counted-claims.md), which
answered the same hotspot on `kani-harnesses`. No code changes.

## What is in force

- `harness-crates` carries no equality marker. Both sites read "at least 30 packages".
- [`notes/counted-claims.md`](../../notes/counted-claims.md) `BUGS` says which counts keep equality
  and which take a floor or a ceiling. Equality suits a count that moves by decision, or one whose
  every change edits the same lines. A count that unrelated lanes move takes a floor or a ceiling.

## Follow-on

- **None.** We measured the other equality markers, and none races. `syscalls`, `rights-bits` and
  `security-audits` move by decision. Each per-crate harness count moves only with an edit beside
  that crate's own line, and git refuses two such edits before the queue sees them. `loom-harnesses`
  and `loom-crates` changed four times in the month to 2026-11-15 and ejected no attempt. They keep
  equality until they do.

## Index row

The harness-crates claims are floors at 30, so two lanes that each add a proof crate no longer
eject each other from the merge queue.
