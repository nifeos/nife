---
status: PARTIAL
raised: 2026-10-11
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 887. Crate repositories share one CI template, adapted from nife's gates

calef, 2026-10-11 (UTC): *"As a gate for each of these milestones, I want to ensure that we're
exposing clean crates without nife assumptions. I also want to ensure that the gates that we have
for nife will be adapted for these crate repos as well."* Approving this block's plan:
*"Yes. Don't we want proofs and falsifications for our crates?"*

857's Fork 1 already ruled the shape on 2026-10-07: one repository per crate under `nifeos`. Its
words: *"Every crate repository uses one shared CI template (Kani, lint, release) through reusable
workflows."*

Fork 4 ruled the proof job (`kani-github-action`), plus a job that applies each falsification
patch and expects Kani to fail. It also ruled that the release refuses to publish unless the
proofs are green. Fork 6 ruled trusted publishing from a `nifeos` workflow. Until this block, no milestone
built any of it, and every per-crate block from 813 to 821 said so in its `BUGS`.

## What this builds

1. A template repository, `nifeos/crate-workflows`, holding reusable workflows
   (`on: workflow_call`), one per row group below: `check`, `proofs`, `weekly` and `release`. A
   crate repository's own workflow files are a few lines each that call them, pinned by commit SHA,
   as nife pins every action today.
2. Versioned releases of the template (`v1`, `v2`, ...), each a tag on one commit.
3. A lag check, in the template itself so no crate repository can forget it. Every reusable
   workflow's first job compares the SHA it was called at (`job_workflow_sha`) with the template's
   newest release and fails when it is older. A weekly schedule in each crate repository runs the
   same check with no pull request open, and Dependabot's `github-actions` ecosystem opens the bump.
4. The release machinery 857 ruled and nobody owns yet. In nife, a lint makes `publish = false`
   the default and allows publishing only for crates on calef's allowlist (Fork 3). The list is
   one file, provisionally `crates/publish-allowlist.txt`, which `script/metrics` already reads. Outside it, the
   `nifeos` crates.io team and the trusted publisher configuration (Fork 6). The `release`
   workflow publishes only from a tag, only after `check` and `proofs` pass on that commit, and only
   through a GitHub environment calef approves.
5. Per-repository CI time. `helpers/ci_job_times.py` takes a repository argument and reads each
   crate repository too, so a slow template job shows up in nife's metrics.

## Repository configuration

calef, 2026-10-11 (UTC): *"nife is MIT OR Apache-2.0, so we should dual license our crates as well.
I agree with your recommendations on the crate repos."* The recommendations he agreed to are below.
They live in the template as one settings file and a script that applies it with `gh api`. A
weekly drift check in each crate repository reads the live settings back and fails on any
difference. A setting changed by hand in the web interface is then a red run, not a surprise.

- Features: issues on, with issue forms for a bug report and a feature request and blank issues
  off. Wiki, projects, discussions and sponsorships off; discussions turn on per crate only if
  questions crowd the issues.
- Merging: merge commits only, no merge queue at this traffic, branches required up to date
  (strict status checks), and auto-merge, update branch and delete-on-merge all on.
- A ruleset on `main`: no deletion, no force push, a pull request required with conversations
  resolved and zero approvals. The required checks are the template's jobs: fmt, clippy, tests,
  Kani, falsification replay, coverage, `crate-standalone`, supply chain and the architect hold.
  calef agreed to keep `needs-architect` as a required hold, so a semver-breaking API change waits
  for him: *"We should keep the needs-architect hold on the crates."* A hold nobody can trigger
  holds nothing. The template's label workflow therefore adds the label itself, on two rules: a
  dependency new to the crate's graph, and a breaking change that `cargo-semver-checks` reports
  against the last published version.
- A tag ruleset on `v*`, in each repository: no tag is updated or deleted, and creating one is
  refused to everyone except the `nife-smelter` app, the only bypass actor, which the release
  workflow's tag job authenticates as. A hand-made `v*` tag is refused. The template's
  `script/apply-settings` applies it from the settings file.
- A `crates-io` environment with calef as required reviewer, and the trusted publisher bound to
  it. Fork 6's approval is then enforced by GitHub rather than remembered.
- Security: private vulnerability reporting and a `SECURITY.md`, secret scanning with push
  protection, and Dependabot alerts, security updates and version updates for `cargo` and
  `github-actions`.
- Actions: only SHA-pinned actions from GitHub, verified creators and an allowlist naming
  `taiki-e/install-action@*`, the template's one third-party action (named in
  `settings/repository-settings.json`). The default `GITHUB_TOKEN` is read-only, and the release job alone gets
  `id-token: write`. A fork's pull request runs no workflow until a maintainer approves it, for
  every outside contributor and every time. That is the `all_external_contributors` approval
  policy nife has had since milestone 588 (a merge needs a review no fork can supply), not
  GitHub's default of first-time contributors only. calef asked for it here on 2026-10-12 (UTC).
  The settings script sets it on each repository rather than trusting an organization default,
  and the drift check reads it back. Read from the API on 2026-10-17 (UTC): the policy is
  `all_external_contributors` on `crate-workflows`, `crate-workflows-test`, `nife` and `basalt`.
  calef set it for the whole organization.
- Files: a README, `LICENSE-MIT` and `LICENSE-APACHE` with `license = "MIT OR Apache-2.0"`,
  `SECURITY.md`, `CHANGELOG.md`, and `CONTRIBUTING.md`, which says our work is disclosed as
  agent-written (Fork 8) and outside contributions are welcome. Also `CODEOWNERS` naming the
  `nifeos` team, the issue forms, a pull request template, and a label set synced from the
  template, `needs-architect` among it.
- Access: the crates team with Maintain, calef as admin, the `nife-smelter` app with Write.
- Metadata: a one-line description, docs.rs as the homepage, and the topics `rust`, `no-std`,
  `formal-verification`, `kani` and the crate's domain.

Both license files, because GitHub's license detection reads nife itself as Apache-2.0 although the
tree carries both. `script/crate-standalone` checks the pair and the field.

## The names: ruled

calef ruled the names on 2026-10-17 (UTC):

| Name | His reply |
|---|---|
| The repository `nifeos/crate-workflows` | *"Keep"* |
| The canary repository `nifeos/crate-workflows-test`, over `crate-workflows-canary` | *"Keep test"* |
| The seven callable workflow filenames: check, proofs, weekly, release, hold, label and ruled | *"Keep"* |
| The team `crates-io` becomes `crate-maintainers`; its crates.io owner string is `github:nifeos:crate-maintainers` | *"crate-maintainers"* |
| The required checks: `check / fmt`, `check / clippy`, `check / tests`, `check / coverage`, `check / standalone`, `check / supply-chain`, `proofs / kani`, `proofs / falsify` and `hold / architect hold` | *"Per your suggestion"* |

The `crates-io` environment keeps its name. Template release `v6` (`d5ad709`, nifeos/crate-workflows
#9) carries the check names, and the canary's `main` ruleset requires all nine. The team
`crate-maintainers` exists. No `Name:` block in nife carries these names, so `script/names` has
nothing to ratify.

## The first publish: ruled

crates.io binds a trusted publisher only to a crate that already exists. calef, 2026-10-17 (UTC),
answered *"B"* to this option for how the first publish happens:

> B. The release workflow does the first publish with a scoped token. A crates.io token limited to
> `publish-new`, restricted to that crate's name and given a short expiry, is stored as a secret on
> the `crates-io` environment. The workflow uses it only when the crate doesn't exist on crates.io
> yet. So v1 goes through the same proofs-green gate and your environment approval as every later
> release. After v1 you bind the trusted publisher and delete the secret.

He declined publishing v1 by hand. The `release` workflow decides by the crate's
presence on crates.io, never by the secret's: an existing crate publishes through trusted
publishing only, with no fallback to the token. A missing secret on an absent crate fails naming
it. Built in nifeos/crate-workflows #8 and released in `v6`. The secret is
`CRATES_IO_FIRST_PUBLISH_TOKEN`, a provisional name.

A first publish waits on its soak (milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic, released on its own) is the pilot).

## Each nife gate, kept, adapted or dropped

Read 2026-10-11 from `script/ci-build`, `script/lint`, `.github/workflows/` and their helpers. A
crate repository is a `no_std` library with host tests and Kani proofs, and no kernel, QEMU,
roadmap or merge queue; each row applies that test.

| nife gate | crate repository | reason |
|---|---|---|
| `script/fmt` (rustfmt) | adapt | Stable rustfmt, without nife's two unstable options |
| clippy `-D warnings` | adapt | Host, each listed target and a `--cfg kani` build, at the stated MSRV and at the newest stable |
| rustdoc broken links | keep | The docs are the crate's manual |
| cargo-machete | keep | Downstream users pay for an unused dependency |
| host tests (`cargo test`) | keep | No QEMU leg; the crates are pure logic |
| coverage, 85% per file | keep | Same floor, same tool (cargo-llvm-cov) |
| fuzz (cargo-fuzz) | keep | For crates that parse bytes; nightly, so weekly |
| Miri, weekly | keep | Same job, nightly |
| mutation (cargo-mutants), weekly | keep | A report |
| `script/verify` (Kani) | adapt | Kani installed with `cargo install` at the version in `.kani-version` (0.68.0), run on every pull request (Fork 4) |
| falsification sweep | adapt | Every patch applied with `-p1` must turn its harness red at its own assertion; on every pull request, since a crate has few |
| `script/falsifications --check` | keep | Every harness has a patch or a stated reason |
| kani-reach | drop | A dispatch-only measurement of nife's reach |
| loom interleavings | drop | No crate uses loom yet |
| cargo-deny | adapt | nife's policy without its own exceptions |
| vendor-verify, vendor-watch | drop | No vendored code |
| toolchain drift and bump | adapt | A weekly stable and beta build replaces the nightly pin |
| MSRV build | new | The stated `rust-version` builds; nife has no MSRV to check |
| semver check | adapt | cargo-semver-checks against the last release stands in for milestone 610 (the interface's stability is measured weekly) |
| `script/crate-standalone` | adapt | Its citation and file checks run in the repository, keeping nife's vocabulary out |
| file-length ratchet (2,000 lines), comment-block ratchet | keep | Same rules |
| workspace lints (`unsafe_op_in_unsafe_fn`, `missing_docs`) | keep | Inlined into each crate's `[lints]` |
| orphan Rust files | keep | Every `.rs` file is in the module tree |
| prose ratchet | adapt | README and changelog, at nife's caps |
| house style, typos, no em-dashes, markdown links resolve | keep | Same checks |
| EXAMPLES and BUGS | new | The README must carry both headings |
| actions pinned to a SHA | keep | Supply chain, unchanged |
| a workflow that swallows a failure says where it goes | keep | Same rule |
| shellcheck, `sh -n`, tracked-but-ignored files | keep | Same checks |
| job budgets | adapt | Per repository, with time read by `helpers/ci_job_times.py` |
| CodeQL (Rust) | keep | Same job |
| commit-msg `Co-Authored-By` hook | keep | Agent attribution everywhere nife writes |
| empty diff | keep | Same check |
| architect label (`helpers/architect-label-rules.py`, `architect-label.yml`) | adapt | Keeps the new-dependency rule and adds a semver rule (below); drops the abi-surface, spawnproto, syscall-error and decisions rules |
| architect hold (`architect-hold.yml`) | keep | Red while `needs-architect` is on, and a required check |
| `script/record-ruling` | adapt | The same label swap, from a template script |
| name provenance blocks (`script/names`) | drop | A crate's name is ratified before first publish; `crate-standalone` refuses a `Name:` block |
| citations, decisions, roadmap, fatal risks, audits index, TODO cites a milestone | drop | Records that live in nife; a TODO cites an issue in the crate repository |
| naming checks 1 to 3, branch names, ready-status, script docs, env names in prose, agent instructions | drop | nife's own vocabulary and lane process |
| merge drain, trunk health, ci-failing, verify shard refresh | drop | No merge queue at one crate's traffic |
| audit and stranger cadence | adapt | A stranger builds and proves the crate from its README |
| kernel and image gates: boot, swish, icount, bench, cpu-matrix, stack frames, W^X, reproducible image, arch rules, syscall registers, `#[path]` sharing | drop | A library crate has no kernel, QEMU or image |

## Reuse

Taken, not written: GitHub's reusable workflows, `rust-lang/crates-io-auth-action` (RFC 3691),
Dependabot, `cargo-semver-checks` and `taiki-e/install-action`. Kani is the exception to taking an action: the template installs it with
`cargo install` at the version in `.kani-version`, so the Actions allowlist names no Kani action. A
weekly template job proposes the version bump as one pull request (crate-workflows #5). Prior art for
the shape: rust-embedded's and RustCrypto's shared workflows, read for structure rather than
copied. The workflows themselves are written, since none publishes a falsification job.

## Exit

A crate repository, `nifeos/crate-workflows-test`, calls every reusable workflow from releases v1
and v2. Built 2026-10-16 (UTC). The same day, calef ruled that it
stands as the template's release canary, adopting each release before a crate depends on it. It
has adopted release v6 (commit d5ad7091019cae8122b0c673fd790936039485ea) through its pull request
7, merged at d8c155c39c97a74b9fcc5e4492814bdc374ba4af on `main`, where all nine ruled check names
reported green. The template's own selftest is green on every job, plus the release gate's
refusal. The canary's earlier pull requests show fmt, tests, coverage, standalone and the falsification replay red on a commit made
to break them and green on the next. Others show the lag check naming a stale pin and the
supply-chain refusal of a floating action. A pull request held by `needs-architect` took a recorded ruling that swapped the label and turned the hold green. The apply script configures the canary
from the settings file and reports what it cannot do. It sets the features, the security toggles,
the Actions permissions, the `crates-io` environment with calef as required reviewer, the team,
the labels, and the `main` ruleset with the nine required checks. The drift check reads the
settings back. It went red naming a hand-changed setting. The tag ruleset is live on the canary
(ruleset 25290943), and `script/apply-settings` applies it from the settings file. The release gate passed on a green commit and created no tag under `dry-run`. The publish job then sat in the `crates-io`
environment waiting on a person, which is the "publishes nothing without calef's environment
approval" row. In nife, the allowlist lint fails a crate that drops `publish = false` without
being on the list, and `helpers/ci_job_times.py` reads a crate repository by argument.

## BUGS

- The table is a reading of nife's gates on 2026-10-11, not a ruling. A gate added to nife
  since is not in it, and nothing makes adding one ask "and the template?".
- The template's release cadence is provisional. The check names are measured against what GitHub
  shows for cross-repository reusable workflow calls.
- `needs_person` is `yes`. The first publish needs calef to create the scoped token on his
  crates.io account and, after v1, to bind the trusted publisher. Organization administration,
  which the lane's token does not carry, covers the rest.
- The canary's callers at v6 run `check` and `proofs` on pull requests only. A pull request's runs
  are recorded at its head, never at the merge commit a release dispatches at, so the release gate
  refuses every release (crate-workflows-test run 39711321512). nifeos/crate-workflows #12 adds a
  push trigger on `main` to the scaffold callers; it reaches the canary with the next release.
- The semver half of the labeler never fired against a real baseline: the test crate is
  unpublished, so `cargo-semver-checks` has nothing to compare. The first published crate is the test.
- The settings-drift check's security rows and ruleset read want an administration-scoped token;
  with `GITHUB_TOKEN` they report what was unreadable.

## Follow-on

- **Outstanding.** A real crates.io publish end to end. calef creates the scoped token
  (`publish-new`, one crate's name, a short expiry) as `CRATES_IO_FIRST_PUBLISH_TOKEN` on the
  `crates-io` environment, a template release carries #8, and v1 goes through the gate and the
  environment approval. Then calef binds the trusted publisher and deletes the secret. Not yet
  shown: that an environment secret reaches the called workflow's job under that name. Checked by
  the first publish; the dry-run rehearsal shows only which door the release would take.
- **Outstanding.** The labeler's semver rule firing against a published baseline. The test crate
  is unpublished, so only the no-baseline path has run; the false positive it produced is fixed in
  v2. The first published crate (milestones 813 to 821) is the test.
- **Outstanding.** The canary ritual's driver running live. `canary-adopt.yml` (nifeos/crate-workflows
  #10) opens the adoption pull request as `nife-smelter`, which holds contents, pull requests and
  workflows write (measured 2026-10-22 UTC). It needs the app's ID and private key as
  organization secrets visible to `crate-workflows` and `crate-workflows-test`, which calef
  creates; the template's weekly Kani bump and the canary's release `tag` job need the same pair. Checked by a dispatch for `v6` opening its pull request.
- **Outstanding.** The stranger cadence's first run, a person's as in nife; the weekly job fails
  until `docs/stranger-test.md` records one.
- **Outstanding.** The team `crates-io` is deleted by calef, leaving `crate-maintainers` the one
  crates team. Checked by `gh api orgs/nifeos/teams` listing it alone.

## Index row

One shared CI template for every crate repository nife releases, as reusable workflows adapted
gate by gate from nife's own. A check fails when a repository lags the template, and the template
carries the release machinery 857 ruled: the allowlist lint, the crates.io team and trusted
publishing.
