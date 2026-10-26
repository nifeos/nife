# icount drift, and what a baseline was read against

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds why the tripwire is 10%, the toolchain
and emulator stamps, and the 2026-07-28 `-smp 4` artifact, with the dates, tables and corrections
behind them. Name: ratified 2026-09-24 (calef); [the naming record](README.md) holds it.*

## What the icount instrument cannot see

TCG models no caches, no TLB and no branch prediction. So a change that is count-neutral but
cache-hostile passes `--check` silently. The roadmap block states this limit too. The `--real`
numbers are the net that catches what counts cannot, read by a human rather than a gate.

## A correction: the counts drift across builds, so "attributable to the commit" was too strong

The original milestone-21 note said a count change "is a change in a code path, attributable to the
commit that made it." Building the EL0 primitive suite disproved the attribution half. Milestone 25 (cross-OS performance comparison)
folds in the fix.

icount is deterministic per binary (byte-identical runs, verified twice). It is not stable across
different binaries. Adding the `null_syscall_el0` bench, which touches no other benchmark's code,
moved `yield_switch` -7% and `ipc_rtt` +1.8% at the same time. Two controlled facts pin the
mechanism:

- A dead function added to `bench.rs` moved nothing. So it is not raw code layout (addresses do not
  change instruction counts anyway).
- The shifts are non-uniform and opposite in sign across benchmarks. So it is no common-mode offset
  to subtract out.

What remains is the compiler's whole-crate decisions. Adding live code that calls into shared
functions (`sched::spawn`, `user::run`) changes inlining and monomorphization elsewhere. So other
functions' executed-instruction counts move, each its own way. The session's drift also held a real
increase from the 19f object-capability refactor, which genuinely grew the scheduler and thread hot
paths. The instrument cannot separate that from the codegen churn.

### The fix (milestone 25)

`--check` became a coarse 10% tripwire in place of a 2% gate. It still catches a gross regression
("you 3x'd IPC"). It no longer pretends to attribute a 3% wiggle to the commit in front of it. The
`--real` medians, read by a human, are the fine signal, and a few-percent codegen shuffle is already
in their noise.

Ideas not taken, and why:

- Pinning hot-path layout: fragile, and layout was not the cause.
- Per-operation deltas that cancel fixed overhead: the shift is in the measured body, not the fixed
  overhead, so it would not cancel.
- Common-mode subtraction: the shifts are not common-mode.

### The floors name the nightly they were read against (2026-09-21)

A toolchain bump revalues every number in `bench/baseline-*.txt`, and no commit is responsible for
it. icount counts guest instructions, and a new compiler emits different ones for the same source.
On 2026-09-15 the pin went to `nightly-2026-09-15` with the floors left alone, and most of the
headroom went with it. The tree took another bump, to `nightly-2026-09-20`, before anybody acted,
and the drift then happened to evaporate. Measured 2026-09-21, the worst margin was +0.02% on
aarch64 and +2.27% on x86_64, with riscv64's `rfence_self` 7.49% faster than its floor (see
[the `rfence_self` appendix](rfence-self-row.md)).

So each baseline carries a `# toolchain: nightly-YYYY-MM-DD` line. `cargo xtask bench --save`
writes it from the pin rather than from whichever compiler happens to be running. `script/lint`
fails when that line and `rust-toolchain.toml` disagree. It can only fire on a branch that raises
the pin: that is where the delta arises, where a human is already deciding something, and where
re-recording is attributable. A general staleness check would have failed branches that caused
nothing.

Auto-re-saving on a bump is refused (calef, 2026-09-21). The refusal appears where somebody would
reach for it, in `script/toolchain-bump` and `.github/workflows/toolchain-bump.yml`. A floor that
tracks the compiler by construction moves by exactly as much as a nightly moved the kernel. A
nightly that made this kernel slower would then report nothing, and catching that is most of what
the tripwire is for.

Restamping is allowed, and that half of the refusal is overturned (calef, 2026-09-26, milestone 598
(a nightly bump restamps the floors it proves it did not move)). The refusal left a person
re-recording three floors on every daily bump, and each of those saves said it could not separate the
compiler from drift already on `main`. `cargo xtask bench --restamp` builds the same tree under the
stamped nightly and the pin on one runner, which separates them exactly. When every row moved under
0.5%, and under 2% compounded since the last `--save`, it rewrites the `# toolchain:` line and adds a
`# why:` line carrying the measurement; otherwise it changes nothing and the bump stays red. No number
moves, so the stamp now means the nightly the numbers were last proven valid for.

Who re-records, and whether the bump then waits, changed on 2026-09-24. calef ruled:

> Our default should be to merge and we should only block it if there is a compelling reason to.

The refusal above stands: a person or a maintainer session still reads the delta and writes the `--why`. But that is
not a hold. Once the floors are re-recorded and CI is green, the bump merges like any other pull
request. It is held with `needs-architect` only for a compelling reason: a changed proof or
falsification result, or a bench move beyond tolerance. A lint silenced only by a crate- or
workspace-level allow counts too, as do a changed test output and a compile break too large to fix
small. The list lives in
`.github/workflows/toolchain-bump.yml`'s header.

To separate the compiler from the code, compare the bump pull request's bench job with the
merge-group bench run of its base commit: same tree, old nightly. `--check` alone compares against
the floors, which also carry whatever drift `main` accumulated since the last save. The first bump
under the ruling, `nightly-2026-09-24` (PR #1250), is the worked example. On identical source,
aarch64 and x86_64 did not move at all and riscv64 `ipc_rtt` got 0.58% faster. The x86_64 floors
were already 0.4% to 2.3% under the tree on `main`.

**The compiler term depends on the host that runs the compiler** (2026-10-06, PR #1755). The bump
to `nightly-2026-10-06` moved aarch64 `ipc_rtt_el0` by +0.603% on the ubuntu-24.04 runner, in both
the restamp job and the ordinary bench job, so the restamp refused. The same `--restamp` on
patagonia moved no row by 0.01%, and the two nightlies' EL0 benchmarker came out
instruction-identical there. The two hosts already disagree on the count itself (10,975,081 on the
runner against 10,966,032 on patagonia under the old nightly, same tree), so a host term rides
inside every number. Its cause is unexplained. The floor was restamped from patagonia with the
runner's larger terms written into the cumulative ledger, so the next CI restamp compounds onto what
CI measured. If a bump refuses on one host and passes on the other again, that is the case to
investigate, by diffing the guest binaries the two hosts build from one commit.

### The emulator is the other half of the same fact (2026-09-21)

An icount count is a function of two things: the compiler that emitted the instructions and the
emulator that counted them. The section above made the first a record. The `rfence` lane found the
second while chasing a benchmark that had apparently got 7.49% faster. `.qemu-version` pinned
11.0.2, this machine had run 11.1.1 since 2026-08-28, and `script/bench` never checked. Every
committed baseline had been measured against an emulator nobody recorded and nothing verified.
`script/qemu-check`'s own warning text asserted the baselines "were recorded against" the pin, which
was an assumption stated as fact.

The stamp records the emulator that ran, not the pin. `rustup` resolves the compiler from
`rust-toolchain.toml`, so there the pin and the thing that ran are one fact by construction (the one
escape, `RUSTUP_TOOLCHAIN`, is named in the code). Nothing resolves QEMU from `.qemu-version`. It is
a wish about the machine, and on 2026-08-28 the machine stopped granting it. So `cargo xtask bench
--save` asks the binary, and when the answer differs from the pin the line says both.

The check compares the stamp against the emulator in front of it. It lives in `--check` rather than
in `script/lint`, because there is no emulator in a repository. Two comparisons were available:

| compared against | what it would do |
|---|---|
| `.qemu-version` | fails any baseline truthfully recorded off-pin, so it forbids the file from stating the truth, and still passes a comparison run on a third version |
| **the emulator this run used** | asks the only apples-to-apples question there is: is the counter reading this floor the counter that produced it |

`unrecorded` is a truthful answer and does not fail, the posture milestone 115 (the names that were
refused) takes for names. Every baseline carried it on 2026-09-21, because nobody had written down
which QEMU produced those counts. The check fires at full strength from the first honest `--save`
onward, and never on a number nobody stamped.

A failure does not mean "upgrade something". A re-record and the pin have to land in the same
commit, because CI builds the pinned emulator (`script/ci-qemu`) and reads these floors on it. Which
version to run stayed an architect's call.

*Correction, 2026-09-24: both open points above have since closed. `e9c8723c3` (2026-09-21) pinned
`.qemu-version` to 11.1.1, the version this machine had been running. All three
`bench/baseline-*.txt` now carry `# qemu: 11.1.1` rather than `unrecorded`. Milestone 302 (a
baseline records what it was saved against) merged on 2026-09-23 (#1126): each baseline also carries
a `# why:` line, and `script/bench --save` requires `--why "<reason>"`. The current files' `# why:`
line reads "unrecorded, predates milestone 302", because the saves they hold came before it.*

*2026-10-10: the pin moved again, 11.1.1 to 11.1.2, after Homebrew upgraded this machine (calef ruled
it the same day, as its own pull request apart from any nightly bump). The emulator term was measured
rather than assumed: one tree and one nightly under both emulators, 11.1.1 built from source for the
comparison. Every row on all three architectures read the same count on both, so the term is zero for
that binary, as it was for 11.0.2 to 11.1.1. The floors were re-saved to stamp `# qemu: 11.1.2`; their
`# why:` lines carry what moved against the old floors and why none of it was the emulator.*

## 2026-07-28: the day `--check` failed on every primitive, and why it was the harness

Roughly eight merges landed on `main` in one day. They were milestone 32 (a real filesystem) phase 1 block writes, 16b
IOMMU, 28 line discipline, 27 std, 30 net in three stages, 31 capability shell, and 22 phase A's
fault endpoint with the DESTROY force-kill amendment. None ran `bench --check`, because bench is not in
the `script/test` gate. When the dust settled, `--check` failed IMPROVED on four primitives and
REGRESSED on two, all far past the 10% tripwire:

| primitive | old baseline (smp=4) | HEAD (smp=4) | reported delta |
|---|---|---|---|
| call_reply | 1,876,614 | 965,679 | **-49%** |
| spawn_reap | 1,769,595 | 175,185 | **-90%** |
| ctx_switch | 6,308,105 | 3,648,857 | **-42%** |
| ipc_rtt_el0 | 21,364,834 | 11,950,853 | **-44%** |
| map_el0 | 408,897 | 575,752 | **+41%** |
| spawn_el0 | 3,132,783 | 3,633,897 | **+16%** |

Improvements that large and that uniform are suspicious; eight unrelated merges do not hand a -90%
on spawn for free. Bisecting the merge points, one deterministic icount run each, showed the tell at
once. `coremark`, which is pure compute and touches no OS primitive, moved +63% at the
capability-shell merge (20.9M to 34.1M ticks). `spawn_reap` did not creep, it jumped: 1.77M at the
base, 172k three merges later, 3.5M one merge after that. A compute loop cannot move 63% because a
kernel merged a socket API.

### Root cause: the aarch64 icount bench ran `-smp 4`, and CNTVCT is global under icount

The bench reads `CNTVCT_EL0` (`arch::timer::now()`) around each loop. Under `-icount shift=0` all
vCPUs share one deterministic virtual-instruction clock. So that counter advances with the global
instruction stream across every hart, not just the core running the benchmark. The aarch64 runner
defaults to `-smp 4` (`NIFE_SMP:-4`, matching the SMP tests), and the bench never overrode it.

Each measured window therefore silently counted three other harts' idle loops. Worse, under
`-icount` an idle secondary hart parked in `wfi` jumps virtual time forward to the next timer tick.
That dumps a large quantized lump of ticks into whatever window happens to be open. Add the
load-balanced spawner, and the count for `spawn_reap` or `ipc_rtt_el0` becomes a function of how
four harts interleaved, which any code change perturbs. The result is deterministic per binary, so
`--check` "worked" and the old baseline looked stable. It was the machine's four-hart idle pattern,
sampled, not the primitive's path length.

The proof is a re-run at `-smp 1`. With a single hart the counter advances only with the bench
thread, and the same commits that swung wildly at `-smp 4` go flat:

| primitive | baseline | iommu(16b) | cap-shell(31) | HEAD | smp=1 spread |
|---|---|---|---|---|---|
| coremark | 20,914,947 | 20,913,678 | 20,913,678 | 20,913,678 | ~0.006% |
| spawn_reap | 166,860 | 170,952 | 170,952 | 175,890 | +5.4% |
| ctx_switch | 2,664,204 | 2,679,734 | 2,680,270 | 2,685,272 | +0.8% |
| ipc_rtt_el0 | 9,603,751 | 9,694,986 | 10,005,385 | 10,101,111 | +5.2% |
| call_reply | 956,768 | 956,769 | 956,893 | 963,080 | +0.6% |
| spawn_el0 | 1,574,632 | 1,609,770 | 1,750,084 | 1,754,438 | +11% |
| ipc_rtt | 861,095 | 864,935 | 861,346 | 922,720 | +7.2% |
| null_syscall | 427,706 | 457,705 | 457,705 | 457,705 | +7.0% |

`coremark` is invariant to five decimal places, the sanity check the smp=4 run failed. The `-42%` to
`-90%` improvements and the `+41%` regression were entirely the smp=4 artifact. They attribute to no
merge's code: the old baseline froze one sample of the four-hart interleaving and that day's merges
reshuffled it.

### This was a known bug on one ISA and an unfixed one on the other

The riscv bench path already pinned `NIFE_SMP=1`, with a comment describing this exact failure ("a
`wfi` jumps virtual time to the next timer tick, inflating the spawn primitives to timer-quantized
nonsense"). That fix landed with the riscv icount bench (commit 494514b) and was never mirrored to
aarch64. So the aarch64 icount instrument had measured four-hart noise since milestone 21 (performance measurement).

The fix is one line: the aarch64 icount path now sets `NIFE_SMP=1` like riscv, and the baseline is
re-saved at single hart. Real per-core magnitudes still come from `--real` (HVF). There each core
keeps its own counter and parallel harts do not inflate elapsed time, so SMP is not a confound.

### What actually moved, once the noise is gone

At `-smp 1` every primitive is within ~11% of the old baseline's intent, and the real movement is
small and mostly explained:

| primitive | true delta (smp=1) | cause | assessment |
|---|---|---|---|
| ipc_rtt | +7.2% | the IPC mailbox widened 3 words to 5 (milestone 22 (trusted init) §26 (the fault endpoint), the fault-message carrier); every `ipc_send`/`ipc_receive` now copies five words via `wide()` | real, small, expected; the step lands exactly at the M22 merge (cap-shell 861k to HEAD 923k) |
| ipc_rtt_el0 | +5.2% | same mailbox widening, on the EL0 path | real, small |
| spawn_el0 | +11% | the M31 SPLIT rights-inheritance change (child budget gets full delegable rights); spawn_el0 does a SPLIT + retype per iteration | real, small; the step lands at the cap-shell merge (1.61M to 1.75M) |
| null_syscall | +7.0% | one-step at the blk-write/iommu merges, then flat; kernel layout/codegen drift in the syscall entry path, not a redesign | codegen drift, in the build-to-build drift documented above |
| spawn_reap, map_new, map_el0, call_reply, ctx_switch | +0.6% to +5% | whole-crate codegen churn across eight merges | codegen drift, expected and sub-tripwire |
| coremark, yield_switch | ~0% | pure compute / tight kernel yield, no structural change | invariant, as they should be |

No merge needed reverting and no path needed investigating. The only defect was the harness. No
bench had an elided loop or an early exit; the fiction was the counter, reading four harts where the
benchmark meant one. The new smp=1 baseline is the first aarch64 icount baseline that measures the
primitive rather than the machine, and it now agrees in shape with the riscv one.
