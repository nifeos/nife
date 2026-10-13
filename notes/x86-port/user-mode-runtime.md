# The x86_64 port: `user_mode_runtime` and the cycle counter

*An appendix to [`notes/x86-port.md`](../x86-port.md), which is the page to read. This file holds
the ring-3 syscall stubs, why `rdtsc` is ambient here, the `CR4.PCE` door, and `cntfrq()`. It exists
to verify or challenge the main page. A reader who only needs to build, boot or test the x86_64 port
should not have to open it. The directory `notes/x86-port/` and this file's stem are provisional
names; naming is calef's.*

*Records cited below: §121 (a device with no page) and §124 (the x86_64 syscall ABI).*

## `user_mode_runtime` needed five transliterations and two decisions

The five are mechanical once DECISIONS §124 is read: `syscall` where aarch64 writes `svc` and
RISC-V writes `ecall`, the number in `rax`, the arguments in `rdi`/`rsi`/`rdx`/`r10`/`r8`/`r9`.
Three facts at those sites have no counterpart on either other architecture and all three are the
instruction rather than a choice. `syscall` clobbers `rcx` (the return address) and `r11` (the
caller's `RFLAGS`) unconditionally, so every site declares them. `r10` carries the fourth argument
because `syscall` has already taken `rcx`. And `syscall` pushes nothing. That is why the kernel's
entry path parks `rsp` by hand and why `options(nostack)` is honest here.

The two that are not transliterations are `now()` and `cntfrq()`. aarch64 reads `CNTVCT_EL0`
and `CNTFRQ_EL0`; RISC-V reads the `time` CSR and hardcodes the rate because nothing tells
userspace. x86 has neither register.

`now()` is `rdtsc`, and ring 3 may read it because `CR4.TSD` is clear at reset and this kernel does
not change it. That is the same shape as aarch64 needing `CNTKCTL_EL1.EL0VCTEN` and RISC-V needing
`scounteren.TM`, with the difference that here the permissive state is the default and the kernel
would have to act to *close* it. One trap: `rdtsc` answers in `edx:eax`, so reading it into a single
`out(reg)` compiles and silently returns a counter that wraps every four seconds.

### The cycle counter is ambient here, and that was inherited rather than chosen

Recorded 2026-09-02 by milestone 228 (the cycle counters are closed by assumption, and on two
architectures the assumption is a comment). It closed the equivalent door on the other two
architectures and deliberately left this one open.

The paragraph above says the permissive state is x86's default. What it does not say is that on the
other two architectures the register userspace gets and the register a profiler would want are
different registers, and here they are the same one:

| | what userspace gets | what stays shut | how |
|---|---|---|---|
| aarch64 | `CNTVCT_EL0`, ~62.5 MHz under QEMU | `PMCCNTR_EL0`, the cycle counter | `CNTKCTL_EL1.EL0VCTEN` set, `PMUSERENR_EL0` written to zero |
| riscv64 | the `time` CSR, 10 MHz under QEMU | the `cycle` and `instret` CSRs | `scounteren` written to exactly `TM` |
| `x86_64` | the TSC, via `rdtsc` | the performance counters, via `rdpmc` | `CR4.TSD` left clear, `CR4.PCE` established clear |

So every ring-3 program on this architecture holds a sub-nanosecond instrument, roughly two orders of
magnitude finer than what the other two hand out, and no line of this kernel decided that. It is the
reset value. Milestone 228 says so out loud rather than implying that writing the other two registers
made three architectures agree.

### There were two doors, and the second one did close

`CR4.PCE` is bit 8 and it is not the same bit as `TSD`. It gates `rdpmc`, which reads a performance
counter by index, and fixed counter 2 (`CPU_CLK_UNHALTED.REF_TSC`) runs at the TSC rate, so an open
`PCE` is a second path to a cycle-rate instrument reached by a different instruction. Nobody had
looked when milestone 228 was minted. That is why its block first named only two architectures as
fixable; a research lane reading the ISA for a *coarse* clock found it.

`arch::init` now establishes it clear, per core, beside the GDT and the IDT, which is the same defect
fix the other two architectures got. It cost nothing, as forecast. Nothing in this tree reads a
performance counter from ring 3, and nothing under `arch/x86_64/` programs a perf MSR at all. So with
the counters unprogrammed an open `PCE` would have exposed zeros or firmware's leftovers rather than
anything useful. Had it broken something, that would have been the finding, because it would have
meant this tree already depended on a counter it never granted itself.

It reads `CR4` back rather than trusting the reset value, which is the habit this whole milestone
exists to install, and doing so paid immediately. A temporary probe on 2026-09-02 printed 0x20 on
the PVH boot, `PAE` alone, which is exactly what `boot.s` sets, and 0x668 under OVMF: `DE`,
`PAE`, `MCE`, `OSFXSR` and `OSXMMEXCPT`. Bit 8 was clear in both, so nothing was actually closed. But
five bits this kernel never wrote were already set by firmware before any of our code ran, on the one
"firmware" this port has ever booted under. That is the argument for the read in one number. On
xenon, the OptiPlex, real firmware runs first and the value is unknown.

Why it was not closed with them. `CR4.TSD` is one instruction away, and setting it today would
break `Instant`, `thread::sleep`, the random seed, smoltcp's timestamps in `std_net` and the
benchmark harness simultaneously. That is because `user_mode_runtime`'s `now()` on this architecture is `rdtsc` and
there is no coarse alternative to fall back to. Closing it needs a second time source first: a coarse
monotonic value published in a page, the same move DECISIONS §43 (reading the clock is a page) already
made for the wall clock, one axis over. Nothing proposes building that here; it is named so this row
is a limitation with a price rather than an exception with no plan.

What it means for the open decision. `x86_64` has already answered milestone 75 (who may read the
cycle counter, and by what authority) with "everyone, always", by inheritance. Whichever way that
decision goes for aarch64 and riscv64, this architecture will not match it until the page above
exists. §19 (architectural parity is a tenet) should read that as a scope note rather than as a
gap somebody forgot. Linux names the same asymmetry from the other side: its arm64 per-task
`PMUSERENR_EL0` work opens the counter only on request, explicitly to avoid "the information leaks
x86 has".

`cntfrq()` is RISC-V's gap, one architecture worse, and it is a constant with a `BUGS` section
rather than a number with a comment. There is no architected TSC rate at all: `CPUID` leaf 0x15
gives a ratio to a crystal that leaf 0x16 may not report, and neither leaf exists on every part. So
the kernel does not read the rate, it measures it against the PIT (`timer::init_frequency`) and
stores it in `TSC_HZ`. A ring-3 program cannot repeat that measurement, and should not be able to.
The PIT is at ports 0x40..0x43, `IOPL` is 0 and the TSS's permission bitmap is empty, so an `in`
from a process is a #GP. That is §121 not being decided yet rather than an oversight to route
around. The constant returned is QEMU's 1 GHz, which the kernel measures as 1001 MHz. On milestone
87's real Dell it will be the CPU's base frequency and this will be wrong with no way for a caller
to tell. Both architectures want the same fix: hand the frequency to a process at start, the
way Linux passes `AT_HWCAP` in the aux vector, so the one component that measured it is the one
that reports it.
