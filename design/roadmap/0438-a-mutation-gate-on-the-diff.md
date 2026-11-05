---
status: BUILT
raised: 2026-09-19
built: 2026-09-19
---
# 438. Would a diff-scoped mutation check have caught the 55, and what would it cost

The answer is no, and the reason is not the one this block
expected: the decisive measurement came back negative because the premise was false. Milestone
319's pull request did not introduce 55 survivors. It introduced four. *(Number provisional until
the merge queue lands it.)*

## The result, first, because measurement 1 was written to end this

`cargo mutants --in-diff` replayed against [#927](https://github.com/nifeos/nife/pull/927)'s own
diff, at the merge commit `aa6a50b`, reports four survivors. The census of 2026-09-19 found 77 in
`crates/machine_discovery`. Seventy-three of them were already there the day before that pull request
merged, measured rather than inferred:

```console
$ git archive aa6a50b^1 | tar -x -C /tmp/parent          # main, immediately before #927
$ cd /tmp/parent && script/mutation -p machine_discovery
686 mutants tested in 7m: 73 missed, 533 caught, 71 unviable, 9 timeouts
```

73 before, 4 added, 77 after, which is the census's own number to the unit. The second measurement
says the same thing from the other side: the pull request added seven mutants to the crate in
total, out of 693.

```console
$ cd /tmp/parent    && cargo mutants -p machine_discovery --list | wc -l   # 686
$ cd /tmp/aa6a50b   && cargo mutants -p machine_discovery --list | wc -l   # 693
```

So the sentence this milestone was minted on, *"they did not accumulate, they arrived in one pull
request"*, is wrong. The survivors accumulated while the crate grew from the baseline's 212 mutants
(2026-08-03) to 693, and the instrument only looked twice in that window. This is a cadence
finding wearing a rate finding's clothes, and it is recorded here rather than quietly fixed because
`design/fatal-risks/README.md`'s risk 3 rests on the attribution. The correction to that record is
proposed in `design/roadmap/0512-the-census-blamed-a-pull-request-that-added-four.md`; this
lane does not edit `design/fatal-risks/README.md`.

Where the 22 probably came from, offered as the likely reading rather than as a measurement: the
`(baseline missed)` column `script/mutation --report` prints is `.cargo/mutants-baseline.txt`, where
`machine_discovery` reads `147 22 3 40`, and that file is the 2026-08-03 baseline. A rise from 22
to 77 against that column is six weeks of growth, not two days.

## What the four survivors actually are, which is the friction number

| survivor | verdict |
|---|---|
| `acpi.rs:491:25` `<` to `==` in `McfgEntry::size` | real host-test gap, **in code this same pull request proves** |
| `acpi.rs:491:25` `<` to `<=` in `McfgEntry::size` | same |
| `x86_64.rs:314:29` `+` to `-` | **unkillable**: inside `#[cfg(kani)] mod verification` |
| `x86_64.rs:314:29` `+` to `*` | same |

The first pair is the case that decides the recommendation. `McfgEntry::size`'s underflow guard
is proved for every input by `verification::an_ecam_windows_size_is_total_and_counts_one_mebibyte_per_bus`,
a harness #927 itself wrote, and the guard exists because that harness found the defect. A
blocking gate would have failed the pull request that proved the code, and the author's two remedies
would have been a unit test restating a proof or an exclusion with a reason.

The second pair is a defect in the instrument, the same family as milestone 326's `proofs.rs`
glob and wearing different clothes again. The line is `const N: usize = V1_LEN + 1;`, inside a
`#[cfg(kani)]` module that `cargo test` never compiles, so both mutants survive by construction.
`.cargo/mutants.toml`'s `exclude_re = ["verification::"]` matches the mutant name, and a `const`
gets no module path in its name, so the regex cannot see it. Three such mutants exist tree-wide
today; the third is `PROVED_NS` in `crates/network_time_protocol/src/lib.rs`. This has no home yet and is proposed
in `design/roadmap/0492-a-const-in-a-proof-module-escapes-its-exclusion.md`.

## Measurement 2: what it costs, which is not the objection

Every run below is `script/mutation --in-diff <the pull request's own diff>` in a `git archive` of
that pull request's merge commit, on a 4-core Linux box, the tool pinned at 27.1.0.

| sample | pull request | shape | mutants | wall clock |
|---|---|---|---|---|
| A | #950 `f81efb1` | documentation only, one file in `design/` | 0 | **3.7s** |
| B | #943 `82ebda6` | one crate, four lines, all inside a doc comment | 0 | **3.9s** |
| C | #934 `cbf5edc` | `crates/pci` plus five kernel files | 70 | **61s** |
| D | #955 `b74c556` | a rename across 70 files, six crates, kernel, xtask | 0 | **3.8s** |
| (1) | #927 `aa6a50b` | one crate, proofs plus the parser fixes they forced | 22 | **14.3s** |

Nobody had this number and it is cheap. The floor is about four seconds, because
`cargo mutants --in-diff` scopes its own baseline to the packages the diff touches
(`cargo test --no-run --package=machine_discovery` in #927's case, not the workspace), so a
documentation change never builds anything. The one parser change in the sample costs a minute, and most
of that minute is three timeouts at 20s each, spread over its two jobs. A timeout is a detected
hang rather than a finding, so the cost of a diff-scoped check is set by the hangs it finds more
than by the mutants it tests.

**Two of the five samples are the reason cost is not the argument.** D is a 70-file rename that
produced zero mutants, because `--in-diff` includes a mutant only when the mutant's own line
changed and every Rust line that rename touched was a doc comment or a path. C's five kernel files
produced zero for a different reason: `kernel/**` is excluded from the mutation corpus outright, as
are `components/**`, `crates/user_mode_runtime`, `crates/system_initializer`, `crates/virtio`,
`uefi_loader/src/main.rs` and `xtask/**`.

## Measurement 3: the ratio, in cases

Across the five replays the gate reports **six survivors and three timeouts**, and two of five pull
requests would have failed.

- Two are clean signal. #934's `BusQueue::enqueue` pair (`&` to `|`, `&` to `^`) are real missing
  tests. Under `|` the guard always returns and the walk never leaves bus 0; under `^` a revisited
  bus is enqueued twice. No host test walks a bridged topology or revisits a bus, and the gate says
  so correctly.
- Two want an exclusion or a test that restates a proof: #927's `McfgEntry::size` pair.
- Two are the instrument: #927's `const N` pair, which no test can ever kill.

So one third of what it reports is the thing the gate exists to report. That is the number the
block asked for, and it is an order of magnitude rather than a statistic: five pull requests.

## The recommendation

**Do not make it a blocking gate on this evidence, and this milestone's own rule already ends it
at measurement 1.**

Cost is not the objection; four seconds on a documentation change is cheaper than several checks
already required. Three things are:

1. **The case it was built on is not a case it would have caught.** The mechanism is not refuted,
   the story is. A diff-scoped check running on every pull request through August would plausibly
   have reported these 73 as they landed, and that is a different experiment, on pull requests
   this clone does not contain (see BUGS).
2. Two of six survivors, in the sample, are an artifact of the exclusion mechanism, and the
   pull request they would have failed is the one that added machine-checked proofs to the crate
   this milestone is about. A gate whose first act is to block a proof lane teaches the wrong thing
   about proofs.
3. It is silent where the tree's risk is. `kernel/**` and `components/**` are not in the
   corpus at all, so a kernel change passes by construction, and a rename across 70 files produces
   nothing. `design/fatal-risks/README.md`'s risk 3 is a claim about the whole tree's derivative, and this
   instrument can only see the part of the tree a host test can execute.

What would be a different question, and it is an architect's to ask rather than a lane's to
answer. The measurement that is missing is not "does `--in-diff` work", which it plainly does, but
"what fraction of the corpus's survivor growth arrives on lines a pull request touched". That is
answerable, and answering it needs the pull requests between 2026-08-03 and 2026-09-14 that grew
`machine_discovery` from 212 mutants to 686.

## Why this is not the gate milestone 85 refused

Milestone 85 refused a full-corpus pull-request gate and its reason still holds: *"a survivor is a
worklist entry, not a defect in whatever commit happened to precede the weekly cron."* A full run
blames whoever's pull request happens to be underneath it, and costs 52 minutes across eight runners.

`--in-diff` has neither property. It mutates only lines the diff touched, so every survivor it
reports is in code that pull request wrote, and the attribution is correct by construction rather
than by luck. The cost scales with the diff instead of the corpus. Measurement 2 confirms both, and
neither was the thing that ended this.

And 85's stated trigger is the wrong test for this gate, which is worth saying because it would
otherwise read as the blocker. 85 says wait *"until the weekly numbers prove stable enough that a new
survivor deserves to fail something."* That is a condition about the corpus rate, and the two
censuses that exist disagree by a point, so by that test the wait never ends. A diff-scoped check does
not depend on corpus stability at all. This is the same defect risk 3's own green condition had: a
condition written for one quantity being applied to another.

## The original premise, kept because it was wrong

Left standing rather than rewritten, because the correction above is only legible beside it.

The census of 2026-09-19 found the tree's like-for-like mutation score had fallen a point in five
days, 93.6% to 92.6%, in a window that included 77 new tests. One crate accounted for it:
`crates/machine_discovery`, 22 survivors to 77.

This block then said they did not accumulate, that they arrived in one pull request,
[#927](https://github.com/nifeos/nife/pull/927) (merge `aa6a50b`, milestone 319, 2026-09-17), in
the firmware-parsing code that milestone added, and sat unnoticed for two days.
Four of them did. The other 73 were there before it.

## The constraint, stated so it is not rediscovered

**It ships as a blocking gate or not at all.** calef, 2026-09-19: *"Advisory gates don't seem to work
for us."* [§97](../decisions/0097-advisory-checks.md) decided this on 2026-08-25 and its own `BUGS`
predicted the failure in as many words, *"a check added to CI is advisory by default, so the list
grows silently"*; `image-permissions` is the seventh check sitting in that state and milestone 340 is
the block about it. So an advisory period is not the safe default here, it is the known failure mode,
and this milestone does not propose one.

**This milestone does not switch anything on.** It produced three numbers and a recommendation. The
ruling is an architect's, because a gate on every pull request changes what contributing costs,
which is the irreversible half.

## Follow-on

- **Milestone 512.** milestone 512 (the census blamed one pull request for), `design/roadmap/0512-the-census-blamed-a-pull-request-that-added-four.md`, the
  correction to `design/fatal-risks/README.md`'s risk 3, which this lane measured and does not own.
- **Milestone 492.** milestone 492 (a const inside a proof module is), `design/roadmap/0492-a-const-in-a-proof-module-escapes-its-exclusion.md`, the
  three mutants that sit inside `#[cfg(kani)]` modules and are not excluded, two of which are half of
  this milestone's own result.
- **Refused.** A blocking `--in-diff` gate, per this block's recommendation and the ruling that ends
  it at measurement 1. Nothing is switched on and no workflow, `script/ci-build` row or ruleset entry
  was touched.
  This refusal is milestone 479 (design/roadmap/0479-a-blocking-mutation-gate-on-the-diff.md), which
  carries it with the condition that would change it.

## BUGS

- The re-ask cannot be run from this clone. The lane's checkout is shallow to 2026-09-14, so the
  pull requests that grew `machine_discovery` from the baseline's 212 mutants to 686 are not in its
  history and `--in-diff` cannot be replayed against them. The experiment in the recommendation's
  last paragraph needs a deeper fetch first.
- **A replay is not a live run.** `--in-diff` against a merged diff sees the code as it landed. A
  real gate would see it as proposed. And a pull request that was amended during review differs
  from its merge. PR #927 was amended during review (the falsification patches were refreshed twice),
  so its 22 mutants are the merged shape rather than the shape a gate would first have seen.
- The sample is five pull requests, because each run costs real time and the point was deciding.
  A ratio computed from six survivors is an order of magnitude, not a statistic.
- Measurement 1 ran against the exclusions of the day, `.cargo/mutants.toml` as it stood at
  `aa6a50b`, which is the honest replay and is **not** today's file: milestone 326 added
  `**/src/proofs.rs`, `**/src/verification.rs` and `interleavings::` on 2026-09-19, after the merge.
  None of the four survivors is affected (all four are outside those globs), so the number stands
  either way, and the `const N` pair is the case none of those entries reaches.
- **Timeouts were not classified.** #934 produced three, all of them the shape
  `script/mutation`'s own note calls a detected hang, and a gate would have to decide whether
  cargo-mutants' exit 3 fails a pull request. This milestone did not decide it.
- It cannot catch an untested line the diff does not touch. A pull request that adds a caller for
  existing untested code leaves that code exactly as untested and passes. The weekly census stays the
  instrument for the standing corpus; this is only about the derivative.

## Index row

Measured and refused. Replaying `cargo mutants --in-diff` against milestone 319's pull request
reports four survivors where the census of 2026-09-19 attributed 55 to it. A full sweep of the
tree immediately before that merge found 73 already present. So the premise that they arrived in one
pull request is false and the decisive measurement is negative. The cost is not the objection, since
the check scopes its own baseline to the packages a diff touches and runs in under four seconds on a
documentation change and a minute on a parser change. The friction is: across five replayed pull
requests it reports six survivors, of which two are real missing tests and two want a unit test
restating a Kani proof. Two are mutants inside a `#[cfg(kani)]` module that no test can compile.
It would have failed the one pull request in the sample that added machine-checked proofs. It is
also silent on `kernel/**` and `components/**`, which are outside the mutation corpus, so it cannot
be the instrument for a claim about the whole tree's derivative.
