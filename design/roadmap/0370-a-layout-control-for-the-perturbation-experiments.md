---
status: BUILT
raised: 2026-09-04
built: 2026-09-19
---
# 370. A layout control, because the perturbation experiments cannot tell footprint from addresses

Filed as a proposal on 2026-09-04 out of the E3 session on radon,
promoted by milestone 433 (drain the proposal pile to zero) on 2026-09-19, built the same day. "What was built" below is the record.
Everything after it is the proposal's own argument, kept because it is why this exists.

The promoted block said `Gate: MILESTONE 134`, and that was backwards, which is worth recording
rather than deleting: the gate read as though the control had to wait on the register of measures.
It did not. It is an instrument for 134's E3 and needed only E3's existing padding, and 134 is
what waits on this. A BUILT block carries no gate, so the line is gone.

## What was built

Two numbers, read at build time under the existing `fastpath_pad` feature (kernel/build.rs,
kernel/src/fastpath_pad.rs). `NIFE_FASTPATH_PAD=<units>` sizes the sled in units of what the feature
has always linked; unset is 1, the sled the 2026-09-04 session booted, and 0 is the
dose-response's zero, the guard and a `ret`. `NIFE_FASTPATH_SHIFT=<bytes>` appends that many
unreferenced zero bytes after the sled, in the sled's own input section, so it moves exactly the
code the pad moves and adds nothing any path can reach. A layout variant is `PAD=0` with a
non-zero `SHIFT`: an un-padded kernel at different addresses. E3's evening is eight images, four
pad sizes and four shifts; notes/footprint-perturbation.md, "The next radon evening", is the
procedure, the sizes and why each was chosen.

The linker scripts pin the sled's section first in `.text`, right after the boot stub, and that
is what makes the two numbers comparable: each moves the whole kernel text by the bytes asked
for. A pad and a shift of equal displacement are the same binary apart from the sled's own
bytes. Checked: `PAD=1` and `SHIFT=5088` place all 1,047 riscv64 text symbols at identical
addresses. Two of the four shifts are therefore matched twins of pads, and they are the
sharpest comparison the experiment has. Any difference between a pad and its twin is the counted
footprint alone, which the physics says must be zero because the sled is never fetched.

**Pinning it was not tidiness.** Nothing had pinned the section, so where the linker dropped it was
redrawn every commit: on 2026-09-04 it landed ahead of the entire trap path, and by 2026-09-19 it
landed past all but 5% of the fastpath's bytes. That would have made the evening a
dose-response on a path the doses barely touched. `--layout` now prints that share
(`the sled precedes N of M hot symbols`), so the check is a line of output rather than a hope. The
default build has no such section, so the default link order is unchanged, checked by comparing
default boot images byte for byte on both ISAs across the change.

The proof each image is what it claims, and it needs no hardware.
`script/fastpath-footprint --layout` hashes every instruction of both IPC closures and the entry
set with address operands normalized away (direct call and branch targets, `auipc`/`adrp` uppers,
and the low-12 immediates that pair with one; it prints how many of each it touched). All eight
images share one hash, on riscv64, on aarch64 and on the `board,bench,single_hart` card set. Read
off the built binaries as well: only the 7 boot symbols ahead of the sled keep their address, all
1,038 after it move by exactly the bytes asked for and none changes size. The sled has exactly one
reference in the whole binary, the guard's skipped branch, and the shift block has none. The
same flag prints where the hot path landed: each symbol's address, its set in the U74's L1i, and
its line phase. The L1i is 32 KiB, 2-way, 64-byte lines, virtually indexed, so 256 sets from
address bits 6 to 13 (`SiFive` U74-MC Core Complex Manual 21G3.02.00, section 4.2.2). It prints
whether each symbol starts 8-byte aligned (section 4.2.6: the BTB predicts a taken branch or jump
with no bubble only for an 8-byte-aligned target). And it prints where `.text` ends and `.data`
starts, because the page-aligned sections after `.text` move in whole pages and the L1d way is
8 KiB (section 4.4.1).

A bench boot names its own image: `bench-probe: fastpath_pad units <u> shift <s>`. Nothing on a
card said which build it carried, and an interleave of eight is where that matters most.
`script/board-image` echoes both values on its `features:` line and refuses to build when they are
set without the feature.

### Three things the proposal did not anticipate

1. A feature per size would have reproduced the defect. It was built that way first, as seven
   sibling Cargo features. A feature's *name* enters cargo's `-C metadata` hash, which renames every
   symbol and repartitions codegen units. Measured on the card build, that moved code linked
   ahead of the sled by up to 11 KB and put `system_call::dispatch` anywhere across 240 of the 256
   L1i sets. Each image was an uncontrolled layout draw and the variable of interest was noise on
   top of it. Environment variables do not enter that hash. Editing the module's own source has the
   same effect for the same reason, which is why every image in an evening must come from one
   commit. Nor was the sled's *position* pinned, which had the same shape: an accident of each
   commit's link order decided how much of the hot path a pad perturbed, and by 2026-09-19 it was
   5% of it. Both are now properties of the build rather than draws.
2. **A pad that is never executed can only act through addresses.** It is never fetched, so it
   evicts nothing on its own; what it does is push other code apart. So E3 tests whether
   `script/fastpath-footprint`'s number predicts latency, which is what milestone 188 phase 4 leans
   on, and **not** Liedtke's claim about an *executed* footprint. That claim needs M6 or a
   perturbation that adds executed instructions. Stated here, in the procedure and in the note's
   `BUGS`, because it is the sentence most likely to be dropped when a result is quoted.
3. **The bench card's kernel is not the kernel the static table describes.** `bench` changes the IPC
   path's codegen (riscv64 `ipc_call_reply` is 5,212 bytes with it against 5,936 without, and the
   normalized instruction stream differs) and `single_hart` adds four instructions; `board` alone
   changes nothing. So the static step measures the card's own feature set, which the 2026-09-04
   session did not.

Found on the way, and fixed: `cargo xtask bench --riscv|--x86 --extra-features <f>` ignored the
flag and built a plain `bench` kernel, so a riscv64 E3 rehearsal through that path measured an
un-padded kernel and printed its numbers under the flag. riscv64 is the ISA radon runs.

## In brief

E3 compares two kernels that differ in one Cargo feature, `fastpath_pad`, and reads the difference
as the cost of the footprint that feature adds. **That inference does not hold**, and the
2026-09-04 session on radon proved it does not by producing an effect footprint cannot explain.

Six interleaved boots, three per condition, on a `single_hart` card where the placement lottery
cannot appear:

| row | unpadded | padded | delta |
|---|---|---|---|
| `ipc_rtt` | 4259 · 4259 · 4259 | 4311 · 4310 · 4310 | +1.21% |
| `call_reply` | 5015 · 5013 · 5013 | 5088 · 5089 · 5088 | +1.49% |
| `ipc_rtt_el0` | 128606 · 128626 · 128615 | 124958 · 124391 · 124903 | **-3.01%** |

All three are non-overlapping, with a within-condition spread of 0 to 2 units against gaps of 74
and 3,865. **The third row says the padded build is 3% faster, and the padding is never executed.**
It is reached through `core::hint::black_box` on a compare-and-branch that is never taken, so there
is no mechanism by which 5 KB of dead text makes a round trip 193 ns quicker.

What changed is the layout: different cache-line boundaries, different set indices in a 32 KB
2-way L1i, different branch-predictor aliasing. Mytkowicz, Diwan, Hauswirth and Sweeney measured
exactly this in *Producing Wrong Data Without Doing Anything Obviously Wrong* (ASPLOS 2009).
Changing link order or the size of a UNIX environment variable, neither of which alters an
instruction, moved measured performance by more than the optimization being studied.

**So the +1.49% is a real difference between two binaries and is not evidence about two
footprints.** It is the sum of a footprint effect and a layout effect, and this experiment reports
the sum.

## What to build

A layout control: several unpadded kernels that differ only in address assignment. Each adds no
reachable instruction, so any difference between them is layout by construction. Candidates, in
increasing order of how much of the binary they disturb:

- A dead `#[used]` static of varying size in the same section.
- A dead symbol of varying size ahead of the fastpath, which is what `fastpath_pad` already is
  minus the claim that its size is the variable of interest.
- A link-order change in the linker script.

Read the same three rows across, say, five of them. That yields a layout distribution instead of
a single unpadded point, and E3's padded reading becomes interpretable for the first time: inside
the distribution is layout, outside it is footprint.

The cheapest honest version is smaller than that. `fastpath_pad` is already a size knob. Running
it at four sizes rather than two (0, 1x, 2x, 3x) gives a dose-response curve, and **footprint
predicts monotonicity where layout does not.** A row that rises with padding is footprint; one that
jumps and comes back is layout. That is one Cargo feature taking a value instead of a boolean, and
it may be a day rather than a project.

## Why it is worth doing rather than shrugging at

It decides milestone 188 phase 4. That block is holding a hand-written IPC fastpath, a standing
verification obligation and a permanent maintenance cost, on evidence that the footprint costs
something. The evidence currently available is a 19 ns effect with a 193 ns artifact sitting on top
of it, and nobody should buy a second IPC path with that.

It also protects the numbers already published. `bench/` holds stored baselines that a future
reader will diff across builds, and every one of those diffs has this exposure. The E4 displacement
percentages in the same capture (0 to 1% at ordinary load, 5 to 8% at 96 threads, peaking at 32 KiB
which is radon's L1d exactly) are the more interesting result of that session. They are a
single-build measurement, so they are unaffected. The moment anyone compares them against another
build, they are affected.

And it is the difference between a demonstrator and a marketing claim, which is AGENTS.md's own
standard for benchmarks: state what a number means and where it is not apples-to-apples. An
overlapping range reported as a trend is the outcome this project's own bench note already refuses.
A confounded comparison reported as an attribution is the same failure with better arithmetic.

## What it is not

It is not a reason to distrust E1 or E4. Both are single-build sweeps, where the layout is
constant and the independent variable is thread count or working-set size. Nothing in this proposal
touches them, and E1's knee at 16 threads and E4's peak at 32 KiB are the session's real findings.

It does not need the board free for long. Each additional build is one card write and about 75
seconds of boot, measured. The cost is the interleaving discipline, which with one card means one
write per boot.

## Where it came from

The 2026-09-04 radon session. notes/footprint-perturbation.md's own BUGS carries the defect;
`design/roadmap/0188-ipc-fastpath.md` is what it blocks.

## Scope note

An instrument, not a measure and not a decision. No system call surface, no wire format, no dependency;
one Cargo feature that already existed now takes two build-time numbers. It produces no result by
itself: the result is milestone 134's radon evening.

## Follow-on

- **Milestone 134.** The evening this control exists for, and the one thing between 134 and BUILT.
  What it must produce is in that block's Follow-on and in notes/footprint-perturbation.md.
- **Recorded.** Four layout images are a small sample of a distribution; Stabilizer (Curtsinger and
  Berger, ASPLOS 2013) randomizes layout repeatedly for exactly this reason. Four draws bound an
  effect loosely and cannot prove one absent. In notes/footprint-perturbation.md's `BUGS`.
- **Recorded.** A never-executed pad reaches a clock only by moving code, so no E3 reading can
  settle Liedtke's executed-footprint claim. In the same `BUGS` section and in item 2 above.
- **Recorded.** The bench card's kernel differs from the bare release kernel in the IPC path, so
  static figures taken without `bench` describe a kernel nobody boots. In the same `BUGS` section;
  milestone 134's optional per-IPC-depth boot rests on the same assumption.
- **Done.** `xtask`'s riscv64 and x86_64 bench arms now pass `--extra-features` through.
- **Recorded.** Every name this lane minted is **provisional**, as always, and listed where a reader
  meets it (kernel/Cargo.toml, kernel/src/fastpath_pad.rs, `script/fastpath-footprint`'s usage):
  `NIFE_FASTPATH_PAD`, `NIFE_FASTPATH_SHIFT`, `--layout`, `fastpath_layout_shift`, the
  `bench-probe: fastpath_pad` line, `PAD_UNITS`, `SHIFT_BYTES` and `BUILD`.

## Index row

E3 compares two kernels that differ in one Cargo feature and reads the difference as the cost of the
footprint that feature adds. The 2026-09-04 radon session proved that inference does not hold: six
interleaved boots on a `single_hart` card separated cleanly in three rows, and the third said the
padded build was 3.01% faster on a padding that is never executed. What changed is the layout,
which is *Producing Wrong Data Without Doing Anything Obviously Wrong* (ASPLOS 2009) in one capture.
So the +1.49% on the row that matters is the sum of a footprint effect and a layout effect, and the
experiment reports the sum. The control is several unpadded kernels differing only in address
assignment, which by construction add no reachable instruction, read across the same three rows to
give a layout distribution: inside it is layout, outside it is footprint. The cheapest version is
`fastpath_pad` taking a value rather than a boolean, because footprint predicts monotonicity where
layout does not. It decides milestone 188's phase 4, which is holding a hand-written IPC fastpath on
evidence of a 19 ns effect with a 193 ns artifact sitting on top of it. BUILT 2026-09-19:
`NIFE_FASTPATH_PAD` and `NIFE_FASTPATH_SHIFT` size the sled and add an unreachable shift, the
linker scripts pin that section first in `.text` so both move the whole kernel text and a pad has a
byte-identical un-padded twin. Eight images share one normalized instruction hash that
`script/fastpath-footprint --layout` prints, and a bench boot names its own image. Building it found that a Cargo feature per size reproduces the
defect (a feature name repartitions codegen units, moving 11 KB of unrelated code), and that a pad
never executed can only act through addresses. E3 tests whether the footprint number predicts
latency, not Liedtke's claim about an executed footprint.
