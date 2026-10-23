# The user-mode runtime: time

This appendix to [the user-mode runtime](../user-mode-runtime.md) covers `clock.rs`: the counter,
its frequency, the cycle counter and the current CPU.

## The counter

`now` is a plain register read with no syscall on every architecture. Each target opens it to user
mode differently:

| architecture | instruction | what opens it |
|---|---|---|
| aarch64 | `mrs cntvct_el0` | the kernel sets `CNTKCTL_EL1.EL0VCTEN` in `timer::init` (notes/abi.md) |
| riscv64 | `rdtime` | the kernel sets `scounteren.TM` in its per-hart timer init |
| `x86_64` | `rdtsc` | `CR4.TSD` is clear, the reset state, and the kernel never writes it |

On `x86_64` the permissive state is the default, so the kernel would have to act to close it.
`rdtsc` returns the count split across `edx:eax`, so `now` shifts and ors the halves. A single
`out(reg)` compiles and reads only the low half, which wraps every four seconds at 1 GHz. `now` does
not serialize `rdtsc`. The reordering window is tens of cycles, the trade the kernel's own
calibration accepts.

## The frequency

`cntfrq` says how many ticks make a second. aarch64 states it in `CNTFRQ_EL0`, so there it cannot
fail. QEMU reports 62.5 MHz under TCG and the host's counter frequency under HVF.

RISC-V has no register that reports the timebase. The machine states it in the device tree at
`/cpus/timebase-frequency`, and a process holds neither a pointer to the blob nor a capability to
its memory. `kernel::arch::riscv64::timer::init_frequency` reads it on the boot hart and
`kernel::user::timebase_page_phys` publishes it. radon (the VisionFive 2) runs its `time` CSR at
4 MHz, so no constant could stand in for it.

x86 has no architected rate a ring-3 program can ask for. `CPUID` leaf `0x15` is absent on many
parts, and calibrating against the 8254 PIT needs port I/O, which §121 (what a device capability is
when the device has no page: x86 port I/O) keeps from userspace.
`arch::x86_64::timer::init_frequency` reads the leaf or calibrates, and every kernel path that
builds a top-level address space maps the page (`kernel::user::map_x86_timebase_page`).

Both publish the rate on the timebase page, mapped read-only into every process at
`counter_frequency_contract::PAGE_VA`. `supervision_protocol::build_child_space` fills a child's
page from its parent's rate, so a subtree inherits the measured number. A parent that does not know
the rate writes a zeroed page, and its whole subtree refuses rather than guessing.
`cntfrq_checked` returns `None` for a zeroed or unrecognized page, and `cntfrq` panics on it.
calef's ruling behind that, and what the hardcoded rate it replaced cost, are in
notes/benchmarks/counter-frequency-and-calibration.md. A plausible wrong rate produces a benchmark
figure somebody quotes. The panic traps and the kernel kills the process with no message, the floor
every userspace `expect` has. The checked and unchecked pair matches the kernel's
`arch::timer::frequency` and `frequency_checked`, so `monotonic_nanos` keeps its `u64`.

## Monotonic nanoseconds

`monotonic_nanos` is here because the naive form is wrong. `ticks * 1_000_000_000` overflows a
`u64` about five minutes into a boot at 62.5 MHz, so the conversion splits into whole seconds and a
remainder. `components/src/clock.rs`, `date` and the shell's `time` each need it. It needs no
capability, because the counter is ambient, so any process can measure a duration. The wall clock
is this plus an offset only the clock page carries, and that does need a capability
(notes/clock.md).

## The cycle counter

`cycle_reading` returns a count together with what it counts, because the count alone does not
compare across machines. aarch64 counts core cycles at EL0 and EL1 under the `PMCCFILTR_EL0` of zero
the kernel writes on every core. riscv64's `cycle` is `mcycle`, every privilege mode. `x86_64`'s TSC
counts reference cycles at a constant rate. A difference of two reads means something only when
both were taken on the same core, because the aarch64 and riscv64 counters are per core and start at
different times.

On aarch64 and riscv64 the instruction traps unless the kernel opened `PMUSERENR_EL0` or
`scounteren.CY` for the thread (milestone 229 (build the cycle-counter grant DECISIONS 139
decided)), and an ungranted thread is killed. §139 (who may read the cycle counter, and by what
authority) option 4 chose a grant that keeps the read a single instruction with no syscall.

On `x86_64` the TSC is both the coarse and the fine counter. `CR4.TSD` is clear at reset and never
written, so every ring-3 program holds a sub-nanosecond timing instrument it was never granted.
`CR4.PCE`, the `rdpmc` door, is closed per core in `arch::init`. Setting `CR4.TSD` would take away
`Instant`, `thread::sleep`, the random seed and smoltcp's timestamps at once. It needs a coarse
monotonic page first, the move §43 (reading the clock is a page) made one axis over. Until then
`x86_64` answers milestone 75 (who may read the cycle counter, and by what authority) with
"everyone, always", against §19 (architectural parity is a tenet).

## The current CPU

`current_cpu` is a plain load from a per-thread page. §204 (how userspace asks where a thread runs)
gives a thread observing itself a page and a thread observing another a selector. The consumer is a
memory allocator keeping a per-CPU cache, which asks once per allocation, and a crossing at that
rate cannot be amortized away. `crates/current_cpu_contract` carries the layout, the ordering
argument and why this is not a register on any of the three targets.

The answer can be stale before it is used, because the kernel may migrate the thread the instruction
after the load. A per-CPU cache built on it must be correct when the answer is the previous core's,
and merely faster when it is not. Linux solves this with `rseq`'s restartable sequences, which this
tree does not have. The value is a CPU id, not an index into anything counted. The VisionFive 2's
online set is `{1, 2, 3}`, so a per-CPU array sized by a core count misses a core. Size it by
`current_cpu_contract::CPU_ID_BOUND`.

Every space that runs a thread gets the page: `kernel::user::AddressSpace::new` maps it for the
kernel-built spaces and `kernel::sched::configure_thread_control_block` for the userspace-built
ones. The space with no page is one with no thread in it, which has nothing to ask.

