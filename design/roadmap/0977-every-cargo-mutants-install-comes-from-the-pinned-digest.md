---
status: NOT-STARTED
promoted_from: every-cargo-mutants-install-comes-from-the-pinned-digest
raised: 2026-10-28
milestone_dependencies: 955
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 977. Every cargo-mutants install in CI comes from the pinned digest

*(Title and slug provisional.)*

Reuse: the mechanism `lane/955-mutants-install-fix` built in
`.github/workflows/kernel-mutants-diff.yml` on 2026-10-28 (PR #2860). That is the version pin
`.cargo-mutants-version`, the digest pin `.cargo-mutants-sha256` beside it, and the
download-and-check step that reads both. The work here is four copies of that step and four
deletions. Nothing here needs new design.

`lane/955-mutants-install-fix` raised this on 2026-10-28 (UTC). While fixing the diff-scoped
check's install, the lane found the same defect in four more workflows. That check is part 3 of
milestone 955 (mutation testing reaches the kernel and its programs under QEMU, a diff and a
slice at a time).

## The finding

cargo-mutants publishes no aarch64 Linux release asset. Every upstream release ships x86_64
darwin, x86_64 windows and x86_64 linux only (the Releases API across all tags, read
2026-10-28). `taiki-e/install-action` at the tree's pin (v2.87.31) has no aarch64_linux entry for
the pinned 27.1.0. On `ubuntu-24.04-arm` each of its cargo-mutants steps therefore warns that the
version has no aarch64_linux entry, then falls back to cargo-binstall.

The fallback installs `cargo-binstall@latest`, a version nobody pins. It then takes a QuickInstall
build of the crate, and those bytes nobody pins either. The step still exits green, so nothing
surfaces this. Every arm job that mutates depends on an unpinned tool from a third-party rebuild
that can change or vanish with no gate noticing. The diff-scoped check's lane replaced its own
step with a checksummed direct download. The four sites below still carry the fallback.

## Where it stands

`git grep` on 2026-10-28 finds one cargo-mutants install site per workflow, four in all, each in a
job on `ubuntu-24.04-arm`:

| workflow | what the installing job does |
|---|---|
| `mutation-gate.yml` | the required per-pull-request mutation gate |
| `mutation.yml` | the weekly host census and its reports |
| `kernel-mutants-cost.yml` | the part 1 cost measurement of milestone 955 |
| `kani-reach.yml` | the proof-reachability reports |

`ci.yml` and `metrics.yml` install other tools (cargo-deny, cargo-llvm-cov) through the same
action. This proposal claims only cargo-mutants and does not measure the others.

## What done means

- No workflow installs cargo-mutants through `taiki-e/install-action` or any other unpinned path.
- All four sites carry the download-and-check step the diff-scoped check runs, reading the same
  two pin files. One version bump and one digest re-pin then move every site together.
- The digest pin's comments still name the URL and the verification method. The lane that bumps
  the version knows to verify and re-digest.

## Plan

1. `mutation-gate.yml` first, because it is the required check. Copy the step in one pull request;
   the gate's own next run proves it.
2. The other three workflows, one pull request each. Each workflow's next scheduled or dispatched
   run proves its copy.
3. A bump drill on a scratch branch: raise the version and the digest, and confirm all four sites
   take the new build. The procedure then exists in writing before the first real bump needs it.

## Risks

- QuickInstall is a third party. The pinned digest catches changed bytes, and the job fails
  loudly; a missing artifact fails the download. If QuickInstall stops publishing the build, the
  recovery is `cargo install --locked` from the crates.io source. That pins the source but not
  the toolchain's bytes, so the trade is an architect's call if it ever comes. The digest pin's
  comments are where that decision goes.
- A version bump that forgets the digest fails all four sites at once. The design wants that: a
  loud failure over a quiet drift. The bump drill keeps it from surprising anyone.

## Index row

Four workflows install cargo-mutants on arm runners through an unpinned fallback
(`cargo-binstall@latest` plus third-party bytes), because cargo-mutants publishes no aarch64
Linux release. Proposed: every site copies the diff-scoped check's checksummed download and reads
the one version pin and the one digest pin.
