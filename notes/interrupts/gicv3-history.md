# GICv3: the gate against silent loss, and what no CPU matrix catches

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.*

An appendix to [notes/interrupts.md](../interrupts.md), moved there on 2026-10-11 (UTC) under
§212 (a prose budget). It holds the three points where a wrong GIC assumption now fails, the
measurement that made GICv3 dangerous, the price milestone 222 (a leg that fails instead of skipping) put on a driver, and why aarch64
has no CPU-model matrix.

## The gate against silent loss

A GIC assumption that stops being true now fails loudly, at three points:

- An `intc@` node whose binding is not a GIC this kernel knows panics in `memory::init`, naming
  the `compatible` it found, rather than being driven as a GICv2 on the strength of its name.
- A tree the hardware contradicts panics in `arch::irq::init` before any configuration write,
  naming the version claimed and the revision read. This is the check that would have stopped
  milestone 222's boot: a GICv2 claim over a redistributor frame reads `GICC_IIDR` as zero.
- A core with no redistributor, or whose `ICC_SRE_EL1.SRE` will not set, panics as it comes
  online. And `arch::irq::tests::every_online_cpu_takes_its_own_timer_ticks` holds every core, not
  only the test's own, to taking interrupts; injecting "secondaries never enable Group 1" fails it
  naming core 1.

## The measurement, taken 2026-09-02, and why the GIC pin was the most dangerous line in the file

Until milestone 227 the runner's `gic-version=2` pin was protection rather than support, and this
section said so. The measurement below is what made it load-bearing, and it is kept because it is
the shape the gates above exist to make impossible.

This section once said `-machine virt,gic-version=3` had never been booted here, that it was one
command, and that it would turn "our driver does not support it" from an assumption into a recorded
failure with an error message attached. Milestone 222 ran the command. There is no error message.

```
  timer       : 100 Hz tick, counter at 62 MHz, interrupts ON
  scheduler   : 5 thread(s), round robin, preemptive
  ...
    thread 1 (hostile) :          0 iterations
    thread 2 (polite)  :          0 iterations
    preemptions        :          0
```

Under `-machine virt,gic-version=3,iommu=smmuv3 -cpu cortex-a72 -smp 4` the kernel boots all the
way through the tour, brings four cores online over PSCI, prints that the timer's interrupts are
on, and then never receives one. Nothing faults and nothing says anything is wrong.

The reason is that a GICv3 device tree node carries two register blocks like a GICv2 one does, so
`memory::gic_regions()` finds a pair and hands them over without complaint. The second block is the
redistributor rather than a CPU interface, and `gic::init_this_cpu` writes `GICC_CTLR` and
`GICC_PMR` into it. Those are device-memory writes to registers that are not there: they land, they
do nothing, and the real CPU interface (`ICC_*`, system registers) is never enabled. Every interrupt
the distributor routes is then delivered to a core that has not agreed to receive any.

So the honest statement of the limitation is not "GICv3 is unsupported", it is "GICv3 boots and
silently loses every interrupt". That was the more dangerous shape, and it was why the QEMU runner's
`gic-version=2` pin was load-bearing rather than tidy. Without it, a QEMU that changed its default
would have produced a kernel that came up and then quietly stopped preempting anything.

## What a GICv3 driver would actually be, priced in advance, and what it turned out to be

Milestone 222 priced it from the failure: a version decision at init, a redistributor per core, a
system-register CPU interface under `arch/aarch64/`, affinity routing, and both drivers coexisting
behind roughly ten call sites. Milestone 227 built exactly that list and found two things the
pricing did not have. First, SGIs must stay permanently enabled on a GICv3. QEMU's GICv2 model
and the GIC-400 treat them that way, and the Irq capability's mask-on-fire, unmask-on-ACK protocol
relied on it without saying so. Second, HVF does not report the system registers in the ID register
that says whether a core has them (see BUGS in [the main page](../interrupts.md)). The call sites all moved to `arch::irq`.

## Why there is no aarch64 CPU-model matrix, unlike RISC-V's (milestone 59 (the CPU-model matrix), DECISIONS §53 (parity is a matrix, not a pair))

The asymmetry is real and worth stating, because "we did it for RISC-V" is the obvious argument for
doing it here and it is wrong.

- We already test on a conservative real core. The aarch64 runner uses `-cpu cortex-a72`, an
  ARMv8.0-A chip, not QEMU's `max`. RISC-V's default was the maximalist model, which is what made a
  matrix worth building there. Here the emulator is *less* capable than a modern board, and code
  that runs on an A72 runs on an A76.
- aarch64 has architectural feature discovery and RISC-V does not. The `ID_AA64*` registers are
  mandatory and readable at EL1, and this kernel already uses them: `arch/aarch64/mmu.rs` reads
  `ID_AA64MMFR0_EL1::PARange` and feeds it to `TCR_EL1::IPS` rather than assuming a physical address
  range. That is why milestone 60 (ISA discovery) is a RISC-V milestone specifically; RISC-V omitted
  CPUID on purpose and left discovery to a device-tree string.

## What no CPU matrix catches on either ISA

Memory ordering. Different microarchitectures reorder differently, and a missing
`Acquire`/`Release` can pass on one core and fail on another. **QEMU's TCG does not faithfully model
reordering**, so no `-cpu` value tests it. That class is covered by a different mechanism and
`ci.yml` says so: CI runs on a real aarch64 runner, because a missing barrier passes on an x86_64
host and fails only on real ARM. Real silicon is the test; an emulator cannot be.
