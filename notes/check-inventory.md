# Every check in this repository: does anything run it, does it block, and what does green mean

Milestone 232 (audit every check against two questions: does anything run it, and does it block).
Measured on 2026-09-03 against `a0059022`, on patagonia and against the live repository
settings. Every number below was taken by running the thing or by reading the API, not recalled.

Why an inventory and not a sweep of new gates. Four findings on 2026-09-02 each showed this
tree's verification reporting something that was not true, and no two were the same defect:

- milestone 214 (a test that prints "skipping" and returns is counted as passed);
- milestone 222 (the one command a person runs before pushing has a leg that fails instead of
  skipping);
- milestone 230 (`script/shell-check` is red on `main`, on both architectures, and nothing says so);
- milestone 233 (`login` dies on every boot, and the boot says it is ready).

`script/lint` has had three checks deleted for the "only ever rejects legitimate work" signature, so
the answer to four bad checks is not six more.

## How to re-run this audit

```
gh api repos/nifeos/nife/rulesets                                    # find the ruleset id
gh api repos/nifeos/nife/rulesets/19596094 \
  --jq '.rules[] | select(.type=="required_status_checks")
        | .parameters.required_status_checks[].context'                   # what blocks
grep -n 'name:' .github/workflows/*.yml                                   # what runs
for w in .github/workflows/*.yml; do
    gh run list --workflow "$(basename "$w")" --limit 6 \
        --json conclusion,event,createdAt; done                           # what is red
```

The third command is the one that pays. Two of the five findings below are visible only in run
history and in nothing that lives in this tree.

## The three questions, and which one has teeth

1. Does anything run it? An entry point nothing calls has whatever result somebody last saw.
2. Does it block? CI runs nineteen checks on a pull request and the `main` ruleset requires
   eleven. The other eight are worth deciding rather than inheriting.
3. What does a green result actually assert? This is the one an inventory cannot answer by
   listing. `script/swish-check` was green while `login` was dead on every boot; that check ran and
   would have blocked, and its passing simply meant less than its name.

## A. The workflow jobs

Nineteen check names reached a pull request then. The 2026-09-30 split (#1468) replaced the first
row with four jobs pending calef's ruleset edit; later checks have no rows yet. "Blocks" means in
ruleset 19596094's `required_status_checks`.

| check name | workflow | trigger | blocks | result 2026-09-03 |
|---|---|---|---|---|
| `test`, `swish-check`, `boot-check` | ci | PR, merge queue, push | pending | new |
| `rustfmt` | ci | PR, merge queue, push | **yes** | green |
| `lint` | ci | PR, merge queue, push | **yes** | green |
| `cpu matrix (riscv64 across QEMU CPU models)` | ci | PR, merge queue, push | **yes** | green |
| `bench (icount regression tripwire)` | ci | PR, merge queue, push | **yes** | green |
| `coverage (host crates)` | ci | PR, merge queue, push | **yes** | green |
| `supply chain (advisories, licences, vendored integrity)` | ci | PR, merge queue, push | **yes** | green |
| `fastpath footprint (the IPC path must stay L1i-sized)` | ci | PR, merge queue, push | **yes** | green |
| `stack frames (no frame over a third of a thread stack)` | ci | PR, merge queue, push | **yes** | green |
| `fuzz (parsers, 60s per target)` | ci | PR, merge queue, push | **yes** | green |
| `verify (Kani proofs)` | verify | PR, merge queue, push | **yes** | green |
| `image permissions (no kernel image ships a writable-executable segment)` | ci | PR, merge queue, push | no | green |
| `re-falsify the harnesses this change can reach` | verify | PR, merge queue, push | no | green today, **was red through a merge** |
| `architect hold (needs-architect label)` | architect-hold | PR, merge queue | no | green |
| `verify scope` | verify | PR, merge queue, push | no | green |
| `prove (shard N/3)` | verify | PR, merge queue, push | no | green |
| `draft gate` (two jobs, one name) | ci, verify | PR, merge queue, push | no | green |

Three of the unrequired five are correct as they stand. `verify scope` and the two `prove` shards
are aggregated by `verify (Kani proofs)`, which is required. It fails unless scope succeeded and
every shard reported `success` or `skipped`. Requiring the shards as well would add names to
keep in sync and catch nothing. `draft gate` produces an output rather than a verdict.

The scheduled workflows, which nothing blocks by construction, are nine: mutation testing, the
undefined-behavior check, toolchain drift, audit cadence, the falsification sweep, project metrics,
stranger cadence, vendor watch and toolchain bump. `script/cadence-check`, from milestone 238 (two
scheduled checks have never once succeeded), now derives their last result rather than leaving it to the Actions tab. On 2026-09-03 the first two
had never once succeeded. The table of each one's cadence, result and purpose is in
[`check-inventory/scheduled-workflows.md`](check-inventory/scheduled-workflows.md).

## B. The `script/` entry points

Fifty-four files. Not all of them are checks: `apropos`, `bootstrap`, `setup`, `update`, `console`,
`server`, `board-console`, `board-image`, `catch-up`, `qemu-check`, `ci-qemu` and
`runner-container` are tools or provisioning. What follows is every entry point that renders a
verdict or a measurement, and who calls it.

*The "called by" column names `script/ci-build`'s table (milestone 286 (one enumeration of the
checks that gate a pull request)); the measurements are 2026-09-03's.*

| script | called by | blocks a merge | result 2026-09-03 |
|---|---|---|---|
| `fmt --check` | `ci-build` (`local`), ci `rustfmt`, the `pre-push` hook | yes | green |
| `lint` | `ci-build` (`local`), ci `lint` | yes | green |
| `test` | `ci-build` (`local`), ci `test`, `toolchain-drift` | yes | green |
| `swish-check` | `ci-build` (`local`), ci `swish-check` | yes | green |
| `icount` | `ci-build` (`local`), ci `bench` | yes | green |
| `image-permissions` | `ci-build` (`local`), ci `image permissions` | **no** | green |
| `bench --check` | ci `bench` | yes | green |
| `coverage` | ci `coverage`, `script/metrics` | yes | green |
| `cpu-matrix` | ci `cpu matrix` | yes | green |
| `fastpath-footprint` | ci `fastpath footprint` | yes | green |
| `stack-frame-check` | ci `stack frames` | yes | green |
| `stack-depth-check` | ci `stack frames` (same job) | yes | green |
| `fuzz` | ci `fuzz` | yes | green |
| `supply-chain` | ci `supply chain`, `vendor-watch` | yes | green |
| `vendor-verify` | `script/supply-chain`, `vendor-watch` | yes, transitively | green |
| `verify` | `verify.yml` `prove` shards | yes, via `verify (Kani proofs)` | green |
| `falsifications --affected-since` | `verify.yml` `falsify` | **no** | green |
| `falsifications --check` | `script/lint` | yes | green |
| `roadmap --check` | `script/lint` | yes | green |
| `decisions --check` | `script/lint` | yes | green |
| `citations --check` | `script/lint` | yes | green |
| `fatal-risks --check` | `script/lint` | yes | green |
| `fatal-risks --selftest` | `script/lint` | yes | green (11 fixtures; 8 must go red, 3 must stay quiet) |
| `names --check` | `script/lint` | yes | green |
| `audits --check` | `script/lint` | yes | green |
| `stranger-test --check` | `script/lint` | yes | green |
| `audits --due` | `audit-cadence.yml` | no | **red, and correctly: two audits are due** |
| `stranger-test --due` | `stranger-cadence.yml` | no | green |
| `mutation` | `mutation.yml` | no | **red every week since it was written** |
| `undefined-behavior-check` | `undefined-behavior-check.yml` | no | **red** |
| `drift` | `toolchain-drift.yml` | no | red (against a nightly newer than the pin) |
| `metrics` | `metrics.yml` | no | unknown, never run |
| `vendor-watch` | `vendor-watch.yml` | no | green |
| `toolchain-bump` | **nothing** | no | unknown; `toolchain-bump.yml` reimplements a subset |
| `interleaving-check` | the `interleavings` job in `ci.yml` (milestone 746, provisional) | yes | green, 24 seconds cold (2026-10-03) |
| `crate-probes` | **nothing** | no | **green, measured today, 43 of 50, about 3 minutes** |
| `repeat-under-load` | `script/runner-container`, which nothing calls | no | unknown |
| `soak-test` | **nothing** | no | unknown |
| `rule-violations --check` | **nothing** | no | green (2 open strikes, threshold 3) |
| `journeys` | **nothing** | no | report only, cannot fail |
| `apropos`, `catch-up` | **nothing** | no | tools, no verdict |

`script/lint` is one required check carrying 42 sub-checks (ten clippy passes and 32 others;
the list is its own `==>` lines, and `grep -c '^echo "==>' script/lint` is the derivation, since the
number written here has already been stale once). They all block, because it exits on the first
failure. That
concentration is worth knowing: a slow or wrong sub-check there stalls every lane at once, which is
why three have been deleted rather than fixed.

## C. Checks that are neither a script nor a workflow job

An inventory that stopped at `script/` and `.github/` would miss the two mechanisms the four
2026-09-02 findings actually left behind, because both live inside the thing under test.

- **`Testable::run` in `kernel/src/testing.rs`** panics when a test printed the word `skip` and set
  no skip reason. That is milestone 214's rung-two answer, it runs inside every `script/test` boot,
  and it blocks. Its own limits are recorded in that milestone's block: it reads a substring rather
  than a structure, and a reason split across two `write_str` fragments is missed.
- **The killed-thread assertion in `script/swish-check`** fails the run if the kernel reported
  killing any user thread, on both architectures. That is milestone 233's answer to a check that
  passed while `login` was dead, and it was proven able to fail before being believed.

## What a green result asserts, where that is not obvious

Six checks pass for reasons narrower than their names. Every one of these is already recorded
somewhere in the tree; collecting them is the point.

- **The four build jobs, `cpu matrix`, `bench`, `fuzz` and `coverage` are green-by-skip on a
  documentation-only change.** Each guards its *steps* on the same predicate
  (`^(notes/|design/|[A-Z_]+\.md$)`) while the job still runs, deliberately, because a required
  check that never reports jams the queue forever. A docs-only pull request collects eight green
  required checks, only `test`'s corpus step executing anything (the #1168 fix). Correct
  behavior, worth knowing.
- **`verify (Kani proofs)` is also green when `verify scope` says nothing can reach a proof.** The
  aggregator treats `skipped` as passing on purpose. The proofs' coverage therefore depends on
  `script/verify --affected-since` being right about what a change reaches, and that predicate is
  the whole gate.
- `script/swish-check` can pass a boot it should have failed, and says so in its own `BUGS`. The
  kernel's fault printer and the userspace console server drive the same UART with nothing
  arbitrating. So the boot-line checks put their teeth on the sentence the progenitor prints when the answer
  is no, and a line destroyed by interleaving is reported rather than failed when the kernel
  demonstrably wrote during the boot.
- **`script/crate-probes` measures compile and link and nothing else**, per its own `BUGS`.
  `tempfile` passes and every one of its operations returns "operation not supported" at run time. A
  green row is not a working crate, and fatal risk 1 rests on this instrument.
- **`script/audits --due` red is the healthy state** when an audit is due. Its last line says so:
  "Red means run the audit. Nothing here ran one, and nothing here can." A reader who treats red as
  breakage will learn to ignore it, which is the failure mode this whole note is about.
- **`script/interleaving-check` models C11, not ARM and not RISC-V.** A bug it finds is real; a
  clean run is not a proof about silicon.

## The five findings

Ranked by what a wrong answer costs. Each is stated here as it stood on 2026-09-03; the evidence for
each is in [`check-inventory/the-five-findings.md`](check-inventory/the-five-findings.md).

1. The mutation workflow had never produced a result, and fatal risk 3 cited it as though it had.
   Four scheduled runs, zero successes, and nobody noticed for four weeks, because a scheduled
   workflow's red has no badge and no notification.
2. The Miri check was red for three weeks on a missing environment variable, not on undefined
   behavior. The one-flag fix turned out to be three layers, each hiding the next (milestone 238).
3. `re-falsify the harnesses this change can reach` does not block, and PR #663 merged with it red
   on 2026-09-03, leaving two stale falsification patches on `main`.
4. Three instruments ran nowhere: `script/interleaving-check` (since in CI, milestone 746 (Loom
   runs in CI)), `script/crate-probes` and `script/rule-violations --check`.
5. `image permissions` and `architect hold` were already-recorded open asks, both waiting on one
   ruleset click.

## The recommendation on the required list

Which checks join the list is a repository setting and calef's. Four to add, with what each would
have caught:

| add | what it would have caught | cost of adding |
|---|---|---|
| `re-falsify the harnesses this change can reach` | PR #663 merging with two stale falsification patches, 2026-09-03 | it already runs on every PR and every merge group; making it required adds no minutes |
| `image permissions (no kernel image ships a writable-executable segment)` | the x86_64 image's RWX segment (milestone 208), which shipped through every gate | already runs; no minutes |
| `architect hold (needs-architect label)` | the 2026-08-27 `gh pr merge --auto` on a `needs-architect` pull request | already runs; one API call |
| nothing else | | |

**On `architect hold`, milestone 232's own block says the third is correct as it stands, since a
label gate should not block a queue. The evidence in the tree disagrees and is worth reading before
deciding.** The workflow was built to be required. It listens for `merge_group` for that reason
alone, it listens for `labeled`/`unlabeled` so the result flips without a push, and it holds its
runtime to one API call so it can never be the check that jams a queue. Its founding incident is a
merge that a non-blocking check could not have stopped. The honest cost is real and is the reason to
decide rather than assume. A label added to a pull request already sitting in the merge queue turns
this red mid-group, and an ALLGREEN group of up to five is evicted together. That is a narrow window
and the label is normally applied long before enqueue.

**Do not add** `verify scope`, `prove (shard N/3)`, or `draft gate`, for the reasons under A.

## What this said about fatal risk 3, without changing its status

The status is `design/fatal-risks/README.md`'s question and calef's. On 2026-09-03 two facts
belonged in front of him. The 92.4% was a month old and could not be refreshed in CI, where the
mechanism had failed every attempt. And the sentence "the weekly workflow already publishes the
report" was false, which is the clause that made a stale number acceptable. Neither said the 92.4%
was wrong when it was taken. The full argument is in [the five
findings](check-inventory/the-five-findings.md). The number has since been refreshed: the 2026-09-14
census found no fall (`notes/mutation-testing/the-first-weekly-censuses.md`).

## BUGS

- ~~This is a snapshot with no mechanism behind it.~~ Half-answered by milestone 238.
  `script/cadence-check` reports any scheduled workflow with no successful scheduled run in 15 days,
  and `helpers/trunk-health.sh` runs it. The shape this bullet proposed, a scheduled job reading run
  history, was refused for the reason the bullet itself was circling: a cron that watches crons dies
  the way its subjects die. The delivery is the `launchd` watcher instead, which has no cadence to
  lapse and does not block a lane. Still a snapshot in the other direction: it answers "has this
  workflow produced a result", never "does its green mean anything", which is the question the rest
  of this document asks and which nothing automates.
- The third question was answered by reading, not by a method. Six checks are listed above whose
  green means less than their name, and they were found by opening each file and asking. There is no
  reason to believe six is the whole set. Milestone 233's `login` was found by somebody asking what a
  passing check proved, and that remains the only known way to find the next one.
- The mutation failure is diagnosed (milestone 238). `--shard 4/4` is an argument error,
  because cargo-mutants counts shards from zero, so shard 4 never ran a mutant and shard 0 never
  ran at all. The other deaths were one mutant going from 1.4 GB to 15.8 GB in twenty seconds and
  taking the runner down, which milestone 277 (bound what one mutant may allocate) bounds. See
  notes/mutation-testing.md and the workflow header. The lesson is about the *category*: a
  scheduled check that fails silently. That is why `script/cadence-check` exists.
- The `result 2026-09-03` column for the required checks is read from `main`'s last run, not from
  a run of this branch. A check green on `main` this morning is not a promise about tonight.
- `script/toolchain-bump`'s status is unknown and was not measured. Running it would raise the
  pin in the working tree, which is not something an audit should do.
