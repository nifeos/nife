# `script/idle-pull-requests`: no open pull request waits with nobody owning its next step

Name: provisional, minted 2026-11-18 (UTC) by lane/idle-prs-check: the script, this note and the `Held on` line.

A pull request can sit green, or sit behind another pull request that has since merged, because the
person or lane that owned its next step has stopped. A draft is the easy one to lose: the merge queue
and the failing-CI watcher both skip drafts, and a lane that ends its turn waiting is no longer
anyone's job. `script/idle-pull-requests` reads every open pull request and says which ones need an
owner now. The queue sweep runs it and acts on each line it prints.

## Declaring a hold

A pull request that cannot move yet says what it waits for, in one line of its body:

```text
Held on #1234
Held on nifeos/basalt#56
```

`Held on #N` names a pull request or issue in the same repository; `Held on <owner>/<repo>#N` names
one in another. Write one line per blocker. Edit the body through the REST API, since `gh pr edit`
fails on the Projects (classic) error. A label that names a ruling wait (`needs-architect`,
`held-by-lane`, `held-for-red-trunk`) also holds a pull request, without a line.

Once every blocker is closed or merged, the sweep reports the pull request as `HELD-UNBLOCKED`.
Whoever owns the pull request then merges main in, deletes the `Held on` line, marks it ready if it is
a draft, and arms it with `helpers/arm.sh`. This is the same idea as `Blocked-by: #N`
([merge-queue.md](merge-queue.md)), which the merge drain reads for ready pull requests; `Held on`
covers drafts and other repositories too.

## What it prints

`script/idle-pull-requests [<repo> ...]` takes `owner/name` or a bare name under nifeos; with none it reads
nife, crate-workflows, entropy_contract, designware_mobile_storage, block_roster_contract,
jh7110_entropy and basalt. `--all-nifeos` reads every repository of the organization. One line per
flagged pull request, and exit status 1 if any printed (2 if it cannot read a repository):

| class | meaning | next step it names |
|---|---|---|
| `HELD-UNBLOCKED` | a `Held on` blocker has closed or merged | resume the pull request |
| `RED` | not a draft, not queued, a check failed | fix the check, or say why it waits |
| `IDLE-GREEN` | every check passed or skipped, not queued, not armed, not held | arm it, or for a draft mark it ready first |
| `IDLE-STALE` | not held, not queued, not armed, no activity for 2 hours | resume the lane, or declare a hold |

The sweep never flags a queued pull request, an armed one, a held one or one updated within 30
minutes (the stand-in for "a lane is working"), except as `HELD-UNBLOCKED`. It ignores the
`architect hold` check, which goes red on purpose. It does not read the required-check set, so any
other failing check counts as `RED`.

A lane that ends its last turn with its pull request still a draft names the next step in its report,
so the sweep has an owner to hand the line to. `script/idle-pull-requests --selftest` runs the classifier on fixtures with no network, and `script/lint` runs it.
