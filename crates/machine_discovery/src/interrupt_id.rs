//! Which interrupt line does this device raise? The `interrupts` property, decoded per the
//! interrupt parent's own `#interrupt-cells`.
//!
//! The walk is the device's `interrupts`, the nearest `interrupt-parent` (inheritable), the
//! controller that phandle names, its `#interrupt-cells`, then the entry. One cell is a PLIC source
//! number; two cells are `<number trigger>` only when the parent is a PLIC; three are a GIC's
//! `<type number flags>`, whose bank-relative number becomes an INTID. See
//! `notes/machine-discovery.md` ("Interrupt ids").

//! # BUGS
//!
//! - Only the first `interrupts` entry is decoded, the convention every UART binding here follows.
//! - Any other cell count answers `None`: a number read from the wrong cell would arm the wrong
//!   source, which is the bug this module exists to end.
//! - `interrupts-extended` on the device is not read; the PLIC's own use of it is
//!   [`crate::plic`]'s.
//!
//! Name: provisional (`interrupt_id` for the module, following `cpu_list`'s pattern of a shared
//! question beside the two arch records; the GIC spelling "INTID" is where the noun comes from).

use device_tree_blob::{DeviceTreeBlob, Error};

/// The GIC's SPI bank base: `interrupts = <0 n ...>` means INTID `32 + n`.
const GIC_SPI_BASE: u32 = 32;
/// The GIC's PPI bank base: `interrupts = <1 n ...>` means INTID `16 + n`.
const GIC_PPI_BASE: u32 = 16;

/// **The interrupt id of the first node whose name starts with `node`**, decoded per its interrupt
/// parent's `#interrupt-cells`. `Ok(None)` when the tree does not say: no such node, no
/// `interrupts`, no resolvable `interrupt-parent`, or an entry shape this decoder does not
/// understand (see the module's BUGS). The caller owns the fallback, and should say which source
/// won, so a bench transcript names the number's origin.
pub fn of_node(dt: &DeviceTreeBlob<'_>, node: &[u8]) -> Result<Option<u32>, Error> {
    let Some(interrupts) = dt.node_prop(node, b"interrupts")? else {
        return Ok(None);
    };
    let Some(parent) = dt.node_prop_inherited(node, b"interrupt-parent")? else {
        return Ok(None);
    };
    let Some(phandle) = be32_word(parent, 0) else {
        return Ok(None);
    };
    let Some(cells_prop) = dt.phandle_prop(phandle, b"#interrupt-cells")? else {
        return Ok(None);
    };
    let Some(cells) = be32_word(cells_prop, 0) else {
        return Ok(None);
    };

    Ok(match cells {
        // A PLIC (or any single-cell parent): the cell is the source number.
        1 => be32_word(interrupts, 0),
        // A PLIC stating two cells: <number trigger>, the binding's newer form, which T-Head's
        // TH1520 uses. Two cells mean something else on other controllers, so the first cell is
        // the source only when the parent says it is a PLIC.
        2 if is_a_plic(dt, phandle)? => be32_word(interrupts, 0),
        // A GIC: <type number flags>, the number relative to its bank's base.
        3 => match (be32_word(interrupts, 0), be32_word(interrupts, 1)) {
            (Some(0), Some(n)) => GIC_SPI_BASE.checked_add(n),
            (Some(1), Some(n)) if n < 16 => Some(GIC_PPI_BASE + n),
            _ => None,
        },
        _ => None,
    })
}

/// Is the controller `phandle` names a PLIC: does it state one of [`crate::plic::COMPATIBLES`]?
fn is_a_plic(dt: &DeviceTreeBlob<'_>, phandle: u32) -> Result<bool, Error> {
    let Some(compatible) = dt.phandle_prop(phandle, b"compatible")? else {
        return Ok(false);
    };
    Ok(compatible
        .split(|&b| b == 0)
        .any(|entry| crate::plic::COMPATIBLES.contains(&entry)))
}

/// Big-endian cell `i` of a property's bytes, `None` when the property is too short.
fn be32_word(bytes: &[u8], i: usize) -> Option<u32> {
    let at = i.checked_mul(4)?;
    bytes
        .get(at..at.checked_add(4)?)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}
