//! Name: ratified 2026-10-25 (#2720).

/// The command or transfer did what was asked.
///
/// Name: ratified 2026-10-25 (#2720).
pub const SUCCESS: u8 = 1;
/// USB transaction error: the device did not answer, or answered garbage.
///
/// Name: ratified 2026-10-25 (#2720).
pub const TRANSACTION_ERROR: u8 = 4;
/// The device stalled the endpoint: it refused the request.
///
/// Name: ratified 2026-10-25 (#2720).
pub const STALL: u8 = 6;
/// The transfer finished with fewer bytes than the buffer: normal for a descriptor read.
///
/// Name: ratified 2026-10-25 (#2720).
pub const SHORT_PACKET: u8 = 13;
/// No slot was free for Enable Slot.
///
/// Name: ratified 2026-10-25 (#2720).
pub const NO_SLOTS: u8 = 9;
