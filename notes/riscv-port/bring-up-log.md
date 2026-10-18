# The RISC-V port's incremental plan, step by step

An appendix to [notes/riscv-port.md](../riscv-port.md), moved there on 2026-10-11 (UTC) under §212
(a prose budget). Each step was a provable, committable piece, and each is recorded here with the boot line that proved it and the bugs it surfaced.

1. Compiles for `riscv64`: done. The full arch contract stubbed (pure primitives real, every
   deferred piece a loud `unimplemented!()`), all four leaks resolved, the build wiring in place.
   `cargo build --target riscv64imac-unknown-none-elf` links a RISC-V ELF, and both `cargo clippy`
   passes (aarch64 and riscv64) are clean under `-D warnings`. aarch64 stays fully green: 116 kernel
   tests pass. This proves the boundary is complete: a second architecture compiles against the entire
   kernel with no change above `arch/` (the four leaks were the exceptions, now closed). What is *not*
   yet done is running: every `unimplemented!()` in `arch/riscv64` is real work for the steps below.
2. Boots and prints: done. OpenSBI hands off in S-mode; `_start` sets `sp` and zeroes `.bss`.
   `kernel_main` sets the `tp` per-CPU register, brings up the NS16550 console (a plain-volatile byte
   UART, `drivers/ns16550.rs`, selected by a compile-time alias in `console.rs`), and prints a banner
   including the real device-tree pointer OpenSBI passed in `a1`. That one line exercises the whole
   chain: S-mode entry, stack, `.bss`, `tp`, `interrupts::disable` (the console lock uses it), the
   UART, and the DTB handoff. Then a clean `wfi` halt. The milestone-1 moment on a second ISA. A fifth
   leak surfaced and was deferred, not fixed. Portable code (`sched`, `smp`, `syscall`, `user`) named
   `drivers::gic` directly, an interrupt-controller coupling that the traps step resolves with a PLIC
   abstraction. Until then the GIC driver stays un-gated (it compiles on riscv and is dead).
3. Traps: done. `stvec` + the `TrapFrame` + trap.s (save 36 registers, dispatch on `scause`,
   restore, `sret`). The syscall-ABI leak is resolved: `syscall.rs` reads the number and args through
   `TrapFrame::{syscall_nr, arg, set_arg}`, so `ecall`'s a7/a0..a5 map without portable code naming a
   register. Proven by a boot self-test: an `ebreak` is caught, `sepc` stepped past it, and `sret`
   returns. First cut is S-mode traps on the current stack; the `sscratch` stack switch for U-mode
   traps arrives with the user path.
4. MMU (Sv39): done on the kernel side. It took the paging-format work (leak #2) and the
   higher-half boot transition (calef chose high-half, DECISIONS §17 (the second architecture); proven by the banner's live code
   address `0xffffffc0_8020_xxxx`). It also took the fine-grained W^X kernel tables (`mmu::init` via `Mapper<_, _,
   Sv39>`, replacing the coarse RWX boot table, `satp` switched live with the console surviving), and
   the kernel mapping surface (`map_page`/`unmap_page`/`translate`/`flush_tlb`, proven by a
   map/write/read/unmap self-test). Remaining: the *user*-mapping surface and per-process `satp`
   (the user path). The RISC-V single-`satp` model means every process root shares the kernel's
   high-half top-level entries; `reserved_root`/`switch_user_root` already implement the kernel-thread
   side of that.
5. Timer and interrupts: the timer is done. SBI TIME `set_timer` + `sie.STIE`, the dispatcher routes
   `scause` = timer to `timer::tick`; proven by ~17 ticks in 0.2 s at 100 Hz. Remaining: the PLIC
   (external/device interrupts), which resolves the `drivers::gic` leak; it is exercised by the
   userspace-driver path, so it lands with that.
6. The capability core runs: done, and a user program runs at U-mode. The scheduler and context
   switch run on RISC-V ("2 of 2 kernel threads ran"). The user-mapping surface and the single-`satp`
   model work (a process `satp` is installed, the kernel survives via `share_kernel_half`, a user page
   maps and translates); and a hand-written RISC-V program runs at U-mode and makes syscalls:
   "a program ran at U-mode and made 3 syscalls (yield/yield/exit via ecall)". That exercises the
   whole path: `enter_user`'s `sret`, the `sscratch` U-mode trap entry, `ecall` dispatch through the
   ABI accessors, and the return to U-mode, twice, then `exit`.

   The last-mile bug and its root cause, a genuine HAL lesson: `tp` is a general register on
   RISC-V, not a system register. aarch64 keeps the per-CPU pointer in `TPIDR_EL1`, a system
   register that survives an EL0 round trip untouched. RISC-V's `tp` (x4) is an ordinary GPR, so the
   user trap frame left it 0, the `sret` gave U-mode `tp = 0`, and the ecall trap handler ran with
   `tp = 0` and null-dereferenced `cpu::current()`. That panicked. The panic handler then re-panicked
   while *formatting* the message (a `core::fmt` path), recursing into itself and blowing the kernel
   stack. That is what presented as the mystery "store fault at the frame page" (the nested-fault
   cascade of the overflow, unrelated to the frame's mapping, which was fine all along). QEMU's
   `-d int` exception log pinned it: the recursion was in `rust_begin_unwind`, and a raw-UART dump of
   the panic location (bypassing `core::fmt`) named `cpu.rs:160`. Fix: `for_user_entry` carries the
   kernel `tp` in the frame, so it survives the round trip. Follow-up (noted, not yet done): that
   leaks the kernel per-CPU address into U-mode's `tp`; the leak-free fix restores `tp` in `trap.s`
   from a per-hart source (the standard sscratch-trapframe approach), needed for SMP.

   IPC and capabilities work too. A second program is built from parts (a TCB, a code and stack
   mapping, an endpoint capability granted in slot 0) and started. It `SYS_INVOKE`s that cap to SEND a word home,
   which the kernel receives: "a U-mode process invoked a cap and SENT 0xc4". That is the whole
   capability boundary (cap lookup, rights check, IPC) from U-mode on RISC-V. One RISC-V-specific bug
   surfaced there. The TCB entry path is *shallow* (trampoline → `user_thread_entry` → `enter_frame`).
   A trap frame placed at the top of the kernel stack (fine for aarch64, whose entry paths are
   deep) overlapped and corrupted `enter_frame`'s own frame as `frame.write` ran, sending the `sret`
   to a garbage `sepc`. Fixed by placing the RISC-V trap frame below the live `sp` (`sscratch`
   tracks it, so re-entries stay consistent).

   Preemption works too, and it is the property that separates a kernel from a runtime. The
   S-mode timer already fired every 10ms; the missing half was hanging a reschedule off it. Now
   `riscv_trap_dispatch` records the tick (`sched::on_tick`) and defers the switch to the trap tail,
   the same four lines as aarch64's `handle_irq`: `if take_need_resched() && is_running() {
   count_preemption(); schedule(); }`. Two threads whose entire body is a tight loop (no yield, no
   syscall, not even a call) both make progress: ~680k and ~666k iterations in 0.2s, 18 preemptions.
   Under any cooperative scheduler the first would own the CPU forever. This is the RISC-V half of
   DECISIONS §5 (preemptive threads with real stacks). It works for a U-mode thread and an S-mode kernel thread alike, because `trap.s`
   saved a full frame for whichever the timer interrupted, and `schedule()` returns through
   `trap_return` to exactly the instruction it left.

   A real compiled ELF runs at U-mode too. Every user program above is a hand-written
   machine-code blob. The `worker` program is a Rust binary compiled to a riscv64 ELF, delivered as
   the initrd (QEMU `-initrd`, read from `/chosen/linux,initrd-start` the same way Linux gets its
   initramfs). It runs through the kernel's *real* ELF loader: `user::load` parses the file, builds an
   address space with each `PT_LOAD` segment mapped W^X at the VA it names, and maps a stack. The
   worker is granted WRITE on one endpoint (slot 0), started with an input in `a1`, squares it, and
   SENDs the answer home: "loaded a 13568-byte riscv ELF, ran worker(7) at U-mode, it sent 49". Three
   small pieces made this work, each closing an aarch64 assumption. `user_mode_runtime` (the userspace
   syscall runtime) grew a RISC-V ABI (`ecall`+`a7` beside `svc`+`x8`). The `elf` crate accepts the
   running kernel's machine (`EXPECTED_MACHINE`, cfg-selected, symmetric: each kernel refuses the
   other's ELF). And `worker.rs` arch-gated its one trap instruction (`ebreak` vs `brk`). The loader itself
   (`load`, `map_segments`) was already arch-neutral and needed no change. This is the first RISC-V
   userspace program the kernel did not hand-write.

   And userspace init builds the system, from a richer initrd. The single-ELF initrd became a
   nifefs archive holding two programs: `init` (the portable `builder`) and `worker`. The kernel
   (`riscv_initrd_demo`, a minimal `spawn_progenitor` with no GIC/PL011/IRQ baggage) loads only
   `init` and maps the whole archive read-only into it. `spawn_elf_child_builder` is that function's
   descendant. It grants `init` two capabilities: an untyped budget and a report endpoint. From those, `init` reads `worker` out of the archive by name. It builds it as a child
   *entirely from its own budget* through the capability verbs: retype an address space, copy each
   segment into retyped frames and map them, retype a TCB, endow it, configure, start. It hands the
   child a WRITE view of the report endpoint, and starts it with an input. The child squares it and
   SENDs `81` straight home; the kernel never parsed or mapped the worker. This exercised the *full*
   userspace capability syscall surface on RISC-V for the first time (untyped RETYPE/RETYPE_OBJ,
   aspace MAP_INTO, frame MAP, tcb CAP_INSERT/CONFIGURE/START), all across the `ecall` ABI. `builder`
   is fully portable; it builds for aarch64, though aarch64's init is the PL011-wired role of
   `hello`. Build the archive with `cargo xtask initrd-riscv`.

   And a real device interrupt flows through the PLIC. A keystroke becomes a message. The
   NS16550 raises its receive line into the PLIC, and the PLIC delivers a supervisor external
   interrupt (`scause` = 9). `riscv_trap_dispatch` claims it (acknowledging and masking that source),
   looks up the endpoint it is routed to, and notifies it. A kernel driver thread blocked there wakes, reads
   the byte, and re-arms the source. Boot shows "a keystroke reached a kernel driver through the
   PLIC: 0x41 ('A')". The shape is exactly aarch64's `handle_irq` (mask, notify, complete) and it
   reuses the arch-neutral routing (`bind_irq` / `irq_route` / `irq_notify`) unchanged. The PLIC's
   base comes from the device tree, which exposed one parser bug: `node_reg` only matched nodes at
   depth 2 (the aarch64 GIC is a direct child of the root), but the riscv PLIC is at
   `/soc/plic@c000000`, depth 3. `node_reg` now matches at any depth and decodes `reg` with the
   parent's cell counts. Two test caveats: pipe the byte *after* boot (the console clears the RX FIFO
   during init), and the demo driver runs in the kernel for now.

   The driver moved to userspace, and the last leak closed. First the demo driver became an
   unprivileged U-mode process (`components/src/serial_driver.rs`). The kernel maps the NS16550 into it
   device-typed and grants it an `Irq` capability and a report endpoint. It runs the seL4
   IRQHandler loop, `WAIT` on the cap (block for the interrupt as a message), read the byte through
   its own device mapping, `SEND` it, `ACK`. "an unprivileged userspace driver serviced the UART via
   its Irq cap and sent 0x5a". The kernel is never in the data path. That exercised the `Irq`
   capability `WAIT`/`ACK` from RISC-V U-mode for the first time, and the `ACK` re-arm is what forced
   the interrupt-controller seam into being.

   Then `arch::irq` was extracted with two working consumers behind it (the GIC and the PLIC), and
   `drivers::gic` was gated to aarch64. Every portable caller (`sched::place_on`, `smp`,
   `user::spawn_elf_child_builder`, `main::interrupts_init`, the `Irq` ACK) now names `arch::irq`, not a
   controller. The only code still naming `drivers::gic` is aarch64 arch code (rule #1 lets arch name
   its own driver) and the `cfg(test)` aarch64 IRQ tests. That was the last HAL leak. A new ISA is
   a new `arch/` directory, not a diff across the kernel, with zero exceptions in portable non-test
   code.

   The port is complete. Boot, MMU, traps, timer, scheduler, preemption, U-mode user programs,
   capability invocation, userspace-built processes (init loads and builds children), and device
   interrupts serviced by a userspace driver all run on RISC-V, the same portable core as aarch64.
