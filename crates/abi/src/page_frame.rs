/// `invoke(cap, MAP, va, writable, memory_region_slot)` -> 0. Map this frame at `va` in the
/// caller's own address space. `writable` != 0 maps it read/write (needs `WRITE` on the frame); `0`
/// maps it read-only (needs `READ`). Page tables to reach `va` come from the untyped named by
/// `memory_region_slot`, so the kernel allocates nothing. `BadPointer` for a misaligned or high
/// `va`, `OutOfMemory` when that untyped is exhausted.
///
/// The tables belong to that untyped, so its `DESTROY` cuts them out of this address space,
/// and every page mapped beneath them stops translating, this frame's included. A mapping
/// meant to outlive the untyped needs its tables from a region that lives as long.
pub const MAP: u64 = 0;

/// `invoke(cap, REVOKE, _, _, _)` -> 0. **Un-share this page**: unmap it from every address space
/// and delete every capability to it, the caller's own included (milestone 13 (capability
/// revocation), §13 (untyped reclamation)). Needs `GRANT`: a holder trusted to lend the frame may
/// take it back. It does **not** reclaim the page; a region's `DESTROY` does. See
/// notes/capability-lifecycle.md.
///
/// **On a device capability it means take-back** (milestone 23 (live replacement), §41 (a device is
/// revoked by taking it back)): every `DeviceFrame` capability to the page goes and it is unmapped
/// everywhere **except the caller's own**, so only the asker still reaches the registers. A device
/// capability is minted once, at boot, so a symmetric revoke would strand the device.
pub const REVOKE: u64 = 1;

/// `invoke(cap, SLICE, first, count, _)` -> the slot of a new capability in x0, or a negative
/// [`crate::Error`]. **Derive a capability naming a sub-run of this one** (milestone 599 (a frame
/// per filesystem client channel), calef's ruling of 2026-09-27 on option 4; name and number
/// provisional): `count` pages starting `first` pages in, with this capability's rights, so a
/// pool's holder hands a client exactly its window, as §102 (a Frame names a run of pages) foresaw.
///
/// - Needs `GRANT`: §132 (what `PageFrame::REVOKE` owes an overlapping run) scopes a revoke to the
///   exact run, so a slice escapes its source's revoke, a delegation-class power.
/// - [`crate::Error::BadPointer`] for `count` of 0 or a range past the run's end.
pub const SLICE: u64 = 2;
