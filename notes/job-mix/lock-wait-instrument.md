# The lock-wait instrument

*Name: provisional, for this stem, and for the module, feature and `job-mix-lock:` line it
describes. Naming is an architect's.*

`kernel/src/lock_wait.rs`, built only with `--features lock_wait`, which implies `job_mix`. It
answers one question about the job mix: how often its threads found a kernel lock held, and how long
they waited. No other kernel carries a byte of it, and `script/fastpath-footprint` builds neither
feature. [null-syscall-under-load.md](null-syscall-under-load.md) is what it was built to explain
and what it found.

## What it counts

Per sweep point, summed over cores:

- Per lock rank: acquisitions that found the lock held, and the ticks spent spinning for it.
- For the reaper: threads reaped, the ticks spent freeing their kernel stacks with no lock held,
  and the ticks its final `IPC_TABLES` section held the lock to remove them.
- For `sched::current_cap`: every call, and the calls whose one acquisition found its lock held,
  with their wait. Every `invoke` makes exactly one such call, so `contended / calls` is the chance
  the cheapest syscall waits, and `wait_ticks / calls` is what that costs it on average. That is
  directly comparable with the `null_syscall` job's per-trap growth. The lock is the running
  thread's own capability table, so `site=current_cap` means "how often the cheapest syscall waited
  for anything".

Counting runs only while the supervisor has armed it, for each timed subrun, and not during the
untimed breakdown exchange. Each core's counters sit on cache lines of their own, written only by
that core, so the instrument does not add the cross-core traffic it measures.

A rank that `sync::rank` gives to more than one lock (59 is the inbox, the kernel object budget and
the mappings table) is printed by number only. A name would claim an attribution the counter cannot
make.

## What it costs

An acquisition that finds the lock free costs what it always did. `IrqSafeMutex::lock` opens with
`try_lock`, the same compare-and-swap `spin_lock`'s `lock` opens with. `current_cap` pays a flag store, a
counter increment and a flag clear, all on its own core's line. The clock is read only on the
contended path, twice per wait. On radon a `rdtime` may be emulated by firmware (a recollection about
the SiFive U74, not checked against radon's OpenSBI), and reading it on every call would add to the
cheapest syscall the very cost this is trying to find. The reaper reads the clock on every reap,
which is cold: one call per thread exit.

## What it cannot see

- A wait is quantized to the platform timer, 4 MHz on radon or 250 ns a tick, so one wait means
  nothing and only sums over many do. The sums are unbiased because a wait's start is not
  phase-locked to the timer. If `rdtime` is emulated, each recorded wait also carries part of the
  cost of reading the clock.
- An acquisition that finds the lock free but must pull its cache line from another core is not
  counted. That cost is real and rises with busy cores. The residual between the job's growth and
  the counted wait is where it would show.
- The `current_cap` attribution is a per-core flag set around the acquisition. An interrupt that
  arrived after the flag and before the lock masked interrupts, and took a contended lock of its own,
  would be charged to `current_cap`. On the syscall path interrupts are already masked there.
