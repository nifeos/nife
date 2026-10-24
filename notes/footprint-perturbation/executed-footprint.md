# E5: does an executed fastpath cost time on radon

*An appendix to [`notes/footprint-perturbation.md`](../footprint-perturbation.md), whose "The next
radon evening" section is the evening's plan. Written 2026-10-03 (UTC) by the
`maintainer/radon-footprint-experiment` lane. Name provisional, the directory and the stem both;
naming is an architect's. E5 is a provisional label too, the next free one after E4.*

Status: a plan, not runnable. Nothing here has run, and the instrument it needs is not built.
The build is proposed in
[`design/roadmap/0708-an-executed-footprint-ladder-for-radon.md`](../../design/roadmap/0708-an-executed-footprint-ladder-for-radon.md).
This page says what to build, how to run it, and what each outcome means, before any number exists.

## The question

On radon (SiFive U74: 32 KiB L1i, 2-way, 64-byte lines), does executing more fastpath bytes cost
measurable time, with code layout held fixed?

It is the question the 4 KiB target waits on. calef ruled on 2026-09-21 that the target is reported
and not gated. Whether 4 KiB is the right number waits on milestone 370 (a layout control) and
belongs with milestone 132 (the fast path's footprint) and milestone 188 (the IPC fastpath). Both
halves of that ruling are recorded at `L1I` and `TARGET` in `script/fastpath-footprint`.

Where the path stands, from `bench/fastpath-riscv64.txt` (saved 2026-09-26 under
`nightly-2026-09-26`): `ipc_call_reply` 6,116 bytes (1.49x the target, 19% of the L1i) and
`ipc_send_receive` 4,778 bytes (1.17x).

## Why E3 cannot answer it, even under the control

E3's sled is never executed. `kernel/src/fastpath_pad.rs` reaches it only through
`core::hint::black_box(false)`, so the bytes sit in `.text` and nothing fetches them. Milestone 370
made a pad and a shift of equal size byte-identical outside the sled. That pair is the cleanest test
of counted bytes this tree has, and the physics predicts no difference between its two halves.

So E3 under the control answers two other questions. Does the number `script/fastpath-footprint`
prints predict latency? And does moving the kernel's text cost time? Its own BUGS says the same:
Liedtke's claim is about an executed footprint, which needs instructions that run.

Growing the dead sled with placement held fixed is exactly E3's matched pair, which is already on
the evening's plan. E5 is the executed version, and it needs new code.

## The design

Three pieces, all built at compile time from one commit, as E3's images are.

### A fixed region, so layout cannot move

Every E5 image reserves the same region, 20 KiB, in the section milestone 370 places right after
`.text.hot`. The region's contents change between images and its size never does. Every symbol after
it therefore sits at the same address in every image. `script/fastpath-footprint --layout` already
proves this for E3, with a code hash that normalizes addresses away; E5 reuses that check.

### An executed chain, and a dense twin for each rung

`ipc_call` and `ipc_send` call a chain of N jump instructions, unconditionally in an E5 build, from
the call sites E3's `maybe_pad` uses today.

- Sparse: one jump per 64-byte line, so N jumps touch N lines, N times 64 bytes.
- Dense: the same N jumps at an 8-byte stride, so they touch N times 8 bytes.

The two execute the same number of instructions and take the same number of branches. Every target
is 8-byte aligned in both, because the U74 predicts a taken jump with no bubble only then (U74-MC
Core Complex Manual 21G3.02.00, section 4.2.6, as `kernel/src/fastpath_pad.rs` cites it). So the
sparse image minus its dense twin is footprint alone, with instruction count and branch count held
equal.

| N | sparse adds | dense adds | `ipc_call_reply` executed, about |
|---|---|---|---|
| 0 | 0 | 0 | 6.0 KiB, the reference |
| 16 | 1 KiB | 128 B | 7.0 KiB |
| 32 | 2 KiB | 256 B | 8.0 KiB |
| 64 | 4 KiB | 512 B | 10.0 KiB |
| 128 | 8 KiB | 1 KiB | 14.0 KiB, 44% of the L1i |
| 256, optional | 16 KiB | 2 KiB | 22.0 KiB, 69% of the L1i |

The right-hand column is an upper bound, since `script/fastpath-footprint` counts whole symbols.
Nine images are required (N = 0 is shared) and two more are optional.

### An application that wants the same L1i

E4 already measures application displacement, but its working set is data: `appdisp_workload` in
`kernel/src/bench.rs` reads and writes a buffer, so it competes for the L1d. Liedtke's claim is
about the kernel evicting the application's code too, and nothing in the tree measures that.

E5 adds a code working set. A workload thread runs a sparse chain of W bytes, then one `call_reply`
round trip, and repeats. W is 16, 24 and 28 KiB, chosen to sit under the 32 KiB L1i so that only
the kernel's share decides whether it still fits. On a `single_hart` card the two share one L1i.

Each boot prints, per W, the time per chain pass and the time per round trip. It also prints an
idle `call_reply` row with no workload running. A probe line names the image
(`bench-probe: e5 lines <N> stride <s> region <bytes>`), the way 370's probe line names E3's.

## What exists, and what does not

| piece | what it gives E5 | what is missing |
|---|---|---|
| Milestone 370 (BUILT 2026-09-19) | the pinned-first section, `NIFE_FASTPATH_PAD` and `NIFE_FASTPATH_SHIFT`, the `--layout` code hash, a probe line per image | the sled is dead by design; a pad unit is 5,092 bytes on riscv64, too coarse for 1, 2, 4 and 8 KiB rungs |
| Milestone 74 (cycle counters), riscv64 half, built 2026-09-04 | SBI PMU cycles; radon measured `cycles_per_tick 250.00` on 2026-09-16 | it configures one counter for one event, CPU cycles; nothing else |
| The bench rows | ticks at 4 MHz, convertible to cycles at 250 per tick | milestone 374 (cycles per IPC) is not built, and E5 does not need it: on 2026-09-04 the spread within a condition was 0 to 2 ticks on 1,000-iteration rows |
| An L1i miss count | nothing; no architecture reads a cache-miss counter | see below |
| E4 | the shape of a displacement sweep in `bench.rs` | a code working set |

The miss counter is optional, and this paragraph is from memory rather than read. The SBI PMU
specification defines generic hardware cache events, an L1I read miss among them. The U74 manual
lists an instruction cache miss among its memory-system events. Whether radon's OpenSBI maps either
to a counter depends on its device tree, and nobody has checked. A probe that prints whether
firmware accepted the event costs one boot and settles it. The decision rule below rests on cycles,
so E5 runs without misses; misses would show the mechanism rather than the cost.

The build is therefore four pieces, and it is a lane rather than an evening:

1. An executed chain with a fixed region, under a feature of its own, sized by two build-time
   variables: one for lines and one for stride. The build lane names them.
2. The code-working-set workload and its rows in `kernel/src/bench.rs`.
3. `--layout` extended to check the region's address and size are equal across images.
4. Optional: an L1I miss event in `kernel/src/arch/riscv64/pmu.rs`, behind a probe line.

aarch64 has E3's sled already, and DECISIONS §19 (architectural parity) wants the chain on both
ISAs, so argon can run E5 later against its 48 KiB L1i.

## Step 0, on the build machine

The code does not exist, so `LINES` and `STRIDE` below stand in for whatever the build lane names
its two variables, and `fastpath_exec` for its feature.

```sh
git log -1 --format=%h          # every image comes from this commit
for c in 0:64 16:64 32:64 64:64 128:64 16:8 32:8 64:8 128:8; do   # lines:stride
  LINES=${c%%:*} STRIDE=${c##*:} \
    script/fastpath-footprint --arch riscv64 \
    --features board,bench,single_hart,fastpath_exec --layout
done
```

Three checks, each of which voids the evening if it fails:

- The code hash is the same for all nine images.
- The region starts at the same address with the same size in all nine.
- Every hot symbol's `layout: 0x...` line is equal across all nine.

## The boots

```sh
LINES=0 STRIDE=64 \
  script/board-image --bench --tftp --extra-features fastpath_exec
script/board-console --for 20m --until none --log bench/radon-<date>/bench-e5-n0-s64-1.log
```

Three boots per image, interleaved: all nine in order, then around twice more. Check the first
lines of each capture as E3 does. `bench: cntfrq 4000000` says it is the board, and the E5 probe
line says which image booted.

## The time budget

Twenty-seven boots. A bench boot took about 75 seconds on 2026-09-04. The W sweep adds three
pressure conditions per boot; at the rate E4's sweep ran, call it a minute more, not measured. With
a rebuild and a power cycle between images under `--tftp`, that is roughly 100 minutes.

That does not fit beside E3's 24 boots and the five of milestone 168 (a multi-tasking workload
benchmark) on one night. Give E5 its own evening. Milestone 168 moves risk 4's verdict, and nothing
in that verdict waits on E5.

## Reading it, and what each outcome means

For each row and each N, take F(N), the sparse image's time minus its dense twin's. The noise is
the largest boot-to-boot spread any one image shows on that row. An effect counts only when F(N)
clears that spread on every boot, and rises with N.

| the capture shows | what it means | where it goes |
|---|---|---|
| F within the spread at every N through 128, idle and at every W | no measurable cost from executing up to about 14 KiB of path on radon | the 4 KiB target moves, with this data as its reason; a proposal to milestones 132 and 188, and the architect decides the new figure |
| F flat idle, rising with N under W | Liedtke's mechanism is live, and the application pays | the target stands with a measured price per KiB; the code shrinks |
| F rising idle, with nothing competing | a cost while everything still fits, which capacity cannot explain | suspect fetch or branch prediction first; read the miss counter if it exists before routing anything |
| dense rungs not monotone against N = 0 | the instrument is wrong, since dense images differ only in jump count | nothing routes until it is explained |
| spread wider than any F | no verdict | record the spread, as milestone 168's step 7 does |

The 16 KiB ceiling of DECISIONS §144 (the fastpath footprint gate gets a delta and a ceiling) sits
beyond every required rung, so no outcome here moves it.

## If the code has to shrink, what goes first

Outlining milestone 151 (notification objects)'s bound-notification receive check off `ipc_receive`'s
common path looks like the obvious first move, and it is already done. `take_bound_signal` in
`kernel/src/sched.rs` is `#[cold]` and `#[inline(never)]`, and DECISIONS §101 (notification objects)
priced what remains on the path at one load and one branch.

The candidate the tree itself names is `syscall::dispatch`, the general decoder that seL4's fastpath
exists to skip. [`kernel-footprint-and-caches.md`](../benchmarks/kernel-footprint-and-caches.md)
calls it the first place to look. Skipping it is milestone 188 phase 4, which is an architect's
call.

## BUGS

- Nothing here is built, and nothing has run.
- A sparse chain is a model of a footprint, not a fastpath. It fetches one instruction per line where
  real code uses the whole line. If the U74 prefetches the next line, the chain could hide misses
  that real code would not, or the reverse. A stride of 128 bytes is a cheap extra rung that would
  show it.
- One fixed region is one layout draw. The L1i is 2-way, so which sets the chain shares with the hot
  path matters, and E3's layout images are the range to read E5 against.
- One board, one card, one hart, as with every E3 reading.
- An N = 0 image still calls the chain and returns, so every rung pays that call; it cancels between
  rungs and does not cancel against an ordinary build.
