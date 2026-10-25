//! Where the JH7110's clock-and-reset windows are, and what its PMIC's bus needs, for
//! `jh7110_clock_and_reset`.
//!
//! The crate this serves began as milestone 220 (this kernel drives no clock or reset controller,
//! and the first real device will need one); the SYS window and the PMIC bus are milestone 592
//! (radon's cold reboot dies in OpenSBI's PMIC write, and the kernel sets that write up), and the
//! AON window is milestone 53 (the board's own peripherals: network and storage on real silicon).
//!
//! Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, after the crate it
//! serves. calef has not named it.

use device_tree_blob::{DeviceTreeBlob, Error, Region};
use jh7110_clock_and_reset::{
    AON_WINDOW, Found, MAX_WINDOWS, PMIC_COMPATIBLES, PmicBus, STG_WINDOW, SYS_WINDOW, Window,
};

/// **Find the STG clock and reset window in `tree`**, where the TRNG's clocks and reset live.
///
/// Never fails to produce an address: a tree that names no controller gets the constant both
/// published trees agree on, with `from_tree: false`. The caller must not store through that
/// answer on a machine the tree does not name as a JH7110.
///
/// # Errors
///
/// Propagates [`Error`] if the blob is malformed.
///
/// Name: recorded (milestone 220), which wrote it as `jh7110_clock_and_reset::discover`. The
/// module path carries the chip and the controller, so the verb alone is left. calef has not
/// ruled on it.
pub fn discover(tree: &DeviceTreeBlob<'_>) -> Result<Found, Error> {
    discover_window(tree, &STG_WINDOW)
}

/// **Find the AON clock and reset window in `tree`**, the domain holding `gmac0`'s bus clocks,
/// transmit clock and resets. [`discover`]'s twin, under its rules.
///
/// # Errors
///
/// Propagates [`Error`] if the blob is malformed.
///
/// Name: provisional (milestone 53's lane, 2026-10-06 UTC), after [`discover_sys`]; moved here
/// from `jh7110_clock_and_reset` by lane crates-stand-alone, 2026-10-25 (UTC).
pub fn discover_aon(tree: &DeviceTreeBlob<'_>) -> Result<Found, Error> {
    discover_window(tree, &AON_WINDOW)
}

/// **Find the SYS clock and reset window in `tree`**, the domain holding I2C5's clock and reset.
/// [`discover`]'s twin, under its rules.
///
/// # Errors
///
/// Propagates [`Error`] if the blob is malformed.
///
/// Name: provisional (milestone 592), 2026-09-25; moved here from `jh7110_clock_and_reset` by
/// lane crates-stand-alone, 2026-10-25 (UTC). calef names public functions.
pub fn discover_sys(tree: &DeviceTreeBlob<'_>) -> Result<Found, Error> {
    discover_window(tree, &SYS_WINDOW)
}

/// The walk the three `discover` functions share: each spelling in order, then the constant.
fn discover_window(tree: &DeviceTreeBlob<'_>, window: &Window) -> Result<Found, Error> {
    for spelling in &window.spellings {
        let mut regions = [Region { start: 0, size: 0 }; MAX_WINDOWS];
        let n = tree.node_reg_compatible(spelling.compatible, &mut regions)?;
        if n == 0 {
            continue;
        }
        let names = match spelling.window {
            None => None,
            Some(_) => tree.node_prop_compatible(spelling.compatible, b"reg-names")?,
        };
        let mut pairs = [(0, 0); MAX_WINDOWS];
        for (pair, region) in pairs.iter_mut().zip(&regions[..n]) {
            *pair = (region.start, region.size);
        }
        if let Some(found) = Found::from_regions(spelling, &pairs[..n], names) {
            return Ok(found);
        }
    }
    Ok(window.fallback())
}

/// **Read the PMIC's I2C bus's clocks and resets out of `tree`**: the `clocks`, `resets` and
/// `reg` of the AXP15060's parent bus and the PMIC's own `reg`, mainline's `compatible` tried
/// first, handed to [`PmicBus::from_bus`]. A tree naming no known PMIC gets
/// [`PmicBus::without_pmic`]. Like [`discover`], an answer is not evidence of a JH7110.
///
/// # Errors
///
/// Propagates [`Error`] if the blob is malformed.
///
/// Name: recorded (milestone 592), which wrote it as `jh7110_clock_and_reset::pmic_bus`. calef
/// has not ruled on it.
pub fn pmic_bus(tree: &DeviceTreeBlob<'_>) -> Result<PmicBus, Error> {
    for pmic in PMIC_COMPATIBLES {
        if tree.node_prop_compatible(pmic, b"compatible")?.is_none() {
            continue;
        }
        let clocks = tree.parent_prop_compatible(pmic, b"clocks")?;
        let resets = tree.parent_prop_compatible(pmic, b"resets")?;
        // Both addresses are optional in the tree and neither affects the plan, so a read that
        // fails leaves each unstated rather than failing the plan.
        let pmic_reg = tree.node_prop_compatible(pmic, b"reg").ok().flatten();
        let bus_reg = tree.parent_prop_compatible(pmic, b"reg").ok().flatten();
        return PmicBus::from_bus(pmic, pmic_reg, bus_reg, clocks, resets, |phandle, name| {
            tree.phandle_prop(phandle, name)
        });
    }
    Ok(PmicBus::without_pmic())
}
