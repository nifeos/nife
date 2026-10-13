# The HVF leg: semihosting is not answered, and what that looks like

An appendix to [`notes/hvf-leg.md`](../hvf-leg.md). It records what milestone 81 (an HVF leg: the
test suite on the physical core) measured about ARM semihosting under HVF on QEMU 11.0.2, and how
the host ends the run instead.

The test harness reports its verdict by asking the host to exit. `testing::runner` ends in
`semihosting::exit`, and so do the panic handler and both watchdogs. QEMU 11.0.2 does not
intercept ARM semihosting on the HVF path. The tree already believed this: `notes/benchmarks.md`
records "semihosting does not work under HVF", and the bench kernel parks in `wfi` instead of
exiting. Milestone 81 measured it, and measured what happens next.

The probe was nine instructions: write `A` to the PL011, then `hlt #0xf000` with `x0 = 0x18`
(`SYS_EXIT`).

| accelerator | result |
|---|---|
| TCG | the process exits at the trap |
| HVF | `A` appears, then nothing; QEMU runs until killed |

Booting the real kernel says what the guest sees. The trap is not swallowed. It raises a real
synchronous exception into the guest's own vector table: `EC 0x00` ("Unknown reason",
`ESR_EL1 = 0x02000000`), with `x0 = 0x18` and `x9 = 0x20026` still in the registers. Our handler
does the correct thing for an exception it does not know, which is to panic. The panic handler
under `cfg(test)` calls `semihosting::exit`, which takes the same trap again. Four cores doing
that write interleaved garbage at native speed, forever. `kernel/src/arch/aarch64/semihosting.rs`
predicted exactly this in a comment ("without one it raises a real exception"). It did so without
knowing it had a case.

So the host, not the guest, ends the run. `hvf_kernel_leg` owns the QEMU child and reads its
transcript, the same shape `run_bench` uses (milestone 21 (performance measurement)). It
takes its verdict from three markers the harness prints *before* the exit that will not happen:
`test result: ok. N passed`, `[PANIC] `, and `WATCHDOG:`. Once something has failed it relays 200
more lines (enough for the longest thread dump) and stops, because there is no end of output to read
to.

Without that budget an xtask spins on the storm, holding `target/nifefs-blank.img`, and the *next*
run fails with `Failed to get "write" lock`, which is precisely the trap CLAUDE.md warns about.
