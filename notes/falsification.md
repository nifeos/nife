# Falsification records: what a harness proves it can catch

Milestone 194, building DECISIONS §134, rescoped by milestone 212. The convention, the tooling,
and what running it on six real harnesses actually found, which was more than the mechanism was
expected to show.

`notes/verification.md` has said the rule since milestone 35:

> Falsify a property before believing it. Break the code the harness guards and confirm the harness
> fails. Every milestone 35 property was falsified this way, and one falsification corrected a claim in
> the code (the load-bearing guard was not the one the comment pointed at). A harness that cannot be
> made to fail is not evidence.
>
> -- notes/verification.md

Rung four, honored by whoever remembered; milestone 191 (did the proofs catch the bugs?) measured
it: 141 harnesses, none recording what was done to falsify it. This note is the record's home.

## Inductive Validity Cores: checked first, and they are not available

§134 asked that IVCs, the minimal set of model elements a proof needed, be checked first. They
are not available: an IVC falls out of an induction step, and Kani is a bounded model checker with
none. What was examined on 2026-08-31 is in
[falsification/inductive-validity-cores.md](falsification/inductive-validity-cores.md). So §134's
option C is the affordable option that exists, and this is it.

## The convention

A `Falsification:` block in the comment run immediately above `#[kani::proof]`, past any other
attributes. At the thing, never in a registry, which is milestone 115's argument reused rather than
a new one: two lanes touching two crates cannot collide, and a computed report cannot drift from the
tree.

`crates/capability/src/lib.rs` `from_bits_cannot_forge_a_right` is the worked example: its doc
comment states the property and ends `Falsification: replayable` with the backticked patch path.

Four states, and the unknown one is first-class, for exactly the reason `script/names` insists
`unrecorded` stays one: inventing evidence to fill a row puts a false claim in the record whose only
job is saying what is known.

| state | means | what the sweep does |
|---|---|---|
| `replayable <path>` | a patch exists that turns this harness red (backtick the path in a `///` comment; `clippy::doc_markdown` rejects a bare one) | applies it, runs that one harness, requires red, reverts |
| `attested <date>` | a person broke the code and watched it fail; nothing can re-check it | counts it, and it is a worklist entry |
| `unfalsifiable <why>` | examined; no defect in the code turns it red | counts it apart; reason required |
| `unfalsified` | nobody has | counts it, and this is the claim's honest denominator |

A patch lives at `<package>/falsifications/<module.path>.<harness_fn_name>.patch`
(`crates/capability/falsifications/verification.subset_is_reflexive.patch`,
`kernel/falsifications/syscall.proofs.the_run_end_is_exact_and_refuses_exactly_what_does_not_fit.patch`).
Backtick the path in the record: `script/lint`'s kani-shim pass compiles these `#[cfg(kani)]`
modules with `-D warnings`, where `clippy::doc_markdown` rejects a bare dotted path in a `///`
comment, and that pass is what found it. The reporter
strips backticks rather than requiring them, so a `//` comment, which has no such rule, can carry
the same form. Each patch opens with prose above its first `diff --git` line, which `git apply`
ignores and a reader does not. The prose says which property the patch expects to break, because a
falsification that makes a harness fail for the wrong reason proves nothing. `script/falsifications
--check` requires that header to be non-empty; it cannot check that it is true, which is the same
limit `script/names` records for its own citations.

## Running it

```
script/falsifications                       the table, by package, and the ratio
script/falsifications --unfalsified         the worklist, biggest package first
script/falsifications --check               form only; script/lint runs this
script/falsifications --sweep [package...]  apply every recorded patch, require red, revert
script/falsifications --affected-since <base>   sweep only what a diff can reach
```

**The gate is presence, never `replayable`.** `script/lint` fails a build only when a harness does
not say which of the three states it is in, for the reason `script/names --check` gates on presence
rather than on `ratified`. A lint demanding the queue be drained would hold every unrelated merge
behind proof work nobody can hurry.

The sweep is a report, weekly (`.github/workflows/falsifications.yml`, 06:00 UTC Monday), with
`script/mutation`'s posture deliberately copied. A **survivor** is a harness that stayed green with
its own falsification applied, which is exactly the signal nothing else in this tree detects. A
**stale** patch is one that no longer applies, which §134 argues is the mechanism working: the
covered code moved, so the falsification must be redone rather than trusted.

The per-pull-request half is a gate, in `verify.yml`, and the split is `script/lint`'s own: a
patch that a commit staled is a defect in that commit, where a weekly survivor is a fact about the
tree that morning. It costs seconds because a falsification runs one harness.

### The cost, measured

`script/verify` is ~650 seconds and this milestone adds nothing to it: the sweep is a separate
script and a separate workflow.

| what | cost |
|---|---|
| `script/falsifications --check` (in `script/lint`) | 0.19 s, no build |
| `script/falsifications --sweep`, six records, warm | 4.4 s end to end, rebuilds included |
| one harness alone, `capability`, warm | 0.35 s |

The sweep's cost is dominated by the rebuild each patch forces, so it scales with the number of
distinct crates carrying records rather than with the record count. That is why the weekly workflow
is unsharded, and it is a measurement to re-take rather than a rule.

### A record that never reaches a verdict: the per-record limit

Each replay runs under a wall-clock limit, and one that runs out is `ERROR ... no verdict`, never
red and never a survivor. The sweep goes on to the next record and exits non-zero at the end. The
limit was added on 2026-09-24, after a `nifefs` record (#1156) unrolled an unbounded loop for 1 h 44
min and starved every record after it.

| record kind | limit | where the number comes from |
|---|---|---|
| Kani harness | 15 min | 1.8 times the slowest record measured |
| kernel test | 30 min | `ci.yml`'s bound on building the kernel and running the whole suite |

`FALSIFICATIONS_RECORD_LIMIT` (seconds) overrides both. The whole process group is killed, not the
child, so no `cbmc` or QEMU grandchild outlives its record. The timing sample, an example run and
the limit's limitations are in [falsification/record-limit.md](falsification/record-limit.md).

## Milestone 212: the ratio was a fraction of one directory

The reporter once walked `crates/` only, so the ratio it printed described one directory and said
nothing about saying so. Milestone 212 (`script/falsifications` walks `crates/` only) made the scope
come from `cargo metadata`, the module path from the Cargo target, and the sweep pass `--bin` and
`--ignore-global-asm` where a package needs them. A patch that opens with ``Falsifies
`<fully::qualified::name>` `` and names an existing function with no `#[kani::proof]` is a known gap
(a kernel `#[test_case]` no sweep runs), printed after the ratio and counted in neither half.
Anything else without a harness is rot. The detail is in
[falsification/milestone-212-scope.md](falsification/milestone-212-scope.md).

## What six real records found

The first six records were chosen to embarrass the mechanism, and three findings came out
([falsification/six-real-records.md](falsification/six-real-records.md)).

The capability crate's central theorem was blind to the predicate underneath it. With
`Rights::is_subset_of`'s operands swapped, ten of twelve harnesses stayed green.
`derive_never_widens_rights` states its property *through the predicate it is testing*. It asserts
`derived.rights.is_subset_of(src_rights)` while `derive` guards on
`rights.is_subset_of(src.rights)`, so a consistently wrong `is_subset_of` satisfies both sides and
the harness cannot see the break. Milestone 211 rewrote it (below).

`subset_is_reflexive` can be turned red, but its red cases are a strict subset of
`subset_matches_allows`'s, so it adds no discrimination.
`filesystem_protocol::a_grandchild_is_bounded_by_the_root` is the same shape one crate along.

The patch path was wrong at first, because `paging` states six properties once per ISA under shared
function names. calef amended §134 on 2026-08-31: the module path is always included, with no
branch, and the sweep filters with `--harness <qualified> --exact`. `script/falsifications --check`
keeps a collision check, which now guards its own module-path tracking.

## The sweep for self-referential harnesses

Milestone 211 (a harness that states its property through the function under test), 2026-09-01,
asked every harness whether it states its property through the function under test. 146 were swept
and 11 were blind, each proved by a patch the old phrasing stayed green on and the rewrite goes red
on. All eleven now carry a replayable record of that defect. Three more looked like the class and
were not, and say so. Every harness was read, because the narrowing script flagged three of the
eleven for the wrong reason. A harness is fine when its claim is in arithmetic the implementation
does not own, when it compares two independent implementations, when it carries its own model, when
it proves totality, or when it asserts a constant. The table of eleven, the negative half and the
patterns are in [falsification/self-referential-sweep.md](falsification/self-referential-sweep.md).

## The sweep for harnesses that duplicate the implementation

Milestone 213 (a harness that re-implements the code instead of calling it), 2026-09-02, asked a
blunter question of 148 harnesses, read one by one: is the function under test in the harness at
all? The discriminator is which side of the assertion the crate produced. The crate must produce the
subject; the expectation should come from somewhere the implementation does not choose. The defect
is a harness where the crate produced neither side.

One harness had it: `nifefs::the_validation_implies_reads_slice_is_in_bounds` re-implemented both
`parse` and `read` and proved a property of the copy. The repair, `Fs::entry_bounds`, has two
ordinary callers, so §46 (thin primitives or whole subsystems) and its refusal of a function that
exists only for the prover did not apply. The test for next time: would the function be worth
extracting with no harness? Two more have the shape and are not findings, and why is written at each
harness. The full record is in
[falsification/duplicated-implementation-sweep.md](falsification/duplicated-implementation-sweep.md).

## `machine_discovery` swept clean, and the gap was a harness that did not exist

2026-09-23: all fifteen `machine_discovery` harnesses already carried replayable records and swept
15 red. Two of them had found real boot-path bugs when written. What was missing was a harness for
342 new lines in `x86_64.rs`: code with no harness is invisible to the worklist, so a crate can
reach 100% falsified by proving nothing.
`a_feature_is_never_reported_from_a_leaf_the_part_does_not_answer` (name provisional) closes it. The
detail is in [falsification/machine-discovery-sweep.md](falsification/machine-discovery-sweep.md).

## BUGS

- The weekly sweep's transcript goes to `$RUNNER_TEMP`, not the checkout: `script/falsifications
  --sweep` refuses a dirty tree, and a `tee` into the checkout behind a pipe makes the step refuse
  and still exit 0. Read the log, not the green tick. The account is
  `notes/corrections/2026-09-23-the-sweep-that-swept-nothing.md`.

- The self-referential sweep cannot be a gate and cannot be repeated cheaply. No check
  distinguishes "asserts through the function under test" from "legitimately asserts agreement",
  which is why milestone 211 is a sweep with a worklist rather than a lint. Its per-finding
  evidence is a falsification patch, so the weekly sweep does re-run it. But a harness found
  *fine* today carries no artifact at all and will go quiet if the code under it is refactored
  into the shape it was cleared of.
- **Eleven findings is a floor, not a count.** Each one is a defect somebody thought of. A
  harness with no demonstrated blind spot is a harness nobody has broken the right way yet, and
  the three recorded above as "not findings" are exactly that claim made honestly rather than a
  clean bill of health.

- A wall clock on a shared machine can turn a slow record into a false `no verdict`. The
  headroom is 1.8 times one measured maximum, not a proven margin, and a timeout on a busy laptop
  that does not reproduce in CI is load.
- Nothing forces the ratio upward. A harness may sit at `unfalsified` for ever with `script/lint`
  green; the backlog reached zero on 2026-10-06 by work, not by gate.
- **A recorded falsification proves the harness catches *that* defect, not the class.** It is a
  floor, and a low one. `derive_never_widens_rights` above is the worked example of exactly this: it
  now carries a green record and a documented blind spot at the same time, both true.
- A patch rots against refactors. §134 argues that is correct rather than harmless, and the
  sweep reports a stale patch as a failure rather than a skip. It is still churn, and a heavily
  refactored crate re-falsifies often.
- `--sweep` refuses to run on a dirty working tree. It applies and reverts with `git apply`, and
  a failure mid-run would otherwise leave a deliberate defect in somebody's source. So you cannot
  sweep while you work.
- **`--affected-since` is crate-granular, not harness-granular.** A change anywhere in a harness
  crate's dependency closure re-falsifies every replayable harness in that crate, because nothing in
  this tree knows which lines a harness covers. That is what an IVC would have bought. It fails
  toward doing too much, which for a run measured in seconds is the right direction.
- **Module paths come from brace counting, not from a Rust parser.** A brace inside a string or a
  comment at module scope would miscount and produce a wrong patch path. Nothing in this tree does
  that, and the failure surfaces as a path `--check` reports rather than as a silently wrong sweep,
  but it is a real limit of a 30-line derivation standing in for `syn`.
- `kernel/src/arch/` carries no harnesses and gains nothing here. The architecture layer is
  outside this record entirely, the same scope gap §134 (a harness carries a machine-replayable
  falsification record, or it is not evidence) names. The VisionFive 2's "undelivered wake" was a
  completed tour's terminal state, not a stranded receiver (`notes/visionfive2.md`'s fifth bench
  stop), so no defect lives there.
- A file's module path comes from the Cargo target it belongs to, and one shape defeats that.
  `components/src` holds 49 `[[bin]]` roots and two single-consumer `#[path]` modules that rule 7
  permits; `fixtures/src` holds 23.
  A root contributes no module segment; a `#[path]` module contributes whatever the including file
  calls it, which need not be its filename. Neither carries a harness today, and one that arrived in
  a `#[path]` module would get a patch path naming the file, which `--check` reports as a mismatch
  rather than accepting.
- A kernel `#[test_case]` falsification is recorded and unswept, and the section above says how
  it is told apart from rot. This bullet said it stays that way "until milestone 210 (no kernel test
  can be run by name) lands"; 210 landed on 2026-08-31, and `cargo xtask test --test <substring>`
  now filters the kernel suite, so the sweep this was waiting on is affordable and nobody has run
  it. That is a lane rather than a limitation now.
- Vacuity is still unguarded. §134 recommended option A (a lint requiring `kani::cover!`)
  alongside option C, and this milestone built only C. 23 `cover!` sites across 4 of 24 harness
  crates is the current state, from milestone 191. That wants a lane.
- One finding is a floor too, and a lower one than eleven was. 213 asked a narrower question
  than 211 and got a narrower answer, and the honest reading is not "the tree has one such
  harness". It is that one harness had the shape in its pure form, where the crate is on neither
  side of the assertion. The mixed cases are the ones a sweep is bad at. A harness whose subject
  comes from the crate can still rest on a recomputed assumption, and telling a safe restatement
  from an unsafe one took reading the code both times rather than applying the rule.
- The sweep leaves no artifact on the 147 harnesses it cleared, the same limit 211 records.
  A harness that is fine today becomes a duplicate the moment somebody inlines the function it
  calls, and nothing says so. The two recorded above are the exception only because their reasons
  are written at the harness rather than here.
- A model of a caller has no repair and no expiry date. The kernel harness above is faithful
  because somebody read two call sites on one day. Nothing re-reads them, and the failure is
  silent in the worst direction: the harness stays green while proving a claim about a guard the
  kernel no longer applies.
- A hand dispatch of the weekly sweep cancels any other branch's. `falsifications.yml`'s
  `concurrency` group is the bare string `falsifications` with no ref in the key, and it carries
  `cancel-in-progress: true`. That is right for the cron, where a queued weekly run has nothing to
  say that the newer one will not say better, and wrong for `workflow_dispatch`. On 2026-09-23 three
  falsification lanes dispatched the workflow within twenty minutes of each other and the second
  killed the first. The run it killed was the only one in the workflow's life that would have
  replayed a patch. Check `gh run list --workflow falsifications.yml` for a run in progress before
  dispatching, and re-dispatch if yours is cancelled. See
  `notes/corrections/2026-09-23-the-sweep-that-swept-nothing.md`.
