---
status: BUILT
built: 2026-11-15
raised: 2026-11-15
---
# 1116. nife's basalt pin follows basalt's newest Release

The rule in force since 2026-11-15 (UTC): `xtask/basalt-pin`, the file that names the basalt commit
and Release the gate takes its programs from (milestone 1115 (the gate builds basalt's delivered
programs from the candidate tree)), is moved daily by a pull request that a workflow opens. The
branch is `milestone/1116-basalt-pin-follows-release`.

What drives it: the four merge-group rows run the recipes and build script at the pinned basalt
commit, and the pin was a month behind because only a person moved it. A recipe basalt changed did
not reach the gate.

Reuse: basalt's own `pin-bump.yml` is the shape (one branch, force-pushed on each run, one open
pull request, the automation App's token), in the other direction. `vendor-watch.yml` and
`verify-shard-refresh.yml` supply the App-optional fallback.

## What is in force

- `.github/workflows/pin-basalt.yml` runs daily and on dispatch, as `nife-smelter[bot]`. When
  `helpers/basalt_pin.py resolve` names a Release other than the one the pin holds, it writes both
  lines of `xtask/basalt-pin` (commit and release), force-pushes the branch `pin/basalt`, and opens
  the pull request or updates the one that is open. When the pin already names that Release, the run
  is silent.
- The target is the newest Release of basalt, by publication time, that is published (not a draft
  or a pre-release), complete (every package nife builds has a `.nifepkg` for each of the three
  architectures and its source archive, fully uploaded), and cut at a commit on basalt's `main`
  (a compare of `main...<commit>` is `identical` or `behind`). A newer Release that fails a test is
  skipped. The pin never moves backward.
- `helpers/basalt_pin.py` holds the rule, and `--selftest` tests it with no network, run by
  `script/lint`. `resolve` is the only part that reads GitHub.
- The workflow does not arm auto-merge. A maintainer arms the pull request with `helpers/arm.sh`
  until an architect rules on auto-merge for this pin. The pull request runs the normal gates.

## Follow-on

- **Recorded.** The workflow cannot run before it is on `main`, because the automation App's
  secrets are scoped to `main`, so its first scheduled or dispatched run is its first test of the
  push and pull-request steps: `.github/workflows/pin-basalt.yml`, header.
- **Recorded.** `REQUIRED_PACKAGES` in `helpers/basalt_pin.py` repeats the package list
  `xtask/src/basalt.rs` builds: `helpers/basalt_pin.py`, `BUGS`.

## Index row

nife's `xtask/basalt-pin` follows basalt's newest complete Release on basalt's main by a daily pull request that a workflow opens and a maintainer arms, so a recipe basalt changes reaches the gate within a day.
