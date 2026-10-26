//! Where the JH7110's first Ethernet port and its AON syscon are, and what the board wired to the
//! port, for `designware_ethernet`.
//!
//! The port and its PHY came with milestone 53 (the board's own peripherals: network and storage
//! on real silicon).
//!
//! Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, after the crate it
//! serves. calef has not named it.

use designware_ethernet::jh7110::{
    AON_SYSCON_NODE, COMPATIBLE_AON_SYSCON, COMPATIBLES, Discovered, Syscon, VENDOR_PHY_NODE,
    first_cell, mainline_phy, vendor_phy,
};
use device_tree_blob::{DeviceTreeBlob, Error, Region};

/// **Find the first port in `tree`**: mainline's spelling first, then the vendor's, the order the
/// TRNG's discovery uses and for its reason. `Ok(None)` when neither names a node. The PHY is read
/// through `phy-handle` when the port has one (mainline), else from the vendor's PHY node.
///
/// # Errors
///
/// Propagates [`Error`] if the blob is malformed.
///
/// Name: recorded (milestone 53), which wrote it as `designware_ethernet::jh7110::discover`.
/// calef has not ruled on it.
pub fn discover(tree: &DeviceTreeBlob<'_>) -> Result<Option<Discovered>, Error> {
    for compatible in COMPATIBLES {
        let mut regions = [Region { start: 0, size: 0 }; 1];
        if tree.node_reg_compatible(compatible, &mut regions)? == 0 {
            continue;
        }
        let prop = |name: &[u8]| tree.node_prop_compatible(compatible, name);
        let phy = match prop(b"phy-handle")?.and_then(first_cell) {
            Some(handle) => mainline_phy(|name| tree.phandle_prop(handle, name))?,
            None => vendor_phy(|name| tree.node_prop(VENDOR_PHY_NODE, name))?,
        };
        return Discovered::from_node(regions[0].start, regions[0].size, compatible, prop, phy)
            .map(Some);
    }
    Ok(None)
}

/// **Find the AON syscon**: mainline's compatible, then the vendor's node name, then the constant
/// both trees agree on. Like [`super::clock_and_reset::discover`], never fails to produce an
/// address, so an answer here is not evidence the machine is a JH7110.
///
/// # Errors
///
/// Propagates [`Error`] if the blob is malformed.
///
/// Name: recorded (milestone 53), which wrote it as
/// `designware_ethernet::jh7110::discover_aon_syscon`. calef has not ruled on it.
pub fn discover_aon_syscon(tree: &DeviceTreeBlob<'_>) -> Result<Syscon, Error> {
    let mut regions = [Region { start: 0, size: 0 }; 1];
    if tree.node_reg_compatible(COMPATIBLE_AON_SYSCON, &mut regions)? == 1
        || tree.node_reg(AON_SYSCON_NODE, &mut regions)? == 1
    {
        return Ok(Syscon {
            base: regions[0].start,
            size: regions[0].size,
            from_tree: true,
        });
    }
    Ok(Syscon::fallback())
}
