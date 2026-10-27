# x86_64 instruments: the icount leg, CR4, and two cores

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds the x86 icount leg, why
`CR4.PGE`/`PCIDE` escape measurement under TCG, and the two-core counters, with the dates, tables
and corrections behind them. Name: ratified 2026-09-24 (calef); [the naming record](README.md) holds
it.*

## 2026-08-25: an icount leg for x86_64, and the "no icount leg" line was wrong

Milestone 161's roadmap (item 3, the `CR4.PCIDE`/`CR4.PGE` question) named the gap: turning either
bit on is an architect's call and wants a number, and `script/icount` had no x86 leg to produce one.
This section is that leg. It corrects the 2026-08-24 entry in [the TSS appendix](x86-tss-iomap.md),
which inferred, untested, that nothing pins x86's virtual clock to the instruction stream on `q35`.
Measuring showed the inference was backwards.

Two different questions were being conflated, and they have different answers.

- Does `icount()` (the instrument of milestone 78 (the load-sensitive assertions), `kernel/src/icount.rs`) work on `x86_64`? No, and
  this has not changed. Its claims compare an interrupt's arrival, and a re-armed deadline, against
  the deadline the kernel last wrote (`CNTV_CVAL_EL0` on aarch64, the SBI `DEADLINE` word on
  riscv64). `kernel/src/arch/x86_64/timer.rs::initialize` arms the local APIC timer in periodic mode
  (`interrupt_controller::arm_periodic_timer`, a fixed reload count the hardware reloads on its
  own). There is no
  deadline word to read back, so claims 1 and 4 have no x86_64 referent as designed. Building one
  would mean moving the shipping x86_64 tick source to one-shot/TSC-deadline mode. That is an
  architecture change to a production path, not a small addition to an instrument, and it stays
  out of scope. `icount()` still refuses `--arch x86_64` and should.
- Can QEMU pin its virtual clock to the instruction stream on `q35` at all, for a plain
  duration measurement? Yes. `kernel/src/bench.rs`'s `timed()` helper, which every `bench:` line
  uses on every ISA, needs only two `now()` reads around a span and has no opinion about
  deadlines. `now()` on `x86_64` already dispatches to `rdtsc` (`kernel/src/arch/x86_64/timer.rs`).
  The empirical question was whether `rdtsc` tracks `-icount`'s virtual clock under TCG on `q35`
  the way `CNTVCT_EL0` and riscv64's `rdtime` do on `virt`. It does.

### The evidence

`-icount shift=0,sleep=off` was added to `helpers/qemu-runner-x86_64.sh`'s invocation (the runner
already forwards extra QEMU args), booting the existing `--features bench` kernel. Three
consecutive boots produced byte-identical tick counts on every line. That includes the
PIT-calibrated TSC frequency itself (`bench: cntfrq 999935600`, all three runs). The calibrated
frequency lands within 0.006% of a clean 1 GHz, icount's own 1 instruction = 1 ns rate, which is
the tell `icount.rs`'s `calibrate()` checks for on the other two ISAs. The PIT-polling calibration
loop reads real ISA ports (`in8`/`out8` on `PIT_GATE_PORT`) and still introduced no
non-determinism. QEMU's 8254 model is clocked from `QEMU_CLOCK_VIRTUAL`, which `-icount` pins like
everything else a guest can observe.

`cargo xtask bench --x86` now takes the shape `bench()` already gives aarch64. The default is TCG +
`-icount shift=0,sleep=off`, deterministic, gated against `bench/baseline-x86_64.txt` with the same
10%-or-64-tick tripwire every other leg uses. `--real` is the plain-TCG statistical path the
2026-08-24 measurement used, unchanged and never gating. `--release` still implies `--real`: an
optimized build changes instruction counts, so it never gates on this ISA either, the rule aarch64
and riscv64 follow.

### One operational bug, which leaked a CPU-burning QEMU

`helpers/qemu-runner-x86_64.sh` is the one runner of the three that does not `exec` into
`qemu-system-x86_64`. Its header explains why: it translates `isa-debug-exit`'s always-odd exit
status. So on this leg `run_bench`'s `Child` is the wrapper shell, not QEMU, and killing it after
`bench: done` orphaned the real `qemu-system-x86_64`. Under plain TCG that orphan idles at ~0% CPU
in `hlt`. Under `-icount sleep=off` a parked guest's virtual clock never waits on the host, so the
orphan spun a full core indefinitely. Fixed in `run_bench` (`xtask/src/bench.rs`): `pkill -9 -P
<runner pid>` runs before the runner is killed. That is a no-op for aarch64 and riscv64, whose
runners already `exec`, so `pkill -P` finds no children there.

### Reproducibility

Deterministic here means deterministic against this exact QEMU build, this exact kernel binary, on
`q35`. Every icount leg carries that caveat, which is why `.qemu-version` pins QEMU for all three
ISAs. Three consecutive runs in one session show that TCG's icount accounting for `x86_64` under
`q35` does not fall back to wall-clock timing anywhere in this kernel's boot path. It was not tested
across a QEMU upgrade, a different host OS, or `-smp` greater than one. The runner already forces
one hart for the reason the other two legs do: under `-icount` every vCPU shares one virtual clock,
so an idle secondary parked in `wfi`/`hlt` would jump it forward.

### What this gives the `CR4.PCIDE`/`CR4.PGE` question

A gated number now exists for `yield_switch`, a bare kernel-thread switch with no I/O bitmap write:
18,264,216 ticks / 2000 iters = 9,132 instructions/switch, deterministic. Turning either `CR4` bit
on and re-running `--check` would show whether the change moves it at all. It does not answer the
question alone, because nothing on this port switched address spaces under load yet:
`switch_user_root` still skips the `CR3` write `yield_switch` exercises. So this baseline is a
kernel-thread switch's cost, not an address-space switch's. The tooling gap closed. The
measurement gap, a workload that actually switches `CR3`, stayed open and stayed an architect's
call. The 2026-09-19 entry below is that workload.

## 2026-09-19: `CR4.PGE` and `CR4.PCIDE`, and why this tree cannot yet measure either (milestone 161 (the x86_64 kernel port))

Milestone 161 carried "measure `CR4.PGE` and `CR4.PCIDE`" from item 3 onward, deferred each time
because nothing switched address spaces under load. Something does now, and a measurement exists.
The instrument cannot see the effect, so the obvious reading of the table below ("PGE saves
nothing") is wrong.

### What changes when PGE is on, and what does not

`CR4.PGE` lets a TLB entry whose leaf has `G` set survive a `CR3` write. This kernel already marks
every kernel mapping global and no user mapping global (`paging::Flags`). Turning it on changes no
instruction on any path. The only difference is which kernel translations the hardware still holds
after a context switch writes `CR3`. The saving is refills, and refills are not instructions.

### The workload

`cargo xtask bench --x86` boots with no initrd, so every `_el0` bench self-skips and nothing in its
baseline writes `CR3` (`yield_switch` and friends are kernel threads sharing one root). Run by hand
with `target/initial-ramdisk-x86_64.img` (`cargo xtask initial-ramdisk-x86`) attached, the EL0 plane runs on x86_64 for
the first time. `ctx_switch` is two processes yielding to each other, so every switch is a `CR3`
write. `ipc_rtt_el0` is a round trip between two processes, two per iteration.

### icount: byte-identical, by construction

Same tree, two builds differing only in setting `CR4.PGE` after the fine map is installed (in
`mmu::initialize` and `mmu::initialize_secondary`), `-icount shift=0,sleep=off`, one boot each:

| bench | iters | ticks, PGE off | ticks, PGE on |
|---|---|---|---|
| `yield_switch` | 2000 | 18,903,108 | 18,903,108 |
| `null_syscall` | 20000 | 4,520,052 | 4,520,052 |
| `ctx_switch` | 5000 | 48,943,033 | 48,943,033 |
| `ipc_rtt_el0` | 5000 | 178,398,567 | 178,398,567 |
| `map_el0` | 500 | 5,959,166 | 5,959,166 |

Every one of the 17 lines matched to the tick. That confirms the premise (PGE costs no instruction)
and says nothing about the benefit, since icount's clock advances per retired instruction and a TLB
walk retires none.

### Plain TCG wall clock: noise, and it could only ever be noise

Three boots each, same host, `--real`-style (no icount), ns per iteration from the guest's
PIT-calibrated TSC at 1.002 GHz:

| bench | PGE off (3 boots) | PGE on (3 boots) |
|---|---|---|
| `ctx_switch` | 40.35, 40.03, 40.27 µs | 40.23, 39.79, 39.62 µs |
| `ipc_rtt_el0` | 114.40, 117.05, 117.18 µs | 116.36, 116.10, 112.54 µs |

Overlapping, about 1% apart, on a shared host with other lanes running. No difference was possible.
The pinned QEMU (11.0.2) implements `CR3` writes as `cpu_x86_update_cr3`, which calls `tlb_flush` on
the whole softmmu TLB whenever paging is on and reads neither `PGE` nor `PCIDE`
(`target/i386/helper.c`, read at the `v11.0.2` tag on 2026-09-19). Under TCG a global entry never
survives a switch, and a `PCIDE`-tagged entry goes with everything else.

*(Correction, 2026-09-24: these `ns` figures come from the stored TSC rate, which the 2026-09-21
calibration finding showed could be inflated by up to 12x on any boot. See
[the calibration appendix](counter-frequency-and-calibration.md). The rate quoted here, 1.002 GHz,
is within 0.2% of the true 1000.000 MHz, so these boots were not badly hit. The conclusion does not
rest on the figures anyway: TCG flushes the whole TLB either way.)*

### What would measure it

A real TLB. Two routes exist and both are one step from usable:

- KVM on cordoba (i5-4670, Haswell: `pcid`, `invpcid`, `pdpe1gb` and `pge` all in
  `/proc/cpuinfo`). A no-sudo QEMU 8.2.2 was unpacked from Ubuntu's own packages into
  `~/nife-kvm/root` there and runs. `-enable-kvm` fails with `Permission denied`, because
  `/dev/kvm` is `root:kvm 0660` and calef's account is not in `kvm`. `sudo usermod -aG kvm calef` on
  cordoba is the whole blocker, and it is calef's to run. Then: the two bench kernels above, with
  the initrd, under `-enable-kvm -cpu host`, a few boots each, `ctx_switch` / `ipc_rtt_el0` read
  against each other.
- A bench boot on xenon (i5-7500T), the machine the answer is actually about.

Neither is gating. The number from either is a `--real` number: it reports and never gates.

### PCIDE is a design, not a bit

Turning PGE on is two lines and changes no invariant this kernel relies on: every kernel unmap
already ends in `invlpg`, which does invalidate a global entry. PCIDE is different. With it on,
`invlpg` and a `CR3` write act on the current tag only. So `mmu::unmap_user_at` on a space that is
not installed, `mmu::flush_asid`, and the NMI shootdown's discard-everything arm would each leave
another space's tagged entries alive, a stale-translation defect. The fix is `INVPCID` on each of
those paths (present on both machines above) and a tag-reuse rule against
`crates/address_space_identifier`'s generations. `kernel/src/arch/x86_64/mmu.rs`'s BUGS records both
bits and this measurement.

## 2026-09-23: five x86_64 counters left the tripwire and no benchmarked code had changed

Milestone 315 (a port revoke that reaches every core) flipped `helpers/qemu-runner-x86_64.sh`'s
`NIFE_SMP` default from 1 to 2, per DECISIONS §153 (how a two-core x86_64 test earns its place).
CI's `bench (icount regression tripwire)` then failed on x86_64 with five counters out of bounds,
four of them faster:

| counter | measured | baseline | change |
|---|---|---|---|
| `tss_iomap_lazy_switch` | 5,624,840 | 23,586,100 | -76% |
| `tss_iomap_switch` | 8,094,599 | 23,937,905 | -66% |
| `tss_iomap_lazy_nop` | 7,225,718 | 19,609,872 | -63% |
| `yield_switch` | 16,763,552 | 19,188,747 | -13% |
| `spawn_reap` | 21,026,750 | 2,846,009 | **+639%** |

A 7.4x regression on `spawn_reap`, next to a fix that touched the scheduler's locked region, fits
that shape as well as the flip does. So it was measured rather than assumed.

### `bench_x86` was the one arm that never pinned its core count

`bench()` pins `NIFE_SMP=1` for aarch64 and `bench_riscv` pins it for riscv64, each with the reason
at the call site: a primitive benchmark measures per-core path length. `bench_x86` set the variable
nowhere and took the runner's default, while both its `eprintln!`s said "single hart". The flip made
that line false and nothing said so. It is pinned now.

### The measurements

Five builds, all `cargo xtask bench --x86`, TCG + `-icount shift=0,sleep=off`, QEMU 11.1.1,
`spawn_reap` in ticks over 64 iterations:

| tree | cores | `spawn_reap` |
|---|---|---|
| milestone 315 | 1 | 2,911,921 |
| `main` (d53d6bb19) | 2 | 11,816,362 |
| `main` + only the `install_port_grant` move | 2 | 14,409,216 |
| milestone 315 | 2 | 21,026,750 |
| milestone 315 + two `fetch_add` probes | 2 | 12,093,916 |

At one core this branch is honest against the old baseline. `--check` passes, the worst row is
`spawn_reap` at +2.3%, and every other row is inside 1.2%. So no counter moved because of the code.

At two cores the flip alone accounts for most of it. `main`, with none of milestone 315 in it,
already runs `spawn_reap` at 4.15x the baseline.

The counters that involve no cross-core scheduling do not move at all. `ipc_rtt`, `relay_rtt`,
`call_reply`, `broker_rtt` and `coremark` are within 0.4% at one core and two. That rules out the
obvious systematic explanation: a second core is not inflating the shared icount clock, because a
halted vCPU consumes no instructions.

### `spawn_reap`, specifically

`bench::spawn_reap` spawns a thread that exits immediately and then busy-yields until the reaper has
returned `thread_count` to baseline. A probe counting the parent's spins and the cross-core rounds
inside the timed window:

| | 1 core | 2 cores |
|---|---|---|
| parent `yield_now` calls, 64 iterations | 22 | 4,205 |
| shootdown broadcast rounds | 390 (all no-ops: no other core) | 384 (real NMI round trips) |

The cause is the benchmark's wait loop, not new work. At one core the child runs on the same core
and is usually reaped before the first check, so the loop spins 0.34 times per iteration. At two
cores the child is placed on the other core, and the parent spins 66 times per iteration while
waiting, 191x more. Each spin is a full `schedule()` taking `IPC_TABLES`. At about 2,200 ticks for a
yield with nothing else runnable, 4,183 extra spins is ~9.2M ticks, the entire gap. The 384
cross-core TLB-shootdown NMIs are the smaller term: about six per iteration, each waking a halted
core and spinning for its acknowledgement. They are real new cost at two cores. At one core
`broadcast` finds no other core and returns.

Milestone 315's own broadcast (`segments::revoke_port_grant_everywhere`) is not on this path at all.
It runs from `PortRange::REVOKE`, which `spawn_reap` never calls.

### At two cores the counters stop being a function of the code

Three consecutive `NIFE_SMP=2` runs of the same binary were byte-identical on every row, so the
instrument has not gone random. But the table above is not monotone and cannot be read as one:

- Moving `install_port_grant` inside the lock, alone, on `main` (a `#[cfg]`-gated call of roughly
  fifteen instructions) moved `tss_iomap_lazy_switch` +89% and `yield_switch` +15%.
- Adding two `fetch_add`s for the probe, which changes no semantics, moved `spawn_reap` -42%. That
  swing is as large as the whole difference between `main` and this branch.

So the apparent 1.78x between `main` at two cores and this branch at two cores is not attributable
to milestone 315's code. It is the instrument's sensitivity to which core wins a race, and a 10%
tripwire over that would fire on unrelated changes forever.

### What was done, and what is left

The baseline was not re-saved. Nothing it records has changed, because the x86_64 icount bench is
single-core again, as its own output always claimed. Gating x86_64 at two cores wants its own
baseline file, its own tolerance, and benchmarks whose wait loops are not races. That is a proposed
milestone (unnumbered when written; a lane does not mint one), now filed as
`design/roadmap/proposals/two-core-bench-is-a-different-instrument.md`. Until it exists the two-core
numbers are available by hand with `NIFE_SMP=2 script/bench --x86` and are not gated.
