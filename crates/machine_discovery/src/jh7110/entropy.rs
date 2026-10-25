//! Where the JH7110's TRNG is, for `jh7110_entropy`.
//!
//! Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, after the crate it
//! serves. calef has not named it.

use device_tree_blob::{DeviceTreeBlob, Error, Region};
use jh7110_entropy::{COMPATIBLES, Discovered};

/// Find the JH7110 TRNG in `tree`, if it is there at all.
///
/// `Ok(None)` is not a parse failure: it is the answer on QEMU's riscv64 `virt`, which every CI
/// boot uses, and a test pins it. The spellings are tried in [`COMPATIBLES`]' order, mainline's
/// first, and every property is read from the node that matched, so a tree carrying an unrelated
/// node under the other spelling can never contribute a `reg` or an `interrupts` to the answer.
///
/// # Errors
///
/// Propagates [`Error`] if the blob is malformed.
///
/// Name: recorded (milestone 159), which wrote it as `jh7110_entropy::discover`, a real hardware
/// entropy source for the JH7110. The module path carries the chip and the device, so the verb
/// alone is left. calef has not ruled on it.
pub fn discover(tree: &DeviceTreeBlob<'_>) -> Result<Option<Discovered>, Error> {
    for compatible in COMPATIBLES {
        let mut regions = [Region { start: 0, size: 0 }; 1];
        if tree.node_reg_compatible(compatible, &mut regions)? == 0 {
            continue;
        }
        let interrupt = tree
            .node_prop_compatible(compatible, b"interrupts")?
            .and_then(jh7110_entropy::interrupt);
        let status = tree.node_prop_compatible(compatible, b"status")?;
        return Ok(Some(Discovered {
            reg_base: regions[0].start,
            reg_size: regions[0].size,
            interrupt,
            compatible,
            status_okay: jh7110_entropy::status_okay(status),
        }));
    }
    Ok(None)
}
