---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# Fuzz findings filed, fixed and published with no person in the loop

Raised by lane/fuzz-findings-without-a-person on 2026-11-01 (UTC), to record two architect rulings of
that date and split them into milestones. Both amend the mechanics of the disclosure ruling of
2026-10-31 (UTC) in [the risk 7 proposal](the-fuzz-budget-that-counts-for-risk-7.md) ("Disclosure"),
recorded on #2955. That ruling stands: every finding starts private, in a private security advisory
on nifeos/nife, and a held finding turns public after its fix, or 90 days after the report.

Reuse: the supervisor, the held directory and the daily report of
[the host note](../../../notes/fuzzing/continuous-host.md) stay as they are. GitHub's repository
security advisories API does the filing and the publishing; nothing here writes a new transport.

## The question

The disclosure ruling said where a finding goes and when it turns public. It did not say who moves
it. Read literally, a person had to notice each held window, file the advisory, hand the reproducer
to a fix lane and publish the advisory after the fix. A fuzzer that runs around the clock and a
triage clock of 3 days cannot wait on that person, and every hand-off is a place where a reproducer
can leak.

## What is in force

Ruled 2026-11-01 (UTC): the fuzzing loop runs with no person in it, after a person places one
credential once. Findings are handled automatically, in five steps.

1. **Filing.** A plain service on the fuzz host, not an agent and not a language model, files a
   private security advisory on nifeos/nife for each new held finding. It acts through a GitHub App
   that holds only the repository security advisories permission, installed on that one repository.
   The advisory holds the finding class and the reproducer. A person places the App's key on the
   host once. No lane handles, reads or looks for that key.
2. **Reading.** A fix lane reads the advisory, never the host's held-findings directory. The standing
   rule stays in force: no lane opens the held directory. The advisory is the one narrow exception,
   read through a read-only credential that a person places in the lane environment. No lane looks
   for tokens.
3. **Fixing.** The fix pull request opens only when the fix is ready, with auto-merge armed, so the
   window of exposure is the time the pull request spends in review and the merge queue. Its
   regression test does not carry the reproducer before the fix is on main.
4. **Publishing.** After the fix lands on main, the host reruns the finding's input against the
   fixed build. A pass publishes the advisory automatically. A failure keeps it private and raises a
   new lane.
5. **Never in public text.** Crash inputs, seeds and reproducers do not appear in public text before
   the fix is on main: not in pull request titles, bodies or comments, not in logs, not in
   artifacts.

Ruled 2026-11-01 (UTC): the fuzz host updates itself from main with no person pulling. Between
windows the supervisor, running as the unprivileged fuzz account, fetches and fast-forwards its clone
and re-executes itself, under four limits.

1. Fast-forward only. It refuses a new commit whose history does not contain the current one.
2. It fetches only the main branch of the public nifeos/nife repository, over https.
3. It moves only to a commit whose checks on main are green, read from the public API without a
   credential. A red or unreadable result keeps the current commit.
4. It does not apply a change to the unit file, which needs root. It writes one journal line saying
   that the unit changed.

Nothing in the update widens what the unprivileged account may do. It needs no credential, it
reads only what any stranger can read, and it runs code that has already passed main's gates. The
consequence is that the host runs main's code unattended, and the account that runs it holds the
filing App's key. Anything merged to main can therefore act with that key within one window. The
credential runbook says so.

## How the lane fills in the mechanics

Each item below is a two-way choice, decided by the maintainer, provisional; reversing costs one pull
request.

- **How a fix lane keeps the reproducer out of the fix pull request.** The fix pull request carries
  the fix and a regression test of the invariant that broke, written so that it does not name the
  seed, the replay line or the input. The seed joins the fixed-seed regression list in a second pull
  request, which the lane opens only after the advisory is published (step 4). This is stricter than
  putting the seed in the same commit as the fix, which the ruling allows. The same-commit form
  would put the seed in the public diff, and in public CI logs, for the whole merge-queue window.
  Loosening later costs one rule edit.
- **How a fix lane claims.** A fix lane for a held finding does not open a draft pull request at its
  start, because the ruling opens the fix pull request only when the fix is ready. It runs
  `script/claim --no-claim` and keeps its commits local until then. Two lanes cannot take the same
  advisory because the maintainer raises at most one lane per advisory. This departs from §90 (the
  claim is a draft pull request) for this one kind of lane only; the integrator records that
  amendment.
- **What the rerun reruns.** Once a day the host rebuilds the newest green main and replays every open
  advisory's seed against it, one seed per guest. It needs no link from the fix commit to the
  advisory, so no commit trailer or other agreement between programs exists to keep in step.

## What is not verified

Read from GitHub's documentation on 2026-11-01 (UTC):

- The permission exists for Apps. GitHub's table of App permissions lists "Repository security
  advisories" with write access for creating and updating an advisory, read access for listing and
  reading one, and both installation and user tokens for every endpoint. An App installs on "only
  select repositories", and an installation token can be narrowed further to named repositories and
  permissions. A fine-grained personal token can hold the same permission. Installation tokens
  expire after one hour.
- The create endpoint's documentation says the caller "must be a security manager or administrator"
  of the repository. Whether an App installation with the advisory permission alone meets that is
  not stated, and nobody has tried it.
- Publishing is an update that sets the state to `published`. No permission beyond the update's is
  named.
- Check runs on a public repository read without a credential: a request for the check runs of a
  main commit returned 200 with no token on 2026-11-01 (UTC). The unauthenticated limit is 60
  requests an hour per address, and one commit carried 125 check runs, two pages. One look every 5
  minutes costs 24 requests an hour. Many of those runs are watchers (a cancelled merge-drain run)
  or skipped jobs, so "green" has to mean a named set of gate checks, not every run.

Not verified, and why:

- Temporary private forks are not the fix path. GitHub's documentation says that status checks do not
  run on pull requests in a temporary private fork, that integrations including CI cannot reach one,
  and that branch protection is not enforced on its merge. A fix merged that way skips every gate
  and the merge queue, so it is not used here.
- The exposure window is not measured. It is the time from the push of the fix branch to the merge,
  and it is longer whenever a hold applies: `needs-architect` on a fix that touches the syscall
  surface waits for an architect, and a red trunk holds the queue. The first fix measures it.
- Whether GitHub requires private vulnerability reporting to be on before a maintainer-side draft
  advisory can be created is unverified (from memory: it does not).
- Whether a published advisory's page shows anything filed by the App (its body, comments) is
  unverified.

## Prior art

OSS-Fuzz files each crash in a restricted issue automatically. ClusterFuzz reruns open test cases
against new builds, marks a case fixed when it stops reproducing, and opens the issue 30 days after
the verified fix or 90 days after the report, whichever is earlier (read from copies of the OSS-Fuzz
documentation, not the live site). nife differs in three ways: it publishes at the verified fix
rather than 30 days later, the record is a GitHub advisory rather than an issue tracker, and the fix
is written by an agent lane that never sees the fuzz host.

## Milestones

Each is a proposal in this directory, unnumbered until an integrator promotes it.

- [The fuzz host files each held finding as an advisory](the-fuzz-host-files-held-findings-as-advisories.md)
- [Fix lanes read the advisory](fix-lanes-read-the-advisory.md)
- [A held finding turns public after its fix reruns clean](a-held-finding-turns-public-after-its-fix-reruns-clean.md)
- [Placing the fuzz advisory credentials](placing-the-fuzz-advisory-credentials.md)
- [The clean-range table does not locate a held seed](the-clean-range-table-does-not-locate-a-held-seed.md)
- [The fuzz host follows main](the-fuzz-host-follows-main.md)
