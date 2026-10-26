---
status: PROPOSED
raised: 2026-10-26
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A nightly builds where no write token can be read

Raised by the 2026-10-26 security audit
([its report](../../audit-reports/2026-10-26-the-compiler.md), finding 1). Name: provisional.

Reuse: the `restamp` job's shape in `toolchain-bump.yml`, which already builds under a new nightly
with `contents: read` and hands its results to `propose` as an artifact.

## The failure

The `propose` job in `.github/workflows/toolchain-bump.yml` mints the automation App's installation
token in its first step. The token writes contents and pull requests. The job then checks the tree
out with it. `actions/checkout` persists the token for later git commands in the job by default, so every
later step can read it. One of those steps is `cargo xtask std-exerciser`, which runs code nobody
here has read. It runs a nightly that rustup installed minutes earlier. It also runs the build
scripts in `xtask`'s and `std_exerciser`'s registry graphs, `rayon-core` and `crossbeam-utils`
among them.

With that token, a compromised build step can push to any branch and open a pull request that
triggers CI. The ruleset on `main` requires no approving review, and this job arms auto-merge on its
own branch, so a commit pushed to `toolchain/nightly-bump` merges once CI is green. A compiler
already owns what it compiles. The token adds commits on `main` beyond the build's output, and
nobody reads them.

The two other jobs that check out with the App token (`falsifications.yml`'s `record-times` and
`mutation.yml`'s `record`) run only this tree's scripts over artifacts, and no third-party code.
They are outside this failure. A fix here should not make them worse.

## The fix

Build where no write credential exists, and push from a job that builds nothing:

1. `propose` does its overlay build in a job with `contents: read`, no environment and no App
   token, the way `restamp` already does, and uploads the build's outcome and log.
2. A small job that names `automation` downloads those, checks out with
   `persist-credentials: false`, mints the token, and hands it only to the step that pushes and
   calls `gh`. It runs no cargo, no rustup and no tree script beyond the commit itself.
3. `propose-kani` gets the same `persist-credentials: false`; `script/kani-bump` reads a release
   list and compiles nothing, but nothing should be able to read the token.

## Exit test

On `main`, a scheduled or dispatched run opens or updates the bump pull request as before, and the
job log shows the build job with no `automation` deployment and no token step. A probe step,
removed after that run, in the build job prints whether `git config --get-regexp
'http.*extraheader'` and the checkout's credentials file are empty.
