---
status: DECIDED
raised: 2026-07-27
decided: 2026-07-27
ratified_by: calef
---

# 17. The second architecture: RISC-V, and the page-table format trait

The port to RISC-V (rv64, QEMU `virt`) is the first real test of rule #1 ("all architecture-specific
code lives under `arch/`"), an assumption held on faith since milestone 1 (boot to Rust on QEMU).
RISC-V over x86_64 because it is clean-different rather than legacy-different. It exercises the HAL
abstraction (a different trap, paging, interrupt, and firmware model) without the real-mode / GDT /
IDT / APIC / UEFI tax. x86_64 is the third port. Full plan and findings: notes/riscv-port.md.

The port found the `arch/` boundary was **almost** complete, and each leak it exposed was pushed under
`arch/`, not bodged around:

- **`thread::Context`** was aarch64-register-shaped in portable code. Now arch-owned, behind
  intent-named constructors (`Context::for_kernel_thread`, `for_user_thread`). `thread.rs` names no
  register.
- The userspace-entry path (the EL0/U-mode trap frame, the enter-userspace glue, cache
  coherence) lived in `user.rs`. Now `TrapFrame::for_user_entry`, `arch::enter_user`,
  `arch::sync_icache`, `arch::current_sp` seams. The hand-written aarch64 demo programs are gated to
  aarch64 (RISC-V reaches U-mode through the ELF loader).
- The `paging` crate encoded the aarch64 descriptor format outright.

Decision on `paging` (the significant one, calef steered it): generalize behind a `PageFormat` trait
rather than duplicate the walk or write a separate RISC-V pager. `Flags` became a format-neutral
capability set (write / user / user-exec / kernel-exec / global / device) with the same constructor
and predicate API. The `Mapper` walk (`map`/`unmap`/`translate`) is written once and generic over
`F: PageFormat`. Each format (`Aarch64`, `Sv39`) supplies only `LEVELS`, the half split, and the
handful of encode/decode operations. The walk is proved once and the encoding proved per format. The
aarch64 and Sv39 modules each carry Kani proofs of index-in-bounds, address/permission separation,
and the half split, and the shared walk inherits both. The alternatives (a duplicated sibling
module, or a fresh un-verified RISC-V pager) were rejected because only the trait gives RISC-V
paging the same formal verification aarch64 has. That is the demonstrator's whole point. The cost
was real (it touched a verified crate and every `Flags` consumer) but landed with aarch64 fully
green. Base Sv39 has no device-memory PTE bit, so `CAP_DEVICE` rides in an RSW (software) bit to
keep the `Flags` round-trip exact. Real hardware would use Svpbmt. Portable code that must name the
format (the user-VA gate, the user `Mapper`) refers to `arch::mmu::Format`, so the choice lives in
`arch/`.

The whole capability core runs on RISC-V today. That covers boot (higher-half Sv39, `.bss`, the `tp`
per-CPU register, an NS16550 console, the OpenSBI device-tree handoff). It covers the MMU
(fine-grained W^X Sv39 tables + `satp`, the single-`satp` process model where every root carries the
kernel high half). It covers traps (`stvec`, the `sscratch` dance, the system-call-ABI reconciliation),
the SBI timer, the scheduler and context switch, and U-mode user programs making system_calls. It covers
a capability invocation (`SYS_INVOKE` → lookup → rights check → endpoint SEND) from U-mode and
**preemption** (the timer preempts a thread that never yields, DECISIONS §5 (execution model)). And
it covers a real compiled ELF at U-mode: the `worker` Rust binary, delivered as the initrd and run
through the kernel's arch-neutral ELF loader, squares an input and SENDs it home.

The ELF step needed three small aarch64 assumptions closed. `user_rt` grew a RISC-V system call ABI
(`ecall`+`a7`). The `elf` crate accepts the running kernel's machine (a symmetric, cfg-selected
`EXPECTED_MACHINE`, so each kernel refuses the other's binaries). One trap instruction in the
program was arch-gated. The loader itself was already portable.

The port surfaced two RISC-V-specific bugs worth recording. `tp` is a general register, not a system
register, so a U-mode trap must restore the kernel `tp` from a known source rather than trust it
(notes/riscv-port.md). The shallow TCB entry path let a trap frame placed at the top of the kernel
stack overlap `enter_frame`'s own frame. That bug was fixed by placing the frame below the live
`sp`.

Userspace init builds the system too. From a nifefs archive holding `init` (a portable minimal
builder) and `worker`, the kernel loads only `init`, maps the archive into it, and grants it a
budget and a report endpoint. `init` loads `worker` by name and builds it as a child through the
capability verbs, wiring it to the report. The kernel never touches the child's bytes. That
exercised the full userspace capability system call surface on RISC-V.

Device interrupts flow through the PLIC too. A keystroke raises the NS16550's line into the PLIC,
which delivers a supervisor external interrupt. The handler claims it, masks the source, and
notifies the endpoint it is routed to. A driver blocked there wakes, reads the byte, and re-arms.
This is the real second consumer the interrupt-controller abstraction was waiting for.

The last leak is closed. The device driver moved to userspace (an unprivileged process holding an
`Irq` capability and a device mapping, running the seL4 `WAIT`/read/`SEND`/`ACK` loop), and its ACK
forced the interrupt-controller seam. Then `arch::irq` was extracted with two working consumers
behind it, the GIC and the PLIC, and `drivers::gic` was gated to aarch64. Every portable caller
names `arch::irq`. The only code naming `drivers::gic` is aarch64 arch code and `cfg(test)` aarch64
tests. The order was deliberate. Prove a real interrupt through the PLIC, then a userspace driver
exercising the ACK on both arches, then extract. That way the abstraction was factored out of two
exercised controllers rather than guessed ahead of one.

The RISC-V port is complete. Every primitive that defines the kernel runs on both aarch64 and
RISC-V from one portable core. Those primitives are boot, the MMU, traps, the timer, the scheduler,
preemption, U-mode user programs, capability invocation, userspace-built processes, and device
interrupts serviced by an unprivileged userspace driver. Rule #1 held: the second ISA was a new
`arch/` directory, not a diff across the kernel, with no exceptions left in portable non-test code.
