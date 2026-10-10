# The thread pointer, and the one asymmetry below its ABI

*Scope note for milestone 812 (`std::thread::spawn` runs real threads in one address space), written
2026-10-10 (UTC) by its lane, as §269 (how threads share a process) fork 4 asks. It plays the role
for the thread pointer that §139 (who may read the cycle counter, and by what authority) plays for
`rdtsc`: the ABI is one, the hardware is not, and this says where they part.*

## The ABI, which is the same on all three architectures

A thread's thread pointer is the register its thread-local storage hangs off: `TPIDR_EL0` on
aarch64, `tp` on riscv64, the `FS` base on `x86_64`. The kernel keeps one value per thread
(`Thread::thread_pointer`) and is the only party the contract lets set it:

- `ThreadControlBlock::CONFIGURE` takes the first value in the sixth argument register (Linux's
  `CLONE_SETTLS`). Zero means none, and every caller built before 812 sends zero.
- `ThreadControlBlock::SET_THREAD_POINTER` changes it later (seL4's `SetTLSBase`), on an embryo or on
  the caller itself. Any other started thread is refused with `WrongObject`.
- A value outside the user half is refused with `BadPointer` on every architecture, because
  `x86_64`'s `wrmsr` faults in the kernel on a non-canonical one.
- The kernel installs the value whenever the thread is switched in, so no thread ever sees a
  sibling's.

`system_tests/src/user/thread_pointer_tests.rs` proves those four properties from user mode on all
three, with a falsification patch that turns it red on all three.

## The asymmetry: two of the three let a program write its own register

| | Can user mode write it? | What the kernel does at a switch |
|---|---|---|
| aarch64 | yes, `msr tpidr_el0` at EL0 | saves the outgoing register, installs the incoming one |
| riscv64 | yes, `tp` is a general register | nothing; `tp` rides the trap frame both ways |
| `x86_64` | no: `CR4.FSGSBASE` is off, and there is no `arch_prctl` | installs the incoming value, lazily |

**A user write is not part of the contract.** Where the hardware allows one, it persists for that
thread and reaches no other, because the kernel saves what the thread left there (aarch64) or the
trap frame does (riscv64). A program that writes the register therefore works on two architectures
and cannot be written on the third. That is the gap, and nothing in this tree writes the register:
the `std` PAL sets it through the kernel on all three.

**Why `x86_64` is the strict one, and stays so.** Turning `CR4.FSGSBASE` on would give ring 3
`wrfsbase` and also `wrgsbase`, and a user-chosen `GS` base reaches the kernel's entry path, whose
NMI exit window has no paranoid path yet (`kernel/src/arch/x86_64/trap.s`). calef ruled on parity
over that convenience: *"I am hung up on parity."*

**Why riscv64 is not made strict.** It cannot be: `tp` is an ordinary register.

**Why aarch64 is not, though it could be.** `TPIDRRO_EL0` is EL0-readable and EL1-writable, and
LLVM will use it for TLS under the `tpidrro-el0` target feature (`rustc --print target-features`,
checked 2026-10-10 UTC), so the `std` target could take thread-local storage off the writable
register and leave riscv64 the only gap. This lane did not, for one reason: `TPIDR_EL0` would still
be an EL0-writable register, and if the kernel stopped managing it, it would again be shared by
every thread on a core, which is a channel between processes (a program writes it, the next program
on that core reads it). That was the state of the kernel before this milestone. Closing it as well
means switching two registers instead of one. The choice is reversible until the `std` target ships
with the feature off, and it is put to calef on pull request #1892.

## What would close it

Nothing planned. If `x86_64` ever needs user-written thread pointers (a green-thread runtime that
switches `FS` per task), the path is a paranoid NMI exit plus `FSGSBASE`, which is its own milestone
and its own ruling.

## BUGS

- `SET_THREAD_POINTER` cannot reach a started thread other than the caller. seL4's can. Nothing in
  the tree needs it, and lifting the refusal is additive.
- On aarch64 the kernel's copy lags the register while the thread runs, by design. A reader of
  `Thread::thread_pointer` for a running aarch64 thread gets the value at its last switch in, not
  what it may have written since.
