# The Kani runner shutdown flake, measured (name provisional)

A `prove` shard of `verify.yml` sometimes fails in the merge queue with no proof failure behind it,
because the hosted runner is taken away mid-job. This note records the measurement, why a rerun
cures it, what the known-flakes signature matches, and the second failure mode that it does not.
Every date is UTC.

## Measurement

Over the 97 finished merge-queue attempts from 2026-11-15T10:38Z to 2026-11-16T00:21Z, 24 (25%) lost
a Kani `prove` shard. They split in two:

- **16 jobs failed in 2.2 to 3.3 minutes** with the log line `##[error]The runner has received a
  shutdown signal. This can happen when the runner service is stopped, or a manually started runner
  is canceled.` followed by `The operation was canceled.`. Pull requests 3567, 3574, 3603, 3606,
  3608, 3609, 3611, 3612, 3616 and 3621 each lost one or more attempts this way. The one read in
  full (run 43776160228, `prove (shard 1/5)`) was inside a harness check when the line arrived, 2.7
  minutes into the job.
- **8 `prove (shard 2/5)` jobs were cancelled at exactly 30.0 minutes**, and their logs are
  unreadable: the log API returns `BlobNotFound`.

No Kani proof failure appeared in any log that could be read.

## Why a rerun cures it

The shard is idempotent. In `.github/workflows/verify.yml` a `prove` job has `contents: read`
permission only, checks out the tree, restores the Rust cache (saved only outside a merge group),
installs Kani, runs `script/verify --shard N/5` and prints a memory record. It writes nothing that
a later attempt reads, so the same shard on the same tree gives the same verdict, and a runner taken
away says nothing about the proofs. `fail-fast` is false, so the other shards still report.

## What the signature matches, and what it does not

`helpers/known-flakes.tsv` lists `kani-shutdown`, whose signature is a `prove (shard N/M)` job line
carrying `##[error]The runner has received a shutdown signal`. It is as narrow as the failure's own
wording, and scoped to a `prove` job by the job name that leads every line of the log.

- **The log has to be read from the whole job.** `helpers/arm.sh` matches a signature against
  `gh run view <run> --log-failed`, which prints only the steps that concluded `failure`. A shard
  whose runner is killed ends its step `cancelled`, so that command prints nothing for the shard
  (checked on all 23 failed verify merge-group runs of 2026-11-15, none of which had a shard step
  that concluded `failure`). `helpers/arm.sh` therefore also reads the whole log of any failed job
  with a cancelled step, each line led by the job name and a tab, which is the form `--log-failed`
  uses. `helpers/push-requeue-selftest.sh` covers a killed `prove` shard and a killed job of another
  name.
- **A real proof failure does not match.** One ends its step `failure`, its log has no shutdown
  line, and the aggregate job (`verify (Kani proofs)`) is the only other failed job.
- **The 30.0-minute cancellations are not covered.** Their logs cannot be read, so no signature can
  match them, and they stay a refusal that needs `--despite-failed-checks`.

## Open: what stops a shard at 30.0 minutes

`verify.yml` gives the `prove` job `timeout-minutes: 25` and the proving step 21, so 30.0 is not
either limit. What was read: the check run's annotation on the two cancelled shards examined says
`The job has exceeded the maximum execution time of 25m0s`, and in one of them (run 43772203707,
pull request 3611) the proving step stopped being reported 1.5 minutes after it began, yet the job
stayed open until 30.0 minutes after it started. That fits a runner that stopped answering the
service early, and the service giving up 5 minutes after the job limit, but nothing read here shows
the 5 minutes to be a documented grace period or the cause to be a lost runner. Whether those
shards ever ran the proofs, and what limit closes them at 30.0, is not determined. Finding out
needs a cancelled shard whose log is still readable, or GitHub's account of runner-lost cancels.

## When to remove the line

Remove it when shards stop losing their runners, for example if the hosted pool changes, and the
line has not matched a removal for a month.
