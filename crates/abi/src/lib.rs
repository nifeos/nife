//! The syscall boundary, as one crate the kernel and every user program depend on, so a change
//! to the ABI fails to compile on both sides at once.
//!
//! The surface is five calls <!--count:syscalls-->. `exit`, `exit_thread`, `yield` and
//! `capability_delete` act on the caller itself, and `invoke(capability, method, a0, a1, a2)` acts on everything
//! else. `x8` holds the number, `x0` the slot, `x1` the
//! method and `x2` to `x4` the arguments; `x0` returns a result, or a negative [`Error`].
//!
//! Name: ratified 2026-08-23 (calef). Declined `application_binary_interface`: "ABI" passes the
//! test of whether an architect has to ask what it means, and this is the most-imported crate.

#![no_std]

/// The syscall ABI's version: the generation of the syscall numbers, the method numbers and the
/// error codes a program was built against, all one surface. One constant for the whole surface,
/// module-scoped constants refused (the scope is the syscall surface a program matches). The check is at spawn, not at run time: the
/// progenitor compares the manifest note's declared version against this and refuses a mismatch
/// naming both; the ABI never
/// dual-serves.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane milestone/956-checkable-interface-versions.
pub const ABI_VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 1, naming the syscall surface. The number
/// is the crate's to keep across a rename and no other interface ever reuses it; the ids of every
/// interface crate must differ. A program declares the pair of this id and [`ABI_VERSION`].
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
// Index exception (2026-10-31): the identity stays beside `ABI_VERSION`, the pair a program declares
// in its `INTERFACES` note; one line, and every consumer reads the two together.
pub const INTERFACE_ID: u64 = 1;

/// `exit(code)`: end the calling thread's process and every thread in it. Needs no capability,
/// because it acts on the caller itself.
pub const SYS_EXIT: u64 = 0;
/// `yield()`: give up the CPU without blocking. Needs no capability, as `exit` needs none.
pub const SYS_YIELD: u64 = 1;
/// `invoke(capability, method, a0, a1, a2)`: the one syscall that acts on anything but the caller.
///
/// # Examples
/// ```
/// use abi::{SYS_EXIT, SYS_INVOKE, SYS_YIELD, rendezvous};
///
/// assert_eq!([SYS_EXIT, SYS_YIELD, SYS_INVOKE], [0, 1, 2]);
///
/// // Sending on a rendezvous is a method number, not a syscall number. Adding an operation to the
/// // system is a new method here; adding a *syscall* is a design fork.
/// assert_eq!(rendezvous::SEND, 0);
/// ```
pub const SYS_INVOKE: u64 = 2;

/// `capability_delete(slot)`: drop a capability from the caller's own capability table, freeing the slot
/// Deleting an empty slot is a harmless no-op. A loader that
/// retypes many frames through a finite table recycles slots with it, and it needs no capability
/// because the table is the caller's own.
///
/// Name: provisional, minted 2026-10-27 (UTC) by lane capability-sweep-6. Spelled out from
/// `SYS_CAP_DELETE` under the capability ruling of 2026-10-06 (UTC).
pub const SYS_CAPABILITY_DELETE: u64 = 3;

/// `exit_thread(word)`: **end the calling thread, and only it**; when it is the last running
/// thread of its process the process ends too, as [`SYS_EXIT`] would end it. Needs no capability.
///
/// `word`, if not zero, names a 32-bit word in the caller's own space: once the thread has left
/// user mode for good the kernel stores zero there and wakes every futex waiter on it, so a joiner
/// may free the thread's stack. An unmapped or misaligned word is skipped, never a reason not to
/// exit. *(Number provisional.)*
pub const SYS_EXIT_THREAD: u64 = 4;

/// An index into the calling thread's capability table. **Not a pointer, not a handle you can
/// guess.** The kernel looks in *your* table, and if the slot is empty you get `NoSuchSlot`.
///
/// Name: provisional, minted 2026-10-27 (UTC) by lane capability-sweep-6. Spelled out from
/// `CapSlot` under the capability ruling of 2026-10-06 (UTC).
pub type CapabilitySlot = u64;

/// The number of slots in a thread's capability table, mirroring the kernel's
/// `cap::CAPABILITY_TABLE_SLOTS` so userspace can name [`fault::FAULT_EP_SLOT`] without reaching
/// into the kernel. The two must agree, or the fault slot sits in different places on the two
/// sides of the boundary.
///
/// **Changing it is an ABI change**: [`fault::FAULT_EP_SLOT`] is derived from it, so a supervisor
/// built against another size writes the fault endpoint to a slot the kernel does not read. Every
/// program that names it is built against one copy of this crate. The size was set on 2026-10-04 (UTC).
pub const CAPABILITY_TABLE_SLOTS: u64 = 64;

/// Methods on a `Console` capability. **Historical: no longer wired up.**
///
/// The kernel-served `Console` object was removed (the console became a userspace server
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
/// thread's supervision endpoint is recorded at `START` and an embryo has not
/// started: a child that is built but not yet running is not in its domain yet. `Finished` cannot
/// either, because that is the state of a thread the reaper collects immediately, which is what
/// *un*supervised death looks like; a supervised thread dies into [`DEAD`](self::survey::DEAD) and
/// waits for its supervisor.
pub mod survey;

/// Methods on a `Reply` capability. **A one-shot answer to a specific caller.**
///
/// The kernel mints one on [`rendezvous::CALL`] and hands it to the server through
/// [`rendezvous::RECEIVE_CAPABILITY`]. It names the exact blocked caller, carries `WRITE` and no `GRANT`
/// (so it cannot be passed on), and is consumed the instant it is used, so a server cannot reply
/// twice, reply to the wrong caller, or hoard it. Those are kernel guarantees, not server
/// discipline.
pub mod reply;

/// Object types for [`memory_region::RETYPE_OBJ`]: what a page of untyped becomes.
pub mod objtype;

/// Methods on the **reboot object** (restarting the machine is a kernel object the progenitor
/// hands out).
///
/// The kernel mints exactly one at boot and grants it to the progenitor, which endows it only to a
/// program whose manifest declares `reboot`. Nothing retypes one, so it has no [`objtype`] number.
/// Holding it is the whole authority: no rights bit is asked for.
///
/// *(Provisional, with its method number: the object, the module and `REBOOT` are an
/// architect's to name.)*
pub mod reboot;

/// Methods on a `Notification` capability: **a doorbell, not a meeting.** Created by [`memory_region::RETYPE_OBJ`]
/// with [`objtype::NOTIFICATION`]. A signaller ORs bits into its word and never blocks; a waiter
/// takes what has accumulated. [`BIND`](notification::BIND) attaches it to one thread, so a signal
/// can also end that thread's receive on an endpoint, told apart by [`BOUND`](notification::BOUND).
///
/// Rights: `WRITE` to [`SIGNAL`](notification::SIGNAL) and `BIND`, `READ` to
/// [`WAIT`](notification::WAIT) and [`POLL`](notification::POLL). `BOUND`'s place in `w4` was
/// ruled on 2026-09-26.
pub mod notification;

/// Methods on a `Timer` capability: **the kernel's comparator, lent one deadline at a time.** Created by
/// [`memory_region::RETYPE_OBJ`] with [`objtype::TIMER`]. It has no wait of its own: when its
/// deadline passes it signals a [`notification`].
///
/// Deadlines are absolute, in the counter ticks a program reads without a syscall, and fire at the
/// first scheduler tick (10 ms) at or after them, never before. Rights: `WRITE` on the timer to
/// [`ARM`](timer::ARM) or [`CANCEL`](timer::CANCEL), and `WRITE` on the notification `ARM` names.
/// The surface was confirmed on 2026-09-27 (UTC), with no slack argument.
pub mod timer;

/// Methods on a `ThreadControlBlock` capability: **another thread, under
/// construction.** Created by [`memory_region::RETYPE_OBJ`] with [`objtype::THREAD_CONTROL_BLOCK`].
///
/// **There is deliberately no method here that grants a thread the cycle counter**: the kernel
/// carries the grant, and the method that would set it waits for a consumer with a requirement in
/// hand.
pub mod thread_control_block;

/// Methods on an `AddressSpace` capability: **another process's memory, under
/// construction.** Created by [`memory_region::RETYPE_OBJ`] with [`objtype::ADDRESS_SPACE`];
/// nothing can run in it until TCBs arrive (19c), so today it is a structure you build, revocation
/// can reach, and `ENUMERATE` can look at without touching. The
/// capability keeps naming the space after a thread is
/// bound to it, until that thread is reaped or the space's region is destroyed.
pub mod address_space;

/// **The `flags` word of [`address_space::WAIT`] and [`address_space::WAKE`]**. The bit layout is Linux futex2's (`FUTEX2_SIZE_*` in bits 0 and 1,
/// `FUTEX2_PRIVATE` at bit 7), so a reader who knows that ABI knows this one, and the two forms
/// reserved for later are spellable now and refused rather than undefined.
///
/// Name: provisional (2026-10-10 UTC).
pub mod futex;

/// Methods on a **process** capability. Created by [`memory_region::RETYPE_OBJ`] with [`objtype::PROCESS`], whose second
/// word is the address space the process will hold.
///
/// A process is the capability table its threads share, their space, its supervision and its end.
/// A thread joins one through [`thread_control_block::CONFIGURE`] given this capability with
/// [`rights::BIND`]; a thread configured with an address space gets a process of its own, with no
/// capability. *(Module, method and number provisional, 2026-10-10 UTC.)*
pub mod process;

/// The rights bits, matching `capability::Rights`, so userspace can name the rights to narrow a
/// delegated capability to (the `rights` argument to [`rendezvous::SEND_CAPABILITY`]) without depending on
/// the kernel's `capability` crate.
///
/// **Five bits <!--count:rights-bits-->, and rights only ever narrow.** `capability::Rights::ALL`
/// is the mask `from_bits` filters against, so a bit defined here and missing there is silently
/// dropped at every delegation; the comment on that constant carries the warning.
pub mod rights;

/// **The fault endpoint: thread death becomes a message a supervisor holds**. When a thread faults or exits, the kernel delivers one message to the
/// supervision endpoint its spawner designated, and the thread's corpse persists (dead until the
/// supervisor reaps it with revocation). Restart policy lives in userspace; the kernel never
/// relaunches anything. This module holds two conventions: a message-format convention and a
/// spawn-slot convention. No new syscall and no new method.
pub mod fault;

/// **What a granted user-mode cycle-counter read counts**, per architecture.
///
/// The kernel chooses what `PMCCFILTR_EL0` filters and which counter a grant opens, so this is a
/// promise at the boundary rather than a property of the instruction; no syscall carries it.
/// `user_mode_runtime::cycle_reading` returns one beside the count and the kernel's test checks the
/// variant per architecture (AGENTS.md rule 7). The three are **not the same quantity**: compare
/// [`cycle_counter::CycleMeaning`]s before comparing cycle figures across architectures.
pub mod cycle_counter;

/// Methods on an `Interrupt` capability. **How a userspace driver owns an interrupt.**
pub mod interrupt;

/// Methods on a `Virtio` capability. **How a driver operates a device it cannot point out of its
/// own DMA region.** The kernel owns the queue addresses and the notify; the driver builds
/// requests in its DMA region and submits through here.
pub mod virtio;

/// Methods on a `MemoryRegion` capability. **How a process spends its own memory.**
pub mod memory_region;

/// **Which figure a [`memory_region::USAGE`] asks for**, in pages (the names and numbers were
/// ratified on 2026-09-27). [`SIZE`](usage::SIZE), [`COMMITTED`](usage::COMMITTED) and
/// [`CHILDREN`](usage::CHILDREN) are this region's; [`FRAMES`](usage::FRAMES) and the object kinds
/// count **the whole subtree**, bump-only like a watermark: where the budget went, not what lives.
///
/// # BUGS
///
/// - Notification and timer objects have no record, so the object rows under-count a region that
///   holds either. A record for each is a new wire number, and so a question for an architect.
pub mod usage;

/// Methods on a `PageFrame` capability. **A physical page a process holds, maps, and shares.**
pub mod page_frame;

/// Methods on a `PortRange` capability. **A range
/// of x86 I/O ports a driver holds.**
///
/// A `PortRange` capability is not invoked to *use* the ports: the holder executes `in`/`out`
/// directly, and the kernel enforces the grant through the TSS I/O permission bitmap at context
/// switch, with no syscall on the data path. The one method is administrative, mirroring
/// [`page_frame::REVOKE`]'s take-back meaning on a `DeviceFrame`.
pub mod port_range;

/// **How far apart the console UART's registers are**. Not a capability's methods: the start word the kernel passes the progenitor,
/// and the progenitor its console and input drivers.
pub mod uart;

// Index exception (2026-10-12): the error every child module returns stays at the root, because
// the generator of nife's standard library inlines only `pub mod` children, and a public `error` module would be a
// new path.
/// What went wrong, returned as a **negative** `x0`; each variant says when it is answered.
///
/// # Examples
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
    /// **Or the object refuses the operation whatever the rights.** Two cases. A
    /// [`rendezvous::SEND`], [`rendezvous::SEND_CAPABILITY`] or [`rendezvous::CALL`] naming an endpoint
    /// that carries a hardware interrupt, which takes no message from any program (ruled on 2026-09-26). Nothing is delivered, queued or delegated,
    /// and the caller does not block. And a mapping ([`address_space::MAP_INTO`],
    /// [`page_frame::MAP`]) that would leave one page writable through one mapping and executable
    /// through another in the same address space. Nothing is mapped.
    NotPermitted = -3,

    /// The pointer you passed is not memory **you** could have touched yourself.
    ///
    /// The most interesting error here: a kernel that follows a user
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
    /// one yet. Distinct from `NotPermitted` on purpose: "you may not kill" and "no
    /// such child" are different facts, and a restart policy branches on them differently (wait, or
    /// escalate to the owner's `MemoryRegion::DESTROY`, versus give up on that tid).
    StillAlive = -9,

    /// **This endpoint does not supervise a thread by that tid.** Another supervisor's child, one
    /// already collected, or a tid whose generational name is stale. One error for all three
    /// deliberately: distinguishing "gone" from "not yours" would let a supervisor probe the tid
    /// space of children it has no relationship with.
    NotSupervised = -10,

    /// **The capability names an object that no longer exists**: the slot is not empty, but the thing it named was destroyed, its region
    /// reclaimed or the endpoint revoked while a thread was blocked inside an IPC on it. Nothing
    /// happened.
    ///
    /// Distinct from [`Error::NoSuchSlot`] because a writer branches on the two in opposite
    /// directions: a program never granted a stdout prints into the void, and one whose reader has
    /// exited must end. It reveals nothing [`Error::NotSupervised`] hides, since the caller already
    /// holds the capability. The byte-sink contract builds on this.
    Gone = -11,

    /// **`Reboot::REBOOT`: no route exists to ask for a reset**, such as an aarch64 device tree with no usable `/psci`. The four `Reset…` values
    /// are the reboot object's only answers, because a reset that works never returns. The four
    /// names were ratified on 2026-10-06, one name per concept.
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
    /// Decode a negative syscall return into its `Error`, or `None` for a result or unknown value.
    /// # Examples
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
