/// `invoke(cap, CONFIGURE, entry, user_sp, address_space_slot)` -> the thread's current-CPU page
/// address, never negative (zero if none could be given). Bind the space in `address_space_slot`
/// and set the EL0 entry and user stack of an embryo TCB. Needs `WRITE` on both capabilities.
/// Semantics: §276 (the thread syscalls as built); see notes/abi/method-contracts.md.
///
/// - **The capability is consumed; the space's name is not** (§249 (a running address space stays
///   nameable)). A space that already has a holder is refused with `WrongObject`.
/// - **A process capability with [`rights::BIND`](super::rights::BIND)** instead joins that process,
///   unconsumed; `OutOfMemory` when it already has `current_cpu_contract::THREAD_PAGE_SLOTS` threads.
/// - Argument register 6 is the first thread pointer: zero or a user address, else `BadPointer`.
pub const CONFIGURE: u64 = 0;

/// `invoke(cap, CAP_INSERT, cap_slot, rights, target)` -> `child_slot`. Copy the capability in
/// the caller's `cap_slot`, narrowed to `rights`, into the child's capability table, returning the slot
/// it landed in. The child's whole initial authority is built this way, one grant at a time,
/// before it runs. Needs `WRITE` on the TCB cap and `GRANT` on the inserted capability.
///
/// `target` chooses where the capability lands: **0 places it in the first free slot** (the
/// original behavior, so every existing caller is unchanged), and `n` places it in slot
/// `n - 1`. The explicit target exists for the supervision endpoint, which a supervisor puts
/// in the reserved [`fault::FAULT_EP_SLOT`](super::fault::FAULT_EP_SLOT) rather than wherever
/// first-free happened to fall. `OutOfMemory` if the chosen slot is occupied or out of range.
pub const CAP_INSERT: u64 = 1;

/// `invoke(cap, START, x0, x1, x2)` -> 0. Make the thread runnable: it gets a kernel stack and
/// an entry context and joins the run queue. **The three arguments are the child's first
/// `x0`, `x1` and `x2`** (`a0` to `a2` on RISC-V, `rdi`, `rsi` and `rdx` on `x86_64`), passed
/// through unconditionally, which is how a builder hands a worker its input
/// (milestone 19 (run a real workload), phases d and e). A caller with nothing to pass sends
/// zeros. **Refuses a half-built thread** (no bound address space, or no entry): a TCB must be
/// whole before it runs. Needs `WRITE`.
pub const START: u64 = 2;

/// `invoke(cap, SET_THREAD_POINTER, value, _, _)` -> 0 (milestone 812, §269 (how threads share
/// a process) fork 4; seL4's `SetTLSBase`). **Change the thread pointer of the thread this TCB
/// names**: `TPIDR_EL0` on aarch64, `tp` on riscv64, the `FS` base on `x86_64`. Needs `WRITE`.
///
/// - The target is an embryo, which takes the value when it first runs, or the caller, which takes
///   it before this returns; any other started thread is `WrongObject`.
/// - `value` is zero or a user address, else `BadPointer` and nothing changes. The kernel keeps it
///   per thread; a program writing its own register is outside this contract (`notes/thread-pointer.md`).
///
/// *(Name and number provisional, milestone 812's lane, 2026-10-10 UTC.)*
pub const SET_THREAD_POINTER: u64 = 3;

/// `invoke(cap, SUSPEND, _, _, _)` -> 0 (DECISIONS §300 (the paused state, a TCB verb, silent);
/// seL4's `TCB_Suspend`). **Stop the thread this TCB names until a [`RESUME`]**, keeping its
/// registers. Needs `WRITE`: the TCB capability is the whole authority.
///
/// - **Silent**: nothing reaches the supervision endpoint §26 (the fault endpoint) reports deaths on.
/// - **Stopped when it returns**, even when the thread was running on another core.
/// - A thread blocked in IPC stays in it, and stops before its next user instruction once it ends.
/// - A thread may suspend itself; the call returns 0 once somebody resumes it. Suspending a
///   suspended thread is 0; an embryo is `WrongObject`, an exited or dead thread `Gone`.
///
/// Name: ratified 2026-10-18 (calef: "Yes"), as §300 rules.
pub const SUSPEND: u64 = 4;

/// `invoke(cap, RESUME, _, _, _)` -> 0 (DECISIONS §300; seL4's `TCB_Resume`). **Let a suspended
/// thread run on from where it stopped**: the saved PC, every register as it was. Not a re-run.
/// Needs `WRITE`.
///
/// Resuming a thread that is not suspended is 0 and changes nothing. An embryo is `WrongObject`; a thread that
/// has exited or died is `Gone`.
///
/// Name: ratified 2026-10-18 (calef: "Yes"), as §300 rules.
pub const RESUME: u64 = 5;
