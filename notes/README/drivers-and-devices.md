# Notes index: Drivers and devices

Part of [the notes index](../README.md), which says how to add a line.

- [virtio-blk, driven from userspace](../virtio.md): a block device driven from EL0 with DMA.
- [PCIe, and driving a disk over it](../pcie.md): the PCIe transport, with the kernel as firmware.
- [Scoping a PCIe transport](../pcie-transport-scope.md): the pre-build scope for PCIe and virtio-pci.
- [NVMe: the first non-virtio disk](../non-volatile-memory-express.md): an NVMe driver confined by the IOMMU alone.
- [radon's SD card and eMMC](../designware-mobile-storage.md): the JH7110's DesignWare Mobile Storage controller, why it reads through the FIFO first, and its read-only bench step on radon.
- [A USB keyboard](../usb.md): the xHCI driver at EL0, the register pages it is denied, and the gate that types `echo hello` on it.
- [Fatal risk 6's bench evening on xenon](../risk-6-bench-evening.md): the confined NVMe driver's preflight, throughput boot and outcomes.
- [The `e1000e` NIC](../e1000e.md): the network card a PC actually has, driven from `net_stack` behind the IOMMU, and its bench step on xenon.
- [radon's Ethernet](../designware-ethernet.md): the JH7110's DesignWare controller, the DMA-coherence probe that runs before it is trusted, and its bench step on radon.
- [Confining DMA without an IOMMU](../dma.md): kernel validation of every descriptor a driver submits.
- [Confining DMA with an IOMMU](../iommu.md): hardware DMA confinement with SMMUv3 and the RISC-V IOMMU.
- [AMD-Vi](../amd-vi.md): confining DMA on an AMD machine, where the IOMMU is in the IVRS rather than the DMAR, and where QEMU's model differs from the specification.
- [Block devices: what is attached, and what holding one means](../block-devices.md).
- [The kernel console](../kernel-console.md): `print!`, its one lock over the UART and the screen, the screen hold, and the escapes from the ring.
- [A machine with no serial port](../serial-less-output.md): screen output for machines without a UART.
- [The framebuffer contract](../framebuffer-contract.md): how a confined client gets pixels onto a screen.
- [The compositor](../compositor.md): one screen shared among clients that distrust each other.
- [Compositor confinement claim 25](../compositor-claim-25.md): the compositor attacked part by part, and what each patch reaches.
- [Glyphs, the VT engine, and input](../glyphs.md): the font, VT engine and keyboard behind on-screen text.
- [The VT engine](../video-terminal.md): `crates/video_terminal`'s parser, grid, damage report, handoff blob, keymap and test script.
- [Bold under Solarized](../solarized-and-bold-is-bright.md): why bold goes gray on four colors, and the options.
