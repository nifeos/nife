# The automation's own identity

*Name of the App: ratified 2026-09-23 (calef, milestone 128 (the automation gets its own identity)).
`smelter`, hence the byline `smelter[bot]`. Refused `nife automation` (the lane's original: it names
the mechanism rather than what the thing is, which is the failure mode `design/naming.md` warns
about). Refused `dynamo` (the maintainer's recommendation, on the grounds that the geodynamo is what
a nickel-iron core actually does; calef chose otherwise). Refused `mantle` (carries "take up the
mantle", which is this milestone's own role-not-person thesis, but sits beside the namesake rather
than in it). Refused `lodestone` (connotes direction-finding rather than doing work), and `cobble`
(the mascot; a byline sharing the mascot's name would confuse the two).*

*Why `smelter` won, stated rather than implied: it is an agent noun, which is this tree's existing
convention for actors (`caretaker`, `undertaker`, `credentialer`, `compositor`). And a smelter is
where iron comes from, which suits the identity that will eventually author this project's
artifacts. The maintainer argued against it on literal process-matching grounds and withdrew the
objection: the tree's actor names are metaphors already, so convention-match is the stronger test.*

*Name of this note: provisional. `automation-identity` is a lane's coinage; `design/naming.md` is
the rule and an architect ratifies. One alternative considered was `github-app.md` (names the
vendor's mechanism rather than what it is for, and this note would survive a move off GitHub only by
lying). The other was `bot-identity.md` (`bot` is what GitHub calls the badge, not what the thing
is). The two secret names below, `AUTOMATION_APP_ID` and `AUTOMATION_APP_KEY`, were ratified as-is
on 2026-10-24 (calef, milestone 128): they name the role, not the App, so survive its rename.*

The daily toolchain-bump workflow has to open a pull request that gets CI. GitHub's
anti-recursion rule says a pull request opened with a workflow's own ephemeral `GITHUB_TOKEN`
triggers no further workflow runs, so such a pull request sits with every required check stuck on
"Expected", permanently unmergeable. The workflow therefore authenticates as something else.

That something else is a GitHub App, not a personal access token, because a token on calef's account
has two structural flaws. It expires on a personal timer that nobody reading the workflow can
inspect, and it couples the project's automation to one person's account. A GitHub App owned by the
`nifeos` organization, installed on `nife`, fixes both. Its installation tokens are minted fresh per
run, so nothing stored expires, and the identity belongs to the role rather than to the person. That
is `needs-architect`'s principle applied to credentials.

The App exists: `smelter`, App ID 5053502, created by calef under `nifeos` and installed on `nife`
alone. Its two secrets exist only as secrets of the environment `automation`, which the workflows
name (`environment: automation`), not as organization secrets; steps 10 and 11 below still say
organization secrets, so store them in the environment instead. This note is written as a procedure, because the next person to run it will be
provisioning a second App or replacing a lost key, and the steps are the same either way.

## What it does not fix

Commit authorship. An App's installation token authors the *pull request* and its *comments*,
not the commits inside it. A commit's author is whoever holds the git identity at the moment
`git commit` runs, which in `toolchain-bump.yml` is `github-actions[bot]` by explicit
`git config`, and in a lane's worktree is calef. `Co-Authored-By:` stays the record of who actually
wrote a commit, and that split is honest rather than awkward: the human owns the change, the agent
is named as its co-author, and the conversation around the change is visibly the agent's.

Anything else that authenticates as calef. The App covers the workflows given its token, and agent
sessions on a machine holding its key ([session-identity.md](session-identity.md)). Without that
key a lane's `gh` still runs as calef's login, which the `**Lane:**` line says.

## Why `smelter` has no repository of its own

calef asked, 2026-09-23. No, and the reason was never scope. A GitHub App is a registered
identity plus a permission set, held at the organization level; it has no code unless it has a
webhook handler, and this one deliberately has none. Its only artifacts are an App ID and a private
key, both secrets. A repository for it would hold an empty tree and a README pointing back at this
note. The breakup of the monorepo does not change that, because an App without a webhook still
has no code to put anywhere.

Two things would change the answer, and either is enough. The App grows a webhook handler, a service
receiving events and acting on them, which is real code with its own deploy and tests and does not
belong inside an OS repository. Or it grows custom actions shared by several repositories and
needing their own release cadence. Note what is *not* on that list. The milestone's stated reuse, a
future workflow copying the two steps from `EXAMPLES` below, is copied steps rather than shared
code, and triggers neither. That distinction is the whole reason this is written down: "reuse is the
compounding value" reads like an argument for a repository, and it is not one.

What the split does change is where this note lives, and that is an open question with a
trigger rather than a decision to make now. `smelter` is expected to serve several repositories.
§151 (the goal of the repository split is independent release and third-party programs) is DECIDED.
Milestone 39 (repository structure for a loosely-coupled OS) names option C as the destination, and
milestone 120 (the OS becomes `nife`, and the project gets an organization) already put the manifest
in a separate repository, `basalt`. Once `nife` is one repository among several it stops being the
obvious home for organization-wide tooling documentation, and `basalt` is the plausible destination
for this page. The move happens at the split, not before, because today the argument for keeping it
here is the stronger one. It sits beside `toolchain-bump.yml`, the workflow it documents, so the two
version together and a change to that workflow cannot silently invalidate this page. When it moves,
§201 (one roadmap until a citation has to cross) already governs how the citations survive crossing
a repository boundary, so this is an existing framework rather than a new problem.

## Creating the App

Once, by an owner of the `nifeos` organization.

1. Go to https://github.com/organizations/nifeos/settings/apps and press New GitHub App.
   (The path by clicking: your avatar, Your organizations, `nifeos`, Settings,
   Developer settings, GitHub Apps, New GitHub App.)
2. GitHub App name: type `smelter`. Ratified 2026-09-23 by calef; the refusals and the
   reasoning are in the name block at the top of this note.

   It may be taken. App names are globally unique across the whole of GitHub, not just this
   organization, and a one-word English name very likely already belongs to somebody. GitHub tells
   you at this screen and nowhere earlier. If it is unavailable, use `nife smelter`, which
   renders as `nife-smelter[bot]`. Do not invent a third; bring it back to an architect.

   Whichever of the two you get, the byline (`smelter[bot]` or `nife-smelter[bot]`) is what every
   future bump pull request is authored by, so write down which one you took.
3. Homepage URL: `https://github.com/nifeos/nife`. It is required and unused.
4. Webhook: untick Active. Nothing here listens for webhooks, and an inactive webhook is one
   fewer endpoint to secure.
5. Repository permissions, exactly two, matching what the PAT carries today and nothing wider:
   - Contents: Read and write (push the `toolchain/nightly-bump` branch).
   - Pull requests: Read and write (open, retitle and un-draft the proposal).
   Leave every other permission at No access. Leave organization and account permissions alone.
6. Where can this GitHub App be installed?: Only on this account. It is not a public listing.
7. Press Create GitHub App.

## Installing it and storing the secrets

8. On the App's settings page, note the App ID near the top. For `smelter` it is 5053502,
   created 2026-09-23 by calef; this step is done. Then Generate a private key, which downloads a `.pem` file.
   GitHub never shows it again; a lost key is regenerated rather than recovered.

   The App ID is not sensitive and the private key is the only thing here that is. An App ID is
   visible to anyone who can see the App, which is why it can be written down in this note while
   the key never is; finding one in a log or a diff is not an incident.

   The Client ID on the same page is not used. `actions/create-github-app-token` authenticates
   with the App ID and the private key; a Client ID exists for OAuth flows, which this App does not
   do. It is deliberately not recorded here, because a value nothing reads is a value that rots.
9. In the left sidebar, Install App, then Install next to `nifeos`. Choose Only select
   repositories and pick `nife` alone. Never "All repositories", then or later: add
   repositories to this list as they appear. An App installed on every repository holds authority
   over repositories it has no business in.
10. Store both as organization secrets, scoped to exactly the repositories that may read them.
    The commands are:

        gh secret set AUTOMATION_APP_ID  --org nifeos --repos nife --body '5053502'
        gh secret set AUTOMATION_APP_KEY --org nifeos --repos nife < ~/Downloads/smelter.private-key.pem

    Expect them to fail, and do not fix it the way `gh` suggests. Unless your shell's token
    already carries organization admin, both return:

        failed to fetch public key: HTTP 403: You must be an org admin or have the
        actions secrets fine-grained permission.
        This API operation needs the "admin:org" scope.

    `gh` offers `gh auth refresh -h github.com -s admin:org`, and it works. Do not run it. That
    token is shared by every agent on this machine, and widening it permanently for a one-time task
    grants organization-admin to automation, and many lanes use that token while merging
    unattended. Least privilege says do the one-time thing by hand. The reasoning is the
    durable part of this paragraph; the clicks below are not.

    So use the browser. https://github.com/organizations/nifeos/settings/secrets/actions,
    then New organization secret, twice:

    - `AUTOMATION_APP_ID` = `5053502`
    - `AUTOMATION_APP_KEY` = the whole `.pem`, including both the `-----BEGIN` and `-----END` lines
      and the trailing newline.

    Set Repository access: Selected repositories → `nife` on both. To get the key into the
    clipboard without opening it in an editor, and to remove it afterwards:

        pbcopy < ~/Downloads/*.private-key.pem
        rm ~/Downloads/*.private-key.pem

    You cannot confirm this from the shell either, and that is not something being broken:
    `gh secret list --org nifeos` needs the same `admin:org` scope. It returns the same 403 for any
    shell that is not an organization admin's, which is the ordinary case rather than a fault. On
    the browser path the secrets page itself is the confirmation, and the first real proof is the
    identity probe in step 11, once the workflow change has merged.

    This is the path that was actually used. Both `smelter` secrets were stored this way by
    calef in the browser on 2026-09-23, after the `gh` commands above returned the 403.

    Storing them early is safe, and the argument is structural rather than an observation. Until
    the workflow change has merged, `main`'s `toolchain-bump.yml` contains no reference to
    `AUTOMATION_APP_ID` or `AUTOMATION_APP_KEY` at all, so the secrets cannot reach it whatever they
    contain; the daily bump keeps using the PAT. There is no window in which a half-provisioned App
    can change what the scheduled job does.

    Why a non-secret is stored as a secret, since the App ID is public and this looks wrong at
    first reading. The workflow uses `AUTOMATION_APP_ID`'s *emptiness* as its inertness switch
    (`if: env.AUTOMATION_APP_ID != ''`), so what is wanted is a per-repository value that is absent
    until someone provisions it, and a secret is the kind of value the workflow already reads. A
    repository or organization variable would serve that switch equally and read more honestly,
    since it would not claim to protect something that needs no protection. It is provisioned as a
    secret and stays that way; the alternative is noted here so the next reader does not have to
    work out whether the choice was considered.

    Either way, delete the downloaded `.pem` afterwards. (calef deleted his, and keeps a copy in his Keychain.) A key sitting in a downloads folder is the
    leak this whole exercise is meant to reduce.

    Both secret names were ratified as-is on 2026-10-24 (UTC) by calef. `AUTOMATION_*` says what
    the secrets are for rather than what the App is called, which survives the App being renamed.

## Confirming it took, and only then retiring the PAT

11. Run the workflow by hand and read its identity probe:

        gh workflow run toolchain-bump.yml --repo nifeos/nife
        gh run watch "$(gh run list --workflow=toolchain-bump.yml --repo nifeos/nife \
              --limit 1 --json databaseId --jq '.[0].databaseId')"

    The Say which identity this run is authenticating as step must print
    `identity: the nifeos automation App`. If it prints the PAT line instead, the secrets are
    not visible to the job: check the names, and check that the organization secrets list `nife`
    among the repositories that may read them.

12. Done 2026-10-03 UTC. `toolchain-bump.yml`, `metrics.yml` and `vendor-watch.yml` stopped
    reading `TOOLCHAIN_BUMP_PAT` (#1562). Metrics run 37159312144 and vendor-watch run 37159313976
    (both `workflow_dispatch` from `main`) then succeeded, and the metrics run opened #1563 as
    `app/nife-smelter`, with CI started. calef then deleted the secret
    (`gh secret delete TOOLCHAIN_BUMP_PAT --repo nifeos/nife`) and the fine-grained token itself.
    Nothing is left to revoke.

## The one dependency, and why it was taken

`actions/create-github-app-token`. Approved by calef, 2026-09-23 UTC. §46 (thin primitives or
whole subsystems) makes taking a dependency a decision rather than a convenience, so it is recorded
rather than merely described. It is **maintained by GitHub** and does exactly one thing, mint an
installation token per run. It is **outside the shipping graph**: it runs in CI and no part of the
OS depends on it, which is the distinction §46 draws. **The alternative was worse**, and this is the
case where §46's "write it yourself" guidance does not apply. Minting the token by hand means
signing a JWT with the private key and exchanging it, in shell, inside a workflow: more code, in the
least testable place this project has, handling a signing key. And **the fallback chain survives its
absence**, since the step is skipped entirely when the App secrets are unset, so the action is
load-bearing for neither the PAT path nor the `github.token` path.

## EXAMPLES

**Read which identity yesterday's bump used.** The probe is the first thing in the job, so this
needs no scrolling:

    gh run list --workflow=toolchain-bump.yml --repo nifeos/nife --limit 1 \
      --json databaseId --jq '.[0].databaseId' \
      | xargs -I{} gh run view {} --repo nifeos/nife --log \
      | grep -A4 'identity:'

**Check whether the bump pull request is actually getting CI**, which is the symptom the whole
mechanism exists to prevent. A bump pull request with one or two checks is a pull request opened by
the ephemeral token:

    gh pr list --head toolchain/nightly-bump --repo nifeos/nife --state all --limit 1 \
      --json number --jq '.[0].number' \
      | xargs -I{} gh pr view {} --repo nifeos/nife \
      --json author,statusCheckRollup \
      --jq '{author: .author.login, checks: (.statusCheckRollup | length)}'

On 2026-09-23 that reported `{"author":"calef","checks":16}`: the PAT path, working.

**Use the same App from a second workflow.** Copy the two steps, nothing else. The fallback chain is
per-workflow on purpose, so a workflow that has no business pushing can take the App's token without
inheriting a reason to keep a PAT beside it:

    env:
      AUTOMATION_APP_ID: ${{ secrets.AUTOMATION_APP_ID }}
    steps:
      - id: app-token
        if: env.AUTOMATION_APP_ID != ''
        uses: actions/create-github-app-token@v2
        with:
          app-id: ${{ secrets.AUTOMATION_APP_ID }}
          private-key: ${{ secrets.AUTOMATION_APP_KEY }}
          permission-contents: write
          permission-pull-requests: write

## BUGS

- **The private key is still a stored secret.** An App swaps a stored *token* for a stored *signing
  key*. The win is organization ownership and per-run minting, not the absence of a secret. A leaked
  key is revoked in the App's settings, which is at least an organization-level act rather than a
  personal-account one.

- **The App's own permissions are not checked by anything.** Nothing in this tree verifies that the
  installation still holds Contents and Pull requests write, or that it has not quietly been granted
  more. If someone widens them in the web UI, no gate here notices. The identity probe proves the
  token can read the repository; it does not prove the permission set is the one this note describes.

- **The `if: env.AUTOMATION_APP_ID != ''` guard tests presence, not validity.** A secret set to the
  wrong App ID, or a private key that does not match it, takes the App rung and then fails the
  `create-github-app-token` step outright. That is loud rather than silent, which is the right
  failure, but the diagnostic comes from the action rather than from this note.

- **A bot byline changes what author filters see.** Bump pull requests will arrive as
  `smelter[bot]` (or `nife-smelter[bot]`) rather than as `calef`. Nothing in this tree filters pull requests by author
  today, and `helpers/merge-drain.sh` and `helpers/needs-maintainer.jq` are the files to re-read
  if that changes.

- This note cannot tell you when the PAT expires, and neither can anything else in the
  repository. An Actions secret's value is opaque to every API the project can call, and only the
  account that minted the token can see its expiry at https://github.com/settings/tokens?type=beta.
  That opacity is not a gap in this note, it is the flaw the App removes.
