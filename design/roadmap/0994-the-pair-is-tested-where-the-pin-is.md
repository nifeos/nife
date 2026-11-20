---
status: BUILT
built: 2026-11-19
promoted_from: the-pair-is-tested-where-the-pin-is
raised: 2026-10-30
milestone_dependencies: 1115
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 994. The cross-repository pair is tested where the pin is

The rule in force since 2026-11-19 (UTC): basalt's `packages` workflow runs nife's `rg-threads`
rows on the packages it built at its pin, beside the `swish-check` rows it already ran. nife keeps
its own four rows as milestone 1115 (the gate builds basalt's delivered programs from the candidate
tree) built them. The branch is `994-pair-tested-where-pin-is`.

What drives it: nife's merge group runs basalt's programs built from the candidate tree, so no gate
ran `rg` walking in parallel at four cores, or the unmodified `ioping`, `hackbench` and `sqlite`,
on the bytes basalt publishes. Those bytes link the nife commit basalt pins. A program that works
built from nife's tree and fails as published went unseen.

Reuse: basalt's `install` job already checks out nife at the pinned commit and boots its rows with
`NIFE_BASALT_REPOSITORY`. The two legs it gains run `script/ci-build` rows the pinned nife already
has. No new script and no new dependency.

## What is in force

Each repository tests the pair it owns.

| gate | programs | nife | rows |
|---|---|---|---|
| nife's merge group and a push to `main` | built from the candidate tree (`cargo xtask basalt-build`) | the candidate | `swish-check-*`, `rg-threads-*` |
| nife's pull request run, `full-ci` included | fetched from the Release `xtask/basalt-pin` names | the candidate | the same |
| basalt's `packages` workflow | built at basalt's pin, the bytes `publish` serves | the commit `pins.toml` names | `swish-check-*`, `rg-threads-aarch64-riscv64`, `rg-threads-x86_64` |

- basalt's two `rg-threads` legs are named `jig installs them (rg-threads-aarch64-riscv64)` and
  `jig installs them (rg-threads-x86_64)`. They run aarch64 and riscv64 on the arm64 pool and
  x86_64 on an x86_64 runner under KVM, as the `swish-check` legs do. They are not required
  checks; `publish` needs the `install` job, so a failing leg holds the Release.
- A change to a wire format a delivered program reads lands in one nife pull request. Its merge
  group rebuilds the programs from the candidate tree, and basalt rebuilds them at its next pin
  bump. The pin still never points at an unmerged branch.
- The argument page's magic follows that rule. `program_argument_contract`'s `nifeargv` gives way
  to a numbered magic that does not spell nife in one nife pull request, with no reader-first
  rollout, as milestone 1106 (`program_argument_contract`: what an outside maintainer would fix
  before it leaves) records. That change is a lane of its own.

## Alternatives considered

Moving the `rg`-dependent rows out of nife, so nife fetches no basalt program at all: refused once
milestone 1115 built the candidate-tree rows. It would take the threading row out of the gate that
sees each change first, and the defect it answered is gone.

A reader-first rollout alone: it works, and it makes a discipline load-bearing that the candidate
build makes unnecessary.

Building `rg` in nife's CI from a recipe of nife's own: refused. It duplicates basalt's recipe,
which then drifts from the one basalt publishes. 1115 runs basalt's own build script at the pinned
basalt commit instead.

## Done when

- basalt's `install` job lists the two `rg-threads` legs, and they pass on the pull request that
  adds them: nifeos/basalt#83.
- This block, milestone 1106 and milestone 1017 (the contract crates leave in this order) state
  the rule above.

## Follow-on

- **Done.** nifeos/basalt#83 adds the two `rg-threads` legs to basalt's `install` job.
- **Recorded.** The two legs are not required checks, so a basalt pull request can merge with them
  red and only `publish` stops; basalt's `CONTRIBUTING.md` says so. Adding them to basalt's ruleset
  is a settings change for a person.
- **Milestone 1106.** The numbered magic for `program_argument_contract` is a lane of its own, under
  that block's Follow-on.

## Index row

nife's merge group tests the programs built from its candidate tree, and basalt's gate tests the
published bytes with nife's `swish-check` and `rg-threads` rows at its pin, so each repository tests
the pair it owns.
