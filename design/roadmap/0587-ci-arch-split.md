---
status: BUILT
raised: 2026-09-24
built: 2026-09-24
promoted_from: ci-arch-split
---
# 587. Most CI jobs do not need an arm64 host, and the arm64 queue is where the wait is

Built, 2026-09-24 (pull request #1249). *(Number minted at promotion.)* Promoted from the proposal
`ci-arch-split`, filed 2026-09-24 by the `maintainer/ci-arch-split` lane, which the maintainer
briefed after three merge-group builds waited over an hour at 19:05 UTC with 13 jobs running, all
of them `ubuntu-24.04-arm`. calef ruled option B on 2026-09-24 (*"Yes, proceed with B"*), below
under "The ruling". The text after this paragraph is the proposal's own except for that section,
its gate line (dropped once BUILT), `## What was built`,
`## Follow-on` and the `## Index row` section. On 2026-10-12 (UTC) its measurement moved into an
appendix under §212 (a prose budget), with a short summary left in its place.

## What is being decided

Whether to move the jobs that do not need an arm64 host onto `ubuntu-24.04` (x86_64), keeping
arm64 only where the host architecture changes what a job proves.

## The ruling

Option B, calef, 2026-09-24. Move the 15 jobs listed under "The recommendation" to
`ubuntu-24.04`, and keep `build + test`, `cpu matrix` and `re-falsify` on arm64. Add `prove the
kernel on aarch64` inside the `verify (Kani proofs)` aggregate. Correct `ci.yml`'s header, which
says testing on aarch64 is "the one place CI catches" ordering bugs, a claim the host tests do not
bear out (see "What is lost").

Done means three things. The implementation's own run shows every moved job green on x86_64 and
the aarch64 kernel proof green. This block becomes BUILT in that pull request. The first
merge-group builds after it lands are measured against the 16:00 to 18:00 rows of the hourly table in
[measurement.md](0587-ci-arch-split/measurement.md).

The re-measure trigger. One week after the implementation merges, re-run the created-to-started
queue measurement over that week's `ci.yml` and `verify.yml` jobs, by label. If the arm64 jobs that
remain still wait a median over ten minutes in busy hours, the next lever is option D (the x86_64
guest legs out of `build + test`). If the x86_64 jobs have started waiting like arm64 did, the
premise that x86_64 supply is looser was a single afternoon and this is revisited.

## What was built

- `ci.yml`: every job but `test` (`build + test (host + QEMU)`) and `cpu-matrix` runs on
  `ubuntu-24.04`, the `draft gate` included, so a run's first job no longer waits in the arm64
  queue. The header's weak-ordering paragraph is rewritten with the evidence below: the host tests
  race no threads, so the claim holds only for the SMP guest legs, which are the two jobs kept.
- `verify.yml`: `gate`, `scope`, both `prove` shards and the `verify (Kani proofs)` aggregate
  run on `ubuntu-24.04`; `falsify` stays on arm64. `prove-kernel-x86_64` became
  `prove-kernel-aarch64` rather than gaining a sibling: with the shards on x86_64 their own
  `kernel` row proves `arch/x86_64/`, so the old job would repeat it and `arch/aarch64/` would be
  proved nowhere. The aggregate judges the new job with the same vocabulary, so it is inside the
  required check exactly as the shards are. The runner count per run is unchanged; the arm64 share
  of it went from 18 jobs to 4.
- Caches cannot cross hosts, and nothing was needed to make that so: `Swatinem/rust-cache`
  keys on the host (the measurement's cache keys read `Linux-x64` and `Linux-arm64`) and the QEMU
  cache key already carries `runner.arch`. The one shared key is the vendored tarball cache, which
  holds `.crate` files that are the same bytes on either host.
- Collision: rebased over #1220 (A′, a push to `main` cites the merge group that tested it),
  which edits the same two `gate` jobs. Its `permissions:` blocks and push branch are kept whole;
  only the `runs-on:` line beside them changed.

The implementation's own run, dispatched on the branch before merge: `ci.yml` 36060856009 and
`verify.yml` 36060858268, both `success`. Every moved job green on `ubuntu-24.04`; `prove the kernel
on aarch64` green on `ubuntu-24.04-arm` in 0.8 minutes, proving `arch::aarch64::iommu::proofs`
alongside the system call harnesses; the aggregate printed "the kernel proved on aarch64 too". Both
shards proved on x86_64 in 15.6 and 16.3 minutes of wall time.

## The evidence, in brief

On the afternoon of 2026-09-24 the organization's 60-job cap was not close, and x86_64 jobs in the
same runs waited far less than arm64 ones. One run of each workflow on `ubuntu-24.04` then matched
the arm64 results on every job but one, and that one was an artifact of the copy. No job uses KVM or
any accelerator; what decides whether a job needs an arm64 host is whether the host's architecture
changes what gets proved or tested.

- [measurement.md](0587-ci-arch-split/measurement.md) holds the queue evidence and its hourly
  table, how the x86_64 measurement was taken, and every job classified with its timings and what
  matched beyond pass and fail.

## The recommendation

Move 15 of the 18 arm64 jobs to `ubuntu-24.04`, keep three on arm64, and add one small arm64
job. This is a reversible change: each job is one `runs-on:` line, and reverting it is the same
line.

- To x86_64: both draft gates, verify scope, the `verify (Kani proofs)` aggregate, rustfmt,
  clippy, fastpath footprint, reproducible build, image permissions, stack frames, bench, supply
  chain, fuzz, coverage, and both `prove` shards.
- Stay on arm64: `build + test`, `cpu matrix`, and `re-falsify`.
- New on arm64: `prove the kernel on aarch64`, the mirror image of the existing
  `prove the kernel on x86_64` job (`script/verify --only kernel`, measured at 0.1 minutes of
  proving on the reference run), folded into the `verify (Kani proofs)` aggregate the same way.
  Without it, moving the shards to x86_64 would leave `arch/aarch64/` proved nowhere, which is the
  exact gap milestone 304 closed in the other direction. The shards would then prove the `kernel`
  row on x86_64 as a duplicate of the existing x86_64 job; five seconds, and removable later.

What it buys. On the reference run, every arm64 job waited behind a draft gate that itself
queued 11 minutes on arm64, and then 47 to 62 minutes more. Moving both gates removes the serial
arm64 wait at the front of every run. And a full run's arm64 demand drops from 18 jobs to 4, three
of them the long QEMU jobs that genuinely want the host, so the queue that remains is for work that
earns it.

Why the two QEMU jobs stay, and why only weakly. Recalled from QEMU's documentation, not
measured here: multi-threaded TCG runs each guest core on its own host thread and translates guest
loads and stores to plain host ones. So an aarch64 or riscv64 guest on an aarch64 host can observe
reorderings the host permits, while on an x86_64 host it sees only what TSO allows. The tree already
records the other half of that rule. `helpers/qemu-runner-x86_64.sh` notes that QEMU refuses
parallel cores for an x86_64 guest on an aarch64 host (it falls back to round-robin), which is what
a host weaker than its guest looks like. So the SMP kernel legs on arm64 are the only place CI puts
guest code in front of weak-memory reorderings. Moving them would remove that silently. "Weakly"
because nothing in this tree records a failure those legs caught that an x86_64 host would have
missed. The value is a plausible class, not a demonstrated one.

Would we still choose this if both options cost the same? Yes. The recommendation keeps every
job whose result depends on the host where it is, and moves the ones measured to produce identical
results. The case is capacity, not effort, and the effort is small either way.

## What is lost, said plainly

- Host tests on an aarch64 host, for the jobs that move. Coverage and fuzz would run on x86_64
  builds. This costs almost nothing today, and that is a measurement rather than a hope. In the host
  crates, one non-loom test spawns a real thread (`memory_corruption_canary_gate`'s `bounded`,
  which asserts liveness through a channel). The concurrent protocols (`work_steal_slot`,
  `thread_wake_handshake`, `memory_regions`, `clock_protocol`, `memory_corruption_canary_gate`) are
  checked for ordering by loom, which models C11 on any host and is not in CI at all
  (milestone 80 (loom: the hand-rolled atomic protocols, model-checked)). So `ci.yml`'s header
  sentence that testing on aarch64 "is the one place CI catches that class" is not true of the
  host tests, which exercise no cross-thread ordering. It is true, in the recalled-QEMU sense
  above, of the SMP guest legs, which this proposal keeps on arm64. That header should be corrected
  whichever way this is decided. And `build + test`'s own host-test pass stays on arm64 under this
  proposal, so nothing about host ordering changes unless option C is taken.
- "Assume weak memory ordering" (AGENTS.md's fourth rule) is not weakened by the recommended
  split, for the reason above. It would be weakened by option C: every guest SMP leg would run
  under TSO, and a missing acquire in kernel code that shows up only as a reordering would pass CI.
  The dev Mac (aarch64, and HVF runs real cores) would still see it, but lanes now gate in CI rather
  than locally (`briefs/gate-in-ci.md`), so the dev Mac is no longer where most lanes' code runs.
- `elf`'s host tests would exercise the `x86_64` accept arm, not the `aarch64` one, on any job
  that moves and runs them. Milestone 288 (host tests that assume the host is aarch64) made those
  tests host-neutral, so they pass either way; what changes is which arm is exercised. Under the
  recommendation only coverage is affected.
- The CI host stops matching the dev Mac's architecture for most jobs. `ci.yml`'s header gives
  "passes locally means passes in CI" as a second reason for arm64. It matters less than it did
  because lanes no longer gate locally.
- Cold caches, once. The first x86_64 runs pay a QEMU source build (6 to 7 minutes) in each
  QEMU job and a `cargo install` of the lint tools. The recommended split moves only `bench` among
  the QEMU jobs, so that cost lands on one job.

## The options

- A. Status quo. Everything on arm64 except the one x86_64 Kani job. Costs the waits measured
  in [measurement.md](0587-ci-arch-split/measurement.md) whenever the arm64 pool is short, which
  on 2026-09-24 was most of the afternoon.
- B. The recommendation above. 15 jobs move, three stay, one small arm64 job is added.
- C. Everything to x86_64, with `prove the kernel on aarch64` and `re-falsify` kept on arm64
  because they cannot move without losing proofs. Removes almost all arm64 demand. Loses the SMP
  guest legs' weak-host exposure described above; gains a faster x86_64 guest leg (12.4 to 7.9
  minutes of OVMF shell check).
- D. B, plus splitting the x86_64 guest legs out of `build + test` into an x86_64 job. The
  in-tree comment on that job already names the x86_64 leg as "the one to move to a job of its own"
  if it grows. It would shorten the arm64 job by about ten minutes and run that leg about a third faster. It
  is a larger edit than B (the `test`, `swish-check` and `boot-check` rows would need an `--arch`
  split in `script/ci-build`), so it is listed rather than recommended.

## What is blocked until this is answered

Nothing is blocked. The queue drains when arm64 supply returns, as it did by 19:30. What this
decides is how often a lane waits an hour for a runner, and DECISIONS §203 (capacity is rented
rather than bought) already names "merge throughput" as one of the three things runners buy.

## BUGS

- One x86_64 sample. Queue waits come from a full afternoon of real runs; the per-job timings
  come from one run per host, with a different commit behind the arm64 Kani reference.
- The weak-host argument for the QEMU legs is recalled, not measured. Measuring it would need a
  deliberately broken ordering in kernel code that fails on an arm64 host and passes on x86_64,
  which is its own piece of work.
- The x86_64 guest under multi-threaded TCG on an x86_64 host was not confirmed. It passed at
  `-smp 2`; whether QEMU chose parallel cores or round-robin there was not checked.

## Follow-on

- **Milestone 648.** Milestone 648 (re-measure CI queue waits by runner label, a week after the arm64 split). The one-week re-measure of queue waits by runner label, and option D as the next
  lever if arm64 still waits: `design/roadmap/0648-ci-queue-remeasure.md`, dated so its age
  shows in `script/roadmap --proposed` rather than living only in this block's trigger paragraph.
- **Recorded.** That multi-threaded TCG shows an aarch64 or riscv64 guest the host's reorderings is
  recalled from QEMU's documentation, not measured; the header of `.github/workflows/ci.yml` says
  so where it keeps `test` and `cpu-matrix` on arm64 for that reason.
- **Decision.** The coverage job now runs the host suite on x86_64 on every pull request, which is
  part of what `design/decisions/0184-an-x86-64-host-in-the-host-pass.md` asks about (and milestone
  403 (an x86_64 host in the host pass) builds). It is coverage's pass, not `script/test`'s, so the
  section's question stays open; the maintainer should tell whoever holds it.

## Index row

On 2026-09-24 arm64 hosted runners were the bottleneck, not the 60-job cap: ~20 jobs ran while 150+ waited. One measured x86_64 run of every job matched arm64's results, so 15 of 18 jobs move to `ubuntu-24.04`. The two SMP QEMU jobs and the arch falsification replay stay on arm64, and a one-minute aarch64 kernel proof keeps `arch/aarch64/` proved.
