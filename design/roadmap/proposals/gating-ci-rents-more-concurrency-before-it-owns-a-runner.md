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

Reuse: GitHub's hosted runners and its plan tiers, unchanged. For the staged self-hosted option,
GitHub's just-in-time runner API, and the refusal-by-selftest pattern of milestone 920 (an outsider
pass is launched pinned and floor-checked). This proposal builds nothing and changes no workflow.

Raised 2026-10-24 (UTC) by lane `maintainer/proposal-self-hosted-gating-runners`, at calef's
request, during the run of milestone 929 (comments in the Linux style across first-party Rust).
About 40 pull requests were in flight, and hosted CI was the only thing they waited on. The
measurements and their method are in the appendix,
[measurement.md](gating-ci-rents-more-concurrency-before-it-owns-a-runner/measurement.md).

## The claim and the decision asked

The claim: on 2026-10-24 the gate waited on GitHub's 60-job concurrency cap, not on the work. The
next rented tier raises that cap eightfold for $17 a month more. An owned runner adds capacity only
for x86_64 jobs, and only at a security cost this tree has refused twice.

The decision: may jobs that gate a merge run on hardware the project owns, and if so, which jobs?
[§299 (the build machine is bought)](../../decisions/0299-the-build-machine-is-bought.md) leaves
exactly this open: "Whether anything that gates a merge runs on" the build machine. The options:

- (a) Hosted runners only. Raise the cap by renting: the Enterprise tier, as
  [§214 (the Team plan buys runner concurrency)](../../decisions/0214-the-team-plan-buys-runner-concurrency.md)
  did with Team.
- (b) Self-hosted runners for merge-group runs only.
- (c) Self-hosted runners for every required check on pull requests from members, hosted runners
  for forks.
- (d) A hybrid by job type, such as the QEMU-heavy shards self-hosted.

Recommendation: (a), by moving the organization to the Enterprise tier, measured for a week. Keep
(b) as the staged fallback, with the preconditions under Security met before it runs a single
gating job. Refuse (c). Treat (d) as a variant of (b), because the QEMU-heavy shards are the ones an
owned x86_64 host cannot take.

Why: the hosted tier is the far larger lever, adds no failure mode, and keeps the posture
milestone 488 (a self-hosted CI runner) protects. At equal effort (a) would still win, on security and moving
parts.

## The evidence, from the milestone-929 run

All times UTC. Maintainer snapshots of the Actions API counted 124 runs queued and 4 in progress
at 01:25, 151 and 6 at 03:15, and 249 and 3 at 03:32. A run is queued while any job waits, so jobs
are the sharper reading. At 03:36:13, across every active run, 60 jobs were in progress, 20 on `ubuntu-24.04` and 40 on
`ubuntu-24.04-arm`. Another 430 jobs waited, 257 and 173. Sixty is exactly the Team cap.

- Saturation. From 00:00 to 03:30, sampled each minute, the heavy workflows (`CI`, `verify`,
  `codeql`, `mutation gate`) ran a median of 59 jobs at once. They used 12,335 job-minutes of the
  12,600 that 60 slots give in that time, 98%. The label bots ran in the same slots on top of that.
- Waiting. On 10-24 from 00:00 to 03:36, a job's median wait for a runner was 5 to 13 minutes per
  hour, p90 14 to 23, worst 35. On the afternoon of 10-23 it was under 0.1 minutes. The oldest
  queued `CI` run, 40054840698, waited 11 minutes for its draft gate, a job of seconds.
- Wall clock. A full `CI` run is 26 jobs and about 128 job-minutes. Uncontended, a merge-group run
  took a median 14 minutes (n=6, 10-23 13:00 to 23:59). Under load it took 38 minutes (n=12), and
  a pull request's full run took 46 (n=47, worst 69).
- Merges. `git log --first-parent` on `main` shows 14 merges from 00:00 to 03:36 on 10-24, one per
  15 minutes. They came in a cluster of 12 between 01:06 and 01:55, then none for 101 minutes. At
  03:36 the queue held 13 entries, and its head, #2643, had been awaiting checks since 01:59:29.
- Repeat cost. Each pull request pays a full run and a merge-group run. Since 10-23 00:00, 53
  branches spent 149 full `CI` runs, a median 3 each: 53 were cancelled by a later push and 31
  failed. Thirty pull requests needed 46 merge-group runs, and nine needed more than one.

The owned host doing the lane work was not the limit:
[`briefs/run-a-milestone-wide.md`](../../../briefs/run-a-milestone-wide.md) records its CPUs about
85% idle, because "CI runs off the host".

§214 set the condition for this question: "Reconsider a self-hosted runner only if 60 concurrent
jobs still binds." It binds, so the question is due.

## The options

### (a) Hosted only, with the cap raised

GitHub's limits reference (read 2026-10-24) gives standard hosted runners 60 concurrent jobs on
Team and 500 on Enterprise. Public repositories pay nothing for standard runner minutes on either,
as §214 recorded. GitHub's pricing page lists Enterprise at $21 per user per month for the first 12
months, against Team's $4. With one seat that is $17 a month more, and the page does not state the
price after a year.

At 03:36 the demand was 60 running and 430 waiting, about 490 jobs. A 500-job cap would have run
nearly all of it at once, putting each full run back near its uncontended 14 minutes. The merge
queue's own bound, five entries built at a time, would then decide throughput.

What it does not settle: whether GitHub's arm64 pool supplies hundreds of concurrent public-repo
jobs on demand. Milestone 587 (most CI jobs do not need an arm64 host) found arm64 was where the
queue waited, so the week of measurement watches that label first.

Larger hosted runners lose for now: billed per minute even on a public repository (recalled), against
Actions budgets capped at $0 (§214).

### (b) Self-hosted, merge-group runs only

The merge group is the critical path, so this is where an owned runner would help most per job.
Its reach is small, though. Of a full run's 128 job-minutes, 84 are on `ubuntu-24.04-arm` (9 jobs)
and 44 on `ubuntu-24.04` (17 jobs). The owned host is x86_64 (§299), where aarch64 and riscv64 run
fully emulated.

Milestone 587 kept the build-and-test shards, the CPU matrix and re-falsify on arm64 on purpose:
the host's weak ordering is what the SMP guest legs exercise. Moving them to an x86_64 host loses
that coverage. Those arm64 jobs are also the longest, at 12 to 14 minutes. So (b) takes the x86_64
jobs off the hosted pool and leaves arm64 more of the 60 slots. Merge-group x86_64 work was about
7% of the window's demand.

### (c) Self-hosted for members' pull requests, hosted for forks

This is the largest win an owned host can give: pull-request runs were 7,358 of the window's
12,335 job-minutes. It is also the widest exposure, since it runs every pushed commit on owned
hardware before anyone has looked at it. Refused, for the reasons under Security.

### (d) Hybrid by job type

The QEMU-heavy shards are the arm64 jobs above, which cannot move. What remains is x86_64 work:
`verify`'s prove shards (49 of its 52 job-minutes), `lint`, `fuzz`, `bench` and
`swish-check-x86_64`. Sending those for merge groups is (b); for pull requests too, (c).

## Security

### What GitHub says

From GitHub's secure-use reference, read 2026-10-24: self-hosted runners "should almost never be
used for public repositories on GitHub", because "any user can open pull requests against the
repository and compromise the environment." They "can be persistently compromised by untrusted code
in a workflow." Just-in-time runners "perform at most one job", yet "there is no way to guarantee
that a self-hosted runner only runs one job." A runner group admits only private repositories until
an owner overrides it.

### What this repository already does

Every external contributor's fork run waits for approval, the default token is read-only, and
actions are pinned by SHA (the appendix has the reads). No workflow triggers on
`pull_request_target`, and three label bots say why in their headers. No gating workflow reads a
secret. None of the last 1,000 pull requests came from a fork.

### Who the attacker actually is

Not a stranger's fork. Lanes arm their own pull requests, so the queue admits code no person has
read. The threat is an agent steered by text it read, a dependency's build script, or a stolen
token. So (b) and (c) differ in volume, not kind, and isolation must hold against arbitrary code.

### What an attacker in a job could reach

Inside a job: its disk and memory, a read-only token, the Actions cache for its own ref, and
whatever egress allows. A malicious commit can make its own checks green on hosted runners too,
since it carries the workflow file.

What owning adds is reach beyond the job:

- Persistence. A reused runner lets one job falsify later jobs' results for other pull requests.
  That is the gate's integrity, and only a fresh machine per job prevents it.
- A shared local cache. A writable cache that pull-request and merge-group jobs both read crosses
  the ref scoping GitHub's cache enforces.
- The host. A hypervisor escape lands on whatever the host holds. The owned build host holds lane
  worktrees and the maintainer credential that pushes and merges. That is worth more than anything
  in a job.
- The network. A home connection behind the runner can reach the maintainer's LAN, including the
  bench's power and serial controls, and its address is the maintainer's.

### The shape (b) would have to take

Each is a precondition, not a refinement:

1. A fresh virtual machine per job, from a pinned image, registered as a just-in-time runner and
   destroyed after one job. A container shares the host kernel, and gating jobs need `/dev/kvm` for
   `swish-check-x86_64`. Rootless podman with KVM passed through is the weaker shape. A microVM
   (Firecracker or Cloud Hypervisor, recalled) or a plain KVM guest with nested virtualization
   is the stronger one.
2. No maintainer credential on the hypervisor host. Either a host of its own, or the lane work moves
   into its own VM so the host operating system holds nothing worth escaping to.
3. Its own network segment, with no route to the LAN and outbound traffic denied by default. Allow
   GitHub's endpoints, crates.io, static.rust-lang.org and, until the image bakes its packages, the
   Ubuntu archive. Rate-limit egress.
4. No writable cache shared between jobs. The image carries the pinned QEMU and toolchains, and
   anything else comes through GitHub's ref-scoped cache.
5. A runner group restricted to nifeos/nife, with the public-repository override set on purpose.
6. A launcher selftest that refuses a VM configuration with a host mount, a LAN route or a reused
   disk. That is rung two of `AGENTS.md`'s ladder, and the outsider-pass launcher already shows the
   pattern.

### How the outsider pass already confines untrusted code

`helpers/launch_pass.py` runs an outside reviewer in rootless podman inside a Linux VM. The
container gets the clone, a results directory and one volume, and "nothing else of the host: no
home directory, SSH key, `gh` token or MCP configuration." Its selftest fails if a `podman run`
carries `--privileged`, `--network`, `--volume`, `--cap-add` or seven other flags.

A runner should copy that refusal, but not the rest. The pass allows open egress, keeps its volume
across runs and runs no KVM job; a gate needs per-job disposal and an egress allowlist.

### Forks

Only (a) keeps forks off owned hardware by construction. Options (b) to (d) keep them off only with
two rules. A job for a pull request whose head is another repository runs on hosted labels. And a
fork's pull request is never enqueued directly: a member re-pushes the reviewed commits to a branch
in nifeos/nife, keeping authorship, so no merge group holds fork code. Without the second rule, (b)
runs fork code the moment it is enqueued.

### The supply-chain angle

basalt's workflow builds the image strangers would run. Whatever runs gating jobs must never build
release images: release builds stay on hosted runners, or on a runner group no pull request can
reach. nife's `build-is-reproducible` job helps here. A reproducible build lets a hosted rebuild
check an owned one bit for bit, so trust in an owned build can be earned rather than assumed.

## Capacity and cost

| | hosted, Team | hosted, Enterprise | owned, option (b) |
|---|---|---|---|
| concurrent jobs | 60, both labels | 500, both labels | the host's slot count, x86_64 only |
| arm64 jobs | yes | yes | no (x86_64 host) |
| cash | $4 a month | $21 a month | power, and the owner's time |
| new failure modes | none | none | runner outage, host compromise |

Each owned slot of hosted shape (4 vCPU, 16 GB) adds 1/60 of the Team pool; doubling it takes 60
slots, matching Enterprise 440. A larger slot would speed `verify`, whose `VERIFY_JOBS=2` is a
16 GB bound, but would change what the job measures (below).

Power is unmeasured; at a few hundred watts around the clock (an assumption) it costs the same
order as the seat, and [the ledger](../../../notes/project-metrics/ledger.md) would record it.
The larger cost is maintenance: an image to rebuild, runner releases to take before old ones stop
receiving jobs (recalled), a hypervisor to patch, and somebody who answers when it stops.

### Availability, and what an outage does to the queue

A power loss struck during this run. Under (a) it would have touched lane work only. Under (b) it
would also stall the merge queue. A job on a lost runner fails, so the group fails and is rebuilt,
onto the same dead label. A job aimed at a label with no runner waits up to 24 hours. The queue's
240-minute check timeout evicts entries long before that.

So (b) needs a fallback that is a setting, not a pull request. Each merge-group job's `runs-on`
reads a repository variable that defaults to the hosted label, and one `gh variable delete` flips
it. `helpers/trunk-health.sh` could do that when no runner has been online for ten minutes; runs
already stuck still need a re-run. Milestone 81 (an HVF leg) refused a runner on a laptop that
sleeps for this reason.

## Reproducibility and trust in results

A required check is a name, the same on any runner, so a mixed fleet is invisible in the ruleset.
Each job on an owned runner writes its runner kind and image digest into its summary.

- `bench`'s instruction counts are guest instructions under the pinned QEMU, so host noise does not
  move them. An owned image must build the same `.qemu-version` with `script/ci-qemu`, never a
  packaged QEMU.
- Wall-clock bounds do move: the 20-minute job budget, every `timeout-minutes`, and the timing-bound
  failures. The riscv64 inbound check fails about one boot in six on CI and never locally, per the
  runner container's header. A new host class is a new flake rate. So owned slots match the hosted
  shape, carry their own label, and are compared job by job before they gate.
- The image is pinned by digest and built from the same Ubuntu release as the hosted label. When
  [the Ubuntu 26.04 proposal](ci-runners-move-to-ubuntu-26-04.md) flips a label, the owned image
  flips in the same pull request. Its cache-key step, which adds the release, would add the runner
  kind too, since an owned build and a hosted one may link different libraries.

## Rollout and rollback

1. Move the organization to Enterprise, through a trial if GitHub offers one (recalled, not read).
   Measure for a week: created-to-started wait by label in busy hours, peak concurrency, merges per
   hour, and full-run wall clock. Done when a busy hour's median job wait is under a minute. Rollback
   is a plan downgrade, which no tree state depends on.
2. Only if the cap still binds, or calef refuses the plan: build (b)'s six preconditions and run a
   canary that gates nothing. Run `workflow_dispatch` runs of `main` on owned VMs for a week and
   compare each job with its hosted twin. Measure the slot count, per-job boot overhead, failures
   and power.
3. Then the merge group's x86_64 jobs, behind the repository variable. Measure the same numbers as
   step 1. Rollback is unsetting the variable.

The irreversible part is not the code. A compromised host is a fact that has left the machine: its
credentials are spent, and the host is reinstalled, not cleaned.

## What calef must rule

1. Move the organization from Team to Enterprise, $17 a month more for the first year with the later
   price unstated, to raise the cap from 60 jobs to 500? Recommended. If no, the queue stays bound
   as measured, and question 2 becomes the only lever.
2. If the cap still binds after a week, may (b) proceed under its six preconditions? This would
   settle milestone 488's condition, "the security posture changing", in the way
   [§203 (capacity is rented rather than bought)](../../decisions/0203-capacity-is-rented-not-bought.md)
   already allows.
3. Can runner VMs share the build host with lane work, if the lane work moves into a VM of its own?
   Or does a gating runner need a host that holds no maintainer credential at all?
4. Under (b) to (d), is the fork rule acceptable: a fork's pull request is re-pushed by a member,
   never enqueued directly?
5. Is a green from owned hardware the gate of record? Or must the push to `main` re-test on hosted
   runners for those groups, giving up the merge-group citation's saving for them?
6. Who answers when an owned runner stops, and does the fallback flip automatically?

Not decided here: cutting demand (runs a later push cancels, the label bots' slots), each a lane of its own.

## Index row

On 2026-10-24 the merge queue waited on GitHub's 60-job Team cap, with 430 jobs waiting. Enterprise
raises the cap to 500 for $17 a month. An owned runner adds only x86_64 capacity, and only behind a
fresh VM per job, an isolated network and a fork rule. Rent first; stage merge-group runners behind it.
