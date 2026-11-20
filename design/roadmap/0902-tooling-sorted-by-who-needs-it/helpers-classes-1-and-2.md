# Helpers in classes 1 and 2

An appendix of [nife's tooling, sorted by who needs it](../0902-tooling-sorted-by-who-needs-it.md),
measured 2026-10-12 (UTC) against `e3cc3f128`.
Every file in `helpers/` that is not class 3, including those that straddle class 3.
Noted 2026-10-15 (UTC): four inventoried files are gone since the measurement, deleted 2026-10-13
and 2026-10-14 as basalt's packages took over the foreign-program builds. `build-ioping.sh`,
`build-ripgrep.sh`, `build-speedtest1.sh` and `ripgrep-ignore-walk.patch` exist only at the base,
and the rows and cross-references naming them describe `e3cc3f128`, not the tree a reader stands in.

Columns. Class is 1 (build and prove), 2 (standards), 3 (the fleet) or 4 (one machine). Two
numbers mean the row straddles both. "Called or read by" lists the files that name this path,
comments included. The search covers `script/`, `helpers/`, `.github/`, `.githooks/`, `.claude/`,
`.cargo/` and `xtask/`. "Calls or reads" lists the inventoried paths this file names. Both show four and a
count. `s/` is `script/`, `h/` is `helpers/`, and a bare `.yml` is a workflow. The last
column counts the times the file names calef.

| Path | Class | Why | Called or read by | Calls or reads | Lines | calef |
|---|---|---|---|---|---|---|
| `helpers/baseline_drift.py` | 1 | proof, mutation or coverage measurement | `h/prose_only.py`, `h/prose_ratchet.py`, `s/lint`, `s/metrics` | `s/lint`, `s/metrics`, `s/names`, `h/prose_ratchet.py`, +1 | 456 | 2 |
| `helpers/build-c-program.sh` | 1 | builds a foreign program for nife targets | `h/build-speedtest1.sh`, `h/build-ioping.sh` | `h/build-ioping.sh`, `h/build-ripgrep.sh`, `h/build-speedtest1.sh`, `h/c-library-cflags.sh` | 48 |  |
| `helpers/build-cryptography-exerciser.sh` | 1 | builds a foreign program for nife targets | `h/build-pinned-tls-exerciser.sh`, `xtask/archive.rs`, `xtask/farm.rs` | `s/crypto-probes`, `s/supply-chain`, `s/test` | 58 | 1 |
| `helpers/build-ioping.sh` | 1 | builds a foreign program for nife targets | `h/build-c-program.sh` | `s/test`, `h/build-speedtest1.sh` | 52 |  |
| `helpers/build-pinned-tls-exerciser.sh` | 1 | builds a foreign program for nife targets | `xtask/archive.rs`, `xtask/farm.rs`, `h/build-cryptography-exerciser.sh` | `s/test`, `h/build-cryptography-exerciser.sh` | 42 |  |
| `helpers/build-ripgrep.sh` | 1 | builds a foreign program for nife targets | `ci.yml`, `h/build-c-program.sh`, `h/build-speedtest1.sh`, `h/ripgrep-ignore-walk.patch`, +10 | `s/ci-build`, `s/swish-check`, `s/test`, `h/ripgrep-ignore-walk.patch` | 172 | 1 |
| `helpers/build-speedtest1.sh` | 1 | builds a foreign program for nife targets | `h/build-c-program.sh`, `h/build-ioping.sh` | `s/test`, `h/build-c-program.sh`, `h/build-ripgrep.sh` | 74 | 1 |
| `helpers/c-library-cc.sh` | 1 | builds a foreign program for nife targets | `h/build-ioping.sh`, `h/build-speedtest1.sh` | none | 13 |  |
| `helpers/c-library-cflags.sh` | 1 | builds a foreign program for nife targets | `h/build-c-program.sh`, `h/build-ioping.sh`, `h/build-speedtest1.sh` | none | 37 |  |
| `helpers/c-library-headers.sh` | 1 | builds a foreign program for nife targets | none | none | 39 |  |
| `helpers/cache_save_scope.py` | 1 | CI plumbing: skip classifier, cache scope | `ci.yml`, `s/lint` | `s/ci-qemu`, `s/lint` | 308 |  |
| `helpers/error_paths.py` | 1 | proof, mutation or coverage measurement | none | `s/coverage`, `s/names` | 626 | 1 |
| `helpers/kani_reach.py` | 1 | proof, mutation or coverage measurement | `kani-reach.yml` | `h/memory-bounded-runner.sh` | 601 |  |
| `helpers/lint_icount_baselines.py` | 1 | proof, mutation or coverage measurement | `s/lint` | `s/bench`, `s/lint`, `s/toolchain-bump`, `hook:pre-push` | 73 |  |
| `helpers/memory-bounded-runner.sh` | 1 | QEMU runner, bound or path | `mutation.yml`, `h/kani_reach.py`, `s/mutation` | `s/mutation`, `h/qemu-bounded.sh`, `mutation.yml` | 108 |  |
| `helpers/mutation_inflow.py` | 1 | proof, mutation or coverage measurement | `mutation.yml`, `s/lint` | `s/lint`, `s/mutation` | 461 | 1 |
| `helpers/name-server-peer` | 1 | host peer a QEMU test talks to | `h/qemu-runner-aarch64.sh`, `h/qemu-runner-riscv64.sh`, `h/qemu-runner-x86_64.sh`, `h/qemu-uefi-x86_64.sh` | `h/package-http-peer`, `h/tls-peer` | 161 |  |
| `helpers/network-bench-peer` | 1 | host peer a QEMU test talks to | `h/qemu-uefi-x86_64.sh`, `xtask/network_bench.rs` | `h/qemu-uefi-x86_64.sh` | 12 |  |
| `helpers/package-http-peer` | 1 | host peer a QEMU test talks to | `h/name-server-peer`, `h/qemu-runner-aarch64.sh`, `h/tls-peer`, `xtask/disk.rs`, +4 | `s/swish-check` | 68 |  |
| `helpers/prose_only.py` | 1 | CI plumbing: skip classifier, cache scope | `ci.yml`, `codeql.yml`, `verify.yml`, `h/job-budget.py`, +1 | `s/citations`, `s/lint`, `s/metrics`, `s/names`, +10 | 538 | 1 |
| `helpers/qemu-bounded-selftest.sh` | 1 | QEMU runner, bound or path | `s/metrics` | `s/lint`, `s/test`, `h/qemu-bounded.sh` | 248 |  |
| `helpers/qemu-bounded.sh` | 1 | QEMU runner, bound or path | `h/memory-bounded-runner.sh`, `h/qemu-bounded-selftest.sh`, `h/qemu-runner-riscv64.sh`, `h/qemu-runner-x86_64.sh`, +9 | `s/test`, `h/qemu-runner-x86_64.sh` | 234 |  |
| `helpers/qemu-path.sh` | 1 | QEMU runner, bound or path | `h/branch-name-check.sh`, `h/qemu-stick.sh`, `s/apropos`, `s/bench`, +29 | `s/bootstrap`, `s/ci-qemu`, `s/crate-probes`, `s/lint`, +3 | 77 | 1 |
| `helpers/qemu-runner-aarch64.sh` | 1 | QEMU runner, bound or path | `.cargo/config.toml`, `h/qemu-uefi-x86_64.sh`, `s/ci-build`, `xtask/inspect.rs`, +5 | `s/ci-build`, `s/swish-check`, `s/test`, `h/name-server-peer`, +2 | 599 |  |
| `helpers/qemu-runner-riscv64.sh` | 1 | QEMU runner, bound or path | `.cargo/config.toml`, `xtask/bench.rs`, `xtask/boot_check.rs`, `xtask/icount.rs`, +4 | `s/swish-check`, `h/name-server-peer`, `h/qemu-bounded.sh` | 341 |  |
| `helpers/qemu-runner-x86_64.sh` | 1 | QEMU runner, bound or path | `.cargo/config.toml`, `h/qemu-bounded.sh`, `h/qemu-uefi-x86_64.sh`, `s/falsifications`, +6 | `s/soak-test`, `h/name-server-peer`, `h/qemu-bounded.sh` | 426 |  |
| `helpers/qemu-stick.sh` | 1 | QEMU runner, bound or path | `h/stick-maker-proof.sh`, `xtask/stick.rs` | `h/qemu-bounded.sh`, `h/qemu-path.sh` | 99 |  |
| `helpers/qemu-uefi-x86_64.sh` | 1 | QEMU runner, bound or path | `h/network-bench-peer`, `s/netboot-rehearsal`, `xtask/disk_throughput.rs`, `xtask/install.rs`, +4 | `s/bootstrap`, `s/ci-qemu`, `s/names`, `s/swish-check`, +6 | 329 | 1 |
| `helpers/ripgrep-ignore-walk.patch` | 1 | builds a foreign program for nife targets | `h/build-ripgrep.sh` | `s/falsifications`, `s/swish-check`, `h/build-ripgrep.sh` | 85 | 2 |
| `helpers/stick-maker-proof.sh` | 1 | stick_maker end to end | `stick-maker-hosts.yml`, `s/coverage`, `.cargo/mutants.toml` | `h/qemu-stick.sh` | 97 |  |
| `helpers/tls-peer` | 1 | host peer a QEMU test talks to | `h/name-server-peer`, `h/qemu-runner-aarch64.sh`, `h/qemu-uefi-x86_64.sh`, `xtask/suite.rs`, +2 | `h/package-http-peer` | 216 |  |
| `helpers/verify_times.py` | 1 | proof, mutation or coverage measurement | `metrics.yml`, `verify-shard-refresh.yml`, `h/ci_job_times.py`, `h/prose_only.py`, +3 | `s/falsifications`, `s/names`, `s/verify`, `verify-shard-refresh.yml`, +1 | 751 | 2 |
| `helpers/ci-job-times-fixtures/` | 1/3 | fixtures (1 files) | none | none | 404 |  |
| `helpers/ci_job_times.py` | 1/3 | CI time per job plus near-budget tracking issues | `metrics.yml`, `h/job-budget.py`, `h/merge-drain.sh`, `h/needs-maintainer.jq`, +3 | `s/ci-build`, `s/metrics`, `s/names`, `h/job-budget.py`, +8 | 777 | 2 |
| `helpers/job-budget.py` | 1/3 | CI job wall-time budget; budget is the queue's | `.github/ci-job-budgets`, `ci.yml`, `h/ci_job_times.py`, `s/lint` | `s/lint`, `h/ci_job_times.py`, `h/prose_only.py`, `.github/ci-job-budgets`, +1 | 331 | 1 |
| `helpers/scope-merge-base-selftest.sh` | 1/3 | merge base for CI scope and for the architect label, shared | `hook:pre-push`, `architect-label.yml`, `h/scope-merge-base.sh`, `s/lint` | `h/scope-merge-base.sh` | 107 |  |
| `helpers/scope-merge-base.sh` | 1/3 | merge base for CI scope and for the architect label, shared | `architect-label.yml`, `ci.yml`, `codeql.yml`, `verify.yml`, +2 | `h/scope-merge-base-selftest.sh`, `architect-label.yml`, `ci.yml` | 74 |  |
| `helpers/cap_abbreviation.py` | 2 | ratchet or style check run by lint | `h/line_citations.py`, `s/lint` | `s/lint`, `s/names` | 420 | 4 |
| `helpers/coe_actions.py` | 2 | correction-of-error action items | `s/metrics`, `s/roadmap` | `s/metrics`, `s/names`, `s/roadmap` | 204 | 2 |
| `helpers/comment_block_ratchet.py` | 2 | ratchet or style check run by lint | `s/lint` | `s/lint`, `s/names`, `h/file_length_ratchet.py`, `h/prose_ratchet.py` | 785 | 5 |
| `helpers/crate_release.py` | 2 | crate release readiness for 857 | `s/metrics` | `s/crate-standalone`, `s/metrics`, `s/names` | 186 | 2 |
| `helpers/file_length_ratchet.py` | 2 | ratchet or style check run by lint | `h/comment_block_ratchet.py`, `h/lint_ratchet.py`, `h/parent_index.py`, `s/lint`, +1 | `s/lint`, `s/metrics`, `s/names`, `h/prose_ratchet.py` | 382 | 3 |
| `helpers/function_length.py` | 2 | ratchet or style check run by lint | `h/lint_ratchet.py`, `s/metrics` | `s/lint`, `s/metrics`, `s/names`, `h/lint_ratchet.py`, +1 | 437 |  |
| `helpers/house_style.py` | 2 | ratchet or style check run by lint | `h/interface_stability.py`, `s/lint`, `h/cap_abbreviation.py` | `s/bootstrap`, `s/lint`, `s/names` | 543 | 5 |
| `helpers/interface_stability.py` | 2 | interface stability series (gates the split) | `metrics.yml`, `h/prose_only.py`, `s/lint`, `s/metrics` | `s/lint`, `s/metrics`, `s/names`, `h/house_style.py`, +1 | 1047 | 2 |
| `helpers/kani-lint-shim/kani.rs` | 2 | clippy over proof harnesses | `s/lint`, `h/kani-lint-shim/kani_attributes.rs` | `s/lint`, `s/verify` | 59 |  |
| `helpers/kani-lint-shim/kani_attributes.rs` | 2 | clippy over proof harnesses | `s/lint` | `s/lint` | 40 | 1 |
| `helpers/line_citations.py` | 2 | ratchet or style check run by lint | `s/lint` | `s/lint`, `s/names`, `h/cap_abbreviation.py` | 352 | 3 |
| `helpers/lint-no-cargo-selftest.sh` | 2 | pre-push runs lint without cargo | `s/lint` | `s/lint`, `h/lint-no-cargo.awk` | 62 |  |
| `helpers/lint-no-cargo.awk` | 2 | pre-push runs lint without cargo | `hook:pre-push`, `h/lint-no-cargo-selftest.sh`, `s/lint` | `s/lint` | 61 |  |
| `helpers/lint_arch_parity.py` | 2 | codebase-rule check run by lint | `s/lint` | `s/lint` | 539 |  |
| `helpers/lint_ratchet.py` | 2 | ratchet or style check run by lint | `h/function_length.py`, `s/lint` | `s/lint`, `s/metrics`, `s/names`, `h/file_length_ratchet.py`, +1 | 531 | 1 |
| `helpers/name_provenance.py` | 2 | shared reader for roadmap, names, Rust source | `h/roadmap_proposals.py`, `s/metrics`, `s/names` | `s/lint`, `s/metrics`, `s/names`, `s/repeat-under-load`, +1 | 491 | 3 |
| `helpers/observed_failing.py` | 2 | codebase-rule check run by lint | `s/lint` | `s/lint` | 279 |  |
| `helpers/orphan_rust.py` | 2 | codebase-rule check run by lint | `s/lint` | `s/lint`, `s/names` | 348 | 1 |
| `helpers/packages.py` | 2 | codebase-rule check run by lint | `s/lint`, `s/metrics`, `s/verify` | `s/lint`, `s/metrics` | 749 | 7 |
| `helpers/parent_index.py` | 2 | ratchet or style check run by lint | `s/lint` | `s/lint`, `s/names`, `h/file_length_ratchet.py` | 827 | 2 |
| `helpers/prose_ratchet.py` | 2 | ratchet or style check run by lint | `h/baseline_drift.py`, `h/comment_block_ratchet.py`, `h/file_length_ratchet.py`, `h/prose_only.py`, +5 | `s/citations`, `s/decisions`, `s/fatal-risks`, `s/lint`, +5 | 1405 | 30 |
| `helpers/roadmap_block.py` | 2 | shared reader for roadmap, names, Rust source | `h/prose_ratchet.py`, `h/roadmap_migrate.py`, `s/catch-up`, `s/citations`, +2 | `s/catch-up`, `s/citations`, `s/journeys`, `s/metrics`, +4 | 237 | 4 |
| `helpers/roadmap_migrate.py` | 2 | shared reader for roadmap, names, Rust source | `h/roadmap_block.py`, `s/roadmap` | `s/citations`, `s/roadmap`, `h/prose_ratchet.py`, `h/roadmap_block.py`, +1 | 412 | 3 |
| `helpers/roadmap_proposals.py` | 2 | shared reader for roadmap, names, Rust source | `h/roadmap_migrate.py`, `s/roadmap` | `s/metrics`, `s/names`, `s/roadmap`, `h/name_provenance.py`, +1 | 185 | 3 |
| `helpers/rust_source.py` | 2 | shared reader for roadmap, names, Rust source | `h/function_length.py`, `h/name_provenance.py`, `h/roadmap_proposals.py`, `s/falsifications`, +3 | `s/falsifications`, `s/lint`, `s/metrics`, `s/verify` | 673 | 3 |
| `helpers/system_call_asm.py` | 2 | codebase-rule check run by lint | `s/lint` | `s/lint`, `s/names` | 162 | 1 |
| `helpers/workflow_swallows.py` | 2 | codebase-rule check run by lint | `s/lint` | `s/lint`, `s/names` | 432 |  |
| `helpers/commit-msg-selftest.sh` | 2/3 | Co-Authored-By trailer on an agent commit; 887 keeps it | `hook:commit-msg`, `s/lint` | `s/setup`, `hook:commit-msg` | 75 | 2 |
| `helpers/empty-diff-check.sh` | 2/3 | a merge that changes nothing; 887 keeps it for crate repos | `empty-diff.yml`, `h/empty-diff-selftest.sh`, `s/lint` | `s/lint`, `h/empty-diff-selftest.sh`, `empty-diff.yml` | 64 | 1 |
| `helpers/empty-diff-selftest.sh` | 2/3 | a merge that changes nothing; 887 keeps it for crate repos | `empty-diff.yml`, `h/empty-diff-check.sh`, `s/lint` | `s/lint`, `h/empty-diff-check.sh` | 72 |  |
| `helpers/week_digest.py` | 2/3 | metrics digest of what moved a series | `metrics.yml`, `h/prose_only.py`, `s/metrics` | `s/metrics` | 115 |  |
