/// An IPC rendezvous, page-resident, owned by the caller's budget (milestone 19a).
pub const RENDEZVOUS: u64 = 1;

/// An address space (milestone 19b): the retyped page **is the L0 root table**, and the
/// untyped it came from becomes the space's backing region, paying for every intermediate
/// page table and revocation record the space ever needs, exactly as an exec-built space's
/// region does (milestone 14 B.4). One budget model, decided on challenge: the seL4
/// principle (the user pays, the kernel allocates nothing) in this kernel's idiom, rather
/// than seL4's per-call paging-structure objects, which belong to the explicitness this
/// project deliberately did not copy.
pub const ADDRESS_SPACE: u64 = 2;

/// A thread (milestone 19c.3): the retyped page holds the TCB, and the object is born an
/// **embryo**, in no queue and not runnable. It becomes a running thread only through
/// [`crate::thread_control_block::CONFIGURE`] (bind an address space, set entry and stack) and [`crate::thread_control_block::START`], with
/// [`crate::thread_control_block::CAPABILITY_INSERT`] granting its initial authority in between. A half-built TCB can never
/// run: `START` refuses one with no bound space or no entry.
pub const THREAD_CONTROL_BLOCK: u64 = 3;

/// A notification (milestone 151, DECISIONS §101): a data word and a wait queue, the
/// asynchronous half of IPC. See [`crate::notification`].
pub const NOTIFICATION: u64 = 4;

/// A timer (milestone 106 (a wait that ends on either the interrupt or the deadline), DECISIONS
/// §147): one deadline and the notification it signals when
/// the deadline passes. See [`crate::timer`]. Confirmed by calef on 2026-09-27 (UTC).
pub const TIMER: u64 = 5;

/// **A process** (milestone 812 (`std::thread::spawn` runs real threads in one address space),
/// §269 (how threads share a process) forks 3 and 5): the capability table its threads share,
/// the address space they run in, its threads, and its supervision. The page is the creator's
/// region's, which is what pays for it. `RETYPE_OBJ`'s second word names the address space the
/// process holds, a `WRITE` capability that is consumed as `CONFIGURE` consumes one. See
/// [`super::process`]. *(Number provisional.)*
pub const PROCESS: u64 = 6;
