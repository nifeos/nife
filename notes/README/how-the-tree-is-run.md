# Notes index: How the tree is run

Gates, records and the merge queue: the machinery that keeps many lanes honest.

Part of [the notes index](../README.md), which says how to add a line.

- [The `script/` entry points](../scripts.md): the normalized front-door commands and what each does.
- [Every check in this repository](../check-inventory.md): audit of what runs, blocks, and asserts. Name provisional.
- [A workflow records the run it was watched failing in](../observed-failing.md): no gate is trusted until it has been seen to fire. Name provisional.
- [Selectors that can select nothing](../empty-selectors.md): gates that pass when their pattern matches nothing. Name provisional.
- [A mechanism says how many things it examined](../denominators.md): workflow steps that swallow a failure, and what each published ratio is of. Name provisional.
- [What to do when `main` goes red](../main-is-red.md). Names provisional.
- [The merge queue, and the three things that watch it](../merge-queue.md): the scripts that land, watch, and flag queue work. Names provisional.
- [A ready pull request failing a required check says so](../ci-failing.md): the scheduled flag that labels and comments once per head. Names provisional.
- [A push to a queued branch is refused by the pre-push hook](../push-while-queued.md): what GitHub does to a queued pull request on a push, the check, its override and its gaps. Names provisional.
- [A merge-queue ejection, caught before the queue and recovered after it](../queue-ejection.md): the ready check, the text checks of lint in the pre-push hook, the `needs-maintainer` label on an ejected pull request, and the drain's hold on an ejected head. Names provisional.
- [Which pull requests pay down debt](../debt-paydown.md): what the `debt-paydown` label means, the calls that set precedent, and how a lane gets it. Name provisional.
- [A paused draft names its blocker, and the drain reads it back](../blocked-by-drafts.md): `Blocked-by:` on a draft pull request, and the `unblocked` label. Name provisional.
- [Working from a cloud session](../working-from-a-cloud-session.md): what past cloud sessions hit, how to set up, claim and gate in CI, and what needs a local Mac. Name provisional.
- [Build output on its own volume](../dev-machine-disk.md): why deleted `target/` directories fill a Mac's disk through local snapshots, and the runbook that moves lane worktrees to an unsnapshotted APFS volume. Names provisional.
- [The automation's own identity](../automation-identity.md): the `smelter` GitHub App that replaces a personal token. Name provisional.
- [Agent sessions act as the App](../session-identity.md): `helpers/smelter-token`, so a lane runs as nife-smelter rather than as calef. Name provisional.
- [Hardening the repository itself](../repo-hardening.md): the GitHub settings that cannot be committed.
- [The roadmap](../roadmap.md): how to add a milestone, and its vocabularies.
- [Follow-on work, and what happened to it](../follow-on-work.md): the Follow-on section every finished block must answer. Name provisional.
- [The untracked-work sweep, and what each finding became](../untracked-work-sweep.md).
- [The dependency census](../dependency-census.md): real prerequisite edges between milestones, measured against declared ones.
- [Citations that name what they cite](../citations.md).
- [What Rust comments cost, 2026-10-07](../comment-cost-2026-10-07.md): comment-only Rust changes and the CI they ran, why comments change, density against seL4, Linux and Rust's library, and a sampled audit. Name provisional.
- [The lint census, 2026-10-09](../lint-census-2026-10-09.md): panics, casts, `static mut`, suppressions and function length in the non-test Rust that runs on nife, counted for five proposals. Name provisional.
- [The Actions cache budget, 2026-10-07](../actions-cache-budget-2026-10-07.md): which caches earn their bytes against GitHub's 10 GB limit, why merge groups save nothing now, and the forks left. Name provisional.
- [What script/lint costs a lane, 2026-10-18](../lint-cost-2026-10-18.md): per-section time locally and in CI, which sections stop the run, reruns per lane on one evening, the pre-push hook, and fixes ranked by time saved. Name provisional.
- [CI's toolchain download, 2026-10-19](../ci-toolchain-download.md): how often the rust-lang CDN failed CI, the retry that answers it, and what caching the toolchain would cost and buy. Name provisional.
- [Counted claims](../counted-claims.md): numbers in prose that a gate re-derives. Name provisional.
- [The register of measures](../register-of-measures.md): the numbers this kernel holds itself to. Name provisional.
- [Project metrics: what moved, week by week](../project-metrics.md): weekly charts of the project's measures, from git history. Script and data names provisional.
- [How stable the interface is](../interface-stability.md): what the interface is, the weekly counts of what broke and what grew, the proposed threshold, and what the counts cannot see. Names provisional.
- [The violation ledger](../rule-violations.md): counting how often each written rule is broken. Name provisional.
- [Load-sensitive assertions](../load-sensitive-assertions.md): the register of assertions that fail under host load, how to fix one, and each site's status. Appendix names provisional.
- [The kernel test harness](../kernel-test-harness.md): the two hang watchdogs, skips, filters and repeats, the frame ledger, tick budgets, and the preemption window tests.
- [The `uefi-test` exit-1-after-green rate](../uefi-test-exit-rate.md): the measured rate, the exit-path protocol the failure message now names, and why the VT-d lines are expected. Name provisional.
- [The `uefi-boot` two-core rate at `-smp 2`](../uefi-boot-two-core-rate.md): the dedicated rate the UEFI gate's two-core assertion was owed, zero in 42 across a load gradient, and which stale records the fix left behind. Name provisional.
- [The CI log baseline](../ci-log-baseline.md): which check failed each CI job, from expiring logs. Names provisional.
- [CI job warnings, classified](../ci.md): the swish-check job's every warning, read and ruled benign, tracked or fixed. Name provisional.
- [Every place that enumerates architectures, and whether the list is complete](../architecture-list-sweep.md).
- [Rustdoc coverage](../doc-coverage.md): the doc-example floor and the `missing_docs` ratchet.
- [The documentation sweep](../documentation-audit.md): how to run a documentation sweep, and what counts.
- [Prior art and reuse](../prior-art.md): where to look before building, and the build-versus-reuse rule.
- [Handing a session over](../session-handoff.md): the 2026-07-29 restart point, a record of that session.
- [Corrections of error](../coes/README.md): the register of recorded errors and the mechanisms
  that replaced them, one dated file each.
- [BUGS entries with no milestone](../bugs-without-milestones/README.md): the survey of limitations that are real future work, by area, and the question of a gate.
- [Cobble, the mascot](../mascot.md): the project's mascot, drawn by calef's kid.
- [The system tests and the kernel crate](../system-tests-and-the-kernel-crate.md): what the kernel linked for its tests, the gate that keeps them out, and where the tests go. Name provisional.
- [The system-test modules](../system-test-modules.md): what each module of the whole-system suite proves, and why each test is shaped as it is. Name provisional.
- [The suite's frame budget](../frame-budget.md): what the system tests may leave unreturned, what they legitimately keep, the conventions, and every number the ceiling has been. Name provisional.
- [The swish-check CI flake, measured](../swish-check-flake.md): two timing-race reds at one gate, counted over 30 days, explained mechanically. Name provisional.
- [How swish-check reads what the shell answered](../swish-check-harness.md): the transcript readers, the gauges, the timeouts with their measurements, and why each script row asserts what it does. Name provisional.
- [swish-check failures, 2026W40](../swish-check-flake-2026w40.md): the week's 18 failures classified; the echo splice that looked like it survived milestone 342 (the kernel and the `console` server drive one UART from two address spaces) all predates it. Name provisional.
- [The truncated echo, 2026W42](../swish-check-flake-2026w42.md): a typed line whose input stopped mid-echo on aarch64, a receive interrupt cleared while a byte waited in the PL011's FIFO, and the `--input-race` boot that proves the fix. Name provisional.
- [Crates that could be released on their own, 2026-10-07](../releasable-crates-2026-10-07.md): which crates could leave, who else implements each, and whether to offer them our proofs. Name provisional.
