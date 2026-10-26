//! The spawn handoff, proved: it round-trips, and an accepted one keeps every doorbell inside the
//! one mapped page.
//!
//! Name: provisional, minted 2026-10-26 (UTC) by lane driver-contracts-folded, after the sibling
//! `extensible_host_controller_interface::proofs`.

use ::non_volatile_memory_express::{Doorbell, doorbell};

use super::*;

/// **The spawn handoff is lossless in the direction it is used**: whatever the kernel's admin
/// plane packs into three words, the EL0 data plane unpacks unchanged. The split between a
/// kernel admin plane and an unprivileged data plane makes this the only channel by which it
/// learns its own geometry, so a shift expression that was not its own inverse would be a
/// driver addressing the wrong LBA with nothing in between to notice.
/// Falsification: replayable `crates/driver_process_contract/falsifications/non_volatile_memory_express.proofs.the_spawn_handoff_round_trips.patch`
#[kani::proof]
fn the_spawn_handoff_round_trips() {
    let h = Handoff {
        dstrd: kani::any(),
        blocks_per: kani::any(),
        entries: kani::any(),
        data_plane_phys: kani::any(),
        size_bytes: kani::any(),
    };
    kani::assume(h.dstrd <= MAX_DSTRD && (1..=8).contains(&h.blocks_per) && h.entries >= 2);
    assert_eq!(Handoff::unpack(h.pack()), Some(h));
}

/// **An unpacked handoff can always drive the doorbell arithmetic**, for every word the
/// kernel could have packed. [`MAX_DSTRD`] is what buys this, and this harness is why that
/// constant exists: a stride the spec permits and QEMU never reports computes a doorbell
/// offset outside the one page of BAR0 the EL0 data plane is mapped, which is the one
/// arithmetic error in this driver that reaches hardware.
/// Falsification: replayable `crates/driver_process_contract/falsifications/non_volatile_memory_express.proofs.an_accepted_handoff_keeps_its_doorbells_inside_the_mapped_page.patch`
#[kani::proof]
fn an_accepted_handoff_keeps_its_doorbells_inside_the_mapped_page() {
    let words: [u64; 3] = [kani::any(), kani::any(), kani::any()];
    if let Some(h) = Handoff::unpack(words) {
        // Queue 0 (admin) and queue 1 (the one I/O pair the admin plane creates) are the only
        // doorbells this driver can name; with the largest stride `CAP` can report they are
        // the four lowest offsets in the doorbell page.
        for qid in 0..=1u16 {
            for which in [Doorbell::SubmissionTail, Doorbell::CompletionHead] {
                let off = doorbell(qid, which, h.dstrd);
                assert!((0x1000..0x2000).contains(&off));
            }
        }
    }
}
