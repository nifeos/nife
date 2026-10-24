# Notes index: The machine

What the hardware is and how the kernel meets it: read these before any kernel code.

Part of [the notes index](../README.md), which says how to add a line.

- [Registers](../registers.md): the CPU's whole state; the most fundamental note.
- [Harts and PEs](../harts-and-pes.md): precise words for one instruction stream, not "core".
- [aarch64](../aarch64.md): the instruction set, privilege levels, and our target triple.
- [Reading aarch64 assembly](../reading-assembly.md): five rules for decoding assembly; start here.
- [The stack, `sp`, and `x30`](../stack.md): how the stack works, and our stack incidents.
- [Stack high-water](../stack-high-water.md): measuring how deep each kernel stack actually goes.
- [Exceptions](../exceptions.md): faults, interrupts and syscalls as one aarch64 mechanism.
- [Interrupts: the GIC and the timer](../interrupts.md): the GIC, the timer, and interrupts as messages.
- [The device tree](../device-tree.md): the machine's self-description, and how we parse it.
- [ISA discovery](../isa-discovery.md): reading each CPU's features and core list at boot.
- [Machine discovery: the shared readers](../machine-discovery.md): core lists, interrupt ids, PLIC contexts, the GIC, the screen token, PVH and ACPI. Name provisional.
- [The UART](../uart.md): the serial port, and our PL011 driver line by line.
- [The PMU, and the two clocks in a core](../pmu.md): the cycle counter versus the virtualization-proof generic timer.
- [QEMU](../qemu.md): the emulator we develop on, and its flags.
- [Semihosting](../semihosting.md): how the kernel reports a test exit status to QEMU.
- [Call-frame information in hand-written assembly](../cfi-unwind.md): unwind tables for the hand-written `.s` files. Name provisional.
