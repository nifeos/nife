# The check inventory: the five findings in full

*Name: provisional. Named after the parent's own heading.*

An appendix to [`notes/check-inventory.md`](../check-inventory.md), measured on 2026-09-03 (UTC)
against `a0059022`. The parent carries each finding in a sentence or two. This is the evidence for
each, and what fatal risk 3 should be read with. Later corrections are in the parent's BUGS.

Ranked by what a wrong answer costs.

## 1. The mutation workflow has never produced a result, and fatal risk 3 cites it as though it had

`design/fatal-risks/README.md`'s third risk (the tests do not test anything, and the quality is illusory)
stands at MEASURED, green, on a run from 2026-08-03: 92.4% of viable mutants killed. Its closing
line is *"the weekly workflow already publishes the report."*

It has published nothing. Four scheduled runs (2026-08-10, 08-17, 08-24, 08-31), zero successes, at
least one shard red every week. On the most recent run all four shards died together about fourteen
minutes in with `The runner has received a shutdown signal`. That is the shape of the whole run
being reclaimed rather than of a defect in any shard; on the three before it, shard 4 died in about
twenty seconds every time while others ran for half an hour. The `report against the baseline` job
runs `if: always()` and aggregates whatever shards survived, into a step summary nobody has read.

Nobody noticed for four weeks, and the reason is structural rather than careless. A scheduled
workflow's red is an entry in the Actions tab with no badge and no notification, which is exactly
the diagnosis `toolchain-bump.yml`'s own comments already wrote down about `toolchain drift` on
2026-07-31.

## 2. The Miri check has been red for three weeks on a missing environment variable, not on undefined behavior

`crates/documentation/tests/render.rs:279` reads `CARGO_MANIFEST_DIR` at run time, deliberately, with a
comment explaining that the compile-time form bakes a stale absolute path. Miri does not forward the
environment by default, so `every_character_survives` panics with `cargo sets this for tests:
NotPresent` and the job exits 1. The same test passes under `cargo test`.

So milestone 79's check (Miri over the host crates) has reported failure for three consecutive weeks
for a reason that has nothing to do with undefined behavior. That is worse than a check nobody
runs: it is a check that cries wolf on a schedule, and the only available response to it is to stop
reading it. The candidate fix is one flag, `MIRIFLAGS=-Zmiri-env-forward=CARGO_MANIFEST_DIR`, which
Miri's own message suggests; it is not applied here because that is a change to a check rather than
an audit of one.

It was not one flag, and the correction is worth keeping (milestone 238 (two scheduled checks never once
succeeded), 2026-09-03). The flag
works and reveals a second wall: the same test calls `read_dir`, which Miri's isolation refuses.
Behind *that* sat five `board_console` tests doing host I/O, invisible until the first two were
cleared because `cargo miri test` stops at the first failure. Three layers, each hiding the next.
The estimate above was made in good faith from the one error message available, and that is the
general hazard in this document: an audit reads the symptom it can see, and a symptom is not a
count of causes.

## 3. `re-falsify the harnesses this change can reach` does not block, and something walked through the hole

`verify.yml`'s `falsify` job carries a comment saying it is not a required check deliberately, so
that it can fail loudly without being one more name to keep in sync. On 2026-09-03 PR #663
(milestones 231 and 233) merged with that check red while all eleven required checks were green,
and `main` carried two stale falsification patches until a follow-up landed.

The deliberate reasoning is sound about ruleset maintenance and wrong about what the check is. A
falsification record that no longer falsifies is a defect in the commit that caused it, which is the
job's own comment saying so; a defect in the commit is the definition of something that should block
the commit.

## 4. Three instruments run nowhere, and two of them are cheap

- **`script/interleaving-check`** (milestone 80 (Loom: the hand-rolled atomic protocols,
  model-checked)) was in no workflow and no gate at audit, though it is the only thing
  that can falsify a violation of AGENTS.md's fourth rule, assume weak memory ordering. Measured:
  12.4 seconds, 26 harnesses green, including the falsification witness. Milestone 746 (Loom
  runs in CI), provisional, added the `interleavings` job in `ci.yml`. It is required (calef, 2026-10-04 UTC).
- **`script/crate-probes`** is the instrument behind fatal risk 1 (only software written for nife
  runs on nife), which is recorded GREEN. Its own `BUGS` explains why it is not a CI gate: it needs
  the network. Measured today: 43 of 50
  built, 7 failed, in about 3 minutes including the std farm refresh. The recorded 43/7 split in
  `notes/crates-io-on-nife.md` still holds, and the seven failures are the same seven
  (`zip`, `ring`, `gix-config`, `gix`, `tar`, `diesel`, `rocket`). This is the one place the audit
  found the record already true.
- **`script/rule-violations --check`** exits non-zero when any open rule reaches three strikes.
  Nothing calls it. It is green today at two open strikes across three rules, so nothing has been
  missed yet, but the threshold milestone 118 defined is currently checked by nobody.

`script/repeat-under-load` and `script/soak-test` are also uncalled and are genuinely instruments rather
than gates: one is milestone 62's acceptance evidence for a flake, the other is milestone 219's
rehearsal of a bench run. Neither wants a cadence; both want a caller when the question they answer
is being asked.

## 5. `image permissions` and `architect hold` are already-recorded open asks

Neither is a discovery. `ci.yml`'s `image-permissions` job carries a `BUGS` paragraph saying it is
not required because that is a repository setting rather than a file in this tree, and that making
it required is calef's and is one checkbox. `architect-hold.yml` describes itself three times as "a
required status check" and its job comment says the name "is what goes into ruleset 19596094's
`required_status_checks` once someone with admin access adds it". Both are waiting on the same
click.
## What this says about fatal risk 3, without changing its status

The status is `design/fatal-risks/README.md`'s question and calef's. Two facts belong in front of him
before he re-reads it.

**The number is older than it looks and cannot currently be refreshed.** 92.4% was measured on
2026-08-03. 2,529 commits have landed since, and the roadmap has gone from that month's count to 130 milestones marked BUILT. The block says the
remaining experiment is cheap, "re-run it and compare against `.cargo/mutants-baseline.txt`", and
that is still true on a developer machine; it is not true in CI, where the mechanism has failed
every attempt.

The sentence "the weekly workflow already publishes the report" is false and has been since the
workflow was written. That is the load-bearing clause: it is what makes a stale number acceptable,
because a refresh is supposed to be arriving on its own. Nothing is arriving.

None of this says the 92.4% was wrong when it was taken. It says the tree has no current measurement
and no working mechanism for taking one, which is a different claim from the risk being red.
