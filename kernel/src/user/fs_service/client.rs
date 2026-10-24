//! Spawning a client of the service, and the two waits that order a client after the
//! servers and caretakers it depends on.
//!
//! See notes/fs-server/kernel-wiring.md.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// The most extra stack pages [`spawn_fs_client`] will map below the one `run` gives every
/// program. Small on purpose: a client that needs more than this is a client whose frames want
/// looking at, not a number that wants raising.
pub(super) const CLIENT_EXTRA_STACK: usize = 8;

/// The pages a granted client's endpoints are retyped from: its caretaker's two, the program's
/// report, and one spare. One page per endpoint, so a grant's whole wiring goes back with one
/// region.
pub(super) const GRANT_ENDPOINT_PAGES: u64 = 4;

/// Spawn one client holding exactly a file-service endpoint, a report endpoint and its view of
/// the shared page. Returns the client's thread.
///
/// `role`, `arg` and `arg2` are the client's three `START` words. `report` is the caller's, so a
/// granted client's comes out of the region its caretaker's endpoints do and goes back with them.
///
/// `extra_stack` is pages below the one `run` maps, at most [`CLIENT_EXTRA_STACK`]. A shell's need
/// tracks the largest value `grant_plan` hands it; an overflow presents as a data abort on `sp`,
/// then the 60-second lost-wakeup watchdog. See notes/fs-server/kernel-wiring.md.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_fs_client(
    client_image: &'static [u8],
    file_ep: RendezvousId,
    file_shared: u64,
    role: u64,
    arg: u64,
    arg2: u64,
    extra_stack: usize,
    report: RendezvousId,
) -> crate::thread::ThreadId {
    assert!(
        extra_stack <= CLIENT_EXTRA_STACK,
        "an FS client asked for more stack than this wiring maps",
    );
    crate::sched::spawn(move || {
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; FILE_PAGES + CLIENT_EXTRA_STACK];
        // The whole channel: the throughput benchmark comes through here, and a client may not ask
        // for more than it mapped (`filesystem_contract::fs::TRANSFER_PAGES`).
        let n = map_channel(&mut maps, FILE_VA_CLIENT, file_shared, FILE_PAGES);
        for (k, m) in maps[n..n + extra_stack].iter_mut().enumerate() {
            m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
            m.phys = page_frame();
        }
        run(
            client_image,
            Spawn {
                arg0: role,
                arg1: arg,
                // `rm` is started as a grant is (`filesystem_contract::grant`); others pass zero.
                arg2,
                grants: &[
                    rendezvous_cap(file_ep, Rights::WRITE), // slot 0: CALL the FS server
                    rendezvous_cap(report, Rights::WRITE),  // slot 1: report to the kernel
                ],
                maps: &maps[..n + extra_stack],
            },
        )
    })
    .expect("could not spawn the FS client")
}

/// Wire the service (or reuse this boot's) and spawn the hand-written client
/// (`fixtures/src/fs_test_client/main.rs`): the file-service endpoint, which IS its directory capability, the
/// report endpoint, and its view of the shared file page. It names nothing else in the system.
///
/// Returns `(readiness, report)`: the two readiness endpoints if this call wired the service,
/// and the endpoint the client reports on.
pub fn start(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    client_image: &'static [u8],
    client_role: u64,
) -> Option<(Readiness, RendezvousId)> {
    let (file_ep, file_shared, readiness) = ensure(blk_image, fs_server_image)?;
    // 0 = the end-to-end proof; 1 = the fs_read benchmark loop.
    let report = crate::sched::create_rendezvous();
    spawn_fs_client(
        client_image,
        file_ep,
        file_shared,
        client_role,
        0,
        0,
        0,
        report,
    );
    Some((readiness, report))
}

/// Drain the FS service's two readiness sentinels, which is a sequencing act and not only an
/// assertion.
///
/// Both servers announce with a blocking `SEND`, so each is parked inside its own announcement
/// until somebody receives it. Nothing they serve can be answered before that. Every caller that
/// needs the service to be *running* rather than merely spawned has to come through here first,
/// and [`wait_for_caretaker`] documents what that ordering is load-bearing for.
///
/// `None` means an earlier caller in this boot already wired the service and drained these.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn wait_for_service(readiness: Readiness) {
    let Some((blk_ready, fs_ready)) = readiness else {
        return;
    };
    // `None` means the block service was already running when this one was wired, so its sentinel
    // has been drained by whoever started it and there is nothing here to wait for.
    if let Some(blk_ready) = blk_ready {
        assert_eq!(
            crate::sched::ipc_receive(blk_ready)[0],
            filesystem_contract::READY,
            "the block server did not bring the RedoxFS device up",
        );
    }
    assert_eq!(
        crate::sched::ipc_receive(fs_ready)[0],
        filesystem_contract::READY,
        "the FS server did not open the RedoxFS image",
    );
}

/// Wait for a caretaker's startup request to have been answered, before the program it confines
/// exists at all.
///
/// The FS server, the caretaker and the client share one frame. At startup the caretaker stages
/// the granted name there and blocks in a `CALL`; a client that already existed could write over
/// it, and the FS server would resolve whatever it found. So: drain the service, wait for this
/// sentinel, then spawn the client. See notes/fs-server/kernel-wiring.md.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(super) fn wait_for_caretaker(caretaker_ready: RendezvousId) {
    assert_eq!(
        crate::sched::ipc_receive(caretaker_ready)[0],
        filesystem_contract::READY,
        "the caretaker could not open what it was granted, so there is nothing to attenuate",
    );
}
