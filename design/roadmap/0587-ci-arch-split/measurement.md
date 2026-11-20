# Milestone 587 appendix: the queue evidence and the x86_64 measurement

An appendix to [milestone 587 (most CI jobs do not need an arm64 host, and the arm64 queue is where
the wait is)](../0587-ci-arch-split.md), moved out of that page on 2026-10-12 (UTC) under [§212 (a
prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main page is complete
without it. This is the proposal's evidence as filed on 2026-09-24: the queue waits by host, the
one measured run per host, and every job classified.

## The evidence that arm64 supply, not our quota, is the constraint

- The organization's 60-job cap was not close. Every job in every nife run from 15:24 UTC onward
  was counted (nife is the only repository in the organization that ran Actions that afternoon).
  19 to 25 jobs were running between 17:50 and 18:50 while 128 to 173 jobs sat queued for more
  than a minute. The running count reached 50 only at 19:31, after the queue had drained.
- x86_64 jobs in the same runs waited far less. `prove the kernel on x86_64` is the one job
  already on `ubuntu-24.04`, so every `verify.yml` run is a paired sample. Across the day's `ci.yml`
  and `verify.yml` jobs, by the hour each job was created:

  | Hour (UTC) | arm64 jobs | arm64 median / max queue | x64 jobs | x64 median / max queue |
  |---|---|---|---|---|
  | 16 | 249 | 14.8 / 42.9 min | 10 | 0.2 / 7.1 min |
  | 17 | 315 | 23.9 / 70.8 min | 7 | 8.1 / 15.1 min |
  | 18 | 290 | 14.5 / 57.3 min | 10 | 2.6 / 13.6 min |
  | 19 | 405 | 0.1 / 29.2 min | 4 | 0.0 / 5.9 min |

  In every same-run pair where the arm64 jobs waited more than a few minutes, the x86_64 job
  waited less, often by an order of magnitude (41.0 against 13.6 min, 28.4 against 2.7). When
  neither pool was backed up, both waited under a minute and the order between them is noise.
- x86_64 is not immune, and the table says so: one x86_64 job waited 15 minutes at 17:00. The
  claim is that the arm64 pool is the tighter of the two, not that the other one never waits.
- The measurement run itself queued for 0.0 to 0.1 minutes on every job, but it started at
  19:12, after the arm64 backlog had cleared, so it is not evidence on its own. The hourly table is.

## The measurement

One run of each workflow on `ubuntu-24.04`, from a temporary branch whose workflows were copies of
`ci.yml` and `verify.yml` with the runner label changed, the draft gate dropped, and (for verify)
the scope job dropped so the proofs actually ran. Compared against the latest `main` run of the same
job on `ubuntu-24.04-arm`.

| | x86_64 run | arm64 reference |
|---|---|---|
| `ci.yml` | 36046550177 (tree at `47c3a3c9d`) | 36030250577 (`main` at `47c3a3c9d`, same tree) |
| `verify.yml` | 36046550184 (tree at `47c3a3c9d`) | 36029132635 (`main` at `0b72f6736`, the latest `main` run whose shards proved) |

The check step is the comparison, not the job's wall time. The x86_64 jobs ran with every cache
cold: the QEMU cache key includes `runner.arch`, and `Swatinem/rust-cache` keys on the host triple.
So their wall time includes a from-source QEMU build of 6.1 to 7.1 minutes and a first `cargo
install` of the lint tools. That is paid once and then cached; the named `script/ci-build` step is
what recurs.

## Every job, classified

No job in either workflow uses KVM or any accelerator. Every QEMU leg in CI is TCG
(`helpers/qemu-runner-aarch64.sh` takes HVF only when `NIFE_ACCEL=hvf`, which only the dev Mac's
`script/test --hvf` sets, and `script/ci-build`'s `hvf` row is never named by a CI job). So "needs
host-native speed" applies to nothing here. What does apply is whether the host's architecture
changes what gets proved or tested. That is three things: Kani compiles for the host,
falsification records for `kernel/src/arch/` compile only on their own architecture, and guest
cores under multi-threaded TCG run on the host's memory model.

Times are minutes: the named check step, then the job's wall time in brackets. Queue is the
reference run's created-to-started wait for that job.

| Job | Needs arm64? | Why, from its steps | x64 | arm64 | Queue x64 / arm64 |
|---|---|---|---|---|---|
| `ci` draft gate | No | One `gh pr view` call | (not run) | 0.1 | n/a / 11.1 |
| rustfmt | No | `script/fmt --check`: text | 0.4 (0.7) | 0.2 (0.4) | 0.0 / 51.2 |
| clippy | No | `script/lint`: clippy on host and bare-metal targets, then text gates. On x86_64 the host build lints the `x86_64` arm of the few `cfg(target_arch)` items in host crates; the aarch64 arm is still linted through `aarch64-unknown-none-softfloat` | 1.7 (4.3), red, measurement artifact | 1.8 (2.6) | 0.0 / 47.8 |
| fastpath footprint | No | Cross-compiles release kernels for all three targets and walks `objdump` | 0.2 (0.7) | 0.3 (0.6) | 0.0 / 49.5 |
| reproducible build | No | Two builds on one host compared to each other | 0.2 (0.6) | 0.2 (0.6) | 0.0 / 51.1 |
| image permissions | No | Cross-compiles three kernels, reads ELF segment flags | 0.2 (0.8) | 0.1 (0.7) | 0.1 / 60.1 |
| stack frames | No | Cross-compiled `.stack_sizes`; host only supplies the tools | 0.5 (1.2) | 0.5 (1.1) | 0.1 / 53.1 |
| bench | No | TCG with `-icount`, single hart: counts guest instructions | 0.9 (10.5) | 0.7 (1.2) | 0.1 / 57.3 |
| supply chain | No | `cargo-deny` and tarball hashes | 0.1 (0.6) | 0.1 (0.5) | 0.1 / 54.9 |
| fuzz | No | libFuzzer on host builds of pure-logic parsers, 60 s a target | 5.7 (6.2) | 5.3 (5.7) | 0.1 / 47.4 |
| coverage | No | Host tests under `cargo llvm-cov`, 80% per-file floor | 0.6 (1.2) | 0.9 (1.4) | 0.1 / 62.5 |
| build + test | Keep, weakly | Host tests, then the kernel suites under TCG at `-smp 4` (aarch64, riscv64) and `-smp 2` (x86_64), then `swish-check` and `boot-check`. The SMP guest legs are the one place CI runs guest code on a weakly ordered host. See "What is lost" | 15.7 (24.1) | 20.2 (20.8) | 0.0 / 52.4 |
| cpu matrix | Keep, weakly | The riscv64 suite under five QEMU CPU models, `-smp 4`; the same weak-host argument | 8.7 (17.8) | 11.2 (11.9) | 0.0 / 59.8 |
| `verify` draft gate | No | One `gh pr view` call | (not run) | 0.1 | n/a / 11.8 |
| verify scope | No | `script/verify --affected-since`: git and `cargo metadata` | (not run) | 0.0 (0.4) | n/a / 4.3 |
| prove (shard 1/2) | No, except the `kernel` row | Kani compiles for the host, so the `kernel` row proves `arch/aarch64/` on arm64 and `arch/x86_64/` on x86_64 (milestone 304 (`cargo kani -p kernel` only ever compiled one architecture, and it was the runner's)). Every other row is portable logic | 15.1 (15.9) | 12.9 (13.4) | 0.1 / 27.1 |
| prove (shard 2/2) | No, as above | As above | 13.6 (14.3) | 16.9 (17.5) | 0.1 / 9.1 |
| re-falsify | Yes, for kernel arch records | `script/falsifications` replays `kernel/falsifications/arch.aarch64.iommu.*` only on an aarch64 host; its own header records that the sweep "replays a harness only on a host whose architecture compiles it" | (not run) | 45.2, cancelled at its timeout | n/a / 21.0 |
| `verify (Kani proofs)` | No | Reads two job results as strings | (not run) | 0.0 (0.1) | n/a / 4.0 |

Results matched on every job but one, and that one is an artifact of the copy. clippy went red
on x86_64 because the temporary workflow file copied `ci.yml`'s comments, and `script/citations
--ratchet` read their `milestone N` references as new unglossed citations. clippy itself had
finished and passed by then; the failure is the ratchet's, on a file that was never going to merge.
Beyond pass/fail:

- Host tests: 3,067 `ok` lines on both hosts, and the same distribution of per-binary results.
- bench: every instruction count matched to within one tick (`coremark` 20,913,779 against
  20,913,778; `null_system_call` 410,003 against 410,004). `-icount` is host-independent in practice,
  as `ci.yml` says it is designed to be.
- fastpath footprint and stack frames: the check output was identical line for line; the only
  differences were cache keys and temporary paths in the post-job cleanup.
- Kani: both shards green on both hosts. The shard times move in opposite directions
  (shard 1 slower on x86_64, shard 2 faster), and the arm64 reference is a different commit. So the
  honest reading is "no difference this sample can see", with a critical path of 15.1 minutes on
  x86_64 against 16.9 on arm64.
- The x86_64 guest legs were much faster on an x86_64 host: `swish-check (x86_64)` under OVMF
  took 7.9 minutes against 12.4, and the OVMF boot 16 seconds against 56. The aarch64 and riscv64
  kernel legs were a few seconds *slower* (78 against 70 s, 90 against 85 s). Net, the `build +
  test` check step was 15.7 against 20.2 minutes.

