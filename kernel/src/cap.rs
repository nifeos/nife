//! What a capability in nife can point at.
//!
//! The table itself is `crates/capability`, which is pure logic and knows nothing about this file.
//! This is the kernel's half: **the set of nouns.**
//!
//! DECISIONS §10, notes/capabilities.md, and notes/capability-objects.md for each object.

use core::num::NonZeroU64;

/// Every kind of thing a process can be handed: the kernel's set of nouns. Each variant is a
/// decision, and notes/capability-objects.md says why each is here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Object {
    /// An IPC endpoint, by generational name in the scheduler's registry, and the capability's
    /// badge. `WRITE` lets the holder `SEND` and `READ` lets it `RECEIVE`. The badge is a `u64` the
    /// kernel delivers to a server's `RECEIVE_CAP` so it can tell apart clients sharing one
    /// endpoint; `0` is unbadged, and `abi::rendezvous::BADGE` sets one once (milestone 599 (a
    /// frame per filesystem client channel)). See notes/capability-objects.md.
    Rendezvous(crate::sched::RendezvousId, u64),

    /// **A memory region** (milestone 11): a capability to a chunk of raw physical memory the
    /// process may retype into pages. Invoking it grows the process's address space out of its
    /// own budget, and the kernel allocates nothing to do it. See `kernel/src/memory_region.rs`.
    MemoryRegion(u64),

    /// A hardware interrupt, by INTID. The holder may `WAIT` for it and `ACK` it; the kernel's
    /// handler masks the line and turns the interrupt into a message, so a driver owns an interrupt
    /// without owning any privilege.
    Irq(u32),

    /// A contiguous run of physical pages, by base address and page count, that the holder may map
    /// and delegate: the shared-memory capability of §10 (process model: capability-based,
    /// microkernel). `PageFrame::MAP` maps the whole run, and `PageFrame::REVOKE` unmaps it and
    /// deletes every capability naming it (§102 (a Frame names a run of pages)). The count is
    /// `NonZeroU64` because a zero-page run would underflow every end address derived from it. See
    /// notes/capability-objects.md and notes/frames.md.
    PageFrame(u64, NonZeroU64),

    /// A device's MMIO page, by physical address: the authority to map one device's registers with
    /// device memory attributes (uncacheable, unreordered), which a `PageFrame`'s cacheable mapping
    /// must never give MMIO. The kernel mints one per known device, in `boot_progenitor`,
    /// `spawn_elf_child_builder` and the system tests, for the progenitor to delegate.
    DeviceFrame(u64),

    /// A one-shot reply channel to a blocked caller, named by the caller's thread id. The kernel
    /// mints one at a `CALL` rendezvous and hands it to the server; invoking it delivers the reply
    /// to exactly that caller and consumes the capability. See §12 (call/reply IPC: a one-shot
    /// reply capability) and notes/ipc-naming.md.
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

    /// A contiguous range of x86 I/O ports, by base port and count, enforced by the TSS I/O
    /// permission bitmap rather than the MMU. The holder may execute `in`/`out` on `[base, base +
    /// count)` from ring 3 and on no other port. The capability must carry `WRITE`, because the
    /// bitmap cannot grant `in` without `out` (`sched::thread_control_block_insert_from`). `x86_64`
    /// only: the other architectures have no port space. See notes/capability-objects.md and
    /// `kernel/src/arch/x86_64/segments.rs`.
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

// A slot is 32 bytes: `Object` is as wide as `PageFrame`, its widest variant, plus the rights.
// §102's slot-count arithmetic assumed 24; these pin the real figure. When they fire, update them
// and re-read §102. notes/capability-objects.md has the arithmetic.
const _: () = assert!(core::mem::size_of::<Object>() == 24);
const _: () = assert!(core::mem::size_of::<Cap>() == 32);

/// A thread's capability table: [`CAPABILITY_TABLE_SLOTS`] slots fixed at the type, so creating one
/// cannot allocate. A full table fails silently (the progenitor dies before a console exists), so
/// the size is a measured peak plus headroom, [`CAPABILITY_TABLE_PEAK_MEASURED`].
///
/// 64 is calef's ruling of 2026-10-04 (UTC), milestone 754 (the capability table grows to 64
/// slots). The fault slot is 63. Per-process sizing is
/// `design/roadmap/0778-capability-tables-sized-per-process.md`. See notes/capability-objects.md.
pub const CAPABILITY_TABLE_SLOTS: usize = 64;

// The free-slot word is a `u64`, so 64 is also the ceiling the type allows. `capability` asserts it
// for every size; it is repeated here, where the next raise will be typed. Raising past it needs a
// wider word or a second one (`capability::MAX_SLOTS`).
const _: () = assert!(
    CAPABILITY_TABLE_SLOTS <= capability::MAX_SLOTS,
    "the capability table's free-slot bitmap is one u64; widen it before raising the slot count"
);
// The table's layout, pinned: 64 slots of 32 bytes, the `u64` free word and two `u16` counts,
// rounded to eight. If this fires, `crate::thread`'s page-fit check is the next thing to read.
const _: () = assert!(core::mem::size_of::<CapabilityTable>() == 2064);
pub(crate) type CapabilityTable = capability::CapabilityTable<Object, CAPABILITY_TABLE_SLOTS>;

/// The progenitor's peak capability count on `script/swish-check`'s aarch64 leg: 37 of
/// [`CAPABILITY_TABLE_SLOTS`] (2026-10-19 UTC). A record, not a target: [`report_peak`] says on the
/// console when a boot goes past it, and `script/swish-check` fails on that sentence. When it
/// fires, measure and update this rather than raising the slot count.
/// `notes/capability-peak-trace.md` itemizes the peak, and notes/capability-objects.md has the
/// measurement.
pub(crate) const CAPABILITY_TABLE_PEAK_MEASURED: usize = 37;

// The headroom is what this pair means, so the two cannot silently invert.
const _: () = assert!(CAPABILITY_TABLE_PEAK_MEASURED < CAPABILITY_TABLE_SLOTS);

/// The highest peak [`report_peak`] has already said out loud, so the same number is not printed
/// twice by two cores or on two passes.
static PEAK_REPORTED: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// What the previous idle pass saw, which is half of the coalescing. See [`report_peak`].
static PEAK_LAST_PASS: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// How many consecutive idle passes have seen the same peak. The other half of the coalescing.
static PEAK_STABLE_PASSES: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

/// How many idle passes the mark must stay still before [`report_peak`] believes the climb is over.
/// A coalescing window, not a margin on the measurement: too few prints a line at each IPC pause in
/// the progenitor's climb, and no value can make the peak wrong. See notes/capability-objects.md.
const PEAK_STABLE_PASSES_NEEDED: usize = 16;

/// Say the boot's capability-slot high-water mark once it has stopped moving. Called from the
/// scheduler's idle loop. It waits for [`PEAK_STABLE_PASSES_NEEDED`] passes with the mark
/// unchanged, and `fetch_max` lets exactly one caller report a new peak, so a four-core machine
/// prints one line. The number is `capability::highest_seen`'s, the highest any table reached, not
/// a named thread's (notes/capability-objects.md's `BUGS`).
pub(crate) fn report_peak() {
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

/// Say the gauge out loud. Split from [`report_peak`] so the test kernel leaves out the whole
/// sentence. The test kernel counts and does not print: the guest suite's transcript is a gate's
/// input that a third writer would split, and the interactive boot's record is the wrong yardstick
/// for the suite's workload.
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

// The rights vocabulary is written twice, in two crates that cannot see each other: `abi::rights`
// is what userspace names a right with, `capability::Rights` what the kernel means by one. These
// make a right added to one and not the other a compile error rather than a delegation that drops a
// bit at `from_bits`. See notes/capability-objects.md.
const _: () = assert!(Rights::READ.bits() as u64 == abi::rights::READ);
const _: () = assert!(Rights::WRITE.bits() as u64 == abi::rights::WRITE);
const _: () = assert!(Rights::GRANT.bits() as u64 == abi::rights::GRANT);
const _: () = assert!(Rights::ENUMERATE.bits() as u64 == abi::rights::ENUMERATE);
const _: () = assert!(Rights::BIND.bits() as u64 == abi::rights::BIND);

// **`ALL` is exactly the ABI's vocabulary and no more.** Both directions are load-bearing and they
// fail differently. A bit in `abi::rights` that is missing from `ALL` is a right userspace can name
// and `Rights::from_bits` masks to zero, so a delegation asking for it succeeds while conferring
// nothing. A bit in `ALL` with no ABI name is a right the kernel honors that no caller can ask for
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

impl Object {
    /// **How many page frames a holder of this object may map into itself**: a `PageFrame` run's
    /// length, and zero for everything else. What `user::entry::run_with` sizes a new process's
    /// region by, so the revocation log has room for every page it maps from a grant.
    ///
    /// Name: provisional.
    pub const fn mappable_pages(&self) -> u64 {
        match self {
            Object::PageFrame(_, count) => count.get(),
            _ => 0,
        }
    }
}

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

/// A capability to an untyped region with every right but `GRANT`: the spend-only budget a leaf
/// child is handed. The holder may retype, `SPLIT`, `MAP` and `DESTROY` but not delegate, since
/// `SEND_CAP` and `CAP_INSERT` gate on `GRANT`. A retype mints its budget's rights (milestone 824
/// (a retype mints no right its budget lacks)), so a leaf can use what it carves and lend none of
/// it. See notes/capability-objects.md.
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

/// A capability to an untyped region with explicit rights. Its one caller is
/// [`memory_region_root_cap`]; `MemoryRegion::SPLIT` mints through `Cap::mint_child`, whose
/// `split_never_widens_rights` proof pins a child's rights to its parent's. Rights are set once at
/// the root and only narrow below it.
pub fn memory_region_cap_rights(region: u64, rights: Rights) -> Cap {
    Cap {
        object: Object::MemoryRegion(region),
        rights,
    }
}

/// The delegable root untyped the kernel hands the progenitor at boot, with [`Rights::ALL`]. Every
/// right on every object comes from here, since a retype mints its budget's rights, and rights only
/// narrow below it (a `SPLIT` child inherits, `CAP_INSERT` narrows). `ENUMERATE` lets
/// `MemoryRegion::USAGE` answer what the region was spent on. See notes/capability-objects.md.
pub fn memory_region_root_cap(region: u64) -> Cap {
    memory_region_cap_rights(region, Rights::ALL)
}

/// A capability to a virtio device's transport. `WRITE` lets the holder operate it.
pub(crate) fn virtio_cap(id: usize) -> Cap {
    Cap {
        object: Object::Virtio(id),
        rights: Rights::WRITE,
    }
}

/// A virtio transport capability with explicit rights (DECISIONS §120's 2026-08-26 amendment):
/// The progenitor holds one with `GRANT` so it can delegate the device to an entropy service it builds,
/// [`irq_cap_rights`]'s own reason one object type over.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) fn virtio_cap_rights(id: usize, rights: Rights) -> Cap {
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
pub(crate) fn reply_cap(tid: crate::thread::ThreadId) -> Cap {
    Cap {
        object: Object::Reply(tid),
        rights: Rights::WRITE,
    }
}

/// A capability naming a device's MMIO page (milestone 19d.2). `WRITE` lets the holder map it
/// (device access is read/write by nature); minted by the kernel for a known device. Same
/// callers as [`Object::DeviceFrame`].
pub fn device_frame_cap(phys: u64, rights: Rights) -> Cap {
    Cap {
        object: Object::DeviceFrame(phys),
        rights,
    }
}

/// A capability naming one physical page at `phys`, a run of 1. `READ` maps it read-only, `WRITE`
/// read/write, and `GRANT` passes it on; a fresh retype gets all three and delegation narrows them.
/// The kernel mints one directly for a page a program should hold: a DMA region, a shared buffer,
/// the clock page. [`page_frame_run_cap`] is for any other count. See notes/frames.md.
pub fn page_frame_cap(phys: u64, rights: Rights) -> Cap {
    Cap {
        object: Object::PageFrame(phys, NonZeroU64::MIN),
        rights,
    }
}

/// A capability naming a run of `count` contiguous physical pages from `phys`: one capability for a
/// DMA region or a scanout rather than one per page. `count: 1` ([`NonZeroU64::MIN`]) is
/// [`page_frame_cap`]. The count is a [`NonZeroU64`] because a `debug_assert!` would be compiled
/// out of the release build `xtask bench --release` publishes.
pub fn page_frame_run_cap(phys: u64, count: NonZeroU64, rights: Rights) -> Cap {
    Cap {
        object: Object::PageFrame(phys, count),
        rights,
    }
}

/// `n` pages as a run length, refusing zero. `const`, so a caller passing a literal or a `const`
/// gets the refusal at compile time rather than at boot.
pub const fn page_frame_run_len(n: u64) -> NonZeroU64 {
    match NonZeroU64::new(n) {
        Some(count) => count,
        None => panic!("a PageFrame run must name at least one page"),
    }
}
