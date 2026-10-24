/// `invoke(cap, MAP, va, _, _)` -> 0. Retype one page out of the untyped and map it, writable,
/// at `va` in the caller's own address space. The page and any page tables it needs both come
/// from the untyped; the kernel allocates nothing. Returns `OutOfMemory` when the untyped is
/// exhausted (the *process* is out of budget, not the kernel).
pub const MAP: u64 = 0;

/// `invoke(cap, RETYPE, pages, _, _)` -> slot. Retype `pages` contiguous pages out of the untyped
/// into one **`PageFrame` capability** naming the run,
/// per §102 (a Frame names a run of pages), and return the slot it landed in.
/// `pages == 0` means one page, which is what every caller passed before the count existed
/// (calef's ruling of 2026-09-26). Nothing is mapped: the caller decides where to map it, and
/// may delegate it first. This is the split that makes a page a first-class, delegatable object
/// rather than something mapped in one shot. `OutOfMemory` when the run does not fit, and then
/// the region's budget is untouched; also when the caller's capability table is full, and then
/// the run is spent, exactly as a single page always was.
pub const RETYPE: u64 = 1;

/// `invoke(cap, RETYPE_OBJ, objtype, space, floor)` -> slot: retype one page of the untyped into a
/// **kernel object** living in it (milestone 19a (run a real workload)), with a full-rights
/// capability at the returned slot. One object per page; see design/init-and-granular-spawn.md.
///
/// `objtype` names what to make (see [`objtype`](super::objtype)). A region that has produced a
/// kernel object is **pinned**: [`DESTROY`] reclaims it once the objects are torn down. `space` is
/// read only for [`objtype::PROCESS`](super::objtype::PROCESS), the slot of the space it will hold.
/// `floor` is the lowest slot the new capability may land in, `0` for the first free one; a `std`
/// program passes `std_runtime_contract::SLOTS`. `BadMethod` for an unknown objtype; `OutOfMemory`
/// when the untyped, the object registry or the capability table is exhausted.
pub const RETYPE_OBJ: u64 = 2;

/// `invoke(cap, SPLIT, pages, _, _)` -> slot. Carve `pages` off this untyped's unspent budget into
/// a **new child untyped**, and return the slot holding a full-rights capability to it. seL4's
/// untyped-retype-into-untyped: a spawner splits a child its own region so it can be reclaimed
/// independently. This untyped is then marked as having children and can no longer be `DESTROY`ed
/// (its pages are committed; the child frees them at its own `DESTROY`). `OutOfMemory` when the
/// budget or region table is exhausted or the capability table is full; `NotPermitted` without
/// `WRITE`.
pub const SPLIT: u64 = 3;

/// `invoke(cap, DESTROY, _, _, _)` -> 0. **Reclaim this region and every object retyped from
/// it** (object revocation, the region-owner's half). The objects are torn down and their pages
/// returned; every capability to them goes stale on next use (generational names, no derivation
/// tree to walk). `NotPermitted` while a live thread still occupies the region, or if it has
/// been `SPLIT` into children (destroy the children first), or without `WRITE`.
pub const DESTROY: u64 = 4;

/// `invoke(cap, USAGE, record, _, _)` -> pages: **how much of this region has been spent, and on
/// what**, for a `record` from [`usage`](super::usage) (milestone 126 (the `procps` package), §225
/// (`free` sees the machine and your share) part 1). Needs `ENUMERATE` alone, the rule §114
/// (`ENUMERATE` extends to the address-space object) set for `pmap`. `BadMethod` for an unknown
/// record, checked first; [`crate::Error::Gone`] for a region already reclaimed. Name and number
/// ratified 2026-09-27 (calef, #1360's table).
///
/// # BUGS
/// - **A subtree record walks the whole region table under the region lock, interrupts masked**:
///   up to 62 µs a call, repeatable by any `ENUMERATE` holder; accepted (notes/abi/method-contracts.md).
pub const USAGE: u64 = 5;
