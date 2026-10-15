---
status: NOT-STARTED
raised: 2026-10-15
milestone_dependencies: 740
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 924. A survivor on a pull request's own lines cannot merge untriaged

This is the rule milestone 479 (a blocking `--in-diff` mutation gate) refused. calef ruled on 2026-10-15
(UTC), asked whether 479 reopens as this rule: *"Reopen, build it."* The number 924 is provisional
until the merge queue lands it.

calef raised it on 2026-10-15 (UTC), after the census of 2026-10-14 found 43 untriaged survivors on
lines that pull requests merged in the two days since the previous census: *"Nobody will read an
advisory check. Maybe survivors can't merge untriaged."*

## The rule

A pull request fails a required check when `cargo mutants --in-diff` finds a surviving mutant on a
line the pull request changed and `notes/project-metrics/mutation-triage.csv` has no `equivalent` or
`gap` row for it. The author clears the check in the same pull request, with a test that kills the
mutant or a row that says why it cannot be killed.

A `killed` row never clears a survivor. The gate has just run the mutant and seen it live, so a
`killed` row for it is false, and the gate says so. That makes the commonest wrong verdict
unrepresentable: `notes/mutation-testing.md` records that a verdict reached by reading is wrong
about one time in ten, and a verdict reached by running is not.

## What answers 479's objections

479 rests on two objections from milestone 438 (would a diff-scoped mutation check have caught the
55, and what would it cost). Its evidence condition is met.

- Proof modules produced survivors no test can reach. In 438's sample, two of six survivors sat
  in a `#[cfg(kani)]` module. `.cargo/mutants.toml` now excludes any module named `verification` or
  `proofs`, by path pattern and by name, so the gate generates no mutants there. All 51
  `#[cfg(kani)]` modules under `crates/` and `uefi_loader/` carry one of those names. A proof module
  under any other name is not excluded, and nothing fails the tree for one; the gate's first such
  survivor is the signal to rename it. Two more of 438's six wanted a unit test restating a Kani
  proof; those are `equivalent` rows naming the proof.
- The corpus does not contain the kernel. `kernel/**` and `components/**` generate no mutants,
  so a kernel pull request passes by construction. This limits the gate's scope but does not argue
  against it. The host crates hold every counted survivor and are what fatal risk 3 (the tests do
  not test) measures, and gating them leaves the kernel no worse off. The check's summary has to say
  "no mutants: outside the corpus" on such a pull request, so a green check is never read as
  mutation-tested.

## Design

- Where: On the pull request, diffed against its base, as a required check. Not in the merge
  queue, whose diff changes as other pull requests land.
- Cost: Milestone 517 (what fraction of survivor growth arrives on lines a pull request touched)
  measured the replay at four seconds on a documentation change and under two minutes on the largest
  crate landing in six weeks. It would have named 62 of 761 pull requests (8.1%), a median of 6
  survivors each.
- A cap: A diff whose mutants do not finish in 20 minutes fails with "split this pull request
  or triage by hand", never a pass. A timeout is a failure here, not a kill.
- Timeouts: A single mutant that hangs is untriaged, the same as a survivor, because the gate
  cannot tell a detected hang from a slow test.
- The key: The same `crate,function,mutation` key that `helpers/mutation_inflow.py` matches, so
  the gate and the weekly census agree about what a row covers.
- One piece, many repositories: The check is a reusable workflow plus the helper, not steps
  written into `ci.yml`. A repository split out of this one inherits it with one `uses:` line and its
  own ledger. See below.

Reuse: `cargo mutants --in-diff`, which finds the mutants on a diff's lines, and
`helpers/mutation_inflow.py`'s key and ledger match, both taken. What is written is the glue
between them and the required check.

## What it does not catch

- The kernel and `components`, as above.
- A survivor on a line the pull request did not touch, such as code whose only test the pull
  request deleted. The weekly census stays the instrument for that, and fatal risk 3's green
  condition keeps reading the census, not this gate.
- A wrong `equivalent` row. The gate makes the row exist; review makes it right.

## The repository split, and why it does not replace this

Splitting crates into their own repositories does not change what is caught. Each mutant runs only
its own package's tests: every one of the 2,333 tested mutants in shard 0 of the 2026-10-14 census
ran `cargo test` with exactly one `--package`. A crate's survivors are the same in any repository.

What the split changes is cost. The census grows with the whole tree, about 12 runner-hours across
eight shards today. Split, each repository runs a census of its own code, on its own runners, only
when that code changes. That is horizontal scaling of the census. The per-pull-request gate already
scales with the change rather than the tree, so it needs no split to scale.

The split does not come soon for these crates. `notes/the-first-split.md` puts the contract crates
last, because they churn most. The 43 survivors include `filesystem_protocol`, `abi` and
`socket_protocol`. The first cut, milestone 756 (`process-tools`, `core-tools` and `disk-tools` are
the first code to leave this repository), moves programs in `components`, which generate no mutants
at all. So the gate is needed here first, and building it as a reusable piece is what lets each
repository the split creates start with it.

## Done means

- A reusable workflow and `helpers/` script run `cargo mutants --in-diff` on a pull request and fail
  on a survivor with no `equivalent` or `gap` row, or with only a `killed` row.
- A pull request touching only `kernel/**` or `components/**` passes with the summary "no mutants:
  outside the corpus".
- The cap and the timeout rule above hold, each with a selftest case.
- calef adds the check to the required checks; the lane cannot.
- Fatal risk 3's file records the gate as a fact. Its color stays calef's.

## Index row

The mutation gate on the diff, switched on. A surviving mutant on a line a pull request changed
blocks the merge until a test kills it or a ledger row says why it cannot be killed, so new code
stops arriving less tested than the census allows. It covers the host crates only; the kernel and
`components` generate no mutants.
