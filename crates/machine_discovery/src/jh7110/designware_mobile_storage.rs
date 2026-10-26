//! Where the JH7110's two SD/MMC controllers are, and how each slot is wired, for
//! `designware_mobile_storage`.
//!
//! The controllers came with milestone 53 (the board's own peripherals: network and storage on
//! real silicon).
//!
//! Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, after the crate it
//! serves. calef has not named it.

use designware_mobile_storage::jh7110::{NODE_NAMES, Slot};
use device_tree_blob::{DeviceTreeBlob, Error, Region};

/// **Find slot `index` (0 or 1) in `tree`**, or `None` when the tree names no such controller
/// under either node name, or names one whose `compatible` is neither binding.
///
/// No constant fallback, unlike the clock and reset windows: a tree that does not describe the
/// controller is not a tree for a board this driver knows the wiring of, and guessing a bus width
/// for a slot nobody described is how a write lands on the wrong device.
///
/// # Errors
///
/// Propagates [`Error`] if the blob is malformed.
///
/// Name: recorded (milestone 53), which wrote it as `designware_mobile_storage::jh7110::discover`.
/// calef has not ruled on it.
pub fn discover(tree: &DeviceTreeBlob<'_>, index: u8) -> Result<Option<Slot>, Error> {
    let Some(names) = NODE_NAMES.get(usize::from(index)) else {
        return Ok(None);
    };
    for name in names {
        let mut reg = [Region { start: 0, size: 0 }; 1];
        if tree.node_reg(name, &mut reg)? == 0 {
            continue;
        }
        let prop = |p: &[u8]| tree.node_prop(name, p);
        if let Some(slot) = Slot::from_node(index, reg[0].start, reg[0].size, prop)? {
            return Ok(Some(slot));
        }
    }
    Ok(None)
}
