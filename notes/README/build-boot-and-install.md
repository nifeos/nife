# Notes index: Build, boot and install

Part of [the notes index](../README.md), which says how to add a line.

- [LLVM](../llvm.md): how rustc and LLVM turn Rust into aarch64.
- [Linker scripts](../linker-scripts.md): who places code, zeroes `.bss`, and sets the stack.
- [ELF](../elf.md): the file format the kernel ships in.
- [The boot protocol](../boot-protocol.md): the arm64 Image header that marks a kernel.
- [The boot stick, and the program that makes it](../boot-stick.md).
- [What a nife package is, and what still cannot be done with one](../packages.md).
- [Serving the lab's update index from cordoba: a spec for calef's homelab agent](../lab-index-on-cordoba.md).
- [Package boundaries: every crate and program in a package, every path with a home](../package-boundaries.md).
- [Who may write the activation set: a proposal](../who-may-write-the-activation-set.md).
- [Two boot slots, so a bad upgrade cannot brick the machine](../boot-slots.md).
- [Packages that build the OS, and divisions that release together: the measurements](../packages-and-divisions.md).
- [The first repository split: which preconditions hold, which cut goes first, and what basalt must be](../the-first-split.md).
- [Installing nife onto a disk from a boot stick](../installing.md).
- [The FAT32 writer](../file-allocation-table.md): the volume's shape, VFAT long names, and who reads it back.
- [The sealed pair](../sealed-pair.md): checking on the host that a kernel vouches for the archive beside it.
- [xenon bench evening](../xenon-bench-2026-10-18.md): the I219, the network boot, a fresh install, 801's fetch, then ioping and the soak.
- [Turning Secure Boot off, before booting the nife stick on a PC](../turning-secure-boot-off.md): the install page's first instruction.
- [xenon bench card, October 2026](../xenon-bench-2026-10.md): risk 6's throughput boots, then the install, in that order.
