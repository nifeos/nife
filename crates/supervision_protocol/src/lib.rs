#![no_std]
// `Result<_, ()>` is deliberate throughout the build path, and clippy's `result_unit_err` is asking
// for an error type this layer has nothing to put in. Every failure here is a syscall that already
// returned its own error to the caller through the ABI; a second, richer error would be inventing
// detail the kernel did not provide. The unit is honest: it means "the syscall said no", and the
// caller's recourse is the same regardless of which one.
//
// Per-crate rather than per-item because it is one decision about one calling convention, and
// DECISIONS §38's rule is against blanket dead-code suppression hiding unreachable code, which this
// is not: nothing here is hidden, and the lint is a style opinion about an error type.
#![allow(clippy::result_unit_err)]
// `ChildEndowment::new(retention)` is an empty endowment, and a `Default` impl would give a second
// spelling for the same thing in a crate whose whole job is that two binaries agree on one
// spelling. Since DECISIONS §142 it could not be `Default` anyway: the constructor takes the
// retention declaration precisely so that nothing supplies one silently.
#![allow(clippy::new_without_default)]
//! **The supervision tree: the shared half.** The programs that shrink init's authority
//! (`root_supervisor`, `spawner`, `sub_server_supervisor`, and the `flaky` sub-server they manage)
//! share this: the protocol words they speak, and **the tree's only userspace ELF loader**.
//! `system_initializer` builds with it too; nothing else builds a process in userspace.
//!
//! The loader takes the builder's own budget and the budget the child is built *from* as separate
//! arguments (the same for a server building from its own memory, different for a spawner building
//! each child in its own reclaimable region), a capability for the reserved fault slot so the child
//! is born supervised, capabilities for named slots, and **blobs** to copy into the child, which is
//! what lets a construction sub-server hold one program image instead of the whole initial ramdisk.
//! A [`ChildEndowment`] states the child's complete authority in both directions: what it is given,
//! and what its spawner keeps (§142 (what a spawner retains over a child after `START`)).
//!
//! The examples are not checked by the gate: this crate depends on `user_mode_runtime`, so
//! `script/test`'s host pass excludes it; they run under `cargo test --doc -p supervision_protocol`
//! on an aarch64 host.
//!
//! Name: recorded (design/naming.md's crate section). The `_protocol` contract suffix (calef,
//! 2026-09-05) plus the stem, the tree's word for the restart discipline; `ChildEndowment` is an
//! open naming question of its own (§69 (`Endow` is a verb)).

use user_mode_runtime::{cap_delete, invoke};

// ===========================================================================================
// The report protocol. Every process in the tree holds a WRITE view of one report endpoint and
// says what happened on it; the kernel test is the receiver. Mirrored in kernel/src/user.rs
// (`authority_tests`), the same way the net client's selectors are.
// ===========================================================================================

/// init dropped its construction authority. `w1` = 1 if using the dropped budget then failed,
/// `w2` = the negated error code (1 = `NoSuchSlot`: the slot is empty, there is nothing to name).
pub const REPORT_INIT_DROPPED: u64 = 1;
/// A sub-server instance ran. `w1` = which attempt it is (0 = the original, 1+ = a restart).
pub const REPORT_SERVER_RAN: u64 = 2;
/// The sub-server's supervisor saw one of its children die. `w1` = the child's label (one of
/// [`SUB_SERVER_LABELS`]) as the kernel delivered it with the death message, `w2` = the event
/// (fault or exit). `root_supervisor` reports its own children's deaths under the same kind, with
/// the tid in `w1`, because it labels nobody. *(The label in `w1` is milestone 105 (the two forks)'s; it
/// was the tid until then.)*
pub const REPORT_SUP_SAW_DEATH: u64 = 3;
/// The supervisor's retry budget ran out; its policy is to stop. `w1` = restarts attempted.
pub const REPORT_SUP_GAVE_UP: u64 = 4;
/// Something in the tree could not be built. `w1` = a stage code, so a broken tree is debuggable
/// rather than silent.
pub const REPORT_FAILED: u64 = 9;

// ===========================================================================================
// The construction protocol: the supervisor asks, the spawner builds. This is the authority split
// made concrete. The supervisor holds no memory at all, so it cannot build anything itself; the
// spawner holds a budget and exactly one program image, so it cannot build anything else.
//
// **Building is all that is asked here now** (DECISIONS §32). There used to be a `REQ_REAP` too,
// because reaping was `MemoryRegion::DESTROY` and only the spawner held the region capability, so the
// supervisor had to proxy the reap through the process that could. §32 put the reap on the
// supervision endpoint the supervisor already holds, so the hop is gone and with it the handle the
// spawner had to invent to name an instance the kernel names by tid.
// ===========================================================================================

/// `send(req, REQ_BUILD, attempt, label)` -> `(REP_BUILT, 0, 0)` or `(REP_FAILED, 0, 0)`. The
/// spawner starts the child with `attempt` in its second argument register and stamps `label` on
/// its supervision capability (milestone 105), so the kernel hands `label` back with its death
/// message. `label` must be nonzero, because `rendezvous::BADGE` refuses zero; the child is never
/// told it.
pub const REQ_BUILD: u64 = 1;

/// **The labels `sub_server_supervisor` gives its two children, and the attempt each starts at**
/// (milestone 105, DECISIONS §148 (resolves by asking the kernel) as amended 2026-10-04; name
/// provisional). The supervisor
/// tells its children apart by nothing else. The first starts at attempt 0, which `flaky` crashes
/// on; the second at attempt 10, which it finishes cleanly. So a supervisor that confused the two
/// would restart the wrong one, which is what makes the difference observable. Mirrored in the
/// kernel's `authority_tests`.
///
/// Name: provisional, milestone 105 (the two forks)'s lane, 2026-10-05 (UTC). Plural because it holds both children's labels and their
/// first attempts; `CHILD_LABELS` was considered and is less specific about whose children.
pub const SUB_SERVER_LABELS: [(u64, u64); 2] = [(0x1abe_0a01, 0), (0x1abe_0b02, 10)];

/// Reply code: the build succeeded.
pub const REP_BUILT: u64 = 1;
/// Reply code: the build failed (out of budget, a bad image, or a loader refusal).
pub const REP_FAILED: u64 = 3;

// ===========================================================================================
// The loader.
// ===========================================================================================

/// The page size the loader maps in.
pub const PAGE: u64 = 4096;

/// Where a child's highest stack page sits. One address for every process this system builds, which
/// is what lets [`configure_child`] compute the entry `sp` without being told.
///
/// **The map's `STACK_TOP_PAGE`** (milestone 206 (a program image has under 896 KiB), DECISIONS §171 (where a program image starts) option D), the same page
/// `kernel::user::USER_STACK_VA` names, derived rather than restated. It was `0x50_0000` until
/// 2026-09-26, and moving the kernel's copy alone broke `authority_tests` at stage 10 because this
/// one did not move with it; now neither can move without the other.
pub const CHILD_STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;

/// The stack a child gets when its builder does not say otherwise ([`ChildEndowment::new`]). Four pages,
/// which is enough for the supervision tree's programs; the flaky sub-server would be fine with one.
///
/// **A child at the interactive prompt gets three times this** (`system_initializer::CHILD_STACK_PAGES`),
/// and the difference is deliberate rather than drift: the prompt's children run the shell's
/// redirection path, whose frames grew twice under measurement. The number is a field on [`ChildEndowment`]
/// so a caller states it, because a builder that silently inherits somebody else's stack size finds
/// faults that builder does not have (notes/pipes/the-boot.md).
pub const CHILD_STACK_PAGES: u64 = 4;

/// **The builder's scratch window**: where a builder maps each frame it fills for a child, and each
/// frame of somebody else's it reads (the image request of DECISIONS §219 (how the shell names an
/// installed program to the spawner)), in its own address space.
/// 64 MiB, 16,384 pages, from the bottom of the map's runtime windows.
///
/// **Why a window rather than a cursor that only climbs** (milestone 604 (provisional), the
/// builder's scratch cursor is bounded). Written when there was no unmap: a builder that holds no
/// capability to its own address space still cannot give a scratch page back itself (one that does
/// gives each page up as soon as it is filled, [`give_up_own_page`]). But the kernel takes
/// it back for the builder: a scratch page maps a frame retyped from the child's region, and
/// destroying that region revokes every mapping of its pages, the builder's included (DECISIONS §13
/// (frame revocation)). So the page is free again once the child is reaped, and only the builder's
/// cursor did not know it. Until 2026-09-26 the cursor advanced one page per page built and never
/// came back; in the progenitor it ran into the kernel's initial ramdisk window at `0x2000_0000`
/// after 256 MiB, or into the end of its own page-table budget first (milestone 604's block has the
/// arithmetic). Now [`map_scratch`] wraps at the window's end and asks the kernel, page by page,
/// which addresses are free.
///
/// **Siting.** In [`address_space_map::RUNTIME_WINDOWS`], starting where the cursor always started,
/// and ending well under the progenitor's initial ramdisk window. `std_runtime_contract`'s three
/// pages sit inside it, but only in a `std` program's space, and no `std` program builds children.
/// A window some other party does map in a builder's space costs a probe, not a failure:
/// [`map_scratch`] skips any page the kernel says is taken.
///
/// **Why this size.** Big enough to hold every page one builder can have built and not yet
/// reaped, which is what must fit at once; small enough that its page tables are a budget a
/// builder can state ([`SCRATCH_TABLE_PAGES`]). The progenitor checks both at compile time against
/// its own numbers (`system_initializer::INIT_OWN_PAGES` and `JOBS_BUDGET_PAGES`).
///
/// Name: provisional (milestone 604, 2026-09-26).
pub const SCRATCH_WINDOW: address_space_map::Band = address_space_map::Band {
    start: address_space_map::runtime_window(0x1000_0000),
    end: address_space_map::runtime_window(0x1400_0000),
};

/// [`SCRATCH_WINDOW`] in pages. Name: provisional (milestone 604).
pub const SCRATCH_WINDOW_PAGES: u64 = SCRATCH_WINDOW.bytes() / PAGE;

/// **The most last-level page tables [`SCRATCH_WINDOW`] can cost a builder**: one per 512 pages,
/// which is a 4 KiB table of 8-byte entries on all three architectures. A builder pays them out of
/// the budget it passes as `own_ut`, once, as the cursor first reaches each 2 MiB; after the first
/// lap every table already exists. The levels above are one or two tables shared with everything
/// else the builder maps under 1 GiB. Name: provisional (milestone 604).
pub const SCRATCH_TABLE_PAGES: u64 = SCRATCH_WINDOW_PAGES / 512;

const _: () = assert!(
    SCRATCH_WINDOW.start.is_multiple_of(512 * PAGE)
        && SCRATCH_WINDOW.end.is_multiple_of(512 * PAGE)
);

/// The next page [`map_scratch`] tries. Starts at the window's base and wraps.
static SCRATCH_NEXT: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(SCRATCH_WINDOW.start);

/// **Map `frame` at the next free page of [`SCRATCH_WINDOW`] in our own address space**, with page
/// tables from `own_ut`, and return the address. `writable` asks for read/write (the frame needs
/// `WRITE`), otherwise read-only (`READ`).
///
/// **The kernel's page table is the free list.** `PageFrame::MAP` refuses an address that is
/// already mapped, and refuses it whole, so a refusal here means "taken" and the next page is
/// tried. That refusal is `BadPointer`, which the kernel also returns for a misaligned or
/// kernel-half address; neither can happen here, because every address this tries is a page of
/// [`SCRATCH_WINDOW`]. Any other refusal (a frame without the right, an `own_ut` out of pages for a
/// table) is returned at once, and the cursor stays where it was.
///
/// The cursor wraps at the window's end, so a page a reaped child's region gave back is used again
/// on the next lap. Pages that stay mapped for good (the boot servers' in the progenitor, a running
/// job's) cost one refused probe per lap each. `Err` after a whole lap with nothing free, which
/// means the builder holds [`SCRATCH_WINDOW_PAGES`] pages of live children at once.
///
/// # BUGS
///
/// - **One thread per builder**, which is every builder in the tree: the cursor is read, probed and
///   written back without a lock, so two threads building at once could try the same page. The
///   kernel would refuse the second, and it would then be skipped, so the failure is a wasted probe
///   rather than a shared page.
/// - **`BadPointer` means "taken" only because every address tried is valid.** A distinct
///   `AlreadyMapped` error would let the probe say what it means; that is an ABI change, not worth
///   one on its own.
/// - **A builder that holds no capability to its own address space still maps every live child's
///   pages**, read/write. Wrapping bounds how much address space that costs, not what it lets the
///   builder reach. One that does (the progenitor, at slot 28 since §249 (a running address space
///   stays nameable)) names it with [`give_up_scratch_through`], and then every page this crate
///   fills for a child is given up with `AddressSpace::UNMAP` the moment it is in the child
///   ([`give_up_own_page`]). No other builder in the tree is handed one: §249 makes the grant the
///   builder's builder's choice, and nothing chose it for them.
/// - **The progenitor's use of it is computed, not measured**: milestone 604's block has the
///   arithmetic, and `system_initializer` checks its budgets against the window at compile time.
///
/// Name: provisional (milestone 604). It replaces `scratch_pages`, which handed out a contiguous
/// run that was never reused.
pub fn map_scratch(frame: u64, writable: bool, own_ut: u64) -> Result<u64, ()> {
    use core::sync::atomic::Ordering::Relaxed;
    let mut va = SCRATCH_NEXT.load(Relaxed);
    for _ in 0..SCRATCH_WINDOW_PAGES {
        let next = if va + PAGE >= SCRATCH_WINDOW.end {
            SCRATCH_WINDOW.start
        } else {
            va + PAGE
        };
        // SAFETY: `invoke` is the syscall; the kernel validates the capability, the rights and the
        // address, and maps nothing on any refusal.
        let r = unsafe { invoke(frame, abi::page_frame::MAP, va, u64::from(writable), own_ut) };
        if r == 0 {
            SCRATCH_NEXT.store(next, Relaxed);
            return Ok(va);
        }
        if r != abi::Error::BadPointer as i64 {
            SCRATCH_NEXT.store(va, Relaxed);
            return Err(());
        }
        va = next;
    }
    SCRATCH_NEXT.store(va, Relaxed);
    Err(())
}

/// **The builder's own address space**, or [`NO_OWN_SPACE`] when it was never named (§249 (a running
/// address space stays nameable); milestone 95 (an unmap primitive)). Set once, by
/// [`give_up_scratch_through`], and read by [`give_up_own_page`].
static OWN_SPACE: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(NO_OWN_SPACE);

/// [`OWN_SPACE`]'s "never named". `u64::MAX` is no capability slot.
const NO_OWN_SPACE: u64 = u64::MAX;

/// **Name the slot holding a `WRITE` capability to this builder's own address space**, so the
/// scratch window is given back page by page rather than held for the life of the child (milestone
/// 95 (an unmap primitive), DECISIONS §162 (whether a holder can give up a mapping), §249 (a running
/// address space stays nameable)).
///
/// The progenitor is the one caller: the kernel grants it a capability to its own space at slot 28
/// on every boot. A builder that never calls this keeps today's behaviour, which is a scratch page
/// that stays mapped until the child's region is destroyed and revokes it (DECISIONS §13).
///
/// Name: provisional (milestone 95's §249 lane, 2026-10-05 UTC).
pub fn give_up_scratch_through(own_space: u64) {
    OWN_SPACE.store(own_space, core::sync::atomic::Ordering::Relaxed);
}

/// **Give up one page this process mapped for itself**: `AddressSpace::UNMAP` of `va` in its own
/// space, through the capability [`give_up_scratch_through`] named. A page of the scratch window
/// once the loader has filled it, and any other window a builder opened onto memory it hands on (the
/// progenitor's view of the shell's output page, and of two devices' DMA pages). `true` if the page
/// is gone, `false` if no capability was named or the kernel refused, in which case the page stays
/// mapped exactly as it did before §162 had a method, and a scratch page is taken back when the
/// child's region is.
///
/// After this the builder cannot reach the child's page at all: a write to `va` faults, which is
/// milestone 95's negative control (`system_tests`' `running_space_tests`). And the address is free
/// again, so [`map_scratch`]'s probe finds it on the next page rather than a lap later.
///
/// Name: provisional (milestone 95's §249 lane, 2026-10-05 UTC).
pub fn give_up_own_page(va: u64) -> bool {
    let own = OWN_SPACE.load(core::sync::atomic::Ordering::Relaxed);
    if own == NO_OWN_SPACE {
        return false;
    }
    // SAFETY: `invoke` is the syscall; the kernel validates the capability, the right and the
    // address, and changes nothing on a refusal. The caller is done with every byte at `va`.
    unsafe { invoke(own, abi::address_space::UNMAP, va, 0, 0) == 0 }
}

/// **Everything a child is born holding.** The same idea as the kernel's `Spawn`: read one of these
/// and you know the complete authority of the thing about to run.
///
/// # Examples
///
/// Every field is public and [`ChildEndowment::new`] is the nothing-endowment to build from:
///
/// ```
/// use supervision_protocol::{CHILD_STACK_PAGES, ChildEndowment, Retention};
///
/// // A sub-server that gets exactly one endpoint, is born supervised, and holds nothing else.
/// // `..ChildEndowment::new(Retention::Nothing)` is the intended shape: what is not listed is not
/// // granted, and what the spawner keeps is stated rather than inherited.
/// let endow = ChildEndowment {
///     caps: &[(4, 0b11)], // our slot 4, read/write, landing in the child's slot 0
///     fault: Some(7), // our slot 7 holds its supervision endpoint
///     ..ChildEndowment::new(Retention::Nothing)
/// };
///
/// assert_eq!(endow.caps.len(), 1);
/// assert!(endow.maps.is_empty() && endow.blobs.is_empty() && endow.placed.is_empty());
/// assert_eq!(endow.stack_pages, CHILD_STACK_PAGES);
/// assert_eq!(endow.retention, Retention::Nothing);
///
/// // A construction sub-server, holding exactly the one program image it may build. That is what
/// // `blobs` buys: the child is handed *data* it has no capability to reach, so the sub-server
/// // never needs the whole initial ramdisk.
/// let image = &[0x7f, b'E', b'L', b'F'][..];
/// let builder = ChildEndowment {
///     blobs: &[(0x2000_0000, image)],
///     ..ChildEndowment::new(Retention::Nothing)
/// };
/// assert_eq!(builder.blobs[0].1.len(), 4);
/// assert!(builder.fault.is_none()); // unsupervised, and visibly so
/// ```
pub struct ChildEndowment<'a> {
    /// Capabilities to insert, `(our_slot, rights)`, landing in the child's slots 0, 1, 2, ...
    pub caps: &'a [(u64, u64)],
    /// Capabilities to insert at a slot the caller **names**, `(child_slot, our_slot, rights)`,
    /// after [`caps`](ChildEndowment::caps). One caller today: a declared diagnostic stream (DECISIONS §67),
    /// which cannot take the next free slot because how many low slots a child gets depends on what
    /// else the command line granted it, and a program that probes one slot number needs that
    /// number not to move.
    ///
    /// It cannot collide with [`fault`](ChildEndowment::fault): that one lands in the last slot of the
    /// capability table, and a manifest's diagnostics slot is one the program reads at startup, far below it.
    ///
    /// `our_slot` may be [`CHILDS_OWN_SPACE`], which places the child's own address space rather
    /// than a capability of ours; see that constant.
    pub placed: &'a [(u64, u64, u64)],
    /// Pages of ours to map into the child, `(child_va, our_slot, mode)`.
    pub maps: &'a [(u64, u64, u64)],
    /// Bytes to copy into fresh pages in the child, at consecutive VAs from `va`. This is how a
    /// child is handed *data* it has no capability to reach: a program image, in our case.
    pub blobs: &'a [(u64, &'a [u8])],
    /// Our slot holding the child's supervision endpoint (DECISIONS §26). Placed in the reserved
    /// `FAULT_EP_SLOT`, where `START` reads it and clears it, so the child is born supervised and
    /// cannot forge messages on its own death channel.
    pub fault: Option<u64>,
    /// Stack pages, mapped down from [`CHILD_STACK_VA`]. See [`CHILD_STACK_PAGES`] for why this is
    /// a field a caller sets rather than one number for the whole tree.
    pub stack_pages: u64,
    /// **What the spawner keeps**, the reciprocal of every field above (DECISIONS §142). See
    /// [`Retention`]. There is no default: [`ChildEndowment::new`] takes it as an argument, so
    /// `..ChildEndowment::new(Retention::Nothing)` states it rather than inheriting it.
    pub retention: Retention,
}

/// **`our_slot` in a [`ChildEndowment::placed`] entry that means "the child's own address space"**
/// (§255 (each socket is its own capability); name provisional). For a server that maps pages its
/// clients hand it and must `UNMAP` one before the next client's page takes its place: the network
/// stack, which is the only caller today. The rights must be exactly `WRITE`, which lets the child
/// `MAP_INTO` and `UNMAP` its own space (`PageFrame::MAP` already let it half do that); anything else
/// refuses the spawn, so no child is ever handed `GRANT` on its own memory. It is inserted before
/// `CONFIGURE`, while the builder still holds the space, and goes on naming the space while the
/// child runs (§249 (a running address space stays nameable)).
///
/// **A sentinel rather than a field, and that is an exception.** A typed `Option<u64>` field was the
/// first version, and it cost the progenitor 352 bytes of stack: debug `boot` keeps about twenty-two
/// `ChildEndowment`s in its frame, each 16 bytes larger, and that pushed the install path below
/// `kernel/src/progenitor_stack.rs`'s floor. No capability slot is `u64::MAX`, so the value cannot
/// name one of ours by accident.
pub const CHILDS_OWN_SPACE: u64 = u64::MAX;

/// **[`ChildEndowment::placed`]'s marker for the child's own process** (milestone 812
/// (`std::thread::spawn` runs real threads in one address space)): place it and the child is built
/// as a process, so it can add threads to itself. [`build_child`] creates the process around the
/// child's space, inserts the process capability at the placed slot with the placed rights (which
/// must be exactly `BIND`, the authority to add a thread and nothing more), and joins the child to it
/// with `CONFIGURE`. A sentinel for [`CHILDS_OWN_SPACE`]'s stack reason.
pub const CHILDS_OWN_PROCESS: u64 = u64::MAX - 1;

/// **[`ChildEndowment::placed`]'s marker for the child's own first thread** (milestone 812): its TCB,
/// which a `std` program's main thread sets its own thread pointer through. The rights must be
/// exactly `WRITE`.
pub const CHILDS_OWN_THREAD: u64 = u64::MAX - 2;

impl<'a> ChildEndowment<'a> {
    /// An endowment of nothing: no capabilities, no mappings, no supervision, and the default
    /// stack, retaining whatever `retention` says. Every other field is public, so the intended use
    /// is `..ChildEndowment::new(Retention::Nothing)` at the end of a struct literal, which is also
    /// what keeps a later field from being a change to every caller.
    ///
    /// **Retention is the one thing this constructor will not supply for you**, and that is
    /// deliberate (DECISIONS §142). Every other field defaults to "not granted", which is a safe
    /// default because the failure mode of forgetting one is a child that cannot do something. The
    /// failure mode of forgetting retention is the opposite: a spawner that silently keeps
    /// authority over a running child. So it is an argument rather than a default, and the
    /// nothing-endowment idiom still reads as one line while saying both halves.
    pub const fn new(retention: Retention) -> Self {
        Self {
            caps: &[],
            placed: &[],
            maps: &[],
            blobs: &[],
            fault: None,
            stack_pages: CHILD_STACK_PAGES,
            retention,
        }
    }
}

/// **What the spawner keeps over the child after `START`** (DECISIONS §142). The reciprocal of the
/// rest of [`ChildEndowment`]: those fields say what the child is given, this one says what the
/// parent does not let go of.
///
/// The tree's answer today is [`Nothing`](Retention::Nothing), which is R0 in §142's vocabulary and
/// what 25 of the 30 `START` sites already did by calling `cap_delete` a line later. Writing it
/// down is the point: before this existed the convention was inferable only by grepping for
/// `cap_delete`, and a spawn path that forgot to decide looked exactly like one that had decided.
///
/// **Nothing here can grant an authority the kernel does not offer.** A `ThreadControlBlock`
/// capability held after `START` authorizes nothing at all: all three of its methods
/// (`CONFIGURE`, `CAP_INSERT`, `START`) refuse anything but an `Embryo`, and the refusal is in the
/// scheduler rather than the dispatcher. So retaining one is a claim about intent, not about power,
/// and that is why the retaining variant has to carry a reason.
///
/// # Examples
///
/// ```
/// use supervision_protocol::{ChildEndowment, Retention};
///
/// // The ordinary case, and it says so out loud rather than by omission.
/// let ordinary = ChildEndowment { fault: Some(7), ..ChildEndowment::new(Retention::Nothing) };
/// assert!(matches!(ordinary.retention, Retention::Nothing));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retention {
    /// The spawner keeps nothing. [`start_child`] deletes the `ThreadControlBlock` capability, so
    /// after `START` the only authority over this child is the region it was built from
    /// (`MemoryRegion::DESTROY`) and the supervision endpoint it was born under (`REAP`).
    Nothing,
    /// The spawner keeps the child's `ThreadControlBlock` capability past `START`, for the reason
    /// given.
    ///
    /// The reason is a required part of the variant rather than a comment beside it, which is
    /// milestone 50 (pipes and redirection)'s `InputDeclaration::Required` shape: a spawn path cannot keep a capability without
    /// saying why, because the variant will not build. Every one of the five sites that kept theirs
    /// before §142 was written had no reason at all, and all five turned out to be
    /// [`Nothing`](Retention::Nothing) once anybody asked.
    ///
    /// # BUGS
    ///
    /// **The reason is not checked and cannot be.** Nothing reads the string, no gate parses it,
    /// and `Retention::ThreadControlBlock { reason: "" }` compiles. This is rung one of AGENTS.md's
    /// ladder for the *question* (you cannot omit the declaration) and rung three for the *answer*
    /// (the reason is a written record at the thing itself). A caller determined to say nothing can
    /// still say nothing; what it cannot do is say nothing invisibly.
    ThreadControlBlock {
        /// Why this spawner keeps the capability, in the words a reader of the call site needs.
        reason: &'static str,
    },
}

/// **A built child, and what its builder declared it would keep.** What [`build_child`] returns and
/// [`start_child`] consumes.
///
/// The pair exists so the declaration travels with the handle. `build_child` cannot act on
/// [`ChildEndowment::retention`] itself, because the deletion happens at `START` and `START` is a
/// separate step (that seam is [`build_child_space`]'s, and milestone 23's live replacement needs
/// it). Returning the tcb alone would leave the declaration behind at the build site and the
/// deletion up to whoever remembered, which is the arrangement §142 exists to end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Child {
    /// The child's `ThreadControlBlock` capability slot, in the builder's own table.
    pub tcb: u64,
    /// What the builder declared it keeps once the child runs.
    pub retention: Retention,
}

/// Build a child from `elf` and configure it, ready for [`start_child`]. The whole job in one call,
/// which is what every caller but the hot-swap operator wants.
///
/// `own_ut` pays for **our** scratch mappings (they are ours, and a child's region must not have our
/// page tables freed under it when the child is reaped). `build_ut` is what the child itself is made
/// of: its address space, frames, stack, and TCB. Passing a per-child region as `build_ut` is what
/// makes a single `DESTROY` reap the whole instance.
///
/// # Examples
///
/// ```no_run
/// # use supervision_protocol::{ChildEndowment, Retention, build_child, start_child};
/// # fn demo(own: u64, per_child: u64, elf: &elf::Elf) -> Result<(), ()> {
/// let endow = ChildEndowment { fault: Some(7), ..ChildEndowment::new(Retention::Nothing) };
///
/// // `own` pays for OUR scratch mappings; `per_child` is what the child is made of. Passing a
/// // per-child region as the second argument is what makes a single `DESTROY` reap the whole
/// // instance, and passing our own would free our page tables under the child.
/// let child = build_child(own, per_child, elf, &endow)?;
/// start_child(child, 0, 0, 0); // and the tcb capability goes back here, as declared
/// # Ok(())
/// # }
/// ```
pub fn build_child(
    own_ut: u64,
    build_ut: u64,
    elf: &elf::Elf,
    endow: &ChildEndowment,
) -> Result<Child, ()> {
    let (child, aspace) = build_child_space(own_ut, build_ut, elf, endow)?;
    let process = endow
        .placed
        .iter()
        .find(|&&(_, ours, _)| ours == CHILDS_OWN_PROCESS);
    let configured = match process {
        None => configure_child(child.tcb, aspace, elf.entry()),
        Some(&(slot, _, rights)) => {
            configure_child_as_process(build_ut, child.tcb, aspace, elf.entry(), slot, rights)
        }
    };
    if configured.is_err() {
        // `CONFIGURE` consumes the address space only when it succeeds, so both are still ours.
        cap_delete(child.tcb);
        cap_delete(aspace);
        return Err(());
    }
    Ok(child)
}

/// Everything [`build_child`] does **except** the final `CONFIGURE`: lay each segment W^X at the VA
/// it names, map a stack, copy the blobs in, map `maps`, retype a TCB, insert the endowment.
/// Returns `(child, address space)`, both still held by us. The [`Child`] carries the retention the
/// endowment declared, so [`start_child`] can act on it without the caller re-stating it.
///
/// Split out for milestone 23's live replacement (DECISIONS §41), which needs to do one more thing
/// to the child's address space *between* building it and configuring it: map in a device the
/// operator could not hand over any earlier, because revoking it from the outgoing owner would have
/// taken the incoming owner's copy too. `CONFIGURE` consumes the address space capability, so once it has
/// run there is no way to reach the child's memory again; this is where that seam is.
///
/// **A failure gives back every capability slot it took** (milestone 757 (a test kernel fails a
/// process on its Nth retype), provisional). Until then every `?` here returned `Err(())` with the
/// address space, the frame it was part-way through and any TCB still in the **caller's** table,
/// recorded 2026-08-26 by milestone 49 (users, login, and attribution) and left because no test could reach it. Milestone 757's
/// sweep reached it: failing `login`'s tenth retype of a login (this function's first frame) left
/// one slot behind per failed login. Now the address space is deleted on the way out of
/// [`lay_out_child`], the TCB on the way out of [`endow_child`], and a frame by whoever retyped it.
/// The memory itself is the caller's to give back, by destroying `build_ut`, as it always was.
pub fn build_child_space(
    own_ut: u64,
    build_ut: u64,
    elf: &elf::Elf,
    endow: &ChildEndowment,
) -> Result<(Child, u64), ()> {
    // The map's image band, checked before anything is retyped, the same check `kernel::user::load`
    // makes. This loader has no channel for the reason (every refusal is `Err(())`, see BUGS above),
    // so a too-large image still reads as "could not build"; what the check buys is that it fails
    // here, cleanly, rather than half-built at the first stack page it would have collided with.
    let (mut lo, mut hi) = (u64::MAX, 0);
    for seg in elf.segments() {
        let (start, end) = seg.page_range(PAGE);
        lo = lo.min(start);
        hi = hi.max(end);
    }
    if address_space_map::check_image(lo, hi).is_err()
        || endow.stack_pages > address_space_map::MAX_STACK_PAGES
    {
        return Err(());
    }

    let aspace = retype_obj_from(build_ut, abi::objtype::ADDRESS_SPACE)?;
    match lay_out_child(own_ut, build_ut, elf, endow, aspace) {
        Ok(tcb) => Ok((
            Child {
                tcb,
                retention: endow.retention,
            },
            aspace,
        )),
        Err(()) => {
            cap_delete(aspace);
            Err(())
        }
    }
}

/// [`build_child_space`] after the address space exists: the segments, the stack, the blobs, the
/// maps, then the TCB and its endowment. Returns the TCB's slot. On `Err` it holds nothing it made;
/// the address space is its caller's to delete.
fn lay_out_child(
    own_ut: u64,
    build_ut: u64,
    elf: &elf::Elf,
    endow: &ChildEndowment,
    aspace: u64,
) -> Result<u64, ()> {
    for seg in elf.segments() {
        let mode = if seg.is_executable() {
            abi::address_space::MAP_CODE
        } else if seg.is_writable() {
            abi::address_space::MAP_RW
        } else {
            abi::address_space::MAP_RO
        };
        let (start, end) = seg.page_range(PAGE);
        let mut va = start;
        while va < end {
            let file_lo = seg.vaddr;
            let file_hi = seg.vaddr + seg.data.len() as u64;
            let lo = va.max(file_lo);
            let hi = (va + PAGE).min(file_hi);
            let src = if lo < hi {
                Some((
                    (lo - va) as usize,
                    &seg.data[(lo - file_lo) as usize..(hi - file_lo) as usize],
                ))
            } else {
                None
            };
            fill_and_map(own_ut, build_ut, aspace, va, src, mode)?;
            va += PAGE;
        }
    }

    for k in 0..endow.stack_pages {
        let stack_frame = retype_page_frame_from(build_ut)?;
        let va = CHILD_STACK_VA - k * PAGE;
        // SAFETY: `invoke` traps to the kernel, which validates the capability and the method
        // before acting (user_mode_runtime's contract). A caller cannot break an invariant by passing a
        // bad slot or method; it gets an error back.
        if unsafe {
            invoke(
                aspace,
                abi::address_space::MAP_INTO,
                va,
                stack_frame,
                abi::address_space::MAP_RW,
            )
        } != 0
        {
            cap_delete(stack_frame);
            return Err(());
        }
        cap_delete(stack_frame);
    }

    // The timebase page, from milestone 161 (the kernel port) and its `cntfrq` follow-up;
    // riscv64 joined on 2026-09-21. This is
    // the tree's only
    // userspace ELF loader, so it is the one place that reaches every child any program here
    // builds (`root_supervisor`, `spawner`, `hello`'s roles, `system_initializer`, ...) without
    // every individual caller needing to know about it.
    //
    // **The child inherits our rate, and this needs no capability from the kernel.** The first
    // version of this mapped a freshly retyped, *zeroed* page, on the reasoning that nothing hands
    // this crate a capability naming the kernel's physical frame, so it could not forward the real
    // number; a child then read "unknown" and `user_mode_runtime::cntfrq` substituted 1 GHz. The
    // premise was true and the conclusion did not follow. The real frame is not what a child needs:
    // the *number* is, and this process already has it, mapped read-only at the same VA by whoever
    // built us. So we read it (`cntfrq_checked`, no syscall) and write a fresh page for the child out
    // of `build_ut`, the child's own budget, exactly the way `fill_and_map` already fills every
    // other blob we hand down.
    //
    // **A parent that does not know the rate writes a zeroed page**, deliberately, and the child
    // then refuses rather than inheriting a number nobody measured (calef, 2026-09-21: only
    // accurate numbers, never hardcoded ones). Truth propagates down the supervision tree and so
    // does its absence, which is the honest shape: a subtree cannot know more than its root did.
    #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
    {
        let page = user_mode_runtime::cntfrq_checked().map(counter_frequency_contract::build_page);
        fill_and_map(
            own_ut,
            build_ut,
            aspace,
            counter_frequency_contract::PAGE_VA,
            page.as_ref().map(|bytes| (0usize, &bytes[..])),
            abi::address_space::MAP_RO,
        )?;
    }

    // The blobs: fresh read-only pages carrying bytes we chose. Read-only because a program image
    // is data the child reads, and a child that could rewrite the image it was handed could hand a
    // different one on.
    for &(base, bytes) in endow.blobs {
        let mut off = 0usize;
        while off < bytes.len() {
            let n = core::cmp::min(PAGE as usize, bytes.len() - off);
            fill_and_map(
                own_ut,
                build_ut,
                aspace,
                base + off as u64,
                Some((0, &bytes[off..off + n])),
                abi::address_space::MAP_RO,
            )?;
            off += n;
        }
    }

    for &(va, our_slot, mode) in endow.maps {
        // SAFETY: as above: the kernel validates the capability and the method.
        if unsafe { invoke(aspace, abi::address_space::MAP_INTO, va, our_slot, mode) } != 0 {
            return Err(());
        }
    }

    let tcb = retype_obj_from(build_ut, abi::objtype::THREAD_CONTROL_BLOCK)?;
    if endow_child(tcb, endow, aspace).is_err() {
        cap_delete(tcb);
        return Err(());
    }
    Ok(tcb)
}

/// Insert the endowment's capabilities into the embryo `tcb`: [`ChildEndowment::caps`] first-free,
/// then the placed ones (one of which may be the child's own address space, [`CHILDS_OWN_SPACE`]),
/// then the fault endpoint in its reserved slot.
fn endow_child(tcb: u64, endow: &ChildEndowment, aspace: u64) -> Result<(), ()> {
    for &(our_slot, rights) in endow.caps {
        // SAFETY: as above: the kernel validates the capability and the method.
        if unsafe {
            invoke(
                tcb,
                abi::thread_control_block::CAP_INSERT,
                our_slot,
                rights,
                0,
            )
        } < 0
        {
            return Err(());
        }
    }
    for &(child_slot, our_slot, rights) in endow.placed {
        let source = if our_slot == CHILDS_OWN_SPACE {
            // `WRITE` to shape it, or `READ` alone for the futex waits a `std` program's locks make
            // on it (milestone 812); never `GRANT`.
            if rights != abi::rights::WRITE && rights != abi::rights::READ {
                return Err(());
            }
            aspace
        } else if our_slot == CHILDS_OWN_THREAD {
            if rights != abi::rights::WRITE {
                return Err(());
            }
            tcb
        } else if our_slot == CHILDS_OWN_PROCESS {
            continue; // placed once the process exists, by `configure_child_as_process`
        } else {
            our_slot
        };
        // `target = n` lands the capability in slot `n - 1`; 0 would mean "first free", which is the
        // behaviour this call exists to avoid.
        // SAFETY: as above: the kernel validates the capability and the method.
        if unsafe {
            invoke(
                tcb,
                abi::thread_control_block::CAP_INSERT,
                source,
                rights,
                child_slot + 1,
            )
        } < 0
        {
            return Err(());
        }
    }
    if let Some(fault) = endow.fault {
        // The spawn-slot convention: target slot `n + 1` means "slot n", so the supervision endpoint
        // lands in the reserved last slot rather than wherever first-free fell.
        // SAFETY: as above: the kernel validates the capability and the method.
        if unsafe {
            invoke(
                tcb,
                abi::thread_control_block::CAP_INSERT,
                fault,
                abi::rights::READ,
                abi::fault::FAULT_EP_SLOT + 1,
            )
        } < 0
        {
            return Err(());
        }
    }
    Ok(())
}

/// **[`configure_child`] for a child that is a process** (milestone 812): the process is made in
/// `build_ut` around `aspace` (which that consumes, as `CONFIGURE` would have), its capability goes to
/// the child at `slot` with `rights` (exactly `BIND`), and the child joins it. The builder keeps no
/// capability to the process.
fn configure_child_as_process(
    build_ut: u64,
    tcb: u64,
    aspace: u64,
    entry: u64,
    slot: u64,
    rights: u64,
) -> Result<(), ()> {
    if rights != abi::rights::BIND {
        return Err(());
    }
    // SAFETY: as above: the kernel validates each capability and method.
    let process = unsafe {
        invoke(
            build_ut,
            abi::memory_region::RETYPE_OBJ,
            abi::objtype::PROCESS,
            aspace,
            0,
        )
    };
    if process < 0 {
        return Err(());
    }
    let process = process as u64;
    // SAFETY: as above.
    let joined = unsafe {
        invoke(
            tcb,
            abi::thread_control_block::CAP_INSERT,
            process,
            rights,
            slot + 1,
        ) >= 0
            && invoke(
                tcb,
                abi::thread_control_block::CONFIGURE,
                entry,
                CHILD_STACK_VA + PAGE,
                process,
            ) >= 0
    };
    cap_delete(process);
    if joined { Ok(()) } else { Err(()) }
}

/// Bind the address space and set the entry point: the last step before [`start_child`]. The `aspace`
/// capability is **consumed** by the kernel here, so this is the moment after which the builder can
/// no longer shape the child's memory.
pub fn configure_child(tcb: u64, aspace: u64, entry: u64) -> Result<(), ()> {
    // SAFETY: as above: the kernel validates the capability and the method.
    if unsafe {
        invoke(
            tcb,
            abi::thread_control_block::CONFIGURE,
            entry,
            CHILD_STACK_VA + PAGE,
            aspace,
        )
    } < 0
    {
        // A negative answer is a refusal. Since milestone 812 (`std::thread::spawn` runs real
        // threads in one address space) a success answers the child's current-CPU page address
        // rather than zero.
        return Err(());
    }
    Ok(())
}

/// Retype one page out of `build_ut`, fill it (zeroed, then `src` bytes at an offset) through our own
/// scratch window, and map it into `aspace` at `va`. The one place a page's contents are written, so
/// segments and blobs cannot drift apart.
fn fill_and_map(
    own_ut: u64,
    build_ut: u64,
    aspace: u64,
    va: u64,
    src: Option<(usize, &[u8])>,
    mode: u64,
) -> Result<(), ()> {
    let frame = retype_page_frame_from(build_ut)?;
    let Ok(scratch) = map_scratch(frame, true, own_ut) else {
        cap_delete(frame);
        return Err(());
    };
    // SAFETY: `scratch` is a page we just mapped read/write in our own address space.
    let dst = unsafe { core::slice::from_raw_parts_mut(scratch as *mut u8, PAGE as usize) };
    dst.fill(0);
    if let Some((at, bytes)) = src {
        dst[at..at + bytes.len()].copy_from_slice(bytes);
    }
    // SAFETY: as above: the kernel validates the capability and the method.
    let mapped = unsafe { invoke(aspace, abi::address_space::MAP_INTO, va, frame, mode) } == 0;
    cap_delete(frame);
    // The page is written and in the child (or the build failed): either way this builder is done
    // with its own window onto it. Given up here, one page at a time, rather than after the child
    // is built, so the window is never wider than the page being filled.
    give_up_own_page(scratch);
    if mapped { Ok(()) } else { Err(()) }
}

/// Retype one page of `ut` into a kernel object of `objtype` (`abi::objtype`), returning the
/// capability slot it landed in.
pub fn retype_obj_from(ut: u64, objtype: u64) -> Result<u64, ()> {
    // SAFETY: as above: the kernel validates the capability and the method.
    let r = unsafe { invoke(ut, abi::memory_region::RETYPE_OBJ, objtype, 0, 0) };
    if r < 0 { Err(()) } else { Ok(r as u64) }
}

/// Retype one page of `ut` into a plain `PageFrame` capability, unmapped, returning the slot it
/// landed in.
pub fn retype_page_frame_from(ut: u64) -> Result<u64, ()> {
    // SAFETY: as above: the kernel validates the capability and the method.
    let r = unsafe { invoke(ut, abi::memory_region::RETYPE, 0, 0, 0) };
    if r < 0 { Err(()) } else { Ok(r as u64) }
}

/// Carve `pages` off `ut` into a new child untyped we hold in full: the region a single `DESTROY`
/// reclaims. `Err` carries the negated error code, which is how the dropped-authority proof reports
/// *why* a retype from a deleted budget failed.
pub fn memory_region_split(ut: u64, pages: u64) -> Result<u64, i64> {
    // SAFETY: as above: the kernel validates the capability and the method.
    let r = unsafe { invoke(ut, abi::memory_region::SPLIT, pages, 0, 0) };
    if r < 0 { Err(r) } else { Ok(r as u64) }
}

/// §16 object revocation: reclaim a region and every object retyped from it, by its **owner**. The
/// stronger of the two reaps, because `WRITE` on a region is also what builds a process out of it,
/// and the only one that can tear down a *live* thread (§16's amendment arms the kill). A supervisor
/// collecting a dead child wants `user_mode_runtime::reap` instead (§32).
pub fn memory_region_destroy(ut: u64) -> bool {
    // SAFETY: as above: the kernel validates the capability and the method.
    unsafe { invoke(ut, abi::memory_region::DESTROY, 0, 0, 0) == 0 }
}

/// **Make `child` runnable and then keep or drop its capability as its endowment declared**, with
/// `a0`, `a1`, `a2` in its entry registers. `false` if the thread was not fully configured (no bound
/// address space or no entry point) and the kernel refused.
///
/// This is the only way to `START` a child in this tree, and that is the mechanism rather than a
/// convention (DECISIONS §142). A bare `START` helper taking a slot number would let a spawn path
/// run a child without ever saying what it keeps over it, which is precisely the state §142 wrote
/// the [`Retention`] field to make unrepresentable. [`Child`] carries the declaration from
/// [`build_child`] to here, so no call site has to remember it and none can restate it differently.
///
/// **The disposal does not depend on whether `START` succeeded.** Retention is a statement about
/// what the spawner keeps, not about what worked: under [`Retention::Nothing`] the capability goes
/// back either way, which is what every call site did by hand before this existed (`START`, then an
/// unconditional `cap_delete`, then a check of the result).
///
/// # Examples
///
/// ```no_run
/// # use supervision_protocol::{ChildEndowment, Retention, build_child, start_child};
/// # fn demo(own: u64, per_child: u64, elf: &elf::Elf) -> Result<(), ()> {
/// let endow = ChildEndowment { fault: Some(7), ..ChildEndowment::new(Retention::Nothing) };
/// let child = build_child(own, per_child, elf, &endow)?;
/// // The capability is gone after this line, because the endowment said so.
/// start_child(child, 0, 0, 0);
/// # Ok(())
/// # }
/// ```
///
/// # BUGS
///
/// **The retaining branch has no caller and therefore no test.** Every spawn site in the tree
/// declares [`Retention::Nothing`] (DECISIONS §142 chose R4 declaring R0), so the arm that keeps
/// the capability is reached by nothing that runs, and this crate is excluded from `script/test`'s
/// host pass anyway for its unconditional `user_mode_runtime` dependency. The arm is three lines and does
/// nothing, which is the only reason that is tolerable; the first site that declares
/// [`Retention::ThreadControlBlock`] is exercising it for the first time and should say so.
pub fn start_child(child: Child, a0: u64, a1: u64, a2: u64) -> bool {
    // SAFETY: as above: the kernel validates the capability and the method.
    let started = unsafe { invoke(child.tcb, abi::thread_control_block::START, a0, a1, a2) == 0 };
    match child.retention {
        Retention::Nothing => cap_delete(child.tcb),
        // The reason is the caller's, recorded at the call site; nothing here reads it.
        Retention::ThreadControlBlock { reason: _ } => {}
    }
    started
}

/// Trap. A half-built system is not worth limping along, and a fault is legible: the kernel prints
/// the pc and the process dies where the mistake was.
///
/// The instruction moved to [`user_mode_runtime::trap`] in milestone 130; this stays because callers here
/// mean something more specific than "trap" (a supervision-protocol step failed, and limping on
/// would build half a system), and that reason is worth a name. What it no longer does is spell
/// the asm out: this body and `hot_swap_contract::fail`'s were byte-identical copies of each other and of
/// forty-six other sites.
pub fn fail() -> ! {
    user_mode_runtime::trap()
}
