# Benchmarks with teeth

*(Milestone 21 (performance measurement). `script/bench`, `kernel/src/bench.rs`, `bench/baseline-<arch>.txt`. This page carries
the current numbers. The dated history is in the
[appendices](#appendices), whose names calef ratified on 2026-09-24 ([the naming record](benchmarks/README.md)).)*

## Why two instruments

One tool cannot both gate commits and tell the truth about magnitudes.

| | icount (default) | HVF (`--real`) |
|---|---|---|
| what runs | TCG translation, `-icount shift=0,sleep=off` | the kernel, natively on the M-series core |
| virtual time | a deterministic function of instructions executed | the hardware counter, 24 MHz |
| numbers are | exact and reproducible per binary | real: the host's caches, TLBs and predictors |
| numbers mean | path length; magnitudes are fiction | nanoseconds; determinism is gone |
| job | regression gating: `--check` fails on drift past 10% | knowing what a path costs |
| architectures | aarch64, riscv64, x86_64 | aarch64 only (Hypervisor.framework runs the host ISA) |

The committed baseline is the performance record.

## Running it

```sh
script/bench                    # aarch64 icount counts
script/bench --check            # diff against bench/baseline-aarch64.txt; fails past the tripwire
script/bench --riscv --check    # the same for riscv64
script/bench --x86 --check      # the same for x86_64
script/bench --save --why "<reason>"   # re-record, in the commit that moved the numbers
script/bench --real             # HVF, one hart: per-core magnitudes, never gates
script/bench --real --release   # optimized kernel and userspace; the build every comparison uses
script/bench --real --smp       # HVF, four harts: smp_* and, with a disk, the fs_* benches
```

CI runs all three `--check` legs on every pull request (`script/ci-build`'s `bench` row).
[`notes/bench-runbook.md`](bench-runbook.md) says which board to spend an evening on and in what
order.

### The rules a baseline save follows

- The tripwire is coarse on purpose, `max(base / 10, 64)` ticks, because icount drifts
  a few percent across builds as the compiler remakes inlining
  ([icount drift](benchmarks/icount-drift-and-provenance.md)).
- Every leg boots one hart: under `-icount` all vCPUs share one clock, and an idle hart's `wfi` jumps
  it forward. Every leg has measured one hart since; x86_64 briefly lost the pin on
  2026-09-23.
- A save runs with shipping features on and measurement features off. `icount`
  is the one bench feature allowed, because it changes how time is observed and not what is measured.
- Each baseline records its nightly (`# toolchain:`, checked by `script/lint`), the QEMU that ran
  (`# qemu:`, checked by `--check`) and a `# why:` line (milestone 302 (a baseline records what it was saved against)). Auto-re-saving on a toolchain bump is refused (calef, 2026-09-21): such a floor could
  never report a nightly that made the kernel slower. Restamping is not (calef, 2026-09-26, milestone 598 (a nightly bump restamps the floors it proves it did not move)).
  The bump workflow rewrites only `# toolchain:` when an A/B of the two nightlies on one runner
  moves no row by 0.5%, and none by 2% since the last save. The stamp now means "last proven valid
  for".
- An unexplained movement is investigated, never re-saved away. The 2026-08-15 riscv64 `map_new`
  +15.6% nearly was (the
  [`map_new` episode](benchmarks/riscv-map-new-and-the-rfence-probe.md)).
- A timed window that a preemption can land in is not a measurement. `map_new` masks interrupts
  across its window since milestone 541 (a timed window that excludes preemption).

## What is measured

Kernel-side benches call scheduler and IPC functions directly. EL0
benches are userspace programs making real traps, as lmbench and seL4 do, so they are the cross-OS
numbers.

| bench | plane | one iteration |
|---|---|---|
| `yield_switch` | kernel | one voluntary yield in a two-thread ping-pong: two switches |
| `ipc_rtt` | kernel | send and receive round trip: two rendezvous, one address space, no trap |
| `call_reply` | kernel | the service shape: mint a one-shot Reply cap, rendezvous, reply |
| `relay_rtt` | kernel | client, relay, backend and back: one confined intermediary |
| `broker_rtt` | kernel | `call_reply` with the queue broker interposed |
| `spawn_reap` | kernel | spawn a thread, let it exit, reap it |
| `map_new` | kernel | retype a fresh page from a region, walk, write the leaf (interrupts masked) |
| `coremark` | EL0 | pure compute; the control that must never move |
| `null_syscall` | EL0 | one `svc` the kernel rejects at once |
| `ctx_switch` | EL0 | `SYS_YIELD` to a peer process and back |
| `ipc_rtt_el0` | EL0 | `SEND` and `RECEIVE` between two processes: four traps |
| `map_el0` | EL0 | `MAP_INTO` of an aliased frame: the mapping mechanism with no zeroing |
| `spawn_el0` | EL0 | build a child from nothing, run it to exit, reap, reclaim its region |

Also: `sink_throughput` (a byte stream), `rfence_self` (riscv64's SBI fence), `tss_iomap_*` (x86_64's
I/O bitmap), and on `--smp` only, `smp_*` and `fs_*`.

## The current numbers

### Against Linux and macOS, on one core (HVF, release)

Linux runs as a static musl `/init` under QEMU-HVF on the same M-series core, nife's tier.
Native macOS is the bare-metal ceiling. The host side is `bench/host/`.

| metric | nife | date | Linux (HVF) | macOS (native) |
|---|---|---|---|---|
| null syscall | 27 ns | 2026-08-04 | ~139 ns | ~76 ns |
| context switch, per switch, derived | ~29 ns | 2026-07-29 | ~415 ns | ~818 ns |
| IPC round trip, EL0 | 350 ns | 2026-08-04 | ~1,723 ns | ~2,620 ns |
| provision and map a fresh page | ~500 ns (`map_new` + one trap) | 2026-07-29 | ~534 ns | ~556 ns |
| map mechanism only | ~92 ns (`map_el0`) | 2026-07-29 | n/a | n/a |
| spawn, build to reap | ~4.4 us (`spawn_el0`) | 2026-07-29 | ~19.7 us (`fork`+`exit`) | ~291 us |

The Linux and macOS columns were taken on 2026-07-25 (spawn 2026-07-26); the nife column is the
latest release reading of each row, and the 2026-08-04 figures are a median of five boots.

nife wins four rows and ties one. The null syscall and the IPC round trip are about 5x faster than
Linux at the same tier. Page provisioning is a three-way tie near 500 ns, because zeroing 4 KiB is
bandwidth-bound on all three. The 92 ns mechanism is not a page a program can use, so it
is no win. [Cross-OS primitives](benchmarks/cross-os-primitives.md) has the
methods and the debug-build history.

### Against seL4's published cycles

seL4 publishes, for the same-core different-address-space path, 413 cycles for the IPC call and 426
for the IPC reply, one-way each. The machine is a Jetson TX1 (Cortex-A57, 1.9 GHz), and a round trip
in our sense is ~839 cycles. Ours is 350 ns. HVF passes through no PMU, so a cycle count here is
nanoseconds times an assumed clock. The M3 runs 2.75 GHz on an E-core and 4.05 GHz on a P-core, which
puts us at ~960 to ~1,420 cycles. So the corrected figure is roughly 1.1x to 1.7x an L4-lineage round
trip, not 4 to 7 times.

Read that as "same order", never tighter; the caveats are under
[what is not apples to apples](#what-is-not-apples-to-apples). sel4bench needs a real PMU, which this
host lacks and [xenon](benchmarks/sel4bench-on-xenon.md) and argon (milestone 127 (the seL4 machine))
have. [Calibration against seL4](benchmarks/calibration-against-sel4.md) has the three errors a first
version of this comparison made, and the build recipe.

### The regression floors (icount, `nightly-2026-09-23`, QEMU 11.1.1)

Ticks per iteration from `bench/baseline-<arch>.txt`, the source of truth, read as of 2026-09-24.
Ticks are not comparable across architectures: aarch64 counts at 62.5 MHz (16 instructions a tick), riscv64 at 10 MHz (about 100), x86_64 at 1 GHz
(one). [Icount tick scales](benchmarks/icount-tick-scales.md) ground-truth measures them and
corrects the cross-arch table.

| bench | aarch64 | riscv64 | x86_64 |
|---|---:|---:|---:|
| `ipc_rtt` | 1,032 | 171 | 17,202 |
| `call_reply` | 1,059 | 177 | 17,734 |
| `relay_rtt` | 2,058 | 343 | 34,331 |
| `broker_rtt` | 2,115 | 354 | 35,429 |
| `yield_switch` | 563 | 94 | 9,594 |
| `spawn_reap` | 3,383 | 538 | 44,469 |
| `map_new` | 240 | 37 | 2,758 |
| `null_syscall` | 20.5 | 3.7 | (EL0 plane not in this leg) |
| `ctx_switch` | 612 | 103 | |
| `ipc_rtt_el0` | 2,203 | 372 | |
| `spawn_el0` | 13,721 | 2,167 | |

### What a userspace server costs

The confined-server tax is one intermediary: `relay_rtt` minus `ipc_rtt`, about 1,030 ticks on
aarch64, or two extra switches and two extra rendezvous. The queue broker costs the same shape again,
`broker_rtt` at 2.0x `call_reply` on both ISAs, which is why it is opt-in per channel. A live service
swap adds nothing to the steady state. [The service path](benchmarks/service-path.md) has the
derivation.

### Scaling across four harts (HVF, `--smp`)

Independent compute scales 3.5x on four cores, 89% of the ceiling. Sixteen synchronous IPC pairs go
backwards (0.18x). A pair split across cores wakes a vCPU the host has descheduled, the
hypervisor's cost. Real silicon is where pipelines should scale.
[Per-core and multi-hart](benchmarks/per-core-and-multi-hart.md) has both workloads and why `--real`
boots one hart.

### Filesystem throughput (HVF, `--smp`, 2026-08-19)

| | nife after milestone 138 (close the read gap) | milestone 38 (filesystem throughput), where it started |
|---|---|---|
| sequential read, 64 KiB request | 329,168 ns, 189.9 MiB/s | 2.68 MiB/s at 4 KiB |
| sequential write, 64 KiB request | 936,583 ns, about 67 MiB/s | 1.52 MiB/s at 4 KiB |

That is 70.9x on sequential read. The block path is at parity with Linux: 39.0 us marginal per block
through a confined userspace block server, against 38.7 to 53.3 us for a raw virtio read at the same
tier. Buffered Linux reads at about 7,141 MiB/s, because it has a page cache and nife does not.
Closing that is out of milestone 138's scope. Five [appendices](#appendices) hold the history,
starting at [milestone 38's](benchmarks/filesystem-throughput.md).

### A directory walk (HVF, `--release`)

[Walk costs](walk-cost.md), milestone 606 (a directory walk costs what it does on Linux).

### The IPC fastpath's footprint

`script/fastpath-footprint` walks the call graph out of the disassembly and sums the bytes an IPC
round trip can fetch. It fails on 5% growth against `bench/fastpath-<arch>.txt`, whose figures milestone
188 (the IPC fastpath) recorded.

| | aarch64 | riscv64 | x86_64 |
|---|---:|---:|---:|
| `ipc_call_reply`, stored 2026-09-04 | 7,028 | 5,936 | 8,122 |
| `ipc_call_reply`, measured 2026-09-21 on `nightly-2026-09-20` | 7,104 | 6,038 | 8,234 |
| `syscall_entry`, stored | 1,508 | 1,828 | 1,637 |

The target is under 4 KiB, about an eighth of the smallest L1i we run on (radon's U74, 32 KB). The
shape the system runs is 1.47x to 2.01x that target. calef ruled on 2026-09-21 that the target is
printed and not gated. [Kernel footprint and caches](benchmarks/kernel-footprint-and-caches.md)
derives the target from Liedtke's argument, and [the gate](benchmarks/fastpath-footprint-gate.md)
has the method and its limits.

### x86_64

There is no HVF or KVM for x86_64 on this host, so `--real` there is plain TCG, whose magnitudes
mean nothing. The icount leg works and gates (since 2026-08-25). The I/O-bitmap write that
§121 (x86 port I/O) priced costs +216 ticks per iteration when nothing holds a port, against +2,373 for the
naive 8 KiB write ([x86 TSS I/O bitmap](benchmarks/x86-tss-iomap.md)). `CR4.PGE` and `CR4.PCIDE`
cannot be measured under TCG at all ([x86 instruments](benchmarks/x86-instruments.md)).

## What the instruments cannot see

icount models no caches, no TLB and no branch predictor. So:

- A count-neutral, cache-hostile change passes `--check`. The `--real` medians are the net.
- A TLB flush is one instruction. Milestone 58 (RISC-V TLB shootdown) removed one from every RISC-V switch and the count went
  up 1.2%.
- An 8 KiB `rep stos` is a handful of instructions. icount put the naive and lazy x86 bitmap writes
  16% apart when their memory traffic differs by 4,000x.
- `CR4.PGE` saves refills, and refills retire no instructions. Under TCG a global entry never survives
  a `CR3` write anyway.
- A struct growing from 128 to 136 bytes moved every icount row by at most 0.12% while the footprint
  gate failed. Neither tripwire substitutes for the other.

HVF sees real magnitudes on one machine only: no PMU, a 41 ns counter grain, a desktop OS
underneath, an L1 large enough to hide any footprint effect. argon's PMU is where that changes.

## What is not apples to apples

- Map is a tie, as above. `map_el0` aliases a frame and skips
  the zeroing, so it measures a different operation from Linux's first-touch fault.
- Spawn builds a lighter object. `fork` duplicates the parent; `spawn_el0` builds a minimal process
  from nothing, paying about ten traps to Unix's two. Most of the gap is that structural difference.
  The 4.4 us is also older than the path: icount has since measured it 41% shorter (2026-08-27), then
  6.3% longer for the current-CPU page (2026-09-21) ([`spawn_el0`](benchmarks/spawn-el0.md)).
- The context switch is derived by subtraction from two different mechanisms, a yield on our side and
  a pipe pass on theirs. Read the gap as a direction.
- Our IPC is a synchronous five-word rendezvous; `lat_pipe` is a buffered byte stream. A Mach port
  would likely beat the pipe. XNU is a hybrid whose syscalls are mostly in-kernel BSD calls.
- Against seL4: 2015 silicon against 2023, so part of the gap is the machine. Their fastpath is on and
  we have none. Their round trip is two syscalls; a service here issues three and `ipc_rtt_el0` four,
  and no EL0 twin of `call_reply` exists. They time one operation by PMU; we average a loop. We run
  under a hypervisor. Both sides use a benchmarking build (`cycle_counter_grant` ships off).
- Filesystem: the other side has a page cache and in-kernel metadata caching. Our write commits a
  transaction per request but issues no device flush unless asked, which sits between ext4's
  `O_DIRECT` and `O_DIRECT`+`O_DSYNC`. macOS APFS runs natively on NVMe, a different tier.
- The appendices' debug figures are debug: the IPC path pays about 6.7x at `-O0`, and a
  debug number in a release comparison is the error the calibration appendix records.

## Where a new finding goes

A dated finding goes in the appendix for its theme, or a new one under `notes/benchmarks/`, linked
from the table below. If it changes a current number, update that number here with its date in the
same commit. The prose budget, ratified by calef on 2026-09-23, caps this page and every appendix at 3,000
words.

## Appendices

The third column lists the dated entries each holds, so a citation of "notes/benchmarks.md, the
2026-08-24 section" resolves here.

| appendix | what it verifies | holds |
|---|---|---|
| [calibration-against-sel4](benchmarks/calibration-against-sel4.md) | the seL4 comparison | first numbers (2026-07-23); calibration (2026-08-04); the PMU wall |
| [sel4bench-on-xenon](benchmarks/sel4bench-on-xenon.md) | the method | 2026-10-21 |
| [cross-os-primitives](benchmarks/cross-os-primitives.md) | the cross-OS table and its caveats | milestone 19e (run a real workload); first EL0 and cross-OS numbers |
| [icount-drift-and-provenance](benchmarks/icount-drift-and-provenance.md) | the tripwire, the stamps, one-hart pinning | drift across builds; 2026-07-28; stamps (2026-09-21) |
| [per-core-and-multi-hart](benchmarks/per-core-and-multi-hart.md) | the `smp_*` numbers, single-hart `--real` | §28 (SMP placement); 2026-07-29 refresh |
| [service-path](benchmarks/service-path.md) | the confined-server tax | service path; 2026-07-29 and 2026-08-04 re-saves |
| [riscv-map-new-and-the-rfence-probe](benchmarks/riscv-map-new-and-the-rfence-probe.md) | never re-save an unexplained move | 2026-08-15; 2026-08-17 |
| [kernel-footprint-and-caches](benchmarks/kernel-footprint-and-caches.md) | the 4 KiB target | 2026-08-17; 2026-08-18; the target |
| [fastpath-footprint-gate](benchmarks/fastpath-footprint-gate.md) | the footprint numbers | 2026-08-18; 2026-08-27; 2026-09-04; 2026-09-21 |
| [filesystem-throughput](benchmarks/filesystem-throughput.md) | ext4 and APFS | milestone 38 (2026-08-18) |
| [record-level-sweep](benchmarks/record-level-sweep.md) | the 39.0 us block | 2026-08-18 sweep |
| [five-blocks-per-request](benchmarks/five-blocks-per-request.md) | the five repeated reads | the 208 us; option 2; the workload |
| [read-path-record-and-request-size](benchmarks/read-path-record-and-request-size.md) | 8 KiB record, 64 KiB request | 2026-08-18; 2026-08-19 |
| [read-path-block-contract-and-metadata-cache](benchmarks/read-path-block-contract-and-metadata-cache.md) | 189.9 MiB/s, the cache | 2026-08-19; controlled comparisons |
| [x86-tss-iomap](benchmarks/x86-tss-iomap.md) | +216 and +2,373 ticks | 2026-08-24; 2026-09-15 |
| [x86-instruments](benchmarks/x86-instruments.md) | what x86_64 can measure | 2026-08-25; 2026-09-19; 2026-09-23 |
| [spawn-el0](benchmarks/spawn-el0.md) | spawn since the HVF reading | 2026-08-27; 2026-09-21 |
| [drift-decomposition](benchmarks/drift-decomposition.md) | the shipping-feature rule | milestone 300 (decompose the icount baseline drift) |
| [baseline-save-audit](benchmarks/baseline-save-audit.md) | drift accumulates | 2026-09-15 audit |
| [counter-frequency-and-calibration](benchmarks/counter-frequency-and-calibration.md) | rates come from the machine | 2026-09-21 (two) |
| [icount-tick-scales](benchmarks/icount-tick-scales.md) | a tick is not an instruction | 2026-09-30 |
| [swish-check-x86-leg](benchmarks/swish-check-x86-leg.md) | the x86_64 shell leg was paced by ticks | 2026-10-03 |
| [rfence-self-row](benchmarks/rfence-self-row.md) | the open `rfence_self` row | 2026-09-21; 2026-09-23 |
| [preemption-in-the-window](benchmarks/preemption-in-the-window.md) | the masked `map_new` window | 2026-09-21 (three) |
| [drift-since-the-2026-09-26-floors](benchmarks/drift-since-the-2026-09-26-floors.md) | drift on `main` on all three ISAs | 2026-10-02 |
| [drift-since-the-2026-10-10-floor](benchmarks/drift-since-the-2026-10-10-floor.md) | four merges, and a tick that doubles one | 2026-10-19 |

## BUGS

- The HVF numbers are old. The newest is 2026-08-04, the cross-OS host columns are from 2026-07-25,
  and `spawn_el0`'s path has moved since. notes/register-of-measures.md flags the cross-OS row as due
  for re-taking, and it wants a quiet machine.
- No seL4 number from the same silicon exists. It waits on xenon or argon (milestone 127).
- The tripwire compares against the last floor, so steps under 10% accumulate. The 2026-09-15 audit
  found riscv64 `ctx_switch` 10.78% up across saves that never fired. Milestone 415 (sub-tripwire
  drift accumulates across baseline saves) now reports it weekly rather than gating it:
  [the drift report](project-metrics/baseline-drift.md).
- `map_el0` has `map_new`'s preemption-in-the-window shape and is not masked, because a kernel cannot
  mask a window it does not own. It has not been measured for this.
- The x86_64 two-core counters are deterministic but not a function of the code, and are not gated.
  Proposed in `design/roadmap/proposals/two-core-bench-is-a-different-instrument.md`.
- riscv64's `rfence_self` has oscillated between 5991 and 6476 across saves, and the cause (a hart
  count or the compiler) is unsettled. It reads 5991 today.
- `bench/fastpath-*.txt` names its nightly only from its next `--save --why` (since 2026-09-24),
  and nothing checks that stamp against the pin. Until then a compiler bump can fail that gate with no
  commit responsible. riscv64's `syscall_entry` sat at +4.7% of a 5% band on 2026-09-21.
