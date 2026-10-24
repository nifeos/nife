# Security: what an owned runner exposes, and the settings that narrow it

An appendix to
[gating-ci-rents-more-concurrency-before-it-owns-a-runner.md](../gating-ci-rents-more-concurrency-before-it-owns-a-runner.md),
which keeps the threat model and the preconditions. This file holds GitHub's own words, what this
repository already sets, what a job could reach, the stronger VM shape, and the pull-request
settings behind option (e). Everything was read on 2026-10-24 (UTC).

## What GitHub says

From GitHub's secure-use reference: self-hosted runners "should almost never be used for public
repositories on GitHub", because "any user can open pull requests against the repository and
compromise the environment." They "can be persistently compromised by untrusted code in a
workflow." Just-in-time runners "perform at most one job before being automatically removed", yet
"there is no way to guarantee that a self-hosted runner only runs one job", and "re-using hardware
to host JIT runners can risk exposing information from the environment."

From the runner-group page: Team organizations may create runner groups, restricted to chosen
repositories. "By default, only private repositories can access runners in a runner group, but you
can override this." So a public repository's runner group is an override an owner sets on purpose.

## What this repository already sets

| setting | value | read with |
|---|---|---|
| fork pull request workflows | approval required from `all_external_contributors` | `gh api repos/nifeos/nife/actions/permissions/fork-pr-contributor-approval` |
| default `GITHUB_TOKEN` | `read`, cannot approve reviews | `.../actions/permissions/workflow` |
| actions pinned by SHA | required | `.../actions/permissions` |
| pull requests | enabled, `pull_request_creation_policy: all` | `gh api repos/nifeos/nife` |
| forking | allowed; one fork exists, the maintainer's own | the same, and `.../forks` |
| interaction limits | none set on the repository | `.../interaction-limits` |

In the tree: no workflow triggers on `pull_request_target`, and three label bots say in their
headers why they refuse it. No workflow triggers on `issues` or `issue_comment`, so a stranger's
comment runs nothing. No gating workflow reads a secret; the App key appears only in scheduled and
cadence workflows. The gating workflows use two third-party actions, `Swatinem/rust-cache` and
`taiki-e/install-action`, both pinned by SHA. The last 1,000 pull requests came from no fork, and
the last 100 issues were opened by the maintainer or `nife-smelter[bot]`.

## What a job could reach

Inside any job, hosted or owned: its disk and memory, a read-only token for a public repository,
the Actions cache for its own ref, and whatever egress allows. A malicious commit can make its own
checks green on hosted runners too, because it carries the workflow file.

What an owned runner adds:

- Persistence. A reused runner lets one job falsify a later job's result for another branch. That
  is the gate's integrity, and one job per registration on a clean account or container is what
  prevents it.
- A shared local cache. A writable cache that two jobs both read crosses the ref scoping GitHub's
  cache enforces.
- The host. An escape from the runner's account or container lands on what the host holds: lane
  worktrees, and the maintainer credential that pushes and merges.
- The network. Without an egress rule, a job on a home connection reaches the maintainer's LAN,
  including the bench's power and serial controls, from the maintainer's own address.

## The stronger shape: a VM per job

When the threat model includes code from people who are not the maintainer, the lighter
preconditions stop being enough, and these take their place:

1. A fresh virtual machine per job from a pinned image, registered just in time and destroyed
   after one job. Gating jobs need `/dev/kvm` for `swish-check-x86_64`, so the VM needs nested
   virtualization. A microVM (Firecracker or Cloud Hypervisor, recalled, not read) or a plain KVM
   guest serves.
2. No maintainer credential on the hypervisor host: a host of its own, or the lane work moved into
   a VM of its own.
3. Its own network segment, with no LAN route and outbound denied by default.
4. A launcher selftest that refuses a VM configuration with a host mount, a LAN route or a reused
   disk.

## The outsider pass, as a pattern to copy

`helpers/launch_pass.py` runs an outside reviewer in rootless podman inside a Linux VM. The
container gets the clone, a results directory and one volume, and "nothing else of the host: no
home directory, SSH key, `gh` token or MCP configuration." Its selftest fails if a `podman run`
carries `--privileged`, `--network`, `--volume`, `--cap-add` or eight other flags. That refusal is
rung two of `AGENTS.md`'s ladder, and a runner launcher should carry the same kind. The pass does
not limit egress and keeps its volume across runs; a runner needs both.

## Pull-request settings: option (e)

GitHub's REST reference for updating a repository lists two settings, and `gh api repos/nifeos/nife`
returns both:

- `has_pull_requests`: "Either true to allow pull requests for this repository or false to prevent
  pull requests." Off would end nife's own workflow, since every lane lands through a pull request
  and the merge queue. It is not an option here.
- `pull_request_creation_policy`: "all or collaborators_only." nife reads `all`. GitHub's page on
  disabling pull requests calls `Collaborators only` a way to "restrict contributions", and defines
  a collaborator in an organization repository as a user with the write, maintain or admin role.

It is a per-repository setting, so other nifeos repositories keep pull requests open. The org-wide
policies could not be read: they need the `admin:org` scope, which this token lacks.

What `collaborators_only` blocks: a stranger opening a pull request. With no pull request, no
fork code reaches a workflow at all.

What it does not block, by GitHub's documentation or its silence:

- Forks. Anyone may still fork; `allow_forking` is separate. A fork with no pull request triggers
  nothing here.
- Issues and comments. The page does not mention them, and the reasonable reading is that they stay
  open. Nothing here triggers on either, so they run no code.
- Existing pull requests, which stay.
- A collaborator's own credential. A stolen token, or a misled agent holding the maintainer's, is
  unaffected.
- Unverified: whether `nife-smelter[bot]`, a GitHub App, counts as a collaborator. Seven workflows
  open pull requests as that App (toolchain bumps, metrics, vendor watch and others). A flip must be
  followed by one App-opened pull request, or the bots stop proposing silently.

The other candidates:

- Interaction limits restrict comments, issues and pull requests to collaborators, at repository or
  organization level, but expire after at most six months (recalled, not read today). A setting
  that lapses is rung zero.
- Disabling fork pull request workflows exists only for private repositories. The repository
  endpoint answers "Fork PR workflow settings is not allowed for public repositories."
- Approval for all external contributors is already set. It stops a fork's workflow until the
  maintainer approves, and one mistaken approval runs it.

Does (e) make routing pull-request jobs to an owned runner safe? It removes the stranger's pull
request by construction, which is the case GitHub's warning is about. The other four threats are
unchanged, and they apply to pull-request and merge-group runs alike, since lanes push and enqueue
their own work. It does not change the routing either. calef's ruling keeps every job reachable
from `pull_request` on hosted runners whatever the policy says, because a lint over the workflows
can check that rule and cannot check who opened a pull request. A reviewed outside change reaches
owned runners only after a member re-pushes its exact commits to a branch here.
