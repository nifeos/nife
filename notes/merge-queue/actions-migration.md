# How the watchers run: in Actions, as `nife-smelter[bot]`

*Name: provisional.*

An appendix to [notes/merge-queue.md](../merge-queue.md), which keeps the table of workflows. This
file holds the move of 2026-09-24 in full: what it bought, the premise tested before anything was
written, what the maintainer must run on his Mac, and what was lost.

## Why the mechanism lives here

It lives here by the principle of `design/decisions/` §155 (the naming conventions move out of the
constitution): the constitution keeps the duty; this document keeps the mechanism. What `AGENTS.md` still says is that a session confirms the
watchers are alive and acts on what they found.

Each workflow mints a one-hour installation token with `actions/create-github-app-token`, from the
environment secrets `AUTOMATION_APP_ID` and `AUTOMATION_APP_KEY`. No key is at rest on anybody's
machine. That is the property that made this the recommendation over putting the App's private key
on patagonia: the credential-at-rest question does not arise for anything that runs here.

What this bought, in the order the proposal argued it:

- Everything these do is attributed to `nife-smelter[bot]` rather than to `calef`. So what is still
  attributed to calef is genuinely calef, which is the negative half a local log can never give.
- The singleton is a singleton by construction (`concurrency:`), rather than because one laptop
  happened to be awake.
- The run list is a log every contributor can read, with timestamps and exit codes, where
  `~/Library/Logs/nife/` on one Mac was readable by one person.
- A stopped watcher shows as a disabled workflow in the Actions tab, instead of living in one
  session's transcript. That is how the drain's unloading on 2026-09-23 was knowable to exactly one
  reader.

## The premise this rested on, tested before anything was written

`cli/cli#7213` reports `gh pr merge --auto` failing under a GitHub App installation token where a
personal token succeeds. The merge drain's entire job was then arming pull requests. So if that were
true here, every option that authenticates the drain as `smelter` loses, and the fork collapses back
to a machine account or the status quo.

It was measured on 2026-09-24 in a throwaway workflow, under a token minted from the `nife-smelter`
App, against a throwaway pull request that was closed and deleted the same minute:

```console
$ gh pr merge 1176 --repo nifeos/nife --auto --merge
! The merge strategy for main is set by the merge queue      # exit 0, armed
$ gh api graphql -f query='mutation{enablePullRequestAutoMerge(input:{pullRequestId:"...",mergeMethod:MERGE}){clientMutationId}}'
{"data":{"enablePullRequestAutoMerge":{"clientMutationId":null}}}
$ gh api graphql -f query='mutation{enqueuePullRequest(input:{pullRequestId:"..."}){clientMutationId}}'
gh: Pull request 14 of 14 required status checks have not succeeded: 2 expected.
```

The issue does not reproduce on this repository with this App. The third line is the interesting one,
and is why it was run. A permission denial and an eligibility refusal read differently.
`enqueuePullRequest` and `dequeuePullRequest` both reached GitHub's business logic, which is the
answer "permitted" in the only form the API gives. `gh pr list --json` read, and `gh pr comment`
posted, rendering as `nife-smelter[bot]`.

## What the maintainer must run on his Mac to retire the old jobs

This is not optional, and it is not automatic. Until it is done there are two drains, one in Actions
and one on a laptop, both arming the same pull requests. Nothing in this repository can do it:
`launchd` jobs live in `~/Library/LaunchAgents/` on one machine.

```console
$ launchctl unload -w ~/Library/LaunchAgents/com.nife.merge-drain.plist
$ launchctl unload -w ~/Library/LaunchAgents/com.nife.trunk-health.plist
$ rm ~/Library/LaunchAgents/com.nife.merge-drain.plist ~/Library/LaunchAgents/com.nife.trunk-health.plist
$ launchctl list | grep nife          # expect nothing yet
```

Then install the one watch that stays per developer. Retiring `com.nife.trunk-health` retires the
at-risk check with it (it was folded into that script's loop), and that is the watch AGENTS.md calls
the more valuable of the two. Write `~/Library/LaunchAgents/org.nifeos.at-risk.plist`, substituting
your own paths:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>org.nifeos.at-risk</string>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/sh</string>
    <string>/path/to/nife/helpers/at-risk-check.sh</string>
  </array>
  <key>WorkingDirectory</key><string>/path/to/nife</string>
  <key>StartInterval</key><integer>300</integer>
  <key>StandardOutPath</key><string>/Users/you/Library/Logs/nife/at-risk.log</string>
  <key>StandardErrorPath</key><string>/Users/you/Library/Logs/nife/at-risk.log</string>
</dict>
</plist>
```

```console
$ launchctl load -w ~/Library/LaunchAgents/org.nifeos.at-risk.plist
$ launchctl list | grep nife
-	0	org.nifeos.at-risk
```

An already-installed plist names its script by absolute path. So the rename of `scripts/` to
`helpers/` on 2026-09-23 breaks it silently: `launchd` logs the missing file, and the watch simply
stops reporting. Fix an existing one in place, or reinstall it from the block above.

```console
$ sed -i '' 's|/scripts/|/helpers/|' ~/Library/LaunchAgents/com.nife.at-risk.plist
$ launchctl unload ~/Library/LaunchAgents/com.nife.at-risk.plist
$ launchctl load -w ~/Library/LaunchAgents/com.nife.at-risk.plist
```

The label follows the project's domain, nifeos.org, from 2026-10-12 (UTC); before that it was
`com.nife.at-risk`, and the two fix-up commands above name the file as it was then. Rename an
older install in place:

```console
$ launchctl unload ~/Library/LaunchAgents/com.nife.at-risk.plist
$ sed 's/com\.nife\.at-risk/org.nifeos.at-risk/' ~/Library/LaunchAgents/com.nife.at-risk.plist \
    > ~/Library/LaunchAgents/org.nifeos.at-risk.plist
$ rm ~/Library/LaunchAgents/com.nife.at-risk.plist
$ launchctl load -w ~/Library/LaunchAgents/org.nifeos.at-risk.plist
```

`launchd` is the loop here, which is why `helpers/at-risk-check.sh` did not grow one. It does a pass
and exits, and `StartInterval` runs it every five minutes. A script with no loop cannot be killed
mid-loop by a prune of the checkout it was launched from, which is the failure that killed both
watchers on 2026-08-18.

## What is lost, and it is smaller than the proposal priced it

Cadence. GitHub's shortest `schedule` interval is five minutes. Scheduled runs are delayed under load
and dropped at peak, which is a documented behavior rather than a caveat. The proposal priced this
against the script's own 150-second loop, and that comparison was wrong. The `launchd` jobs fired
`--once` every five minutes, so the real loss is only the delay and the drops, not two and a half
minutes. calef accepted it on 2026-09-23.

Transition reporting. `trunk-health.sh`'s loop said "red" once and "recovered" once, because it
remembered the previous poll. A scheduled run remembers nothing, so a trunk red for an hour is twelve
failed runs. The workflow's own BUGS section says so. Carrying state in an artifact was more
machinery than the fact is worth.

The gap that was accepted and is now closed. Patagonia asleep meant nobody was watching. That was
named rather than hidden. A cron on cordoba was declined on 2026-08-26 in favor of the simpler thing
on the machine already in use. Actions closes it for the two that moved, and leaves it exactly where
it was for the at-risk check. That is correct: a laptop that is asleep has no lane worktree being
edited on it.

calef approved these two scheduled workflows on 2026-09-23. An unattended scheduled *agent* is still
declined (calef, 2026-08-26): it shuts down when the session driving it does. What runs on
a timer is a shell script reading GitHub and labeling what a session must pick up, with no judgment
in it. A queue reports, it does not resolve, is still the boundary. Neither workflow resolves a
conflict, retries a failed check, or marks anybody's draft ready.

## Three things a reader of the drain's output should know

Why a comment speaks once per episode. `merge-drain.sh` comments once per cause per episode (the
ejection's time, the conflicting head), and the `needs-maintainer` label is what persists. Until
2026-10-03 the drain commented on stalls and went quiet, and nothing re-announced a stall to a
session that opened later. The label is the fix, because `gh pr list --label` is a query and a
comment is not.

Which drain spoke. Every line the drain prints, and every comment it posts, is prefixed
`merge-drain[<instance>]`: `actions:<run id>` from the workflow, the hostname from a laptop. An
installation token carries the App and not the caller. So GitHub cannot tell a reader which instance
acted, once the automation runs in more than one place. The comment dedupe markers are deliberately
untagged, so two instances cannot each post the same comment once.

Two fields lie to a session watching one pull request.
`autoMergeRequest` goes null the moment GitHub enqueues the pull request. So "auto-merge is off"
reads exactly like "dropped from the queue", when it means the opposite. And `statusCheckRollup`
keeps every run, including the ones a newer push cancelled. So a `CANCELLED` conclusion is usually an
older run sitting beside its own `SUCCESS`. Ask the queue itself instead: `gh api graphql` for
`pullRequest(number: N) { mergeQueueEntry { state position } }`, where no entry while open means out
of the queue. Treat only `FAILURE` and `TIMED_OUT` as failures.
