//! The reboot object: the authority to restart the machine, as a capability, §251 (restarting
//! the machine is a kernel object the progenitor hands out).
//!
//! Its one method, [`abi::reboot::REBOOT`], runs `arch::reboot` and nothing board-specific, and
//! returns only if the firmware refused, with one of four portable `abi::Error`s. It syncs no
//! filesystem (`reboot` sends `SYNC` first), quiesces no device and writes no PMIC: a board whose
//! power controller is the reset is asked through its PMIC route, a process `reboot` calls before
//! this object (§251, "A board's PMIC route runs in userspace"). See notes/board-reboot.md.

//! # BUGS
//! - A firmware that accepts the call and hangs is indistinguishable from a slow reset.
//! - The caller gets the portable reason, not the firmware's number, which only the console has.
//!
//! Name: provisional (milestone 805). The module, the object, its method and [`MARKER`] are
//! calef's to name; §251 calls them "the reboot object" and `REBOOT` for want of anything better.

use crate::{arch, println};

/// The prefix on every line the reboot method prints, distinct from the soak's
/// `soak-test-reboot:` so a capture with both shows two callers.
///
/// Name: provisional, milestone 805's lane, 2026-10-06 (UTC).
pub const MARKER: &str = "reboot:";

mod reset;

pub(crate) use reset::*;
