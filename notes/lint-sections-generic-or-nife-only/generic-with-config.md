# The generic-with-config sections of script/lint

*An appendix to [`../lint-sections-generic-or-nife-only.md`](../lint-sections-generic-or-nife-only.md). Name: provisional, minted 2026-11-03 (UTC) by lane/lint-section-tagging.*

Thirty-six sections port to a second repository once it supplies the values in the config column. The number is the section's place in `script/lint`, counted by `helpers/lint-no-cargo.awk`. Seconds come from merge-group run 41694693300.

The section headers cite §81 (a dependency stays upgradable), §212 (a prose budget), §213 (writing
standards), §266 (a Rust source file stays under 2,000 lines), §277 (cite a heading or a symbol,
never a line number), §282 (when a file is split, the parent becomes an index) and milestone 929
(comments in the Linux style across first-party Rust).

| # | section | what it checks | tag | what it reads | config a second repository supplies | seconds (wall / CPU) |
|---|---|---|---|---|---|---|
| 1 | the pre-push hook's contract with this script | the pre-push hook exists and every script and flag it names still exists; two hook selftests | generic-with-config | `.githooks/pre-push`, `script/lint` | the hook path and the runner's flag names | 3 / 3.1 |
| 2 | the commit-msg hook (an agent's commit carries a Co-Authored-By trailer) | the commit-msg hook exists and refuses an agent commit with no trailer; the runner's own two selftests | generic-with-config | `.githooks/commit-msg`, `helpers/lint-sections.sh`, `helpers/lint-no-cargo.awk` | the hook file; the runner selftests move with the runner | 5 / 4.9 |
| 3 | no reproducer in this branch's commit messages (a replay line, a seed assignment) | no replay line or seed assignment in the branch's commit messages | generic-with-config | `git log origin/main..HEAD`, `helpers/replay-line-check.sh` | the seed variable prefix (`NIFE_`), the base branch | 1 / 0.4 |
| 11 | clippy: host crates + xtask | clippy `-D warnings` over the host workspace | generic-with-config | `helpers/host-excluded-crates.sh` (names `user_mode_runtime`), `helpers/clippy_record.py` | the exclusion list; where diagnostics are saved | 10 / 37.5 |
| 16 | clippy: the Kani proof harnesses (--cfg kani) | clippy over `cfg(kani)` code through a shim | generic-with-config | `helpers/kani-lint-shim/`, `helpers/host-excluded-crates.sh` | the exclusion list | 10 / 38.6 |
| 38 | every committed .dtb matches its .dts | every committed `.dtb` decompiles to what its `.dts` compiles to | generic-with-config | `.dts` and `.dtb` pairs, `dtc` | opt-in: zero pairs fails | 0 / 0.4 |
| 40 | every fence names its counterpart | every fence and non-relaxed ordering names its counterpart (`PAIR:`) | generic-with-config | every `.rs` file | the marker word; the note its message cites | 5 / 4.6 |
| 45 | TODO markers cite a milestone | a TODO names the milestone that owns it | generic-with-config | every tracked file | the citation form (milestone or issue) | 1 / 1.4 |
| 47 | rustdoc (broken intra-doc links) | rustdoc broken intra-doc links and HTML tags | generic-with-config | the workspace | packages to exclude, targets | 38 / 63.9 |
| 51 | dependency upgrades stay reachable | a Dependabot `ignore:` carries a recorded exception | generic-with-config | `.github/dependabot.yml` | the exception marker text (`EXCEPTION (§81)`) | 0 / 0.0 |
| 52 | every committed Cargo.lock has an update watcher | every committed `Cargo.lock` has a Dependabot cargo entry | generic-with-config | `.github/dependabot.yml`, every `Cargo.lock` | the excluded directories | 0 / 0.0 |
| 64 | script docs | every script has an entry in the scripts note | generic-with-config | `script/`, `notes/scripts.md` | the script directory and its index file | 0 / 0.2 |
| 69 | fixed /tmp paths in scripts (they race across worktrees; see the note above) | no redirect into a fixed `/tmp` path in a script | generic-with-config | `script/*`, `helpers/*.sh` | the script globs | 0 / 0.1 |
| 72 | every Kani harness carries a falsification record | every Kani harness carries a falsification record | generic-with-config | `script/falsifications` | the harness list; crate-workflows runs its own `falsify.py` | 2 / 3.1 |
| 75 | environment names in prose | every environment name in prose is live or retired | generic-with-config | every tracked file | the prefix (`NIFE_`, `CRICKER_`) and the retired table | 2 / 2.1 |
| 84 | every .rs file under a crate is declared by a module tree | every `.rs` file is in a module tree | generic-with-config | every `.rs` file | `ALLOWLIST` (vendored files) | 2 / 2.0 |
| 86 | markdown links and the notes index | markdown links resolve; every note is in the notes index | generic-with-config | every `.md` file, `notes/README/` | the index pages; the excluded directories | 0 / 0.3 |
| 87 | the prose ratchet (§212 words, §213 sentences, bold and passives) | the prose ratchet (words, sentences, bold, passives) | generic-with-config | `PROSE_DIRS`, `design/prose-baseline.tsv`, `helpers/roadmap_block.py` | the prose directories, the baseline, the caps | 11 / 10.6 |
| 89 | the file-length ratchet (§266's 2,000-line ceiling on Rust files) | the file-length ratchet, 2,000 lines | generic-with-config | `design/file-length-baseline.tsv` | the baseline path; `CEILING` | 0 / 0.1 |
| 91 | the lint ratchet's selftest (clippy lints counted against a baseline that only falls) | the lint ratchet's selftest | generic-with-config | `helpers/lint_ratchet.py` | as section 92 | 0 / 0.1 |
| 92 | the lint ratchet (the clippy sections' saved diagnostics, judged; no clippy of its own) | saved clippy diagnostics against a falling baseline | generic-with-config | `target/lint-clippy`, `design/lint-baseline.tsv` | `CONFIGURATIONS` (names `kernel`), the baseline | 7 / 7.2 |
| 93 | the comment-block ratchet (calef's 40-line cap on a comment block, 2026-10-07; per-directory caps and in-body ceilings, milestone 929) | the comment-block ratchet, 40 lines | generic-with-config | `design/comment-block-baseline.tsv` | the baseline; per-directory caps naming `kernel/` | 5 / 4.5 |
| 94 | the parent-index check (§282: when a file is split, the parent becomes an index) | a split file's parent becomes an index | generic-with-config | `design/parent-index-baseline.tsv` | the baseline | 0 / 0.7 |
| 95 | American spelling and metric units (calef, 2026-10-06) | American spelling and metric units | generic-with-config | every tracked file, `_typos.toml` | `CEILINGS`, the word lists | 7 / 16.2 |
| 96 | §298 (a document states what is in force now), calef 2026-10-13 | a document states what is in force now | generic-with-config | every `.md` file | `PHRASES`, the excluded directories, `CEILING` | 2 / 2.6 |
| 98 | unit symbols are spelled out (2026-10-27) | unit symbols are spelled out | generic-with-config | every `.rs` file, `helpers/cap_abbreviation.py` | `KEPT`, `CEILING` | 3 / 3.1 |
| 99 | no line-number citations (§277: cite a heading or a symbol) | no line-number citations | generic-with-config | every tracked file | `CODE_CEILING` (712) | 2 / 1.7 |
| 101 | exception markers the ratchets honor (a warning, never a failure) | exception markers the ratchets honor (a warning) | generic-with-config | three ratchets imported | the same as sections 87, 94 and 99 | 1 / 1.5 |
| 103 | CI job wall-time budgets | CI job wall-time budgets | generic-with-config | `.github/workflows/ci.yml`, `verify.yml`, `.github/ci-job-budgets` | `WORKFLOWS`, the budgets file | 0 / 0.0 |
| 106 | every workflow records the run it was watched failing in | every workflow records a run it was watched failing in; three unrelated selftests | generic-with-config | `.github/workflows/` | `CUTOFF_COMMIT` (a nife commit) | 2 / 2.4 |
| 116 | em-dashes | no em-dash outside vendored trees and quotations | generic-with-config | every tracked file | the excluded directories | 0 / 0.1 |
| 122 | unused dependencies | no unused dependency (cargo-machete) | generic-with-config | the listed directories | the directory list | 1 / 3.6 |
| 123 | tests the suite cannot see | no `#[test]` inside `cfg(kani)`; every system-test module carries a `cfg` | generic-with-config | every `.rs` file, `system_tests/src/user.rs` | the second half is nife's | 1 / 0.6 |
| 126 | halves that are not halves | no count above two before "halves" | generic-with-config | every tracked text file | the word rule | 0 / 2.5 |
| 128 | sh -n (does the dev machine's own /bin/sh parse every script) | every `#!/bin/sh` script parses under `/bin/sh` | generic-with-config | `script/*`, `helpers/*.sh` | the script globs | 1 / 0.5 |
| 130 | every kani-verifier install reads .kani-version | every Kani install reads `.kani-version` | generic-with-config | `.kani-version`, `script/` | the install scripts | 0 / 0.7 |
