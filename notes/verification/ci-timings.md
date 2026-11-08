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

This is milestone 119 (the long pole is one prover). The packer balances crates by their measured
seconds, so the slowest shard can never beat the slowest crate, and the slowest crate can never
beat its slowest harness, one formula in one single-threaded solver. These are medians of the ten
merge-group and push runs 42456372750 to 42523187972 (2026-11-08 UTC), with the largest harness
of each crate from run 42523187972:

| crate | median | largest harness |
|---|---|---|
| `glob` | 9.1 min | 9.0 min, `no_magic_means_the_pattern_is_its_own_only_match` |
| `designware_mobile_storage` | 9.0 min | 2.2 min, four bring-up harnesses near that size |
| `file_allocation_table` | 8.7 min | 3.5 min, `a_write_extends_the_chain_to_the_size_and_stays_inside_it` |
| `machine_discovery` | 5.9 min | 7.1 min, `every_hex_field_the_loader_writes_is_one_the_kernel_reads_back` |
| `calendar` | 4.3 min | 2.2 min |
| `subtree_scope` | 3.6 min | 2.5 min |
| the other 37 | 15.1 min | |
| serial | 55.7 min | |

A harness can outlast its crate's median because two run at once (`VERIFY_JOBS=2`) and the
runner's speed varies by about 20% from run to run. Packed by these medians, each prove job also
spends about 0.7 minutes on setup:

| prove shards | slowest shard, median | slowest shard, worst of ten | without the driver crates leaving the tree |
|---|---|---|---|
| four | 14.1 min | 16.1 min | 11.4 and 13.1 min |
| **five (what runs)** | **11.6 min** | **12.3 min** | **9.3 and 10.3 min** |
| six | 9.4 min | 10.7 min | 9.2 and 10.3 min |

Four shards put the job past the 85% warning of its 20-minute budget, which #3350 reported. Five
hold the slowest job near 60% of it. A sixth buys under two minutes while the driver crates
(`designware_mobile_storage`, `extensible_host_controller_interface` and
`non_volatile_memory_express`, about 10 serial minutes) are in the table, and nothing once they
leave, because `glob`'s one harness then sets the pace. `helpers/verify_times.py` keeps the table
current, and [notes/project-metrics.md](../project-metrics.md) charts it.

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
