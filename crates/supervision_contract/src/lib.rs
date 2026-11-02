#![no_std]
// `ChildEndowment::new(retention)` is the one spelling of an empty endowment, and it takes the
// retention declaration so nothing supplies one silently (§142 (what a spawner retains over a child
// after `START`)).
#![allow(clippy::new_without_default)]
//! **The supervision tree: the shared half.** The protocol words `root_supervisor`, `spawner`,
//! `sub_server_supervisor` and `first_attempt_crasher` speak. A [`ChildEndowment`] states a child's
//! complete authority in both directions (§142). See notes/supervision.md.
//!
//! The loader these words drive is `process_builder`, a separate crate, so this one carries no
//! `user_mode_runtime` edge (DECISIONS §235 (the OS is built and updated from packages, and its source divides by purpose) P3: a contract never links the runtime). Its examples
//! run in `script/test`'s host pass, which the loader's dependency kept them out of until the
//! split.
//!
//! Name: ratified 2026-10-28 (calef, relayed by the maintainer), replacing
//! `supervision_protocol`. Refused keeping `_protocol` under the standard's exception
//! (`network_time_protocol`): supervision is nife's own protocol, not an external standard's.
//! The split that lifted `process_builder` out of this crate landed 2026-10-29 (UTC), in lane
//! `milestone/607-item-3-contracts-drop-the-runtime`; `process_builder`'s own name is provisional.

// ===========================================================================================
// The report protocol. Every process in the tree holds a WRITE view of one report endpoint and
// says what happened on it; the kernel test is the receiver. Mirrored in kernel/src/user.rs
// (`authority_tests`), the same way the net client's selectors are.
// ===========================================================================================

/// init dropped its construction authority. `w1` = 1 if using the dropped budget then failed,
/// `w2` = the negated error code (1 = `NoSuchSlot`: the slot is empty, there is nothing to name).
///
/// Name: ratified 2026-10-18 (calef: "Progenitors numbers are ratified."), replacing `REPORT_INIT_DROPPED`.
pub const REPORT_CONSTRUCTION_AUTHORITY_DROPPED: u64 = 1;
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
// The construction protocol: the supervisor asks, the spawner builds. The supervisor holds no
// memory, so it cannot build anything; the spawner holds a budget and one program image, so it
// cannot build anything else. Reaping is not asked here: it is on the supervision endpoint the
// supervisor already holds (§32 (a supervisor may collect a corpse without being able to build
// one)).

/// `send(req, REQ_BUILD, attempt, label)` -> `(REP_BUILT, 0, 0)` or `(REP_FAILED, 0, 0)`. The
/// spawner starts the child with `attempt` in its second argument register and stamps `label` on
/// its supervision capability (milestone 105), so the kernel hands `label` back with its death
/// message. `label` must be nonzero, because `rendezvous::BADGE` refuses zero; the child is never
/// told it.
pub const REQ_BUILD: u64 = 1;

/// **The labels `sub_server_supervisor` gives its two children, and the attempt each starts at**
/// (milestone 105, DECISIONS §148 (resolves by asking the kernel) as amended 2026-10-04; name
/// provisional). The first starts at attempt 0, which `first_attempt_crasher` crashes on; the
/// second at attempt 10, which it finishes, so a supervisor that confused them would restart the
/// wrong one. Mirrored in the kernel's `authority_tests`.
///
/// Name: provisional, milestone 105 (the two forks)'s lane, 2026-10-05 (UTC). Plural because it holds both children's labels and their
/// first attempts; `CHILD_LABELS` was considered and is less specific about whose children.
pub const SUB_SERVER_LABELS: [(u64, u64); 2] = [(0x1abe_0a01, 0), (0x1abe_0b02, 10)];

/// Reply code: the build succeeded.
pub const REP_BUILT: u64 = 1;
/// Reply code: the build failed (out of budget, a bad image, or a loader refusal).
pub const REP_FAILED: u64 = 3;

/// Where a child's highest stack page sits, one address for every process this system builds, so
/// `process_builder::configure_child` computes the entry `sp` without being told.
///
/// The map's `STACK_TOP_PAGE` (milestone 206 (a program image has under 896 KiB), DECISIONS §171
/// (where a program image starts) option D), which `kernel::user::USER_STACK_VA` also names, so
/// neither can move without the other.
pub const CHILD_STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;

/// The stack a child gets when its builder does not say otherwise ([`ChildEndowment::new`]): four
/// pages, enough for the supervision tree's programs. A child at the interactive prompt gets three
/// times this (`system_initializer::CHILD_STACK_PAGES`), which is why it is a field each caller
/// states. See notes/progenitor-and-loading.md and notes/pipes/the-boot.md.
pub const CHILD_STACK_PAGES: u64 = 4;

/// **Everything a child is born holding.** The same idea as the kernel's `Spawn`: read one of these
/// and you know the complete authority of the thing about to run.
///
/// # Examples
///
/// Every field is public and [`ChildEndowment::new`] is the nothing-endowment to build from:
///
/// ```
/// use supervision_contract::{CHILD_STACK_PAGES, ChildEndowment, Retention};
///
/// // A sub-server that gets exactly one endpoint, is born supervised, and holds nothing else.
/// // `..ChildEndowment::new(Retention::Nothing)` is the intended shape: what is not listed is not
/// // granted, and what the spawner keeps is stated rather than inherited.
/// let endow = ChildEndowment {
///     capabilities: &[(4, 0b11)], // our slot 4, read/write, landing in the child's slot 0
///     fault: Some(7), // our slot 7 holds its supervision endpoint
///     ..ChildEndowment::new(Retention::Nothing)
/// };
///
/// assert_eq!(endow.capabilities.len(), 1);
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
    ///
    /// Name: provisional, minted 2026-10-27 (UTC) by lane capability-sweep-1. Spelled out from
    /// `capabilities` under the capability ruling of 2026-10-06 (UTC).
    pub capabilities: &'a [(u64, u64)],
    /// Capabilities to insert at a slot the caller **names**, `(child_slot, our_slot, rights)`,
    /// after [`capabilities`](ChildEndowment::capabilities). For a declared diagnostic stream (DECISIONS §67 (a
    /// program's second stream is a declaration, not a number)), whose slot must not move with how
    /// many low slots the command line granted. It cannot collide with
    /// [`fault`](ChildEndowment::fault), which lands in the table's last slot.
    ///
    /// `our_slot` may be [`CHILDS_OWN_SPACE`], [`CHILDS_OWN_PROCESS`] or [`CHILDS_OWN_THREAD`].
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
/// (§255 (each socket is its own capability); name provisional), for a server that must `UNMAP`
/// one client's page before the next takes its place: the network stack. The rights must be
/// exactly `WRITE`, so no child is handed `GRANT` on its own memory. Inserted before `CONFIGURE`,
/// and it names the space while the child runs (§249).
///
/// A sentinel rather than a field, and that is an exception: a typed field cost the progenitor's
/// stack 352 bytes. See notes/progenitor-and-loading.md.
pub const CHILDS_OWN_SPACE: u64 = u64::MAX;

/// **[`ChildEndowment::placed`]'s marker for the child's own process** (milestone 812
/// (`std::thread::spawn` runs real threads in one address space)): place it and the child is built
/// as a process, so it can add threads to itself. `process_builder::build_child` creates the process around the
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
    /// stack, retaining whatever `retention` says. Use it as
    /// `..ChildEndowment::new(Retention::Nothing)` at the end of a struct literal.
    ///
    /// Retention is an argument, not a default (DECISIONS §142): forgetting any other field leaves
    /// a child unable to do something, but forgetting retention would leave a spawner silently
    /// keeping authority over a running child.
    pub const fn new(retention: Retention) -> Self {
        Self {
            capabilities: &[],
            placed: &[],
            maps: &[],
            blobs: &[],
            fault: None,
            stack_pages: CHILD_STACK_PAGES,
            retention,
        }
    }
}

/// **What the spawner keeps over the child after `START`** (DECISIONS §142,
/// design/decisions/0142-spawn-retention.md). A `ThreadControlBlock` kept after `START` authorizes
/// only `SUSPEND` and `RESUME` (DECISIONS §300 (the paused state, a TCB verb, silent)), so the
/// variants that keep one carry a reason. The child's region is not declared here (§303).
///
/// # Examples
///
/// ```
/// use supervision_contract::{ChildEndowment, Retention};
///
/// // The ordinary case, and it says so out loud rather than by omission.
/// let ordinary = ChildEndowment { fault: Some(7), ..ChildEndowment::new(Retention::Nothing) };
/// assert!(matches!(ordinary.retention, Retention::Nothing));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retention {
    /// The spawner keeps nothing. `process_builder::start_child` deletes the `ThreadControlBlock` capability, so
    /// after `START` the only authority over this child is the region it was built from
    /// (`MemoryRegion::DESTROY`) and the supervision endpoint it was born under (`REAP`).
    Nothing,
    /// The spawner keeps the child's `ThreadControlBlock` capability past `START`, for the reason
    /// given. The reason is part of the variant, milestone 50 (pipes and redirection)'s
    /// `InputDeclaration::Required` shape, so a spawn path cannot keep it without saying why.
    ///
    /// # BUGS
    ///
    /// - The reason is not checked and cannot be: `reason: ""` compiles. A caller can still say
    ///   nothing, but not invisibly.
    ThreadControlBlock {
        /// Why this spawner keeps the capability, in the words a reader of the call site needs.
        reason: &'static str,
    },
    /// The spawner hands the child's `ThreadControlBlock` capability to whoever asked for the
    /// child, and keeps no copy (DECISIONS §309 (a supervised job's `ThreadControlBlock` goes to
    /// the shell that asked for it)). `process_builder::start_child` leaves the slot alone; the caller sends it on
    /// and deletes it. The reason is required for [`ThreadControlBlock`](Retention::ThreadControlBlock)'s.
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by lane/48-ctrl-z.
    Requester {
        /// Why the requester holds the capability, in the words a reader of the call site needs.
        reason: &'static str,
    },
}

/// **A built child, and what its builder declared it would keep.** What `process_builder::build_child`
/// returns and `process_builder::start_child` consumes.
///
/// The declaration travels with the handle because the deletion happens at `START`, a separate step
/// from the build (`process_builder::build_child_space`'s seam), and §142 exists so it is not left to
/// whoever
/// remembers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Child {
    /// The child's `ThreadControlBlock` capability slot, in the builder's own table.
    pub tcb: u64,
    /// What the builder declared it keeps once the child runs.
    pub retention: Retention,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The nothing-endowment states exactly that: no capabilities, no placements, no mappings, no
    /// blobs, no supervision, the default stack, and the retention the caller declared. Every
    /// struct literal spreads it (`..ChildEndowment::new(Retention::Nothing)`), so a field that
    /// defaulted to something would hand a child authority nobody wrote down.
    #[test]
    fn the_nothing_endowment_grants_nothing_and_keeps_what_it_says() {
        let endow = ChildEndowment::new(Retention::Nothing);
        assert!(endow.capabilities.is_empty());
        assert!(endow.placed.is_empty());
        assert!(endow.maps.is_empty());
        assert!(endow.blobs.is_empty());
        assert_eq!(endow.fault, None);
        assert_eq!(endow.stack_pages, CHILD_STACK_PAGES);
        assert_eq!(endow.retention, Retention::Nothing);
    }
}
