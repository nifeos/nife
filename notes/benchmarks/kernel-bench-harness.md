# The kernel bench harness: why each row is built the way it is

*Name: provisional.*

An appendix to [notes/benchmarks.md](../benchmarks.md). It carries the reasons behind
`kernel/src/bench.rs` and its children under `kernel/src/bench/`: how the bench boot ends, and
why each row's window, workload and gate are what they are. The numbers are on the main page.

## How the bench boot ends

`--features bench` makes the boot diverge into `bench::run` before the milestone tour. It runs
every benchmark in a fixed order, prints machine-readable lines, prints `bench: done` and halts. It
never semihosts. Under HVF the semihosting `hlt` traps to the guest instead of exiting, so the
contract is output-based in both modes: `xtask bench` owns the QEMU process, watches for the
marker and terminates it. The kernel parks in `wfi`, so a forgotten bench QEMU costs nothing.

Each line is `bench: <name> <counter_ticks> <iters>`, with the counter frequency printed first;
xtask does the division. Warmup iterations run untimed before each measurement, so thread spawn
and first-rendezvous costs land outside the window. Every program the boot enters is measured
first through `trust::require_program`, the chain the progenitor would have run; the hashing
happens at set-up, outside every timed window.

The two instruments, icount and HVF, and why icount is a coarse tripwire and HVF never gates, are
in the main page's first table and in [icount drift](icount-drift-and-provenance.md).

## The tick window

`TickWindow::open` waits for this core's next tick and `close` panics if another arrived since.
A ping-pong row (`ipc_rtt`, `ipc_rtt_el0`) is bimodal: each receive either finds its message
waiting or blocks for it, and a tick inside the window can flip the loop between the two. Under a
free-running tick an unrelated change moves those rows by whole percent. At the bench build's
`TICK_HZ` of 1, a window that opens on a tick has a full second before the next, and the close
proves on every run that the row fit. The opening tick also fixes a ping-pong's starting mode, so
`ipc_rtt` always measures the receiver-first mode. `yield_switch` and `spawn_reap` still do not
mask interrupts; this only moves where involuntary switches fall. Ticks are counted per core, so a
thread that changes cores inside the window cannot be checked, and a `--real --smp` run prints a
probe line instead.

## `map_new`: preemption masked across the window

The window is about 250 microseconds of guest time and the scheduler tick outside the bench build
is 10 ms. Whether a timer interrupt lands inside it depends on the phase the boot left the timer
in, which moves when unrelated code changes size.
[preemption-in-the-window.md](preemption-in-the-window.md) has the +26.4% reading that showed it.
Masking removes the coin flip: the interrupt is pending at `restore` and taken there, outside the
window, which is why `bench-probe: map_new_preemptions` can still read 1. Under `-icount` a masked
window also excludes whatever the rest of the machine would have done inside it, which is wanted:
the row prices mapping a page, not being descheduled while mapping one. It is safe because nothing
on the path blocks (`map_new` takes only spin locks). A row that means to measure scheduling, or
whose body can block, must not mask.

The preemption, fence and mask probes are counts, not durations, so they are probes and never
`timed` rows. A 10% tolerance over a count is not a tripwire, and their interesting values are
exact.

## `rfence_self`: one remote fence, on one hart

The 2026-08-15 `map_new` reading had two rival explanations, remote fences against an over-reporting
mask or the mask arithmetic itself, and they differ by orders of magnitude, so one fence's cost
decides it ([riscv-map-new-and-the-rfence-probe.md](riscv-map-new-and-the-rfence-probe.md)). A hart
may name itself in an SBI hart mask. The firmware serves that legal call with a local fence, and it
exercises the whole `ecall`-into-firmware path a shootdown pays and a local `sfence.vma` does not.
It must be single-hart: under `-icount` all harts share one virtual clock, so a second hart's idle
`wfi` dumps quantized time into the open window.

## The cycle probe

`cycles_per_tick` converts every board row from ticks of the reference counter into the cycles the
literature uses. It is a probe, never a row, because the rate moves with frequency scaling. Its
`cycles_per_tick_means` line names both halves of the ratio on each architecture, because a rate
printed under one name on three machines will be compared across them. `notes/pmu.md` has the
counters, and the aarch64 filter is calef's ruling A1 of 2026-10-07 UTC. Under emulation the
ratio is a fact about the emulator, and the line says so.

- The window is a timed spin, so both counters are read across the same span. A fixed iteration
  count would make the ratio depend on the code inside the loop, which is what differs between an
  emulator and a core.
- On riscv64 the window is a constant 100,000 ticks (10 ms on QEMU, 25 ms on radon), kept so every
  radon run stays comparable with the `100002 ticks` recorded by milestone 74 (cycle counters). On
  x86_64 it is ten milliseconds of a TSC whose rate this kernel measured, because a constant would
  be microseconds on one part and a visible pause on another.
- An unavailable counter prints the reason and never a number. A fallback to the reference clock
  would print an exact 1.00 on x86_64, where that clock is the TSC.
- x86_64's fixed counters are commonly 48 bits, so its delta is width-aware; a plain subtraction
  across a wrap would print a ratio near 2^64.
- The core is checked at each end, because on aarch64 each core's cycle counter is its own number.

## The EL0 rows

The `os_primitives_benchmarker` roles must match `fixtures/src/os_primitives_benchmarker.rs`. The
two crates share no header, so the numbers are mirrored by hand. `MAP_EL0_ITERS` and
`MAP_EL0_WARMUP` must equal its `MAP_ITERS` and `MAP_WARMUP`: the target region is sized for their
sum plus overhead, and maps past what it funds fail cheaply and skew the row.

`SPAWN_EL0_BUDGET` is 64 pages on purpose. Each child is split, run and destroyed strictly LIFO, so
its pages return to the budget (§16 (object revocation: reclaim the objects a process built)) and
only one child lives at a time. That 64 funds a 100-iteration loop, which would need over 1,000
pages without return-to-parent, is itself the proof that LIFO reclaim works.

`sink_throughput` reports bytes rather than iterations, because bytes compare against a Unix pipe.
`bench/host/pipe_throughput.rs` measures a Unix pipe twice: with the same sixteen-byte writes, which
isolates the cost of having no buffer, and with 64 KiB writes, which is what Unix gets. Only the
first is apples to apples. The producer holds `WRITE` and nothing else, as the left of a shell `|`
does.

## The filesystem rows

Every filesystem row is `--real --smp` only. Each is device- and interrupt-driven, interrupt timing
is not part of `-icount`'s instruction clock, and an icount baseline for them would enshrine that
non-determinism.

`fs_read` is a warm read loop through the confined stack. After the first read it is served from
the FS server's memory, like a Linux buffered read from the page cache, so the row is the confined
IPC path plus a copy (about 260 ns under HVF, release, 2026-09-26). The device path is what
`fs_seq_read` and its siblings measure, because their 1 MiB file is over the memo's 256 KiB limit.
An isolated file-server tax is not measured here: a few hundred nanoseconds of serving layer sits
inside a block read's run-to-run spread. The per-hop tax is `relay_rtt`'s, where it is gated.

`fs_throughput` prints a row and a MiB/s probe per phase. The row's ns/iter is the per-request
latency, which compares with `fs_read`; the probe compares with another operating system. One
transfer is 4096 bytes and cannot be more, because a request carries its payload through the one
page the client shares with the server, so these are a request rate in disguise.
[filesystem-throughput.md](filesystem-throughput.md) has the numbers. MiB/s is computed in integer
hundredths on the kernel side, where the counter frequency and transfer size are both known; a phase
whose ticks did not advance is a broken run and reports zero.

## The keyboard rows

`keyboard_up` and `keyboard_event` time the virtio keyboard driver, and only `script/bench
--keyboard` prints them, on any of the three architectures. That flag attaches the keyboard and a
monitor socket. An ordinary bench boot finds no keyboard, prints neither row, and gates nothing new.
`--keyboard` refuses `--check`, `--save` and `--real`, because no baseline holds the rows and a host
actor is in the loop.

`keyboard_up` runs from the spawn to the driver's `KEYBOARD_UP` report. That covers feature
negotiation, queue setup and posting the event buffers, and the host plays no part in it.

`keyboard_event` sums 16 key presses. Each runs from the kernel delivering the keyboard's interrupt
(`sched::watched_irq_at`, a bench-only timestamp in `irq_notify`) to the driver's `COMMIT` reaching
the bench thread, which stands in for the compositor. The wait for the host is outside the window,
and under icount an idle guest skips that wait anyway. The kernel prints a cue before each press,
and `xtask` answers it with one `sendkey` after 200 ms of real time. By then the previous release has
had its own interrupt, so every press is one interrupt carrying an `EV_KEY` and an `EV_SYN`. Two
runs on aarch64 gave identical counts. Milestone 947 has the numbers before and after the driver
moved onto `virtio-drivers`.

## Multi-hart throughput

Every other row is hart-pinned, so it is per-core path length and cannot show the placement of §28
(SMP placement: two random choices at spawn, message-shaped stealing, local wakes). `smp_throughput`
runs N independent workloads and times them by wall clock. It needs more than one hart, and under
`-icount` all vCPUs share one virtual clock and TCG serializes them onto one host thread, so it runs
only on the `--real --smp` boot and never gates. The scaling factor is the solo ns/iter over the all
ns/iter.

- Compute workers make no syscalls, so they make no cross-core wakes and scale cleanly to the core
  count: the direct picture of placement filling the machine.
- Ping-pong pipelines do not scale under HVF. A semi-idle guest vCPU is descheduled by the host, so
  each cross-core wake a split pair needs pays host reschedule latency. That is a virtualization
  property, not a scheduler defect ([per-core-and-multi-hart.md](per-core-and-multi-hart.md)).
- The main thread blocks through the window rather than busy-yielding. A runnable main makes the
  scheduler scatter the solo pair, every rendezvous becomes a cross-core wake, and the derived
  scaling goes superlinear, which is not physical.
- Threads are spawned first and released together after the clock starts, so an N-pipeline batch is
  not charged N spawns. Each batch drains to the baseline thread count before the next.
- Each batch runs several times and the minimum is kept: the host preempts guest vCPUs, and the
  minimum is the least-contended sample.

## E1 and E4: one real core

E1 (`ipc_thread_scaling`) and E4 (`app_displacement`) are tier A of milestone 134 (the register of
measures)
([design/roadmap/0134-the-measurements-that-decide.md](../../design/roadmap/0134-the-measurements-that-decide.md)).
E1 asks whether the kernel's own IPC path goes cache-cold as more threads cycle through it; E4 asks
what an IPC-heavy kernel costs an unrelated application's working set, Liedtke's actual claim. Both
need a real cache and one hart, so every thread contends for the same core's cache.
`real_single_hart_or_skip` checks both. A board card needs `single_hart`, which parks radon's
secondaries, and `smp_throughput` then self-skips on that card, so the runbook takes two cards
([footprint-perturbation.md](../footprint-perturbation.md)).

The real-core test is one value: the counter frequency QEMU `virt` states, 62.5 MHz on aarch64
and 10 MHz on riscv64. `NIFE_ACCEL` is host-side and never reaches the guest. It fails safe: an
unknown machine reads as real. It says "not QEMU `virt`", not "has a 32 KB L1", so the runbook
records which machine a number came from.

`SCALE_MAX_PAIRS` is 48, so the sweep stops at 96 threads, where the roadmap says "N from 2 to 128".
Read "N" as distinct threads cycling through IPC, as the prediction ("somewhere between 16 and 32
threads") does: 96 is three times past the predicted knee. It stays at 48 although
`sched::MAX_THREADS` is now 256, because the sweep is a published measurement and moving its end
would make every later run incomparable. Change it only if the knee moves.

E4 sweeps working sets of 4 to 128 KiB, below, at and above a typical 32 KB L1d and into L2, so it
can show whether displacement tracks one cache level. Its low-load condition is 8 pairs, inside
E1's flat region, so a null result there is expected from E1's curve. Its high-load condition is
`SCALE_MAX_PAIRS` pairs, past E1's knee (on radon the knee is at 16 threads, 1.68x), which is where
it can tell the kernel's path slowing from the application's cache being evicted
([cache-experiments.md](../register-of-measures/cache-experiments.md)). The workload yields about 64
times per run in every condition, so a compute-only thread cannot finish before the background pairs
run and the conditions differ only in what else is ready. It keeps the minimum of 5 batches, one
more than the pipelines' 4, because a batch that long catches more host preemptions.

## The IPC topology rows

`relay_rtt` is a two-hop request (client, relay, backend, relay, client), the shape the FS server
and `net_stack` both use. Its difference from `ipc_rtt` is what one confined intermediary costs,
deterministic and gated, which the real servers' end-to-end paths cannot be under `-icount`.

`broker_rtt` is the opt-in rung of milestone 23 (a capability-routed component OS with live
replacement) and §41 (the endpoint is the broker): `broker` interposed so a producer never blocks on
an absent consumer. Read against `call_reply`, the same client and backend with nothing between, the
difference is the whole tax, paid on every request in the steady state. The default rung has no row
of its own because it has no cost of its own: no process stands in the data path, so its steady
state is `call_reply`.

## The x86_64 I/O-bitmap rows

`tss_iomap_switch` writes the whole 8 KiB bitmap on every resume in both threads, since a real
switch-in hook fires whichever thread it lands on. `tss_iomap_lazy_switch` prices what ships,
`set_port_range_grant`, on the worst case: the two threads disagree (no ports, and COM1's
production range), so every switch crosses a holder. `tss_iomap_lazy_nop` is the common case, both
threads holding nothing, where the lazy form costs a load and a branch. Each reads against
`yield_switch` from the same boot. [x86-tss-iomap.md](x86-tss-iomap.md) has the numbers.
