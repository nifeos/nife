---
status: BUILT
built: 2026-11-14
raised: 2026-11-14
---
# 1115. The gate builds basalt's delivered programs from the candidate tree

The rule in force since 2026-11-14 (UTC): the four merge-group rows that run the programs basalt
delivers build those programs from the candidate tree, for the architectures each row runs. They
use basalt's own `helpers/packages.py` at the basalt commit `xtask/basalt-pin` names, and hand the
result to the row with `NIFE_BASALT_REPOSITORY`. They build on every merge group and every push to
`main`, not only when the standard library's inputs change: while the pin is a month old, a
narrower trigger would build on every merge group anyway. The measurements and the options weighed
are in [the note](../../notes/gate-builds-delivered-programs-from-tree.md). The branch is
`milestone/1115-gate-builds-from-tree`.

Every program basalt delivers links nife's standard library, and the C programs link nife's C
library. A program built at an older nife commit reads that commit's wire formats. Before this
milestone the rows fetched programs built at the nife commit basalt pinned, so a nife change that
broke a format one of them read could not land in one pull request.

Reuse: basalt's own `helpers/packages.py`, at the pinned commit, is the build; the gate adds only a
wrapper that gives it a build root of its own. No other build of these recipes exists.

## What is in force

| row | builds | runner |
|---|---|---|
| `swish-check` | aarch64, riscv64 | hosted arm64 |
| `swish-check-x86_64` | x86_64 | owned x86_64 when `NIFE_RUNNER_OWNED` is set, else hosted |
| `rg-threads` | aarch64, riscv64 | hosted arm64 |
| `rg-threads-x86_64` | x86_64 | hosted arm64 |

- `cargo xtask basalt-build <architecture>...` (`xtask/src/basalt.rs`, `build`) checks out basalt
  at the pinned commit into a scratch directory and builds `ripgrep`, `ioping`, `hackbench`,
  `sqlite` and `demos` from this checkout's committed tree into `target/basalt-built`.
- The build root is a directory of its own per run, so two builds on one host do not collide.
- basalt's digest check fails every package built from a tree other than the one basalt pins. A run
  that exits 1 counts as built when its fresh index lists every package asked for, because the
  script writes the index only after it packs every package.
- A pull request's run, `full-ci` included, fetches the published programs as before.
- basalt's `packages` workflow keeps testing the published bytes, and runs nife's `swish-check`
  rows on them, at every pin bump.

Only the merge queue proves the workflow: no pull request run takes this path.

## Follow-on

- **Recorded.** A basalt flag that skips the digest check, so the gate need not infer success from
  exit status 1: `xtask/src/basalt.rs`, `BUGS`.
- **Recorded.** `xtask/basalt-pin` does not follow basalt's Release, so a recipe basalt changes
  reaches the gate only when someone moves the pin: `xtask/src/basalt.rs`, `BUGS`.
- **Recorded.** A pull request's `full-ci` run and a local `script/ci-build` still fetch, so a
  wire-format break is red there and green in its merge group: `xtask/src/basalt.rs`, `BUGS`.

## Index row

The four merge-group rows that run basalt's programs build them from the candidate tree with
basalt's own build at the pinned basalt commit, so a wire-format break lands in one pull request.
