---
status: NOT-STARTED
raised: 2026-10-03
promoted_from: the-install-gates-run-nowhere
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 712. The install gates run nowhere, so rung 2a can rot without anybody hearing

*Title and slug are drafts.*
Raised 2026-10-03 by the lane briefed to build milestone 198 (a package manager, and the trivial
install) rung 2a. It found the rung already built and re-ran its gates instead (see the status
section of milestone 515 (the installer: a stick that puts itself on the machine's disk and is then
not needed)).

## The finding

`cargo xtask install-boot`, `rollback-boot` and `confirm-boot` are the only proof that a nife stick
installs itself and that an installed disk survives a bad upgrade. **No workflow, `script/test` leg
or `script/verify` row runs any of them** (`grep -rn install-boot .github script` finds nothing). They
passed on 2026-10-03 at `e613c520` because nothing between 2026-09-21 and that day broke them, not
because anything checked.

The reason recorded for leaving `install-boot` out of `script/test`'s default legs was cost:
*"The two boots take several minutes under TCG"* (`xtask/src/install.rs`, `BUGS`). Measured on
patagonia on 2026-10-03, warm: 42.5 seconds wall for the whole gate, build included. The
comment is corrected in the same change as this proposal.

## What the work is

Run the three gates on a schedule, not on every pull request: a nightly or weekly workflow in the
shape of `stranger-cadence.yml`, posting under `nife-smelter[bot]` when one fails. Per pull request
would add an x86_64 OVMF job to a CI that has already split one job for being slow (`ci.yml`,
beside milestone 628 (the x86_64 swish-check leg costs what the others do)). These gates also
guard a path few pull requests touch.

What it needs first is one CI measurement: the 42.5 seconds above is an Apple M-series host
emulating x86_64, and the arm64 runner pool may be slower. The workflow's first run is that number.

## BUGS

- A scheduled run finds a break up to a week late, and the culprit has to be bisected rather than
  read off the failing pull request. Accepted against the per-pull-request cost; revisit if a break
  is ever found that way.

## Index row

Nothing runs the three install gates on any schedule, and the cost recorded as the reason (several
minutes) measured 42.5 seconds warm on 2026-10-03; a scheduled workflow is the proposed fix.
