# Agent sessions act as the App, not as the architect

*Name of this note, of `helpers/smelter-token` and of `helpers/smelter-bin/`: provisional, minted
2026-10-07 (UTC) by the lane for milestone 128 (the automation gets its own identity). It extends
[automation-identity.md](automation-identity.md), which covers the App itself and the workflows,
and is already over the prose budget.*

## Turning it off, from a plain terminal

Nothing here is persistent outside a session that opted in. No file in `.git/config`,
`~/.gitconfig` or gh's configuration is written; the opt-in lives only in that session's
environment. So any of these, none of which needs a tool this change added, returns everything to
the state before it:

- One session: start it again without `NIFE_SMELTER=1`. Its `gh` and `git` use calef's login and
  SSH key exactly as before.
- Every session on the machine: `security delete-generic-password -s nife-smelter -a private-key`.
  An opted-in session then refuses `gh` once its cached token expires, within the hour. Delete the
  cache too (`rm -rf ~/Library/Caches/nife-smelter`) to make that immediate.
- The identity everywhere: on GitHub, organization Settings, GitHub Apps, `nife-smelter`, Suspend.
  That stops every token it has minted, workflows' included, until it is unsuspended.

A shell that never set `NIFE_SMELTER=1`, calef's own terminal among them, is never affected.

## What calef ruled

All four on 2026-10-07 (UTC), on #1828:

- A session that opts in acts as `nife-smelter[bot]`: its pull requests, comments, labels and
  pushes. Commits keep calef's git identity, with `Co-Authored-By:` naming the model.
- One App with two scopes, `lane` by default and `maintainer` only when the command sets
  `NIFE_SMELTER_SCOPE=maintainer`. A guard against accident, not a boundary (BUGS).
- The App is installed on All repositories in `nifeos`, and goes back to selected if a repository
  that should be off-limits to agents ever appears.
- calef's classic `gh` token stays as it is: no regeneration and no logout (BUGS).

## The gap this closes

Without it, an agent session on patagonia runs `gh` and `git push` as calef. His classic `gh` token
carries the scopes repo, workflow, admin:public_key, gist and read:org, so such a lane holds a
credential that reaches his personal repositories and his SSH keys. Every pull request, comment and
label it makes carries his name. The scheduled workflows run as the `nife-smelter` App.

## How a session becomes the App

`helpers/smelter-token` signs a JSON Web Token (JWT) with the App's private key, finds the App's
installation on `nifeos`, and exchanges the JWT for an installation token that lives an hour. It
caches the token per scope (mode 0600, under `~/Library/Caches/nife-smelter/`) and remints it with
ten minutes left. The key goes to `openssl` on a pipe and is never written to disk.

A session opts in by setting `NIFE_SMELTER=1`. Then `helpers/smelter-token env` prints environment
variables for that session, as shell lines (`env --json` gives JSON). Without the opt-in it prints a
comment and changes nothing, whether or not the key is installed. The variables do three things:

- They put `helpers/smelter-bin/gh` first on PATH. That shim hands the real `gh` a fresh token as
  `GH_TOKEN`, and refuses to run, rather than fall back to the stored login, when it has none. It is
  on PATH only in an opted-in session.
- Through `GIT_CONFIG_COUNT` and its key and value pairs, they make the helper git's only credential
  helper for `https://github.com`. Any configured before it is cleared, because the macOS system
  git asks osxkeychain, which may hold a personal credential.
- The same way, they rewrite `nifeos` SSH remotes to HTTPS, so the SSH key is never offered to the
  organization. Remotes outside it are left alone.

The selftest checks the "nothing persistent" claim mechanically. It runs every subcommand with an
empty home and working directory and fails if either holds a file afterward, or if a stand-in global
git config changed; a mutation that writes `git config --global` fails it.

## Opting a session in, in any harness

An agent's shell tool starts a fresh shell per command, so the variables must reach each command.
The one form that works in every harness is a prefix:

    eval "$(NIFE_SMELTER=1 helpers/smelter-token env)"; gh pr list

Wiring a harness to apply it to every command (Claude Code's `SessionStart` hook and
`CLAUDE_ENV_FILE`, opencode's `shell.env` plugin hook) is deliberately left to a later change, after
a trial on single sessions. `helpers/smelter-token check` exits 1 with a warning when a session set
`NIFE_SMELTER=1` but the variables are not applied. `script/claim` runs it at every lane's first act;
a session that did not opt in sees nothing.

## Two scopes, one App

A token is minted with one of two permission sets, and GitHub enforces the one asked for.

| Scope | Permissions | How a session gets it |
|---|---|---|
| `lane` | Contents, Pull requests, Issues, Actions, Workflows write; Checks, Commit statuses read | the default |
| `maintainer` | the above, plus Administration write | `NIFE_SMELTER_SCOPE=maintainer` in the command |

Administration is what repository settings, rulesets and the merge queue's configuration need.
Keeping it out of the default means a settings change names itself on its own command line.
Every workflow's token names Contents and Pull requests write explicitly, which was the App's whole
set before this, so widening the App for sessions widens no workflow.

## Switching it on (calef, in order)

1. Widen the App at https://github.com/organizations/nifeos/settings/apps/nife-smelter/permissions.
   Repository permissions: Actions, Administration, Contents, Issues, Pull requests and Workflows
   Read and write; Checks and Commit statuses Read-only; Metadata stays Read-only. Save, then
   approve the pending request on the installation at
   https://github.com/organizations/nifeos/settings/installations.
2. On the same installation page, set Repository access to All repositories. An installation
   belongs to one organization, so this reaches every `nifeos` repository as the monorepo splits,
   and nothing outside it.
3. On the App's General page, Generate a private key. It is a second key, kept apart from the one in
   the `automation` environment so either can be revoked alone. Store it in the login keychain,
   base64 on one line (`security -w` prints a value containing newlines as hex), then delete the
   download:

       security add-generic-password -s nife-smelter -a private-key -T /usr/bin/security \
         -w "$(base64 < ~/Downloads/nife-smelter.*.private-key.pem | tr -d '\n')"
       rm ~/Downloads/nife-smelter.*.private-key.pem

   `-T /usr/bin/security` lets the helper read it without a prompt on every mint.
4. Check it from the repository: `helpers/smelter-token identity` prints `nife-smelter[bot]`. Then
   `NIFE_SMELTER_SCOPE=maintainer helpers/smelter-token token >/dev/null && echo ok` prints `ok`; an
   HTTP 422 there means step 1's Administration was not approved.
5. Try one session: `eval "$(NIFE_SMELTER=1 helpers/smelter-token env)"; gh api /installation/repositories --jq .total_count`
   answers only for the App. Wiring a harness comes after the trial.

To switch it off, see the first section; revoke the key on the App's General page as well.

## What changes once pull requests are the App's

Checked against the tree and the `main` ruleset on 2026-10-07 (UTC):

- Nothing in `.github/workflows/` or `helpers/` filters on a pull request's author or on the actor
  of an event. The ruleset has no bypass actors and requires no approving review. So the queue and
  the required checks treat a bot-authored pull request as they treat calef's.
- Events an App token creates do start workflows, unlike `GITHUB_TOKEN`'s, so a label a lane adds
  reruns `architect-hold.yml` on the pull request page.
- calef can review and approve lane pull requests. GitHub refuses a review request from a pull
  request's own author, which is why `notes/skills/decisions/SKILL.md` says a review request
  "silently does nothing" today.
- `helpers/session_steps.py` (milestone 642 (the record should say whether a person or the
  machinery took a step)) counts `calef`-actor events. After the switch a session's events carry
  `nife-smelter[bot]`, so a `calef` event is calef's own.
- Commits stay authored by the git identity, calef, with `Co-Authored-By:` naming the model. The
  ruleset's extra approval for unattributed changes looks at commit authors, so it is untouched.

## EXAMPLES

```sh
helpers/smelter-token identity                       # nife-smelter[bot]
helpers/smelter-token check                          # warns if opted in but not applied
eval "$(NIFE_SMELTER=1 helpers/smelter-token env)"; gh pr list   # one opted-in command
GH_TOKEN="$(helpers/smelter-token token)" gh api /repos/nifeos/nife --jq .full_name
NIFE_SMELTER_SCOPE=maintainer gh api -X PATCH /repos/nifeos/nife -F allow_auto_merge=true
helpers/smelter-token --selftest                      # no network, no keychain
```

## BUGS

- The two scopes are a guard against accident, not a boundary. Every session runs as calef's macOS
  user, so a session can read the App key from the keychain and mint `maintainer` itself, and can
  read calef's `gh` login and SSH key the same way. What changes is what a session uses by default
  and what a leaked token reaches: the `nifeos` organization, for an hour. A boundary needs the key
  out of the session's reach (another macOS user, or a sandbox that denies the keychain).
- calef's classic `gh` token stays on the machine, as he ruled on 2026-10-07 (UTC), and stays
  within reach of any session (`gh auth token` works for whoever runs it as his user). This is
  the audit finding left open; confinement, the key and the token out of a session's reach, is
  its fix.
- No harness-neutral automatic hook exists, and this change wires none. Until a later change wires
  Claude Code and opencode after a trial, a session is on the App only for commands that carry the
  `eval` prefix. A session that set `NIFE_SMELTER=1` and forgot the prefix acts as calef; `check`
  says so at `script/claim`, but nothing refuses it.
- Deleting the key does not stop an opted-in session at once: the shim serves the cached token until
  it expires, within the hour. The first section says how to make that immediate.
- Step 3's command puts the base64 key on `security`'s argument list while it runs, visible to
  `ps` for that moment.
- Nothing checks the App's granted permissions against the table above. A widening in the web UI
  is invisible here, as [automation-identity.md](automation-identity.md)'s BUGS already says of the
  workflows.
- Two sessions that find the cache stale together both mint. Tokens are independent, so this costs
  an API call, not correctness.
