# Porting nife to RISC-V (the second architecture)

The point of a second port is not to reach RISC-V. It is to prove rule #1: that all
architecture-specific code lives under `kernel/src/arch/`, an assumption maintained on faith since
milestone 1. A genuinely different ISA is the first real test of it. RISC-V (rv64, `qemu-system-riscv64
-machine virt`) is chosen over x86_64 because it is clean-different rather than legacy-different. It
exercises the HAL abstraction (a different trap, paging, interrupt, and firmware model) without the
real-mode / GDT / IDT / APIC / UEFI tax that would make most of an x86 port about x86 plumbing, not
about the abstraction. See the reasoning in the roadmap (milestone 20). x86_64 is the third port.

The detail behind this note is in [riscv-port/](riscv-port/README.md).

## The arch boundary RISC-V must satisfy

`arch/mod.rs` dispatches by `#[cfg(target_arch)]` and re-exports the arch module flat as `crate::arch`.
A RISC-V arch adds `#[cfg(target_arch = "riscv64")] mod riscv64; pub use riscv64::*;`. The contract the
rest of the kernel calls through `crate::arch`:

- Top-level: `set_percpu`/`percpu` (the per-CPU pointer), `psci_cpu_on` (start a secondary),
  `init`, `halt`, `wait_for_interrupt`, `direct_memory_access_write_barrier`.
- `mmu`: `KERNEL_VA_BASE`, `phys_to_virt`/`virt_to_phys`, `init`/`init_secondary`, the user-mapping
  surface (`map_current_user_page/frame`, `unmap_user_at`, `translate_at`, `map_page`/`unmap_page`,
  `activate_user`/`deactivate_user`, `switch_user_root`, `reserved_root`, `flush_asid`, `flush_tlb`,
  `user_can_read`/`write`, `current_user_root`, `translate`/`translate_user`, `is_enabled`), and the
  `VIRTIO_MMIO_BASE`/`SIZE`/`IRQ_BASE` consts.
- `interrupts`: `enabled`/`disable`/`restore`/`enable`.
- `timer`: `TIMER_INTID`, `TICK_HZ`, `init`, `tick`, `ticks`, `now`, `frequency`, `uptime_ms`,
  `spin_for`, `interval`, `missed_ticks`.
- `exceptions`: the `TrapFrame` type `syscall::dispatch` consumes, the fault stat statics, `init`.
- `semihosting`: `EXIT_SUCCESS`/`EXIT_FAILURE`, `exit` (the test-harness exit).
- Assembly symbols: `switch_to`, `thread_trampoline`, `user_entry_trampoline`, `secondary_boot`.

## The two HAL leaks the port exposes (and must fix)

The port is worth doing precisely because it finds where the abstraction leaked. It found five,
and every one is closed. The full account of each is in
[riscv-port/hal-leaks.md](riscv-port/hal-leaks.md).

1. `thread::Context` was aarch64-register-shaped in portable `thread.rs`. It now lives in
   `arch/*/context.rs` behind `Context::for_kernel_thread` and `Context::for_user_thread`, and
   `thread.rs` names no register.
2. The `paging` crate encoded the aarch64 descriptor format. A `PageFormat` trait now carries the
   seam, the `Mapper` walk is written once, and `Aarch64` and `Sv39` each implement it with their
   own Kani proofs (DECISIONS §17 (second architecture)).
3. `user.rs` embedded the whole userspace-entry mechanism: an aarch64 `TrapFrame`, cache
   maintenance `asm!`, and hand-written aarch64 programs. Arch seams (`TrapFrame::for_user_entry`,
   `arch::sync_icache`, `arch::current_sp`, `arch::enter_user`) replaced them. `enter_user` must be
   `#[inline(always)]`, because the user-entry frame sits on the caller's own kernel stack.
4. The syscall ABI read `frame.x[8]`; it now goes through `TrapFrame::{syscall_nr, arg, set_arg}`.
5. Portable code named `drivers::gic`; it now names `arch::irq`, with the GIC and the PLIC behind it.

Where the user-entry frame goes was wrong on RISC-V for a year (milestone 71 (thread-start fault),
2026-08-03). The port placed it below the live `sp`, where a trap frame could overwrite it, and the
thread then returned to a garbage PC and hung silently. The fix is a reservation:
`user_entry_trampoline` drops `sp` by a frame before any Rust frame exists, and `enter_frame` asserts
the slot sits at or above the live `sp`. `thread_trampoline` does not reserve. Only the TCB
entry path can be shallow; the exec path reaches `enter_frame` through `run` and the ELF loader, far
below the stack top, and a reservation there would spend a frame on every kernel thread. The lost-wakeup hang once blamed on it was a test bug, not a
RISC-V one (milestone 72 (lost wakeup)).

Finding these is the point; each gets pushed under `arch/`.

## RISC-V specifics (the clean-different)

- Boot: OpenSBI runs in M-mode and hands the kernel control in S-mode. No arm64 `Image` header;
  `-kernel <elf>` boots the ELF directly. The hart id arrives in `a0`, the DTB physical pointer in
  `a1` (aarch64 put the DTB in `x0` and needed the Image header to get QEMU to pass it at all).
- Firmware ABI = SBI (the PSCI analog): HSM (hart state management) `sbi_hart_start` for SMP
  bring-up (replaces `psci_cpu_on`). TIME for the timer; DBCN/legacy console for the earliest
  prints before the UART driver exists; SRST (system reset) for the test-harness exit (replaces
  ARM semihosting `exit`).
- Per-CPU: the `tp` register (thread pointer), the direct analog of `TPIDR_EL1`.
- Traps: a single `stvec` vector (vs aarch64's 16-slot `VBAR` table), with `scause` (cause),
  `stval` (faulting value), `sepc` (return PC). The `TrapFrame` holds the RISC-V GPRs. Interrupt vs
  exception is the top bit of `scause`; the syscall path is the `ecall` cause.
- Interrupts: masked via `sstatus.SIE` (vs `PSTATE.DAIF`); enabled per-source in `sie`, pending in
  `sip`. The external-interrupt controller is the PLIC; software/timer interrupts come from the
  CLINT (or the newer Sstc extension). Both sit under `drivers/`, like the GIC does today.
- Timer: the `time` CSR + `stimecmp` (Sstc extension) or CLINT `mtimecmp`, or SBI TIME. Replaces
  the ARM generic virtual timer (`CNTV_*`).

  Worth knowing before choosing SBI TIME on the next board: SBI `set_timer` takes an absolute
  deadline but is write-only, unlike `CNTV_CVAL_EL0`, which can be read back. So the fixed deadline
  grid that keeps the tick rate honest is kept in software. A handler that re-arms from `now()`
  runs the clock at 80 Hz against a configured 100 Hz and nothing says so. See
  notes/riscv-arch-tests.md.
- UART: QEMU virt's console is an NS16550 at `0x1000_0000`, not a PL011. A new
  `drivers/ns16550.rs` (the PL011 driver stays; this is a sibling, like a second board's UART).
- **Paging:** Sv39 (three-level, 39-bit VA) to start, `satp` holding the root PPN + mode. The
  high-half direct map uses Sv39's top VA range (sign-extended); `KERNEL_VA_BASE` is chosen to fit it.

## The per-hart `tp` hazard: a preempted kernel thread must not carry its origin hart's pointer

`tp` being a *general register* (line above), not a system register, is the one place the "clean
different" bites. `TPIDR_EL1` is banked per core and no thread ever saves or restores it; a thread
that migrates from one aarch64 core to another reads whatever `TPIDR_EL1` the destination core
set at boot. RISC-V has no such register. The kernel per-CPU pointer lives in `tp`, and the trap
entry saves the full GPR set, `tp` included (`x[4]`), because a trap from U-mode arrives with the
*user's* `tp` and the frame has to preserve it.

That save is correct. The naive *restore* is not. Reloading `x[4]` unconditionally in
`trap_return` is right for a return to U-mode (the user owns `tp`), and harmless for a kernel thread
only while a kernel thread always resumes on the hart it was preempted on. DECISIONS §28 (SMP placement) breaks
that assumption: idle harts steal runnable threads, so a kernel thread preempted on hart A (its frame
holding `tp = &percpu[A]`) can resume on hart B. An unconditional restore then loads `&percpu[A]`
while running on B, and every `cpu::current()` after that reads hart A's per-CPU block: its
`current` tid, its `idle`, its run queue, its `held_rank`.

The failure is spectacular and non-deterministic. A migrated worker, thinking it is on A, calls
`exit()` and marks *hart A's* current thread (often A's idle) `Finished`; A's next `schedule()` then
panics "the last thread exited". Other interleavings trip the rank-`assert!` in `sync.rs` (a bogus
`held_rank` read from the wrong hart) or lose a thread and hang. Adding tracing moves it, the
classic weak-ordering-adjacent tell, though the root cause is not a missing barrier: it is reading a
hart-local register restored from another hart's frame.

**Fix (in `trap.s`, the return path):** restore `x[4]` only for a return to U-mode. For an S-mode
(kernel) return, keep the live `tp`. It is already this hart's: `trap_entry` reloaded it from the
per-hart stash (`sscratch`), and `switch_to` never touches `tp`, so it survives a migration intact.
This is the exact mirror of `trap_entry`'s `ld tp, 8(t0)`. The regression test
(`smp::tests::a_migrated_kernel_thread_keeps_its_hart_pointer`) drives the steal-a-preempted-thread
interleaving and checks the invariant directly: `sscratch` names this hart's `TrapStash` and never
migrates, so `(sscratch → hart) == cpu::id()` must hold on every spin; the aarch64 twin of the check
is a constant `true`. The lesson generalizes: anything the kernel keeps in a general register is
part of the thread context on RISC-V, and the trap path must decide per privilege level whether it
is the thread's or the hart's. `tp` is the hart's.

## Build setup

- **Target:** `riscv64imac-unknown-none-elf`, the integer-only target, the analog of aarch64's
  `-softfloat` (no FP state in the kernel). It runs on QEMU virt's rv64gc CPU (imac is a subset).
- **Linker:** `link-riscv64.ld`. RAM base is `0x8000_0000`; OpenSBI loads the payload at `0x8020_0000`.
  It links high and loads low (`AT()`), like the aarch64 script: the kernel lives in the Sv39
  high half, and `boot.s` does the higher-half transition. (An earlier cut linked low / bare-mode for
  the first boot+console; that was replaced when high-half landed.)
- **Wiring:** `build.rs` selects the linker script by `CARGO_CFG_TARGET_ARCH`; `.cargo/config.toml`
  adds a `[target.riscv64imac-unknown-none-elf] runner` pointing at a new `helpers/qemu-runner-riscv64.sh`
  (`qemu-system-riscv64 -machine virt -bios default -kernel <elf> -smp N -serial stdio ...`); `xtask`'s
  hardcoded `TARGET` becomes arch-parameterized; `rust-toolchain.toml` lists the target.

## The incremental plan (each a provable, committable piece)

Each step was a provable, committable piece, and all six are done. The boot line that proved each
one, and the bugs each surfaced, are in [riscv-port/bring-up-log.md](riscv-port/bring-up-log.md).

1. Compiles for `riscv64`, with the full arch contract stubbed and no change above `arch/`.
2. Boots and prints: OpenSBI hands off in S-mode and the NS16550 console prints a banner.
3. Traps: `stvec`, the `TrapFrame` and `trap.s`, proven by a boot self-test that catches an `ebreak`.
4. MMU: Sv39 kernel tables, W^X, high half, and the kernel mapping surface.
5. Timer and interrupts: SBI TIME at 100 Hz, then the PLIC for device interrupts.
6. The capability core: U-mode programs, capability invocation, preemption, a compiled ELF, an init
   that builds its children from its own budget, and a userspace UART driver on an `Irq` capability.

The port is complete. Boot, MMU, traps, timer, scheduler, preemption, U-mode user programs,
capability invocation, userspace-built processes (init loads and builds children), and device
interrupts serviced by a userspace driver all run on RISC-V, the same portable core as aarch64.

The reward beyond the proof: when a real RISC-V board (a ~$70 StarFive VisionFive 2 / Milk-V Mars, or a
rented Graviton later) is on hand, the QEMU-virt work transfers. Real boards use the same
OpenSBI + S-mode + device-tree model. And "one verified capability microkernel, aarch64 and RISC-V" is
a stronger demonstrator line than any single board.
