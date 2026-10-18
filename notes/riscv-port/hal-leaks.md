# The HAL leaks the RISC-V port exposed, and how each closed

An appendix to [notes/riscv-port.md](../riscv-port.md). It is the full account of each leak,
including the user-entry frame placement on RISC-V that milestone 71 (thread-start fault) fixed.

The port is worth doing precisely because it finds where the abstraction leaked. Two are already known:

1. `thread::Context` is aarch64-register-shaped in *portable* `thread.rs`. ~~It names `x19`..`x30`,
   the aarch64 callee-saved set, and is a contract with `context.s`.~~ Closed (commit `fdc4376`),
   before RISC-V was started, as an aarch64-only refactor proved against the green baseline. The
   deeper leak was not the field names but the two construction sites, which encoded the register
   *mapping* (`x19`=closure/entry, `x20`=shim/user-sp, `x21`..`x23`=args, `x30`=trampoline). The
   struct, the `switch_to`/trampoline externs, and the frame construction now live in
   `arch/aarch64/context.rs` behind two intent-named constructors, `Context::for_kernel_thread(closure_at,
   call_shim)` and `Context::for_user_thread(entry, user_sp, args)`; the fields are private. `thread.rs`
   names no register. RISC-V implements the same two constructors with its own set (`s0`..`s11` + `ra`,
   `a0`/`a1` for args) and `thread.rs` does not change.

2. The `paging` crate encoded the aarch64 descriptor format. Closed (the trait option, calef's
   call; DECISIONS §17 (second architecture)). `Flags` was aarch64 descriptor bits (`AF`, `SH`, `AP_*`, `PXN`, `UXN`, `NG`,
   MAIR index) and the walk assumed 4 levels. Now `Flags` is a format-neutral capability set (same
   constructor/predicate API). A `PageFormat` trait captures the seam (`LEVELS`, the half split, and
   is-present / extract-address / encode-table / encode-and-decode-leaf). The `Mapper` walk is written
   once and generic over `F: PageFormat`, and `Aarch64` (4-level) and `Sv39` (3-level) each implement
   it. The walk is proved once; each format carries its own Kani proofs of index-in-bounds,
   address/permission separation, and the half split, so RISC-V paging inherits the same formal
   verification aarch64 has. Base Sv39 has no device-memory PTE bit, so `CAP_DEVICE` rides in an RSW
   software bit to keep the round-trip exact. Portable code names the running format as
   `arch::mmu::Format`. aarch64 stayed green throughout (116 kernel tests, 37 paging host tests, both
   `cargo clippy` passes clean). The Sv39 format is defined and proved; wiring the RISC-V `mmu.rs` to
   use it (real kernel tables + `satp` + the high-half) is the remaining MMU-step work.

3. `user.rs` embedded the entire userspace-entry mechanism (the headline leak). Closed. This
   was the big one the compile step surfaced, larger and more delicate than the other two because it
   is the privilege boundary. The nominally-portable `user.rs` directly:
   - constructs an aarch64 `TrapFrame` (`elr`/`spsr`/`sp_el0`, a 31-entry `x`) to drop to EL0, and
     defines the `SPSR_EL0T` constant and the `enter_userspace` extern;
   - carries `sync_icache` with `dc cvau`/`ic ivau`/`dsb`/`isb` inline `asm!` (RISC-V wants a single
     `fence.i`);
   - embeds a corpus of hand-written aarch64 user programs (`USER_HELLO`, the hostile program) via
     `global_asm!` right in the file, a standing rule-1 violation (`asm!`/`global_asm!` outside
     `arch/`) that predates the port.

   The fix was a set of arch seams mirroring how leak #1 was closed:

   - a `TrapFrame::for_user_entry(entry, user_sp, args)` constructor in `arch`;
   - an `arch::sync_icache(va, len)` (aarch64 `dc cvau`/`ic ivau`; RISC-V `fence.i`);
   - an `arch::current_sp()` (a fourth small leak: `stack.rs` read `sp` with inline `asm!`);
   - the `SPSR`/`enter_userspace` items relocated under `arch/aarch64` behind `arch::enter_user`;
   - the embedded aarch64 programs (plus their test and boot-tour consumers) gated to aarch64.

   RISC-V
   reaches U-mode through the ELF-load path, not the hand-written programs. `user.rs` now names no
   register and no arch instruction.

   A sharp lesson from the extraction, recorded because it will bite again. The user-entry
   `TrapFrame` is written onto the *top of the caller's own kernel stack*, overlapping the caller's live
   call frames. It is intact only until `enter_userspace` does `mov sp, x0` provided nothing pushes
   onto the stack in between. The pre-seam code satisfied this because the jump was a direct tail call
   from the frame-writing function. Wrapping it in an ordinary `arch::enter_user` function silently
   broke it: the wrapper's own call frame push corrupted the just-written frame (seen as a child thread
   getting `sp_el0 = 0`, then a translation fault). The exec path survived by luck of stack depth; the
   deeper TCB-child path did not. The fix is `#[inline(always)]` on `enter_user`, which is load-bearing,
   not cosmetic, and is commented as such at the definition. Only the full aarch64 test suite caught
   this; a compile-only check would have shipped it.

   And the RISC-V answer to that lesson was wrong for a year (milestone 71 (thread-start fault), 2026-08-03). The
   paragraph above says the frame goes at the top of the kernel stack. RISC-V did not do that. Its
   TCB entry path (`user_entry_trampoline` -> `user_thread_entry` -> `enter_frame`) is shallow enough
   that a frame at the top would have overlapped `enter_frame`'s own stack, so the port computed the
   address from the live `sp` instead:

   ```rust
   let slot = (crate::arch::current_sp().min(top) - size_of::<TrapFrame>() as u64) & !15;
   ```

   That trades a deterministic overlap for an intermittent one, and the intermittent one is worse
   because everything below `sp` belongs to somebody else. Two facets, both real:

   - `current_sp()` is a real call at opt-level 0 with a 16-byte frame, so it returns `sp - 16`
     and the frame landed at `sp - 304`. `trap.s` builds an S-mode trap frame at `sp - 288`. The two
     differ by exactly 16 bytes, so the user frame's fields sat on the trap frame's fields shifted by
     two register slots:

     | user frame field | offset | reads, after a trap | value |
     |---|---|---|---|
     | `x[2]`, the user `sp` | +16 | trap `x[0]` | **literally 0**, `trap_entry` does `sd zero, 0*8(sp)` |
     | `sepc` | +256 | trap `x[30]` | `t5`, which is 0 only sometimes |
     | `sstatus` | +280 | trap `scause` | UXL = 0, an illegal U-mode XLEN |

   - any call from `enter_frame` after the frame is built lands in the same region. There are
     none in the shipped code, but the investigation's own instrumentation tripped over it twice:
     `core::hint::spin_loop()` and `ptr::write_volatile` are calls at opt-level 0, and their 48-byte
     frames landed on the user frame's `sepc` and `stval`.

   The two faces of the fault, and why one guard cannot see both. The `sepc == 0` guard in
   `enter_user` fires only in the subcase where `t5` happened to be 0. Every other time the thread
   `sret`s to a garbage PC with an illegal `sstatus`, dies on its first instruction, and never answers
   whoever was waiting on it. That is a lost-wakeup hang carrying no guard message at all, which
   is why a CI job can hang in the watchdog with the guard compiled in, live, and silent. A run that
   does not print the guard's message is not a run in which this did not happen.

   Which threads were exposed, which is narrower than it looks. The clobber needs an interrupt,
   and the two user-entry paths differ on that. A thread started through the TCB surface arrives at
   `user_entry_trampoline` from `schedule()`, which masked interrupts and does not restore them
   before the `sret`, and `user_thread_entry` never unmasks (unlike `thread_entry`, which does, right
   after `finish_switch`). So the TCB path ran `enter_frame` with `sstatus.SIE` clear and could not be
   hit. The exec path could: `spawn` makes an ordinary kernel thread, `thread_entry` enables
   interrupts, and that thread later becomes a user process through `run` -> `enter_frame`. Every
   `spawn_bare` subject and `spawn_elf_child_builder` is on that path.

   The two paths are not distinguishable from a panic message, and an early reading of this
   investigation got it wrong by trying. A TCB child configured at `CODE_VA` 0x40_0000 with
   `STACK_VA + FRAME_SIZE` and an exec child at an ELF base of 0x40_0000 with `USER_STACK_TOP` print
   the same two numbers. Read the path from the test, not from the entry and stack values.

   The fix is a reservation, not a moving target: `user_entry_trampoline` drops `sp` by a frame's
   worth before the first Rust frame exists, so the region is off-limits to the entry path by
   construction, and `enter_frame` uses `top - size_of::<TrapFrame>()` on both ISAs. aarch64 took the
   same reservation: its overlapping slots only happen to be dead by the time `frame.write` runs, and
   "happens to be dead" is not an invariant. `enter_frame` now asserts the slot is at or above the
   live `sp`, which the old placement failed on the first user entry.

   It unstranded two things. `user_pc` on RISC-V was a stub returning 0 because there was no
   fixed address to read; it is now the aarch64 twin. And on the exec path the old placement left the
   user thread with only the kernel stack that happened to lie below `enter_frame`'s `sp`, with
   everything `run` and `load` had consumed stranded above the frame and never reclaimed.

   How it was proved, since it had never reproduced locally in nine full runs: widening the window
   with a call-free spin between `frame.write` and `enter_user` reproduced it on the first run,
   deterministically, printing `sp 0x0 want 0x501000`. The same probe with the fix in place never
   fired.

   BUGS. This closes the `sepc == 0` fault. It does not close every lost-wakeup hang in the
   RISC-V suite, and one of them is on the record as surviving this reasoning. A documentation-only
   PR (#21, zero lines of code) hung the watchdog on
   `reclaim_frees_a_started_then_exited_childs_regions`, with the guard compiled in and silent. That
   test's child is started through the TCB path, which is the one that runs `enter_frame` with
   interrupts masked and therefore cannot take the clobber described above. Its child is
   `REPORT_STUB`, a `SEND` to an endpoint the test is `ipc_receive`ing on, so a hang there reads as an
   IPC rendezvous that missed rather than as a frame that was overwritten. What the section above
   does settle is the *inference*: a silent run is not a run in which the frame fault did not happen,
   because the guard sees only the `t5 == 0` subcase.

   That hang is closed, and it was never a RISC-V bug (milestone 72 (lost wakeup)). One line of test code
   probed `reclaim_region(...).is_err()` on the child's own TCB region, which under DECISIONS §16 as
   amended *arms the kill* on the child; the child was then reaped at its next preemption, before it
   could `SEND`. Widening the window reproduces it on aarch64 as deterministically, which is
   the control that rules the ISA out. Full account in
   [notes/scheduler/reclaim-lost-wakeup.md](../scheduler/reclaim-lost-wakeup.md). The dump that made
   it legible is still owed to `user_pc` no longer returning 0 here, which is the one thing this
   section can claim.

A related, smaller ABI leak the traps step resolves: `syscall.rs` reads the syscall number from
`frame.x[8]` and args from `frame.x[0..]`, the aarch64 `svc`+`x8` convention. RISC-V's `ecall` ABI
puts the number in `a7` and args in `a0`..`a5`; the RISC-V `TrapFrame` compiles today but the index
convention has to be reconciled when the trap dispatcher is real.

Finding these is the point; each gets pushed under `arch/`.
