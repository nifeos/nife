---
status: BUILT
built: 2026-11-15
raised: 2026-11-15
---
# 1116. nife's basalt pin follows basalt's newest Release

The rule in force since 2026-11-15 (UTC): a workflow opens a daily pull request that moves
`xtask/basalt-pin`. That file names the basalt commit and Release the gate takes its programs from
(milestone 1115 (the gate builds basalt's delivered programs from the candidate tree)). The branch
is `milestone/1116-basalt-pin-follows-release`.

What drives it: the four merge-group rows run the recipes and build script at the pinned basalt
commit. Only a person moved the pin, so it fell a month behind, and a recipe basalt changed did not
reach the gate.

Reuse: basalt's own `pin-bump.yml` has the shape (one branch, force-pushed on each run, one open
pull request, the automation App's token), in the other direction. `vendor-watch.yml` and
`verify-shard-refresh.yml` supply the fallback for a repository without the App.

## What is in force

- `.github/workflows/pin-basalt.yml` runs daily and on dispatch. When `helpers/basalt_pin.py
  resolve` names a Release other than the one the pin holds, the workflow writes both lines of
  `xtask/basalt-pin` (commit and release), pushes the branch `pin/basalt` with force, and opens the pull
  request as `nife-smelter[bot]` or updates the one that is open. When the pin already names that
  Release, the run says nothing.
- The target is the newest Release of basalt, by publication time, that passes three tests. basalt
  has published it, as neither a draft nor a pre-release. It holds, for every package nife builds,
  a `.nifepkg` for each of the three architectures and a source archive, all fully uploaded. Its
  commit sits on basalt's `main`: a compare of `main...<commit>` says `identical` or `behind`. The
  workflow skips a newer Release that fails a test, and the pin never moves backward.
- `helpers/basalt_pin.py` holds the rule. Its `--selftest` tests the rule with no network, and
  `script/lint` runs it. Only `resolve` reads GitHub.
- The workflow does not arm auto-merge. A maintainer arms the pull request with `helpers/arm.sh`
  until an architect rules on auto-merge for this pin. The pull request runs the normal gates.

## Follow-on

- **Recorded.** The workflow cannot run before it lands on `main`, because the automation App's
  secrets belong to `main` alone, so its first scheduled or dispatched run is its first test of the
  push and pull-request steps: `.github/workflows/pin-basalt.yml`, header.
- **Recorded.** `REQUIRED_PACKAGES` in `helpers/basalt_pin.py` repeats the package list
  `xtask/src/basalt.rs` builds: `helpers/basalt_pin.py`, `BUGS`.

## Index row

nife's `xtask/basalt-pin` follows basalt's newest complete Release on basalt's main by a daily pull request that a workflow opens and a maintainer arms, so a recipe basalt changes reaches the gate within a day.
