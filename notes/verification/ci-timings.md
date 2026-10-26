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
  `kernel/src/arch/x86_64/interrupt_controller.rs`. Only an x86_64 host runs them, which means the `prove` shards
  since milestone 587 (most CI jobs do not need an arm64 host). So the count is of the tree, not of
  any one run, and no single host compiles all of them.
- Milestone 161 (the x86_64 kernel port) added eight on 2026-09-19, the block-leaf proofs in
  `crates/paging`.

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

This is milestone 119 (the long pole is one prover), re-measured 2026-10-07. On 2026-08-14 `glob`
was half of a 30.3-minute job. So two shards reached 15.1 minutes, and a third bought nothing. By
2026-10-06 the shards took about 20 and 27 minutes, on x86_64 runners since milestone 587. These are
medians of 40 green runs, 37424472664 to 37562060742:

| crate | then (the table) | now (40-run median) |
|---|---|---|
| `glob` | 15.0 min | 11.8 min |
| `machine_discovery` | 3.0 min | 11.5 min |
| `calendar` | 10.0 min | 5.2 min |
| `subtree_scope` | 2.0 min | 4.4 min |
| `usb` | 0.5 min | 2.8 min |
| the other 30 | 9.2 min | 11.4 min |
| serial | 39.7 min | 47.1 min |

`glob` got faster; the growth is `machine_discovery`, `subtree_scope` and `usb`. Packed:

| prove shards | proving | job |
|---|---|---|
| two, old table (what ran) | 26.6 min | about 27 |
| two, refreshed | 23.6 min | about 24 |
| **three (ruled, #1802)** | **15.7 min** | **about 16.5** |
| four | 11.8 min (`glob` alone) | about 12.5 |

Rebalancing alone does not reach 20 minutes. A third prove shard does, for about one more
runner-minute per run, and calef ruled for it on 2026-10-07. The re-falsify shards were planned at 20
minutes of replay, and ran a median 22.5 job-minutes at their slowest. He ruled their target down to
17 the same day. `helpers/verify_times.py` keeps the table current, and
[notes/project-metrics.md](../project-metrics.md) charts it.

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
