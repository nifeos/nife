# E1 to E4: the cache experiments and what each reading says

*An appendix to [the register of measures](../register-of-measures.md). The name is provisional.*

## Radon, 2026-09-04

E1, E3 and E4 were re-taken on radon, the board all three were designed against. The run was six
boots on a `board,bench,single_hart` card, interleaved unpadded and padded, with one microSD card
written six times. Every boot printed `cntfrq 4000000` and reached `bench: done`; nothing
self-skipped. The capture is `bench/radon-2026-09-04/` and the reading is
notes/footprint-perturbation.md.

- E1 found a knee at 16 threads: 1.00x, 1.03x, 1.20x, 1.68x, then flat to 96 (the last four points
  span 1.2%). The prediction was a knee in the low tens, computed by name against radon's 32 KB
  L1i. It is recorded against `design/decisions/0096-process-kernel-or-event-kernel.md`'s input 3.
  The caveat: E2's customer path runs 4 to 8 threads, below the knee.
- E4 found displacement that is real and load-dependent. It is 0 to 1% at ordinary IPC load and 5
  to 8% at 96 threads. It peaks at a 32 KiB working set, which is exactly radon's L1d. That is three
  times the magnitude the dev Mac read, and the peak sits on the cache size rather than at an end
  of the sweep.
- E3 separated cleanly and cannot be attributed. The padded build is 1.49% slower on `call_reply`
  and 3.01% faster on `ipc_rtt_el0`, both non-overlapping across three boots each. Resident dead
  code cannot make anything faster. So the experiment measures code layout summed with footprint
  and reports the total. **Do not quote an E3 number as a footprint result.** The control that
  milestone 370 (a layout control, because the perturbation experiments cannot tell footprint from
  addresses) specifies was built on 2026-09-19, and milestone 796 (pin the hot trap path's
  placement) leaves it nothing to move (notes/footprint-perturbation/step-0-under-the-pin.md).
  calef retired E3's question on 2026-10-20 (UTC) in favor of E5, milestone 708 (an executed
  footprint ladder for radon), so E3 has no attributable number. The flaw was present in the
  2026-08-22 dev-Mac reading too; the small cache made it visible rather than causing it.

## Per-IPC kernel stack depth, 2026-09-19

This reading retires E1's one estimated input. E1's prediction assumed "roughly 1 to 2 KiB" of
kernel stack per IPC. It was measured by painting each thread's own stack around each operation.
In the release kernel radon boots, one round trip reaches about 600 bytes of each kernel thread's
stack (riscv64: 608 client, 576 server, E1's SEND/RECEIVE shape). EL0 threads reach 0.5 to 1 KiB,
trap frame included. The debug build reaches 2 to 4 KiB.

On 2026-10-20 the riscv64 release figures were re-taken at one commit, in QEMU and on radon. The
kernel-thread CALL/REPLY series match to the byte; every EL0 series reads 16 bytes deeper on the
board, where `single_hart` grows the trap path's frame. notes/stack-high-water/per-ipc-depth.md
has the comparison.

The row is `dated` rather than gated: the number never existed before, and nothing depends on it
yet.

It changes the reading of E1, not the curve. At 600 bytes a thread, stacks fill a 32 KB L1d near 54
threads, not 16. So capacity does not explain radon's knee at 8 to 16. Page-aligned stack tops
sharing set indices would, since radon's 32 KiB 4-way L1D allows at most 8 lines per page offset.
Page-aligned TCBs would too. notes/stack-high-water.md carries the arithmetic and the coloring
experiment that separates the two.

## The board path, 2026-09-04

The three rows whose 2026-08-22 readings all end with "and it wants a small-cache board" (E1, E3
and E4) now had a board to run on.

E1 and E4 were compiled for aarch64 only. The reason recorded in `bench.rs` was that this tree has
no riscv64 *accelerator* with a real cache. That is true, and it was never the same statement as
"no riscv64 machine with a real cache". Radon is a VisionFive 2 whose four U74s have 32 KB L1i and
32 KB L1d each, the number E1's prediction was computed against by name.

What was missing was a path to running there. It is now three things:

- the two benchmarks build for riscv64;
- the `single_hart` kernel feature parks the other three harts, since both experiments need one
  hart and a card has no `-smp 1` to pass;
- `script/board-image --bench` writes the card.

The procedure and its outcome table are in notes/footprint-perturbation.md.

This paragraph was written on 2026-09-04, before that evening's run, and the radon run of
2026-09-04 replaces it. The register's date column said "never on a board" rather than leaving it
to a reader's inference. That is the register's own complaint about the cross-OS row, applied to
itself. Nothing had been measured on silicon then. The board was powered off, and there was no bench session
on the day the instrument was made runnable.

## E3's padding moved underneath it

The padding was reachable only from `sched::ipc_send`. Milestone 188 (the IPC fastpath) phase 1
split the IPC shapes on 2026-09-04. On riscv64 that moved `ipc_send_receive` to 2.10x and
`ipc_call_reply` to 1.00x. E3 was doubling the footprint of the shape nothing in this tree runs,
and leaving untouched the shape every service issues. `sched::ipc_call` now pads too, and both
shapes read roughly 1.85x on both ISAs. The 2026-08-22 E3 reading was correct when taken, but it
describes a quantity that no longer means what it said.

## The dev-Mac readings, 2026-08-22

E1 through E4 were taken in the Tier A lane of milestone 134 (the register of measures), on the dev
Mac under HVF. None needed silicon, as the block promised. But three of them (E1, E3's
latency half, E4) need a *real cache* to say anything, and this tree's only accelerator with one is
HVF. So they self-skip under TCG rather than print a fictional number. TCG covers the default
icount instrument and every riscv64 run, since riscv64 has no HVF equivalent. E3's static
footprint measurement is exempt: it is `objdump`-based and runs on both ISAs with no QEMU at all.

### E2, taken first because it is cheapest

It came close to decisive. The naive reading of `sched::thread_count()` inside the SMB/FS gate test
read 95 (aarch64) and 82 (riscv64). Both were wrong: the full kernel test suite
runs 279 `#[test_case]`s in ONE continuous boot, so an absolute count taken partway through
includes whatever earlier tests left allocated.

The topology's own cost is the delta against a baseline taken at the top of the SAME test, before
it wires anything. It is 4 new threads on both ISAs: `net_stack`, the echo
client, the SMB adapter and the mDNS responder. The FS service (block server + FS server) and the
credential service add nothing: they are already running, latched from earlier tests
in the same boot and reused rather than re-spawned. A from-scratch boot would add roughly three
more, and a generous estimate is 7 to 8 total. Either number is deep in single digits, nowhere near
E1's knee. §96 (process kernel or event kernel, and how to decide it) is moot for this workload as
currently shaped. E2 was raised to check that before spending on E1.

### E1

`ipc_scale_N` sweeps N from 2 to 96 threads, via `tp_batch`/`tp_best` pinned to one hart. Across
three repeated runs it is flat at roughly 1,270 to 1,310 ns/iter from 2 through 16 threads. Then it
rises reproducibly to roughly 1,360 to 1,420 ns/iter (8 to 11%) by 64 to 96 threads.

The knee starts where predicted, in the low tens. The magnitude is small
because the dev Mac's L1d is far larger than the 32 KB `SiFive` U74 the prediction was built
against. A reproducible cost at 96 threads on a large-cache machine is the "positive result is
conclusive" case the block's own BUGS names. It argues for a small-cache board, not against the
mechanism. Read against E2, the customer path (4 to 8 threads) sits inside
the flat region, well below where any cost appears on this machine.

### E3

The static half: padding roughly doubles `ipc_fastpath` on both ISAs. On aarch64 it goes from 5,792
to 11,628 bytes (2.01x); on riscv64, from 5,088 to 10,152 bytes (2.00x). That confirms the padding works
before asking whether it costs anything.

The latency half ran on aarch64 only. `ipc_rtt` and `ipc_rtt_el0` move by 2 to 3% between the
padded and un-padded builds. That is inside the run-to-run noise both builds show independently:
repeated `--real` runs of the SAME binary vary by a similar amount. So there is no effect on this
machine, which the block's own BUGS calls the weak direction. The dev Mac's L1i comfortably holds
11.2 KiB, so a negative result here proves little and wants the same small-cache board E1 does.

### E4

E4 uses a 5-repeat minimum. The first unrepeated run swung 2 to 3x between nominally identical
conditions. That is a methodological finding in its own right: this workload's batches run long
enough to catch host preemption, and `smp_throughput`'s shorter ones do not.

Throughput lost to 8 concurrent IPC pairs (16 threads) is 0 to 5% across working sets from 4 to
128 KiB, over three repeated runs. The first cut read "0 to 3%"; two more runs on the same shared,
noisy machine widened it (see the E4 entry in BUGS). There is no effect worth calling one on this
machine. That agrees with E1: 16 threads of background traffic sits inside E1's own flat region,
below where E1 found any cost on this hardware.

The stronger follow-up, originally deferred, was taken 2026-08-23. A second background-load
condition, `SCALE_MAX_PAIRS`, joined the original 8-pair one in `app_displacement`. It runs 48 pairs (96 threads), the same pair count E1's own sweep tops out at. Over the same three
repeated runs, throughput lost at 96 threads reads 2 to 9%. It is higher than the low-load figure
at every one of the 5 working sets on every one of the 3 runs.

That is a small, reproducible, direction-consistent effect, not a knee. The two ranges (0-5% and
2-9%) overlap. E1's own equivalent point (94-96 threads) shows only an 8-11% cost, so a modest
displacement number at the same load is the mutually consistent reading, not a stronger
independent finding. It wants the same small-cache board E1 and E3's latency half want. On this
machine the two conditions are separated by a few percentage points, riding on run-to-run noise of
a similar size.

## The full commands

The register's Dated table keeps a short form of each command. These are the cells in full, as the
table carried them before the split.

| measure | last taken | the command that re-takes it |
|---|---|---|
| E1: IPC round trip against thread count | 2026-09-04 (radon, 6 boots); 2026-08-22 (dev Mac) | `cargo xtask bench --real` (`ipc_scale_*` rows); on radon, `script/board-image --bench` and notes/footprint-perturbation.md |
| E2: thread census on the customer path | 2026-08-22 | `cargo xtask test`, the "E2 thread census" line in `a_host_process_connects_to_the_guest_and_is_answered` (both ISAs) |
| E3: IPC fastpath footprint doubled, and the latency it costs | retired by calef, 2026-10-20; last read 2026-09-04 (radon, 6 boots), confounded, see the radon section; 2026-08-22 (dev Mac) | `script/fastpath-footprint --features fastpath_pad [--layout]` (both ISAs); `cargo xtask bench --real --extra-features fastpath_pad` against `cargo xtask bench --real`; on radon, eight images over `NIFE_FASTPATH_PAD`/`NIFE_FASTPATH_SHIFT` (the layout control, 2026-09-19) built by `script/board-image --bench --extra-features fastpath_pad`, procedure in notes/footprint-perturbation.md |
| E4: application working-set displacement under IPC traffic, at typical (8-pair) and high (48-pair, E1's-knee) background load | 2026-09-04 (radon, 6 boots); 2026-08-23 (dev Mac) | `cargo xtask bench --real` (`appdisp_*_ipc`/`appdisp_*_ipc96` rows); on radon, `script/board-image --bench` and notes/footprint-perturbation.md |
| per-IPC kernel stack depth, per shape and role (milestone 134) | 2026-10-20 (riscv64 release, QEMU and radon at one commit); 2026-09-19 (QEMU, all three ISAs, debug and release) | debug: `script/test`, the `ipc-stack-depth:` lines of `one_ipc_reaches_a_measured_depth_into_its_kernel_stack`; release: a `bench,ipc_stack_depth` kernel booted on one hart (notes/stack-high-water.md, "Per-IPC depth") |

## BUGS

- E1, E3's latency half and E4 need a real cache, and this tree has one accelerator that provides
  one. "Tier A needs no silicon" is true of the experiments' *design*. But a Rust benchmark still
  needs real caches to run on, and today that is HVF, aarch64-only. `cargo xtask bench
  --riscv` always runs under TCG, since no riscv64 accelerator exists in this tree. So all three
  self-skip there rather than print a fiction. It is the same self-skip shape
  `real_single_hart_or_skip` in `kernel/src/bench.rs` already uses for the icount case. This is not
  the Tier B kind of gap, a counter or an authority question that does not exist yet. This
  instrument needs hardware the tree already has, on one architecture. E3's static footprint
  measurement is unaffected (see the dev-Mac readings).
  The board of milestone 127 (the seL4 machine), when it lands, is the natural second data point, not a
  blocker for a first one.

- E2's naive reading was wrong. `sched::thread_count()` taken partway through the full test suite
  (279 `#[test_case]`s in one continuous boot) read 95 and 82 on the two ISAs. Both were dominated
  by threads that earlier, unrelated tests left allocated. The fix is in `system_tests/src/user/tests.rs`
  and `riscv_virtio_tests.rs`: a baseline taken at the top of the same test, with the census
  reported as a delta. Any future count of "how many threads does X create" taken inside the
  shared-boot suite should take the same delta rather than trust an absolute `thread_count()`.

- Closed 2026-08-23. E4's original background load (8 pairs, 16 threads) sat inside E1's own flat
  region. A null result there was expected from E1's curve, not independent evidence against
  displacement. `app_displacement` now also runs 48 pairs (96 threads, E1's own top pair count) as
  a second condition. The result, in the E4 section under the dev-Mac readings, is a small
  reproducible cost consistent with E1's own finding at the same load, not stronger than it. It is
  not a knee. The honest reading is that this machine's cache is still too large to see one, and
  that wants the small-cache board, not another thread-count sweep here.
