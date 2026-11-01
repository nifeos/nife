/// `invoke(reply_capability, REPLY, r0, r1, _)` -> 0. Deliver `{r0, r1}` to the caller, wake it, and
/// consume this capability (a second use is [`crate::Error::NoSuchSlot`]).
pub const REPLY: u64 = 0;

/// `invoke(reply, REPLY_CAPABILITY, r0, r1, capability_slot)` -> 0, or 1 when the copy did not
/// reach the caller: [`REPLY`], carrying one capability (each socket is its own capability; name ratified
/// 2026-10-07 UTC, number provisional); its `CALL` returns the copy's slot in `x2`.
///
/// - **The carried capability needs `GRANT`** and keeps its rights; there is no rights mask, so a
///   server narrows first. A refused source (`NoSuchSlot`, `NotPermitted`) consumes nothing.
/// - **A full table drops the copy, not the answer**: the caller gets [`super::rendezvous::NO_CAPABILITY`]
///   in `x2` and this call returns 1, as it does for a caller no longer waiting.
/// - Every `CALL` returns `x2`, `NO_CAPABILITY` after a plain [`REPLY`].
pub const REPLY_CAPABILITY: u64 = 1;
