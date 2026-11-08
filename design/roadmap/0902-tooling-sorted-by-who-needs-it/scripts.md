# Scripts

An appendix of [nife's tooling, sorted by who needs it](../0902-tooling-sorted-by-who-needs-it.md),
measured 2026-10-12 (UTC) against `e3cc3f128`.
Every file in `script/`.

Columns. Class is 1 (build and prove), 2 (standards), 3 (the fleet) or 4 (one machine). Two
numbers mean the row straddles both. "Called or read by" lists the files that name this path,
comments included. The search covers `script/`, `helpers/`, `.github/`, `.githooks/`, `.claude/`,
`.cargo/` and `xtask/`. "Calls or reads" lists the inventoried paths this file names. Both show four and a
count. `s/` is `script/`, `h/` is `helpers/`, and a bare `.yml` is a workflow. The last
column counts the times the file names calef.

| Path | Class | Why | Called or read by | Calls or reads | Lines | calef |
|---|---|---|---|---|---|---|
| `script/bench` | 1 | boots, tests or measures the OS | `ci.yml`, `toolchain-bump.yml`, `h/lint_icount_baselines.py`, `s/ci-build`, +8 | `s/ci-qemu`, `h/qemu-path.sh` | 67 |  |
| `script/board-console` | 1 | real-board images and consoles | `s/board-image`, `s/job-mix`, `s/soak-test`, `xtask/soak.rs` | `s/ci-qemu`, `s/console`, `h/qemu-path.sh` | 76 | 2 |
| `script/board-image` | 1 | real-board images and consoles | `s/board-netboot`, `s/card-check`, `s/job-mix`, `s/soak-test`, +6 | `s/board-console`, `s/board-netboot`, `s/card-check`, `s/ci-qemu`, +1 | 512 | 1 |
| `script/board-netboot` | 1 | real-board images and consoles | `s/board-image`, `s/lint`, `s/netboot-rehearsal` | `s/board-image`, `s/lint`, `s/server` | 301 | 2 |
| `script/boot-check` | 1 | boots, tests or measures the OS | `s/ci-build`, `xtask/main.rs` | `s/ci-qemu`, `s/lint`, `s/swish-check`, `s/test`, +1 | 79 | 2 |
| `script/boot-file-size-check` | 1 | boots, tests or measures the OS | `s/ci-build`, `xtask/uefi.rs` | `s/ci-build`, `s/fastpath-footprint`, `s/stack-frame-check`, `h/qemu-path.sh` | 169 | 2 |
| `script/bootstrap` | 1 | builds or provisions the toolchain, QEMU or dependencies | `.github/runner-container/Containerfile`, `ci.yml`, `falsifications.yml`, `toolchain-bump.yml`, +15 | `s/ci-build`, `s/ci-qemu`, `s/claim`, `s/drift`, +9 | 320 | 1 |
| `script/build-is-reproducible` | 1 | boots, tests or measures the OS | `s/ci-build` | `s/fatal-risks`, `s/icount` | 144 | 2 |
| `script/card-check` | 1 | real-board images and consoles | `s/board-image`, `xtask/card_check.rs` | `s/board-image`, `s/lint`, `h/qemu-path.sh` | 52 | 1 |
| `script/ci-build` | 1 | the single enumeration of every PR gate | `.github/pull_request_template.md`, `ci.yml`, `h/build-ripgrep.sh`, `h/ci_job_times.py`, +13 | `s/bench`, `s/boot-check`, `s/boot-file-size-check`, `s/bootstrap`, +24 | 364 | 3 |
| `script/ci-qemu` | 1 | builds or provisions the toolchain, QEMU or dependencies | `.github/runner-container/Containerfile`, `ci.yml`, `falsifications.yml`, `toolchain-bump.yml`, +31 | `s/bootstrap`, `s/qemu-check`, `s/setup`, `s/test`, +2 | 119 |  |
| `script/console` | 1 | boots, tests or measures the OS | `.github/ISSUE_TEMPLATE/bug-report.yml`, `s/board-console`, `s/setup` | `s/ci-qemu`, `h/qemu-path.sh`, `notes/scripts.md` | 18 |  |
| `script/coverage` | 1 | boots, tests or measures the OS | `.cargo/mutants.toml`, `ci.yml`, `metrics.yml`, `h/error_paths.py`, +9 | `s/bootstrap`, `s/fuzz`, `s/metrics`, `s/verify`, +3 | 205 | 1 |
| `script/cpu-matrix` | 1 | boots, tests or measures the OS | `ci.yml`, `s/ci-build`, `s/ci-log-baseline`, `xtask/suite.rs`, +1 | `s/ci-qemu`, `s/setup`, `h/qemu-bounded.sh`, `h/qemu-path.sh` | 178 |  |
| `script/crate-probes` | 1 | builds nife-target crates to see what ports | `h/qemu-path.sh`, `s/crypto-probes`, `s/lint` | `s/ci-qemu`, `s/test`, `h/qemu-path.sh` | 579 | 1 |
| `script/crypto-probes` | 1 | builds nife-target crates to see what ports | `h/build-cryptography-exerciser.sh` | `s/ci-qemu`, `s/crate-probes`, `s/test`, `h/build-ripgrep.sh` | 465 | 1 |
| `script/drift` | 1 | builds or provisions the toolchain, QEMU or dependencies | `.cargo/mutants.toml`, `toolchain-drift.yml`, `s/audits`, `s/bootstrap`, +2 | `s/lint`, `s/toolchain-bump`, `notes/scripts.md` | 165 |  |
| `script/falsifications` | 1 | proves a Kani harness can fail | `falsifications.yml`, `verify.yml`, `h/ripgrep-ignore-walk.patch`, `h/rust_source.py`, +9 | `s/ci-qemu`, `s/citations`, `s/lint`, `s/metrics`, +8 | 2327 | 8 |
| `script/fastpath-footprint` | 1 | boots, tests or measures the OS | `ci.yml`, `s/boot-file-size-check`, `s/ci-build`, `xtask/bench.rs` | `s/bench`, `s/bootstrap`, `s/lint`, `s/stack-frame-check` | 1006 | 2 |
| `script/fuzz` | 1 | boots, tests or measures the OS | `ci.yml`, `s/ci-build`, `s/coverage`, `s/undefined-behavior-check` | `s/lint`, `s/test` | 137 | 1 |
| `script/icount` | 1 | boots, tests or measures the OS | `s/build-is-reproducible`, `s/ci-build`, `s/ci-log-baseline`, `xtask/icount.rs`, +2 | `s/bench`, `s/ci-qemu`, `s/test`, `h/qemu-path.sh` | 67 | 3 |
| `script/image-permissions` | 1 | boots, tests or measures the OS | `s/ci-build` | `s/lint` | 154 | 1 |
| `script/interleaving-check` | 1 | boots, tests or measures the OS | `.cargo/mutants.toml`, `ci.yml`, `Cargo.toml`, `s/lint` | `s/ci-build`, `s/lint`, `s/test`, `s/undefined-behavior-check`, +2 | 73 | 2 |
| `script/job-mix` | 1 | boots, tests or measures the OS | `xtask/soak.rs` | `s/bench`, `s/board-console`, `s/board-image`, `s/ci-qemu`, +3 | 109 |  |
| `script/mutation` | 1 | boots, tests or measures the OS | `.cargo/mutants-baseline.txt`, `.cargo/mutants.toml`, `falsifications.yml`, `mutation.yml`, +6 | `s/ci-build`, `s/coverage`, `s/mutation-census`, `s/test`, +1 | 219 | 1 |
| `script/mutation-census` | 1 | boots, tests or measures the OS | `mutation.yml`, `s/mutation` | `s/mutation`, `notes/project-metrics.md` | 462 |  |
| `script/qemu-check` | 1 | builds or provisions the toolchain, QEMU or dependencies | `h/at-risk-check.sh`, `h/lane-claim-check.sh`, `h/qemu-path.sh`, `s/bootstrap`, +3 | `s/bench`, `s/bootstrap`, `s/ci-qemu`, `s/undefined-behavior-check`, +1 | 96 | 2 |
| `script/repeat-under-load` | 1 | boots, tests or measures the OS | `.github/runner-container/Containerfile`, `h/name_provenance.py`, `s/runner-container`, `s/soak-test`, +1 | `s/ci-build`, `s/test` | 213 | 1 |
| `script/runner-container` | 1 | builds or provisions the toolchain, QEMU or dependencies | `.github/runner-container/Containerfile` | `s/bootstrap`, `s/ci-qemu`, `s/lint`, `s/repeat-under-load`, +3 | 239 | 1 |
| `script/server` | 1 | boots, tests or measures the OS | `.github/ISSUE_TEMPLATE/bug-report.yml`, `s/board-netboot`, `s/setup` | `s/ci-qemu`, `h/qemu-path.sh`, `notes/scripts.md` | 17 |  |
| `script/setup` | 1 | builds or provisions the toolchain, QEMU or dependencies | `hook:commit-msg`, `hook:pre-push`, `h/commit-msg-selftest.sh`, `h/qemu-path.sh`, +6 | `s/bootstrap`, `s/ci-qemu`, `s/console`, `s/server`, +4 | 43 |  |
| `script/soak-test` | 1 | boots, tests or measures the OS | `ci.yml`, `h/qemu-runner-x86_64.sh`, `s/ci-build`, `s/job-mix`, +1 | `s/bench`, `s/board-console`, `s/board-image`, `s/ci-qemu`, +2 | 113 | 4 |
| `script/stack-depth-check` | 1 | boots, tests or measures the OS | `ci.yml`, `s/ci-build` | `s/stack-frame-check`, `s/toolchain-bump`, `notes/scripts.md` | 556 | 1 |
| `script/stack-frame-check` | 1 | boots, tests or measures the OS | `.cargo/config.toml`, `ci.yml`, `h/at-risk-check.sh`, `h/lane-claim-check.sh`, +5 | `s/test`, `s/verify` | 270 | 1 |
| `script/supply-chain` | 1 | supply chain: vendored code and advisories | `ci.yml`, `vendor-watch.yml`, `h/build-cryptography-exerciser.sh`, `s/ci-build`, +1 | `s/bootstrap`, `s/coverage`, `s/lint`, `s/vendor-verify`, +1 | 96 | 1 |
| `script/swish-check` | 1 | boots, tests or measures the OS | `.cargo/mutants.toml`, `h/build-ripgrep.sh`, `h/package-http-peer`, `h/qemu-runner-aarch64.sh`, +17 | `s/ci-build`, `s/ci-qemu`, `s/lint`, `s/test`, +1 | 150 | 2 |
| `script/test` | 1 | boots, tests or measures the OS | `.cargo/mutants.toml`, `.github/ISSUE_TEMPLATE/bug-report.yml`, `ci.yml`, `toolchain-drift.yml`, +44 | `s/ci-qemu`, `s/setup`, `h/qemu-path.sh`, `notes/scripts.md` | 22 |  |
| `script/toolchain-bump` | 1 | builds or provisions the toolchain, QEMU or dependencies | `toolchain-bump.yml`, `h/lint_icount_baselines.py`, `h/prune_toolchains.py`, `rust-toolchain.toml`, +6 | `s/bench`, `s/ci-qemu`, `s/fmt`, `s/lint`, +2 | 143 | 1 |
| `script/undefined-behavior-check` | 1 | boots, tests or measures the OS | `undefined-behavior-check.yml`, `s/interleaving-check`, `s/qemu-check` | `s/bootstrap`, `s/ci-build`, `s/ci-qemu`, `s/coverage`, +3 | 40 | 1 |
| `script/update` | 1 | builds or provisions the toolchain, QEMU or dependencies | `s/bootstrap` | `s/bootstrap`, `s/ci-qemu`, `h/qemu-path.sh`, `notes/scripts.md` | 23 |  |
| `script/vendor-verify` | 1 | supply chain: vendored code and advisories | `ci.yml`, `vendor-watch.yml`, `s/ci-log-baseline`, `s/supply-chain`, +1 | `s/toolchain-bump`, `s/verify`, `notes/scripts.md` | 234 |  |
| `script/vendor-watch` | 1 | supply chain: vendored code and advisories | `vendor-watch.yml` | `s/vendor-verify`, `notes/merge-queue.md` | 372 | 1 |
| `script/verify` | 1 | boots, tests or measures the OS | `.cargo/mutants.toml`, `ci.yml`, `falsifications.yml`, `kani-reach.yml`, +15 | `s/bootstrap`, `s/citations`, `s/coverage`, `s/falsifications`, +8 | 506 | 3 |
| `script/verify-riscv64` | 1 | boots, tests or measures the OS | `ci.yml`, `verify.yml`, `s/lint`, `s/verify` | `s/ci-qemu`, `s/verify` | 230 | 1 |
| `script/crate-standalone` | 1/2 | a crate stands outside nife: build check and vocabulary check | `h/crate_release.py`, `s/metrics` | `s/falsifications`, `s/metrics`, `s/names` | 240 | 4 |
| `script/cadence-check` | 1/3 | which scheduled workflows stopped producing a result; also reads effort and the watcher | `trunk-health.yml`, `undefined-behavior-check.yml`, `h/trunk-health.sh`, `h/watcher_watch.py`, +4 | `s/audits`, `s/ci-build`, `s/effort`, `s/lint`, +4 | 284 | 3 |
| `script/ci-log-baseline` | 1/3 | mines failed CI logs per check; gate evidence, but kept for the queue's triage | `ci.yml` | `s/coverage`, `s/cpu-matrix`, `s/icount`, `s/lint`, +4 | 392 | 3 |
| `script/netboot-rehearsal` | 1/4 | a netboot rehearsal; carries one machine's hardware address and the home local area network prefix | none | `s/board-netboot`, `s/ci-qemu`, `h/qemu-bounded.sh`, `h/qemu-path.sh`, +1 | 716 | 3 |
| `script/apropos` | 2 | search the documentation | `s/lint`, `xtask/main.rs`, `xtask/manual.rs` | `s/ci-qemu`, `h/qemu-path.sh`, `notes/scripts.md` | 109 | 1 |
| `script/audits` | 2 | audit cadence and index | `audit-cadence.yml`, `s/cadence-check`, `s/lint`, `s/roadmap`, +2 | `s/citations`, `s/drift`, `s/lint`, `s/roadmap`, +2 | 576 | 1 |
| `script/citations` | 2 | citations resolve and quotations still hold | `h/needs-maintainer-selftest.sh`, `h/open-lane.sh`, `h/prose_only.py`, `h/prose_ratchet.py`, +10 | `s/decisions`, `s/falsifications`, `s/lint`, `s/metrics`, +3 | 1340 | 5 |
| `script/decisions` | 2 | decisions tooling | `h/prose_ratchet.py`, `s/catch-up`, `s/citations`, `s/fatal-risks`, +5 | `s/lint`, `s/roadmap`, `h/prose_ratchet.py`, `briefs/rebase-onto-main.md` | 569 | 9 |
| `script/fatal-risks` | 2 | fatal-risk record | `h/prose_ratchet.py`, `s/build-is-reproducible`, `s/lint`, `s/metrics`, +1 | `s/cadence-check`, `s/citations`, `s/decisions`, `s/falsifications`, +4 | 869 | 14 |
| `script/fmt` | 2 | formatting | `hook:pre-push`, `audit-cadence.yml`, `Cargo.toml`, `s/ci-build`, +3 | `s/lint`, `s/toolchain-bump`, `notes/scripts.md` | 85 | 1 |
| `script/journeys` | 2 | user-journey record | `h/roadmap_block.py` | `s/decisions`, `s/lint`, `s/roadmap`, `h/roadmap_block.py` | 223 | 1 |
| `script/lint` | 2 | the lint gate; also runs ~20 class-3 selftests (see edges) | `.cargo/mutants.toml`, `hook:commit-msg`, `hook:pre-push`, `architect-label.yml`, +103 | `s/apropos`, `s/audits`, `s/board-netboot`, `s/bootstrap`, +87 | 4074 | 41 |
| `script/names` | 2 | names and provenance | `h/baseline_drift.py`, `h/cap_abbreviation.py`, `h/ci_job_times.py`, `h/coe_actions.py`, +29 | `s/catch-up`, `s/decisions`, `s/lint`, `s/metrics`, +4 | 734 | 19 |
| `script/roadmap` | 2 | roadmap tooling | `hook:pre-push`, `ci.yml`, `ready-status.yml`, `h/coe_actions.py`, +15 | `s/audits`, `s/citations`, `s/decisions`, `s/fatal-risks`, +12 | 1819 | 27 |
| `script/stranger-test` | 2 | a stranger reaches a build from the tree alone (principle 3) | `stranger-cadence.yml`, `s/lint` | `s/audits`, `s/cadence-check`, `s/lint`, `s/setup`, +5 | 882 | 2 |
| `script/catch-up` | 2/3 | derived "what changed" view; written for a returning reviewer | `h/roadmap_block.py`, `s/lint`, `s/names` | `s/decisions`, `s/drift`, `s/roadmap`, `h/roadmap_block.py`, +2 | 545 | 3 |
| `script/metrics` | 2/3 | project metrics; mixes kernel series with method series (effort, models, queue) | `hook:commit-msg`, `ci.yml`, `metrics.yml`, `h/baseline_drift.py`, +28 | `s/coverage`, `s/crate-standalone`, `s/effort`, `s/fatal-risks`, +21 | 3731 | 48 |
| `script/rule-violations` | 2/3 | rule-violation ledger; rules are the tree's, strikes are lane reports | none | `s/audits`, `s/citations`, `s/decisions`, `s/lint`, +3 | 121 |  |
| `script/architect-queue` | 3 | the architect's queue | `h/merge-drain.sh`, `h/open-question.jq` | `h/needs-maintainer.jq`, `h/open-question.jq`, `notes/skills/decisions/SKILL.md`, `notes/scripts.md` | 89 | 4 |
| `script/claim` | 3 | cuts a lane: branch, worktree, empty commit, draft PR | `h/branch-name-check.sh`, `h/claim-selftest.sh`, `s/bootstrap`, `s/lint` | `s/lint`, `s/metrics`, `h/branch-name-check.sh`, `h/claim-selftest.sh`, +7 | 245 | 4 |
| `script/effort` | 3 | machine effort from local agent session records | `s/cadence-check`, `s/metrics` | `s/cadence-check`, `s/lint`, `s/metrics`, `metrics.yml`, +2 | 1052 | 10 |
| `script/nanny` | 3 | delegates queue watching to an agent | none | `s/redo-rate`, `h/merge-drain.sh`, `briefs/rebase-onto-main.md`, `notes/open-model-lanes.md` | 70 | 2 |
| `script/preflight-queue` | 3 | replays the merge queue locally and dequeues | none | `s/ci-build`, `s/falsifications`, `s/lint`, `s/test`, +5 | 556 | 1 |
| `script/record-ruling` | 3 | records an architect ruling and swaps labels | `architect-label.yml`, `h/architect-label-comment.sh`, `h/architect-ruled.sh`, `h/merge-drain.sh`, +3 | `h/needs-maintainer.jq`, `h/open-question.jq`, `notes/scripts.md` | 145 | 8 |
| `script/redo-rate` | 3 | redo rate of rented-model lanes | `s/nanny` | `s/citations`, `s/lint`, `s/roadmap`, `h/open-lane.sh`, +1 | 56 | 1 |
