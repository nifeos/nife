# The generic sections of script/lint

*An appendix to [`../lint-sections-generic-or-nife-only.md`](../lint-sections-generic-or-nife-only.md). Name: provisional, minted 2026-11-03 (UTC) by lane/lint-section-tagging.*

Twelve sections run unchanged on any Rust repository in git, with the usual exclusions (`vendor/`, `target/`, `patches/`). The number is the section's place in `script/lint`, counted by `helpers/lint-no-cargo.awk`. Seconds come from merge-group run 41694693300.

| # | section | what it checks | tag | what it reads | config a second repository supplies | seconds (wall / CPU) |
|---|---|---|---|---|---|---|
| 32 | unsafe fn contracts | every `unsafe fn` outside a trait impl carries a `# Safety` section | generic | every `.rs` file | none | 1 / 0.7 |
| 41 | module-wide dead-code suppression | no module-wide `dead_code` or `unused` allow | generic | every `.rs` file | none | 0 / 0.1 |
| 43 | conflict markers | no merge conflict markers | generic | every tracked file | none | 0 / 0.2 |
| 44 | actions are pinned to a commit SHA | every `uses:` pinned to a 40-hex SHA | generic | `.github/` | none | 0 / 0.0 |
| 76 | tracked files the tree ignores | no tracked file matches `.gitignore` | generic | the index | none | 0 / 0.0 |
| 77 | the empty-diff check | the empty-diff check's selftest | generic | `helpers/empty-diff-selftest.sh` | none | 0 / 0.1 |
| 78 | workflow steps that swallow a failure | a `continue-on-error` step says where its failure goes | generic | `.github/workflows/` | none | 0 / 0.1 |
| 79 | every workflow file parses | every workflow file parses | generic | `.github/workflows/` | none | 2 / 0.4 |
| 81 | every issue form renders | every issue form renders | generic | `.github/ISSUE_TEMPLATE/` | none | 1 / 1.4 |
| 113 | the shared scope/architect-label merge-base resolution | the merge-base resolution's selftest | generic | `helpers/scope-merge-base.sh` | none | 0 / 0.1 |
| 118 | shellcheck | shellcheck (its commands run under section 127) | generic | none | none | 0 / 0.0 |
| 127 | spelling | typos, then shellcheck over the scripts | generic | `_typos.toml`, `script/*`, `helpers/*.sh` | none | 10 / 13.9 |
