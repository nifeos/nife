//! The register window and the spawn handoff, proved.
//!
//! Name: provisional, minted 2026-10-26 (UTC) by lane xhci-names-ratified, after the sibling
//! `designware_mobile_storage::proofs`.

use super::*;

/// **No page that overlaps a withheld range is ever in the mask**, for any capability values,
/// any BAR size and any one withheld range: the MSI-X table is never mapped into the driver.
/// Falsification: replayable `crates/driver_process_contract/falsifications/extensible_host_controller_interface.proofs.the_window_never_maps_a_withheld_page.patch`
#[kani::proof]
#[kani::unwind(18)]
fn the_window_never_maps_a_withheld_page() {
    let capabilities = Capabilities::decode(
        kani::any(),
        kani::any(),
        kani::any(),
        kani::any(),
        kani::any(),
        kani::any(),
    );
    let bar: u64 = kani::any();
    let a: u64 = kani::any();
    let len: u64 = kani::any();
    kani::assume(len > 0 && len <= 0x1_0000 && a <= u64::MAX - len);
    if let Ok(mask) = register_window(&capabilities, bar, &[(a, a + len)]) {
        for page in 0..MAX_REGISTER_PAGES {
            if mask & (1 << page) != 0 {
                let (s, e) = (page * PAGE_BYTES, (page + 1) * PAGE_BYTES);
                assert!(!(a < e && s < a + len));
            }
        }
        assert!(mask >> MAX_REGISTER_PAGES == 0);
    }
}

/// **The handoff round-trips, and anything `unpack` accepts is a layout the driver can use.**
/// Falsification: replayable `crates/driver_process_contract/falsifications/extensible_host_controller_interface.proofs.the_handoff_round_trips.patch`
#[kani::proof]
fn the_handoff_round_trips() {
    let w: [u64; 3] = kani::any();
    if let Some(h) = Handoff::unpack(w) {
        assert_eq!(h.pack(), w);
        assert!(u64::from(h.dma_pages) >= WORK_PAGES);
        assert!(h.registers & 1 != 0);
    }
}
