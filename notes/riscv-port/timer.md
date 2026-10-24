# The riscv64 timer, and the counter permissions it sets

*Name: provisional, minted 2026-10-23 (UTC) by milestone 929 (comments in the Linux style)'s riscv64
lane.*

An appendix to [notes/riscv-port.md](../riscv-port.md). It carries the design behind
`kernel/src/arch/riscv64/timer.rs`, the S-mode analog of aarch64's generic virtual timer.

## SBI TIME, and a deadline kept in software

The free-running counter is the `time` CSR, read with `rdtime`. Each tick is armed through the SBI
TIME extension: `set_timer` is an `ecall` to OpenSBI in M-mode, and it both arms the next S-mode
timer interrupt and clears the pending one. The Sstc extension's `stimecmp` CSR would skip the
M-mode round trip. SBI TIME works on every OpenSBI, so it is the portable choice.

aarch64 re-arms from `CNTV_CVAL_EL0`, an absolute deadline it can read back from the hardware, so
`next = CVAL + interval` keeps its deadlines on a fixed grid. SBI `set_timer` is write-only: it
takes an absolute `time` value and returns nothing. So the grid lives in software, in the per-hart
`DEADLINE` array, anchored when `init` arms the hart's first deadline.

`rearm` computes `deadline + interval`, never `now + interval`. `now()` is read after the trap
entry and the SBI round trip, so re-arming from it makes every period `interval + latency`. The
lateness compounds, and the delivered rate falls below the configured one. On the grid a slow
handler makes one tick late and leaves the rest where they were.

When the next deadline is already past, arming it would fire at once, again and again. `rearm`
gives that tick up instead: it counts it in `MISSED_TICKS` and re-anchors the grid at
`now + interval`. A nonzero miss count means a whole period elapsed inside the handler or inside a
critical section.

In test builds `miss_detail` keeps the last miss per hart, as the counter value and the deadline it
had passed, plus a count. Late by less than one interval is a slow handler, which is this kernel's
bug. Late by a whole interval or more is the emulator having been descheduled. From inside the
guest the two look the same without those numbers. One record is enough to tell them apart, and it
is cheaper in trap context than a ring buffer.

`TICKS`, `MISSED_TICKS` and `DEADLINE` are all per hart. Masking `sstatus.SIE` masks one hart's
timer, so a single shared count would advance on the other harts' ticks while this one held a
lock (DECISIONS §11 (SMP: per-CPU run queues)).

## The counter rate comes from the device tree

RISC-V has no `CNTFRQ_EL0`. The architected answer is the device tree's
`/cpus/timebase-frequency`, which the binding requires. `init_frequency` reads it once, on the boot
hart, before `init` arms the first deadline. `crates/machine_discovery`'s `cpu_list` reads `/cpus`
first and falls back to the first hart's own property.

It is a separate call from `init` because `init` also runs on every secondary, which has no device
tree pointer. The counter is machine-wide, so the boot hart's reading serves every hart.

`TIMEBASE_HZ` is zero until that read, and the readers assert it nonzero. A boot that never read
the machine panics, rather than running quietly at QEMU's 10 MHz on a board with another rate. A
timer at the wrong rate preempts at the wrong rate and returns from `sleep` at the wrong time, and
nothing reports a fault. `frequency_checked` is the one reader that does not panic: the timebase
page builds one layout for every process and must represent "not read yet".

## What U-mode may read: `scounteren`

`init` writes `scounteren` to exactly `TM`. U-mode `rdtime` is the RISC-V twin of aarch64's
`CNTKCTL_EL1.EL0VCTEN`. `crates/user_mode_runtime`'s `now()` needs it, and through it std's
`Instant`, `thread::sleep` and the random seed.

The write is a whole-register write, not a bit set. QEMU's OpenSBI leaves `TM` permitted, and a
platform's firmware may leave any bit set or clear. A bit set would open `TM` and leave `CY` and
`IR` at whatever firmware chose. After the whole write the register is exactly `TM` on every hart,
whatever OpenSBI or vendor firmware handed over. The write also clears the HPM bits (3 to 31).
Nothing reads those counters, and a zero in this CSR can only remove a U-mode read permission.

`scounteren` is per hart, so the write belongs in the per-hart `init`. A secondary that skipped it
would fault only on the threads that landed there.

Whether U-mode is ever granted `CY` is milestone 75 (who may read the cycle counter, and by what
authority), an architect's decision.

### The cycle-counter grant

`set_cycle_counter_grant` runs on every context switch, beside `mmu::switch_user_root`, with the
same shape: compare first, write only on a change (DECISIONS 139 option 4). It reads `scounteren`
back, where the aarch64 twin caches what it last wrote. `scounteren` always exists in S-mode, while
an `mrs` of `PMUSERENR_EL0` is UNDEFINED on a part without `FEAT_PMUv3`. And this one CSR carries
two policies: `TM` is open for every thread and `CY` is per thread. Changing one bit of the live
value cannot get `TM` wrong, where a cache would have to model it.

`is_cycle_counter_grantable` is always true here, because `scounteren` is mandatory and `cycle` is
one of its named bits. It exists so the context switch and the tests ask the same question on all
three architectures. Tests are its only callers, and they skip rather than fault where no counter
can be granted.

The grant, its constant and its query are built only under `test` or `--features
cycle_counter_grant`. That is a measurement build, like `soak_test`, and `kernel/Cargo.toml`'s
feature block carries the reasoning and the measured cost. The closed default in `init` is built
always.

## Why `tick` is never inlined

`riscv_trap_body` is in `script/fastpath-footprint`'s `syscall_entry` set, because an `ecall`
arrives through the same handler as every other trap. The timer arm of that handler is not
something a syscall fetches. When LLVM folds `tick` in, its bytes land inside `riscv_trap_body` and
the flat entry-set sum counts them. That happened once from an unrelated change in `sched.rs`, and
moved the sum 12.1% (1,870 to 2,096 bytes) against a 5% bound. `#[inline(never)]` keeps the number
measuring what it names, and one `jal` per 10 ms tick costs nothing worth having. The gate's own
limit is milestone 368 (`script/fastpath-footprint`'s entry set is flat).

## The instruction-count bounds

`ARRIVAL_BOUND` and `HANDLER_BOUND` are the ceilings `icount::run` asserts
([notes/instruction-clock.md](../instruction-clock.md)). `DEADLINE` is the kernel's own array, so
reading it back proves only that the kernel remembers what it meant to write. `ARRIVAL_BOUND`,
the instructions from that word to the handler observing the interrupt, is what shows firmware was
armed with it. An implementation that kept `DEADLINE` on the grid but armed SBI from `now()` would
pass every other test and break this bound within a few ticks.

`HANDLER_BOUND` covers the whole handler: the arrival span, the tick bookkeeping, the `DEADLINE`
store and the SBI `ecall`. It is larger than aarch64's because the firmware round trip is inside
the span. A miss is this span exceeding one tick period, 10,000,000 instructions of virtual time at
100 Hz. `rearm` records the instrument after the SBI call for the same reason: the round trip is
the one cost aarch64 does not pay, and stopping short of it would compare different spans.

`calibration_loop` lives here rather than in `icount.rs` because the codebase's first rule puts
every `asm!` under `arch/`. Its `addi`/`bnez` pair may be assembled compressed on `riscv64imac`,
which changes the encoding and not the count, since `-icount` counts instructions retired.

## The tests

The test runner passes no `-icount`, so `rdtime` follows host time, and a host that deschedules the
vCPU coalesces ticks. A test comparing delivered ticks with elapsed counter time therefore measures
the emulator as much as the handler. `ticks_arrive_at_the_configured_rate` asserts the re-arm law
instead: over a window with no recorded miss, the deadline moved by exactly one interval per
delivered tick. Re-arming from `now()` fails that on the first tick. A deschedule long enough to
slip the grid is a counted miss, and the window is retried.

Eight windows with a miss in each is a skip, printed with `miss_detail`'s numbers, not a failure.
Whether those misses are a contended host or a slow handler is the one question a wall clock cannot
answer from inside the guest. Widening the budget changes how often anyone notices, not what is
measured, which DECISIONS §61 (a lint is adopted on evidence) forbids. `script/icount` asserts the
handler's speed with no host term in it: the handler bound, zero missed ticks, and the re-arm law.
CI runs it on every change that is not documentation only. So no test here asserts that the handler
keeps up with no lock held ([notes/load-sensitive-assertions.md](../load-sensitive-assertions.md)).

The waits on a tick being raised or delivered are bounded waits, never fixed spins. QEMU raises the
timer from its main loop, late by as much as 86 ms on `sifive-u54`
([preemption-window-tick.md](../load-sensitive-assertions/preemption-window-tick.md)).

## BUGS

- The RISC-V binding permits a per-hart `timebase-frequency`. The boot hart's reading serves every
  hart, so a machine whose harts genuinely differ is misread.
- `mcounteren.CY` gates `scounteren.CY` from M-mode, where this kernel never runs. If firmware left
  it clear, a granted U-mode `rdcycle` still takes an illegal instruction. Nobody has read
  `mcounteren` on the VisionFive 2's OpenSBI build (radon).
