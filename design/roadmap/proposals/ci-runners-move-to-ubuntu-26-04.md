---
status: PROPOSED
raised: 2026-10-23
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# CI's hosted runners move from Ubuntu 24.04 to Ubuntu 26.04

*(Title and slug provisional.)*

Reuse: GitHub's own hosted images and labels, unchanged; the canary reuses milestone 587 (most CI
jobs do not need an arm64 host)'s method of one paired run per workflow, compared job by job. Nothing
new is written beyond a cache-key input and a canary dispatch.

Raised 2026-10-23 (UTC) by lane `maintainer/proposal-ubuntu-26-04-runners`. Every Linux job in
`.github/workflows/` names `ubuntu-24.04` or `ubuntu-24.04-arm`. GitHub made both 26.04 labels
generally available on 2026-09-17. This proposal measures what the move touches and stages it. It
changes no workflow.

## Availability

All four facts below were read from actions/runner-images on 2026-10-23.

- Public preview of `ubuntu-26.04` and `ubuntu-26.04-arm` opened 2026-06-11 (issue
  [#14226](https://github.com/actions/runner-images/issues/14226)).
- Both went GA on 2026-09-17 ([#14747](https://github.com/actions/runner-images/issues/14747); the
  README lost its preview badge the same day in PR
  [#14739](https://github.com/actions/runner-images/pull/14739)).
- `ubuntu-latest` moves to 26.04 gradually from 2026-10-19, to finish by 2026-11-19
  ([#14748](https://github.com/actions/runner-images/issues/14748)). That announcement covers x64
  only. There is no `ubuntu-arm-latest` label; a request for one is open
  ([#13885](https://github.com/actions/runner-images/issues/13885)).
- GitHub supports at most two GA Ubuntu images and starts deprecating the oldest when a newer one
  reaches GA (README, "Support Policy"). 22.04's deprecation began 2026-09-17
  ([#14254](https://github.com/actions/runner-images/issues/14254)). Nothing forces nife off 24.04
  until a successor to 26.04 ships.

So availability is not the dependency. The newest released images on 2026-10-23 are
`ubuntu26/20261012.163` (x64) and `ubuntu26-arm64/20261018.155`.

## Inventory

`runs-on` across 26 workflow files, counted by parsing them on 2026-10-23: 35 jobs on
`ubuntu-24.04`, 40 on `ubuntu-24.04-arm`, one matrix, one `windows-latest`.

| workflow | `ubuntu-24.04` (x86_64) | `ubuntu-24.04-arm` |
|---|---|---|
| `ci.yml` | 17: gate, test, swish-check-x86_64, cpu-matrix, fmt, lint, clippy, fastpath-footprint, boot-file-size-check, interleavings, build-is-reproducible, image-permissions, stack-frame-check, bench, supply-chain, fuzz, coverage | 7: test-shard, swish-check, rg-threads, rg-threads-x86_64, boot-check, cpu-matrix-shard, watchdog |
| `ci.yml` `main-caches` | matrix leg | matrix leg (with Kani) |
| `verify.yml` | 5: gate, scope, prove, falsify, verify | 3: prove-kernel-aarch64, prove-kernel-riscv64, falsify-shard |
| `codeql.yml` | 4: gate, analyze, upload, upload-retry | |
| `falsifications.yml` | 3: record-times, sweep-swish-x86_64, sweep-x86_64 | 4: plan, sweep, syscall-fuzz, confined-fuzz |
| `toolchain-bump.yml` | 2: restamp, propose-kani | 1: propose |
| `mutation-gate.yml` | 1: gate | 1: mutation-gate |
| `fleet-metrics.yml`, `verify-shard-refresh.yml`, `stick-maker-hosts.yml` (linux) | 1 each | |
| `mutation.yml` | | 4: mutants, report, inflow, record |
| `kani-reach.yml` | | 3: plan, prove, report |
| `merge-drain.yml`, `architect-ruled-clears-hold.yml` | | 2 each |
| thirteen one-job bot and cadence workflows | | 1 each |

`stick-maker-hosts.yml`'s `windows-latest` job is out of scope.

Other places that name the image or depend on what it ships:

- Cache keys. `qemu-${{ runner.os }}-${{ runner.arch }}-<.qemu-version hash>` appears in 15 steps
  across `ci.yml`, `falsifications.yml` and `toolchain-bump.yml`, and
  `kani-riscv64-${{ runner.os }}-${{ runner.arch }}-...` in two. `Swatinem/rust-cache` keys on
  `os.type()` and `os.arch()` (its `src/config.ts`). None carries the Ubuntu release.
- apt. `script/ci-qemu` installs `libslirp0` and the QEMU build dependencies. `script/bootstrap`
  installs `qemu-system-arm qemu-system-misc qemu-system-x86 ipxe-qemu ovmf`, `clang` and
  `shellcheck`. The `lint` job installs `clang shellcheck device-tree-compiler`, and
  `toolchain-bump.yml` and `toolchain-drift.yml` install `clang`. No version is pinned anywhere.
- QEMU and edk2. CI never runs apt's QEMU. Every job that boots one restores the cache and runs
  `script/ci-qemu` before `script/bootstrap` (checked by parsing all 26 files), and
  `helpers/qemu-uefi-x86_64.sh` takes the built prefix's `edk2-x86_64-code.fd` before any apt OVMF.
- Kani. `script/kani-install` runs `cargo install kani-verifier` at `.kani-version` (0.68.0), then
  `cargo kani setup`, which fetches Kani's release bundle. `script/verify-riscv64` builds the patched
  Kani from source.
- `.github/runner-container/Containerfile` is `FROM ubuntu:24.04`, so `script/runner-container`
  approximates the 24.04 runner.
- basalt, outside this tree: its `gate.yml`, `packages.yml`, `pin-bump.yml` and `codeql.yml` carry
  the same two labels.

## What the tree records as depending on 24.04

`git grep` for 24.04 across `design/`, `notes/`, `briefs/`, `script/`, `helpers/` and `xtask/` finds
about 90 lines. They fall into three kinds.

### Milestone 193's warning, and why it is about the architecture

Milestone 193 (put `kernel/src` within reach of the prover) ends its follow-on with a
`Recorded.` bullet: the `kernel` row works "because every job in `verify.yml` runs on
`ubuntu-24.04-arm`. A change to that runner label breaks the row". The mechanism, in
`notes/kernel-proofs.md`, was `kernel/Cargo.toml` depending on `aarch64-cpu` unconditionally. Kani
compiles for the host, so an x86_64 host failed to build the kernel crate.

That dependency is about the host's architecture, not the Ubuntu release, and it is gone. Commit
`1cab1aab9` (2026-08-31) target-gated `aarch64-cpu`. Milestone 304 (`cargo kani -p kernel` only
ever compiled one architecture) then proved the `kernel` row on both, and since milestone 587
`verify.yml`'s `prove` shards run on x86_64. Moving `ubuntu-24.04-arm` to `ubuntu-26.04-arm` keeps
the architecture, so it breaks nothing here. The bullet and its source paragraph
(`notes/kernel-proofs.md`, near line 278) are stale today.

### Dated observations, which stay true

Milestones 304, 310, 428, 587, 589, 627, 635, 755, 861, 870, 914 and 920, and the notes under
`notes/kani-upstream/` and `notes/net/`, record what was measured on a 24.04 host on a stated day.
Milestone 126 (the `procps` package, now `process-tools`) is one: it records `procps
2:4.0.4-4ubuntu3.2` from a 24.04 container. A move does not falsify a dated measurement, so none
needs an edit.

### Present-tense claims the move makes false, or that are false already

- `script/ci-qemu`'s header and three `ci.yml` comments say GitHub offers no newer Ubuntu image.
  That has been false since 2026-06-11.
- `script/bootstrap` says "No Ubuntu release ships a QEMU with -device riscv-iommu-pci". On
  Ubuntu 26.04 the packaged QEMU is 10.2.1, and its `qemu-system-riscv64` lists
  `riscv-iommu-pci` (verified below). The device arrived in QEMU 9.2.0.
- On 26.04, `qemu-system-riscv64` ships in a new package, `qemu-system-riscv`, and
  `qemu-system-misc` no longer carries it (verified with `apt-cache` and `dpkg -L`).
  `script/bootstrap`'s comment and package list name `qemu-system-misc` for it.
- Current descriptions of CI: `helpers/workflow_parses.py`, `notes/kernel-proofs.md`'s host table,
  `notes/scripts/build-test-and-gate.md`, `script/lint`'s host-pass comment, `script/effort`, the
  Containerfile and `script/runner-container`.
- Milestone 288 (host tests that assume the host is aarch64) says in its BUGS "No host in CI is
  anything but aarch64". It has been false since milestone 587 put jobs on x86_64 on 2026-09-24.

## What changes in 26.04

Versions come from the two 26.04 image READMEs and Launchpad's published sources for `noble` and
`resolute`, read 2026-10-23. "Verified" means run on an Ubuntu 26.04.1 x86_64 host with this tree
checked out. Nothing was run on an arm64 26.04 host.

| item | 24.04 | 26.04 | effect | how known |
|---|---|---|---|---|
| apt QEMU | 8.2.2 | 10.2.1 | none in CI (never used) | verified: `riscv-iommu-pci` present |
| edk2 / OVMF | 2024.02 | 2025.11 | none in CI; `OVMF_CODE_4M.fd` keeps its name | verified |
| glibc | 2.39 | 2.43 | the cache hazard below | verified |
| Python | 3.12.3 | 3.14.4 | `script/lint` needs 3.11+ | verified: lint run under 3.14.4 |
| Ruby | 3.2.3 | 3.3.8, on both images | two lint checks run `ruby -ryaml` | verified: lint run under 3.3.8 |
| shellcheck | 0.9.0 | 0.11.0 | new warnings could fail lint | verified: lint run under 0.11.0 |
| default clang | 18 | 21 | builds `c_shim` | verified it builds; instruction counts unmeasured |
| dtc | 1.7.0 | 1.7.2 | lint compares committed `.dtb` files | verified: 33 pairs match |
| Kani 0.68.0, CBMC 6.11.0 | bundle | same bundle | runs on glibc 2.43 | verified x86_64, a one-harness crate; arm64 a guess |
| `/tmp` | root disk | RAM-backed tmpfs | builds in temp | read from GitHub's issue, below |

### Caches built on one release poison the other

A QEMU 11.1.2 built by `script/ci-qemu` on 26.04 requires `GLIBC_2.43` (`objdump -T` on the built
binary), so it cannot start on 24.04. The keys cannot tell the two apart. Whichever pool saves the
key first serves the other pool a binary it cannot run. `script/ci-qemu`'s warm check then reads an
empty version and rebuilds, which costs about 4.5 minutes in every QEMU job of every run. The rebuild
is never saved, because the key already exists. The patched Kani and every `rust-cache` entry carry
the same hazard. The reverse, a 24.04 build on 26.04, links the same sonames and probably runs, but
nobody has run it.

The fix is one more key input: the release, read from `/etc/os-release` in a step. It is safe to
land on 24.04 first, and it costs one cold rebuild per pool.

### `/tmp` is RAM-backed, and GitHub's fix has not shipped

Ubuntu 26.04 mounts `/tmp` as tmpfs at half of memory. On hosted runners that is about 7.8 GB,
and a regular user's quota stops writes near 6.3 GB
([#14777](https://github.com/actions/runner-images/issues/14777), opened 2026-09-23 and confirmed by
a runner-images maintainer on both 26.04 images the next day). PR
[#14814](https://github.com/actions/runner-images/pull/14814) masks `tmp.mount` and merged
2026-10-20. Both released 26.04 images were built before that, so they still mount tmpfs. That
follows from the dates; it was not observed on a runner.

What nife writes to the system temp directory:

- `script/ci-qemu` builds in `mktemp -d`. Measured on 26.04: the tarball is 142 MB, the
  extracted source 899 MiB, and the tree after the three-target build 2,945 MiB, about 3 GB in all.
  That fits under the quota, but it holds 3 GB of memory for the whole cold build.
- `cargo mutants` (`mutation.yml`, `mutation-gate.yml`) builds in copies of the tree, two at a time
  under `-j 2`, each with its own `target/`. Its `copy_tree.rs` makes each copy with
  `tempfile::Builder::new().tempdir()`, which is the system temp directory (read on cargo-mutants'
  main branch, not at the pinned 27.1.0). At several GB a copy (a guess), two could reach the
  quota.
- tmpfs pages are memory. `verify.yml`'s `prove` shards already sit near the 16 GB runner limit at
  `VERIFY_JOBS=2`, by its own recorded measurement.

So the canary prints `findmnt /tmp` first. If the fix has still not shipped, the alternative is a
`systemctl mask tmp.mount` step in each job that writes large temporaries, or a `TMPDIR` on the
work disk.

## Cost

Measured from the Actions API for one UTC day, 2026-10-21: 2,469 completed runs and 14,668 job
minutes, 7,969 on `ubuntu-24.04-arm` and 6,699 on `ubuntu-24.04`. `ci.yml` and `verify.yml` are
13,146 of them. Dividing by the day's 76 full `ci.yml` suites (the `swish-check` count) and 70
`verify.yml` scopes, one full pair costs about 177 job minutes.

- The canary is one dispatched pair per architecture, about 177 job minutes each, plus a re-run.
- The flip costs one cold QEMU build per pool (265 to 274 s, `ci.yml`'s `main-caches` comment) and
  one patched-Kani build on arm64 (145 s, measured 2026-09-25). Every `rust-cache` entry starts
  cold once.
- Without the key change, a mixed period costs about 4.5 minutes on each QEMU job of each run,
  for as long as it lasts.

Public repositories run standard hosted runners at no charge (recalled from GitHub's billing
documentation, not read today). The cost that binds is arm64 queue time, which milestone 587 found
was where the queue waited.

## What done means

- No workflow in this tree names `ubuntu-24.04` or `ubuntu-24.04-arm`.
- `ci.yml` and `verify.yml` are green on 26.04 on both architectures, with the same host-test
  count, identical `fastpath-footprint` and stack-frame output, and `bench` instruction counts
  equal to the 24.04 run's. That is milestone 587's comparison.
- `/tmp` on every job is either on the root disk or masked by a step that says why.
- The QEMU and Kani cache keys carry the release.
- Every present-tense claim listed above states what is true on 26.04.
- basalt is bumped in its own pull request.

## Plan

1. Separate the caches. Add the release to the QEMU, Kani and `rust-cache` keys, on 24.04, in
   one pull request. It costs one cold rebuild per pool and changes nothing else.
2. Canary x86_64. On a branch, change only the `ubuntu-24.04` labels and dispatch `ci.yml` and
   `verify.yml` (`briefs/gate-in-ci.md`, "Gating while the pull request stays draft"). Every job
   prints `findmnt /tmp`. Compare job by job against the same commit's 24.04 run. x86_64 goes
   first because it carries the Python, Ruby, shellcheck and dtc changes (in `lint`), the `bench`
   counts, and most Kani proving, without arm64's queue.
3. Flip x86_64: 35 jobs and `main-caches`' x86_64 leg, in one pull request.
4. Canary arm64 the same way: the build shards, `cpu-matrix-shard`, `re-falsify`, the two
   `prove-kernel-*` jobs and one mutation shard. These carry the guest's weak-ordering coverage and
   `cargo mutants`' temp copies.
5. Flip arm64: 40 jobs and the other matrix leg. The bot workflows go in this step, though nothing
   in them depends on the release.
6. Correct the record: the present-tense claims listed above, and the Containerfile's base image,
   so `script/runner-container` still approximates the runner.
7. Bump basalt in its own pull request after nife lands, citing the nife commit.

Each flip is one label edit, reverted by one commit. The irreversible part is small: the
present-tense records, and a published `bench` number if the counts were to move.

## Risks

- `bench` instruction counts move. QEMU and the Rust toolchain are pinned and unchanged, so they
  should not, but the C shim's clang changes. The canary decides it.
- An apt tool on the x86_64 image differs from the one measured here. `lint` installs
  `shellcheck`, `dtc` and `clang` unpinned, and the image's weekly update can move them.
- arm64 26.04 is unverified everywhere in this proposal. The arm64 canary is the measurement.
- The runner-images team reverses or delays the `/tmp` fix. The mask step covers that.
- The image changes weekly under a fixed label, on 24.04 as on 26.04. A pin fixes the release, not
  the contents.

## Choices for calef

These are an architect's, and nothing above decides them.

1. Pin `ubuntu-26.04` and `ubuntu-26.04-arm`, or float on `ubuntu-latest`. Floating is only
   possible on x86_64, because no arm64 `-latest` label exists. So floating would leave the two
   pools on different releases whenever `-latest` moves. Recommendation: pin, for the reason this
   tree pins `.qemu-version` and the toolchain. A runner that changes release unannounced changes
   what "the tests passed" means. The cost is this proposal, once every two years.
2. When to flip. Now, with a `tmp.mount` mask in the jobs that need it, or after a released 26.04
   image carries #14814. Recommendation: land step 1 now and run the canaries now. Flip once
   `findmnt` shows the fix, since a mask step is code to delete later.
3. Whether CI keeps building QEMU 11.1.2 now that 26.04's packaged 10.2.1 has the device. A cold
   build is about 4.5 minutes per pool per cache expiry. Using apt's would change the emulator
   under every instruction count and break parity with the development Mac's 11.1.x. This is not
   part of the move. Recommendation: keep the build.
4. Whether basalt moves in lockstep or on its own schedule. Its jobs build and test the same image,
   so lockstep keeps the two gates comparable.

## Lint on 26.04, measured

On 2026-10-23, `script/lint` ran on Ubuntu 26.04.1 x86_64 with this tree, Python 3.14.4, Ruby
3.3.8, shellcheck 0.11.0 and clang 21.1.8. 107 of its 108 sections passed. The one failure was
`script/citations --ratchet` asking for glosses in this file, since given. Ruby parsed all 26
workflow files and the issue forms; shellcheck passed 146 scripts with no warnings. That host had
no `dtc`, so the `.dtb` check skipped; run separately with Ubuntu 26.04's `dtc` 1.7.2, it compared
33 pairs and all matched. A Python 3.14 change that could bite, the `tarfile` extraction default,
does not: the tree's one `extractall` (`script/vendor-verify`) passes `filter="data"` already, and
nothing imports `multiprocessing`.

## Found while measuring

- Milestone 193's follow-on bullet on the runner label, and the paragraph in
  `notes/kernel-proofs.md` behind it, are stale since commit `1cab1aab9`. Step 6 corrects them.
- Milestone 288's BUGS bullet ("No host in CI is anything but aarch64") is stale since milestone
  587. Step 6 corrects it.
- `script/bootstrap` on a stock 26.04 machine installs `qemu-system-misc` for a riscv64 emulator
  it no longer carries. The script still recovers, because `script/qemu-check` fails and
  `script/ci-qemu` builds the pin. Step 6 changes the package list.

## Index row

Every Linux job in nife's CI runs on Ubuntu 24.04, and GitHub made 26.04 generally available on
2026-09-17. The move is a label edit per job, but three things make it more than that: caches keyed
without the release, a `/tmp` that 26.04 puts in memory, and a canary that shows the instruction
counts did not move.
