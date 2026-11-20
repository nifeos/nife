# A mechanism says how many things it examined

*Milestone 641 (a mechanism that reports clean says over how many units, and zero is loud),
provisional number. The name of this note, `helpers/workflow_swallows.py` and the `# outcome:` label
vocabulary are provisional; naming is an architect's.*

From outside, a check that examined nothing looks exactly like a check that found nothing. The
weekly falsification sweep replayed zero patches on three Mondays and reported success each time
([the correction](corrections/2026-09-23-the-sweep-that-swept-nothing.md)). Milestone 401 (a gate
that selects the set it judges can pass by checking nothing) fixed the `script/` half, recorded in
[empty-selectors.md](empty-selectors.md). This note is the workflow half and the metrics half.

## The rule

A step's exit status is one channel carrying two claims. The verdict says the thing ran and found
something a human should see. The outcome says the thing did not run at all. Suppressing a verdict
can be right: §134 (a harness carries a machine-replayable falsification record, or it is not
evidence) rules that a survivor is a worklist entry, not a red trunk. Suppressing an outcome is
never right unless somebody decided so and wrote down why.

So the work splits by rung:

- Rung one, in the script. A script that exits non-zero when it examined nothing needs no workflow
  cooperation. `script/falsifications --sweep` and `script/audits` now do.
- Rung two, in `script/lint`. A workflow can hide a script's loud zero only by discarding its exit
  status. Two constructs do that, and `helpers/workflow_swallows.py` gates both.

The gate is on the swallow and not on a printed count, deliberately. A workflow that runs a script
and keeps its exit inherits the script's denominator. Requiring every workflow to print a count
would be a second copy of what the script already knows, and a count nobody reads is rung four.

## What the gate checks

Every `continue-on-error` that is not `false` carries a label on its own line:

| label | what the gate requires |
|---|---|
| `# outcome: re-raised` | the step has an `id`, and a later step in the same job has an `if:` testing `steps.<id>.outcome == 'failure'` |
| `# outcome: reported` | the step has an `id`, and a later step in the same job reads `${{ steps.<id>.outcome }}` as a value |
| `# outcome: exception, <reason>` | a reason of three words or more on the line |

A test on `steps.<id>.conclusion` is rejected outright. Under `continue-on-error` a failed step's
conclusion is `success`, so such a test can never fire. A job-level `continue-on-error` can only be
an exception, since nothing later in the job can read it.

A pipeline into `tee` needs pipefail, from `shell: bash` or `set -o pipefail`. GitHub's default
`run` shell is `bash -e {0}`, which gives the pipeline `tee`'s status. A pipeline headed by `echo`
or `printf` is exempt.

Replayed against `24a1e0a7b`, the commit that introduced the sweep, the gate fires twice: once for
the unlabeled `continue-on-error` and once for `| tee sweep.txt` without pipefail.

## The workflow sites, labeled

Counted on the branch base, `4c9cae0a9`, 2026-10-04 (UTC): 20 workflows. The block counted 14 files
on 2026-09-23.

### `continue-on-error` (13)

| workflow | step | label | why |
|---|---|---|---|
| `metrics.yml` | install cargo-llvm-cov | re-raised | coverage is a carried cell, so a failure left last week's number standing |
| `metrics.yml` | measure coverage | re-raised | same |
| `metrics.yml` | install the interface target | re-raised | the interface cells are carried too |
| `metrics.yml` | measure the interface | re-raised | same |
| `metrics.yml` | fetch the merge queue record | re-raised | a missed day heals, but a broken fetch never did |
| `mutation.yml` | check new survivors | re-raised | already was: the survivor list uploads, then the job fails |
| `toolchain-bump.yml` | raise the pin and rebuild | reported | the failure is the subject of the draft pull request it opens |
| `toolchain-bump.yml` | install both nightlies | exception | a missing nightly is upstream's; the body says no restamp ran |
| `toolchain-bump.yml` | install the target nightly | exception | a missing nightly means nothing to propose today |
| `toolchain-bump.yml` | fetch the restamp report | exception | the body says the report is missing |
| `trunk-health.yml` | ejection share | exception | a GraphQL failure must not turn a five-minute watcher red |
| `trunk-health.yml` | report a stopped watcher | exception | warns, and the other watcher reports this one |
| `merge-drain.yml` | report a stopped watcher | exception | same |

The five `metrics.yml` sites were outcome swallows with no exit at all before this milestone. They
now share a last step, after the pull request is written, that fails when any of them did. Going
red there costs no column.

### Pipelines into `tee` (5)

All five have pipefail: `audit-cadence.yml`, `stranger-cadence.yml`, `falsifications.yml` twice,
and the overlay build in `toolchain-bump.yml`.

### `|| true` (52 on code lines), by family

Not gated, because no parse can tell the safe direction from the other one. Labeled here instead.

| family | sites | what an empty result does |
|---|---|---|
| scope fallback (`scope-merge-base.sh`, `git merge-base`) | 23 | falls to the next, wider base |
| documentation-only filter in `ci.yml` | 9 | was "docs only" on an empty change set; fixed, see below |
| live lookups that decide to skip (`ci.yml`, `verify.yml`) | 4 | runs everything |
| pull request number from a merge group ref | 2 | exits 1 on the next line |
| finders, where empty means none (`coe-architect-label.yml`, `metrics.yml`, `toolchain-bump.yml`) | 3 | correct by construction |
| optional digest in the metrics pull request | 1 | the body omits it |
| idempotent calls (`gh pr ready`, label delete, `git add`, unmount, `kill` in a trap) | 8 | nothing depends on the outcome |
| `script/bench --restamp` and the stamp check after it, in `toolchain-bump.yml` | 2 | a failed restamp leaves an old stamp, which goes red; an empty floor set is `script/lint`'s toolchain-stamp guard |

Two sites in this population were outcome swallows, and both are fixed:

- `architect-label.yml` read the rules helper through `|| true`. The helper's exit for "nothing
  fired" was 1, which is also Python's exit for an uncaught exception. A crash, or a missing merge
  base, set `add=false` on the job that routes system call, dependency and decision changes to an
  architect. The helper now crashes with 3, and an unexamined diff is labeled with a report
  saying so.
- `ci.yml`'s nine documentation-only predicates read zero changed files as documentation only and
  skipped the build. Each now requires a non-empty set first. No run is known to have hit it.

### `if: always()` (about thirty)

Every one publishes or records after a failure: job-budget checks in `ci.yml`, artifact uploads in
`mutation.yml`, report steps in `falsifications.yml`, watcher reports. None changes a job's
conclusion, so none is a suppressor.

## The reporting jobs, and the number each states

| job | what it says it examined | zero |
|---|---|---|
| `falsifications.yml` sweep | `N swept, ...` | exits 4, from this milestone |
| `falsifications.yml` system call driver | `N seeds` | `xtask` fails a filter that selects no test |
| `audit-cadence.yml` | `N on record, K kinds` | an empty cadence table fails, from this milestone |
| `stranger-cadence.yml` | `Last run: N` | no run heading fails |
| `mutation.yml` | `n of 8 shards` | a partial census is not diffed and says so |
| `metrics.yml` | each column's cell | an instrument failure fails the job, from this milestone |

## The published ratios, and what each is of

A ratio hides a zero in its denominator rather than in an exit status, and it is worse because the
number is quoted. The block asked for the survey, not a better denominator, and that is all this is.

| ratio | where | the denominator counts | outside it |
|---|---|---|---|
| falsification ratio | `script/falsifications`, the harnesses chart | Kani harnesses in workspace packages | code no harness covers; kernel-test, swish-check and host-test records, printed apart |
| unsafe density | the unsafe chart, `script/lint`'s ceiling | non-blank code lines outside `kernel/src/arch/` | `arch/` itself, by design; `xtask/`, `helpers/`, `tools/`, `fuzz/`, `bench/host/`, `patches/`, `vendor/` |
| unsafe by trust boundary | the trust chart | code lines per class | `shared`, `boot_chain` and `unclassified` carry no density |
| coverage | the coverage chart | lines in host-buildable workspace crates | every crate `script/coverage` excludes: `kernel`, `system_tests`, `components`, `fixtures`, `xtask` and four system call wrappers |
| lowest-covered file | the floor chart | files the floor acts on | files `script/coverage` exempts |
| merge queue ejections | the merge queue chart | queue entries, each re-entry counted | pull requests merged without the queue; days before 2026-09-14 |
| cache-read share, context per turn | the cost charts | tokens in session records on one laptop | the z.ai work of 2026-09-29 to 10-01; any session not recorded there |
| tokens per milestone | the effort chart | milestones built that week | work that builds no milestone: decisions, corrections, reviews |
| benchmark drift | the drift chart | rows in `bench/baseline-*.txt` since each anchor | benchmarks with no baseline row |
| interface co-change | the interface line | commits touching a contract crate | crates whose rustdoc did not build, which the line names |

The falsification ratio is the one the block named. Milestone 524 (the three x86_64 boot gates: NX,
SYSTEM CALL, and the invariant TSC) added 338 lines to `crates/machine_discovery/src/x86_64.rs` with no
harness, and the ratio did not move. `script/falsifications` now prints its denominator on the line
after the ratio. The interface line was already doing this: it names its unread crates.

## BUGS

- A label can be wrong. The gate makes the decision written and reviewable, not correct. The
  original sweep would have passed as `# outcome: exception, a survivor is a finding`, and the only
  defense is a reviewer reading the word exception.
- `|| true` is not gated. A new one that swallows an outcome passes lint. The families above are
  rung three.
- A selector inside a `run:` block, such as `mutation.yml`'s `find shards -name missed.txt`, is not
  gated. That is milestone 401's unclosed convention, one directory over.
- The exceptions stay quiet. The two watcher reports warn; the nightly installs say nothing. A
  watcher that cannot report itself is reported by its sibling, which is the whole defense.
- `metrics.yml` now goes red on a transient merge queue fetch failure. That is one red daily run per
  transient, read by `script/cadence-check` only after fifteen days without a success.
- The `script/audits` guard is narrow. An audit whose kind lacks a cadence row already failed, so
  it only adds the case where both tables are empty.
- `re-raised` checks that a later step tests the outcome, not that its body exits non-zero.
- No ratio here has a better denominator. Whether the falsification ratio should be over lines,
  functions or public surface is open, and this survey is what makes it answerable.

## EXAMPLES

Run the gate, which `script/lint` does as check 12b:

```console
$ python3 helpers/workflow_swallows.py
workflow swallows: 20 workflows, 13 continue-on-error step(s) each labeled with where its failure goes, 5 pipeline(s) into tee with pipefail
```

Replay it against the sweep as it first landed:

```console
$ git show 24a1e0a7b:.github/workflows/falsifications.yml > /tmp/sweep.yml
$ python3 -c 'import sys; sys.path.insert(0, "helpers"); import workflow_swallows as w; print("\n".join(w.check_text("sweep.yml", open("/tmp/sweep.yml").read())[0]))'
sweep.yml:65: `continue-on-error: true` with no `# outcome: <kind>` label on the same line. ...
sweep.yml:66: a pipeline into `tee` without pipefail exits with `tee`'s status, which is success. ...
```

Ask the sweep for a package with no replayable record. Before this milestone it printed `nothing to
sweep in no_such_package` and exited 0:

```console
$ script/falsifications --sweep no_such_package; echo "exit=$?"
nothing to sweep in no_such_package
0 swept: --sweep selected no replayable record, so it examined nothing. That is a
broken selection and not a clean sweep (see the docstring of sweep()).
exit=4
```
