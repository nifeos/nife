//! The spawn handoff, proved.
//!
//! Name: provisional, minted 2026-10-26 (UTC) by lane driver-contracts-folded, after the sibling
//! `extensible_host_controller_interface::proofs`.

use super::*;

/// **The handoff round-trips, and anything it accepts is page aligned with a unicast MAC.**
///
/// Falsification: replayable `crates/driver_process_contract/falsifications/e1000e.proofs.the_handoff_round_trips_and_accepts_only_what_pack_can_make.patch`
#[kani::proof]
fn the_handoff_round_trips_and_accepts_only_what_pack_can_make() {
    let a0: u64 = kani::any();
    let a1: u64 = kani::any();
    if let Some(h) = Handoff::unpack(a0, a1) {
        assert!(
            h.data_plane_physical_address.is_multiple_of(PAGE)
                && h.data_plane_physical_address != 0
        );
        assert!(h.station_address[0] & 1 == 0);
        let [p0, p1] = h.pack();
        assert!(Handoff::unpack(p0, p1) == Some(h));
    }
}
