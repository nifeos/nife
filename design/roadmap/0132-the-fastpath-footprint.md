---
status: BUILT
raised: 2026-08-18
built: 2026-08-18
---
# 132. The fast path's footprint, and a gate so Mach's mistake cannot happen quietly

Built 2026-08-18. Raised the same day, from calef: *"We need a fast path footprint to
compete with our benchmarks. Then we need a means to monitor it over time so that we don't introduce
Mach-scale performance issues on our benchmark hardware."* Both halves are the deliverable, and the
second half is the one that lasts.

The number the gate exists to hold still is over target, and this block says so in the same
breath as claiming BUILT, because a gate that keeps a bad number bad is still the right gate. It
converts a gap you can measure into a gap that cannot widen while nobody is looking.

## Why a footprint is a benchmark at all

Liedtke, *On µ-Kernel Construction* (SOSP 1995), is the source and the finding is not the one the
paper is usually cited for. Mach's IPC was slow **not because its code was slow** but because the
hot path's cache footprint evicted the application's working set. So the user paid the miss after
the system call returned, where no microbenchmark of the system call could see it. seL4's answer is a
hand-written fastpath small enough to leave L1 mostly intact.

That makes footprint a first-class number here rather than a curiosity, and it makes it exactly the
kind of number this project gates: an instrument that reports what a latency benchmark structurally
cannot. `script/bench`'s icount tripwire measures instructions retired on the path; nothing
measured how much instruction cache the path occupies, so a change that doubled the footprint while
leaving icount flat would have landed green.

## What was built

`script/fastpath-footprint`, wired as its own CI job, with a committed baseline per ISA under
`bench/` and a 5% tolerance. It walks the call graph out of the release kernel's disassembly, the
technique `script/stack-depth-check` already uses for stack chains, and reports two quantities
because they are two quantities:

| | aarch64 | riscv64 |
|---|---|---|
| `ipc_fastpath`, transitive closure from the IPC and switch roots | 5,780 | 5,074 |
| `system_call_entry`, vector + dispatcher + `system_call::dispatch`, summed **flat** | 4,168 | 2,692 |
| **total, an upper bound** | **9,948 (9.71 KiB)** | **7,766 (7.58 KiB)** |

`system_call_entry` is summed flat rather than closed over on purpose: a system call traverses one path
through the decoder, so closing over `dispatch` would report the whole system call surface as the cost of
this path. Its own bytes are on every system call and its other arms are on none of it.

Five percent, against the icount tripwire's ten. A symbol's size moves when the code moves; an
icount moves whenever the compiler remakes an inlining decision. The tighter band is affordable
because the signal is quieter, not because the number matters more.

## The finding, which is why the number is honest

Closing naively from `finish_switch` returned 11.2 KiB. The reap branch drags in
`KernelStack::drop`, `untyped::destroy`, `revoke_region`, `delete_frame_caps` and the unmap path,
every one of which runs when a thread exits and none of which runs during an IPC. Classifying the
teardown family as cold took it to 5.6.

A gate shipped at 11.2 would have been measuring thread death, calling it IPC, and sitting quiet
through a doubling of the real path. The cold list is therefore the load-bearing judgment in the
script, and it carries a reason per family rather than a bare regex, because a wrong entry there is
silent by construction.

## Where we stand against the target, and what closing it would be

The target in `notes/benchmarks/kernel-footprint-and-caches.md` is under 4 KiB for the fastpath,
an eighth of the smallest L1i among machines this project actually runs on (32 KB, the SiFive U74).
It is expressed as a fraction so it tracks the board list rather than a number somebody liked.
`ipc_fastpath` alone is 5.6 KiB.

The largest single item is `system_call::dispatch` at 2,024 bytes, and it is precisely what seL4's
hand-written fastpath exists to skip: a decode-and-dispatch table on a path that already knows which
operation it is performing.

That work is not this milestone's, and it should not be folded in. A hand-written IPC fastpath is
a kernel design decision with a verification cost (the proofs currently cover one path through
`ipc_send`, not two). It is the sort of change whose entire justification is a measurement, so it
wants the instrument to exist first. It does now. Recording the trigger rather than scheduling the
work, in this file's habit. The day the gate's number moves the wrong way under a change nobody
wanted, or the day milestone 127's TX1 puts a real PMU behind the estimate, the fastpath stops being
a note.

The same applies to the second target line, data touched per IPC under 1 KiB. Nothing measures it
and this gate does not: it counts instruction bytes only. That is a real hole in the pair of numbers
Liedtke's argument rests on, and the honest reason it is not built is that a data footprint needs
either a PMU or an instrumented run rather than a disassembly walk.

## BUGS

- **It is an upper bound, not a footprint.** Whole symbol sizes, so a cold tail parked inside a hot
  function counts against us. Deliberate direction for a tripwire: it errs toward failing.
- Indirect calls are invisible, the same blind spot `script/stack-depth-check` records for the
  same reason, and neither script can fix it from a disassembly.
- riscv64's tail instruction is assumed 4 bytes on an ISA that mixes 2 and 4. Conservative, and
  it inflates riscv64's number by at most two bytes per symbol.
- The cold list is a judgment and a wrong entry would be silent. A family wrongly marked cold
  disappears from the number with no error. This is the one failure mode the script cannot detect
  about itself.
- **It is not a cache measurement.** Nothing here models a cache, an associativity or a line. It is a
  proxy for one, and milestone 74's cycle counters on milestone 127's silicon are what turn a proxy
  into a measurement.
- The hardware figures the target is derived from are from general knowledge, not from the boards.
  notes/benchmarks.md says so where it states them. The U74's 32 KB should be confirmed against the
  VisionFive 2 and the TX1's 48 KB against the silicon when it arrives.

## Scope note

Tooling and a note. It moves no system call, adds no dependency, changes no wire format and takes no
`DECISIONS` section.

**The name is provisional.** By this directory's convention a build-failing gate takes `-check`, and
`fastpath-footprint-check` is the consistent spelling; it reads badly, which is why it is worth
settling rather than applying. Nothing depends on it and it is an architect's call.

**One claim in the prose it replaced did not survive being checked**, and it is recorded in
notes/benchmarks.md rather than here because that is where a reader meets it. L1i has **not** grown
four to six times and is not still growing. It has been static at 32 to 64 KB for a decade, Zen 5
included. L2 is what ballooned, which is a different and weaker safety net: overflowing L1 now costs
tens of cycles instead of a trip to DRAM, rather than not costing anything.

## Follow-on

- **Milestone 188.** The hand-written IPC fastpath this block deliberately refused to fold in, and
  with it the data-footprint half of the target. `system_call::dispatch` at 2,024 bytes is exactly what
  seL4's fastpath exists to skip, and this block's whole argument was that the instrument should
  exist before the change it justifies. 188 also holds the open question of whether "under 1 KiB
  touched per IPC" can be estimated from the structures the path touches without waiting for a PMU.
- **Recorded.** `design/roadmap/0132-the-fastpath-footprint.md`. The gate reports an upper bound
  rather than a footprint: whole symbol sizes, so a cold tail parked in a hot function counts
  against us. Indirect calls are invisible the way `script/stack-depth-check` records for itself,
  and riscv64's tail instruction is assumed 4 bytes on an ISA that mixes 2 and 4.
- **Recorded.** `design/roadmap/0132-the-fastpath-footprint.md`. The cold list is the load-bearing
  judgment in the script and a wrong entry there is silent by construction: a family wrongly marked
  cold disappears from the number with no error. It is the one failure mode the script cannot detect
  about itself, which is why each family carries a reason instead of a bare regex.
- **Recorded.** `notes/benchmarks.md`. The under-4-KiB target is derived from cache sizes taken from
  general knowledge rather than from the boards. The U74's 32 KB wants confirming against the
  VisionFive 2 and the TX1's 48 KB against the silicon when it arrives. Nothing here models a
  cache, an associativity or a line: this is a proxy, and milestone 74's counters on milestone 127's
  hardware are what would turn it into a measurement.
- **Recorded.** `script/fastpath-footprint`. The script's name is provisional and unratified, and
  the script's own header says so where a reader meets it. This directory's suffix for a
  build-failing gate is `-check`, so the consistent spelling is `fastpath-footprint-check`; it was
  not taken because the name reads badly at nine syllables. Nothing depends on it and it is an
  architect's call.

## Index row

Liedtke's actual finding is that Mach's IPC was slow because the hot path evicted the
application's working set. So the user paid the miss after the system call returned where no latency
benchmark could see it. Nothing here measured that: an icount tripwire counts instructions
retired, not instruction cache occupied, so a change doubling the footprint would land green. `script/fastpath-footprint` walks the call graph out of the release disassembly, `stack-depth-check`'s technique, and gates two numbers per ISA at 5%. Closing naively from `finish_switch` returned 11.2 KiB because the reap branch drags in teardown that never runs during
an IPC; excluding it gave 5.6. We are over the 4 KiB target and the gate holds it still rather
than closing it. `system_call::dispatch` at 2,024 bytes is the largest item and is exactly what
seL4's hand-written fastpath skips, which is separate work that wanted this instrument to exist
first
