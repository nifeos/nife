---
status: BUILT
built: 2026-11-16
raised: 2026-11-16
---
# 1117. The prove shards fit the hosted runner's memory

The rule in force since 2026-11-16 (UTC): each `prove` shard in `.github/workflows/verify.yml` runs
`script/verify` inside a memory fence, and a harness too large for the runner fails by name instead
of taking the runner down. The branch is `milestone/1117-prove-shards-fit-the-runner`.

What drives it: the merge queue lost 51 shard jobs to dead runners between 2026-11-14 and
2026-11-16, each one 30 minutes of a queue slot and no log. Shard 1 lost every run that reached a
new `memory_regions` harness from an open pull request, and shard 2 lost `byte_glob` on hosts
without swap.

Reuse: `helpers/kani_peaks.py measure` already reads a run's peak from its cgroup, through a user
systemd scope. A hosted runner has no user manager, so the fence makes the cgroup itself with sudo.

## What is in force

- `helpers/memory_fence.py` runs a command in a cgroup capped at MemTotal less 1536 MiB, with no
  swap. Every 15 seconds it prints the cgroup's use, the host's available memory and each running
  CBMC with the harness it proves. At the end it prints the peak and the out-of-memory kills, and
  an error annotation names the harness each kill took. A kill fails the run. Its `--selftest`
  runs in `script/lint`.
- The `prove` shards run `script/verify --shard k/5` inside it. It replaces the vmstat record and
  its print step, which never ran on a lost runner.
- `KANI_APART` in `script/verify` lists harnesses that prove after the rest of their row, alone.
  It holds `byte_glob`'s `a_negated_class_is_the_exact_complement_of_the_class`, since it and
  `no_magic_means_the_pattern_is_its_own_only_match` together need more than the runner has.
- notes/verification/ci-timings.md, "A shard that stalls", has the measured peaks.

## Follow-on

- **Recorded.** Pull requests skip the prove shards, so a harness too large for the runner first
  fails in the merge queue: notes/verification/ci-timings.md, "A shard that stalls".
- **Recorded.** The fence's reserve is a chosen number, and the harness a kill names is read from
  a one-second sample: `helpers/memory_fence.py`, `BUGS`.

## Index row

The prove shards run inside a memory fence that fails a harness too large for the hosted runner by name, in minutes and with its log, and byte_glob's two largest harnesses no longer run together.
