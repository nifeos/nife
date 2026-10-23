# The per-CPU block

*Name: provisional, for this note. calef names files.*

Each core reaches its own private data in one instruction, through a pointer the architecture
keeps in a register for that purpose: `TPIDR_EL1` on aarch64, `tp` on RISC-V, `IA32_GS_BASE` on
`x86_64`. That block is `kernel/src/cpu.rs`'s `PerCpu`. It is what lets the scheduler keep a run
queue, a current thread and lock bookkeeping per core instead of one per machine
([DECISIONS §11](../design/decisions/0011-per-cpu-run-queues.md)). This note says what holds the block's shape in
place. [scheduler.md](scheduler.md) says how the scheduler uses it.

## Who may touch a block

Only the owning core touches its own block, with interrupts masked. That is why the run queue can
be an `UnsafeCell` with no lock, and why most fields are relaxed atomics: the atomic is interior
mutability through a shared static, not cross-core synchronization. Three things cross cores, and
each says so:

- The migration inbox is a real lock. Another core takes it to hand this core a thread, then sends
  the reschedule SGI. No core ever names another core's run queue.
- The steal request is the one lock-free cross-core protocol in the scheduler. It is the
  `work_steal_slot` crate's type, which loom searches on the host
  ([interleaving.md](interleaving.md)).
- `runq_len`, `inbox_len` and `current` are read by other cores, relaxed and possibly stale. A
  placer accepts a stale load reading on purpose (§28).

`switched_from` names the thread this core just left, finished by the thread it switched to. Two
things must not happen to a thread while a core is still switching off its stack: freeing it, and
queueing a wake that would put it on a second core. Both are recorded before the switch and done
by the successor after it ([intrusive-queues.md](intrusive-queues.md) has the wake half).

## The size is a power of two

`PERCPU[id]` is an index into an array of blocks, so its address is `base + id * size`. A power of
two makes the multiply a shift, and `cpu::current()` and `cpu::id()` inline into a couple of
instructions at every call site, several of them on the IPC fastpath. A `const` assertion fails the
build when the size is not a power of two.

The cost of breaking it was measured. One more `AtomicU64` took the block from 128 bytes to 136 and
added 150 bytes to riscv64's `ipc_send_receive` closure, which turned `script/fastpath-footprint`
red. The field was never read on that path
([benchmarks/preemption-in-the-window.md](benchmarks/preemption-in-the-window.md)). So per-core
state the fastpath does not need goes in an array beside the block, as `cpu::ADOPTED` and
`sched::PREEMPTIONS_PER_CPU` do. The switch count lives in the block because every switch writes
it; the adoption count lives beside it because only a drain does.

On `x86_64` the block carries three words of trap-entry scratch and its own address, and is 160
bytes. It is padded to 256 with `repr(align(256))` rather than exempted from the rule. That took 32
bytes off each of `x86_64`'s IPC closures for 96 bytes of BSS per core.

## Each block on its own 128 bytes

A second assertion holds every block to an alignment of at least `FALSE_SHARING_SPAN`, 128 bytes,
so no two cores' blocks share a cache line. Each core writes `held_rank` twice per lock it takes,
and other cores write a neighbour's `inbox` and `steal_request` when they place work.

The span is 128 on every architecture, not 64. Apple's cores have a 128-byte line, and common
64-byte parts often prefetch lines in adjacent pairs (Intel's spatial prefetcher; recalled, not
measured here). With the blocks packed and 8-aligned, the null syscall's per-trap excess at four
busy cores on an Apple M3 under HVF was 0.041 ticks of a 24 MHz counter, about 1.7 ns on a 33 ns
trap. Aligned, it was within noise of zero
([job-mix/null-syscall-hvf-full-mix.md](job-mix/null-syscall-hvf-full-mix.md)). The block is 128
bytes on aarch64 and riscv64, so the alignment costs no padding there.

## How many seats

`MAX_CPUS` is the number of seats, not the number of cores a machine has. A fixed bound lets the
blocks be a static array, reachable from a core's first Rust instruction before any heap exists.
`smp::read_cpu_list` seats each core at the slot its hart id names, so the bound is on ids, not
counts. It is eight. The VisionFive 2 describes five harts (an S7 monitor and four U74s), and the
mask arithmetic in `cpu_set` and the SBI IPI hart bitmaps is host-tested over exactly eight bits.

A seat costs about 330 bytes of statics whether or not a core fills it, and one 68 KiB secondary
stack slot (64 KiB over a 4 KiB guard page). The number itself is
`current_cpu_contract::CPU_ID_BOUND`, because a user program sizing a per-CPU array needs the same
bound, and the online set is not `0..count` (`crates/cpu_set`). Two binaries agreeing on a number
is AGENTS.md's rule 7.

## Placement randomness

Each block holds a xorshift32 state for §28's two random choices. It is seeded per core per boot
from the core's id, so one boot always makes the same choices and the icount instrument stays
reproducible, while the cores do not sample in lockstep. It is not entropy; there is none before
an OS.
