# The Kani runner shutdown flake, measured

*Name: provisional, minted 2026-11-16 (UTC) by lane/kani-shutdown-known-flake.*

Sometimes a `prove` shard of `verify.yml` fails in the merge queue with no proof failure behind it,
because the hosted runner goes away mid-job. This note holds the measurement, the reason a rerun
cures it, what the known-flakes signature matches, and a second failure mode it does not match.
Every date is UTC.

## Measurement

Across the 97 finished merge-queue attempts from 2026-11-15T10:38Z to 2026-11-16T00:21Z, 24 (25%)
lost a Kani `prove` shard. They split in two.

Sixteen jobs failed in 2.2 to 3.3 minutes. Each log carries `##[error]The runner has received a
shutdown signal. This can happen when the runner service is stopped, or a manually started runner
is canceled.`, then `The operation was canceled.`. Pull requests 3567, 3574, 3603, 3606, 3608,
3609, 3611, 3612, 3616 and 3621 each lost one or more attempts this way. The one log read in full
(run 43776160228, `prove (shard 1/5)`) shows the line arriving 2.7 minutes into the job, while a
harness check was running.

At exactly 30.0 minutes the service cancelled eight `prove (shard 2/5)` jobs. Their logs cannot be
read: the log API answers `BlobNotFound`.

No Kani proof failure appeared in any log that could be read.

## Why a rerun cures it

The shard is idempotent. In `.github/workflows/verify.yml` a `prove` job holds `contents: read`
and nothing more. It checks out the tree, restores the Rust cache (saved only outside a merge
group), installs Kani, runs `script/verify --shard N/5` and prints a memory record. It writes
nothing that a later attempt reads, so the same shard on the same tree gives the same verdict. A
runner taken away says nothing about the proofs. The matrix sets `fail-fast: false`, so the other
shards still report.

## What the signature matches, and what it does not

`helpers/known-flakes.tsv` lists `kani-shutdown`. Its signature is a line led by a `prove (shard
N/M)` job name that carries `##[error]The runner has received a shutdown signal`. That is as narrow
as the failure's own wording, and the job name scopes it to a `prove` job.

- The log has to come from the whole job. `helpers/arm.sh` matches a signature against `gh run view
  <run> --log-failed`, which prints only steps that concluded `failure`. When a shard's runner dies, the
  step ends `cancelled`, so that command prints nothing for the shard. All 23 failed
  verify merge-group runs of 2026-11-15 show this, and none had a shard step that concluded
  `failure`. So `helpers/arm.sh` also reads the whole log of any failed job that has a cancelled
  step. It leads each line with the job name and a tab, the form `--log-failed` uses.
  `helpers/push-requeue-selftest.sh` covers a killed `prove` shard and a killed job of another
  name.
- A real proof failure does not match. Its step concludes `failure`, its log has no shutdown line,
  and the aggregate `verify (Kani proofs)` job is the only other failed job.
- The signature does not cover the 30.0-minute cancellations. Nobody can read their logs, so no signature
  matches them. They still need `--despite-failed-checks`.

## Open: what stops a shard at 30.0 minutes

The `prove` job has `timeout-minutes: 25` and its proving step 21, so 30.0 is neither limit. Two
facts came from the logs and the API. The check run's annotation on the cancelled shards examined says `The job has
exceeded the maximum execution time of 25m0s`. In one of them (run 43772203707, pull request 3611)
the proving step stopped reporting 1.5 minutes after it began, yet the job stayed open until 30.0
minutes after it started. That fits a runner that stopped answering the service early, with the
service giving up 5 minutes after the job limit. Nothing read here shows the 5 minutes to be a
documented grace period, or the cause to be a lost runner. Nobody knows whether those shards ever ran their
proofs. The next step needs a cancelled shard whose log is still readable, or GitHub's
account of how it closes a job whose runner has gone silent.

## When to remove the line

Remove it when shards stop losing their runners, or when the line has gone a month without matching
a removal.
