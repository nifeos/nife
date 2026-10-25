---
status: PARTIAL
raised: 2026-08-15
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 128. The automation gets its own identity, and the agents get their own voice

<!-- writing-standards: exception. Marked 2026-10-03 (UTC) by the maintainer session. Reason: this change
touches the block only to correct a stale claim (that the COE labeler had never fired). Bringing a
2,200-word block to 4 bold spans per 1,000 words is a rewrite for the block's owner, and six of its
bold spans are `## Follow-on` markers `script/roadmap` reads (the measured class is recorded in
design/roadmap/0586-a-prose-ratchet-in-lint.md). Remove this marker when that rewrite lands. -->

Partial as of 2026-10-03 (UTC): steps 1 through 6 are built; step 4 (deleting the PAT
secret and revoking the PAT) was done by calef the same day. The App exists and is installed: scheduled
workflows author as `nife-smelter[bot]`, and the toolchain-bump pull requests since at least
2026-09-28 (#1436, #1439, #1523) carry that author. `toolchain-bump.yml` tries an App installation
token first, falls through to `github.token`, and prints which rung it took.
`TOOLCHAIN_BUMP_PAT` was the only repository secret `gh secret list` showed until calef deleted it on 2026-10-03. Minted 2026-08-15 at calef's request, the same day he set
`TOOLCHAIN_BUMP_PAT` on the transferred repository and asked what other developers would need
(answer: nothing, and that answer is what surfaced this).

What is left is the agent sessions. Since 2026-10-07 (UTC) the mechanism that moves a session onto
the App is built, opt-in and inert until calef widens the App and installs its key on patagonia
(`notes/session-identity.md`). On 2026-10-24 (UTC) he ruled to run those steps himself soon (see
Follow-on). The secret names were ratified the same day. The App secrets are no longer
organization-level; see the 2026-10-03 line under the Done items below.

In brief. The toolchain-bump workflow authenticates as a fine-grained personal access token
(PAT) on calef's account, because a PR opened by the ephemeral `GITHUB_TOKEN` triggers no CI
(GitHub's anti-recursion rule; the workflow's own comment records it). A PAT works and has two
structural flaws: it expires on a personal timer, with "bump PRs silently stop getting checks" as
the only symptom, and it couples the project's automation to one person's account, so the
automation breaks the day that account leaves the organization. A **GitHub App** owned by
`crickertech`, installed on `nife`, fixes both: installation tokens are minted fresh per workflow
run (nothing stored expires), and the identity belongs to the organization rather than to a
person. This is the `needs-architect` principle applied to credentials: name the role, not the
person.

## The second deliverable: attribution, added 2026-08-16

calef, reading a repository whose every pull request, comment and review carries his name:
"it looks like I'm talking to myself a lot and the record would be nice to clarify who is
talking." That is a documentation defect as much as a vanity one. This project's whole claim is
that a system of this size can be built by one architect and many agents. A timeline in which the
architect appears to write, review and merge his own work in a single voice is evidence *against*
the claim it should be evidence for. The provenance tenet applies to authorship the same way it
applies to names: the record should say who did a thing where a reader meets it.

Same App, so the setup is shared. An installation token authors as `<app-name>[bot]` with its
own avatar and badge, so a lane's pull request is visibly not the architect's. What exists today
is half a mechanism: commits carry `Co-Authored-By: Claude ...` and pull request bodies carry the
Claude Code footer, but the *author* of every pull request and comment is calef's account, and
that is the half a reader actually sees.

The alternatives, recorded so the choice is one:

- A separate machine account (`nife-agent` or similar) works today with no App and reads
  unambiguously. Cost: a second identity to secure, an org seat, and the same
  `GITHUB_TOKEN`-cannot-trigger-CI trap the App was minted to escape.
- Convention only, a `**Lane:**` line in the body naming the agent and its milestone. Free,
  cosmetic, and adopted immediately (2026-08-16) as an interim rather than as the answer, because
  a line of prose is rung four and an identity is rung two.

**What it does not fix, and should say so plainly:** an App cannot make the *commits* authored by
the agent, only the pull request and its comments; commit authorship stays with whoever holds the
git identity. `Co-Authored-By` remains the record there, and that split is honest rather than
awkward: the human owns the change, the agent is named as its co-author, and the conversation
around it is visibly the agent's.

## The work, which is small

1. Create the App under the `crickertech` organization (calef; Settings → Developer settings →
   GitHub Apps). No webhook, no public listing. Repository permissions: Contents read/write,
   Pull requests read/write, the same pair the PAT carries.
2. Install it on `nife` only, and store the App ID and private key as repository secrets
   (`AUTOMATION_APP_ID`, `AUTOMATION_APP_KEY`).
3. Done 2026-09-23. In `toolchain-bump.yml`, mint the installation token per run (the maintained
   `actions/create-github-app-token` action does exactly this, and taking it is a workflow-only
   dependency, not one in the shipping graph). Use it where `TOOLCHAIN_BUMP_PAT` is used today.
   Note it in the §46 (thin primitives or whole subsystems) spirit anyway. Keep the
   `|| github.token` fallback: a fork without the App still opens PRs.
4. Delete the `TOOLCHAIN_BUMP_PAT` secret and revoke the PAT, in that order, and update the
   workflow's own explanatory comment, which is where this mechanism is documented for the next
   reader. **Not done, and deliberately sequenced after a run has been observed taking the App
   rung**: revoking a credential the workflow may still be reaching for turns a preparation into
   an outage. `notes/automation-identity.md` carries the order and the reason. **Update 2026-10-03
   UTC:** every workflow now takes the App rung only (plus `github.token` in `toolchain-bump.yml`);
   none reads the secret. Deleting it and revoking the PAT is calef's.
5. Any future workflow needing to trigger CI on its own PRs reuses the same App rather than
   minting another personal token; that reuse is the milestone's compounding value. The two steps
   to copy are in `notes/automation-identity.md`'s `EXAMPLES`.

6. Added 2026-09-23, because an inert change and a broken change look identical. The job prints
   which of the three identities it authenticated as and proves that token is live. Without it, a
   reader cannot tell an App run from a PAT run from a fallback run, and the PAT's expiry stays as
   quiet as the milestone says it is. This is the step that made the fallback observable rather
   than argued: dispatched on the lane's branch with no App secrets present, the job reported
   `identity: TOOLCHAIN_BUMP_PAT` and `token check: OK`, which is the fallback working.

## Why not now, and what the 2026-09-23 audit of that found

The original deferral, written 2026-08-15 and kept here because it is the reasoning the audit
below tests. With one architect, the PAT and the App fail in the same circumstances and the PAT
already exists. The App earns its setup cost at the first of **a second architect joining**,
the PAT's first silent expiry, or a second workflow needing the same authority.

Audited 2026-09-23, each trigger against the tree rather than against a recollection. None has
fired outright; the third is half-fired, and the audit's own difficulty is the finding.

- A second architect: no. One architect. Stated and moved past.

- **The PAT's first silent expiry: not yet, and the project cannot see how close it is.** The
  evidence that it is alive is positive rather than inferred. `toolchain-bump.yml` has run and
  succeeded every day through 2026-09-23; that day's proposal, pull request #1112, was authored by
  `calef` (the PAT's identity, not `github-actions[bot]`), collected sixteen checks including
  `build + test`, `prove` and `cpu matrix`, and merged. A PAT that had expired would have produced
  either a red checkout or a pull request under the ephemeral token with no CI, and neither
  happened. What cannot be established from inside the repository is when it expires: a
  repository secret is opaque to every API this project can call, and the expiry is visible only to
  the account that minted it. The milestone said the symptom would be silent; the audit's actual
  result is stronger and worse, which is that *the warning* is silent too. There is no way to
  schedule around this trigger, only to be hit by it. That asymmetry is the best argument in this
  block for doing the work before the trigger rather than on it.

- A second workflow needing the authority: half. Two workflows calling `gh` landed after this
  milestone was minted, and they are not the same case as each other.
  - `architect-hold.yml` genuinely needs nothing beyond `GITHUB_TOKEN`. It is read-only (one
    `gh api` call for a pull request's labels) and creates no events, so the anti-recursion rule
    does not reach it. This is the case the milestone's trigger would have over-counted: "calls
    `gh`" is not "needs a PAT".
  - `coe-architect-label.yml` is the half. It **writes**: it adds `needs-architect` to a pull
    request that adds a correction-of-error record. An event created by `GITHUB_TOKEN` triggers no
    workflow run, and GitHub's rule is not limited to `push` and `pull_request`, so the `labeled`
    event this job creates does not re-run `architect-hold.yml`. The required check on the pull
    request page therefore stays at whatever it last reported, which is green, while the label says
    the opposite. The gate itself still holds, because `architect-hold.yml` also runs on
    `merge_group` and reads labels fresh from the API, so the merge queue catches it and evicts the
    entry. So this is a display defect rather than an escape. When this was written the labeler
    had never fired, because `notes/corrections/` was not yet on `main`. *(Corrected 2026-10-03:
    §210 (a correction of error, and its action items are decisions, proposals or milestones) is
    DECIDED, the labeler fired on #1247 on 2026-09-24, and correction-of-error records moved to
    `notes/coes/` on 2026-09-30, which the workflow has watched since #1515.)* It is a second workflow that **would** be better on the App, not one
    that is broken without it.

Verdict: still parked as a forced move, and no longer parked as a cheap one. No trigger
compels the work today. The preparation that does not need owner rights was done anyway, because
it costs nothing to carry (the App rung is inert without its secrets). It also converts the
expiry from an outage into a fifteen-minute procedure someone can run without reading this block.

## BUGS

- The private key is still a stored secret. An App swaps a stored *token* for a stored
  *signing key*; the win is org ownership and per-run minting, not the absence of a secret. A
  leaked key is revoked in the App's settings, which is at least an org-level act rather than a
  personal-account one.
- Bot-authored PRs change the byline. Bump PRs would arrive as `<app-name>[bot]` rather than
  as calef; anything filtering PRs by author (none known in-tree today) would need updating. The
  workflow's own `**Lane:**` line is computed from which identity it took, for the same reason: a
  pull request that says calef's account is its author is false the day the App lands.

- Nothing checks the App's permissions after it is installed. The identity probe proves the
  token can read the repository; it cannot prove the installation still holds exactly Contents and
  Pull requests write, or that nobody widened it in the web UI.

- The App's display name may already belong to somebody else. `smelter` is ratified (calef,
  2026-09-23), but App names are globally unique across the whole of GitHub rather than within an
  organization, and that is discovered at the create-App screen and nowhere earlier. The fallback
  is `nife smelter`, rendering `nife-smelter[bot]`; `notes/automation-identity.md` carries both and
  says to bring a third choice back to calef rather than invent one.

## Follow-on

- **Done.** Steps 1 and 2: the App `smelter` exists under the organization and is installed on
  `nife` (the organization was `crickertech` when this block was written and is `nifeos` now).
  Evidence, 2026-10-03: pull request #1523, opened by the toolchain-bump workflow, is authored by
  `nife-smelter[bot]`, and so are #1436 and #1439. `notes/automation-identity.md` was the procedure.

- **Done.** Step 4, 2026-10-03 (UTC): #1562 moved `metrics.yml` and `vendor-watch.yml` onto the App
  token and removed `toolchain-bump.yml`'s PAT fallback. Metrics run 37159312144 and vendor-watch run
  37159313976 (both `workflow_dispatch` from `main`) then succeeded, and the metrics run opened #1563
  as `app/nife-smelter`, with CI started (26 checks passed when this was written). calef then deleted
  the repository secret `TOOLCHAIN_BUMP_PAT` and the fine-grained token from his account.

- **Done.** The App's display name is **`smelter`**, ratified 2026-09-23 by calef, with
  `nife smelter` as the fallback if the global namespace has taken the first. It is an agent noun,
  matching this tree's convention for actors (`caretaker`, `undertaker`, `credentialer`,
  `compositor`), and a smelter is where iron comes from. Carried by `notes/automation-identity.md`,
  whose name block holds the date, the ratifier and the five refusals (`nife automation`, `dynamo`,
  `mantle`, `lodestone`, `cobble`).

- **Done.** The two secret names, `AUTOMATION_APP_ID` and `AUTOMATION_APP_KEY`, ratified as-is by
  calef on 2026-10-24 (UTC): *"Ratify as-is"*. They name the role, not the App, so they survive a
  rename of the App. Recorded by nifeos/nife#2695 here and in `notes/automation-identity.md`, whose
  own name stays provisional. No workflow comment called them provisional.

- **Done.** On 2026-10-03 (UTC) calef deleted the organization-level Actions secrets
  `AUTOMATION_APP_ID` and `AUTOMATION_APP_KEY` from `nifeos`. The App's ID and key now exist only
  as secrets of the repository environment `automation`. Every workflow that reads them
  (`merge-drain.yml`, the `propose` job of `toolchain-bump.yml`, `trunk-health.yml`) does so from a
  job that declares `environment: automation`. `trunk-health` run 37149031146 succeeded on the
  environment secrets before the deletion, and run 37156264104 (`workflow_dispatch`, 2026-10-03
  21:46 UTC) succeeded after it. `TOOLCHAIN_BUMP_PAT` was deleted later that day (see Step 4 above).

- **Recorded.** `coe-architect-label.yml` adds `needs-architect` with `GITHUB_TOKEN`, whose
  `labeled` event re-runs no workflow, so `architect-hold.yml`'s required check on the pull request
  page stays stale-green while the label says otherwise. The merge-queue run of the same check
  still catches it, so this is a display defect and not an escape. It lives in the BUGS section of
  `notes/automation-identity.md`, beside the mechanism that would fix it.

- **Outstanding.** Built and waiting on calef: agent sessions act as the App, added 2026-10-07 (UTC)
  after a security audit found every lane held calef's classic `gh` token and SSH key.
  `helpers/smelter-token` (provisional) mints an hour-long installation token in a `lane` or
  `maintainer` scope, for a session that opts in with `NIFE_SMELTER=1`; it changes that session's
  environment only, never a configuration file. Every workflow's App token is pinned to Contents
  and Pull requests write so widening the App reaches none of them. Wiring Claude Code and opencode
  to opt in automatically is a later change, after a trial. `notes/session-identity.md` has calef's
  steps and the kill switch. calef ruled the four forks on #1828 on 2026-10-07 (UTC): opted-in
  sessions author as `nife-smelter[bot]`. One App with `lane` and `maintainer` scopes. Installed
  on All repositories in `nifeos`. His classic token left as it is, with confinement recorded as
  the fix in that note's BUGS. Ruled and scheduled, no longer waiting on a decision (calef,
  2026-10-24 UTC: *"Do the steps soon"*): he runs the five steps of that note's "Switching it on"
  himself. Then a lane trials one session, and only after it wires the harnesses.

- **Done.** The second deliverable (attribution) needs nothing further from a lane. The interim
  `**Lane:**` convention is in `AGENTS.md` and applied; `toolchain-bump.yml` now computes that line
  from which identity the run took, so it stops claiming calef's byline the moment the App lands.
  Carried by this milestone's own commits.

## Index row

The toolchain-bump PAT expires on a personal timer and couples the project's automation to one
account; an org-owned App names the role, not the person. Deliberately parked until a second
architect, the PAT's first expiry, or a second workflow needing the authority
