//! The block server's window, proved.
//!
//! Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the device-tree
//! walk and the process layout left the driver crates.

use super::{SECTORS_PER_BLOCK, Window};

/// No block request the server translates reaches a sector outside its window.
///
/// Falsification: replayable `crates/driver_process_contract/falsifications/designware_mobile_storage.proofs.a_served_block_never_leaves_its_window.patch`
#[kani::proof]
fn a_served_block_never_leaves_its_window() {
    let w = Window {
        first: kani::any(),
        sectors: kani::any(),
        writable: kani::any(),
    };
    kani::assume(w.first.checked_add(w.sectors).is_some());
    let block: u64 = kani::any();
    let count: u64 = kani::any();
    if let Some(lba) = w.translate(block, count) {
        assert!(lba >= w.first);
        let span = count * SECTORS_PER_BLOCK;
        assert!(lba + span <= w.first + w.sectors);
    }
}
