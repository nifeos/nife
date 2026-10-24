# Does an aarch64 CI host show a guest reorderings an x86_64 host hides?

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), measured 2026-10-24 (UTC) by the lane
maintainer/measure-arm64-host-reordering. Name: provisional (that lane). It tests the claim
`.github/workflows/ci.yml` gives for keeping the SMP QEMU jobs on `ubuntu-24.04-arm`. Milestone
587 (most CI jobs do not need an arm64 host) records that claim as recalled from QEMU's
documentation rather than measured.*

## The answer

Two halves, and they come apart.

- **The host half holds, strongly.** On an aarch64 host, multi-threaded TCG shows an aarch64 or
  riscv64 guest message-passing reorderings at rates from 170 to 88,000 per million messages, and
  a spinlock with a Relaxed unlock lost 20% of its updates. On an x86_64 host the same guest
  binaries showed none, in 3.8 billion messages over two runs and 96 million increments under a
  Relaxed unlock.
- **The suite half does not.** First every kernel spinlock's unlock was weakened to Relaxed, then
  its acquire too, so that no kernel lock orders anything. The aarch64 and riscv64 legs of the
  kernel suite passed on both hosts: 4 runs on aarch64, 5 on x86_64 in CI, all green.
  The likely reason is that the suite never contends a lock hard enough, inside the reorder window,
  to turn the exposure into a failure.

So the arm64 placement buys exposure the suite does not use. It is partial: it earns its cost only
with a dedicated contended test, which the tree does not have.

## What QEMU 11.1.2 predicts

`.qemu-version` pins 11.1.2. References are to its source tarball.

- A guest's own memory model is `TCGCPUOps::guest_default_memory_order`: 0 (no ordering) for Arm
  (`arm_tcg_ops`) and RISC-V (`riscv_tcg_ops`), TSO for x86 (`x86_tcg_ops`). A host backend's is `TCG_TARGET_DEFAULT_MO`: 0 for aarch64
  (`tcg/aarch64/tcg-target-mo.h`), TSO for x86_64 (`tcg/x86_64/tcg-target-mo.h`).
- Before each guest load or store, `tcg_gen_req_mo` (`tcg/tcg-op-ldst.c`) emits a host barrier
  for the orderings the guest model promises and the host lacks. An Arm or RISC-V guest promises
  none, so its plain loads and stores become plain host loads and stores on either host.
- Guest barriers become `tcg_gen_mb`. `DMB` maps by type (`trans_DSB_DMB` in
  `target/arm/tcg/translate-a64.c`); `STLR` is a full barrier then the store (`trans_STLR`); `LDAR`
  the load then a full barrier (`trans_LDAR`); a RISC-V `FENCE` of any kind is a full barrier
  (`trans_fence` in `target/riscv/tcg/insn_trans/trans_rvi.c.inc`).
- The x86_64 backend emits a barrier only when it must order a store before a later load, as
  `lock orl`; the aarch64 backend emits `DMB ISH`, `ISHLD` or `ISHST` (`tcg_out_mb` in each
  backend's `tcg-target.c.inc`).
- Every guest core gets its own host thread whenever the target supports it and `-icount` is off
  (`tcg_init_machine` in `accel/tcg/tcg-all.c`). The suite does not use `-icount`; only `cargo xtask bench` does,
  single-hart.

The prediction: on an x86_64 host the guest sees exactly TSO. Message passing (MP) cannot reorder
however weak the guest's code, and store buffering (SB) can. On an aarch64 host the guest sees what
the host's own cores allow, which includes MP. One more falls out of the third bullet: `STLR` then
`LDAR` puts no barrier between the store and the load, so an aarch64 guest's SeqCst store followed
by a SeqCst load can reorder on either host, which Armv8 forbids.

## The method

### The litmus guest

`tools/host_reordering_litmus` is a bare-metal program for QEMU's `virt`
machine, built for `aarch64-unknown-none-softfloat` and `riscv64imac-unknown-none-elf`, the
kernel's own targets, so its atomics lower the way the kernel's do. Four cores; the aarch64 build
turns the MMU on so the litmus lines are Normal, inner-shareable memory. Each test's instruction
order was checked in the disassembly: the compiler keeps program order for every variant, so a
reordering counted is the host's.

- MP, streamed, two pairs at once. The writer stores `data = i` then `flag = i`; the reader loads
  `flag` then `data` and counts `data < flag`, impossible unless a pair was reordered. The flag
  store is Release or Relaxed, the load Acquire or Relaxed. Rates are per million messages sent.
- SB, in batches of 256 fresh slots. Both loads reading 0 is the reordering; TSO allows it, so it
  is the positive control that the guest cores really ran in parallel.
- LOCK. All four cores increment one counter under a spinlock shaped like `spin` 0.12.3's
  `SpinMutex`, which the kernel's `IrqSafeMutex` wraps; unlock Release or Relaxed. Lost updates
  are the reordering.

### The weakened suite

`weaken-spin.sh` points the workspace at a copy of `spin` 0.12.3 with the guard's unlock Relaxed
(`release`), or that and the lock's compare-exchange Relaxed (`both`). Then `script/test --arch
aarch64,riscv64 --no-host` runs, `NIFE_SMP` at its default of 4, as many times as fit a bounded
job. The build logs show the patched crate compiled in.

### The hosts

A temporary workflow ran the same steps on `ubuntu-24.04-arm` (Neoverse-N2, 4 vCPUs)
and `ubuntu-24.04` (AMD EPYC 7763, 9V45 and 9V74, 4 vCPUs), QEMU 11.1.2 from the CI cache, LITMUS_SCALE 60.

## The counts

Runs 40067282280 (MP, SB; suite `release`) and 40068823516 (MP, SB, LOCK; suite `both`). Each
cell is per million, first run / second run.

| guest, test, ordering | aarch64 host | x86_64 host |
|---|---|---|
| aarch64 MP release/acquire | 0 / 0 | 0 / 0 |
| aarch64 MP relaxed/relaxed | 87,802 / 82,635 | 0 / 0 |
| aarch64 MP release/relaxed (reader reorders) | 170 / 259 | 0 / 0 |
| aarch64 MP relaxed/acquire (writer reorders) | 11,834 / 25,461 | 0 / 0 |
| riscv64 MP release/acquire | 0 / 0 | 0 / 0 |
| riscv64 MP relaxed/relaxed | 61,624 / 68,025 | 0 / 0 |
| riscv64 MP release/relaxed | 600 / 671 | 0 / 0 |
| riscv64 MP relaxed/acquire | 3,700 / 4,479 | 0 / 0 |
| aarch64 SB relaxed | 19,551 / 17,453 | 16,726 / 210,567 |
| aarch64 SB SeqCst | 50,275 / 1,640 | 7,146 / 61,431 |
| aarch64 SB SeqCst fence | 0 / 0 | 0 / 0 |
| riscv64 SB relaxed | 2,298 / 3,491 | 8,015 / 24,886 |
| riscv64 SB SeqCst, and SeqCst fence | 0 / 0 | 0 / 0 |
| aarch64 LOCK, Release unlock | (not run) / 0 | (not run) / 0 |
| aarch64 LOCK, Relaxed unlock | (not run) / 201,943 | (not run) / 0 |
| riscv64 LOCK, Release unlock | (not run) / 0 | (not run) / 0 |
| riscv64 LOCK, Relaxed unlock | (not run) / 5,290 | (not run) / 0 |

Per million messages for MP (240 million a test), slot pairs for SB (61 million), increments for
LOCK (48 million). MP repeats within a factor of 2.2. SB swings by up to 30 times between runs,
because how often two cores reach the same slot together depends on how the barrier releases them;
it is a presence test, not a rate. Every zero is zero in both runs.

The suite, weakened. Every run passed all 141 and 388 aarch64 tests and all 144 and 385 riscv64
tests (14 skipped in each `system_tests`, as unweakened):

| weakening | aarch64 host | x86_64 host |
|---|---|---|
| `release` (40067282280) | 2 runs, green | 3 runs, green |
| `both` (40068823516) | 2 runs, green | 2 runs, green |

Two more `release` runs on a local x86_64 machine passed every test and failed only the page frame
ledger, which the same checkout also fails unweakened. That is a checkout-location effect, now in
the BUGS of [the page frame budget](../frames/page-frame-budget.md), and not the weakening.

## What it means for the arm64 jobs

The SMP QEMU jobs on `ubuntu-24.04-arm` are the only CI jobs that put guest code where a missing
Release can be seen. The suite they run does not look: no kernel lock with no ordering at all
failed a test on either host. On today's suite the placement buys nothing measurable for lock
ordering, and it spends runner time in the queue that waits longest.

The exposure is real and cheap to use. The LOCK test, a minute of guest time, turns a Relaxed
unlock into 20% lost updates on an aarch64 host and nothing on x86_64. A kernel test of that shape
(cores contending one `IrqSafeMutex` and checking the data it guards, with `weaken-spin.sh` as its
falsification) would make one arm64 job earn its place. Without it, rule 4 of the codebase rules is
held by loom, by review and by the HVF runs on Apple silicon, and not by CI's host.

The x86_64 guest legs on arm64 (`test-x86_64`, `rg-threads-x86_64`) are a different case, a guest
stronger than its host; nothing here measures them.

## BUGS

- **The suite has no contended-lock test, so nothing in CI fails on a weakened kernel lock.**
  Measured above; the shape of the test that would is in the section before this one.
- **QEMU 11.1.2 does not order an aarch64 `STLR` before a later `LDAR`.** Predicted from the
  source and observed on both hosts (the SB SeqCst row). It can fail correct aarch64 code that
  relies on SeqCst store-then-load (a Dekker handshake), so it is a false failure, never a false
  pass. riscv64 is unaffected: Rust's SeqCst load there carries a leading `fence rw,rw`. It is in
  [notes/qemu.md](../qemu.md)'s BUGS.
- Weakening a lock is one protocol. A suite that cannot catch it could still catch some other
  missing ordering, so "the suite catches nothing on arm64" is measured for locks and inferred for
  the rest.
- 4 and 5 suite runs, 2 litmus runs per host. Each host was one runner model per job, of a fleet
  GitHub can change.
- `helpers/qemu-runner-x86_64.sh` measured round-robin vCPUs for an x86_64 guest on an aarch64
  host under 11.1.1. The 11.1.2 source has no memory-order check left that would choose it
  (the "vCPU Scheduling" section of
  `docs/devel/multi-thread-tcg.rst` still describes one). Not resolved here.
