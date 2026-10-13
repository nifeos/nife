---
status: BUILT
raised: 2026-10-03
built: 2026-10-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 636. The scheduled workflows are failing, and nothing says so

Raised by calef (2026-10-03 UTC). The title and slug are drafts.

## Why

Six scheduled workflows last ran red before the org rename (2026-10-03 07:42 UTC): mutation,
undefined-behavior-check, audit-cadence, metrics, toolchain-drift and toolchain-bump. Mutation's
report is fatal risk 3's measurement, and milestone 238 (two scheduled checks have never once succeeded) fixed
it on 2026-09-03. This milestone diagnoses and fixes each one. It builds no gate and no alerting:
the detection mechanism is calef's, after the COE on the merge rate.

## Built

Each row names the last green run, the first red one, the cause read from the failing log, the fix,
and what proves it. Run ids are GitHub Actions run ids on this repository.

| Workflow | Last green | First red | Cause | Fix | Proof |
|---|---|---|---|---|---|
| mutation | 2026-09-21 (35589550926) | 2026-09-28 (36416663945) | Milestone 609 (the system tests leave the kernel crate) made `system_tests` a bare-metal crate on 2026-09-27, and `.cargo/mutants.toml` did not exclude it. Its one mutant landed in shard 4, whose baseline `cargo test -p system_tests` failed with "unwinding panics are not supported without std" | `system_tests/**` in `exclude_globs`; `cargo mutants --list` drops from 14,854 to 14,853 mutants with none in it | Dispatch 37108924347, green on all eight shards and the report job, 2026-10-03 09:41 UTC |
| undefined-behavior-check | 2026-09-17, a dispatch (35256118545); no scheduled run in the history is green | 2026-09-21 (35593012002), then 2026-09-28 (36420045162) | Three causes, one behind another, because a Miri test binary that aborts ends the run. First, 2026-09-28: `current_cpu_protocol`'s tests passed a 1-aligned `[u8; 16]` to `publish`, whose contract asks for 8-aligned, and Miri reported the unaligned `&AtomicU64`; fixing that exposed immutable buffers under `cpu()`'s `&AtomicU64`. Second, `stick_maker`'s tests stage trees under the temp dir and isolation refuses `statx` (the 2026-09-21 failure, and dispatch 37109746541 at 2 h 32 m). Third, `walk_pricing` the same way with `lstat`. The second and third are the harness refusing I/O, not undefined behavior | `current_cpu_protocol`'s tests and doc examples use an aligned, writable buffer. `stick_maker` and `walk_pricing` leave the run as `board_console` did: no `unsafe` Miri can execute, and the reason is beside each in `xtask/src/suite.rs` | By parts: dispatch 37109746541 passed every crate before `stick_maker` with the first fix in; a local `cargo miri test --no-fail-fast` over the 18 crates after it passed all but `walk_pricing`. Dispatch 37135941293 at this branch's head is the whole run, PENDING when this was written |
| audit-cadence | never: every run since 2026-08-17 is red | 2026-08-17 (32011101111) | Not a defect. Red is its designed answer to "is an audit due", and documentation and security are both due (`script/audits --due` says so today) | None in this lane: the fix is running the two audits | `script/audits --due`, locally, 2026-10-03 |
| metrics | 2026-09-30 (36713434148) | 2026-10-01 (36864216380) | The merge of #1455 (2026-10-01 04:24 UTC) cut `--check-toc` from `script/metrics` and took `--selftest` and `--flags` with it. The workflow calls `--flags` for its pull request body, so each run pushed `metrics/weekly` and then failed | Both modes restored | `script/metrics --flags` exits 0 and `--selftest` passes 18 cases, locally. Not dispatched: the workflow pushes a branch and opens a pull request. Unproven in CI until its next scheduled run |
| toolchain-drift | 2026-09-29 (36576523202) | 2026-09-30 (36720132989) | Not a defect in the tree. Upstream nightly changed `std`'s private `sys` interfaces twice (2026-09-30: `Dir::self_metadata`; 2026-10-02: `ExtraHomeDirs`, `SplitPathsRef`), which is what this workflow exists to say | Already on `main`: #1474 ported the overlay to nightly-2026-10-02, merged 2026-10-03 02:23 UTC, after the last red run | Dispatch 37109792425, green |
| toolchain-bump | 2026-10-01 (36880714315) | 2026-09-30 (36729534299), again 2026-10-02 (37019399265) | The same upstream drift. Its `propose` job went green and opened the bump; the restamp job stays red by design until a person re-records the floors, which #1474 did | #1474, as above | Not dispatched: it pushes a branch and opens a pull request with the App's token. Unproven in CI until its next scheduled run |

**No shared cause.** Three are by-design reds that reported something true (an audit due, upstream
drift twice), and two of those were already repaired on `main`. The other three are three different
defects. What the three defects do share is that nothing in a pull request's gates exercises the
path that broke: `.cargo/mutants.toml`'s coverage of a new crate, Miri, and `script/metrics
--flags`. That is an input to the detection question, not something this lane answers.

**What mutation measured.** The dispatch is the first complete census since 2026-09-21: 85 crates,
14,853 mutants, 13,734 viable, 1,004 missed and 255 timeouts, so 92.7% killed by the measure
`notes/mutation-testing.md` tabulates, against 92.4% on 2026-09-21 over 66 crates. It ran at this
branch's head rather than `main`, which differs only by this milestone's commits. Risk 3's color
is not this lane's to change; the number is offered to it as a fact.

**The App token after the org rename.** It works. `merge-drain` run 37109755917 (2026-10-03 08:27
UTC, after the 07:42 rename) minted an installation token "for this repository (nifeos/nife)", and
its pass armed three pull requests with it. The repository's scripts still name `crickertech/nife`
and reach it through GitHub's redirect; #1510 owns that rename.

## BUGS

- The undefined-behavior check's whole run at this head (37135941293) was still going when this
  block was written. Its proof above is by parts. If the whole run goes red, this block is wrong
  and gets corrected.

- `current_cpu_protocol`'s reader forms an `&AtomicU64` over a page its process maps read-only.
  Recorded in that crate's BUGS, not changed here.
- `metrics/weekly` holds one commit past `main` (the 2026-10-02 run's push, after #1462 merged the
  first). The next scheduled run force-pushes it, so it needs nothing by hand.

## Follow-on

- **Recorded.** `script/lint`'s "host pass excludes exactly the bare-metal crates" derives its list
  from crates that reach `user_mode_runtime`, so a crate that reaches `kernel` instead, as
  `system_tests` does, passes it. Extending the derivation is a guard, and guards wait on calef's
  detection ruling; the reason is in the comment beside the system-test image's entry in `.cargo/mutants.toml`.
- **Recorded.** Nothing outside the weekly workflow calls `script/metrics --flags` or
  `--selftest`, which is how losing both stayed silent for three days. Same reason to wait; the
  restore's comment in `script/metrics` names the merge.
- **Recorded.** The documentation and security audits are due, and audit-cadence will stay red
  until they run. Launching them is the maintainer's, and `design/audit-reports/README.md` says how.
- **Recorded.** A workflow that is red by design (audit-cadence, the bump's restamp job) reads the
  same as one that is broken, which is part of why six could be red at once. An input to calef's
  detection ruling, in this block and in the lane report.

## Index row

The six failing scheduled workflows are diagnosed, each cause named with evidence and fixed.
