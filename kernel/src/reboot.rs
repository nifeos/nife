//! The reboot object: the authority to restart the machine, as a capability, §251 (restarting
//! the machine is a kernel object the progenitor hands out).
//!
//! Its one method, [`abi::reboot::REBOOT`], runs [`prepare_reset_route`] then `arch::reboot`, and
//! returns only if every route was refused, with one of four portable `abi::Error`s. It syncs no
//! filesystem (`reboot` sends `SYNC` first) and quiesces no device. See notes/board-reboot.md.
//!
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

// The JH7110's polled I2C master, used only by the direct AXP15060 route in `pmic`.
#[cfg(target_arch = "riscv64")]
mod designware_i2c;

mod pmic;
mod reset;

#[cfg(all(target_arch = "riscv64", feature = "reboot_soak_test"))]
pub(crate) use pmic::pmic_report;
use pmic::*;
pub use reset::*;
