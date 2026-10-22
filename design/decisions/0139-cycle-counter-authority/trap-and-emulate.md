# §139 appendix: what trap-and-emulate actually costs, and it is worse than the syscall it was meant to beat

Evidence for [§139 (who may read the cycle counter, and by what
authority)](../0139-cycle-counter-authority.md), the measurement that took trap-and-emulate off the
`x86_64` row. The main page is complete without this one, which exists to verify or challenge it.

Xen's "15-20 times slower than the rdtsc instruction when executed natively" is Xen's number on
Xen's path, and [the clock-sources appendix](x86-clock-sources.md#what-a-virtualized-guest-does-when-the-host-denies-the-tsc)
quotes it because it is the industry's answer rather than because
it is ours. A nife trap would not be a VM exit on hardware built to make VM exits cheap. It would be
`#GP` to the IDT, a handler, a read of the faulting instruction out of user memory, a decode, an
emulate, an `RIP` advance and an `iret`. calef asked for our number instead of Xen's, so a research
lane measured it on 2026-09-02. It came back at 1,667 ns, which is four times the syscall this
option existed to avoid.

## The machine and the method

cordoba: Intel Core i5-4670 (Haswell, 4 cores, no SMT, 3.4 GHz nominal, TSC rate measured at
3.3984 GHz and invariant per `constant_tsc`), Ubuntu, `Linux cordoba 7.0.0-30-generic`, PTI and
retpolines both on per `/sys/devices/system/cpu/vulnerabilities/`. The machine is the family's live
backup server and carried a load average of about 3.8 throughout, which is stated because it is the
main reason to distrust the absolute figures and not the ratios.

A throwaway C program, pinned to one CPU, times five things over 200,000 iterations each, eleven
repetitions, median reported. Linux can set `CR4.TSD` for one task: `prctl(PR_SET_TSC,
PR_TSC_SIGSEGV)` makes `rdtsc` fault. The program installs a `SIGSEGV` handler, checks that the two
bytes at the faulting `RIP` are `0F 31`, writes a value into the saved `RAX` and `RDX`, advances the
saved `RIP` by two, and returns. That is a real trap, decode, emulate and resume round trip on real
silicon. It counted 2,200,000 traps against 2,200,000 expected, so every read in the loop went
through the handler and none was skipped. The timing calls sit outside the window where `TSD` is
set, so the vDSO's own `rdtsc` never faults into the same handler and never contaminates the result.

| | median | across three runs | in TSC cycles | against native |
|---|---|---|---|---|
| native `rdtsc` | 10.0 ns | 9.3, 10.0, 30.4 | 34 | 1x |
| `clock_gettime` via the vDSO | 24.3 ns | 22.4, 24.3, 24.9 | 82 | 2.4x |
| `syscall(SYS_getpid)`, a bare ring transition | 370 ns | 370.3, 370.3, 373.3 | 1,258 | 37x |
| `clock_gettime` forced through `syscall()` | 406 ns | 407.5, 405.9, 415.1 | 1,379 | 41x |
| a minor page fault, handled in-kernel, no signal | 1,219 ns | 1,227, 1,212, 1,219 | 4,143 | 122x |
| **`rdtsc` trapped and emulated** | **1,667 ns** | 1,653.6, 1,667.5, 1,668.2 | **5,665** | **166x** |

Run-to-run variance on the trapped figure is under 1%, and within a run the eleven repetitions span
1,638 to 1,692 ns. This is not a noisy measurement; the one visibly load-perturbed row is native
`rdtsc`, whose third run of 30.4 ns is an outlier against a min of 20.2 in that run and 9.25 in the
first.

## Two reference points in the brief did not reproduce, and one of them cannot be right

This lane was pointed at a native `rdtsc` of 0.25 ns and a `clock_gettime` syscall of
596.9 ns, both measured on cordoba earlier in the same session. Neither reproduced, and the
first is worth flagging rather than averaging away: 0.25 ns is under one core cycle at this
machine's 3.4 GHz, and `rdtsc` on Haswell has a documented latency in the low tens of cycles. No
single `rdtsc` can retire in a quarter of a nanosecond. The 10.0 ns measured here is 34 TSC cycles
and is what a back-to-back `rdtsc` throughput loop should look like on this part. The 0.25 ns figure
is most likely a loop the compiler hoisted or an amortization over something other than the reads.
The syscall figure is the same order as the 406 ns measured here and the difference is plausibly
load and frequency, so nothing turns on it.

The ratios are the durable quantity and they do not depend on which absolute number is right.
The trap costs 4.1x a `clock_gettime` syscall and 4.5x a bare ring transition, measured in
the same program on the same core within the same second.

## What this is an upper bound on, and what it is a lower bound on

Upper bound, and it is a generous one. Linux delivers this as a signal. The path is fault into
the kernel, build a signal frame on the user stack, return to ring 3 at the handler, run the
handler, `rt_sigreturn` back into the kernel, restore. That is two ring round trips plus frame
construction and teardown, where a nife `#GP` arm would be one. A nife handler would also not be
running with PTI's address-space switch on every transition. So 1,667 ns is not what nife would pay.

Lower bound, and this is the row that matters. The minor page fault at 1,219 ns is an
in-kernel fault handler with no signal frame at all: `#PF` to the IDT, Linux allocates and zeroes a
page, updates the page tables, shoots the TLB entry, `iret`. One ring round trip, on this silicon,
with these mitigations. It does considerably more work than an `rdtsc` emulate would, so it is not a
substitute for measuring the real thing, and the method here cannot separate the entry and exit cost
from the page work. What it does establish is that **a fault round trip on this machine is in the
same neighborhood as a syscall and above it, not below it**, which is the architectural fact the
option was implicitly betting against. `SYSCALL`/`SYSRET` is the path Intel built to be fast; an
IDT-vectored exception returning through `iret`, a serializing instruction, is the path it did not.

Put together, a nife trap-and-emulate would land somewhere between the 406 ns syscall and the
1,667 ns measured here, and it would land there because it is a ring round trip plus extra work.
It cannot be cheaper than a syscall on the same machine, because a syscall is a ring round trip and
nothing more. Xen escapes this only because its comparison is against a guest syscall into a guest
kernel that then does more work still, on parts with hardware-accelerated exits. Our comparison is
against our own syscall, and that is a fight trap-and-emulate cannot win.

## The two thresholds the brief named, answered

The brief set them out in advance so the number could not be read after the fact. "Near 150 ns"
would have meant the performance objection evaporates. "Near or above 1 microsecond" would have
meant the option is worse than the syscall it was meant to beat. The measurement is 1,667 ns on
the heaviest honest path and 1,219 ns for the lightest in-kernel analogue available. Both are above
the microsecond. Trap-and-emulate fails on its own terms, and it fails on the one axis it was
proposed to win on.

## How often `now()` is actually called, since the brief asked

Every ungranted `now()` would pay this, so the count is part of the price. Measured by grep in the
merged tree on 2026-09-02: 42 direct call sites of `user_rt::now`, plus 18 of
`user_rt::monotonic_nanos`, which is `now()` with a divide. The `Instant::now()` sites in
`crates/credentialer` and `crates/serial_console` reach the same counter through `std`.

Most of them are `let start = now(); ...; now().wrapping_sub(start)` in benchmark and test code and
would not care. Four shapes would.

1. `components/src/net_stack.rs:97`, smoltcp's clock. `service_until` at `:437` calls `instant()`
   twice per iteration of its poll loop, once to poll and once to check its own 15 second bound, and
   there are eight `iface.poll(instant(), ...)` sites in that file. Every packet the network stack
   services costs at least two counter reads, and at 1.7 microseconds each that is a per-packet tax
   on the one server in this tree with a throughput number.
2. `components/src/watch.rs:171`, `while monotonic_nanos() < deadline { yield_now(); }`. A spin-yield
   standing in for the timed wait this kernel does not have, one counter read per iteration. That
   program is gone: milestone 281 (`watch` holds exactly what `ps` holds, so it is nothing) deleted
   it on 2026-09-13 (noted here 2026-10-11 UTC).
3. `poll_gap` in `components/src/network_time_client.rs`, the retry gap, the identical shape. The
   spin is only the fallback: the client first sleeps on a timer from milestone 106 (a wait that ends on
   either the interrupt or the deadline), and spins only when its untyped cannot pay for one.
4. `fixtures/src/login_test_client.rs:573` to `:580`, a polling loop reading the counter up to three
   times per iteration.

The spin loops are the least alarming of the four despite looking the worst, because a loop whose
job is to wait does not mind that each iteration got slower. The network stack is the real one: it
is not waiting, it is working, and the counter read is on the path of every poll.

## What the number does not settle

It disposes of the performance argument and nothing else. The other three objections stand exactly
where they were and should not be read as answered by a measurement that never touched them.

- **A `#GP` handler that decodes instructions is a category this kernel has not entered.** Reading
  the faulting instruction out of user memory and interpreting it is a new kind of thing for this
  kernel to do. It has a new fault surface of its own (an unmapped page under `RIP`, a decode that
  must be exactly right about the two-byte opcode and about `rdtscp` and `rdpmc` being separate
  instructions the same `CR4` bit closes).
- **The x86 port has never booted on metal.** Nothing here was measured on xenon, the Dell OptiPlex,
  which does not exist as a running nife host. Every figure in this section is Linux on cordoba,
  which prices the mechanisms and not the port.
- It says nothing about whether the counter should be closed on `x86_64` at all. That is the
  confinement position, it is still calef's, and the argument for leaving `rdtsc` ambient there was
  never a performance argument.

## This was a spike and it was thrown away

Two C programs on cordoba, in `/tmp`, and nothing else. No `CR4.TSD` change, no fault handler and
no decode was written for nife, on any branch. The nife-side spike suggested in the brief was not
run. An x86 nife boot is young enough that
milestone 165 (x86_64 PCI enumeration: wire `kernel/src/pci.rs` to ACPI's MCFG) found on the same
day that its ACPI walk was bounded at 1 GiB. So no machine with real RAM could find an ACPI table, and QEMU could have shown the
instruction path but not a cycle count, which is the part that decides this. The Linux measurement
answers the question at both ends of the bracket without it.

## What this does to the `x86_64` row

It removes an option rather than choosing one. The recommendation in [the scope note](x86-clock-sources.md#what-this-makes-the-honest-scope-note-for-the-x86_64-row) is unchanged and this
lane did not change it. It names trap-and-emulate and a tick-resolution page as the two realistic
orders for the x86 row, and says `x86_64` keeping `rdtsc` ambient with a recorded reason remains the
right state until one is chosen. What is new is that the first of those two is now priced and
loses. A published page at tick resolution, which is DECISIONS §43 (reading the clock is a page)
one axis over, is the only remaining candidate for giving `x86_64` a second time source. It is a
coarse clock and cannot be anything else, for the arithmetic reason in [the scope note](x86-clock-sources.md#what-this-makes-the-honest-scope-note-for-the-x86_64-row)'s
third clause.

So the honest reading is that the `x86_64` row's realistic choice narrowed from three to two:
ambient `rdtsc` with a recorded confinement position, or a coarse published page plus a closed TSC.
Trap-and-emulate is off the list on the evidence rather than on taste.

## BUGS

- **The trap-and-emulate measurement is Linux's signal path, not a kernel-internal fault handler.**
  1,667 ns includes building and tearing down a signal frame and two ring round trips where a nife
  `#GP` arm would have one. The minor-page-fault row at 1,219 ns is the closest in-kernel analogue
  available and it does more work than an `rdtsc` emulate. So the method brackets a nife trap
  between the 406 ns syscall and the 1,667 ns measured, and does not pin it. Pinning it needs a nife
  spike, which was not run.
- **The native `rdtsc` and syscall reference points from earlier in the same session did not
  reproduce**, and the 0.25 ns one is arithmetically impossible for a single `rdtsc` on a 3.4 GHz
  part. The figures in the measurement section are this lane's own and were taken in one program on
  one core. The earlier ones are left in the paragraphs that cite them rather than silently
  rewritten, and this note is what reconciles them.
- **The `now()` census is a grep, not a profile.** 42 direct `user_rt::now` sites and 18
  `monotonic_nanos` sites is a count of source locations. Nobody has counted dynamic calls per second
  on a busy path, so "two counter reads per network poll" is read off the code and not observed.
