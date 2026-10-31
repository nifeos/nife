# Cross-OS primitives: nife, Linux and macOS

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds the EL0 primitive suite, the first
cross-OS table, and the map tie and spawn caveats, with the dates, tables and corrections behind
them. Name: ratified 2026-09-24 (calef); [the naming record](README.md) holds it.*

## Compute vs. OS primitives: two benchmarks that measure different things (milestone 19e)

The kernel-side microbenchmarks in [notes/benchmarks.md](../benchmarks.md) are the right kind for a
microkernel: IPC, context switch, the paths a microkernel lives on. But "run a real workload" (19e)
wanted a whole compute program. Working out how to compare it across OSs turned up a distinction
that decides what any cross-OS comparison can show.

### Compute is OS-independent

A tight compute loop running in userspace does not touch the OS: the CPU executes the same
instructions whoever scheduled it. So a compute benchmark (CoreMark, Dhrystone) on nife, macOS and
Linux on the same core comes out nearly identical. The small gaps are compiler codegen or allocator
noise, not OS quality. That is a real result ("we add no hidden compute overhead") but a null one by
design, because the OS is not in the loop.

### OS primitives are where an OS shows itself

Syscall entry, context switch, IPC round trip, page map, page fault, thread spawn: these are the OS,
and they distinguish Linux from macOS from us. The same source cannot measure them across three OSs,
because "the same syscall" does not exist on all three. You invoke each OS's own primitive: `getpid`
on Linux, a Mach/BSD call on macOS, our `svc` null-invoke. So the OS-revealing benchmark is a
matched harness per OS: one metric definition, three native implementations. That is what lmbench
is, and how the L4/seL4 papers compare to Linux. Our microbenchmarks are the nife side of it.

### The CoreMark workload (`crates/coremark`, `fixtures/src/coremark.rs`)

19e's real workload is CoreMark: the three work items of a CoreMark iteration (a linked-list sort, a
small-matrix multiply, a state machine over a byte buffer). Each folds into a CRC, so the compiler
cannot delete the work and a run self-validates. It runs as a spawned EL0 program against the native
ABI. The progenitor builds the `"coremark"` binary and grants it one endpoint; it computes and SENDs
the run's CRC home. `coremark::PINNED_CRC_64` (`0x7954` for 64 iterations) is asserted by both the
host crate test and the kernel test. So the same computation gives the same answer on the host and
on the kernel's target, the property a cross-OS comparison rests on.

It is a Rust reimplementation, not EEMBC-certified CoreMark: a certified score needs the unmodified
reference C. The Rust choice buys what matters for our comparison: the identical source compiles for
nife, macOS and Linux, so the compute run is one program on three OSs. This binary reports
correctness, not yet a score. Timing a run needs a userspace clock (the EL0 virtual-counter read, as
Linux does for its vDSO), which lands with the cross-OS suite.

### The measurement plane: kernel-side (gating) vs EL0 (cross-OS)

The kernel-side microbenchmarks run in kernel context. The bench threads are kernel threads calling
`sched::yield_now` and `sched::ipc_send/receive` directly, so they measure each operation's
kernel-internal path length. That is right for regression gating: a code-path change moves the count
next to its commit. It is not what lmbench measures. lmbench runs a userspace program making real
syscalls, so its numbers include the EL0→EL1 trap and return that a kernel-side benchmark skips.

So the cross-OS primitive numbers come from EL0, from a userspace program that self-times a
loop of real `svc` syscalls. That is why milestone 19e (run a real workload) opened EL0 access to the virtual counter
(`CNTKCTL_EL1.EL0VCTEN`; `user_mode_runtime::now`/`cntfrq`; [notes/abi.md](../abi.md)). CoreMark is
the first program to use it, reporting `[crc, ticks, freq]`. The EL0 primitive benchmarks (null
syscall, context switch, IPC round trip, page map, all measured the lmbench way) build on the same
`user_mode_runtime::now`. The kernel-side suite stays, for gating; the EL0 suite is additive, for
cross-OS honesty. The two differ by roughly the trap cost, itself a number worth having.

### The first EL0 numbers (nife, M-series host, HVF, debug build)

The `os_primitives_benchmarker` program (`fixtures/src/os_primitives_benchmarker.rs`), spawned by
the bench boot, self-times each primitive from EL0 and reports it as a normal bench line:

| primitive | HVF ns/iter | what one iteration is |
|---|---|---|
| `null_syscall` | ~42 | one `svc` that the kernel rejects immediately: trap + dispatch + return |
| `ctx_switch` | ~692 | one `SYS_YIELD` to a peer *process* and back: two switches, address space included |
| `ipc_rtt_el0` | ~2272 | a `SEND` to a server process and a `RECEIVE` of its reply: two rendezvous, four `svc`s |
| `map_el0` | ~909 | `invoke(aspace, MAP_INTO, va, frame, RO)`: trap + cap resolve + walk + PTE + record |

Two sanity checks pass. A context switch is ~16x a null syscall: two traps, the scheduler, two
register save/restores and a TTBR0/ASID change, against one bare trap. And the round trip matches its
parts: two context switches (2 × 692) plus four traps (4 × 42) plus dispatch ≈ 2272.

The EL0 round trip has a kernel-side twin, the milestone-21 `ipc_rtt`: ~951 ns in this same
2026-07-25 debug run. (The ~705 ns from 2026-07-23 is a different debug binary; see
[the calibration appendix](calibration-against-sel4.md).) It measures the same rendezvous without the
EL0↔EL1 crossings. The ~1.3 µs gap is the trap cost of the four `svc`s a real round trip pays, which
is why the EL0 numbers are what compare to lmbench. Every figure in this subsection is a debug build.
The cross-OS comparison and the L4 calibration both want release builds on all sides. These line up
against lmbench's `lat_syscall` / `lat_ctx` / `lat_pipe` and `sel4bench`.

### Map: the primitive where the answer is a tie

Map (lmbench's `lat_mmap`) behaves differently from the other three. It taught three things.

First, it consumes resources per call. Every `MAP_INTO` writes a page-table entry and a revocation
record, paid from the target space's untyped region, so unlike a null syscall it cannot loop
forever. The loop runs 500 maps at most, one L3 table's worth; the kernel-side twin `map_new` maps
64. There is no unmap in the surface yet, so each map needs a fresh VA.

Second, the debug and release numbers diverged by ~10x, far more than any other primitive.
`map_el0` aliases one existing frame at every VA, so it does no page allocation and no zeroing. It is
trap + capability resolve + walk + PTE write + a `record_mapping` append. That append scans the head
log page for a free slot, an ~85-entry linear walk on average. In a debug build that unoptimized scan
dominated the number (~909 ns). Release compiles the scan to almost nothing, and the mapping
mechanism shows through: ~91 ns. The kernel-side `map_new` is ~524 ns in release and barely moved
from debug. Its cost is zeroing the 4 KiB a fresh frame needs (`retype_page` hands back a zeroed
page), which is memory-bandwidth bound.

That walk length is a constant in `kernel/src/revoke.rs`, `LOG_ENTRIES`. It was ~128 at the ~909 ns
reading. A log page held 255 records until §132 (what `PageFrame::REVOKE` owes an
overlapping run) gave each one a third word naming the capability it was made under. That took the
page to 170. So `LOG_ENTRIES` is a benchmark input, and
the icount tripwire noticed. §132's branch came in at `map_el0` -16.9% aarch64 and -16.4% riscv64,
unexplained by anything else on the branch. Setting `LOG_ENTRIES` to 170 on `main` and changing
nothing else reproduces it within 0.7%. A future change to that constant moves two benches on two
ISAs; whoever makes it should expect to re-record them.

Third, `map_el0` and the host `lat_mmap` do not measure the same thing. The host number is a
first-touch page fault, which allocates and zeroes a fresh page; `map_el0` skips both. So `map_el0`'s
~91 ns is the pure mapping mechanism, genuinely lean, and not comparable to Linux's ~534 ns, most of
which is zeroing. The like-for-like comparison is our `map_new` (allocate + zero + map), ~524 ns,
plus one trap (~28 ns) for the EL0 crossing the host's fault includes. That gives ~552 ns, against
Linux ~534 ns and macOS ~556 ns: a three-way tie. Page provisioning is dominated by zeroing 4 KiB,
the same silicon and bandwidth for all three. Nobody can win it without zeroing memory faster. A
fair EL0 map that provisions a fresh page waits on retype-from-untyped reaching userspace (a later
milestone). Until then the kernel-side `map_new` is the stand-in.

*Later reading, 2026-07-29: `map_new` measured ~470 ns in
[the per-core refresh](per-core-and-multi-hart.md), within run-to-run noise of 524 and still
zeroing-bound. The tie stands.*

### The first cross-OS numbers (nife vs Linux vs macOS, 2026-07-25; spawn row 2026-07-26)

`bench/host/` holds the host side of each metric:

- `null_syscall.rs`: a raw `getpid` through the syscall gate, not libc's cached `getpid`, which
  never traps.
- `ipc_rtt.rs`: a pipe round trip between two forked processes, lmbench's `lat_pipe`.
- `ctx_switch.rs`: the derived context switch.
- `mmap.rs`: first-touch fault-in, lmbench's `lat_mmap`.
- `spawn.rs`: fork+exit, lmbench's `lat_proc`.

They run two ways. Natively on macOS (`rustc -O ... && ./bin`). And on Linux at the same tier as
nife: `bench/host/run_linux.sh` cross-compiles a static musl binary (`linux_all.rs`, the five metrics
combined), packs it as `/init` in a one-file initramfs, and boots it under QEMU-HVF, the machine nife
boots on. So Linux and nife sit on the same M-series core at the same virtualization tier; native
macOS is the bare-metal ceiling.

Run nife optimized with `cargo xtask bench --release`, which builds an opt-level-3 kernel and
userspace and implies `--real`:

| metric | nife **release** (HVF) | Linux (static musl, HVF) | macOS/XNU (native) |
|---|---|---|---|
| null syscall | **~27 ns** | ~139 ns | ~76 ns |
| context switch (per switch, derived) | **~28 ns** | ~415 ns | ~818 ns |
| IPC round trip | **~337 ns** | ~1723 ns | ~2620 ns |
| map a fresh page (provision + map) | ~552 ns (`map_new` + trap) | ~534 ns | ~556 ns |
| map mechanism only (aliased, no zeroing) | ~91 ns (`map_el0`) | n/a (fault always zeroes) | n/a |
| spawn (build + run + reap + reclaim) | **~7.7 µs** (`spawn_el0`) | ~19.7 µs (fork+exit) | ~291 µs (fork+exit) |

The dates in the heading were recovered from `git log` on 2026-09-24 (commits `a1dc71020` and
`dce3df459`); the table itself carried none, which
[notes/register-of-measures.md](../register-of-measures.md) flagged.

nife wins four and ties one. Same M-series core, same HVF tier as Linux, both optimized. It is ~5x
faster than Linux at the null syscall (27 vs 139) and ~5x at the IPC round trip (337 vs 1723). It
beats native macOS at both, and it builds a process faster than either. An IPC round trip in the low
hundreds of nanoseconds, next to the reference OS on the same silicon, is seL4-class. **"seL4-class"
is a claim about magnitude and nothing more.** Its worth against seL4's published cycles, and the
ways that comparison is not apples-to-apples, are in
[the calibration appendix](calibration-against-sel4.md); quoting one without the other is how the
last overstatement happened.

Map is a deliberate non-win, for the zeroing reason above: all three land near ~550 ns. The ~91 ns
mechanism is real, but it is not a page an application can use, so it stays out of the win column.
The map row compares like with like; the ~91 ns sits below it as the mechanism floor.

### Spawn: a real win, with a caveat

`spawn_el0` builds a whole child from EL0: `SPLIT` a region, retype an address space and a TCB, map
code and a stack, configure, start. It runs the child to exit, reaps it, and `DESTROY`s its region,
in a self-timed loop that only repeats because object revocation reclaims each child
([notes/object-revocation.md](../object-revocation.md)). At ~7.7 µs it beat Linux `fork`+`exit`
(~19.7 µs) by ~2.6x and macOS by ~38x, on the same core. It did so while paying more boundary
crossings than Unix: ~10 `svc`s per spawn against `fork`+`wait`'s two.

The operations differ. `fork` duplicates the parent: its address space copy-on-write, its descriptor
table, its signal state. nife builds a fresh minimal process from nothing. A capability-microkernel
process is a lighter object than a Unix one, so the gap is mostly that structural difference, not a
faster version of the same work. We use `fork`+`exit`, not `fork`+`exec`, to keep the Unix side as
light as it gets (no binary loaded). It still carries duplication that nife's build does not. The
number stands with its meaning: building a process is cheap when a process is a small thing.

*Correction, 2026-09-24, from the split.* An earlier edit (2026-09-21) called the ~7.7 µs "a
2026-09-21 reading" and derived ~8.2 µs and ~2.4x from it after the current-CPU page made the path
6.3% longer ([the spawn appendix](spawn-el0.md)). Both parts are wrong. The ~7.7 µs dates from
2026-07-26. [The 2026-07-29 refresh](per-core-and-multi-hart.md) measured the settled per-core median
at ~4.4 µs and called 7.7 a single sample on a busier machine. So the ~8.2 µs was computed from a
stale figure. The latest measured HVF value is ~4.4 µs (2026-07-29). The icount path has moved
since (-41% on 2026-08-27, +6.3% on 2026-09-21), and no HVF reading postdates it.

### The context switch, the softest of the three

No OS lets you time a bare switch, so it is derived. On the host, `bench/host/ctx_switch.rs`
measures a two-process pipe round trip (two switches plus two pipe passes). It subtracts a self-pipe
pass, a `write`+`read` with no switch. One switch is then `round_trip/2 - self_pipe`. nife's
`ctx_switch` bench is a yield round trip (two switches plus two `SYS_YIELD`s); subtracting the trap
(`~2 x null_syscall`) leaves ~28 ns per switch. The subtraction is approximate and the mechanisms
differ (our lightweight yield against a pipe pass). So read the ~15x gap to Linux as directional,
not exact. It points the same way as the other two: three metrics, three methods, all favoring the
minimal kernel.

### Debug told the opposite story at IPC

Debug nife: null syscall ~42 ns, ctx switch ~692 ns, IPC ~2272 ns. So `-O0` was a ~1.5x tax on the
bare syscall, which still won, but a ~6.7x tax on IPC, which lost to Linux at 1723 ns. The heavier a
path, the more the optimizer matters. The IPC path, two context switches plus four traps plus the
rendezvous, is heavy. The null-syscall win survived the debug handicap; the debug handicap hid the
IPC win. Measuring both builds is why we can say which.

### Caveats that remain

Our endpoint is a synchronous three-word rendezvous; a Unix pipe is a buffered byte stream through
a kernel buffer. So this is our native IPC against Unix's standard IPC (`lat_pipe`), not XNU's
fastest; a Mach port would likely beat the pipe. *(Later, 2026-07-29: the mailbox widened to five
words, milestone 22 (trusted init) §26 (the fault endpoint); see [the per-core refresh](per-core-and-multi-hart.md).)* The host context
switch still wants lmbench's ring method to isolate cleanly. `sel4bench`, the one peer that would
say how close to the state of the art these numbers are, is the remaining comparison.

## Three of the EL0 rows run at the production shell

`null_syscall`, `sink_producer | sink_consumer` (the `sink_throughput` row) and `map_el0 --mem 10`
are programs of their own, so a person types them at the prompt of the kernel that ships. Each
prints one row of ticks, never converted time, under `bench-production:`. The bench boot spawns the
same programs and its rows read `bench:`. `primitive_benchmark_contract` holds the loop counts and
the row spelling, so both runs time the same thing. A production row competes with the console,
`net_stack` and the shell for the core, which makes it a different measure from the quiet boot's.
Record it in `bench/` beside the quiet boot's rows; no gate reads it (milestone 961 (the kernel's
special boots become commands typed at the production shell), decision D6). Quote the boot log's
counter-frequency line beside the ticks to convert them. `ctx_switch`, `ipc_rtt_el0` and
`spawn_el0` need a peer process or process construction, so they stay in the bench boot. The first
production rows are in `bench/qemu-2026-10-31/`.
