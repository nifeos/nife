---
status: BUILT
raised: 2026-07-23
built: 2026-10-03
---
# 16. Real hardware + IOMMU-backed driver isolation (recast 2026-07-27: RISC-V first)

Not the "no board" kind: the board is on the desk. The work that needed somebody sitting at it
(flashing an image, watching a serial console, power-cycling a wedged board) is done. 16a is built:
first silicon boots the full tour on three harts ("nife: the capability core runs on RISC-V"), and
notes/visionfive2.md carries the bench record. The three on-board items are done:

- The on-board test-suite exit. `kernel/src/arch/riscv64/semihosting.rs` replaces the
  `sifive_test` finisher with a UART marker plus SBI SRST under the `board` feature. It is the
  mechanism `script/soak` and `crates/board_console` judge every board run by.
- The DTB-driven UART IRQ. On radon the kernel takes the UART's interrupt source (32) from the
  machine description, and two `kernel::sched::tests` interrupt-delivery tests read
  `user::uart_irq_and_source()` rather than QEMU's source 10.
- The real-cycle benches. Milestone 74 (cycle counters)'s riscv64 half is built: the kernel speaks
  the SBI PMU extension, asks firmware for a counter that counts CPU cycles, checks it is actually
  counting, and `cargo xtask bench --riscv` prints one `cycles_per_tick` probe that converts every
  tick-denominated row in `bench/baseline-riscv64.txt` at once. radon read `cycles_per_tick 250.00`
  on 2026-09-16, a real figure where QEMU-TCG's exact 100.00 is an artifact of one virtual clock
  driving both CSRs.

So milestone 16 has no work of its own left. Whatever milestone 74 still owes is its own block's,
the `sel4bench` comparison is milestone 25 (cross-OS comparison)'s, and carrying 16b's IOMMU driver
to silicon is milestone 143 (silicon IOMMU), because it waits on a board that ships the ratified
RISC-V IOMMU spec and no such board exists today.

In brief. 16a: first silicon on a VisionFive 2-class board, whose firmware contract (OpenSBI, SBI HSM, NS16550, PLIC, Sv39) is exactly what the kernel already speaks. 16b: IOMMU-backed DMA isolation against QEMU's emulation of the ratified RISC-V IOMMU (v1.0.1) first, over the PCIe transport of §18 (one driver, two buses, the seam in the kernel), with §20 (IOMMU-backed DMA isolation) as its seam; silicon when a board ships it

Why it matters. isolation in hardware, under real workloads; the second ISA becomes the first silicon, and the IOMMU work stops waiting on a purchase

The milestone was always two things bundled, first silicon and DMA isolation in hardware, and
the recast splits them, because each is better served on the RISC-V side now.

16a: first silicon, on a VisionFive 2-class RISC-V board. The riscv port's firmware contract
on real boards is IDENTICAL to what the kernel runs today: OpenSBI, SBI HSM bring-up (the hart
lottery is already survived, on the record), NS16550, PLIC, Sv39. A ~$60-100 board boots the
exact contract we speak; the aarch64 side fits real boards worse (a Pi wants TF-A for PSCI, its
default is spin-table, and its IOMMU story is the weak spot notes/target-hardware.md already
flags). Deliverable: boot, UART, SMP, the test suite where semihosting allows, and the benches
on real cycles via the SBI PMU extension. Caveat, stated now: sel4bench's platform coverage is
thinner on RISC-V than ARM, so the milestone-25 seL4 comparison may still eventually want an ARM
board; that purchase moves to "when 25's leftover justifies it".

16b: IOMMU-backed DMA isolation, in emulation, on BOTH boards (parity, calef's direction
2026-07-27). Each `virt` board emulates its architecture's native IOMMU: SMMUv3 on aarch64
(`-machine virt,iommu=smmuv3`, mature) and the ratified RISC-V IOMMU (v1.0.1) on riscv (newer;
its bugs may be QEMU's, and the record should say which is which). Both sit in front of PCIe,
which §18 drives on both boards; both need `iommu_platform=on` per virtio device, and a device
without it silently bypasses translation, the same manufactured-fact hazard the runners now
fail loudly on. The two IOMMUs are structural siblings, and the deep symmetry is the payoff:
each translates with its own CPU's page-table format (VMSAv8-64; Sv39), so the format-generic
`paging` crate, the seam that was HAL leak #2, builds IOMMU domains with the same proved code
that builds process address spaces. Shape: one portable DMA-domain seam, two arch IOMMU
drivers under `arch/` (device table, command queue, fault queue each), the `Virtio` capability
unchanged above, the disk and attacker suites running behind the IOMMU on both ISAs, and the
shadow ring demoted to defense in depth everywhere. Silicon carries 16b's riscv code over when
a board ships the ratified spec; that is the emulate-then-carry pattern the kernel was built
on. Parity is claimed at the QEMU tier; 16a's silicon is one board first, honestly.

Built 2026-07-28 (16b, both ISAs in emulation; DECISIONS §20, notes/iommu.md). The portable
DMA-domain seam (`crate::iommu` over `paging::domain`), the two arch drivers (SMMUv3, RISC-V IOMMU
v1.0.1), boot bring-up (SMMU from the device tree, RISC-V IOMMU enumerated as a PCI function), the
`iommu_platform=on` enablement with the confinement test as the loud-on-bypass guard, and the disk
and both attacker suites passing behind the IOMMU on both boards (aarch64 118 kernel tests, riscv
60). Both emulations behaved to spec, no QEMU-vs-ours bug surfaced. Shadow ring kept as defense in
depth.

Why. This is where the discussion's strongest pro-microkernel argument finally becomes true
for us. Today driver isolation is real only because of the shadow descriptor ring we wrote
(notes/dma.md); an IOMMU makes it real in hardware, with the software ring demoted to defense in
depth.

Prior art. design/driver-domains.md already works the principled version (a driver per VM,
stage-2 behind a hypervisor). Hardware-gated there; 16b's emulation-first path is not.

Also closes an integrity window (milestone 22 (trusted init)'s precondition). Before DMA is confined
in silicon, a malicious device can DMA over any RAM the kernel has not walled off, *including the
initrd holding init before the kernel has loaded and measured it*. Software confinement (the shadow
ring) governs a driver the kernel already trusts to run; it does nothing about a device corrupting
init's bytes at rest. So verifying init (22) is only airtight once 16 removes the way to tamper with
it underneath the check.
## Follow-on

- **Done.** The on-board test-suite exit is `kernel/src/arch/riscv64/semihosting.rs`, whose board
  feature replaces the `sifive_test` finisher with a UART verdict marker plus SBI SRST shutdown.
  `script/soak` and `crates/board_console` are what read it.
- **Done.** The DTB-driven UART IRQ landed: the machine description supplies the source in
  `kernel/src/user.rs`, read by `kernel/src/main.rs` and by the input service, and the nine boots
  of the 2026-09-03 series each printed `uart irq : source 32 (machine description)`.
- **Milestone 74.** The real-cycle benches were the last of 16a, and the riscv64 number was
  taken on 2026-09-16: `cycles_per_tick 250.00`, which converts every tick-denominated board row to
  cycles. 74's own block carries the figure, the reason a round number is real here rather than the
  emulator artifact 16a warned about, and the cycle table it unlocks. 74's own block says what is
  left of its aarch64 half.
- **Milestone 143.** Carrying 16b's RISC-V IOMMU driver to silicon is that block, gated on hardware
  because no board shipping the ratified spec exists.
- **Milestone 241.** The aarch64 board this block deferred to when milestone 25's leftover
  justifies it now has its own block, with the market work in `notes/aarch64-board-survey.md`.
- **Milestone 252.** The gap this block found in itself, that a PARTIAL block goes unread while it
  keeps claiming work, is milestone 252.

## Index row

isolation in hardware, under real workloads
