# The soak supervisor: its constants and its orderings

An appendix to [notes/soak.md](../soak.md). It carries the reasons behind `kernel/src/soak.rs`: why
each constant has its value, and why each step happens where it does. The tick route is
[tick-route.md](tick-route.md), the census is [placement-census.md](placement-census.md), and the
reboot loop's hazard and escapes are [rebooting-soak.md](rebooting-soak.md).

## Where the workload and the detection live

The workload is a user program, `fixtures/src/soaker.rs`, and the detection is in the kernel. The
defect risk 5 produced (a receiver made `Ready` without a delivery) can be caused from userspace
through the real syscall path, so a kernel-mode stress loop would test an artifact of the test. A
user program cannot assert about kernel internals, so the assertions stay in the kernel with the
trace counters.

## The constants

- `BEAT_SECONDS` is 5, against `serial_console`'s fifteen-second default quiet window: three missed
  beats before a run is called a hang. Two would be tight on a board whose console is shared with
  U-Boot's leftovers. Four would take a minute to notice a wedge.
- `WAITERS_PER_GROUP` is 1, and the number does real work. A group's waiters block on one rendezvous
  and a tick wakes one of them, so the waiter count is the number of threads the wake protocol
  chooses between. It wants to be comparable with the core count, so `pick_wake_target` has
  somewhere to put them. One per group is one per core, since `topology_groups` builds a group per
  core, and at least two so a single-core QEMU leg still runs more than one endpoint.
- Each worker's seed is distinct, odd and well spread. The jitter it drives keeps the pairs from
  locking into one interleaving, and two workers given one seed would defeat it for that pair.
- `TICK_ROUTES` holds each rendezvous plus one, as `sched::IRQ_ROUTES` does, because a rendezvous
  name may be zero. They are plain atomics because the reader is an interrupt handler on every core,
  and a lock taken to learn whether there is anything to do is one more thing that can go wrong. The
  boot thread writes each once, after every worker exists. `TICK_CURSOR` is global rather than keyed
  on the core, so every route is covered on a single-core machine, which still has two groups.
- `REBOOT_AFTER_SECONDS` is 120, and both bounds are measured. The floor is when the arrangement is
  knowable: on radon the spawn placement holds for about twenty-five seconds and is then replaced,
  in one drift event, by an arrangement that does not change again. A shorter window records the
  lottery's first draw, not the one the rate belongs to. The ceiling is that fifty boots fit in an
  evening: two minutes plus the twenty-odd seconds a boot takes is about two hours for fifty draws,
  and a hundred is an overnight run. Two minutes buys roughly twenty beats after convergence, so the
  rate is read off several. It is a constant because a board kernel has no configuration path.
- `REBOOT_GRACE_SECONDS` is 5, matching the beat, so a person who reads the announcement has a
  beat's time to act. It is not the main escape: every beat of the window has already asked. It
  serves someone who walks up mid-run and wants the board back without knowing where in the cycle it
  is.
- `WATCHDOG_RESET_AFTER_SECONDS` is 60: twelve beats of margin, so a beat delayed by a loaded
  machine cannot trip it, and short enough that a wedged board is back before a person would walk to
  it. Both watchdogs count twice before they reset, so it is two counts of 30 s: 50 TCO ticks each,
  or 720,000,000 ticks of the JH7110's 24 MHz core clock.
- `WATCHDOG_REPORT_EVERY_BEATS` is 6. Every beat would double the log. The first line proves the
  counter moves, and the rest say it still does.
- `WEDGE_AFTER_SECONDS` is 30, six beats, so the log shows the watchdog petted before it is starved.
- A worker's core in the census is `u8::try_from` rather than `as`. An online cpu id is bounded by
  `MAX_CPUS` and cannot truncate, but a topology that broke that would report `u8::MAX`, the
  census's "no answer", instead of a wrong core number.

## The orderings

- Responders are spawned first. A caller whose responder is not yet receiving parks on the
  rendezvous, so the order is not load-bearing; it keeps a fresh boot's first beats from reading as
  a stall.
- Tick routes are bound before any waiter exists, and the signaling is switched on last.
  [tick-route.md](tick-route.md) has both halves.
- The rebooting soak's questions and the watchdog's pet come after the beat's verdict. A failed beat
  panics, and a panic diverges, so a run that found something never resets over its own evidence;
  the watchdog resets a failed soak only once its dump is on the console. The whole point of fifty
  boots is the one that fails.
- In each beat the escape is asked before the deadline, so a keypress in the beat the window ends
  stops the loop rather than racing it. The data-ready bit is sticky, so a poll every beat cannot
  miss a byte.
- Only a soak that does not reboot drains the console at the watchdog. A rebooting soak drained at
  its own banner, which is where `script/board-console --stop` sends the escape, and a second drain
  would throw that byte away so the series never stopped.
- On a failure the census prints before the thread dump, and the dump before the panic, because the
  panic handler prints none and a failure with no path into the wedge shows only its end state.
- The verdicts are tried in this order: a finding about the kernel (the wake gate refused), then
  about delivery (a mismatch), then about liveness (a stall). The first beat is exempt from the
  stall check only: a worker not yet scheduled has not started.
- A worker's progress is its two counters summed. A waiter leaves `rounds` at zero and the other
  roles leave `wakes` at zero, and neither counter falls, so an unchanged sum means both were
  unchanged, with no table of who plays which part.
- `drifted=` is measured against the last census printed, not against spawn. Against spawn it
  becomes a constant a few beats in (11 of 20 on the aarch64 QEMU leg) and says nothing further.
  Against the census it returns to zero after each reprint, so a nonzero one means a newer block
  follows.

## Which builds reset the machine

The rebooting soak builds on all three architectures. Each has a real reset behind `arch::reboot`
(SBI SRST, PSCI `SYSTEM_RESET`, the FADT reset register and the two legacy ports) and a real escape
behind `console::is_byte_waiting` (the NS16550's and PL011's sticky data-ready bits).
`script/soak-test --reboot` proves under QEMU, per architecture, that the machine came back rather
than that the call returned. A build that compiled and quietly never rebooted would look like a
board that drew the same placement fifty times.

The watchdog soak drives the Intel TCO on x86_64 and the JH7110's watchdog on riscv64. aarch64 has
none, and a `watchdog_soak_test` build for it is a compile error, because a build that quietly never
armed would look like a board that never wedged. Every x86_64 and riscv64 soak arms the watchdog
unless it names `unwatched_soak_test`: "A soak should reset itself when it freezes" (calef,
2026-09-26 UTC). `kernel/build.rs` sets the `soak_watchdog` cfg, and `soak.rs` restates the rule as
a compile error, so a build-script edit that drops the default fails the build.

Before it arms, the JH7110 path refuses twice where the TCO needs neither. The device tree must name
the watchdog and its clocks. The counter must run at the rate its count was computed for, measured
against the kernel's timer over a tenth of a second, spun so the window is the timer's. A
counter on a gated clock reads a frozen count, and arming it would print `ARMED` over a timer that
never fires.

## BUGS

- The round-trip total is sampled while workers write it (`soak_page`'s `BUGS`).
- A panicked worker is reported "stalled", not "died", and is not restarted.
- The tick route skips the interrupt controller's claim, mask and complete, so a run with it on is a
  different soak, comparable only with the same build.
- `REBOOT_AFTER_SECONDS` is one draw of the placement lottery, and many draws do not replace one
  long run.
