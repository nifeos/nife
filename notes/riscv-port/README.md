# Porting to RISC-V: the appendices to notes/riscv-port.md

[`notes/riscv-port.md`](../riscv-port.md) carries why RISC-V was the second port, the arch contract,
the leaks it found, the RISC-V specifics, the `tp` hazard and the build setup. These files carry the
detail its prose budget has no room for.

- [`hal-leaks.md`](hal-leaks.md): each HAL leak the port exposed and how it closed, with the
  user-entry frame placement that was wrong on RISC-V for a year.
- [`bring-up-log.md`](bring-up-log.md): the six steps of the port, each with the boot line that
  proved it and the bugs it surfaced.
- [`timer.md`](timer.md): the SBI timer, its software deadline grid, `scounteren`, and the tests.
- [`firmware.md`](firmware.md): discovery at boot, the SBI call, T-Head harts, and the exit and
  cold-reboot paths.
- [`iommu.md`](iommu.md): the RISC-V IOMMU driver, its MSI position, what its proofs cover.
- [`memory.md`](memory.md): Sv39, the one-`satp` model, the device window, the boot table, ASID
  trust and TLB maintenance, and the model the `satp` proofs assume.
- [`traps.md`](traps.md): the trap entry, the frames it builds, dispatch, first FP use and faults.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/`
beside `notes/X.md`. calef names directories.*

Each stem here is named for its content, and calef names files.
