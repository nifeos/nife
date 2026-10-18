//! What a capability in nife can point at.
//!
//! The table itself is `crates/capability`, which is pure logic and knows nothing about this file.
//! This is the kernel's half: **the set of nouns.**
//!
//! DECISIONS §10 and notes/capabilities.md.

use core::num::NonZeroU64;

/// Every kind of thing a process can be handed.
///
/// **One entry, and that is the milestone-8 result rather than a stub.** There used to be a
/// `Console` variant: the kernel owned the PL011 and printed on a user's behalf. Milestone 8
/// deleted it. The console is now a userspace server reached by `SEND` on an endpoint, so
/// everything a process can name is an endpoint, and **the kernel no longer knows what a UART
/// is** on any path a user program can take.
///
/// The list grows deliberately, and each addition is a decision:
///
/// - `PageFrame` is now here, because **IPC carries control and shared memory carries data** (§10). A
///   shared buffer used to be mapped in at spawn, wired once by the kernel; a `PageFrame` makes
///   delegating memory a runtime operation a process does itself. See notes/frames.md.
/// - `MemoryRegion` at milestone 11, if we take §10's deferred axis, at which point the kernel stops
///   allocating and this enum stops being the interesting part of the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Object {
    /// An IPC endpoint, by **generational name** (milestone 19a: an endpoint is page-resident,
    /// and this is its `slots` name in the scheduler's registry, stale-safe like a `ThreadId`).
    ///
    /// Invoking it is a `SEND` or a `RECEIVE` (which one you may do is a matter of rights). Since
    /// milestone 8 this is how a process reaches the console: it holds a `WRITE` capability on
    /// the console server's endpoint, and printing is sending.
    /// **The second field is the capability's badge** (milestone 599 (a frame per filesystem
    /// client channel), provisional): a `u64` the kernel delivers to a server's `RECEIVE_CAP` so it
    /// can tell which of several clients sharing one endpoint made a request. `0` means unbadged,
    /// which every capability is until `abi::rendezvous::BADGE` stamps one, so the 232 sites that
    /// build an endpoint capability through [`rendezvous_cap`] are unchanged. seL4's model: a badge
    /// is set once on an unbadged capability and never altered, and it rides on the capability, not
    /// on the endpoint, so two clients can hold differently badged views of the same rendezvous.
    /// DECISIONS §148 (a supervisor restarts by asking) refused `BADGE` as a thread-identity method so it stays free for this; see
    /// also §101 (notification objects). The field fits without growing [`Object`], because `PageFrame` is already this
    /// wide (the assertion below holds).
    Rendezvous(crate::sched::RendezvousId, u64),

    /// **A memory region** (milestone 11): a capability to a chunk of raw physical memory the
    /// process may retype into pages. Invoking it grows the process's address space out of its
    /// own budget, and the kernel allocates nothing to do it. See `kernel/src/memory_region.rs`.
    MemoryRegion(u64),

    /// A hardware interrupt, by INTID.
    ///
    /// **The capability that lets a driver own an interrupt without owning any privilege.** Its
    /// holder can `WAIT` for the interrupt (blocking until it fires) and `ACK` it (re-enabling it
    /// at the GIC after the device has been serviced). The kernel's handler does nothing device-
    /// specific: it masks the line and turns the interrupt into a message. Everything that knows
    /// what the *device* is lives in the userspace driver. This is milestone 9's version of the
    /// milestone-8 move (the console driver left; now the interrupt does too).
    Irq(u32),

    /// **A contiguous run of physical pages**, by the base physical address and a page count, that
    /// the holder may map into its own address space and delegate to others.
    ///
    /// The object §10 named as the shared-memory capability: *IPC carries control, shared memory
    /// carries data*. A shared buffer used to be a page the kernel mapped into both parties at
    /// spawn, wired once and never movable. A `PageFrame` makes it a runtime object instead: a process
    /// retypes one out of its own untyped (`MemoryRegion::RETYPE`), maps it (`PageFrame::MAP`), and hands it
    /// (or a read-only view of it, since delegation narrows) to a peer over an endpoint. The peer
    /// maps the *same physical pages* and the two share memory, composed by the processes rather
    /// than arranged for them. The address is the identity: a process can never forge one, because
    /// the only ways to get a `PageFrame` are to retype it or be handed it, and both keep the object.
    ///
    /// **`count` is the run's length in pages, at least 1** (DECISIONS §102 (a Frame names a run of pages), 2026-08-20): a `PageFrame`
    /// names what the hardware names. A DMA region or a scanout is contiguous in physics, in the
    /// address space, and in the IOMMU domain that confines it, so one capability names the whole
    /// run instead of one per page: `PageFrame::MAP` maps all `count` pages with one call, and
    /// `PageFrame::REVOKE` unmaps all of them and deletes every capability naming the run. Every
    /// caller that predates this widening passes `count: 1` and keeps its exact prior behavior; the
    /// syscall's argument shape does not change; `count` rides on the capability, not on `MAP`'s
    /// arguments. See notes/frames.md.
    ///
    /// **`NonZeroU64` rather than `u64`, because a zero-page run is the wrong state and the
    /// compiler is the cheapest place to forbid it** (AGENTS.md's ladder, rung 1). Every dispatch
    /// path derives an end address from the count (`page_frame_map`'s tail-VA check computes
    /// `(count - 1) * PAGE_SIZE`), and a zero would underflow it: a debug build panics, a release
    /// build wraps. The type removes the branch instead of guarding it, and it costs nothing,
    /// since the niche makes `Option<Object>` no larger.
    PageFrame(u64, NonZeroU64),

    /// **A device's MMIO page**, by physical address (milestone 19d.2): a delegatable authority
    /// to map a *specific* device's registers, **device-typed** (nGnRnE, uncacheable, unreordered
    ///: the only attributes MMIO tolerates). The kernel mints one per known device (it alone
    /// knows device physical addresses) and hands it to the progenitor; the progenitor delegates it to the driver it
    /// builds and maps it into that driver's space. This is what turns "the kernel maps the UART
    /// at spawn" into "device access is a capability", so a userspace progenitor can bring up drivers.
    /// Distinct from `PageFrame` precisely because a `PageFrame` maps *normal cacheable* memory, which for
    /// MMIO would let the CPU cache and reorder register accesses: catastrophic for a device.
    ///
    /// Constructed by the boot (`boot_progenitor`'s console device grant on aarch64 and
    /// riscv64) and by `spawn_elf_child_builder` and the system tests beside it, so a build that
    /// never reaches a hand-off never mints one. The syscall path still
    /// *matches* on it, and a match is not a construction.
    #[cfg_attr(all(target_arch = "riscv64", not(feature = "shell")), allow(dead_code))]
    DeviceFrame(u64),

    /// **A one-shot reply channel to a blocked caller** (milestone 12), named by the caller's
    /// thread id.
    ///
    /// The kernel mints one at a `CALL` rendezvous and hands it to the server. It is never
    /// forgeable and nameable no other way, so invoking it delivers the reply to *exactly* that
    /// caller and consumes the capability. "One reply, to this caller, exactly once" is therefore a
    /// kernel guarantee rather than a server convention, which is the whole point of the object.
    /// See DECISIONS §12 and notes/ipc-naming.md.
    Reply(crate::thread::ThreadId),

    /// **An address space under construction** (milestone 19b), by generational name in the
    /// user address-space registry. The object itself is its L0 root page, resident in the page
    /// retyped from its creator's untyped; the kernel-side record (ASID, backing region) sits
    /// behind this name. `WRITE` lets the holder map frames into it; nothing can run in it
    /// until TCBs arrive (19c).
    AddressSpace(u64),

    /// **A thread under construction** (milestone 19c.3), by generational `ThreadId`: an embryo TCB a
    /// process is assembling. `WRITE` lets the holder configure, grant into, and start it. The
    /// `ThreadId` is stale-safe like every generational name, so a capability outliving its thread
    /// resolves to nothing rather than to a stranger.
    ThreadControlBlock(crate::thread::ThreadId),

    /// **A notification** (milestone 151 (notification objects), DECISIONS §101 (notification objects)), by generational name in the
    /// scheduler's notification registry: a data word and a wait queue in a page retyped from its
    /// creator's region. `WRITE` signals it and binds it to a thread; `READ` waits on it and polls
    /// it. Same payload width as `Rendezvous`, so the assertion below on a slot's size still holds.
    Notification(crate::sched::NotificationId),

    /// **A timer** (milestone 106 (a wait that ends on either the interrupt or the deadline),
    /// DECISIONS §147 (a timer a userspace service cannot hold)), by generational name in the
    /// scheduler's timer registry: one armed deadline and the notification it signals, in a page
    /// retyped from its creator's region. `WRITE` arms and cancels it. *(Provisional, with its
    /// method numbers.)*
    Timer(crate::sched::TimerId),

    /// A process (milestone 812 (`std::thread::spawn` runs real threads in one address space), §269
    /// (how threads share a process) forks 1, 3 and 5), by generational name over the scheduler's
    /// process registry: the capability table its threads share, the space they run in, its
    /// supervision and its end. `BIND` on it joins a thread (`abi::process`).
    Process(crate::sched::ProcessId),

    /// A virtio device's **transport**, by id (into the kernel's virtio device table).
    ///
    /// The DMA-confinement capability. The device has no IOMMU, so the kernel keeps the two
    /// DMA-critical powers (programming the queue's ring addresses and ringing the device) and
    /// validates that every descriptor stays within the driver's own DMA region before the device
    /// sees it. The holder drives the device (status, features, submit) through this, but cannot
    /// point it outside its region. See kernel/src/virtio.rs.
    Virtio(usize),

    /// **A contiguous range of x86 I/O ports**, by base port and count (milestone 299,
    /// DECISIONS §121 reversed 2026-09-15). The device-capability kind for the one class of device
    /// that has no page: x86's legacy port-I/O hardware, reached only by `in`/`out`, which the MMU
    /// cannot grant or deny because there is no page table in front of the port space.
    ///
    /// It is the honest analogue of [`Object::DeviceFrame`]: `DeviceFrame` names a device's MMIO page and
    /// the MMU enforces it; `PortRange` names a device's ports and the **TSS I/O permission bitmap**
    /// enforces it. A thread that holds one may execute `in`/`out` on `[base, base + count)` from
    /// ring 3, and no other port; a thread that holds none may touch no port at all. **The capability must
    /// carry `WRITE`** (milestone 768 (provisional), calef 2026-10-05 UTC): the bitmap cannot grant `in`
    /// without `out`, so a `PortRange` without `WRITE` opens no port at all (`sched::thread_control_block_insert_from`).
    /// The granularity
    /// is a range because a 16550 UART is eight consecutive ports (COM1 is `0x3F8..=0x3FF`).
    ///
    /// **`x86_64` only**, because the other two architectures have no port space and no TSS I/O
    /// bitmap: there is no hardware for this object to name, so unlike [`Object::DeviceFrame`] (an
    /// MMIO page, which every architecture has) it is not a parity gap to omit it (§19). Compiling it
    /// only where it exists also keeps it off the other two architectures' syscall dispatcher, which
    /// a new enum variant would otherwise grow (`script/fastpath-footprint`). See
    /// `kernel/src/arch/x86_64/segments.rs` for the bitmap that carries the grant and
    /// `sched::schedule` for the lazy install on switch-in.
    ///
    /// `(u16, u16)` because a port number is 16 bits (`in`/`out` address exactly 64 Ki ports) and a
    /// count never exceeds that; the pair fits the enum's existing 16-byte payload, so adding it does
    /// not grow a capability slot (the assertion below still holds, on every architecture, because
    /// `PageFrame` is the widest variant either way).
    #[cfg(target_arch = "x86_64")]
    PortRange(u16, u16),

    /// **The authority to restart the machine** (milestone 805 (`reboot` at the prompt), DECISIONS
    /// §251 (restarting the machine is a kernel object the progenitor hands out)). No payload: the
    /// kernel stores nothing in it and it names no device, because PSCI, SBI SRST and a PC's reset
    /// register are all "the machine" and none of them is a page or a port a holder could be given
    /// instead.
    ///
    /// The kernel mints exactly one, at boot, into the progenitor's table, and nothing else mints
    /// one: no retype makes it, so it has no `abi::objtype` number. Its one method is
    /// `abi::reboot::REBOOT`; see kernel/src/reboot.rs.
    ///
    /// Name: provisional, milestone 805's lane, 2026-10-06 (UTC).
    Reboot,
}

pub type Cap = capability::Cap<Object>;

/// [`Cap`] spelled out, for new code under calef's 2026-10-06 ruling that `capability` is not
/// abbreviated. Name: provisional (milestone 824's lane, 2026-10-12 UTC).
pub type Capability = Cap;

// **What a slot costs, pinned** (milestone 142's review, MINOR 9).
//
// DECISIONS §102 rejected growing `CAPABILITY_TABLE_SLOTS` on an arithmetic that starts from "24
// bytes a slot, times `MAX_THREADS` = 128", and then, in the same decision, widened `PageFrame`
// from one `u64` to two. **A slot is 32 bytes now, not 24**: the enum is as wide as its widest
// variant, `PageFrame` is that variant, and it grew by a word. Nothing measured it, so §102's own
// figure went stale inside §102. That is what this assertion is for; it is the fact, not a target.
//
// Sixty-four slots is 2,048 bytes a capability table, so the option §102 priced at
// 12 KiB a thread for 512 slots would now be 32 KiB. The refusal does not change (the decision's
// argument was never really about the bytes), but the number a future reader quotes should be the
// one the compiler agrees with. Update these two and re-read §102 when they fire.
//
// **Sixteen when this note was written, seventeen after milestone 49 (users, login, and attribution), twenty-four after milestone
// 230, thirty-two after #1360, sixty-four now** (see that constant's own doc, below). The count changed; the per-slot
// arithmetic this note exists to pin did not.
//
// **And the other half of §102's arithmetic moved too**: `MAX_THREADS` was raised from 128 to 256
// on 2026-08-27 (that constant's own doc comment carries the measurement), so every whole-machine
// figure §102 quoted is doubled on top of both changes above. Same conclusion, same reason, larger
// numbers.
const _: () = assert!(core::mem::size_of::<Object>() == 24);
const _: () = assert!(core::mem::size_of::<Cap>() == 32);

/// A thread's capability table: [`CAPABILITY_TABLE_SLOTS`] slots, fixed at the type (milestone 14
/// (kernel objects from untyped) phase B.1), so creating a capability table cannot allocate.
/// Growing it is a one-number change here, paid in TCB size.
///
/// **A full table fails silently.** The progenitor that fills its table dies at
/// `user_mode_runtime::trap` before a console exists to carry a word, and `script/test` never boots
/// the real progenitor (the kernel plays it); `script/swish-check` is the gate that does. Each
/// past raise was met in that silence, so the size is set from a measured peak plus headroom:
/// [`CAPABILITY_TABLE_PEAK_MEASURED`] keeps the account, and `notes/capability-peak-trace.md`
/// itemizes the progenitor's login-block peak.
///
/// **64** is calef's ruling of 2026-10-04 (UTC) (milestone 754 (the capability table grows to 64
/// slots), ruling A on #1608), chosen over moving holdings into a new process after a
/// gpu-and-keyboard boot reached 31 of 32. The cost is 1,032 bytes a thread, inside a TCB page
/// (`crate::thread`'s page-fit assertion is the check), and no static memory. The fault slot,
/// derived, is 63. Whether a table should be sized for its process instead is
/// `design/roadmap/0778-capability-tables-sized-per-process.md`.
pub const CAPABILITY_TABLE_SLOTS: usize = 64;

// **The free-slot word is a `u64`, so sixty-four is also the ceiling the type allows** (milestone
// 126 (the `procps` package), calef's ruling on #1360, 2026-09-27, UTC, widened by milestone 754 (the capability table grows to 64 slots)).
// `capability::CapabilityTable` asserts this for every size it is built at; it is repeated here
// because this line is where the next raise will be typed, and the message should meet that person
// here rather than inside a const evaluation in another crate. Raising past it is a wider word or
// a second one, which the constant's doc on `capability::MAX_SLOTS` prices.
const _: () = assert!(
    CAPABILITY_TABLE_SLOTS <= capability::MAX_SLOTS,
    "the capability table's free-slot bitmap is one u64; widen it before raising the slot count"
);
// The table's layout, pinned: 64 slots of 32 bytes, the `u64` free word and two `u16` counts, which
// is 2,060 bytes rounded to eight. It was 1,032 at 32 slots with a `u32` word. If this fires, the
// layout moved and `crate::thread`'s page-fit check is the next thing to read.
const _: () = assert!(core::mem::size_of::<CapabilityTable>() == 2064);
pub type CapabilityTable = capability::CapabilityTable<Object, CAPABILITY_TABLE_SLOTS>;

/// **What a real interactive boot actually reaches**: the progenitor's peak capability count,
/// measured on `script/swish-check`'s aarch64 leg. The highest is the gpu-and-keyboard boot, at 34
/// of [`CAPABILITY_TABLE_SLOTS`]; a boot with no gpu peaks lower. `notes/capability-peak-trace.md`
/// itemises what sits on the peak: the capabilities the progenitor never gives back (among them the
/// entropy and network client endpoints it endows children from, the display grants, its own
/// address space, the reboot object and the name resolver's endpoint), the login block's own, and
/// the address space and page `build_child` is working through.
///
/// **This is a record, not a target**: a reader quoting it gets the number the machine agrees with.
/// [`report_peak`] says on the console when a boot goes past it, and `script/swish-check` fails on
/// that sentence. When it fires, measure, update this, and re-read the headroom arithmetic in
/// [`CAPABILITY_TABLE_SLOTS`]'s doc rather than raising that constant reflexively: every slot held
/// for the life of the boot is one slot on the peak, and the headroom is a licence for a decision,
/// not for a quiet spend.
///
/// The check is against this record rather than the ceiling, which would only fire on a boot that
/// had already died, or a fraction of it, which would be a margin picked out of the air. A
/// recorded measurement going stale is a fact about the tree that stopped being true.
pub const CAPABILITY_TABLE_PEAK_MEASURED: usize = 34;

// The headroom milestone 230 left is what this pair means, so the two cannot silently invert.
const _: () = assert!(CAPABILITY_TABLE_PEAK_MEASURED < CAPABILITY_TABLE_SLOTS);

/// The highest peak [`report_peak`] has already said out loud, so the same number is not printed
/// twice by two cores or on two passes.
static PEAK_REPORTED: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// What the previous idle pass saw, which is half of the coalescing. See [`report_peak`].
static PEAK_LAST_PASS: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// How many consecutive idle passes have seen the same peak. The other half of the coalescing.
static PEAK_STABLE_PASSES: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

/// How still the mark has to be before [`report_peak`] believes the climb is over.
///
/// **Measured, and the measurement is the whole argument.** With this at one, an aarch64
/// interactive boot printed six lines (4, 5, 12, 14, 16, 21 of 24): the progenitor blocks on IPC several
/// times while it is building the login stack, so the machine goes idle mid-climb and each pause
/// looked like an ending. At sixteen it prints one, and the number it prints is the same 21.
/// (Those figures are that day's boot, kept because the argument is about the *window*; the peak
/// itself is 22 since milestone 111, and the window did not have to move for it.)
///
/// It is a coalescing window rather than a threshold on the thing being measured, which is why it
/// is not the kind of guessed margin `CAPABILITY_TABLE_PEAK_MEASURED`'s own doc refuses. Getting it
/// wrong costs an extra printed line or a later one; it cannot make the reported peak wrong,
/// because the peak itself never decreases.
const PEAK_STABLE_PASSES_NEEDED: usize = 16;

/// **Say what the boot's capability-slot high-water mark is, once it has stopped moving**
/// (milestone 231).
///
/// Called from the scheduler's idle loop, which is the one place in the kernel that is reached
/// after every phase of a boot and is by definition not busy. Two atomics turn a mark that climbs
/// once per grant into one printed line:
///
/// - **It waits for the mark to go still**, [`PEAK_STABLE_PASSES_NEEDED`] idle passes with the same
///   number. A peak that moved is still climbing, so this resets and looks again. The progenitor blocks on
///   IPC several times while building the login stack, so a shorter window mistakes each of those
///   pauses for an ending; that constant's own doc carries the measurement.
/// - **It never repeats a number.** `fetch_max` gives exactly one caller a previous value below
///   the new peak, so a four-core machine prints one line rather than four.
///
/// The number reported is `capability::highest_seen`'s, which is the highest any table has reached
/// rather than any particular thread's. Finding the owning thread means walking every thread under
/// the scheduler lock; that is a scan this deliberately does not do, and the omission is in this
/// module's BUGS.
pub fn report_peak() {
    use core::sync::atomic::Ordering;
    let (peak, ceiling) = capability::highest_seen();
    if peak == 0 {
        return;
    }
    if PEAK_LAST_PASS.swap(peak, Ordering::Relaxed) != peak {
        PEAK_STABLE_PASSES.store(0, Ordering::Relaxed);
        return;
    }
    if PEAK_STABLE_PASSES.fetch_add(1, Ordering::Relaxed) < PEAK_STABLE_PASSES_NEEDED {
        return;
    }
    if PEAK_REPORTED.fetch_max(peak, Ordering::Relaxed) >= peak {
        return;
    }
    announce_peak(peak, ceiling);
}

/// Say the gauge out loud. Split from [`report_peak`] so the whole sentence, and not one arm of it,
/// is what the test kernel leaves out.
///
/// **The test kernel counts and does not print**, and both halves of that are deliberate.
///
/// It does not print because the guest suite's transcript is a gate's input, and this line lands in
/// the middle of one, splitting a test name across a `capability slots:` sentence. That is the same
/// two-writers-one-stream confusion milestone 230 spent a whole lane on, and adding a third writer
/// to it for a number nobody reads there would be a poor trade.
///
/// And even printed, [`CAPABILITY_TABLE_PEAK_MEASURED`] would be the wrong yardstick here: it is
/// the *interactive boot's* number, and the guest suite runs a much larger and quite different
/// workload through the same kernel, so a test run going past twenty-one would be a true sentence
/// about the wrong thing. The counting itself stays on, so a test that filled a table still moves
/// the mark for anything that asks.
#[cfg(not(any(test, feature = "system_tests")))]
fn announce_peak(peak: usize, ceiling: usize) {
    if peak > CAPABILITY_TABLE_PEAK_MEASURED {
        crate::println!(
            "  capability slots: {peak} of {ceiling} at peak, ABOVE the \
             {CAPABILITY_TABLE_PEAK_MEASURED} recorded in kernel/src/cap.rs"
        );
        return;
    }
    crate::println!("  capability slots: {peak} of {ceiling} at peak");
}

/// See the other arm: the test kernel counts and does not print.
#[cfg(any(test, feature = "system_tests"))]
fn announce_peak(_peak: usize, _ceiling: usize) {}

// The ABI names the reserved fault slot as `CAPABILITY_TABLE_SLOTS - 1`, so the two constants
// must agree or the kernel would read the fault endpoint from a different slot than the
// supervisor wrote it to.
const _: () = assert!(CAPABILITY_TABLE_SLOTS as u64 == abi::CAPABILITY_TABLE_SLOTS);

pub use capability::{Error, Rights};

// **The rights vocabulary is written twice, so the compiler is what keeps the two copies honest.**
//
// `abi::rights` is what userspace *names* a right with: it is the word that travels in a syscall
// register at 79 call sites outside `crates/abi` as this lands, from `system_initializer`'s grant
// tables to every `SEND_CAP`/`CAP_INSERT` in `components/src/` and `fixtures/src/`.
// `capability::Rights` is what the kernel
// *means* by one.
// Until this block existed nothing compared them, and the two are not one definition with two
// spellings: they are two arrays of magic numbers in two dependency-free crates that cannot see
// each other.
//
// The failure this closes is not hypothetical, it is the one milestone 126 hit on its way in. That
// lane added `ENUMERATE` and found `RETYPE_OBJ`'s hand-listed `READ|WRITE|GRANT` had silently
// stopped meaning "full rights", and the symptom surfaced three steps away as `OutOfMemory` at a
// prompt. `Rights::ALL` is now the invariant on the kernel side; this is the same invariant across
// the boundary. A fifth right added to one crate and not the other is a compile error here rather
// than a delegation that quietly drops a bit at a `from_bits`, which is exactly the shape that
// takes three steps to diagnose.
//
// Found by the 2026-08-17 security audit (design/audit-reports/), which was looking for a path
// where widening `Rights::ALL` widened something unintended and found instead that nothing checked
// the two vocabularies at all.
const _: () = assert!(Rights::READ.bits() as u64 == abi::rights::READ);
const _: () = assert!(Rights::WRITE.bits() as u64 == abi::rights::WRITE);
const _: () = assert!(Rights::GRANT.bits() as u64 == abi::rights::GRANT);
const _: () = assert!(Rights::ENUMERATE.bits() as u64 == abi::rights::ENUMERATE);
const _: () = assert!(Rights::BIND.bits() as u64 == abi::rights::BIND);

// **`ALL` is exactly the ABI's vocabulary and no more.** Both directions are load-bearing and they
// fail differently. A bit in `abi::rights` that is missing from `ALL` is a right userspace can name
// and `Rights::from_bits` masks to zero, so a delegation asking for it succeeds while conferring
// nothing. A bit in `ALL` with no ABI name is a right the kernel honours that no caller can ask for
// by name, which is how a hand-listed set drifts into granting more than any manifest says.
const _: () = assert!(
    Rights::ALL.bits() as u64
        == abi::rights::READ
            | abi::rights::WRITE
            | abi::rights::GRANT
            | abi::rights::ENUMERATE
            | abi::rights::BIND
);

// `abi::rights` is `u64` and `Rights` is `u32`, and the syscall path narrows with `a1 as u32`
// (`SEND_CAP` and `CAP_INSERT` in kernel/src/syscall.rs). A right defined at bit 32 or above would
// therefore be truncated to nothing on the way in, silently, and the delegation would appear to
// succeed. Nothing about the ABI's type stops somebody writing `1 << 32`; this does.
const _: () = assert!(
    abi::rights::READ
        | abi::rights::WRITE
        | abi::rights::GRANT
        | abi::rights::ENUMERATE
        | abi::rights::BIND
        <= u32::MAX as u64
);

/// A capability naming an endpoint, with the given rights.
///
/// **`WRITE` lets the holder `SEND`; `READ` lets it `RECEIVE`.** Hand the two ends of one endpoint
/// out with opposite rights and you have a one-way pipe that neither side can run backwards.
pub fn rendezvous_cap(ep: crate::sched::RendezvousId, rights: Rights) -> Cap {
    Cap {
        object: Object::Rendezvous(ep, 0),
        rights,
    }
}

/// An endpoint capability carrying a **badge** (milestone 599, provisional). The kernel delivers
/// `badge` to a server's `RECEIVE_CAP` when this capability's holder `CALL`s or `SEND_CAP`s here, so a
/// server serving many clients on one endpoint can tell them apart. `badge` `0` is unbadged and
/// equivalent to [`rendezvous_cap`]; a non-zero badge is minted once by `abi::rendezvous::BADGE` and
/// then rides through delegation unchanged, because `SEND_CAP` and `CAP_INSERT` copy the object.
pub fn rendezvous_cap_badged(ep: crate::sched::RendezvousId, rights: Rights, badge: u64) -> Cap {
    Cap {
        object: Object::Rendezvous(ep, badge),
        rights,
    }
}

/// A capability naming a hardware interrupt. `READ` lets the holder `WAIT` and `ACK` it.
pub fn irq_cap(intid: u32) -> Cap {
    Cap {
        object: Object::Irq(intid),
        rights: Rights::READ,
    }
}

/// An interrupt capability with explicit rights (milestone 19d.2b): the progenitor holds one with `GRANT`
/// so it can delegate the interrupt to a driver it builds.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn irq_cap_rights(intid: u32, rights: Rights) -> Cap {
    Cap {
        object: Object::Irq(intid),
        rights,
    }
}

/// A capability to an untyped memory region with **every right but `GRANT`**: the holder may spend
/// it (retype pages and objects, `SPLIT`, `MAP`, `DESTROY`) but **not delegate it**, because
/// `SEND_CAP` and `CAP_INSERT` both gate on `GRANT`. This is the spend-only budget a leaf child is
/// handed: least authority for a process that consumes memory and passes none on.
///
/// **Not `WRITE` alone, since milestone 824** (a retype mints no right its budget lacks). A retype
/// now mints the budget's rights, calef's ruling (a) of 2026-10-07 (UTC, PR #1798), so what a budget
/// carries is what everything carved from it carries. With `WRITE` alone a leaf's own notification
/// could not be waited on (`READ`), its own rendezvous not received on, its own process not joined
/// (`BIND`). On the region itself `READ` and `BIND` are consulted by no method and `ENUMERATE` only
/// by `USAGE`, which answers what the holder's own budget was spent on, so the widening confers
/// nothing over the region. `GRANT` is the one right left out, and it is the one the ruling is
/// about: a leaf cannot lend its budget, and since 824 cannot lend anything it carves from it. A
/// builder that must lend what it makes is handed a budget with `GRANT`, delegated from the root.
pub fn memory_region_cap(region: u64) -> Cap {
    Cap {
        object: Object::MemoryRegion(region),
        rights: SPEND_ONLY,
    }
}

/// **A budget whose holder lends what it carves from it**: every right, `GRANT` included. Since
/// milestone 824 (a retype mints no right its budget lacks) a frame or object carries its budget's
/// rights, so a program that retypes a page and `SEND_CAP`s it, or builds a child and endows it,
/// needs `GRANT` on the budget itself; [`memory_region_cap`] withholds it. The kernel-side harnesses
/// hand this to exactly those programs. It is [`memory_region_root_cap`]'s rights under a name that
/// says why, since none of these holders is the root.
///
/// Name: provisional (milestone 824's lane, 2026-10-12 UTC).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn memory_region_lending_capability(region: u64) -> Capability {
    Capability {
        object: Object::MemoryRegion(region),
        rights: Rights::ALL,
    }
}

/// The rights of the spend-only budget [`memory_region_cap`] mints: [`Rights::ALL`] without
/// `GRANT`, the kernel's spelling of `abi::rights::SPEND_ONLY`, which a userspace builder hands its
/// leaves. Name: provisional (milestone 824's lane, 2026-10-12 UTC).
pub const SPEND_ONLY: Rights = Rights::from_bits(abi::rights::SPEND_ONLY as u32);

// `abi::rights::ALL` is what userspace hands a builder and `Rights::ALL` what the kernel's root
// budget carries; a right added to one and not the other would make a builder's objects silently
// narrower than the progenitor's.
const _: () = assert!(Rights::ALL.bits() as u64 == abi::rights::ALL);

/// A capability to an untyped region with explicit rights (milestone 31). Its one caller is
/// [`memory_region_root_cap`], which builds the delegable root from `Rights::ALL`. `MemoryRegion::SPLIT`
/// used to build its child here too; since milestone 35 it mints through `Cap::mint_child` instead
/// (the inheriting mint the `split_never_widens_rights` proof covers), so the child's rights are
/// pinned to the parent's by proved code rather than by passing `cap.rights` here.
///
/// The rights an untyped carries are therefore set once at the root and only ever narrow downward:
/// root (`GRANT`) -> the progenitor's `SPLIT` (inherits `GRANT`) -> `CAP_INSERT` into a child (narrowed).
pub fn memory_region_cap_rights(region: u64, rights: Rights) -> Cap {
    Cap {
        object: Object::MemoryRegion(region),
        rights,
    }
}

/// The delegable root untyped the kernel hands the progenitor at boot (milestone 31). Full rights, `GRANT`
/// included, because handing memory budgets to the children it builds is the progenitor's whole job: the root
/// of the budget tree must carry the right to pass budgets on. Rights narrow monotonically from
/// here (a `SPLIT` child inherits its parent's rights; `CAP_INSERT` narrows again), so `GRANT` never
/// appears anywhere it was not present at the root. Contrast [`memory_region_cap`], the `WRITE`-only
/// spend-only budget a leaf child receives.
///
/// **`ENUMERATE` too**, since milestone 126 (the `procps` package) and DECISIONS §225 (`free` sees the machine and your share) part 1.
/// `MemoryRegion::USAGE` is the first method on this object that consults it, and it answers what
/// the region was spent on and nothing more. It flows down every `SPLIT` with the rest, so the progenitor can hand a job budget's
/// holder a view narrowed to `ENUMERATE` alone, which is `free`'s "yours" line. The audit §114 (`ENUMERATE` extends to the address-space object) asked
/// for when `ENUMERATE` first reached the address-space object applies here the same way: a holder
/// that could already spend, split and destroy the region learns nothing new by asking what it spent.
///
/// **`Rights::ALL`, every defined bit, since milestone 824** (a retype mints no right its budget
/// lacks). A retype now mints the budget's rights and no others, so the root is where every right on
/// every object ultimately comes from: the progenitor's processes need `BIND` to hand a `std`
/// program its own process, and that `BIND` has to be on the budget they are retyped from. `READ`
/// and `BIND` are consulted by no `MemoryRegion` method, so on the region itself they confer
/// nothing. `ALL` rather than a list for the reason `syscall::retype` gives: a list stops being
/// full the next time a right is added.
pub fn memory_region_root_cap(region: u64) -> Cap {
    memory_region_cap_rights(region, Rights::ALL)
}

/// A capability to a virtio device's transport. `WRITE` lets the holder operate it.
pub fn virtio_cap(id: usize) -> Cap {
    Cap {
        object: Object::Virtio(id),
        rights: Rights::WRITE,
    }
}

/// A virtio transport capability with explicit rights (DECISIONS §120's 2026-08-26 amendment):
/// The progenitor holds one with `GRANT` so it can delegate the device to an entropy service it builds,
/// [`irq_cap_rights`]'s own reason one object type over.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn virtio_cap_rights(id: usize, rights: Rights) -> Cap {
    Cap {
        object: Object::Virtio(id),
        rights,
    }
}

/// A capability naming a contiguous range of x86 I/O ports (milestone 299). `WRITE` lets the holder
/// map it (a port is read/write by nature, like a device page); minted by the kernel for a known
/// device, the way [`device_frame_cap`] mints one for an MMIO device. The progenitor holds one with
/// `GRANT` so it can delegate the ports to the console and input drivers it builds, the same
/// disposition [`device_frame_cap`] gets on the other two architectures. `x86_64` only, like the
/// [`Object::PortRange`] it constructs: there is no port space to name elsewhere.
#[cfg(target_arch = "x86_64")]
pub fn port_range_cap(base: u16, count: u16, rights: Rights) -> Cap {
    Cap {
        object: Object::PortRange(base, count),
        rights,
    }
}

/// A capability naming a thread under construction (milestone 19c (run a real workload), step 19c.3), for tests that build one by
/// hand. `RETYPE_OBJ` mints its own through the budget's `mint_child` since milestone 824.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn thread_control_block_cap(tid: crate::thread::ThreadId, rights: Rights) -> Cap {
    Cap {
        object: Object::ThreadControlBlock(tid),
        rights,
    }
}

/// A capability naming a notification (milestone 151). `RETYPE_OBJ` mints one with its budget's
/// rights (milestone 824); delegation narrows.
pub fn notification_cap(id: crate::sched::NotificationId, rights: Rights) -> Cap {
    Cap {
        object: Object::Notification(id),
        rights,
    }
}

/// **The one reboot capability** (milestone 805), for the boot that grants it to the progenitor.
///
/// Name: ratified 2026-10-06 (calef, #1783: "I don't think we should abbreviate capability as
/// cap."), from `reboot_cap`.
pub fn reboot_capability(rights: Rights) -> Cap {
    Cap {
        object: Object::Reboot,
        rights,
    }
}

/// A capability naming a timer (milestone 106). `RETYPE_OBJ` mints one with its budget's rights
/// (milestone 824); delegation narrows.
pub fn timer_cap(id: crate::sched::TimerId, rights: Rights) -> Cap {
    Cap {
        object: Object::Timer(id),
        rights,
    }
}

/// A capability naming a process (milestone 812), for tests that build one by hand. `RETYPE_OBJ`
/// mints its own with its budget's rights, `BIND` among them when the budget has it (milestone 824).
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn process_capability(id: crate::sched::ProcessId, rights: Rights) -> Cap {
    Cap {
        object: Object::Process(id),
        rights,
    }
}

/// A capability naming an address space under construction (milestone 19b). `RETYPE_OBJ` mints one
/// with its budget's rights (milestone 824); delegation narrows, as everywhere.
pub fn address_space_cap(name: u64, rights: Rights) -> Cap {
    Cap {
        object: Object::AddressSpace(name),
        rights,
    }
}

/// A one-shot reply capability naming the caller `tid` (milestone 12). Minted with `WRITE` (may
/// answer) and **no `GRANT`** (cannot be delegated onward), so it is non-transferable as well as
/// single-use. The kernel is the only minter, at a `CALL` rendezvous.
pub fn reply_cap(tid: crate::thread::ThreadId) -> Cap {
    Cap {
        object: Object::Reply(tid),
        rights: Rights::WRITE,
    }
}

/// A capability naming a device's MMIO page (milestone 19d.2). `WRITE` lets the holder map it
/// (device access is read/write by nature); minted by the kernel for a known device. Same
/// disposition as [`Object::DeviceFrame`]: riscv mints one only in the `shell` boot mode.
#[cfg_attr(all(target_arch = "riscv64", not(feature = "shell")), allow(dead_code))]
pub fn device_frame_cap(phys: u64, rights: Rights) -> Cap {
    Cap {
        object: Object::DeviceFrame(phys),
        rights,
    }
}

/// A capability naming **one** physical page at `phys` (the `count: 1` case of DECISIONS §102's
/// `PageFrame`). `READ` lets the holder map it read-only, `WRITE` lets it map it read/write,
/// `GRANT` lets it pass the page on. A freshly retyped frame gets all three; delegation narrows
/// them (a read-only, non-lendable view is `READ` alone).
///
/// (This doc comment was attached to [`device_frame_cap`] until milestone 108, so `page_frame_cap` had
/// none and the device one appeared to have two. A correction, not a rewrite.)
///
/// The kernel mints one directly when it owns a page a program should hold: a driver's DMA region,
/// a shared buffer, the clock page. Since milestone 108 that is how the disk and display paths hand
/// a driver its memory, in place of a `Spawn::maps` entry that no capability stood behind. See
/// notes/frames.md.
///
/// Kept at its pre-§102 signature on purpose: every one of its ~20 existing callers wants exactly
/// one page, and this is additive rather than a rewrite (§102's own text: "existing callers pass
/// count: 1 and get the same behavior"). [`page_frame_run_cap`] is the one to reach for when the
/// count is not 1.
pub fn page_frame_cap(phys: u64, rights: Rights) -> Cap {
    Cap {
        object: Object::PageFrame(phys, NonZeroU64::MIN),
        rights,
    }
}

/// A capability naming a run of `count` contiguous physical pages starting at `phys` (DECISIONS
/// §102, 2026-08-20). The run-capable sibling of [`page_frame_cap`]: one capability for a DMA
/// region or a scanout that is contiguous in physics, in the address space, and in the IOMMU domain
/// that confines it, instead of one capability per page. `count: 1` ([`NonZeroU64::MIN`]) is
/// exactly [`page_frame_cap`].
///
/// The count is a [`NonZeroU64`] rather than a `u64` guarded by a `debug_assert!`, which is what
/// this took until the milestone 142 review: a `debug_assert!` is compiled out of the release build
/// `xtask bench --release` runs, so the one build whose numbers get published was the one build
/// with no check at all. See [`Object::PageFrame`] for what a zero would have underflowed.
pub fn page_frame_run_cap(phys: u64, count: NonZeroU64, rights: Rights) -> Cap {
    Cap {
        object: Object::PageFrame(phys, count),
        rights,
    }
}

/// `n` pages as a run length, refusing zero.
///
/// The bridge for the callers that compute a page count arithmetically (a surface's
/// `SURFACE_BYTES.div_ceil(4096)`, a DMA region's `1 + surface`) and would otherwise each spell out
/// the same `NonZeroU64::new(..).expect(..)`. `const`, so a caller passing a literal or a `const`
/// gets the refusal **at compile time** rather than at boot: that is the rung this whole
/// widening's zero-check wanted and the reason the function exists at all rather than the call
/// sites doing it themselves.
pub const fn page_frame_run_len(n: u64) -> NonZeroU64 {
    match NonZeroU64::new(n) {
        Some(count) => count,
        None => panic!("a PageFrame run must name at least one page"),
    }
}
