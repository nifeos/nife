/// `invoke(reply_cap, REPLY, r0, r1, _)` -> 0. Deliver `{r0, r1}` to the caller, wake it, and
/// consume this capability (a second use is [`crate::Error::NoSuchSlot`]).
pub const REPLY: u64 = 0;

/// `invoke(reply, REPLY_CAPABILITY, r0, r1, capability_slot)` -> 0, or 1 when the copy did not
/// reach the caller: [`REPLY`], carrying one capability (§255 (each socket is its own capability);
/// name ratified by calef 2026-10-07 UTC, number provisional). A copy of the capability in
/// `capability_slot` is filed in the caller's table, and its `CALL` returns the slot in `x2`.
///
/// - **The carried capability needs `GRANT`** and keeps its rights; there is no rights mask, so a
///   server narrows first. A refused source (`NoSuchSlot`, `NotPermitted`) consumes nothing.
/// - **A full table drops the copy, not the answer**: the caller gets [`super::rendezvous::NO_CAP`]
///   in `x2` and this call returns 1, as it does for a caller no longer waiting.
/// - Every `CALL` returns `x2`, `NO_CAP` after a plain [`REPLY`]. See notes/abi/method-contracts.md.
pub const REPLY_CAPABILITY: u64 = 1;
