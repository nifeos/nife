//! **The reboot object: the authority to restart the machine, as a capability** (milestone 805
//! (`reboot` at the prompt), DECISIONS §251 (restarting the machine is a kernel object the
//! progenitor hands out)).
//!
//! PSCI and SBI calls are made from EL1 and S-mode, so on two of three architectures a user program
//! cannot reset the machine at all, and the kernel has to do it on somebody's behalf. Whose behalf
//! is the whole question, and §251's answer is: whoever holds this object. It has no payload, names
//! no device and has one method, [`abi::reboot::REBOOT`]. The kernel mints exactly one at boot and
//! grants it to the progenitor, which endows it only to a program whose manifest declares
//! `grant_plan::Manifest::reboot`.
//!
//! # What the method does, and what it deliberately does not
//!
//! - It calls [`prepare_reset_route`] (a no-op everywhere but a JH7110) and then
//!   `arch::reboot`. On success neither returns.
//! - If every route this architecture has was refused, `arch::reboot` prints the firmware's raw
//!   answer and returns the portable reason as one of four `abi::Error`s (`NoResetMechanism`,
//!   `ResetNotSupported`, `ResetDenied`, `ResetDidNotHappen`), and the method answers it: calef's ruling on §251's amendment, item 3 (2026-10-06 UTC).
//! - **It syncs nothing.** The kernel knows no filesystem, and a microkernel that did would be the
//!   bug. The `reboot` program sends `filesystem_contract::fs::SYNC` first. A holder that skips it
//!   loses whatever the device had not flushed: a foot gun, recorded in §251 and in the program's
//!   `BUGS`, and the mitigation is that exactly one program is endowed.
//! - **It does not quiesce devices.** A DMA transfer in flight is cut off with everything else.
//!
//! # BUGS
//!
//! - **A firmware that accepts the call and hangs is indistinguishable from a slow reset**, from
//!   inside the machine. radon did exactly this on 2026-09-04; milestone 592 (radon's cold reboot
//!   dies in OpenSBI's PMIC write) holds the fix, which has not run on the board.
//! - **The caller gets the reason, not the number.** Four portable reasons cover every firmware;
//!   the raw code (PSCI's or SBI's own) is on the kernel's line just above.
//!
//! Name: provisional (milestone 805). The module, the object, its method and [`MARKER`] are
//! calef's to name; §251 calls them "the reboot object" and `REBOOT` for want of anything better.

use crate::{arch, println};

/// **The prefix on every line the reboot method prints** (milestone 805), so the gate that types
/// `reboot` and a person reading a bench capture can find what the kernel said about the reset in
/// one grep. Distinct from the soak's `soak-test-reboot:` on purpose: a capture that has both is a
/// capture of two different callers.
///
/// Name: provisional, milestone 805's lane, 2026-10-06 (UTC).
pub const MARKER: &str = "reboot:";

/// The AXP15060 direct-write route, on the machines that have the plan for it. A no-op with no
/// output everywhere else, including every machine CI boots. Returns only to say the attempt
/// failed and the firmware route is next; a write the PMIC honors never comes back.
// The JH7110's polled I2C master, used only by the direct AXP15060 route below, so riscv64-only
// too. See kernel/src/reboot/designware_i2c.rs.
#[cfg(target_arch = "riscv64")]
mod designware_i2c;

mod pmic;
mod reset;

#[cfg(all(target_arch = "riscv64", feature = "reboot_soak_test"))]
pub(crate) use pmic::pmic_report;
use pmic::*;
pub use reset::*;
