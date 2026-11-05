---
status: BUILT
raised: 2026-09-04
built: 2026-09-04
---
# 375. Run the footprint-perturbation experiment on radon, where a cycle counter can see it

Checked on 2026-09-19, the record shows six boots on radon, interleaved unpadded and padded, one card
written six times, every boot printing `bench: cntfrq 4000000` and `bench: cycles_per_tick 250.00`.
That is milestone 74 (cycle counters)'s riscv64 PMU reading real cycles on the machine this asked
for. The capture, the procedure and the result are notes/footprint-perturbation.md.

**The result is not the one this file was written to expect, and that is the finding.** E3 separated
cleanly in all three rows and one of them has the wrong sign: the padded build is 3.01% *faster* on
`ipc_rtt_el0`, and the padding is never executed. So the experiment is confounded by code layout
rather than its hypothesis being false, and no E3 number should be quoted as a footprint result
until a layout control exists. That control is milestone 370. The other half this file handed off,
a cycle column on the rows themselves rather than one conversion probe beside them, is milestone
374 and is not built: the rows are still tick counts at a 250 ns quantum.

**Half of it is built, 2026-09-04** (the maintainer/e3-on-radon lane). E1, E3 and E4 now compile and
run on a riscv64 board build: `script/board-image --bench` writes a `board,bench,single_hart` card,
`--extra-features fastpath_pad` writes its padded twin, and the procedure with its outcome table is
notes/footprint-perturbation.md. **No number has come off radon**; the board was powered off and there was no
bench session, so what remains of this proposal is the session itself plus the cycle counter, which
is `design/roadmap/0374-cycles-per-ipc-on-the-bench-card.md`.

That lane also found something this proposal assumed away. The padding was reachable only from
`ipc_send`, so measured on riscv64 it moved `ipc_send_recv` to 2.10x and `ipc_call_reply` to
1.00x. E3 was padding the shape nothing in this tree runs, which is the shape milestone 188 (the IPC
fastpath) phase 1
had just established as the one that matters. `ipc_call` calls `maybe_pad` too now, and both shapes
pad to roughly 1.85x.

## In brief

`script/fastpath-footprint` bounds a quantity, not a harm. Liedtke's argument is that a kernel
touching a lot of memory per IPC evicts the *application's* working set, so the bill arrives as
capacity misses spread through the workload. Nothing in this tree has ever observed that: icount
models no cache, and the HVF development host's L1i is several times the boards'.

Milestone 134's E3 was the experiment built to test it without a PMU. It runs the kernel with
`--features fastpath_pad`, which pads the fastpath with `nop` to roughly double its footprint, and
measures what happens. Run under icount on 2026-08-22 it reported 2 to 3% latency effect for a 2x
footprint growth, which is what a tripwire that cannot see a cache can see.

Radon can now see cycles. Milestone 74's riscv64 half landed 2026-09-04
(`kernel/src/arch/riscv64/pmu.rs`, the SBI PMU extension), on a SiFive U74 with a 32 KB L1i,
which is the binding constraint DECISIONS §144's 16 KiB ceiling was derived from. E3 on that machine
is the same experiment with an instrument that can answer.

## Why it is worth a lane rather than a paragraph

It is the measurement that decides milestone 188 phase 4, a hand-written IPC fastpath, which is
a standing verification obligation and a permanent maintenance cost. That block's own recommendation
is to wait for exactly this. Phases 1 to 3 established that the cheap methods leave the shape the
system runs 48 to 103% over the 4 KiB target. So the arithmetic case for phase 4 is as strong as it
will get, and the empirical case does not exist.

Two outcomes, both useful. If a 2x padded fastpath costs nothing measurable in cycles on a 32 KB
L1i, fatal risk 4 gets its best evidence yet and phase 4 should be refused in writing. If it
costs something, phase 4 has a number to be measured against for the first time.

## What it is not

**It is not the full Liedtke experiment.** A cycle count says whether the round trip got slower; it
does not attribute the cost to instruction-cache displacement, and it says nothing about the
*application's* working set, which is Liedtke's actual claim. That wants cache-miss events
attributed across a workload, which is milestone 134's tier B and milestone 127's silicon. This
proposal is the cheap half that can run today, and it should say so wherever it reports.

## What it needs first

- `kernel/src/arch/riscv64/fastpath_pad.rs` exists, so `--features fastpath_pad` works on radon.
  (x86_64 has no such module, which holds xenon out; recorded in `script/fastpath-footprint`'s
  BUGS.)
- The board rig: radon's UART into cordoba, smart plug 2. See notes/target-hardware.md.

## Follow-on

- **Milestone 370.** A layout control, because E3's separation cannot be attributed. The 3.01%
  wrong-sign row is what this session found and it is why no reading of E3 is a footprint result
  yet: `design/roadmap/0370-a-layout-control-for-the-perturbation-experiments.md`.
- **Milestone 374.** Cycles per IPC on the bench rows themselves. The session read
  `cycles_per_tick 250.00`, which converts a tick and does not subdivide one, so E3's verdict is
  still quantized at 250 ns: `design/roadmap/0374-cycles-per-ipc-on-the-bench-card.md`.
- **Recorded.** This is not the full Liedtke experiment and the block says so where it reports. A
  cycle count says whether the round trip got slower and never attributes the cost to
  instruction-cache displacement, nor says anything about the application's working set. That wants
  cache-miss events across a workload, which is milestone 134's tier B, and nothing in this tree
  reads a cache-miss counter on any architecture.
- **Recorded.** The padding reached only `ipc_send`, so on riscv64 it moved `ipc_send_recv` to 2.10x
  and `ipc_call_reply` to 1.00x, which is E3 padding the shape nothing in this tree runs. Fixed
  before the session by having `ipc_call` call `maybe_pad` too, and recorded in
  notes/footprint-perturbation.md beside the procedure.

## Index row

Milestone 134 (the register of measures)'s E3 pads the IPC fastpath to roughly 1.85x and asks what
that costs. It had only run under icount, which models no cache, on a host whose L1i is several
times a board's. The `maintainer/e3-on-radon` session ran it on a SiFive U74 with the 32 KB L1i that
DECISIONS §144's ceiling was derived from: six interleaved boots, one card written six times, with
milestone 74's SBI PMU reading real cycles. E3 separated cleanly in all three rows and one has the
wrong sign, the padded build being 3.01% faster on a padding that never executes. Code layout
confounds the experiment, and it needs milestone 370 (a layout control). E1 and E4, riding along,
produced the more decisive results.
