# `helpers/ci-failing.sh`: a ready pull request failing a required check says so

Part of [the merge queue's watchers](merge-queue.md). Added 2026-10-04 UTC at calef's approval. calef spotted failing pull request CI twice (#1611,
#1597) before any session did, and the refuse-ci-polling hook forbids a session from running a
watch loop, so the watch is `.github/workflows/ci-failing.yml`, as `nife-smelter[bot]`, every ten
minutes. All names (the workflow, the script, `helpers/ci-failing.jq`, the `ci-failing` label) are
provisional.

```console
$ helpers/ci-failing.sh --dry-run        # 2026-10-04, against the live repository
ci-failing[patagonia]: 19 required checks
```

Nothing was failing on a ready pull request at that moment (#1611, #1613 and #1615 were open and
clean). The flagging path was exercised two ways. The decision ran against 200 recent failed runs' heads.
The shell pass ran against a stubbed `gh` with one failing, one fixed and one draft pull request,
which printed `FAILING #7`, the label and comment writes, and `CLEARED #8`.


What it does, per open non-draft pull request:

- Reads the check runs on the head commit and keeps those named by the required checks of the
  active ruleset on `main`, looked up through the rulesets API each pass (19596094 today), never
  hardcoded.
- A check name is failing only if its newest run (highest check-run id) concluded `failure` or
  `timed_out`. A failure a later run replaced does not count, and neither does one hidden by a
  newer run that is in progress or cancelled. The recorded case is a real head whose `ready status`
  failed and then passed; the raw rollup shows both.
- Failing: adds the label and posts one comment per head SHA naming each failing check with its job
  URL, deduplicated by a marker holding the SHA. A head that flaps red, green, red is announced once.
  A new push is a new SHA and a new comment.
- Green: removes the label.
- Changes nothing else. It never dequeues, disables auto-merge or reruns CI, and
  `helpers/ci-failing-selftest.sh` (run by `script/lint`) fails if the script starts to.

Two choices that look like omissions. Drafts are skipped: a draft is its lane's, and every claim
commit is an empty diff, so the required `empty diff` check fails on each of them by design. And the
`architect hold` check is ignored by name: it goes red on purpose under `needs-architect`, which is a
person's queue and not a failing build.

## Who acts on it

A maintainer session, through `needs-maintainer`. A label alone has no reader: a lane that ended
`WAITING` is not watching, and a session's queue is the other label. So the merge drain reads this
one: a ready pull request from this repository, not held, that has worn `ci-failing` for 30 minutes gets
`needs-maintainer` with cause `red` ([queue-ejection.md](queue-ejection.md)). One queue for a
session to read, rather than two.

## BUGS

- Only checks that have reported are counted. A required check that never ran (a skipped
  workflow, a path filter) is absent, not failing, and a head can look green here while the queue
  still waits on it. `ready status` and the queue see that; this does not.
- `schedule:` is a floor, and here it is close to a ceiling of zero. The workflow reached `main` at
  19:14 UTC on 2026-10-04 and had produced no scheduled run 33 minutes later, with `*/10` set.
  `merge-drain.yml` shows the same: its recent scheduled runs sit hours apart (18:17, 14:25, 08:43,
  02:31 UTC) against a `*/5` cron. GitHub delays and drops scheduled runs under load and gives a new
  workflow no head start, and nothing was wrong with the file. So the workflow also fires on `workflow_run` when
  `ci` completes, as the drain does, which is when a head has just turned red.
- Its first run, a manual dispatch (37227933655), died with `jq: Argument list too long`: the
  check runs were passed as `--argjson`, and one argument may not pass 128 KiB on Linux. They now go
  in on stdin, and the selftest runs the whole pass over 10,000 check runs against a stubbed `gh`.
- The comment is once per SHA, forever. A pull request that fails, is fixed by rerunning, and
  fails again on the same SHA gets the label again and no second comment. That is the rule asked for;
  the label is the live signal.
- A rerun in progress clears the flag early. The newest run being queued or in progress hides an
  older failure, so the label comes off when the rerun starts and the failure is not re-announced if
  it fails again (same SHA, same marker).
- The label name is read as `ci-failing` in three places (the script's variable, its jq
  filter in the pull request listing, and `nm_red_label` in `helpers/needs-maintainer.jq`); a
  rename must change all three.
- Nothing reports its own death, the same gap the drain has. A disabled workflow shows in the
  Actions tab for whoever looks.
- Fork pull requests are included. Their head SHA has check runs in this repository, and the
  App token can comment, so they are treated like any other ready pull request.
- Stale data race. The head SHA is read from the list and the checks a moment later; a push in
  between flags the old SHA, and the next pass corrects it.
