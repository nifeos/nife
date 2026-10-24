//! The shared-frame witness: two live clients on one file service, each on its own window, and the
//! negative control that puts them on one. See notes/a-frame-per-filesystem-client-channel.md.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// Wire two live clients on one file service for the shared-frame witness
/// (`notes/shared-page-audit.md`, finding 1): a victim and an attacker in `victim_role` and
/// `attacker_role`, each on its own claimed window and a badged endpoint naming it.
///
/// Both read one slot layout: FILE at 0, REPORT at 1, SYNC at 2, which sequences them so the test
/// does not depend on a race. `shared` puts both on one window, the witness's negative control.
///
/// Returns `(readiness, victim_report, held)`, or `None` when no RedoxFS disk is attached or the
/// pool has no free window. Provisional name; an architect names functions.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_shared_frame_witness(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    client_image: &'static [u8],
    victim_role: u64,
    attacker_role: u64,
    shared: bool,
) -> Option<(Readiness, RendezvousId, super::holding::Holding)> {
    let (file_ep, _file_shared, readiness) = ensure(blk_image, fs_server_image)?;
    let ep_region = crate::memory_region::create(3).expect("no endpoint region for the witness");
    let endpoint =
        || crate::sched::create_rendezvous_from(ep_region).expect("no endpoint for the witness");
    let sync = endpoint();
    let victim_report = endpoint();
    let attacker_report = endpoint();
    let mut held = super::holding::Holding::new();
    held.add_region(ep_region);

    // The FS server is parked in its readiness SEND until this drains it, so a victim that
    // called first would block forever.
    wait_for_service(readiness);
    // A window per client, badged with its index, unless `shared` puts the attacker on the
    // victim's.
    let (victim_win, victim_phys) = claim_window()?;
    let (attacker_win, attacker_phys) = if shared {
        (victim_win, victim_phys)
    } else {
        claim_window()?
    };

    // A client that mints its own badge gets an unbadged endpoint with `GRANT` and its window in
    // `arg1`; otherwise it is handed a pre-badged endpoint, the shape the progenitor builds.
    let spawn_witness = move |role: u64, report: RendezvousId, win: u64, phys: u64, mint: bool| {
        crate::sched::spawn(move || {
            let mut maps = [Mapping {
                va: 0,
                phys: 0,
                flags: Flags::user_data(),
            }; FILE_PAGES];
            let n = map_channel(&mut maps, FILE_VA_CLIENT, phys, FILE_PAGES);
            let file = if mint {
                rendezvous_cap(file_ep, Rights::WRITE.union(Rights::GRANT))
            } else {
                rendezvous_cap_badged(file_ep, Rights::WRITE, win)
            };
            run(
                client_image,
                Spawn {
                    arg0: role,
                    arg1: if mint { win } else { 0 }, // the window a minting client badges for
                    arg2: 0,
                    grants: &[
                        file,                                                    // slot 0: the FS endpoint
                        rendezvous_cap(report, Rights::WRITE), // slot 1: report to the kernel
                        rendezvous_cap(sync, Rights::READ.union(Rights::WRITE)), // slot 2: handshake
                    ],
                    maps: &maps[..n],
                },
            )
        })
        .expect("could not spawn a shared-frame witness client")
    };

    // The victim mints its badge and the attacker is handed one, so both shapes are covered; the
    // sync handshake makes the start order immaterial.
    held.add_thread(spawn_witness(
        victim_role,
        victim_report,
        victim_win,
        victim_phys,
        true,
    ));
    held.add_thread(spawn_witness(
        attacker_role,
        attacker_report,
        attacker_win,
        attacker_phys,
        false,
    ));

    Some((readiness, victim_report, held))
}
