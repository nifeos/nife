# Starting the secondary cores on x86_64

*Name: provisional, like every stem in this directory.*

aarch64 asks firmware to start a core (PSCI `CPU_ON`) and RISC-V asks SBI (`hart_start`). x86 has
no firmware call for it. The boot core sends the target two kinds of inter-processor interrupt
through its local APIC. An INIT resets the target into a wait-for-SIPI state. A STARTUP IPI (a
"SIPI") names a physical page below 1 MiB, where the target begins executing in 16-bit real mode.
The code is `kernel/src/arch/x86_64/bring_up.rs` (`cpu_start`) and `ap_boot.rs` (the trampoline's
page). This page holds the constraints behind them. The counting bug that shaped `cpu_start` is in
[what-does-not-fit.md](what-does-not-fit.md).

## The trampoline page

INIT-SIPI-SIPI cannot place code or pass a value. `ap_boot::prepare` does both before the messages
are sent. It copies the trampoline's bytes onto its low page and writes the one value that varies
per core, the stack top, which `boot.s`'s 64-bit tail reads back RIP-relative.

`link-x86_64.ld` gives `.ap_trampoline` a fixed low VMA, `AP_TRAMPOLINE_PHYS`, where it has to
execute, and a load address in a gap it opens between `.rodata` and `.data`. That gap is not
`.boot_scratch` or the secondary stacks, both of which change at run time. Neither general mapping
rule reaches the gap. The direct map skips the kernel image, as it skips `.text`, to avoid a
second writable alias. And the gap lies between two mapped sections rather than inside either. So
`mmu::map_everything` direct-maps it by name, the way `map_firmware_regions` covers the other gaps
the general rules miss. The destination is the low megabyte, which `map_firmware_regions` maps
writable and the frame allocator never hands out (`machine::LOW_MEGABYTE`).

Because `secondary_boot` is linked at that low address, its own address already is the physical
page. On aarch64 and RISC-V, `secondary_boot` is an ordinary high-linked label and
`smp::bring_up_secondaries` converts it with `virt_to_phys`. Doing that here computes
`secondary_boot - DIRECT_MAP_BASE`, an underflow. `arch::secondary_boot_entry` is therefore the
identity on x86_64. The page is a multiple of 4096, which lets `cpu_start` derive the STARTUP vector
with a shift.

## One page, one core at a time

There is exactly one trampoline page, shared by every STARTUP IPI the kernel sends, and
`smp::bring_up_secondaries` starts one core per call with nothing else serializing the calls. So
`cpu_start` does not return until the core it started is online or given up on. aarch64 and RISC-V
return as soon as firmware accepts the call. Waiting for `smp::online_count()` to move is what
makes the next call's `prepare` safe to overwrite the page.

## The sequence and its timing

The algorithm is the Intel MP specification's (appendix B.4): INIT, a 10 ms settle delay, a
STARTUP IPI, and 200 µs. The online count is read once, before the INIT, and every wait compares
against that value. A core quick enough to check in during the settle delays has already moved the
count, so a later re-read would wait for a second increment nobody makes.

The specification calls for two STARTUP IPIs, and a core that has already left wait-for-SIPI is
defined to ignore the second (QEMU's `apic_sipi` does). `cpu_start` sends the second only when the
count has not moved, which costs nothing when the first worked and is no less correct.

The delays use `busy_wait_us`, a spin on the calibrated TSC. Parking on `hlt` for them would need
an interrupt nobody is going to send.

## The ten-second budget, and why the wait halts

The budget is how long the boot core waits for the target to run its trampoline, adopt the fine
map and reach `secondary_main`'s online mark. Under QEMU TCG all of that runs on whatever host
thread the emulator's scheduler reaches next. One second was measured too short once. It is
possible that was the counting bug rather than the budget, but a loaded host really can starve a
vCPU thread for that long. `smp::tests::wait_for` has a sixty-second budget for the same host
contention, one level later.

Between checks the boot core halts rather than spins. TCG runs each vCPU as one host thread, and
`pause` does nothing for host scheduling, so a spinning boot core only makes it harder for the
thread it waits on to run. `wait_for_interrupt` parks the core on `hlt` until its own local APIC
timer wakes it. The timer is armed at `TICK_HZ` and already ticking, because interrupts are enabled
before `bring_up_secondaries` runs. That costs at most one tick per check, and it yields host CPU
time. It was measured to turn an occasional full hang, waiting past even a sixty-second budget,
into a clean give-up within the budget when a core does not come up.

## What has been measured

`NIFE_SMP` defaults to 2. Three or more cores are unmeasured on silicon, and xenon has not been
asked to bring all four online. The line to read there is `smp: 4 core(s) online`. The cross-core
TLB shootdown that SMP needs is an NMI, because `unmap_page` runs with interrupts masked on every
core; [x86-tlb-shootdown.md](../x86-tlb-shootdown.md) has the protocol.
