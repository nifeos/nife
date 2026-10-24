---
status: PROPOSED
raised: 2026-10-24
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: yes
---
# Gating CI rents more concurrency before it owns a runner

*(Title and slug provisional.)*

Reuse: GitHub's hosted runners, plan tiers and repository settings, unchanged. For the staged
self-hosted option, GitHub's just-in-time runner API, and the refusal-by-selftest pattern of
milestone 920 (an outsider pass is launched pinned and floor-checked). This proposal builds nothing
and changes no workflow or setting.

Raised 2026-10-24 (UTC) by lane `maintainer/proposal-self-hosted-gating-runners`, at calef's
request, during the run of milestone 929 (comments in the Linux style across first-party Rust).
About 40 pull requests were in flight, and hosted CI was the only thing they waited on. Two
appendices sit beside this file:
[measurement.md](gating-ci-rents-more-concurrency-before-it-owns-a-runner/measurement.md) and
[security.md](gating-ci-rents-more-concurrency-before-it-owns-a-runner/security.md).

## The claim and the decision asked

The claim: on 2026-10-24 the gate waited on GitHub's 60-job concurrency cap, not on the work. The
next rented tier raises that cap eightfold for $17 a month more. An owned runner adds capacity only
for x86_64 jobs. For a project with one maintainer, its security cost is small if forks never reach
it.

The decision: may jobs that gate a merge run on hardware the project owns, and if so, which jobs?
[§299 (the build machine is bought)](../../decisions/0299-the-build-machine-is-bought.md) leaves
exactly this open: "Whether anything that gates a merge runs on" the build machine.

- (a) Hosted runners only, with the cap raised by renting the Enterprise tier, as
  [§214 (the Team plan buys runner concurrency)](../../decisions/0214-the-team-plan-buys-runner-concurrency.md)
  did with Team.
- (b) Self-hosted runners for merge-group runs and pushes to `main` only.
- (c) Self-hosted runners for pull-request runs too.
- (d) A hybrid by job type, such as the QEMU-heavy shards self-hosted.
- (e) Limit who may open a pull request on nife to collaborators, which pairs with any of the
  others.

Recommendation: (a) now, with (e), and (b) as the staged next step under the preconditions below.
Refuse (c). Treat (d) as a variant of (b), because the QEMU-heavy shards are the jobs an owned
x86_64 host cannot take.

Why: the hosted tier is the far larger lever and adds no failure mode. (b) is cheap to make safe
under calef's fork ruling, but it buys little until the arm64 question below is answered. At equal
effort the order would be the same.

## The evidence, from the milestone-929 run

All times UTC. Maintainer snapshots of the Actions API counted 124 runs queued and 4 in progress
at 01:25, 151 and 6 at 03:15, and 249 and 3 at 03:32. A run is queued while any job waits, so jobs
are the sharper reading. At 03:36:13, 60 jobs were in progress, 20 on `ubuntu-24.04` and 40 on
`ubuntu-24.04-arm`. Another 430 waited. Sixty is exactly the Team cap.

- Saturation. From 00:00 to 03:30 the heavy workflows (`CI`, `verify`, `codeql`, `mutation gate`)
  ran a median of 59 jobs at once. They used 12,335 of the 12,600 job-minutes that 60 slots give,
  98%, with the label bots on top.
- Waiting. On 10-24 from 00:00 to 03:36, a job's median wait for a runner was 5 to 13 minutes per
  hour, p90 14 to 23. On the afternoon of 10-23 it was under 0.1 minutes.
- Wall clock. A full `CI` run is 26 jobs and about 128 job-minutes. Uncontended, a merge-group run
  took a median 14 minutes. Under load it took 38, and a pull request's full run took 46.
- Merges. `main` took 14 merges from 00:00 to 03:36 on 10-24, one per 15 minutes: 12 between 01:06
  and 01:55, then none for 101 minutes. The queue head, #2643, had awaited checks since 01:59:29.
- Repeat cost. Since 10-23 00:00, 53 branches spent 149 full `CI` runs, a median 3 each; 53 were
  cancelled by a later push. Thirty pull requests needed 46 merge-group runs.

The owned host doing the lane work was not the limit:
[`briefs/run-a-milestone-wide.md`](../../../briefs/run-a-milestone-wide.md) records its CPUs about
85% idle, because "CI runs off the host".

§214 set the condition for this question: "Reconsider a self-hosted runner only if 60 concurrent
jobs still binds." It binds.

## The options

### (a) Hosted only, with the cap raised

GitHub's limits reference (read 2026-10-24) gives standard hosted runners 60 concurrent jobs on
Team and 500 on Enterprise. Public repositories pay nothing for standard minutes on either (§214).
The pricing page lists Enterprise at $21 per user per month for the first 12 months, against Team's
$4: $17 a month more for one seat, with the later price unstated.

At 03:36 demand was about 490 jobs. A 500-job cap would have run nearly all of it at once, putting
each full run back near 14 minutes; the queue's five-entry build bound would then decide
throughput. Unsettled: whether GitHub's arm64 pool supplies hundreds of public-repository jobs on
demand. Milestone 587 (most CI jobs do not need an arm64 host) found the queue waited there, so the
week of measurement watches that label first. Larger hosted runners lose for now: billed per minute
(recalled), against Actions budgets capped at $0 (§214).

### (b) Self-hosted, merge groups and pushes to `main`

The merge group is the critical path, so an owned runner helps most there per job. Its reach is
small, though. Of a full run's 128 job-minutes, 84 are on `ubuntu-24.04-arm` (9 jobs) and 44 on
`ubuntu-24.04` (17 jobs). The owned host is x86_64 (§299), where aarch64 and riscv64 run fully
emulated.

Milestone 587 kept the build-and-test shards, the CPU matrix and re-falsify on arm64, on the claim
that an arm64 host lets the SMP guest legs exhibit weak-ordering reorderings. That claim was never
measured. #2668 measures it now: whether an arm64 CI host catches a missing Release that an x86_64
host misses. If it does not, those jobs can move to x86_64, and up to the whole 128 job-minutes
becomes owned-runner work. Until #2668 reports, (b) takes only x86_64 jobs, about 7% of the
window's demand, and frees hosted slots for arm64.

### (c) Self-hosted for pull requests

The largest win an owned host could give: pull-request runs were 7,358 of the window's 12,335
job-minutes. Refused. calef's fork ruling is enforced most simply when no job reachable from
`pull_request` may use an owned label at all, and (c) would replace that rule with one about whose
branch a run came from.

### (d) Hybrid by job type

The QEMU-heavy shards are the arm64 jobs above. What remains is x86_64 work: `verify`'s prove
shards (49 of its 52 job-minutes), `lint`, `fuzz`, `bench` and `swish-check-x86_64`. Sending those
for merge groups is (b).

### (e) Pull requests from collaborators only

`gh api repos/nifeos/nife` returns `pull_request_creation_policy: all`. GitHub's documented values
are `all` and `collaborators_only`, per repository, so other nifeos repositories keep pull requests
open. Turning pull requests off (`has_pull_requests: false`) would end nife's own workflow, since
every lane lands through a pull request and the merge queue.

`collaborators_only` stops a stranger from opening a pull request, so no stranger's code reaches a
workflow. It does not stop forks, issues or comments, and none of those triggers a workflow here.
It does nothing about a stolen token. One thing is unverified: whether `nife-smelter[bot]` counts as
a collaborator, and seven workflows open pull requests as it. The other candidates are weaker:
interaction limits expire, fork-workflow disabling exists only for private repositories, and
approval for every external contributor is already set. [security.md](gating-ci-rents-more-concurrency-before-it-owns-a-runner/security.md)
has each, read live.

## Security

### calef's ruling on forks

calef, 2026-10-24: "we shouldn't allow forked repos to build on our infrastructure", refined the
same day: "I could see us approving a PR into our repos to be run locally after we've done a
review." The rule, fixed for this proposal: unreviewed fork code never runs on owned
infrastructure. No job triggered by a fork's pull request, and no `pull_request_target` path that
checks out fork code, ever runs on an owned runner, whatever happens to (e).

A reviewed outside change may. After review, a member pushes the exact reviewed commits to a branch
in nifeos/nife and opens or retargets the pull request from it. From then on it is a member's pull
request, and it reaches owned runners only through the merge queue.

This proposal does not use a label-gated `pull_request_target` that runs the fork's code. The label
approves a pull request, not a commit. A push to the fork after approval would run unreviewed code
with the repository's token. Re-pushing pins the reviewed commit SHAs.

### The threat model

calef is the only contributor. Everything else is agent lanes acting for him. So the risks are:

1. A stranger's fork pull request, since the repository is public. The ruling, the routing below
   and (e) close it.
2. The Actions surface: the job token, secrets and third-party actions. The default token is
   read-only, no gating workflow reads a secret, and the two third-party actions are SHA-pinned.
3. Reach from a job into the maintainer's LAN, including the bench's power and serial controls.
4. Supply chain. A dependency's build script runs in the job, but lanes already run the same code
   on the build host.
5. A misled agent or a stolen token pushing code. Lanes arm their own pull requests, so the queue
   admits code no person has read.

Lanes already build and run every tree commit on the build host as the maintainer. For code
already in the tree, an owned runner adds little new exposure. What it adds is a job that runs
unattended, as something other than the maintainer, which is a chance to run it with less.

### The routing that enforces the ruling

- An owned label appears only on jobs that run for `merge_group`, or for `push` to `refs/heads/main`.
  `runs-on` takes one canonical expression: the repository variable that names the owned label
  when the event is one of those two, and the hosted label otherwise. Unsetting the variable sends
  everything to hosted runners.
- A `script/lint` check parses every workflow and fails if any job reachable from `pull_request` or
  `pull_request_target` could resolve to a self-hosted label. Reusable workflows count. So does any
  `runs-on` that is not the canonical expression or a hosted label literal. Its fixtures include
  each way to get this wrong.
- The job's first step on an owned runner fails unless the event is `merge_group` or a push to
  `main` in nifeos/nife. That covers a workflow that dodges the lint.

### The preconditions for (b)

1. Only the routing above, never `pull_request`.
2. A dedicated unprivileged account or rootless container, with no access to the maintainer's `gh`
   token, SSH keys or home directory.
3. Ephemeral or just-in-time registration, one job each, with no writable cache shared between
   jobs.
4. Egress to the LAN blocked; outbound allowed to GitHub and the crate registry only.
5. A runner group restricted to nifeos/nife, with GitHub's public-repository override set on purpose.

A fresh VM per job is the stronger shape, and the appendix describes it. It becomes necessary the
day a real outside contributor appears, because then the queue holds code from someone other than
the maintainer. (e) shrinks nothing further in (b), whose routing already keeps fork code out. What
(e) buys is the end of stranger pull requests at their source, and a mechanical guarantee in place
of one review step.

### The supply-chain angle

basalt's workflow builds the image strangers would run. Release builds stay on hosted runners, or
on a runner group no gating job shares. nife's `build-is-reproducible` job lets a hosted rebuild
check an owned build bit for bit.

## Capacity and cost

| | hosted, Team | hosted, Enterprise | owned, option (b) |
|---|---|---|---|
| concurrent jobs | 60, both labels | 500, both labels | the host's slot count, x86_64 only |
| arm64 jobs | yes | yes | no, unless #2668 moves them |
| cash | $4 a month | $21 a month | power, and the owner's time |
| new failure modes | none | none | runner outage, host compromise |

Each owned slot of hosted shape (4 vCPU, 16 GB) adds 1/60 of the Team pool; doubling it takes 60
slots, matching Enterprise 440. A larger slot would speed `verify`, whose `VERIFY_JOBS=2` is a
16 GB bound, but would change what the job measures.

Power is unmeasured; at a few hundred watts around the clock (an assumption) it costs the same
order as the seat, and [the ledger](../../../notes/project-metrics/ledger.md) would record it.
The larger cost is maintenance: runner releases to take before old ones stop receiving jobs
(recalled), an image to rebuild, and somebody who answers when it stops.

### Availability

A power loss struck during this run. Under (a) it would have touched lane work only. Under (b) it
would also stall the merge queue: a job on a lost runner fails, the group is rebuilt onto the same
dead label, and a job aimed at a label with no runner waits up to 24 hours. The queue's 240-minute
check timeout evicts entries long before that. So the fallback is the variable: one
`gh variable delete`, with no pull request and no queue. `helpers/trunk-health.sh` could unset it
when no runner has been online for ten minutes. Milestone 81 (an HVF leg) refused a runner on a
laptop that sleeps for this reason.

## Reproducibility and trust in results

A required check is a name, the same on any runner, so a mixed fleet is invisible in the ruleset.
Each job on an owned runner writes its runner kind and image digest into its summary.

- `bench`'s instruction counts are guest instructions under the pinned QEMU, so host noise does not
  move them. An owned image builds the same `.qemu-version` with `script/ci-qemu`.
- Wall-clock bounds do move: the 20-minute job budget, every `timeout-minutes`, and the timing-bound
  failures. The riscv64 inbound check fails about one boot in six on CI and never locally, per the
  runner container's header. So owned slots match the hosted shape, carry their own label, and are
  compared job by job before they gate.
- The image is pinned by digest and built from the hosted label's Ubuntu release. When
  [the Ubuntu 26.04 proposal](ci-runners-move-to-ubuntu-26-04.md) flips a label, the owned image
  flips in the same pull request, and its cache keys carry the runner kind as well as the release.

## Rollout and rollback

1. Move the organization to Enterprise, through a trial if GitHub offers one (recalled). Measure
   for a week: wait by label in busy hours, peak concurrency, merges per hour, full-run wall clock.
   Done when a busy hour's median job wait is under a minute. Rollback is a plan downgrade.
2. If calef rules for (e), flip the policy, then open one App pull request to prove the bots still
   can. Rollback is one setting.
3. If the cap still binds, or after #2668 reports: land the lint and the first-step guard, then run
   a canary that gates nothing, dispatched runs of `main` on the owned runner compared with their
   hosted twins for a week.
4. Then the merge-group and `main` jobs, behind the variable. Rollback is unsetting it.

The irreversible part is not the code. A compromised host is a fact that has left the machine: its
credentials are spent, and it is reinstalled, not cleaned.

## calef's rulings (2026-10-24, UTC)

1. Enterprise: not now, and it stays open. It comes back if the 60-job cap still binds under
   ordinary load after #2668 moves the aarch64 and riscv64 guest jobs to x86_64 runners. Binding
   means jobs at the cap with runners waiting, and merge-group runs well over their uncontended 14
   minutes.
2. The pull-request creation policy is `collaborators_only`, set the same day.
3. (b) proceeds under the five preconditions, with the VM shape held for the day an outside
   contributor appears.
4. A green from owned hardware in the merge queue is the gate of record. A daily hosted run against
   `main` catches runner drift.
5. The fallback flips automatically both ways. It returns to owned hardware only after a canary job
   passes there, and stays on hosted after two fallbacks within 24 hours.

Not decided here: cutting demand, such as runs a later push cancels and the label bots' slots.

## Index row

On 2026-10-24 the merge queue waited on GitHub's 60-job Team cap, with 430 jobs waiting. Enterprise
raises it to 500 for $17 a month. An owned runner takes merge-group and `main` jobs only, under
calef's ruling that unreviewed fork code never runs on owned infrastructure, enforced by a lint.
It adds x86_64 capacity unless #2668 frees the arm64 jobs.
