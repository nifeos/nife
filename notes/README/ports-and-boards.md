# Notes index: Ports and boards

Part of [the notes index](../README.md), which says how to add a line.

- [How portable kernels are written](../portability.md): what belongs in `arch/`, and why port early.
- [The aarch64 port](../aarch64-port.md): what `kernel/src/arch/aarch64/` decides that no subsystem note carries. Name provisional.
- [Porting to RISC-V](../riscv-port.md): the second-architecture port and the `arch/` boundary it tested.
- [Scoping RISC-V / aarch64 parity](../riscv-parity-scope.md): the RISC-V parity gaps, all now closed, with corrections.
- [The RISC-V arch tests](../riscv-arch-tests.md): RISC-V twins of the aarch64 arch unit tests.
- [RISC-V Summit Europe 2026, read for what it changes here](../riscv-summit-2026.md).
- [Porting to x86_64](../x86-port.md): the third-architecture port, and what did not fit `arch/`.
- [The hand-assembled x86_64 user programs](../x86-user-programs.md): the byte-listing fixtures and why a blocking escape must exit.
- [Booting x86_64 from real firmware](../x86-uefi-boot.md): the UEFI loader, and the bench procedure for xenon.
- [xenon's firmware, page by page](../xenon-firmware.md): the OptiPlex 7050 setup UI, transcribed setting by setting. Name provisional.
- [Where nife could actually run, and what the three bench machines are named](../target-hardware.md).
- [The aarch64 board for the seL4 comparison](../aarch64-board-survey.md): choosing a board sel4bench really runs on.
- [A modern aarch64 board for nife's first ARM machine](../aarch64-modern-board-survey.md): boards scored on Armv9 features, SMMUv3 coverage and standard boot. Name provisional.
- [How much of the ARM plan runs under HVF](../arm-plan-under-hvf.md): seL4 against nife on one Apple core, the guest's CPU features, and the SMMUv3. Name provisional.
- [The VisionFive 2: first silicon](../visionfive2.md): radon's board facts, boot paths and bench runbook.
- [Programming a clock and a reset line, for the first time](../jh7110-clock-and-reset.md).
- [The JH7110 watchdog](../jh7110-watchdog.md): how the block counts twice before it resets, and the sources read. Name provisional.
- [Reading a board, without a person watching it](../board-console.md): how `script/board-console` reads a board's serial boot unattended.
- [Pricing milestone 342 (the kernel and the console server drive one UART from two address spaces)](../kernel-console-arbitration-pricing.md): whether argon, radon and xenon have a second usable UART, and what a kernel/console-server arbitration option costs in a panic.
- [The boot ladder](../boot-ladder.md): the boot markers every architecture prints, in order.
- [The boot self-test's checks](../self-test-checks.md): what each check proves and why it is built that way, the bounded waits, injection, and the module's BUGS.
- [What rented metal costs](../rented-metal.md): priced rented hardware for each architecture. Name provisional.
