//! A per-file grant: a caretaker narrows the directory to one file, and the confined program holds
//! only the caretaker. See notes/grant-expression.md.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// A per-file grant, as one value: which file, in which direction, handed to which program started
/// how. One value, so a caller cannot get `rights` and `role`, both bare integers, the wrong way
/// round.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct Grant {
    /// The one name the caretaker will answer for. Must fit [`filesystem_contract::grant::WORDS_NAME`].
    pub name: &'static str,
    /// `grant::READ`, or `READ | WRITE`.
    pub rights: u64,
    /// The confined program's `arg0` (its role) and `arg1`.
    pub role: u64,
    pub arg: u64,
}

/// Wire a per-file grant and the program that holds it: what `run wc report.txt` resolves to,
/// wired by the test suite rather than the shell.
///
/// The FS server, an `fs_file_caretaker` holding the directory, and the program, which holds only
/// the caretaker's endpoint and nothing that names the FS server. All three share one frame.
/// Returns the program's report endpoint and what ends the grant. See
/// notes/fs-server/kernel-wiring.md.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_granted(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    client_image: &'static [u8],
    grant: Grant,
) -> Option<(RendezvousId, super::holding::Holding)> {
    let Grant {
        name,
        rights,
        role: client_role,
        arg: client_arg,
    } = grant;
    let (file_ep, file_shared, readiness) = ensure(blk_image, fs_server_image)?;
    // The caretaker's two endpoints and the program's report, from one region the returned
    // holding reclaims, as [`narrow_dir_held`] does for a directory.
    let ep_region =
        crate::memory_region::create(GRANT_ENDPOINT_PAGES).expect("no endpoint region for a grant");
    let endpoint =
        || crate::sched::create_rendezvous_from(ep_region).expect("no endpoint for a grant");
    let narrow_ep = endpoint();
    let caretaker_ready = endpoint();
    let report = endpoint();

    let (lo, hi) = filesystem_contract::grant::pack_name(name.as_bytes())
        .expect("a granted name rides in two argument words; this one does not fit");
    let spec = filesystem_contract::grant::spec(name.len(), rights);

    // The grant (the name and the direction) rides in the caretaker's START arguments, so a
    // per-file grant costs no frame.
    let caretaker = crate::sched::spawn(move || {
        run(
            caretaker_image,
            Spawn {
                arg0: lo,
                arg1: hi,
                arg2: spec,
                grants: &[
                    rendezvous_cap(file_ep, Rights::WRITE), // slot 0: CALL the FS server
                    rendezvous_cap(narrow_ep, Rights::READ), // slot 1: serve the narrowed capability
                    rendezvous_cap(caretaker_ready, Rights::WRITE), // slot 2: readiness, once
                ],
                maps: &[Mapping {
                    va: FILE_VA_CLIENT,
                    phys: file_shared,
                    flags: Flags::user_data(),
                }],
            },
        )
    })
    .expect("could not spawn the file caretaker");

    // Both handshakes before the client exists, in this order and for [`wait_for_caretaker`]'s
    // reason: the servers are parked inside their announcements until they are drained, and the
    // caretaker's own request is staged in the page all three share.
    wait_for_service(readiness);
    wait_for_caretaker(caretaker_ready);

    let mut held = super::holding::Holding::new();
    held.add_thread(caretaker);
    held.add_region(ep_region);
    held.add_thread(spawn_fs_client(
        client_image,
        narrow_ep,
        file_shared,
        client_role,
        client_arg,
        0,
        0,
        report,
    ));
    Some((report, held))
}
