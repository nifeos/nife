# Does the TSC tick at a constant rate under TCG?

*Name: provisional (`tsc-under-tcg`). A lane's proposal; names are an architect's.*

**Yes, and to sub-100-ppm. The rate is not the problem. The number the boot writes down is.**

Measured 2026-09-21 on patagonia by the `tscdrift` lane, after milestone 524 (the three x86_64 boot
gates: NX, SYSCALL, and the invariant TSC) established that QEMU's TCG refuses to advertise
`CPUID.80000007H:EDX[8]` under any invocation. So no QEMU guest this project runs `x86_64` in
promises that the time-stamp counter keeps a constant rate, and the boot warns rather than
refuses there because the hypervisor bit is set. `kernel/src/arch/x86_64/timer.rs` measures that
rate once at boot and everything time-derived on x86 flows from that one number, and its `BUGS`
section had asserted *"QEMU's TSC is invariant"* with nothing ever having checked.

The headline, in one line each:

- Under plain TCG, the one the test suite runs on, the guest TSC is host monotonic nanoseconds.
  Exactly 1,000,000,000 Hz, measured to within 42 ppm over 32-second windows, and unmoved by host
  load, by a second guest competing for the host, or by a hundredfold change in what the guest
  itself is executing.
- **Under `-icount shift=0,sleep=off`, which is what `script/bench --x86` uses by default, it is not
  a clock at all with respect to real time.** Its rate against an independent clock moved by 37%
  between two workloads in the same boot. That is `-icount` working as designed, and it is a trap
  only for anyone who reads those nanoseconds as wall time.
- The boot's own calibration is the real defect, and it is far worse than anything drift could do.
  Against a TSC that ticks at exactly 1000.000 MHz, twenty-two boots reported between 1001 MHz and
  4330 MHz. The error is unbounded above, one-sided (no boot ever reported low), and grows with
  host load. `bench --x86 --real`, `Instant`, `uptime` and `coremark`'s self-reported rate all read
  that number.

## What our QEMU invocation actually is

`helpers/qemu-runner-x86_64.sh`, which is what `script/test --arch x86_64` reaches through cargo:

```
qemu-system-x86_64 -machine q35 -cpu max -smp 1 -m 256M -display none -serial stdio \
    -no-reboot -device isa-debug-exit,... -device intel-iommu -kernel <elf>
```

No accelerator and no `-icount`. Plain TCG on an aarch64 host, one vCPU by default. QEMU 11.1.1
from Homebrew.

`-icount` does appear in this tree, which matters before reading any x86 timing number: `script/bench --x86` adds `-icount shift=0,sleep=off` unless you pass
`--real`, and `xtask`'s `bench_x86` says why (three consecutive boots produced byte-identical tick
counts on every bench line). `cargo xtask icount`, the instrument of milestone 78 (the load-sensitive
assertions, and the three that measure the wrong thing), refuses `--arch x86_64` outright. So the
suite is one instrument and the bench is another, and they answer this note's question differently.

## The independent clock, and why the obvious one is circular

The TSC cannot validate itself, and neither can the device the boot already calibrates against.

- The 8254 PIT is disqualified twice over. `timer::init_frequency` derives the stored rate from
  it, so agreement would be the calibration agreeing with itself; and in QEMU the i8254 is driven
  from `QEMU_CLOCK_VIRTUAL`, which is the same clock the TSC comes from. It is the same opinion behind a
  different device model.
- The local APIC timer is disqualified for the same reason, and worse. Its own frequency is itself
  measured against the PIT in the same 10 ms window. That is why the `apic timer NN MHz`
  figure on the boot line moves in lockstep with the TSC figure in every transcript below.
- The CMOS RTC is the one that works. QEMU drives it from `rtc_clock`, which is
  `QEMU_CLOCK_HOST`: host wall time, advanced by a host timer rather than by the vCPU. It is the
  only clock in the machine that is independent of the thing under test by construction.
- The host's own wall clock, observed outside QEMU, was used as a cross-check rather than as the
  primary reference: each measurement line was timestamped by the harness on the host as it arrived.
  It agrees (32 reference seconds arrived over 32.9 host seconds, the excess being the harness's own
  per-line cost), which is what rules out the remaining possibility that the RTC and the TSC are
  both wrong together.

**The RTC's resolution is one second, which is coarse, so this does not read it as a time.** It
watches the seconds register for an edge and timestamps the TSC there. The error per edge is
then one poll-loop iteration, and the harness measures its own poll cost and prints it rather than
assuming it: 165 to 1031 TSC counts, i.e. 165 ns to 1.03 us. Over a four-second window that is
0.04 to 0.26 ppm, and over the full thirty-two it is under 0.1 ppm, so edge timestamping is not what
limits any number here.

## The discriminating experiment

The hypothesis to kill first is where TCG gets guest time from. Derived from host wall time, the
rate is constant by construction whatever the guest does; derived from the emulated instruction
stream, it moves with how fast emulation is going. Load on the host is a weak probe of that, because
it changes both sides at once. What separates the two cleanly is changing what the guest
executes while leaving everything else alone.

So the harness alternates windows between two burns with wildly different host cost per guest
instruction:

- **`alu`**: a register-only loop. Many guest instructions, very cheap to emulate.
- **`port`**: `out 0x80` in a loop (the POST diagnostic port, ignored by QEMU and by every real
  chipset). Few guest instructions, each one an exit into device emulation.

The gap between them is about a hundredfold in guest instructions retired per host second, and every
window ends with the same tight RTC poll, so edge detection is identical and only the burn differs.

## The numbers, plain TCG

All on patagonia, QEMU 11.1.1, `q35`, `-cpu max`, `-smp 1`, TCG, no accelerator. "Implied Hz" is the
TSC advance divided by the number of RTC seconds the same window spanned, both read rather than
assumed.

| Condition | RTC seconds | TSC advance | Implied Hz | vs 1.000 GHz | Per-window spread |
|---|---|---|---|---|---|
| Ordinary session load (`uptime` ~7 on 8 cores) | 32 | 32,000,641,000 | 1,000,020,031 | +20 ppm | 1,557 ppm |
| Host deliberately saturated (16 spinners, load 10 -> 29) | 32 | 32,001,342,000 | 1,000,041,937 | +42 ppm | 4,228 ppm |
| Two probe guests at once, guest A | 32 | 31,999,984,000 | 999,999,500 | -0.5 ppm | 4 ppm |
| Two probe guests at once, guest B | 32 | 31,999,972,000 | 999,999,125 | -0.9 ppm | 7 ppm |

Within a single boot, the `alu` and `port` windows agree. From the first row's transcript, in
window order: `alu` 1,000,137,750 / 999,149,250 / 1,000,000,250 / 999,999,750 Hz, and `port`
1,000,706,750 / 1,000,000,000 / 1,000,000,000 / 1,000,166,500 Hz. A hundredfold change in the
guest's emulated instruction rate moves the implied TSC frequency by less than the window-to-window
noise. That is the instruction-derived hypothesis dead.

**The per-window spread is the reference's jitter, not the TSC's.** The spread is 4 ppm
in one boot and 4,228 ppm in another taken minutes apart on the same binary, and the cumulative
figure over thirty-two seconds lands within 42 ppm every time. QEMU's RTC advances its seconds
register on a host timer that a loaded host delivers late, which shortens one window and lengthens
the next; averaging over the whole run is what removes it.

### Why it is exactly a gigahertz, read rather than recalled

The mechanism is in QEMU's source. The x86 TSC under TCG
comes from `cpus_get_elapsed_ticks()` -> `cpu_get_ticks()`, which adds `cpu_get_host_ticks()`, and
`include/qemu/timer.h` (v11.1.0) has no aarch64 case. An ARM64 host falls to the generic arm:

> ```c
> #else
> /* The host CPU doesn't have an easily accessible cycle counter.
>    Just return a monotonically increasing value.  This will be
>    totally wrong, but hopefully better than nothing.  */
> static inline int64_t cpu_get_host_ticks(void)
> {
>     return get_clock();
> }
> #endif
> ```

`get_clock()` is host `CLOCK_MONOTONIC` in nanoseconds. So on this host the guest's TSC *is* the
host's monotonic nanosecond count, which is why it reads 1,000,000,000 Hz and why nothing the guest
or the host does can change its rate: there is no rate to change.

**This is a fact about the host's architecture, not about TCG**, and it is this note's most
important caveat. That header has a case for `x86_64` hosts that reads the host's own `rdtsc`. So
on **xenon** (the x86_64 OptiPlex) or on any x86_64 CI runner, the guest TSC is the host TSC at the
host part's rate, and this note's 1 GHz measures nothing there. What carries across is the method
and the harness, not the number.

## The numbers, under `-icount shift=0,sleep=off`

The same binary, the same host, the flag `script/bench --x86` adds by default:

| Window | Burn | RTC seconds | TSC advance | Implied Hz |
|---|---|---|---|---|
| 0 | `alu` | 5 | 3,312,038,169 | 662,407,633 |
| 1 | `port` | 7 | 3,406,576,195 | 486,653,742 |
| 2 | `alu` | 5 | 3,304,352,152 | 660,870,430 |
| 3 | `port` | 7 | 3,383,959,053 | 483,422,721 |
| 4 | `alu` | 5 | 3,327,656,068 | 665,531,213 |
| 5 | `port` | 7 | 3,402,670,917 | 486,095,845 |
| 6 | `alu` | 5 | 3,362,794,408 | 672,558,881 |
| 7 | `port` | 7 | 3,426,408,993 | 489,486,999 |

Cumulative: 26,926,455,955 counts over 48 reference seconds, 560,967,832 Hz.

The rate against real time depends entirely on what the guest is running, by 37% between the two
burns, reproducibly, in the same boot. And the determinism `-icount` exists for is visible in the
same table from the other side: the work counter read 3704 in every `alu` window and 40496 in
every `port` window, byte-identical, which is the property `bench --x86` gates on.

This is not a bug and nothing here should "fix" it. Under `-icount` the guest's
virtual nanosecond *is* the unit of instruction count, so a bench number derived from it is a
deterministic function of the instruction stream. That is exactly what a regression tripwire wants
and exactly what a wall-clock measurement is not. `notes/benchmarks.md` already says the icount
legs' nanoseconds are not wall time. What this measurement adds is the size: on this path the TSC is
wrong about real time by up to 1.8x, and it is wrong by *different amounts within one run*.

## The finding nobody was looking for: the boot calibration

Every boot line above carries the rate the kernel actually stored. Against a counter now known to
tick at exactly 1000.000 MHz:

Twelve boots at ordinary session load: 1001, 1001, 1002, 1002, 1003, 1003, 1003, 1004, 1004,
1005, 1005, 1042 MHz.

Ten boots with the host deliberately saturated (16 spinners, load average 10 rising to 29):
1003, 1003, 1006, 1422, 1703, 2349, 2364, 2616, 3421, 4330 MHz.

So the stored rate ranges from +0.1% to +333% of the truth, and a single ad-hoc run earlier the
same afternoon caught 1326 MHz on an unloaded machine. Twenty-two boots of twenty-two reported a
rate above the true one and not one reported below it, which is the tell that says what the
mechanism is. `init_frequency` times one 10 ms PIT window by polling, and the poll can only ever
notice the terminal count *late*. The whole of the resulting TSC delta is then attributed to 10 ms. One
host descheduling of the QEMU thread inside that window inflates the answer without bound. A 4330
MHz reading is a 43 ms window reported as a 10 ms one.

`timer.rs`'s `BUGS` already said "a single 10 ms window on a busy host under TCG can be off by a per
cent or so". A per cent is the good case; the distribution has no right tail to speak of. The
same one-shot window sets `APIC_TIMER_HZ`, which is why the APIC figure tracks it in every
transcript, and through that the scheduler's own tick period.

What reads the number: `bench --x86 --real`'s ns/iter, `Instant` and `uptime` through
`counter_frequency_contract`'s page, `coremark`'s self-reported rate, and every `wait_for` deadline
in the x86 suite. Deadlines fail safe (an inflated rate makes a two-second timeout longer in real
time, never shorter); the reported numbers do not.

FIXED, 2026-09-21, the same day, by milestone 571 (the x86 boot calibrates the TSC once, and can be wrong by 4x),
this note's former proposal. The boot now
times several windows and keeps the smallest, because the error being one-sided is what makes the
minimum the right estimator. It stops as soon as two windows agree to one part in a thousand, so the
mean cost is 3.5 windows on a quiet host. The boot line prints the worst window beside the
chosen one, so a calibration the host fought is visible rather than silent.

The numbers in this section are the defect, and they got worse when measured harder. This note
recorded 4330 MHz as the worst of twenty-two boots. Milestone 571's sweep ran 490 boots at three
host loads and saw +1153%, with 56 of 200 boots at load 30 wrong by more than one per cent. The
one-sidedness held without a single exception across all 490. After the fix, 0 of those 200 boots
are wrong by more than one per cent and the worst is +0.47%.

`arch::x86_64::timer`'s `BUGS` section says all of this where a reader meets the feature, including
what it still does not bound.

## What this measurement cannot see

- Drift below about 42 ppm over a 32-second window. That is the largest deviation observed
  across the plain-TCG conditions, and every part of it is consistent with the reference clock's own
  edge jitter rather than with the TSC. So it is an upper bound on TSC drift and not an estimate of
  it. A slower wander (parts per million per hour, say) would need a run of hours and is not
  excluded here.
- Anything about a second core. Every run was `-smp 1`. The probe halts the boot before
  `smp::bring_up_secondaries`, so `NIFE_SMP=4` would have added three vCPUs parked at reset, which
  is not an all-cores-spinning guest and would have measured nothing. Two whole guests running
  concurrently was the substitute, and it is a strictly heavier load on the host than four
  round-robin vCPUs would have been, but it says nothing about whether two cores' TSCs agree with
  each other. `timer.rs`'s second `BUGS` entry already owns that question and says QEMU derives
  every vCPU's TSC from one host clock, which this note's finding corroborates: one host clock is
  exactly what `get_clock()` is.
- **Any host that is not patagonia**, per the QEMU source above. The number is a property of an
  aarch64 host.
- Real silicon. Milestone 87 (the x86_64 bare-metal machine) is where the invariant-TSC bit gets
  read on a part that might actually set it.

This is an `x86_64`-only finding by nature, not a parity gap under §19 (architectural parity is a
tenet; the targets are aarch64, riscv64, and x86_64). aarch64 reads `CNTFRQ_EL0` and riscv64 reads
its rate from the device tree. So neither architecture has a calibration that could be wrong, and
there is nothing for them to be at parity *with*.

## Reproducing it

The harness is in the tree, behind the `tsc_probe` cargo feature, which is off by default and in no
gate. It is `kernel/src/arch/x86_64/tsc_probe.rs`, plus a call site at the end of `x86_64`'s
`arch::timer::init_frequency` that halts the boot when the measurement is done. It is an instrument rather than a
test, the same shape `bench` and `icount` are, because the question it answers takes thirty-five
seconds of wall clock and has no pass/fail. This note is its owner, and the calibration fix
proposed above is what it exists to be measured against.

```sh
cargo build -p kernel --features tsc_probe --target x86_64-unknown-none
./helpers/qemu-bounded.sh 55 ./helpers/qemu-runner-x86_64.sh \
    target/x86_64-unknown-none/debug/kernel            # plain TCG, as the suite runs it
./helpers/qemu-bounded.sh 75 ./helpers/qemu-runner-x86_64.sh \
    target/x86_64-unknown-none/debug/kernel -icount shift=0,sleep=off   # as bench runs it
```

The boot prints one `tscprobe:` line per window and a summary. To vary the host, saturate it with
more spinning processes than it has cores while the run is in flight, or start a second guest from
the same command.

The calibration sweep, the finding that matters most, needs no harness: boot any x86 kernel and read the `clocks      :` line, twenty times, once with
the host loaded.

# BUGS

- **The reference clock's jitter, not the harness, sets the floor**, and that floor moved by three
  orders of magnitude between runs taken minutes apart (4 ppm to 4,228 ppm per window). Everything
  here is therefore stated cumulatively over 32 or 48 seconds, and a short-window number from this
  harness means very little on its own.
- The harness produced two confident wrong answers before a right one, and both look like results. Assuming a window's length instead of reading it off
  the reference reported a 1.25 GHz TSC that does not exist, exactly the ratio of the assumed count
  to the real one. Subtracting raw CMOS bytes without decoding BCD reported four-second windows as
  ten seconds, because the register steps 0x09 -> 0x10 across every decade. Both are in the commit
  message that added the instrument, and in this note's own account of the method above.
- **`helpers/qemu-bounded.sh`'s killer did not fire for this lane**, repeatedly: a bounded run whose
  output consumer exited early left `qemu-system-x86_64` orphaned to `launchd` past its bound, five
  times in one afternoon, each found by `pgrep` rather than by the wrapper. Every one was cleaned up
  by hand. This note is not where that gets fixed; see the lane's report and
  `fix/qemu-bounded-stdin-under-dash`, which is a different symptom of the same script.
