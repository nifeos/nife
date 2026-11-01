/// `invoke(capability, MAP_INTO, va, frame_slot, writable)` -> 0. Map the frame in `frame_slot` into THIS
/// address space at `va`, read-only or read/write. Needs `WRITE` on the address-space capability;
/// the frame capability needs `READ` for a read-only mapping, `WRITE` for a writable one, the
/// `frame::MAP` rule verbatim. Page tables and the revocation record are paid from the space's own
/// backing region; a mapping whose record cannot be afforded is refused (`OutOfMemory`), never left
/// invisible to revocation. A `MAP_RW` of a page the space already maps as code, or a `MAP_CODE` of
/// one it already maps writable, is refused with `NotPermitted`: no page is writable through one
/// mapping and executable through another in one address space (claim 11). The same frame
/// writable in one space and code in another maps.
pub const MAP_INTO: u64 = 0;

/// `MAP_INTO`'s third argument: how to map the frame. Read-only, read/write, or executable
/// code. Code is W^X: mapped RX,
/// never writable, and the kernel makes the I-cache coherent with the loader's data writes.
pub const MAP_RO: u64 = 0;
/// Map read/write.
pub const MAP_RW: u64 = 1;
/// Map executable code: RX, never writable (W^X).
pub const MAP_CODE: u64 = 2;

/// `invoke(capability, LIST, cursor, 0, 0) -> (next_cursor, va, kind)` (`pmap`): list this space's
/// mappings one entry per call, changing none, with `Rendezvous::SURVEY`'s cursors and right.
///
/// - `next_cursor` returns in x0 (a0 on RISC-V), `va` in x1 and `kind` in x2, one of [`MAP_RO`],
///   [`MAP_RW`] or [`MAP_CODE`]. A `DeviceFrame` mapping reads as `MAP_RW`.
/// - Start with `cursor = 0` and feed each `next_cursor` back; `abi::survey::DONE` means finished.
///   A cursor this space's log never minted is [`crate::Error::BadPointer`], and any other negative first word is an [`crate::Error`].
/// - Needs [`rights::ENUMERATE`](super::rights::ENUMERATE), not `WRITE`.
pub const LIST: u64 = 1;

/// `invoke(capability, UNMAP, va, _, _)` -> 0. **This space gives up the one page it maps
/// at `va`, and no other holder is touched.** Needs `WRITE`, the authority [`MAP_INTO`] takes.
///
/// - One page per call; unmapping one page of a run leaves the rest mapped.
/// - A `va` with nothing mapped, misaligned or in the kernel half is [`crate::Error::BadPointer`]
///   and changes nothing. Not idempotent, deliberately. No capability is consumed or changed.
/// - The page is gone from every core's TLB and from the mapping record before this returns.
/// - It reaches a running space: the capability keeps naming a space after a thread runs in it.
pub const UNMAP: u64 = 2;

/// `invoke(capability, WAIT, va, flags, expected)` -> `0` woken, `1` the word did not hold `expected`
/// **Futex wait: park the calling
/// thread on the word at `va` in its own space if it still holds `expected`**, checked and parked
/// as one step against [`WAKE`]. Semantics follow the thread syscalls as built.
///
/// - `flags` is [`super::futex::PRIVATE`] `|` [`super::futex::SIZE_U32`], or else `BadMethod`.
/// - The capability must name the caller's own space (`WrongObject`) with `READ` (`NotPermitted`).
/// - `va` must be 4-aligned, in the user half and mapped readable, else `BadPointer`.
/// - `Gone` if ended without a wake. A `0` may be spurious, so a caller re-checks its word. No
///   timeout yet. *(Name and number provisional.)*
pub const WAIT: u64 = 3;

/// `invoke(capability, WAKE, va, flags, count)` -> the number of threads woken. **Futex wake: release up to `count` threads parked
/// by [`WAIT`] on the word at `va` in the caller's own space**, oldest first. `u64::MAX` wakes all.
/// Refuses what [`WAIT`] refuses, except that the word need not be mapped: a wake reads no memory.
/// *(Name and number provisional.)*
pub const WAKE: u64 = 4;
