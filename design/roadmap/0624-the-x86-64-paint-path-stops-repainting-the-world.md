---
status: BUILT
raised: 2026-09-30
built: 2026-10-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 624. The x86_64 paint path stops repainting the world

Number provisional (the paint-path lane, 2026-09-30): minted by the integrator at merge like every
global name. Raised from the icount attribution's finding
(`notes/benchmarks/icount-tick-scales.md`, PR #1470). The swish-check x86_64 leg spends 321.1 s
against 6.9 s (aarch64) and 7.3 s (riscv64), which is 15 s per transcript line. The kernel is not
the cost: its paths agree within 7 percent once ticks are scaled. The paint path is. Every scroll
repainted the whole 924x344 surface. Every flush crossed an uncacheable aperture one word at a
time. The console blocked on a full paint per write.

Three fixes, one per cost.

1. The engine reports a scroll as movement, and the painter moves pixels it already holds
   (`video_terminal::Damage`, `display_terminal`'s scroll fast path).
2. `Aperture::copy_wide` stages pixel pairs into qword stores: 158,928 against 317,856 stores per
   full-surface flush. A `u64` is the ceiling on a `-mmx,-sse` target.
3. The console batches screen writes into one paint per 20 ms window, over a bound notification
   (milestone 151 (notification objects: async multiplexing without wait-any)) and a timer
   (milestone 106 (a wait that ends on either the interrupt or the deadline)).

The third fix changed what the console's ack means: the wire has the bytes at once, the screen
within one window. That change is recorded in `components/src/console.rs`'s module doc rather than
slipped in.

Measured on patagonia, 2026-10-03 UTC, same day and same machine for both trees:

| leg | `main` (4db8c13bf) | this milestone (4124d6390) |
|---|---|---|
| x86_64 | 119 lines, 753.5 s, 6.3 s/line, slowest 27.5 s | 128 lines, 665.6 s, 5.2 s/line, slowest 22.9 s |
| aarch64 | 122 lines, 20.9 s, 0.17 s/line | 131 lines, 23.6 s, 0.18 s/line |
| riscv64 | 122 lines, 19.7 s, 0.16 s/line | 131 lines, 25.2 s, 0.19 s/line |

So the x86_64 leg is about 18 percent cheaper per line, not the order of magnitude the attribution
hoped for, and the other two legs pay up to 0.03 s per line. Another session's x86_64 leg shared
the machine during both x86 runs. Both `main` runs failed on a content line (`wc < args.txt`) of a
fresh checkout after their timed lines, so their totals stand but their line counts are short.

The console batcher ships **off** (`SCREEN_BATCHING_ENABLED`, the console's module doc), so fix 3 is
built and not yet earning. Turning it off exposed a fault: with every write painting, small
mid-row flushes became common, and `copy_wide` counted `x` twice, reading past the surface. That
is fixed and its host test now covers every rectangle. `SWISH_CHECK_X86_LINE_SECONDS` stays at 90 s;
its doc carries these numbers and the reason. The remaining x86_64 gap is the follow-on below.

## Index row

BUILT on `milestone/624-paint-path` (PR #1473). Three fixes take the x86_64 swish leg's paint cost
apart at its three sources (whole-surface repaint per scroll, word-wide aperture stores, one
blocking paint per write). Measured: x86_64 6.3 to 5.2 s per line (18 percent), aarch64 and riscv64
up to 0.03 s per line; the batcher ships off, and the remaining x86_64 gap belongs to the follow-on lane below.

## Follow-on

- **Recorded.** The rest of the x86_64 gap, at `SWISH_CHECK_X86_LINE_SECONDS` in
  `xtask/src/swish_check.rs`: 5.2 s per line against 0.18 s on aarch64, so the paint path was not
  most of it. A lane the integrator minted on 2026-10-03 takes it from this branch's head
  (acceleration, the QEMU machine configuration, the batcher this block shipped off); at merge
  this bullet becomes that milestone's.
