//! **The syscall boundary, as a single artifact.** The kernel and every user program depend on
//! this crate, so the ABI is one thing rather than two files that agree by luck: if it changes,
//! both sides fail to compile.
//!
//! # The surface is five calls <!--count:syscalls-->, and that is deliberate
//!
//! The syscall surface stays narrow (AGENTS.md rule 3), and §10 (process model:
//! capability-based) is what makes so few enough:
//!
//! ```text
//!   exit(code)                              you always have authority over yourself
//!   exit_thread(word)                       likewise, for one thread of yourself (milestone 812)
//!   yield()                                 likewise
//!   cap_delete(slot)                        likewise: your own capability table is your own
//!   invoke(cap, method, a0, a1, a2)         EVERYTHING ELSE
//! ```
//!
//! No `open`, `read`, `write` or `fork`: **a process can only act on things it was handed**, and
//! `invoke` is how. The first four are bare syscalls because a capability is authority over
//! something else, and nobody needs a grant to stop running or forget a slot. A new operation is a
//! method on a capability; a new syscall is a design fork. The count's marker makes `script/lint`
//! re-derive it from the constants below (notes/counted-claims.md).
//!
//! # The register convention
//!
//! ```text
//!   x8  syscall number
//!   x0  capability slot        (for invoke)
//!   x1  method
//!   x2  arg0
//!   x3  arg1
//!   x4  arg2
//!
//!   x0  return: >= 0 is a result, < 0 is an `Error`
//! ```
//!
//! `x8` for the number is Linux's aarch64 convention, and there is no reason to be different.
//!
//! Name: ratified 2026-08-23 (calef). Declined `application_binary_interface`: "ABI" passes the
//! test of whether an architect has to ask what it means, and this is the most-imported crate.

#![no_std]

/// `exit(code)`: end the calling thread's process, every thread in it (milestone 812; a thread
/// configured with an address space is a process of its own). Authority over yourself, so this
/// needs no capability; see the module docs for why it and its two siblings below are bare syscalls
/// rather than invocations.
pub const SYS_EXIT: u64 = 0;
/// `yield()`: give up the CPU without blocking. Same authority-over-yourself reasoning as `exit`.
pub const SYS_YIELD: u64 = 1;
/// `invoke(cap, method, a0, a1, a2)`: the one syscall that acts on anything other than the
/// calling thread. Every capability operation in the system goes through this number.
///
/// # Examples
///
/// ```
/// use abi::{SYS_EXIT, SYS_INVOKE, SYS_YIELD, rendezvous};
///
/// assert_eq!([SYS_EXIT, SYS_YIELD, SYS_INVOKE], [0, 1, 2]);
///
/// // Sending on a rendezvous is a method number, not a syscall number. Adding an operation to the
/// // system is a new method here; adding a *syscall* is a design fork (§10).
/// assert_eq!(rendezvous::SEND, 0);
/// ```
pub const SYS_INVOKE: u64 = 2;

/// `cap_delete(slot)`: drop a capability from the caller's own capability table, freeing the slot
/// (milestone 19d). The one capability-management operation the surface needs beyond `invoke`:
/// a loader (the progenitor) retypes hundreds of frames through a 16-slot capability table and must recycle slots.
/// Dropping a capability is authority over your *own* table, so like `exit` and `yield` it is a
/// bare syscall, not an invocation on some object. Deleting an empty slot is a harmless no-op.
pub const SYS_CAP_DELETE: u64 = 3;

/// `exit_thread(word)`: **end the calling thread, and only it** (milestone 812
/// (`std::thread::spawn` runs real threads in one address space); calef's ruling on pull request
/// #1892, 2026-10-10 UTC, question 3). Authority over yourself, so like [`SYS_EXIT`] it needs no
/// capability. [`SYS_EXIT`] ends the caller's whole process; this ends one thread. When the caller
/// is the last running thread of its process, the process ends with it, exactly as [`SYS_EXIT`]
/// would end it.
///
/// `word`, if not zero, names a 32-bit word in the caller's own space: the kernel stores zero there
/// and wakes every futex waiter on it once the thread has left user mode for good, so a joiner that
/// waits on the word may free the thread's stack (Linux's `CLONE_CHILD_CLEARTID`). An unmapped or
/// misaligned word is skipped, never a reason not to exit. Prior art: Linux `exit` beside
/// `exit_group`, Zircon `zx_thread_exit` beside `zx_process_exit`, FreeBSD `thr_exit` (whose
/// `state` word is this one). Semantics: §276 (the thread syscalls as built).
/// *(Number provisional.)*
pub const SYS_EXIT_THREAD: u64 = 4;

/// An index into the calling thread's capability table. **Not a pointer, not a handle you can
/// guess.** The kernel looks in *your* table, and if the slot is empty you get `NoSuchSlot`.
pub type CapSlot = u64;

/// The number of capability slots in a thread's capability table. Mirrors the kernel's
/// `cap::CAPABILITY_TABLE_SLOTS`; lives here too so userspace can name the reserved
/// [`fault::FAULT_EP_SLOT`] without reaching into the kernel. The two must agree, and a mismatch
/// would put the fault slot in different places on the two sides of the boundary, which is
/// exactly the drift this crate exists to prevent.
///
/// **Raised 16 -> 17, milestone 49's terminal update**; see `kernel::cap::CAPABILITY_TABLE_SLOTS`'s
/// own comment for the measured reason (`components/src/login.rs`'s eighth permanent grant).
///
/// **Raised 17 -> 24, milestone 230** (2026-09-02), after milestone 49's login stack turned out to
/// have been built against a temporary value of 28 that a later cleanup reverted to 17. Same place
/// as ever for the measurement and the account: the progenitor's boot peaks at 21 simultaneous slots, and
/// three of the seven added here are headroom rather than need.
///
/// **Raised 24 -> 32** (calef, 2026-09-27, UTC), when milestone 126 (the `procps` package) found
/// every progenitor boot slot below the fault slot named by some boot's grant and needed one more
/// for the machine statistics page. calef chose eight rather than one, so the next boot grant does
/// not meet the same wall. [`fault::FAULT_EP_SLOT`] moves from 23 to 31 with it, since it is derived.
///
/// **Raised 32 -> 64** (calef, 2026-10-04, UTC; milestone 754 (the capability table grows to 64
/// slots)), because a gpu-and-keyboard boot reached 31 of 32 in the progenitor
/// (`notes/capability-peak-trace.md`). **This is an ABI change**: [`fault::FAULT_EP_SLOT`] moves from
/// 31 to 63, so a supervisor built against 32 slots would write the fault endpoint to a slot the
/// kernel no longer reads. Every program that names it is built from this tree, and nothing outside
/// it is.
pub const CAPABILITY_TABLE_SLOTS: u64 = 64;

/// Methods on a `Console` capability. **Historical: no longer wired up.**
///
/// Milestone 8 removed the kernel-served `Console` object (the console became a userspace server
/// reached by a `Rendezvous`). This constant is kept so the ABI's history is legible, but nothing
/// in the kernel dispatches it any more.
pub mod console;

/// Methods on a `Rendezvous` capability. **This is IPC.**
///
/// A rendezvous names a synchronous meeting point, and the two methods are the two sides of it.
/// Which one you may call is a matter of *rights*, not of the rendezvous: a capability with
/// `WRITE` can `SEND`, one with `READ` can `RECEIVE`. So the same object, handed out with different
/// rights, is a one-way pipe in whichever direction each holder was trusted with. Neither side
/// can do the other's job, and neither had to be told which end it is.
pub mod rendezvous;

/// What [`rendezvous::SURVEY`] reports about a thread in the domain: the cursor sentinel, and the
/// run states a supervised thread can be found in.
///
/// **Only four states can appear, and the two absentees say something.** `Embryo` cannot, because a
/// thread's supervision endpoint is recorded at `START` (DECISIONS §26) and an embryo has not
/// started: a child that is built but not yet running is not in its domain yet. `Finished` cannot
/// either, because that is the state of a thread the reaper collects immediately, which is what
/// *un*supervised death looks like; a supervised thread dies into [`DEAD`](self::survey::DEAD) and waits for its
/// supervisor.
pub mod survey;

/// Methods on a `Reply` capability. **A one-shot answer to a specific caller.**
///
/// The kernel mints one on [`rendezvous::CALL`] and hands it to the server through
/// [`rendezvous::RECEIVE_CAP`]. It names the exact blocked caller, carries `WRITE` and no `GRANT` (so it
/// cannot be passed on), and is consumed the instant it is used, so a server cannot reply twice,
/// reply to the wrong caller, or hoard it. Those are kernel guarantees, not server discipline.
pub mod reply;

/// Object types for [`memory_region::RETYPE_OBJ`]: what a page of untyped becomes.
pub mod objtype;

/// Methods on the **reboot object** (milestone 805 (`reboot` at the prompt), DECISIONS §251
/// (restarting the machine is a kernel object the progenitor hands out)).
///
/// The kernel mints exactly one at boot and grants it to the progenitor, which endows it only to a
/// program whose manifest declares `reboot`. Nothing retypes one, so it has no [`objtype`] number.
/// Holding it is the whole authority: no rights bit is asked for.
///
/// *(Provisional, with its method number: the object, the module and `REBOOT` are calef's to name.)*
pub mod reboot;

/// Methods on a `Notification` capability (milestone 151, DECISIONS §101): **a doorbell, not a
/// meeting.** Created by [`memory_region::RETYPE_OBJ`] with [`objtype::NOTIFICATION`].
///
/// A signaller ORs bits into the notification's word and never blocks; a waiter takes whatever has
/// accumulated. Two signals before anyone waits arrive as one wake carrying both sets of bits.
///
/// **The binding is what makes it more than a semaphore.** [`BIND`](notification::BIND) attaches a notification to one
/// thread. From then on, a signal that finds nobody in [`WAIT`](notification::WAIT) and that thread blocked receiving
/// on an *endpoint* (`RECEIVE`, `RECEIVE_CAP`, or `Irq::WAIT`) wakes it there, so one blocking wait point
/// ends on either a message or a signal. How the woken thread tells which is [`BOUND`](notification::BOUND). A signal
/// that finds the bound thread anywhere else is kept in the word and delivered the next time that
/// thread enters a receive, or waits or polls.
///
/// Rights, from §101: `WRITE` to [`SIGNAL`](notification::SIGNAL) and [`BIND`](notification::BIND), `READ` to [`WAIT`](notification::WAIT) and [`POLL`](notification::POLL). There
/// is no unbind, and a notification or a thread can be bound at most once (`NotPermitted` for a
/// second bind of either). See `notes/notification-objects.md`.
///
/// **The method numbers are §101's**, and [`BOUND`](notification::BOUND)'s placement in `w4` is calef's ruling of
/// 2026-09-26 on a question §101 had answered with a forgeable encoding; the note has both.
pub mod notification;

/// Methods on a `Timer` capability (milestone 106, DECISIONS §147 (a timer a userspace service
/// cannot hold)): **the kernel's comparator, lent one deadline at a time.** Created by
/// [`memory_region::RETYPE_OBJ`] with [`objtype::TIMER`].
///
/// A timer does not block and has no wait of its own. It signals a [`notification`] when its
/// deadline passes, so whoever waits on that notification (or is blocked receiving with it bound to
/// their thread) wakes on the deadline or on whatever else signals it, whichever comes first. That
/// is the whole of §147's option 1: "a timer object with a blocking `WAIT` has one wake source";
/// this one has none, and borrows the notification's.
///
/// **Deadlines are absolute, in counter ticks**: the same counter a program reads without a syscall
/// (`CNTVCT_EL0`, `rdtime`, `rdtsc`), so `now() + n` is a deadline `n` ticks away and nothing
/// converts between clocks. **Resolution is the scheduler tick** (100 Hz, 10 ms): a deadline fires at
/// the first tick at or after it, never before it. See `design/roadmap/0106-deadline-wait.md`'s
/// BUGS for why, and what a tickless comparator would change.
///
/// Rights: `WRITE` on the timer to [`ARM`](timer::ARM) or [`CANCEL`](timer::CANCEL), and `WRITE`
/// on the notification `ARM` names, which is the right [`notification::SIGNAL`] needs: arming a
/// timer is signalling that notification later, so it takes the same authority.
///
/// **The surface was confirmed by calef on 2026-09-27 (UTC)**, `bits` and `CANCEL` included, and
/// with no slack argument: *"Keep it confirmed. We will build a slack if we need it."* A slack
/// (Zircon's `zx_timer_set` carries one) would be a later change if a tickless kernel wants it.
pub mod timer;

/// Methods on a `ThreadControlBlock` capability (milestone 19c.3): **another thread, under construction.**
/// Created by [`memory_region::RETYPE_OBJ`] with [`objtype::THREAD_CONTROL_BLOCK`].
///
/// **There is deliberately no method here that grants a thread the cycle counter**, and the
/// absence is a decision rather than an omission (milestone 229, DECISIONS 139 option 4).
///
/// The kernel-side mechanism is complete: a thread carries a grant, and the context switch
/// opens or closes the counter for the thread about to run. What is deferred is the syscall
/// surface that would let userspace set it, and the reason is that a method number is
/// irreversible while this one's retirement is already foreseeable.
///
/// **The prior art is `seL4_TCB_SetAffinity`**, a per-thread property expressed as a TCB
/// method, which MCS deleted outright and replaced with a core that is a field of
/// `sched_control_cap`. seL4 could not see that corner coming; this tree can, because
/// milestone 147 (a profiler that holds exactly the counters it was granted) is written down
/// and wants cross-thread authority with a named target, which no method here would provide.
/// Minting a method whose successor is already on the roadmap spends an irreversible number on
/// a path with a visible end.
///
/// **The capability object is not buildable yet either**, which is why the answer is not "build
/// 147's shape now": DECISIONS 139 records that 147's target-naming has no precedent in this
/// tree to price from. So the choice was never method-now against capability-now. It was
/// method-now against **not yet**, and not-yet costs almost nothing, because the grant has no
/// consumer: milestone 74's aarch64 half is the first and does not exist.
///
/// **A field on [`CONFIGURE`](thread_control_block::CONFIGURE) is not the alternative**, and that was established by looking
/// rather than assumed. `invoke` carries three argument registers; `CONFIGURE` spends all three
/// (entry, user stack, address-space slot) and [`START`](thread_control_block::START) spends all three on the child's first
/// registers. There is no spare word, so expressing the grant at creation time through the
/// existing methods would mean widening `invoke` itself, which is a larger and equally
/// irreversible change than the method this defers.
///
/// **Whoever needs it mints it, with a requirement in hand.** See
/// `design/roadmap/0229-the-counter-grant.md` and `notes/abi.md`.
pub mod thread_control_block;

/// Methods on an `AddressSpace` capability (milestone 19b): **another process's memory, under
/// construction.** Created by [`memory_region::RETYPE_OBJ`] with [`objtype::ADDRESS_SPACE`]; nothing can
/// run in it until TCBs arrive (19c), so today it is a structure you build, revocation can reach,
/// and (milestone 126, `pmap`) `ENUMERATE` can look at without touching. Since §249 (a running
/// address space stays nameable) the capability keeps naming the space after a thread is bound to
/// it, until that thread is reaped or the space's region is destroyed.
pub mod address_space;

/// **The `flags` word of [`address_space::WAIT`] and [`address_space::WAKE`]** (milestone 812
/// (`std::thread::spawn` runs real threads in one address space), §269 (how threads share a
/// process) fork 2). The bit layout is Linux futex2's (`FUTEX2_SIZE_*` in bits 0 and 1,
/// `FUTEX2_PRIVATE` at bit 7), so a reader who knows that ABI knows this one, and the two forms
/// §269 reserves are spellable now and refused rather than undefined.
///
/// Name: provisional (milestone 812's lane, 2026-10-10 UTC).
pub mod futex;

/// Methods on a **process** capability (milestone 812 (`std::thread::spawn` runs real threads in one
/// address space), §269 (how threads share a process) forks 1, 3 and 5, as ruled on pull request
/// #1892). Created by [`memory_region::RETYPE_OBJ`] with [`objtype::PROCESS`], whose second word is
/// the address space the process will hold.
///
/// A process is the capability table its threads share, the space they run in, its threads, its
/// supervision and its end. A thread joins one through [`thread_control_block::CONFIGURE`] given
/// this capability with [`rights::BIND`] in place of an address space; the capability is not
/// consumed. A thread configured with an address space instead gets a process of its own, as every
/// thread did before 812, and that process has no capability.
///
/// - **The supervision slot is the process's.** The first member's `START` takes the reserved
///   [`fault::FAULT_EP_SLOT`] out of the shared table, and the process's death is reported there,
///   whichever thread ends it. A member started later finds the slot empty.
/// - **A process ends** when a member calls [`SYS_EXIT`] or faults, when its last
///   running member calls [`SYS_EXIT_THREAD`], when [`DESTROY`](process::DESTROY) is invoked,
///   or when its region is destroyed. Every other member is ended with it, and no thread may join
///   or start in it afterwards (`Gone`).
///
/// *(Module, method and number provisional, milestone 812's lane, 2026-10-10 UTC.)*
pub mod process;

/// The rights bits, matching `capability::Rights`, so userspace can name the rights to narrow a
/// delegated capability to (the `rights` argument to [`rendezvous::SEND_CAP`]) without depending on
/// the kernel's `capability` crate.
///
/// **Five bits <!--count:rights-bits-->, and rights only ever narrow.** `capability::Rights::ALL`
/// is the mask `from_bits` filters against, so a bit defined here and missing there is silently
/// dropped at every delegation; the comment on that constant carries the warning.
///
/// These three lines of documentation spent from milestone 19a until the 2026-08-17 documentation
/// sweep attached to [`objtype`] instead, because 19a inserted that module directly
/// beneath them. The visible cost was on the other side: `objtype`'s rustdoc opened by telling a
/// reader it was the rights bits, and the module that holds them had no documentation at all. It
/// is the module the sweep's own trigger moved, since [`ENUMERATE`](self::rights::ENUMERATE)
/// landed here on 2026-08-17.
pub mod rights;

/// **The fault endpoint: thread death becomes a message a supervisor holds** (milestone 22,
/// DECISIONS §26). When a thread faults or exits, the kernel delivers one message to the
/// supervision endpoint its spawner designated, and the thread's corpse persists (dead until the
/// supervisor reaps it with §16 revocation). Restart policy lives in userspace; the kernel never
/// relaunches anything. This module is the two conventions §26 said it would add: a message-format
/// convention and a spawn-slot convention. No new syscall and no new method (§26).
pub mod fault;

/// **What a granted user-mode cycle-counter read counts**, per architecture (milestone 353 (the
/// aarch64 half of 74), calef's ruling B4 of 2026-10-07 UTC: a read is paired with its meaning).
///
/// This is a promise the kernel makes rather than a property of the instruction, which is why it
/// lives at the boundary: the kernel chooses what `PMCCFILTR_EL0` filters and which counter a grant
/// opens, so the kernel and every program must agree on what the number is. No syscall carries
/// it. `user_mode_runtime::cycle_reading` returns one of these beside the count, and the kernel's
/// own test checks a granted program reports the variant its architecture should, so neither side
/// keeps its own copy of the encoding (AGENTS.md rule 7).
///
/// The three are **not the same quantity**, and that is the reason the type exists. A program that
/// compares cycle figures across architectures must compare [`cycle_counter::CycleMeaning`]s first.
pub mod cycle_counter;

/// Methods on an `Irq` capability. **How a userspace driver owns an interrupt.**
pub mod interrupt;

/// Methods on a `Virtio` capability. **How a driver operates a device it cannot point out of its
/// own DMA region.** The kernel owns the queue addresses and the notify; the driver builds
/// requests in its DMA region and submits through here.
pub mod virtio;

/// Methods on a `MemoryRegion` capability. **How a process spends its own memory.**
pub mod memory_region;

/// **Which figure a [`memory_region::USAGE`] asks for** (milestone 126, DECISIONS §225). A selector
/// on `SURVEY`'s shape, so a new figure is a new value here and an arm in the kernel.
///
/// Every answer is in pages. [`SIZE`](usage::SIZE), [`COMMITTED`](usage::COMMITTED) and
/// [`CHILDREN`](usage::CHILDREN) describe this region alone. [`FRAMES`](usage::FRAMES) and the
/// three object kinds count **the whole subtree**, this region and every live region split from
/// it, because a budget's pages are mostly carved into child regions and the objects live in
/// those. The counts are bump-only like a watermark: a torn-down object's page
/// stays spent until its region is reclaimed, so they say where the budget went, not what is alive
/// now.
///
/// # BUGS
///
/// - Notification and timer objects (`objtype::NOTIFICATION`, `objtype::TIMER`) landed after these
///   records were ratified. Their pages are counted by the kernel and reported by no record, so the
///   object rows under-count a region that holds either. A record for each is a new wire number,
///   and so a question for calef rather than for the lane that noticed.
///
/// Names and numbers ratified 2026-09-27 (calef, #1360's table).
pub mod usage;

/// Methods on a `PageFrame` capability. **A physical page a process holds, maps, and shares.**
pub mod page_frame;

/// Methods on a `PortRange` capability (milestone 299). **A range of x86 I/O ports a driver holds.**
///
/// A `PortRange` capability is not invoked to *use* the ports: the holder executes `in`/`out`
/// directly, and the kernel enforces the grant through the TSS I/O permission bitmap at context
/// switch, with no syscall on the data path. The one method is administrative, mirroring
/// [`page_frame::REVOKE`]'s take-back meaning on a `DeviceFrame`.
pub mod port_range;

/// **How far apart the console UART's registers are** (milestone 890 (riscv64 and `x86_64` join the shared boot path)). Not a capability's methods:
/// the start word the kernel passes the progenitor, and the progenitor its console and input drivers.
pub mod uart;

// Index exception (2026-10-12): the error every child module returns stays at the root, because
// xtask's std generator inlines only `pub mod` children, and a public `error` module would be a
// new path.
/// What went wrong. Returned as a **negative** `x0`.
///
/// # Examples
///
/// Two distinctions are load-bearing. `NoSuchSlot` and `Gone` make a writer branch in opposite
/// directions (print into the void, or end); `NotSupervised` deliberately does not split "gone"
/// from "not yours", which would let a supervisor probe tids it has no relationship with.
///
/// ```
/// use abi::Error;
///
/// // "There is nothing there" and "there was something there and it is gone" are different facts,
/// // and a writer branches on them in **opposite** directions: a program never granted a stdout
/// // keeps running and prints into the void; a program whose reader has exited must end. Both used
/// // to arrive as NoSuchSlot, so the only available behaviour was the wrong one for a pipeline.
/// assert_ne!(Error::NoSuchSlot, Error::Gone);
/// assert_eq!(Error::NoSuchSlot as i64, -1);
/// assert_eq!(Error::Gone as i64, -11);
///
/// // And the one that is deliberately NOT split: "gone" and "not yours" are one error, because
/// // telling them apart would let a supervisor probe the tid space of children it has no
/// // relationship with. `Gone` can be told apart for the opposite reason: the capability is
/// // already in the caller's own capability table, so its death reveals nothing new.
/// assert_eq!(Error::NotSupervised as i64, -10);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum Error {
    /// **The slot is empty.** Not "permission denied": there is nothing there, and there is no
    /// way to name the thing you wanted. This is what no-ambient-authority *feels like*.
    NoSuchSlot = -1,

    /// The capability is real, but it is not that kind of object.
    WrongObject = -2,

    /// You hold the capability, but not with those rights. Rights only ever narrow on
    /// delegation, so somebody upstream chose this.
    ///
    /// **Or the object refuses the operation whatever the rights.** The one case today: a
    /// [`rendezvous::SEND`], [`rendezvous::SEND_CAP`] or [`rendezvous::CALL`] naming an endpoint
    /// that carries a hardware interrupt, which takes no message from any program (DECISIONS §101 (notification objects),
    /// ruling B, 2026-09-26). Nothing is delivered, queued or delegated, and the caller does not
    /// block.
    NotPermitted = -3,

    /// The pointer you passed is not memory **you** could have touched yourself.
    ///
    /// The most interesting error here. See notes/capabilities.md: a kernel that follows a user
    /// pointer using its own authority is the confused deputy, and this is the refusal.
    BadPointer = -4,

    /// No such method on that object.
    BadMethod = -5,

    /// The syscall number is not one of the five <!--count:syscalls-->.
    BadSyscall = -6,

    /// **The untyped region is exhausted.** The process ran out of the memory it was handed. The
    /// kernel is untouched: this is a budget, not a failure of the machine.
    OutOfMemory = -7,

    /// **A device operation was refused.** For virtio `NOTIFY`, this means a descriptor pointed
    /// outside the driver's DMA region and the device was not allowed to touch it.
    DeviceRefused = -8,

    /// **The thread is still running.** [`rendezvous::REAP`] collects a corpse, and this one is not
    /// one yet. Distinct from `NotPermitted` on purpose (DECISIONS §32): "you may not kill" and "no
    /// such child" are different facts, and a restart policy branches on them differently (wait, or
    /// escalate to the owner's `MemoryRegion::DESTROY`, versus give up on that tid).
    StillAlive = -9,

    /// **This endpoint does not supervise a thread by that tid.** Another supervisor's child, one
    /// already collected, or a tid whose generational name is stale. One error for all three
    /// deliberately: distinguishing "gone" from "not yours" would let a supervisor probe the tid
    /// space of children it has no relationship with.
    NotSupervised = -10,

    /// **The capability names an object that no longer exists** (milestone 50). You held a real
    /// capability, in a slot that is not empty, and the thing it named has been destroyed: an
    /// endpoint whose region was reclaimed, or one revoked out from under a thread while it was
    /// blocked inside an IPC on it. Nothing happened.
    ///
    /// **Distinct from [`Error::NoSuchSlot`] on purpose, and the distinction is the whole of
    /// milestone 50's answer to `SIGPIPE`.** "There is nothing there" and "there was something
    /// there and it is gone" are different facts, and a writer branches on them in opposite
    /// directions: a program never granted a stdout keeps running and prints into the void, which
    /// is what every OS does to a process whose stdout is closed, while a program whose reader has
    /// exited must **end**. Both used to arrive as `NoSuchSlot`, so the only available behaviour
    /// was the wrong one for a pipeline. See `crates/byte_sink_contract` and notes/sink-protocol.md.
    ///
    /// It carries no probing risk, which is why it can be told apart here when
    /// [`Error::NotSupervised`] deliberately cannot: the capability is one the caller already
    /// holds, in its own capability table, so learning that its object died reveals nothing it was not
    /// already entitled to know.
    Gone = -11,

    /// **`Reboot::REBOOT`: no route exists to ask for a reset** (milestone 805,
    /// no route exists, an aarch64 device tree with no usable `/psci`). The four `Reset…` values are the reboot object's only
    /// answers, because a reset that works never returns. The four names: ratified 2026-10-06
    /// (calef, #1783, "Yes" to one name per concept under §113 (kernel object plain names)).
    NoResetMechanism = -12,
    /// **`Reboot::REBOOT`: the firmware does not offer a reset** (PSCI or SBI `NOT_SUPPORTED`).
    ResetNotSupported = -13,
    /// **`Reboot::REBOOT`: the firmware refused this caller** (PSCI or SBI `DENIED`).
    ResetDenied = -14,
    /// **`Reboot::REBOOT`: the reset was asked for and the machine is still running**
    /// (every `x86_64` route tried, or any other PSCI or SBI code).
    ResetDidNotHappen = -15,
}

// Index exception (2026-10-12): the decode of the error above, kept beside it for the same reason.
impl Error {
    /// Decode a syscall's negative return value back into the `Error` it encodes. `None` for a
    /// non-negative `v` (success) or a negative value with no assigned meaning.
    ///
    /// # Examples
    ///
    /// One register, no flag bit: no result is ever negative, and an unassigned negative is not an
    /// error, which lets a wire contract layered on top tell "no capability" from "no".
    ///
    /// ```
    /// use abi::Error;
    ///
    /// // A successful `invoke` returning a handle, a count, or nothing.
    /// assert_eq!(Error::from_ret(0), None);
    /// assert_eq!(Error::from_ret(4096), None);
    ///
    /// // A refusal, decoded on the userspace side of the boundary.
    /// assert_eq!(Error::from_ret(-1), Some(Error::NoSuchSlot));
    /// assert_eq!(Error::from_ret(-4), Some(Error::BadPointer));
    ///
    /// // A number that is not one of the kernel's errors is not decoded as one. That is what lets a
    /// // wire contract layered on top (`entropy_contract`, `credential_contract`) tell "no capability" from "no".
    /// assert_eq!(Error::from_ret(-99), None);
    /// ```
    pub fn from_ret(v: i64) -> Option<Error> {
        Some(match v {
            -1 => Error::NoSuchSlot,
            -2 => Error::WrongObject,
            -3 => Error::NotPermitted,
            -4 => Error::BadPointer,
            -5 => Error::BadMethod,
            -6 => Error::BadSyscall,
            -7 => Error::OutOfMemory,
            -8 => Error::DeviceRefused,
            -9 => Error::StillAlive,
            -10 => Error::NotSupervised,
            -11 => Error::Gone,
            -12 => Error::NoResetMechanism,
            -13 => Error::ResetNotSupported,
            -14 => Error::ResetDenied,
            -15 => Error::ResetDidNotHappen,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests;
