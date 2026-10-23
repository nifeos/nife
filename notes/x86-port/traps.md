# The x86_64 port: the trap path, the interrupt controllers, and the TSS

This appendix carries the design behind `kernel/src/arch/x86_64/exceptions.rs`, `irq.rs` and
`segments.rs`. The doc comments there state each item's contract and link here for the reasoning.
[`ring-3.md`](ring-3.md) has how ring 3 was first reached, and
[`interrupts-and-the-fine-map.md`](interrupts-and-the-fine-map.md) has the IO APIC's register
interface and the legacy-IRQ trap in depth.

## Why x86 needs 256 stubs

aarch64 has one vector base and sixteen 128-byte slots the hardware picks between. RISC-V has one
entry point and a cause register. x86 has 256 independent entry points and tells the handler
nothing. So trap.s manufactures the vector number by having one stub per vector, and pads a zero
error code for the vectors whose exception pushes none.

A long-mode interrupt pushes SS:RSP even when the ring does not change. There is therefore one
frame layout, not two, and `iretq` always pops five words.

Every vector has a present gate, including the ones no hardware drives. A gate that is not present
turns an unexpected vector into a general protection fault whose error code is the vector, a second
puzzle on top of the first. A present gate that prints "vector 87, unexpected" costs 4 KiB.

The IDT is built once, by the first core to reach `exceptions::init`, which is the boot core.
`smp::bring_up_secondaries` starts no core before the boot core's `arch::init` returns. Every core
still loads its own IDTR, since `lidt` writes a per-core register. A secondary that rewrote the 256
gates would race the boot core's interrupt delivery, which stays enabled across bring-up so its
timer keeps ticking. A torn gate descriptor read mid-write is a garbled trap, not a compile error.

## Entering ring 3

There is no "return to user" instruction and no mode bit to set. `iretq` pops a context, and the
low two bits of the CS it pops name the ring. It pops SS:RSP as well because the ring changes. The
GDT layout in `segments.rs` is load-bearing for that reason.

A fresh thread's `rflags` is `IF | 0x2`. Bit 1 has read as one on every x86 since the 8086, and a
frame without it is malformed. `IF` keeps a tight-loop user thread preemptible, the same intent as
RISC-V's SPIE and aarch64's DAIF = 0. `IOPL` stays 0. Ring 3 reaches a port only through the TSS's
I/O permission bitmap, which a port capability fills (see "Port grants" below).

`enter_user` refuses a frame whose `rip` is 0. A thread dispatched that way fetches from address 0,
faults and dies, and whatever it served never answers: a lost-wakeup hang far from its cause. The
check is a load and a branch on a frame the function already touches, so it pushes nothing over
the frame at the top of the stack. The RISC-V twin carries the same check.

## The syscall ABI

The syscall number rides in `rax`, following Linux. The tree's shape is one number register plus
six argument registers, with argument 0 doubling as the return: aarch64 uses `x8` and `x0`..`x5`,
RISC-V `a7` and `a0`..`a5`. On x86 that gives `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`.

`r10` replaces `rcx` because of the instruction, not taste. `syscall` overwrites `rcx` with the
return address and `r11` with the caller's RFLAGS, so the C ABI's fourth register cannot carry an
argument. Every x86_64 kernel makes the same substitution. §124 (the x86-64 syscall ABI) is the
ruling. Every hand-assembled program in `user::x86_programs` reaches `crate::syscall::dispatch`
through `TrapFrame::{syscall_nr, arg, set_arg}`, so the portable dispatcher never names an x86
register.

### Why the return is `iretq`

`sysret` is the faster half of the pair, and this port does not use it. It returns to whatever
`rcx` holds without checking that the address is canonical. A non-canonical `rcx` faults in ring 0
on the user's stack, the shape of CVE-2012-0217. Using it safely needs an explicit canonicality
check and an `iretq` fallback. Sharing `isr_restore` costs some tens of cycles per syscall and buys
one return path with one `swapgs` rule. A benchmark should decide it, once this architecture has a
syscall-heavy workload to measure.

The `syscall` path's kernel stack is a per-CPU slot (`SYSCALL_KERNEL_RSP_OFF`). `isr_restore`
writes it and `TSS.RSP0` together on every return to ring 3, so a trap and a `syscall` cannot name
different stacks. RISC-V does the same job with `sscratch`; aarch64 banks `SP_EL1` and needs nothing.

## The vectors the trap body serves

`x86_trap_body` matches on the vector. The arms whose reasoning does not fit in the code:

- **`#NM`, vector 7.** `CR0.TS` raises it on a thread's first FP or SSE instruction (milestone 447
  (a thread's vector registers are its own)). Enabling the unit is the whole handler: `#NM` is a
  fault, so the `iret` re-executes the instruction. `TS` answers "has this thread ever wanted the
  unit", and the registers are always the running thread's or the initial state, which is why this
  is not LazyFP (CVE-2018-3665; `crate::fp` has the argument). The arm serves ring 0 too, because
  every userspace target in `targets/` is soft-float and milestone 447's concurrency proof is
  written as kernel threads.
- **The timer and the reschedule IPI.** Both record and defer (§9 (locking)): the trap body returns `true`
  and `x86_trap_dispatch` runs `schedule()` one frame out, on the thread's own stack, after the EOI.
  Neither counts in `ROUTED_IRQS`, so a tick cannot satisfy a test waiting for a device.
- **The self-test vectors.** The local APIC delivers a vector to its own core through the ICR, the
  IRR, the ISR and an EOI, so the interrupt is the hardware's and not a function call. aarch64
  proves the same with a software-generated interrupt. RISC-V can raise nothing from S-mode
  (`sip.SEIP` and the PLIC's pending block are read-only), so its tests assert the console UART's
  transmit-empty line.
- **MSI.** An MSI vector is its own intid. The device writes the vector to the local APIC with no
  controller input in between, so no inversion is owed and nothing is masked. The used-ring loop in
  `crates/virtio` decides a request is complete.
- **IO APIC lines.** `irq::intid_of_vector` maps the vector back to the intid that armed it.
  Subtraction alone gives the redirection index, and an index is not a legacy IRQ. COM1's receive
  line, legacy IRQ 4, is the one this was built for: the input driver waits on it rather than
  polling the port.
- **The spurious vector** is never acknowledged. The local APIC sets no in-service bit for it, so an
  EOI would acknowledge whatever genuinely is in service.

A page fault's error code is the most forthcoming of the three architectures. aarch64 reads the
class from `ESR_EL1` and RISC-V derives it by walking the tables. Here the CPU pushes a word:

| bit | set means |
|---|---|
| 0 (`P`) | the page was present, so the hardware found it and refused |
| 1 (`W/R`) | the access was a write |
| 2 (`U/S`) | the access came from ring 3 |
| 3 (`RSVD`) | a reserved bit was set in some entry of the walk |
| 4 (`I/D`) | the access was an instruction fetch |

## Naming an interrupt: vectors and legacy IRQs

On x86 an intid is one of two things. A local APIC source (the reschedule IPI, the two self-test
vectors) has no controller input, so it is named by the vector it raises, and the ICR takes a
vector. An MSI is the same case, more strongly: the device writes to the local APIC's message
address, with no redirection entry, swizzle or `_PRT` anywhere. Everything else is a legacy IRQ,
0..15, which `irq::enable` resolves through the MADT's overrides to a GSI.

The two ranges cannot collide. The local APIC's own sources live at 0x20..0x2f and a legacy IRQ
never reaches 0x20. The vector bands are disjoint by construction:

| vectors | what |
|---|---|
| 0x00..0x1f | the CPU's exceptions |
| 0x20..0x2f | the local APIC's own sources: the timer at 0x20, the reschedule IPI at 0x21, the self-test vectors at 0x22 and 0x23 |
| 0x30..0xbf | IO APIC lines, base plus redirection index |
| 0xc0..0xfe | MSI, a bump allocator |
| 0xff | spurious |

The 8259s are still wired on a PC (`PCAT_COMPAT`). At power-on they raise interrupts on vectors
8..15, the CPU's own exception vectors, so a PIC IRQ 0 reads as a double fault. They also see every
line the IO APIC routes. `irq::init_local_apic` masks both before enabling the local APIC. They are
masked, not remapped, because a live 8259 would deliver a second copy of every routed interrupt.

### Proving a device line at boot

The local APIC timer proves the CPU takes an interrupt it did not ask for. A line from outside the
CPU is a different claim, and `irq::init` proves it with the PIT for about 0.2 s. The PIT is the
right device twice over. `timer.rs` already drives it for calibration, and its legacy IRQ 0 is the
line every PC rewires, arriving as GSI 2. Arming redirection entry 0 for "the timer" would arm the
8259 cascade and produce no interrupts and no error.

### MSI and interrupt remapping

The MSI address is `0xfee0_0000 | (local APIC id << 12)`. It is not memory: the local APIC claims
`0xfee0_0000..0xfef0_0000` on the bus, so a device's ordinary posted write there is an interrupt.
The redirection-hint and destination-mode bits (3 and 2) stay zero: physical destination, no
redirection, the policy `route_gsi` takes. The data is the vector with `Fixed` delivery and edge
trigger. A message is a write that happens once, so there is no line to hold off.

With VT-d interrupt remapping enabled, a remapping unit reads writes to that range as an index into
its remapping table and rejects this format as a malformed request. `helpers/qemu-runner-x86_64.sh`
runs `-device intel-iommu` without `intremap=on`, so DMA is translated and messages pass through.
Firmware leaves remapping off by default. Check this first if MSI stops arriving on a machine whose
firmware turns it on.

### Edge lines and masking

A GIC keeps an interrupt pending while it is disabled. An IO APIC drops an edge that arrives on a
masked entry. COM1's IRQ 4 is edge-triggered on every PC, and a 16550 holds its line high until the
driver drains it. Masking it would turn a byte that arrives between the driver's last read and its
ACK into silence: the line is already high when the entry is unmasked, so no new edge comes. Left
live, the byte raises a fresh edge after the drain. So `irq::disable` masks only a level-triggered
line.

## The GDT, the TSS and the stack on a ring change

aarch64 and RISC-V have no segmentation, and x86_64 has almost none. What the GDT still carries is
the code segment's `L` bit (64-bit), `DPL` (which ring) and `D/B`. The TSS answers a question x86
has no other mechanism for: where the stack pointer comes from when the CPU changes ring. aarch64
lands an EL0 exception on the banked `SP_EL1`, and RISC-V recovers its stack from `sscratch`. x86
reads `TSS.RSP0` from memory, and a wrong one makes the first user-mode fault unrecoverable. An IST
entry switches stacks unconditionally, even for a ring-0 trap.

`syscall` enters with CS = `IA32_STAR[47:32]` and SS = CS + 8. `sysret` returns with
CS = `STAR[63:48]` + 16 and SS = `STAR[63:48]` + 8. So the order is kernel code, kernel data, user
data, user code, and the backwards-looking user pair is the tell. Reordering it for tidiness shows
up as a general protection fault on the way back to a program that did nothing wrong. Three const
assertions in `exceptions.rs` hold the two files in agreement.

Loading a segment register in long mode destroys that segment's base MSR, and `gs`'s base is the
per-CPU pointer (aarch64's `TPIDR_EL1`, RISC-V's `tp`). `segments::init` saves and restores it
around the reload. When the reload once zeroed it, the symptom was an instruction fetch from the
middle of a static, several frames away, with a register dump showing a correct GDT, TSS and IDT.

## Port grants in the TSS bitmap

The TSS holds the 8 KiB I/O permission bitmap, one bit per port of the 16-bit port space, plus one
guard byte. A set bit denies the port. While no port-holding thread runs, `iomap_base` points past
the TSS limit and every ring-3 `in`/`out` faults.

`set_port_range_grant` installs a grant on switch-in, lazily. When the grant is unchanged, which is
nearly every switch on nearly every machine, it costs a load and a branch. That is the property
The refinement of §121 (x86 port I/O) found the naive always-write threw away (about 2,682 ns per switch). When the
grant changes, only the moving bits are written: for a 16550's eight ports, one byte and the
`iomap_base` word. The function is `#[cold]` because its effect is rare though it is called on every
switch. `script/fastpath-footprint` follows non-cold calls out of `schedule()`, and cold keeps this
body out of the IPC fastpath's hot footprint for the price of a call and a compare.

Revocation must reach other cores. A revoked holder can be running elsewhere when its capability is
deleted, and clearing the cached grant under `sched::IPC_TABLES` tells no hardware. At `NIFE_SMP=2`,
`user::x86_port_tests::a_revoked_holder_faults_on_its_next_port_write` measured that window: red
in 7 of 12 full runs, each time with the revoker on one core and the grant installed on the other.
`revoke_port_grant_everywhere` resets this core first and sends the rest an NMI.

`PortRange::REVOKE` deletes the range from every table but the invoker's, and the invoker is the
thread on this core. So `spare_this_cpu` skips the local reset; otherwise the invoker would lose its
own bitmap until its next switch-in and fault on its next `out`. Nothing in the tree invokes the
take-back yet.

## BUGS

- The IO APIC's index and data window are a pair with nothing serializing them. A driver's
  `Irq::ACK` reaches `irq::enable` on whichever core ran the syscall, and the trap handler calls
  `irq::disable` on whichever core took the line. With two cores these can interleave, and each
  reads or writes the other's register.
- One IO APIC: the first the MADT lists. `route_gsi` refuses a GSI a second one owns. The
  nonzero-base arithmetic is proved (`proofs::an_owned_gsi_routes_inside_the_io_apic_band`) and
  unit-tested on a pretend part based at 200, and never booted.
- Every redirection entry targets the core that called `irq::enable`, and every MSI the core that
  ran `alloc_msi_vector`, in physical mode. Nothing distributes interrupts.
- Revoking a driver's `Irq` does not mask its line, so a dead input driver's COM1 line stays armed
  into a rendezvous nobody waits on.
- An MSI vector is never handed back: a device brought up twice spends two of the band's 63.
- Recovering from a correctable machine check is not built; every machine check is fatal.
</content>
</invoke>
