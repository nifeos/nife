# A tick not yet raised: 2026-09-24

*(An appendix of [notes/load-sensitive-assertions.md](../load-sensitive-assertions.md), which holds
the register and the rules. Written by the `fix/riscv-cpu-matrix-flake` lane, pull request #1212.)*

## The red

Pull request #1195 changed only `design/decisions/` and `script/decisions`. Its `cpu matrix` job
still went red on `rv64`:

```
test kernel::preemption_window_tests::unmasking_delivers_the_tick_that_was_held ...
[PANIC] panicked at kernel/src/preemption_window_tests.rs:91:5:
no preemption was taken in the tick period after unmasking, so the ticks held across the mask
were dropped rather than deferred; ...
host load (riscv64): 1-minute average 1.96 / 2.04 / 2.13 ... on 4 cores
  Not oversubscribed, ...
```

The scanout and inbound failures after it are consequences. The guest died at the panic, so it never
drew the screens or opened the listeners.

## The base rate

This lane read every `ci.yml` run from 2026-09-14 to 2026-09-24 that was not green on its first
attempt. That was about 2,000 runs. Each failing job's log was classified by its panic line. For
`cpu matrix`:

| cause | failures | models | verdict |
|---|---|---|---|
| `every_character_survives` in `documentation/tests/render.rs` | 71 | all five | a break on `main` (`notes/rented-metal.md`), 09-21 to 09-24 02:00, gone since |
| `the_page_is_returned_when_the_space_is_dropped` in `user/current_cpu_tests.rs` | 7 | four of five | the global frame count; fixed by #1120 |
| `inbound check (riscv64)`, 2 of 4 served | 4, plus 1 in `build + test` | three | host-to-guest networking; none since 09-21 23:16 |
| compile errors, two other panics | 6 | all five | a branch's own doing |
| `a_masked_window_takes_no_preemption` in `preemption_window_tests.rs` | 1 | `rv64` | this appendix |
| `holding_a_lock_masks_the_timer` in `arch/riscv64/timer.rs`, "interrupts did not resume" | 1 | `sifive-u54` | the same cause, this appendix |

The test landed on 2026-09-21. Since then the matrix actually ran 331 times, five models each. That
is one failure in about 1,650 model boots, and none in `build + test`. At that rate, rerunning the
suite to reproduce it was never going to work.

## The cause

The test masked interrupts and spun three tick periods. It then unmasked, spun one more period, and
asserted that the core's preemption count had moved. That assumes a tick is pending by the end of a
30 ms masked spin, and nothing guarantees it. QEMU raises the timer from its own main loop, not from
the spinning vCPU. `rdtime` is host time, so the spin can end before the host raises the deadline.

A throwaway test measured it on the laptop at a load of about 9 on 8 cores. Each window masked
interrupts, read the hart's armed deadline and spun until `sip.STIP` was set. It then unmasked and
read the preemption count at once. There were 2,000 windows per model:

| model | < 0.2 ms | < 1 ms | < 5 ms | < 10 ms | < 30 ms | ≥ 30 ms | worst | taken at the unmask |
|---|---|---|---|---|---|---|---|---|
| `rv64` (Sstc) | 60 | 419 | 1,509 | 11 | 1 | 0 | 18 ms | 2,000 of 2,000 |
| `sifive-u54` (SBI, `mtimecmp`) | 13 | 319 | 1,458 | 107 | 89 | 14 | 86 ms | 2,000 of 2,000 |

The host is often late: a few milliseconds routinely, and tens of milliseconds in the tail. The
kernel never was. Once the pending bit was set, all 4,000 ticks were taken at the unmask, before the
next read. So the likely reading of the red is a tick not yet raised. The log cannot prove that,
because the test never looked, and that was the defect.

The direction agrees. A "not yet" failure is load sensitivity by the page's first diagnostic. The
"not oversubscribed" line does not contradict it, because the delay lives in one QEMU thread's
scheduling and a load average cannot see that.

## The fix

Each architecture now answers `arch::timer::is_tick_pending()`, provisionally `tick_pending` when
this fix landed and ratified under the predicate rule on 2026-09-24. It reads `sip.STIP`
on riscv64, `CNTV_CTL_EL0.ISTATUS` on aarch64, and the timer vector's IRR bit on x86_64. Both tests
in `kernel/src/preemption_window_tests.rs` spin three masked periods. They then keep spinning, still
masked, until the tick is pending. The bound is a second (`RAISE_BOUND_PERIODS`), a leak trap
against a worst case under 90 ms.

- `unmasking_delivers_the_tick_that_was_held` unmasks only once the tick is waiting. Delivery then
  takes a few instructions, and the host is out of it. The core is now read under the mask.
- `a_masked_window_takes_no_preemption` gets stronger. A dead timer used to pass it, since a window
  that held nothing took no preemption. It now requires a raised tick that still did not land.

`holding_a_lock_masks_the_timer` (riscv64 and aarch64) took the same fix. It now waits for the
pending bit inside the lock and asserts again that nothing landed. Only then does it release. Its
opening liveness check and `the_timer_is_ticking` now wait on the property under the same bound.
The liveness check's aarch64 copy is the red that [unowned reds](unowned-reds.md) left open.

## The proof

Three injections ran on riscv64 as filtered runs, and each was reverted:

| injection | old test | new test |
|---|---|---|
| deadline re-armed 60 ms out after the mask (a late host) | fails, with CI's exact message | passes |
| held tick cleared at the unmask (the defect under test) | | fails, "no preemption followed" |
| deadline re-armed 10 s out (a timer that never fires) | passes, vacuously | fails, "never raised" |

Then the old and new delivery sequences alternated in one boot. The last two rows ran the QEMU
process at background priority (`taskpolicy -b`). That starves the emulator's own threads without
loading anyone else's.

| host | model | old form fails | new form fails |
|---|---|---|---|
| laptop, load about 5 on 8 cores | `sifive-u54` | 0 of 1,000 | 0 of 1,000 |
| laptop, load about 5 on 8 cores | `rv64` | 0 of 1,000 | 0 of 1,000 |
| QEMU at background priority | `sifive-u54` | 228 of 300 | 0 of 300 |
| QEMU at background priority | `rv64` | 214 of 300 | 0 of 300 |

The first two rows show why the red was rare. The last two show what it was.

In CI on #1212, the dispatched `ci.yml` run was green on every job. The `cpu matrix` job ran three
times in all, so 15 model boots, all green. Further reruns stopped while runners were starved. No
feasible number of reruns could show a rate of 1 in 1,650 falling; the table above does that.

## BUGS

The pending bit is only as good as the emulator's model of it. On QEMU's riscv64 one timer callback
sets both the bit and the interrupt line, and 4,000 windows found them in step. On silicon it is the
architecture's own state. Nothing here ran on radon or argon.

`a_long_critical_section_costs_a_tick` was not changed. A late host only makes its miss more
certain. It could still time out against a host that went quiet for 200 ms.

The lateness was measured on one laptop. The CI runner's worst case is unknown, and the one-second
bound is eleven times the worst seen here.
