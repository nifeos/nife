//! The register window and the spawn handoff, proved.
//!
//! Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract, after the
//! sibling `extensible_host_controller_interface::proofs`.

use super::*;

/// A structure of up to one page, so on at most two, anywhere in the first 64 KiB of BAR 0 or 1.
fn any_structure() -> Structure {
    let s = Structure {
        bar: kani::any(),
        offset: kani::any(),
        length: kani::any(),
    };
    kani::assume(s.bar < 2 && s.length <= PAGE_BYTES && s.offset < 1 << 16);
    s
}

/// **No mapped page overlaps the withheld range or runs past its BAR, and every byte of the
/// symbolic structure lies on a mapped page at the offset the handoff carries**, for any notify
/// structure of up to one page, any device configuration, any one withheld range and any BAR
/// sizes, beside a common configuration and ISR byte fixed on pages 0 and 1 of BAR 0. Every
/// structure passes through the same per-page loop, so one symbolic structure and one optional
/// one exercise it whole; four symbolic structures ran the C Bounded Model Checker (CBMC) out of
/// memory.
/// Falsification: replayable `crates/driver_process_contract/falsifications/virtual_input_output.proofs.the_window_never_maps_a_withheld_page.patch`
#[kani::proof]
#[kani::unwind(10)]
fn the_window_never_maps_a_withheld_page() {
    let structures = Structures {
        common: Structure {
            bar: 0,
            offset: 0,
            length: COMMON_CONFIGURATION_BYTES,
        },
        notify: any_structure(),
        interrupt_status: Structure {
            bar: 0,
            offset: PAGE_BYTES,
            length: 1,
        },
        device: if kani::any() {
            Some(any_structure())
        } else {
            None
        },
    };
    let withheld = Withheld {
        bar: kani::any(),
        start: kani::any(),
        end: kani::any(),
    };
    kani::assume(withheld.start < withheld.end);
    let bar_bytes: [u64; 6] = kani::any();
    if let Ok(window) = register_window(&structures, &bar_bytes, &[withheld]) {
        for p in window.pages() {
            assert!(p.offset + PAGE_BYTES <= bar_bytes[usize::from(p.bar)]);
            assert!(
                !(p.bar == withheld.bar
                    && withheld.start < p.offset + PAGE_BYTES
                    && p.offset < withheld.end)
            );
        }
        // The notify structure's last byte, found at its window offset, is on the page that
        // holds it.
        let last = structures.notify.offset + structures.notify.length - 1;
        let at = u64::from(window.notify) + (last - structures.notify.offset);
        let page = window.pages()[(at / PAGE_BYTES) as usize];
        assert!(page.bar == structures.notify.bar && page.offset == last / PAGE_BYTES * PAGE_BYTES);
    }
}

/// **The handoff round-trips, and anything `unpack` accepts is a layout a driver can use**: a
/// page-aligned nonzero base and offsets inside the register window.
/// Falsification: replayable `crates/driver_process_contract/falsifications/virtual_input_output.proofs.the_hand_off_round_trips.patch`
#[kani::proof]
fn the_hand_off_round_trips() {
    let w: [u64; 3] = kani::any();
    if let Some(h) = HandOff::unpack(w) {
        assert_eq!(h.pack(), w);
        assert!(h.data_plane_physical_address.is_multiple_of(PAGE_BYTES));
        assert!(h.data_plane_physical_address != 0);
        assert!(u64::from(h.common) < WINDOW_BYTES && u64::from(h.notify) < WINDOW_BYTES);
        assert!(u64::from(h.interrupt_status) < WINDOW_BYTES);
    }
}
