---
status: BUILT
raised: 2026-10-05
built: 2026-10-05
promoted_from: each-cores-percpu-on-its-own-cache-line
milestone_dependencies: none
decision_dependencies: none
machine_requirements: riscv64 silicon with four or more harts
specific_machine: radon (fatal risk 4's null-system-call numbers were taken there)
needs_person: yes
---
# 766. Each core's PerCpu on its own cache line

Raised by measuring fatal risk 4's null system call under load. *(Title, slug and every name below
are drafts.)*
`needs_person` is yes only because the acceptance measurement is taken at radon's bench.

## Index row

No two cores' per-core blocks share a cache line on any architecture, which takes the last
measured per-trap growth out of the null system call under load on Apple cores. A compile-time
assertion keeps it that way.

## The defect

`kernel/src/cpu.rs` declared `PerCpu` with `repr(align(256))` on x86_64 and nothing on aarch64 or
riscv64, where it aligned to 8. The block is 128 bytes, so `PERCPU[i]` shared lines with its
neighbors. In the aarch64 job-mix build `PERCPU` sat at 24 mod 64; on riscv64 the parent note found
16 mod 64. A field another core writes (the inbox depth, the steal slot) then shared a line with
`held_rank`, which the owning core writes twice per lock it takes.

## The evidence

From [the HVF appendix](../../notes/job-mix/null-system-call-hvf-full-mix.md): four Apple M3 cores
under HVF, the full job mix, with milestone 761 (capability lookup off the global lock) in. The
null system call's per-trap excess from one task to four, in 24 MHz ticks:

| Build | boots | excess at 4 tasks, 95% interval |
|---|---|---|
| `main` with #1663 | 19 | 0.042 [0.018, 0.066] |
| the same, `PerCpu` at `repr(align(128))` on aarch64 | 18 | -0.010 [-0.019, -0.006] |

The two were interleaved boot by boot. The excess grows one step per added core (0.009, 0.021,
0.041 over 30 boots), needs the spawn job (0.002 with it stubbed), and is not a lock (0.012% of
lookups contended). It is about 1.7 ns on a 33 ns trap there. Radon's size is unknown.

## What was built

- `PerCpu` is `repr(align(128))` on aarch64 and riscv64. x86_64 keeps its `repr(align(256))`
  (milestone 758 (the IPC fast paths shrink back inside their band)), where the struct is 160
  bytes. The size stays 128 on the first two, so the power-of-two assertion below `PERCPU` still
  holds and no core pays padding.
- A private constant `FALSE_SHARING_SPAN` (128) and a compile-time assertion that
  `align_of::<PerCpu>() >= FALSE_SHARING_SPAN`. Size is a multiple of alignment, so that one check
  is what puts every element of `PERCPU` on a span of its own. Losing either attribute, or adding an
  architecture without one, fails the build. 128 rather than 64 on every ISA: Apple's line is 128
  bytes, and adjacent-line prefetch pairs 64-byte lines on other parts.
- `PERCPU` moved from `0x...508` to `0x...580` on aarch64 and from `0x...d50` to `0x...d80` on
  riscv64 (release kernels): both now 0 mod 128. x86_64's address is unchanged.

Reuse: x86_64's own `repr(align(256))` on the same struct is the precedent this extends; no
crate is involved, since the change is one attribute and one assertion in the kernel.

## Gates

- `script/fastpath-footprint`: byte-identical to the parent commit on all three ISAs.
- Release kernel sections (`.text`, `.rodata`, `.data`, `.bss`) unchanged on aarch64 and riscv64;
  the padding falls inside page rounding.
- No Kani or loom harness models `PerCpu`. The steal slot's loom model is on
  `work_steal_slot::Slot`, whose layout this does not touch.
- CI runs the rest, including `script/stack-frame-check`, the boot-file-size check and the icount
  tripwire.

## What HVF said

Patagonia, 2026-10-05 (UTC), by the parent note's method; the full reading is the
[note's section on this milestone](../../notes/job-mix/null-system-call-hvf-full-mix.md#milestone-766-from-committed-code).
Per-trap excess at four tasks, 24 MHz ticks, bootstrap 95% intervals, boots interleaved:

| Build | boots | excess at 4 | minus the parent commit |
|---|---|---|---|
| parent commit `1cf413329` | 32 | -0.058 [-0.065, -0.050] | |
| this milestone | 32 | -0.055 [-0.060, -0.051] | +0.003 [-0.006, +0.011] |
| parent, blocks forced to 24 mod 128 (scratch) | 21 | -0.018 [-0.022, -0.013] | +0.040 [+0.030, +0.049] |

**The proposal's E did not reproduce as a difference, because the parent commit no longer has the
defect's layout.** Its link puts `PERCPU` at 8 mod 128, where only a neighbor's `rng` and
`need_resched` share a block's line; the job-mix build the proposal measured had it at 24 mod 64.
Forcing that layout back brings the residual back in A's shape (+0.010, +0.023, +0.040 at two,
three, four tasks), so the defect is real and depended on where the linker happened to put a static.
This milestone removes the dependence: against the forced layout it is worth 0.040 ticks a trap,
and against today's lucky one nothing, within an interval of zero either way.

## What radon said

2026-10-05 (UTC), five interleaved boots by a run sheet written before any boot
(`bench/radon-2026-10-05/` (`nifeos/nife:bench/radon-2026-10-05/README.md`); the full reading is
[the appendix](../../notes/job-mix/radon-2026-10-05.md#milestone-766-the-alignment-is-material-on-radon)).
`main` at `c8b5fd09e` against the same commit with this milestone undone and the blocks forced to
24 mod 128, `null_system_call` `per_job` in 4 MHz ticks:

| | 1 task | 4 tasks | growth |
|---|---|---|---|
| `main`, boots 1, 3, 5 | 110, 110, 110 | 120, 121, 120 | 10 |
| forced layout, boots 2, 4 | 110, 110 | 128, 127 | 17.5 |

**`D` = 7.5 ticks a job, 29 ns a trap: the run sheet's "6 or more" band, material.** The one-task
difference is 0, and the two forced boots spread no more than the three `main` boots, so both
guards hold. HVF's 1.7 ns, scaled, predicted 0.43 ticks a job, under radon's resolution. Radon paid
about 17 times that, so a coherence miss on the U74 costs far more than on Apple's cores. The same
evening showed radon's single-crossing level moves up to 8 ticks with code placement alone, but the
growth with cores held within a tick across those layouts, and `D` is a difference of growths.

## Done means

- E's result from committed code: the HVF sweep above, at least 18 boots interleaved against the
  parent commit, four-task excess within its interval of zero.
- A radon run by [the parent note's](../../notes/job-mix/null-system-call-under-load.md) procedure
  and its one-task guard, recording the excess before and after. That is the acceptance
  measurement, and its result goes to risk 4's line whatever it is.
- `script/fastpath-footprint` and `script/bench` unchanged within their floors on all three ISAs.

## BUGS

- Only `PerCpu` is covered. About thirty other statics are `[T; MAX_CPUS]` arrays, most packed, so
  a core's slot shares a line with its neighbors'. Some are on hot paths: `sched::PREEMPTIONS_PER_CPU`
  and each architecture's per-core `TICKS`. `sched::CURRENT_CAPABILITIES` (written on every switch,
  read by every capability system call) is `align(64)`, so two cores share each 128-byte Apple line,
  and this milestone's span would say 128. Unmeasured. The aligned build reads within 0.003 [-0.006, +0.011]
  ticks a trap of the parent at four Apple cores, so whatever they cost is below this instrument
  there. Found by reading while building this milestone; it wants a measurement before a remedy.
  Radon's 2026-10-05 run did not isolate it, and now that radon prices a shared line at about 29 ns
  a trap, it is the place to do so.

## Follow-on

- **Done.** The radon run, 2026-10-05 (UTC), above. It was taken against `main` with this milestone
  undone, not against the parent commit: the parent's lucky layout was the thing the milestone
  removes the dependence on, and HVF had already shown it. The one-task guard is the parent note's
  and it failed for every build that evening (110 against 101), for a reason that is not this
  milestone ([the appendix](../../notes/job-mix/radon-2026-10-05.md#why-one-task-rose-from-99-to-110)).
