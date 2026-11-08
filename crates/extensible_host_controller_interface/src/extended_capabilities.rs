//! **Extended capability ids** (xHCI 1.2 section 7): the ids in the list that starts at
//! [`Capabilities::extended`](crate::Capabilities::extended), for
//! [`find_extended`](crate::registers::find_extended) to look for.
//!
//! Name: ratified 2026-11-08 (UTC), on #3357. It holds capability ids, which are not register
//! offsets, so they sit outside `registers`, where the template reads every plain integer as an
//! offset.

/// Extended capability id: USB Legacy Support, the firmware handoff (xHCI 1.2 section 7.1).
///
/// Name: ratified 2026-11-08 (UTC), on #3357; it replaced `registers::EXTENDED_LEGACY_SUPPORT`,
/// because the template reads a plain integer in `registers` as a register offset and this is a
/// capability id.
pub const LEGACY_SUPPORT_EXTENDED_CAPABILITY_ID: u8 = 1;
