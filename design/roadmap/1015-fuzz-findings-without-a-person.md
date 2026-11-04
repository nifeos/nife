---
status: NOT-STARTED
promoted_from: fuzz-findings-without-a-person
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1015. Fuzz findings filed with no person in the loop

Raised by lane/fuzz-findings-without-a-person on 2026-11-01 (UTC), to record four architect rulings
of that date and split them into milestones. They amend the mechanics of the disclosure ruling of
2026-10-31 (UTC) in [the risk 7 proposal](1003-the-fuzz-budget-that-counts-for-risk-7.md), under
"Disclosure", which #2955 records. That ruling stands. Every finding starts private, in a private
security advisory on nifeos/nife. A held finding turns public after its fix, or 90 days after the
report.

Reuse: the supervisor, the held directory and the daily report of
[the host note](../../notes/fuzzing/continuous-host.md) stay as they are. GitHub's repository
security advisories API files and publishes. Nothing here writes a new transport.

## The question

The disclosure ruling said where a finding goes and when it turns public. It did not say who moves
it. Read literally, a person had to notice each held window and file the advisory. The same person
then handed the reproducer to a fix lane and published the advisory after the fix. A fuzzer that runs
around the clock and a triage clock of 3 days cannot wait on that person. Every hand-off is also a
place where a reproducer can leak.

## What is in force: the findings path

Ruled 2026-11-01 (UTC), narrowed 2026-11-04 (UTC): the fuzzing loop runs with no person in it, after
a person places one credential once, and it ends at reporting. The fuzz host files one private
advisory per held finding on nifeos/nife and stops. A person works the queue of security reports
on the fix-lane machine, in each advisory's private fork, and publishes each advisory when its fix
merges. Four steps handle findings.

1. Filing. A plain service on the fuzz host files a private security advisory on nifeos/nife for
   each new held finding. The service is a program, not an agent and not a language model. The
   advisory holds the finding class and the reproducer.
2. Reading. A fix lane reads the advisory, never the host's held-findings directory. The standing
   rule stays in force: no lane opens the held directory. The advisory is the one narrow exception.
   A lane reads it through a read-only credential that a person places, and no lane looks for
   tokens.
3. Fixing. A fix lane develops the fix in the advisory's temporary private fork, and the approved
   fix lands through the merge queue, as the section on how a fix lane claims says. The regression test does not carry the reproducer before the fix is on main.
4. Never in public text. Crash inputs, seeds and reproducers stay out of public text until the fix is
   on main: pull request titles, bodies and comments, logs and artifacts.

## What is in force: the credential

Ruled 2026-11-01 (UTC): one new GitHub App, provisionally named `nife-fuzz`, installed on
nifeos/nife only, holds two permissions and nothing else. Contents write lets it create the daily
`fuzz-YYYY-MM-DD` release. Repository security advisories write lets it run the findings path. It
holds no workflow or administration permission. It is separate from `nife-smelter`, whose key never
sits on the fuzz host: that App is on every repository and bypasses the tag rulesets of the crate
repositories.

The host keeps the App's private key in a file only root can read and mints one-hour installation
tokens from it. A person creates the App and places the key once. The key does not expire, so no
schedule rotates it. [`notes/fuzzing/the-fuzz-app.md`](../../notes/fuzzing/the-fuzz-app.md) is the
person's runbook.

What the key can do is the exposure. The main ruleset of nifeos/nife has no bypass actor and
requires a pull request, so Contents write cannot push to main. A token minted from the key can
create, move or delete any `fuzz-*` tag and any release. Advisories write lets
it publish any draft advisory, a held finding's included.

Ruled 2026-11-01 (UTC): the architect accepts that exposure, with a tag ruleset on nifeos/nife that
refuses creating, updating and deleting any tag outside `fuzz-*`, with no bypass actor. The ruleset
exists on the repository as "tags: only fuzz-* tags may be made", and nifeos/nife had no tags when
it took effect. The App is created and installed on nifeos/nife only: App ID 5571801, installation
ID 177641059, neither secret. A future nife release that uses a tag outside `fuzz-*` must amend the
ruleset first, in its own step recorded in a pull request.

## What is in force: the reader App

Ruled 2026-11-01 (UTC): fix lanes read a held finding's advisory through a second GitHub App,
provisionally named `nife-fuzz-reader`. It holds Repository security advisories read and nothing
else, installed on nifeos/nife only. A fine-grained personal token is not used, because it expires
and belongs to a person. The App's key lives in a root-only file on the machine that launches fix
lanes. It never sits on the fuzz host and never enters a lane's environment. A lane gets a one-hour
installation token at launch; the milestone
[fix lanes get a one-hour reader token](1013-fix-lanes-get-a-one-hour-reader-token.md) builds that.

## What is in force: how a fix lane claims

Ruled 2026-11-04 (UTC), amending §90 (the claim is a draft pull request): a fix lane for a held
finding develops its fix in the GitHub temporary private fork of its advisory by default, and the
reviewed fix lands as an ordinary pull request through the merge queue.

- The claim is a note on the advisory, not a public draft pull request. The advisory list is the
  claim board for this kind of lane, and the host files one advisory per finding.
- The private fork has no CI and no merge queue, so the lane runs the local gates itself:
  `script/fmt`, `script/lint` and the tests its change reaches. The lane reports each gate's exit.
- A person reviews the fix in the private fork. Once the person approves it, the lane pushes the
  same commits to a branch on nifeos/nife and opens an ordinary pull request, ready for review, with
  auto-merge armed and a neutral title. The merge queue runs every gate on it.
- A person publishes the advisory when that pull request merges. No lane publishes.
- Nobody merges from the private fork. On a test repository, a ruleset with no bypass actor blocked
  that merge: the advisory page disables the merge button, and with a bypass actor the merge needs
  an explicit choice to bypass branch protections. Main's ruleset keeps no bypass actor, because one
  would let any lane holding the admin token skip the merge queue and every required check.
- Exception: a finding the architect judges low severity may use a normal public draft pull
  request, with the regular lane workflow, a neutral title and no reproducer.

The reason is exposure. A draft pull request and its pushed branch are public from the first
commit, so a fix developed in public shows the bug for the whole of its development, review and
the merge queue. Development and review in the private fork leave only the queue.

The cost is that window. The bug is public from the push of the approved fix until it merges and
the advisory is published, about an hour of merge queue, longer under a hold. The low-severity
exception exists for the findings where a longer window costs nothing.

An example. The fuzz host files a held crash in a parser as a draft advisory. The maintainer
launches a fix lane on the fix-lane machine, which adds a note to the advisory naming the lane and
opens the advisory's private fork. The lane commits the fix and a regression test of the broken
invariant there, runs `script/fmt`, `script/lint` and the parser's tests, and reports their exits.
A person reads the diff in the fork and approves it. The lane pushes the same commits to nifeos/nife
and opens the pull request ready, with auto-merge armed. About an hour later it merges from the
queue, and the person publishes the advisory.

## What is in force: the host follows main

Ruled 2026-11-01 (UTC): the fuzz host updates itself from main with no person pulling. Between
windows the supervisor, running as the unprivileged fuzz account, fetches and fast-forwards its clone
and re-executes itself, under four limits.

1. Fast-forward only. It refuses a new commit whose history does not contain the current one.
2. It fetches only the main branch of the public nifeos/nife repository, over https.
3. It moves only to a commit whose checks on main are green, read from the public API without a
   credential. A red or unreadable result keeps the current commit.
4. It does not apply a change to the unit file, which needs root. It writes one journal line saying
   that the unit changed.

The update widens nothing the unprivileged account may do. It needs no credential, it reads only
what any stranger can read, and it runs code that has already passed main's gates. The host does run
main's code unattended, and the runbook says so. The code that mints tokens from the root-only key
therefore stays outside the clone: a person installs it, as with the unit files, and an update never
replaces it.

## What is in force: noticing a broken loop

Ruled 2026-11-01 (UTC): a daily watchdog workflow on a GitHub-hosted runner detects a broken fuzz
loop. It notifies by opening an issue with a fixed label, and it repairs nothing. For now a person
turns each such issue into an action. A lane fixes anything in the tree. A repair of the host itself
becomes a spec that the architect hands to the agent that maintains the host. Another lane
builds the workflow and owns its milestone, so this proposal lists no milestone for it.

Two later decisions stay open until the loop has run for about a month:

- a standing channel from the watchdog to the agent that repairs the host, and
- starting a lane from a watchdog issue automatically.

## How the lane fills in the mechanics

Each item below is a two-way choice, decided by the maintainer, provisional; reversing costs one pull
request.

- The reproducer stays out of the fix pull request. That pull request carries the fix and a
  regression test of the invariant that broke, and names no seed, replay line or input. The seed
  joins the fixed-seed regression list in a second pull request, opened after the advisory turns
  public. The ruling would allow the seed in the fix's own commit. That form puts the seed in the
  public diff and in public CI logs for the whole merge-queue window, so the stricter form comes
  first.

## What GitHub's documentation confirms

Read on 2026-11-01 (UTC):

- An App can hold the advisory permission. The table of App permissions lists "Repository security
  advisories": write for creating and updating an advisory, read for listing and reading one. Every
  endpoint takes installation tokens. Contents is a separate permission in the same table, and an App
  chooses its own set, so one App can hold both.
- An App installs on "only select repositories". An installation token can narrow further to named
  repositories and permissions, and it expires after one hour.
- App private keys "do not expire and instead need to be manually revoked". An App can hold up to 25
  keys.
- Deleting a ref (`DELETE /repos/{owner}/{repo}/git/refs/{ref}`) and deleting a release each need
  Contents write and no other permission. Creating or moving a ref also lists Workflows, which
  applies when the ref's commit changes workflow files.
- A fine-grained personal token can hold the same advisory read permission.
- On a public repository, the check runs of a main commit read without a credential: a request with
  no token returned 200. The unauthenticated limit is 60 requests an hour per address. Main's commit
  carried 125 check runs, two pages, so one look every 5 minutes costs 24 requests an hour.
- Pull requests in an advisory's temporary private fork get no status checks, no CI, and no branch
  protection on their merge. Only an administrator merges, and every open pull request of the fork
  merges at once from the advisory page. A fix merged that way would skip every gate and the merge
  queue, which is one reason the fix lands through an ordinary pull request instead.

## What is not verified

- How long a fix pull request stays public is unmeasured. The window runs from the push of the
  approved fix to the merge, about an hour of merge queue by estimate. It grows whenever a hold
  applies: `needs-architect` on a fix that touches the syscall surface, or a red trunk. The first
  fix measures it.
- Whether a draft advisory needs private vulnerability reporting switched on (from memory: it does
  not).
- Whether a published advisory shows the App's comments to the public.
- Whether every gate of a merge-queue commit appears as a check run on that commit once it is main.

## Prior art

OSS-Fuzz files each crash in a restricted issue automatically. ClusterFuzz reruns open test cases
against new builds and marks a case fixed when it stops reproducing. It opens the issue 30 days after
the verified fix or 90 days after the report, whichever comes first. That account comes from copies
of the OSS-Fuzz documentation, not the live site. nife publishes at the verified fix rather than 30
days later, keeps the record in a GitHub advisory rather than an issue tracker, and gives the fix to
an agent lane that never sees the fuzz host.

Where others develop the fix, read on 2026-11-04 (UTC):

- Chromium keeps critical, high and medium severity bugs visible only to the security team and the
  engineers fixing them, lands the fix in the public repository, and opens the bug about 30 days
  after the fix lands. Its FAQ says attackers can still analyze public commits to find the bugs.
- The Linux kernel security team is a private list that helps the reporter and the area maintainers
  develop a fix. The fix then follows the normal patch process, and its release may wait up to 7
  calendar days (14 in exceptional cases) for the logistics of QA.
- GitHub's temporary private fork is a private copy of a public repository, made from a draft
  advisory, where collaborators on the advisory fix a vulnerability before it is public. It runs no
  CI and an administrator merges its pull requests together, then publishes the advisory.
- The Rust security team holds fixes in private repositories, reviews them privately with the same
  process as public changes, and pushes them to the public repository on the embargo date with the
  announcement.
- Go sorts reports into three tracks. The team fixes PUBLIC issues (niche configurations, very
  limited impact, or already widely known) in public. It fixes PRIVATE and URGENT issues in private
  and keeps them private until the release. The Go security team chooses the track.

nife differs from Chromium and the kernel in how long the fix is public before disclosure. Those two
land the fix in public and disclose later. nife develops and reviews the fix in private, as Rust
does, and then makes it public for the merge queue alone. Rust and Go release by hand, with people
who hold direct push rights to the public repository, so a reviewed fix appears there and ships at
once. nife has no direct push to main by design, so the reviewed fix still goes through the queue,
and the queue time is the public window. Its default private path and low-severity public
exception follow Go's tracks, with the architect choosing the track. Nothing in the private fork
runs the project's CI, so the local gates stand in for it during review, and the queue runs every
gate before the merge.

## Milestones

Each is a proposal in this directory, unnumbered until an integrator promotes it.

- [The fuzz host files each held finding as an advisory](1020-the-fuzz-host-files-held-findings-as-advisories.md)
- [Fix lanes read the advisory](1014-fix-lanes-read-the-advisory.md)
- [Fix lanes get a one-hour reader token](1013-fix-lanes-get-a-one-hour-reader-token.md)
- [A held finding turns public after its fix reruns clean](1010-a-held-finding-turns-public-after-its-fix-reruns-clean.md)
- [The fuzz host mints its tokens from the App key](1022-the-fuzz-host-mints-its-tokens-from-the-app-key.md)
- [The clean-range table does not locate a held seed](1016-the-clean-range-table-does-not-locate-a-held-seed.md)
- [The fuzz host follows main](1021-the-fuzz-host-follows-main.md)

## Index row

The path of a held fuzz finding runs with no person from the finding to a private advisory, after one credential is placed, and ends there. A fix lane develops the fix in the advisory's private fork, the approved fix lands through the merge queue, and a person publishes the advisory when it merges. Rulings fix the credentials, the host and the lane, and this file splits them into milestones.
