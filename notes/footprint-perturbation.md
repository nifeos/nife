# Taking a benchmark on radon: E1, E3 and E4 on the small-cache board

*(Milestone 134. Named by calef on 2026-09-04, from three candidates. `board-bench.md` was refused
for a collision the file itself could not fix: `notes/bench-runbook.md` already exists and answers
"which machine should an evening be spent on", so two pages about benching on boards would have had
names that do not tell them apart. This one is named for the experiment, which is milestone 134's own
word for E3, so a reader looking for "does the fastpath's size cost anything" finds it. That it is a
board procedure is the first line's job rather than the filename's.)*

The session was run on 2026-09-04, and its conclusions are below, under "What the session
measured". Six boots on radon, interleaved, one card written six times. The tables and the reasoning
behind each reading are in [the session appendix](footprint-perturbation/session-2026-09-04.md).
The next evening is planned first, because it is the part a reader acts on.

The headline is not the one this page was written to expect. E3 separated cleanly in all three
rows and one of them has the wrong sign, which says the experiment is confounded by code layout
rather than that its hypothesis is false. The two experiments riding along, E1 and E4, produced the
more decisive results of the evening.

*(The board columns were empty before the session and are filled now. The static footprints
are still `objdump` on patagonia, and the 2026-08-22 figures are still dev-Mac runs; both are in
[background.md](footprint-perturbation/why-it-runs-on-radon.md).)*

## The next radon evening: E3 under the layout control, and how it shares a night with 168

*(Written 2026-09-19 by milestone 134's stack-depth lane, rewritten that day by the control's lane.
It says what each half of an evening shared with notes/job-mix.md needs, and in what order.)*

What is already taken and does not need the board again. E1 and E4 ran on radon on 2026-09-04
and both are single-build sweeps, so the confound below does not touch them; they are quotable as
they stand. The per-IPC kernel stack depth, which E1's prediction used to estimate, is now measured
(notes/stack-high-water.md, "Per-IPC depth").

What is owed is E3, and only E3 under a layout control. Re-running E3 as it was built would
reproduce the 2026-09-04 confound exactly (this page's BUGS: a second card or a second board
changes nothing about addresses, so it would look like confirmation). The control is built, as
of 2026-09-19: `kernel/src/fastpath_pad.rs`, sized by two build-time variables.

### The knob, in one paragraph

`fastpath_pad` still turns E3's padding on. Two environment variables, read by `kernel/build.rs`
only when that feature is on, say how much. `NIFE_FASTPATH_PAD=<units>` is the sled's length in
units of the old pad (5,092 bytes on riscv64; unset is 1, and 0 is the guard and a `ret`).
`NIFE_FASTPATH_SHIFT=<bytes>` appends uncounted zero bytes after it. The linker script places the
sled right after `.text.hot`, the hot trap path that milestone 796 (pin the hot trap path's placement) pins on a 16 KiB boundary after
the boot stub. So both numbers move all kernel text after that block by that many bytes, and neither
moves the block. A pad and a shift of
equal displacement are therefore the same binary apart from bytes nothing fetches. This is milestone
370 (a layout control, because the perturbation experiments cannot tell footprint from addresses).
How the pin was checked, why these are variables rather than features, and why every image must come
from one commit are in [layout-control.md](footprint-perturbation/layout-control.md).

### The eight images, and why these sizes

| image | `NIFE_FASTPATH_PAD` | `NIFE_FASTPATH_SHIFT` | the whole kernel text moves by | what it is for |
|---|---|---|---|---|
| pad 0 | 0 | 0 | 0 | the reference for both ladders |
| pad 1 | 1 (or unset) | 0 | 5,088 (79.5 L1i sets) | the 2026-09-04 sled |
| pad 2 | 2 | 0 | 10,176 (159 sets) | |
| pad 3 | 3 | 0 | 15,264 (238.5 sets) | |
| layout A | 0 | 820 | 820 (12.8 sets) | a small layout draw |
| layout B | 0 | 2460 | 2,460 (38.4 sets) | another |
| match 1 | 0 | 5088 | the same bytes as pad 1 | pad 1's twin, uncounted |
| match 2 | 0 | 10176 | the same bytes as pad 2 | pad 2's twin, uncounted |


Each matched image is byte-identical to its pad outside the sled, so a difference between the two is
the instrument failing. The four un-padded images are the layout distribution. Why the small shifts
are 820 and 2,460 (U74 branch-target alignment and L1d page sets) is in
[layout-control.md](footprint-perturbation/layout-control.md).

### Step 0, on patagonia, before anyone walks to the bench

```sh
git log -1 --format=%h                     # the commit every image below is built from
for c in 0:0 1:0 2:0 3:0 0:820 0:2460 0:5088 0:10176; do   # pad:shift
  NIFE_FASTPATH_PAD=${c%%:*} NIFE_FASTPATH_SHIFT=${c##*:} script/fastpath-footprint \
    --arch riscv64 --features board,bench,single_hart,fastpath_pad --layout
done
```

Two lines out of each run are the check, and they are the reason this step exists:

- `layout: code hash <...>` must be the same 16 hex digits for all eight. It is a hash of
  every instruction in both IPC closures and the entry set, with address operands normalized away.
  Those are direct call and branch targets, `auipc`/`adrp` uppers, and the low-12 immediates that
  pair with them, and the script prints how many of each it touched. Equal hashes are the proof that the eight
  images execute the same code and differ only in where it sits. A different hash means something
  changed the fastpath itself and the evening is measuring two things again.
- `layout: fastpath_pad_body at <addr>, <n> bytes` must show the sled at the same address in
  all eight, with the requested length, and the line under it must read `the sled precedes 16 of
  16 hot symbols, 100% of their bytes`. That is the pad reaching the hot path. Anything less means
  the pad perturbs only part of the path, or none of it, and on a tree with milestone 796's pin it
  reads 0 of 16 (the next section).
- The two matched images must place the hot path exactly where their pads do. Compare the
  `layout: 0x...` lines of `PAD=1` against `SHIFT=5088`, and `PAD=2` against `SHIFT=10176`: every
  address should be equal.

The rest of each readout (every hot symbol's L1i set, line phase and alignment) is the pre-registered
layout record; what it carries is in [layout-control.md](footprint-perturbation/layout-control.md).

### Step 0 fails on a tree with milestone 796's pin, so E3 as specified cannot run

Step 0 ran on 2026-10-20 (UTC) at `68647cfc5`. The code hash and the sled address matched in all
eight images, and every image read `the sled precedes 0 of 16 hot symbols, 0% of their bytes`.
Milestone 796 pins the hot trap path in `.text.hot`, ahead of the sled, so no pad or shift moves a
hot instruction and the eight images are one hot-path layout. No boot ran. The evidence, the cause
and the fork it leaves for an architect (rebuild the sled inside the hot block, retire E3's
question, or run the images as built; retiring is recommended) are in
[step-0-under-the-pin.md](footprint-perturbation/step-0-under-the-pin.md).

### The boots

```sh
NIFE_FASTPATH_PAD=0 NIFE_FASTPATH_SHIFT=0 \
  script/board-image --bench --tftp --extra-features fastpath_pad
script/board-console --for 20m --until none --log bench/radon-<date>/bench-e3-pad0-1.log
```

- Eight images, three boots each, interleaved, in the order pad0, pad1, pad2, pad3, A, B,
  match1, match2, then around again twice. Interleaving is this page's own rule (step 5 of
  [the two-build procedure](footprint-perturbation/two-build-procedure.md)), and it is what keeps
  a room warming up out of the comparison. Twenty-four boots at about 75 seconds each is roughly
  half an hour of booting plus a rebuild between images. `--tftp` (milestone 257 (boot radon over the
  network)) makes an image change a rebuild and a power cycle rather than a card write.
- Check the first two lines of every capture. `bench: cntfrq 4000000` says this is the board
  and not QEMU `virt`. `bench-probe: fastpath_pad units <u> shift <s>` says which image booted,
  which nothing on a card said before 2026-09-19 and which is the mistake an interleaved run of
  eight images is most likely to make. The last line must be `bench: done`.
- `script/board-image` echoes the two variables on its `features:` line, and refuses to build if
  they are set without `fastpath_pad` in the feature list.

### Reading it

Three rows matter, as before: `call_reply` (the shape services run, and the one the verdict should
rest on), `ipc_rtt`, `ipc_rtt_el0`.

1. Read each matched pair first, pad 1 against `SHIFT=5088` and pad 2 against `SHIFT=10176`.
   They are the same binary outside a sled nothing fetches, so a difference beyond the
   boot-to-boot spread means the instrument is measuring something nobody has accounted for, and
   the rest of the reading is suspect until it is explained.
2. Then take the layout distribution. Pad 0 and the layout images differ in nothing a CPU
   executes, so whatever spread appears across them is the layout effect at these
   displacements, per row. Report it as a range, not a mean.
3. Then read the pad ladder against it. With the sled pinned, a pad and a shift of equal
   displacement are the same binary, so the honest question the ladder answers is whether
   displacement costs time and whether more of it costs more. A row that rises monotonically
   and leaves the layout range says it does; one that jumps and comes back is a particular
   alignment rather than a trend; one inside the boot-to-boot spread is nothing. None of those
   is a footprint result, which is the paragraph below.
4. Keep every boot's `cycles_per_tick` line, so the rows convert to cycles (M5) on the
   evening's own commit.

| what the capture shows | what it means | where it routes |
|---|---|---|
| each pad sits on its matched twin, and the pads move no further than the layout images do | the counted footprint predicts nothing: doubling and tripling it costs no more than moving the same code the same distance | §95 (a hand-written IPC fastpath, and whether it can stay proven)'s premise is in serious doubt. 188 phase 4 would buy a standing verification obligation for an effect this instrument cannot find; route to `design/decisions/0095-*` |
| a pad differs from its matched twin, beyond the boot-to-boot spread | impossible on the physics as understood, since the two binaries differ only in bytes nothing fetches. Something else differs between the images or the measurement is not what it seems | nothing routes until it is explained; it is the instrument's own check failing |
| the pad ladder rises monotonically and leaves the layout range, matched twins tracking their pads | displacement costs time and more displacement costs more, which is the strongest reading available here. It is a layout result stated honestly, not a footprint one | 188 phase 4: the magnitude is what a hand-written fastpath would have to beat, and notes/benchmarks.md gets a caveat on every stored baseline |
| the layout range is itself large (several percent) | the 2026-09-04 reading was an artifact, as suspected, and every between-build comparison in `bench/` inherits the same exposure | notes/benchmarks.md, as a caveat on stored baselines; and E3 as built cannot answer §95 at all |
| everything inside the boot-to-boot spread | this kernel's IPC path does not care where it sits, at these displacements, on this core | 188 phase 4 loses its last cheap instrument; M6 is what is left |
| rows disagree (one moves, another does not) | shape-specific, which is a result about *which* path to optimize | 188 phase 4, narrower than sketched |

What this cannot decide, and it should be said before the numbers exist. The padding is never
executed, so it cannot evict anything on its own; the only way it can reach a clock is by moving
other code. E3 therefore tests whether the number `script/fastpath-footprint` reports predicts
latency, which is what milestone 188 (the IPC fastpath) leans on, rather than Liedtke's claim about an *executed*
footprint. That claim needs either M6 (instruction-cache misses per IPC) or a perturbation that
adds executed instructions, and neither is this experiment.

Optional, one boot, if there is time: a card built with `--extra-features ipc_stack_depth`
prints the per-IPC depth lines on the board itself. It should reproduce the QEMU release numbers
to the byte (the `board` and `single_hart` features are the only differences); a disagreement would
mean one of them changes the IPC path's codegen, which nothing currently expects. Its timing rows
are not results, since the instrument paints on every sample.

### Sharing the night with milestone 168

E3 is eight images now, where it was two. The halves cannot share an image (`script/board-image`
refuses `--bench` with `--job-mix`), and `--tftp` makes switching a rebuild and a power cycle. The
night is shared with milestone 168 (a multi-tasking workload benchmark). E5, the executed-footprint
test E3 cannot be, needs a build and an evening of its own:
[its plan](footprint-perturbation/executed-footprint.md).

| order | what | boots | why this order |
|---|---|---|---|
| 1 | on patagonia: step 0 above, all eight images | none | the check that invalidates the whole E3 block if it fails, and it needs no board |
| 2 | E3, `script/board-image --bench --tftp --extra-features fastpath_pad` per image | 3 per image, interleaved | interleaving is the design, so it goes first while the room and the board are at one temperature |
| 3 | milestone 168's job mix, `script/board-image --job-mix --tftp` | at least 5 | notes/job-mix.md, "The next bench evening on radon", steps 0 to 8, unchanged |
| 4 | optional: one `--bench --extra-features ipc_stack_depth` boot | 1 | last, because nothing is blocked on it |

Log names: `bench-e3-pad<n>-<boot>.log` and `bench-e3-shift<bytes>-<boot>.log` here,
`jobmix-boot<n>.log` for 168's, both under `bench/radon-<date>/`. Write the commit down once. The
console rules are notes/job-mix.md's step 0 for both halves: one capture per serial port, and clean
each log with `tr` before committing it.

## What the session measured, 2026-09-04

Six boots on radon, interleaved unpadded/padded, one card written six times, by
[the two-build procedure](footprint-perturbation/two-build-procedure.md). Every boot printed
`bench: cntfrq 4000000`, `bench: cycles_per_tick 250.00` and `bench: done`, and nothing self-skipped.
The tables are in [session-2026-09-04.md](footprint-perturbation/session-2026-09-04.md).

- E3 separated cleanly in all three rows, with a boot-to-boot spread of 0 to 2 ticks. The padded
  `call_reply` was 19 ns (+1.49%) slower. The padded round trip across EL0 was 193 ns (3%) *faster*.
- Dead code that is never executed cannot make anything faster. So the 193 ns is code layout, and
  the 19 ns cannot be attributed to footprint either. It is not a null result: the instrument is
  confounded, rather than the hypothesis false.
- Phase 4 of milestone 188 (the IPC fastpath) stays where it is. A layout control decides it, and
  that control is the one this page now plans.
- E1 has a real knee: latency rises 68% between 2 and 16 threads, then is flat to 96. The effect that
  §96 (process kernel or event kernel) worries about is real on radon. The customer path runs 4 to 8
  threads, below the knee.
- E4 displacement peaks at a 32 KiB working set, radon's L1d exactly, and needs heavy IPC load: 0 to
  1% at ordinary load, 5 to 8% at 96 threads.

## Why it runs on radon, and what was in the way

E3 of milestone 134 (the register of measures) read 2 to 3% on patagonia, inside noise. A negative
result on a large-cache machine proves little, and radon has 32 KB of L1i per U74. Three things kept
E1, E3 and E4 off the board until 2026-09-04, and one was a finding: the padding sat on a shape
nothing runs. The static footprint table, riscv64 `ipc_call_reply` 5,936 bytes unpadded and 11,070
padded, is 18% and 34% of radon's L1i. All of it is in [background.md](footprint-perturbation/why-it-runs-on-radon.md).

## The two-build procedure, and reading it

The 2026-09-04 session wrote one card six times, unpadded and padded interleaved, three boots per
condition. E1 and E4 still run that way: a `single_hart` card removes the fifteenfold placement lottery
of milestone 240 (the soak reports what happened and not where). The bench list, the six steps and the
shell transcript are in [two-build-procedure.md](footprint-perturbation/two-build-procedure.md). How
it controlled placement, and the outcome table it was built to fill, are in
[reading-a-two-build-session.md](footprint-perturbation/reading-a-two-build-session.md).

## What milestone 74's riscv64 half adds, and what it still cannot see

Milestone 74 (cycle counters) reads real cycles on radon at 250 per tick. That adds precision, not a
new answer: the 2026-09-04 within-condition spread was 0 to 2 ticks against a gap of 74, so the ruler
was never E3's problem. It still cannot see the mechanism, because nothing reads an
instruction-cache miss counter (M6). The full section is in
[background.md](footprint-perturbation/why-it-runs-on-radon.md).

## BUGS

The full list, with the reasoning for each, is in
[limitations.md](footprint-perturbation/limitations.md). The ones that change how a number is read:

- E3 has no attributable number yet. The 2026-09-04 reading summed footprint and layout, and the
  control cannot run as specified on a tree with milestone 796's pin, since no pad or shift reaches
  the hot path ([step-0-under-the-pin.md](footprint-perturbation/step-0-under-the-pin.md)).
- The padding is never executed. Even a clean dose-response tests the counted footprint, not
  Liedtke's claim about an executed one; [E5](footprint-perturbation/executed-footprint.md) plans
  that.
- Four layout images are a small sample, so they bound an effect loosely and cannot prove one absent.
- The bench card's kernel is not the kernel the static table measures, since `bench` changes the IPC
  path's codegen. Step 0 measures the card's own feature set for this reason.
- Everything here is one card in one board on one evening. Nothing in CI compiles a card
  (`design/roadmap/0373-board-only-features-nothing-compiles.md`).
- A `single_hart` card skips `smp_throughput`, `fs_read` and `fs_throughput`.
