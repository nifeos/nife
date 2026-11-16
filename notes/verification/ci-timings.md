# Running the proofs in CI: timings, sharding and `--affected-since`

An appendix to [notes/verification.md](../verification.md), which keeps the command and how many
packages carry harnesses. This file holds the history of how long the suite takes, how it is
sharded, and how a pull request that cannot reach a proof skips it.

## Where the harness count comes from

The harness count is generated weekly into `notes/project-metrics.md`. A count typed into the
parent conflicted on every merge that added a harness. Some history of the count's growth:

- Milestone 198 (a package manager) added two harnesses on 2026-09-23, in `crates/package_archive`.
  Both take 4 seconds.
- Milestone 304 (`cargo kani -p kernel` only ever compiled one architecture) added two, in
  `kernel/src/arch/x86_64/interrupt_controller.rs`. Only an x86_64 host runs them, which means the
  `prove` shards
  since milestone 587 (most CI jobs do not need an arm64 host). So the count is of the tree, not of
  any one run, and no single host compiles all of them.
- Milestone 161 (the x86_64 kernel port) added eight on 2026-09-19, the block-leaf proofs in
  `crates/page_tables`.

The package count carries a `<!--count:-->` marker that `script/lint` re-derives each build. The
harness count did too. Either could still be wrong about what it counted, and one was. It answered a
narrower question than the prose asks. Milestone 212 (`script/falsifications` walks `crates/` only,
so the ratio it prints is not the tree's) found that. The derivation walked `crates/`, and so
excluded `kernel` and `user`, both of which are rows in the parent's table. The timing below is
still a dated measurement, because a wall clock is not a thing a gate can cheaply re-derive. See
notes/counted-claims.md.

## Parallelism, measured 2026-08-03

Harnesses within a crate verify in parallel, `-j 4` by default. `VERIFY_JOBS` overrides it. The
script's comment explains the memory bound and the terse-output trade. Measured on 2026-08-03, all
exit-clean on the same tree:

| machine | serial | `-j 4` |
|---|---|---|
| dev Mac (Apple Silicon) | 21m40s | **11m19s** |
| CI (4-core ubuntu-arm) | ~42m | expect ~20m; take the real number from the first merged run |
| cordoba (4-core Haswell) | 58m41s cold, ~40m solve | not measured; nothing decides on it |

The parallel speedup is 1.9x, not 4x, and the gap is structural. Crates still run one at a time. The
wall clock cannot drop below the longest single harness (glob's), because one harness is one formula
in one single-threaded solver. The cordoba row exists because a self-hosted runner there was
considered and declined on these numbers.

## Sharding, and where the floor moved

This is milestone 119 (the long pole is one prover). The packer balances crates by their measured
seconds, so the slowest shard can never beat the slowest crate, and the slowest crate can never
beat its slowest harness, one formula in one single-threaded solver. The table below holds medians of the 29 merge-group runs 43642016303 to 43659021264 (2026-11-14
and 2026-11-15 UTC), each with every shard passing. The largest harness is each crate's median over
the shard logs of the last 40 runs:

| crate | median | largest harness |
|---|---|---|
| `memory_regions` | 9.5 min | 2.9 min |
| `byte_glob` | 9.1 min | 7.8 min, `no_magic_means_the_pattern_is_its_own_only_match` |
| `machine_discovery` | 6.5 min | 6.3 min |
| `driver_process_contract::virtual_input_output` | 5.6 min | 5.6 min |
| `subtree_scope` | 3.8 min | 3.6 min |
| the other 33 | 16.2 min | |
| serial | 50.6 min | |

A harness can outlast its crate's median because two run at once (`VERIFY_JOBS=2`), and the
runner's speed varies by about 1.6 times from run to run (`byte_glob`: 399 to 658 seconds over
those runs). The packer splits these medians into shards as below. The other two columns sum each
crate's 90th percentile and maximum over the same shard, and each job adds about 0.7 minutes of
setup:

| prove shards | slowest shard, median | slowest shard, 90th percentile | slowest shard, maximum |
|---|---|---|---|
| four | 12.7 min | 14.3 min | 15.4 min |
| **five (what runs)** | **10.1 min** | **11.6 min** | **12.5 min** |
| six | 9.5 min | 10.6 min | 11.6 min |

Five hold the slowest job near 60% of its 20-minute budget at the 90th percentile. A sixth buys
under a minute, because `memory_regions` and `byte_glob` are each a shard on their own, and seven
buy nothing more. `helpers/verify_times.py` keeps the table current, and
[notes/project-metrics.md](../project-metrics.md) charts it.

### A shard that stalls

A hosted runner has about 16 GB: 15983 MiB on the Azure hosts, with 3 GiB of swap, and 15700 MiB on
the AWS hosts, some of them with no swap. Unfenced, CBMC processes that outgrow it make the kernel's
out-of-memory killer take the runner's own agent. GitHub then says "The runner has received a
shutdown signal", the job sits until its 25-minute limit, and GitHub cancels it about five minutes
later, so it reads 30.0 minutes with no log past the kill.

So the `script/verify` step runs inside `helpers/memory_fence.py`: a cgroup capped at the runner's
memory less 1536 MiB, with no swap. A harness that outgrows the cap loses its CBMC, Kani reports
that harness as failed, the fence names it in an error annotation, and the shard fails in minutes
with its log. Every 15 seconds the fence also prints its use and each running CBMC with the harness
it proves. A shard that still stalls keeps its log, because the step's own limit, 21 minutes, sits
below the job's.

Two rows have outgrown the runner. Their peaks, alone at `-j 1` and as a crate at `-j 2`, measured
inside the fence on the runner (image 20261111.196.1, cap 14446 MiB):

| what ran | peak (MiB) |
|---|---|
| `byte_glob`, `no_magic_means_the_pattern_is_its_own_only_match` alone | 8484 |
| `byte_glob`, `a_negated_class_is_the_exact_complement_of_the_class` alone | 7598 |
| `byte_glob`, the crate at `-j 2` | the cap, `no_magic` killed |
| shard 2 at `-j 2`, `byte_glob` with `KANI_APART` | 12389 to 12393 |
| `memory_regions`, the crate at `-j 2` | 10937 |

Kani starts `byte_glob`'s two largest harnesses first, and together they need more than the cap. On
the Azure hosts swap carried them; on the AWS hosts without it they lost the runner. script/verify's
`KANI_APART` proves the negated-class harness after the rest of the row, alone. Over four shard-2
runs (run 43784902792) the row then took 416 to 678 seconds, against 621 before.

A new harness meets the runner first in the merge queue, since pull requests skip the prove shards.
One that needs more than the cap fails there, named, on every run.

The dangerous failure mode is a crate that lands in no shard, because an unproved crate is
invisible. The suite goes green *faster*, and nothing says a harness stopped running. So the packer
asserts on every invocation that the shards partition the table exactly. It refuses to prove a
subset while reporting itself as the suite. That was verified by running both shards against a
stubbed `cargo` and diffing the union against the unsharded run: identical, all 19 crates.

The required check is still one job called `verify (Kani proofs)`. A matrix would have renamed it to
`verify (Kani proofs) (1)` and `(2)`. That would leave the ruleset requiring a check that no longer
exists, blocking every merge forever. So the proving happens in a `prove` matrix, and a small
aggregate job carries the name and reports their combined result. See the comment at the top of
`.github/workflows/verify.yml`. It is the same trap that file already records for moving a job
between workflows.

## `script/verify --affected-since`

`script/verify --affected-since <base>` answers a different question without proving anything: can
the diff since `<base>` reach a proof at all? The proofs are a function of the harness crates and
their transitive dependencies. So the script asks `cargo metadata` for that closure and classifies
every changed file. Only some changes report `not-needed`: a change confined to documentation, to
workflow files other than verify.yml, or to crates outside the closure (the kernel and the user
programs, which no proof compiles). Anything it cannot attribute, the workspace manifests and
lockfile included, runs the proofs by default. `.github/workflows/verify.yml` reads the last line.
That is how a kernel-only pull request stops paying the 42 minutes.
