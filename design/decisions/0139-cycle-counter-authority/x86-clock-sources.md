# §139 appendix: does `x86_64` really have only one user-readable clock?

Evidence for [§139 (who may read the cycle counter, and by what
authority)](../0139-cycle-counter-authority.md), the check behind its ruling that `x86_64` keeps its
ambient counter. The main page is complete without this one, which exists to verify or challenge it.

Finding 2 in [`premise-checks.md`](premise-checks.md) was written from knowledge rather than from reading, and the maintainer said so when
he presented it. A research lane was briefed on 2026-09-02 to check it. The claim holds. There is
no second user-readable time source on `x86_64` of resolution meaningfully better than a kernel
tick, and the ISA routes every user-visible one through the same `CR4.TSD` bit on purpose. What
follows is what was read, in the order a reader would want to check it.

## The instruction set: one bit closes three instructions, not one

`CR4.TSD` is not the gate on `rdtsc` alone. Intel's instruction reference gives the same `#GP(0)`
condition on three user-mode instructions, and the third is the one that would otherwise be a
loophole. All three quotations are from the Intel SDM instruction pages as reproduced at
`felixcloutier.com/x86/`, read 2026-09-02.

- `RDTSC`: "When the flag is clear, the RDTSC instruction can be executed at any privilege level;
  when the flag is set, the instruction can only be executed at privilege level 0", with
  "#GP(0) - If the TSD flag in register CR4 is set and the CPL is greater than 0."
- `RDTSCP`: the same sentence and the same `#GP(0)`.
- `TPAUSE`: "The instruction execution wakes up when the time-stamp counter reaches or exceeds the
  implicit EDX:EAX 64-bit input value", and its exception list reads
  "#GP(0) If src[31:1] != 0. If CR4.TSD = 1 and CPL != 0."

`TPAUSE` is worth naming because it is the attack a reader should think of and Intel already closed
it. It consumes a TSC *deadline* rather than returning a TSC value, and it reports through the carry
flag. So a program that could execute it while `rdtsc` was shut would recover the counter to full
precision by binary search over deadlines in about sixty-four executions. Intel put it behind
`CR4.TSD` anyway. That is a design statement: the bit is meant to close *user-visible TSC reads*,
not one opcode.

Two further instructions come up and neither is a time source.

- `RDPID` reads "the value of the IA32_TSC_AUX MSR (address C0000103H) into the destination
  register". That is a processor identifier, not a counter, so it does not answer this question at
  any resolution.
- `RDMSR`, which would reach the TSC MSR and every APIC register in x2APIC mode, "must be executed
  at privilege level 0 or in real-address mode; otherwise, a general protection exception #GP(0)
  will be generated."

`RDPMC` is the one real second door, and it is a door to close rather than a clock to keep.
Its gate is a *different* `CR4` bit: "When the PCE flag is set, the RDPMC instruction can be executed
at any privilege level; when the flag is clear, the instruction can only be executed at privilege
level 0", with "#GP(0) If the current privilege level is not 0 and the PCE flag in the CR4 register
is clear." Fixed-function counter 2 counts at the TSC rate, so ring 3 with `CR4.PCE` set and the
fixed counters enabled would have a TSC-rate clock without ever executing `rdtsc`. That is not an
alternative for `user_rt::now()`, because it needs the kernel to enable the counters through MSRs
this kernel never writes, and `kernel/src/arch/x86_64/` contains no reference to
`IA32_PERF_GLOBAL_CTRL` or the fixed counters at all. It is a finding for the [*recommended outright*
list](../0139-cycle-counter-authority.md#recommendation). `CR4.PCE` must be established as clear by the same code that establishes `CR4.TSD`, or a
future decision to close the TSC would be closing the front door while the side door is only shut by
a reset value nobody wrote. `boot.s`'s two `CR4` writes are still only `or eax, 1 << 5`.

That finding has a home. Milestone 228 (the cycle counters are closed by assumption) was minted
on 2026-09-02 to do exactly the "close what we claim is closed" work this document recommends, and
its x86 item is scoped to leaving `CR4.TSD` alone and writing the record down. It does not mention
`CR4.PCE`, because nobody had looked. **`RDPMC` is a second user-readable path to a TSC-rate count on
`x86_64`, gated by a different bit that this kernel also never writes**. Establishing it clear
belongs in that milestone beside the aarch64 and riscv64 writes rather than in a lane report.

## The chipset timers: two exist, and neither is usable from ring 3 at a price worth paying

The ACPI power management timer. ACPI 6.5, section 4.8.2.1, read 2026-09-02 at
`uefi.org/specs/ACPI/6.5/04_ACPI_Hardware_Specification.html`:

> The power management timer is a 24-bit or 32-bit fixed rate free running count-up timer that runs
> off a 3.579545 MHz clock.

That is 279.4 ns per tick, which is genuinely finer than a tick and finer than nothing. The problem
is where it lives. The register is `PM_TMR_BLK`, which the FADT normally places in system I/O space,
and an `in` from ring 3 needs either `IOPL >= CPL` or a cleared bit in the TSS I/O permission bitmap:
"#GP(0): If the CPL is greater than (has less privilege) the I/O privilege level (IOPL) and any of
the corresponding I/O permission bits in TSS for the I/O port being accessed is 1." Granting a
confined program port access is a strictly larger authority than granting it a counter read, and it
is per-task state on the context switch exactly like option 4, with a worse blast radius. This is
the reverse of what the decision wants.

The HPET. This is the strongest candidate on the list, and it is the one that has already been
tried and withdrawn. It is memory mapped, so a kernel *can* map its page read-only into a program's
address space with no new instruction and no new port authority. The IA-PC HPET Specification 1.0a
(Intel, 2004), section 2.2's recommendation table, gives "Clock Frequency Fmin = 10 MHz", and the
General Capabilities register's `COUNTER_CLK_PERIOD` field "indicates the period at which the counter
increments in femptoseconds (10^-15 seconds) ... The value in this field must be less than or equal
to 05F5E100h (10^8 femptoseconds = 100 nanoseconds)". So the architectural floor is 100 ns per tick
and the common part runs at 14.31818 MHz, about 70 ns.

Linux mapped the HPET into userspace through the vDSO and then deleted the capability. Commit
`1ed95e52d902035e39a715ff3a314a893a96e5b7`, "x86/vdso: Remove direct HPET access through the vDSO",
Andy Lutomirski, authored 2016-04-08, reviewed by Thomas Gleixner, read 2026-09-02 via the GitHub
commit API on `torvalds/linux`:

> Allowing user code to map the HPET is problematic.  HPET
> implementations are notoriously buggy, and there are probably many
> machines on which even MMIO reads from bogus HPET addresses are
> problematic.

and, on the point that decides it here:

> The vclock HPET code has also always been a questionable speedup.
> Accessing an HPET is exceedingly slow (on the order of several
> microseconds), so the added overhead in requiring a syscall to read
> the HPET is a small fraction of the total code of accessing it.

A time source whose read costs several microseconds is slower than asking the kernel. Measured
on cordoba (x86_64, Linux, four cores, load average 3.6) with a throwaway spike on 2026-09-02: a
`clock_gettime(CLOCK_MONOTONIC)` through the vDSO costs 28.4 ns, and the same call forced through
`syscall(SYS_clock_gettime, ...)` costs 596.9 ns. So an HPET read at "several microseconds" is
several times *more* expensive than the syscall that a closed TSC would force. It is not a fallback;
it is a worse version of the thing it would be a fallback from. The spike is not shipped.

The HPET is also not free to reach. `crates/machine_discovery/src/acpi.rs` sees the `HPET` table in
the XSDT walk and does nothing with it. So this route costs table parsing, an MMIO mapping, and a way
to hand that mapping to a program, before it buys a clock that loses to a syscall.

The local APIC. Not a candidate, for three independent reasons. In x2APIC mode, which is what
this era's machines and this era's kernels use (cordoba's `/proc/cpuinfo` reports the `x2apic`
flag), there is no page to map at all. The Intel 64 Architecture x2APIC Specification, reference
number 318148, read 2026-09-02:

> To enhance inter-processor and self directed interrupt delivery as well as the
> ability to virtualize the local APIC, the APIC register set can be accessed only
> through MSR based interfaces in the x2APIC mode. The Memory Mapped IO
> (MMIO) interface used by xAPIC is not supported in the x2APIC mode.

and MSR access is ring 0 per the `RDMSR` quotation above. Even in xAPIC mode the page at
`0xFEE00000` is a CPU-local alias, so a migrated thread would silently read a different core's APIC.
And the register in question is the kernel's own timer, a down-counter reloaded on every tick rather
than a monotonic count. Three ways wrong, and the first one is fatal on its own.

## What Linux's vDSO fast path actually does, since this is the category most likely to change the answer

It reads the TSC from userspace, in every mode it has. This was the specific thing the brief
asked to be determined rather than recalled, so it was read from the current tree of
`torvalds/linux`, fetched 2026-09-02.

`arch/x86/include/asm/vdso/clocksource.h` is the whole list of modes:

> #define VDSO_ARCH_CLOCKMODES	\
> 	VDSO_CLOCKMODE_TSC,	\
> 	VDSO_CLOCKMODE_PVCLOCK,	\
> 	VDSO_CLOCKMODE_HVCLOCK

and `arch/x86/include/asm/vdso/gettimeofday.h` dispatches on it:

> 	if (likely(clock_mode == VDSO_CLOCKMODE_TSC))
> 		return (u64)rdtsc_ordered() & S64_MAX;

The other two are the paravirtual ones, and both of them are a *shared page plus a TSC read*, not a
substitute for one. `vread_pvclock()` in the same file ends its seqlock loop with

> 		ret = __pvclock_read_cycles(pvti, rdtsc_ordered());

and `__pvclock_read_cycles` in `arch/x86/include/asm/pvclock.h` is

> 	u64 delta = tsc - src->tsc_timestamp;
> 	u64 offset = pvclock_scale_delta(delta, src->tsc_to_system_mul,
> 					     src->tsc_shift);
> 	return src->system_time + offset;

The Hyper-V path is the same shape: `vread_hvclock()` calls `hv_read_tsc_page_tsc()`, whose comment
in `include/clocksource/hyperv_timer.h` states the protocol as
"ReferenceTime = ((RDTSC() * ReferenceTscScale) >> 64) + ReferenceTscOffset". So the shared page
carries *scale and offset*, and the counter still comes from the guest's own `rdtsc`. **The page is
how the guest learns to interpret the TSC, not a way to avoid reading it.**

And when the TSC is unusable, the fast path is simply gone. `.vdso_clock_mode` is set only by the TSC
clocksources. `arch/x86/kernel/tsc.c` sets `.vdso_clock_mode = VDSO_CLOCKMODE_TSC` on both
`clocksource_tsc_early` and `clocksource_tsc`, while `clocksource_hpet` in `arch/x86/kernel/hpet.c`
and `clocksource_acpi_pm` in `drivers/clocksource/acpi_pm.c` set no such field, which leaves it at
`VDSO_CLOCKMODE_NONE`, the zero value of the enum in `include/vdso/clocksource.h`. A zero mode falls
through `__arch_get_hw_counter` to `return U64_MAX;` and the caller takes the syscall path.

**So the answer to the question the brief singled out is no: Linux's x86 vDSO fast path cannot work
without a userspace TSC read, and Linux does not pretend otherwise.** Boot an x86 Linux with
`clocksource=hpet` and userspace loses the fast path entirely; it does not get a slower fast path.
That is the strongest single piece of evidence for [finding 2](premise-checks.md), because it is the largest x86 userspace
in existence declining to build the thing this section went looking for.

## What a virtualized guest does when the host denies the TSC

The same problem under a different name, and the answer is that nobody substitutes a different
clock; the read traps and is emulated, and the guest still gets a TSC. Xen documents this most
plainly of the hypervisors. `xen-tscmode(7)`, Dan Magenheimer, read 2026-09-02 at
`xenbits.xen.org/docs/4.13-testing/man/xen-tscmode.7.html`:

> This trap can be detected by Xen, which can then transparently "emulate" the results of the rdtsc
> instruction and return control to the code following the rdtsc instruction.

> TSC emulation is relatively slow -- roughly 15-20 times slower than the rdtsc instruction when
> executed natively. However, except when an OS or application uses the rdtsc instruction at a high
> frequency (e.g. more than about 10,000 times per second per processor), this performance
> degradation is not noticeable (i.e. <0.3%). And, TSC emulation is nearly always faster than
> OS-provided alternatives (e.g. Linux's gettimeofday).

Two things fall out of that and both are useful here. The industry's answer to "this program may not
read the raw counter" is trap and emulate, which keeps the resolution and pays roughly an order
of magnitude in latency. It is a fourth option for the `x86_64` row that neither milestone 75 (who may read the cycle counter)'s
block nor this document had named. And Xen's own measured judgment is that even a trapping TSC beats
the OS-provided alternative, which is the same conclusion the HPET evidence reached from the other
direction.

## What this makes the honest scope note for the `x86_64` row

Not "x86 is different", which explains nothing and reads as an excuse. The reason is specific and
falsifiable, and it has three clauses:

1. On `x86_64` the coarse clock and the fine counter are the same register, so `CR4.TSD` is not
   an analogue of `PMUSERENR_EL0.CR`. It is an analogue of `PMUSERENR_EL0.CR` *and*
   `CNTKCTL_EL1.EL0VCTEN` at once. aarch64 and riscv64 can close the fine one because they have a
   second, coarser, architecturally guaranteed user-readable clock; x86 has no second one at all.
2. The alternatives exist and all of them lose to a syscall. The HPET is mappable and costs
   several microseconds a read, which Linux measured and acted on in 2016. The ACPI PM timer is
   279 ns but lives behind an I/O port grant that is a larger authority than the thing being denied.
   The APIC is not addressable from ring 3 in x2APIC mode. So closing the TSC on `x86_64` does not
   demote userspace from 0.25 ns to 70 ns. It demotes userspace to a syscall, or to a published page
   at tick resolution, which is DECISIONS §43 (reading the clock is a page) one axis over.
3. A published page cannot close the gap by being updated more often, because its update rate is
   the interrupt rate. Getting from a 10 ms tick to a microsecond costs ten thousand interrupts a
   second, per core, forever. This is arithmetic rather than a measurement, and it is why the page is
   a coarse-clock answer and not a fine-clock one.

The consequence for the plan is the one this document already states in
[option 4](../0139-cycle-counter-authority.md#option-4-a-per-thread-grant-enforced-at-the-context-switch) and is now
established rather than asserted. Option 4 on `x86_64` is blocked behind giving that architecture a
second time source, and there is no cheap one to give. The realistic order for the x86 row is
therefore a tick-resolution page, and it is a decision with a price rather than a gap in §19
(architectural parity) that a lane can close by trying harder. Trap-and-emulate was the other
candidate, and it lost: [`trap-and-emulate.md`](trap-and-emulate.md) measures it at 1,667 ns on
cordoba, four times the syscall it was meant to beat. `x86_64` keeping `rdtsc` ambient, with a recorded
reason, remains the right state until one of those is chosen. That is a published confinement
position and it is still calef's.

## BUGS

- **The clocksource comparison was not measured**, only read. Switching cordoba's
  `current_clocksource` to `hpet` and to `acpi_pm` would have priced Lutomirski's "several
  microseconds" directly; it needs root and passwordless `sudo` is not configured there, and the
  machine is the family's live backup server. The 28.4 ns and 596.9 ns figures were measured; the
  HPET read cost is Linux's number, not ours.
- Nothing was measured on xenon, the x86 target machine, which does not exist as a running nife
  host yet. Every x86 measurement here is Linux on cordoba, which prices the mechanisms and not the
  port.
- **`TPAUSE`'s binary-search argument is reasoned from the instruction's specification, not
  demonstrated.** It does not need to be demonstrated for the conclusion, since Intel gates the
  instruction on `CR4.TSD` either way, but it is reasoning rather than observation and is marked so.
- **AMD's APM was not read.** All ISA quotations are Intel's. `CR4.TSD` and `CR4.PCE` are
  architectural on both vendors and this is not expected to differ, but it was not checked, and
  xenon is an Intel machine so nothing here has been checked against an AMD part.
