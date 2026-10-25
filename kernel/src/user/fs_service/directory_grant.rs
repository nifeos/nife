//! A per-directory grant: the root directory capability, a subtree caretaker that narrows it, and
//! the program confined to that subtree. See notes/dir-capability.md.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// A per-directory grant and the program confined to it, as [`start_granted_dir`] wires them: the
/// FS server, an `fs_subtree_caretaker` narrowing the image root to one subtree, and the program,
/// which holds only the caretaker's endpoint.
///
/// The FS server's handle table is per server, so a rights-carrying handle alone would not confine
/// a program that could still name [`filesystem_contract::fs::ROOT`]. See
/// notes/fs-server/kernel-wiring.md.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct DirGrant {
    /// The directory the caretaker descends into, one component under the image root. Must fit
    /// [`filesystem_contract::grant::WORDS_NAME`].
    pub name: &'static str,
    /// The [`filesystem_contract::dir`] rights the caretaker asks for. The FS server intersects
    /// them with the root's and refuses if the answer is smaller, so asking for more than exists
    /// fails at the caretaker's first request.
    pub rights: u64,
    /// The confined program's `arg0` (its role) and `arg1`.
    pub role: u64,
    pub arg: u64,
    /// Its `arg2`. Zero for every client but `rm`, which is started with a grant's three words (a
    /// spec and two of name), its "role" word being the spec.
    pub arg2: u64,
    /// Stack pages beyond the one `run` maps, for a confined program that needs them. The
    /// hand-written attacker needs none; a shell does (see [`spawn_fs_client`]).
    pub stack_pages: usize,
}

/// Wire the filesystem and hand back the root directory capability: the entry point the
/// interactive boot uses.
///
/// It brings up the block server and the FS server and drains both readiness sentinels, so the
/// service is running before the progenitor exists. Returns `(the file-service endpoint, the
/// physical frame its clients map)`; the endpoint is the directory capability at the image root
/// (§27 (the filesystem service)), handed to the shell unnarrowed. `None` means no RedoxFS disk is
/// attached, the normal case for a plain `cargo xtask run`.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn root_directory(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
) -> Option<(RendezvousId, u64)> {
    let (file_ep, file_shared, readiness) = ensure(blk_image, fs_server_image)?;
    wait_for_service(readiness);
    Some((file_ep, file_shared))
}

/// Wire a subtree caretaker for `name` with `rights` and hand back the narrowed capability,
/// without spawning whatever is going to hold it: [`start_granted_dir`]'s first half, for a client
/// unlike [`spawn_fs_client`]'s.
///
/// Returns `(narrow_ep, file_shared)`, with both handshakes already drained for
/// [`wait_for_caretaker`]'s reason, so a caller may spawn its client the moment this returns.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn narrow_dir(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    name: &'static str,
    rights: u64,
) -> Option<(RendezvousId, u64)> {
    let narrow_ep = crate::sched::create_rendezvous();
    let caretaker_ready = crate::sched::create_rendezvous();
    let (file_shared, _caretaker) = spawn_caretaker(
        blk_image,
        fs_server_image,
        caretaker_image,
        name,
        rights,
        narrow_ep,
        caretaker_ready,
    )?;
    Some((narrow_ep, file_shared))
}

/// [`narrow_dir`], with a caretaker the caller can end, and a report endpoint for the program.
///
/// [`narrow_dir`]'s caretaker parks in `RECEIVE` on an endpoint no region owns, so it lives for the
/// boot. Here the endpoints come out of a region of their own, and the returned
/// [`super::holding::Holding`] reclaims it, which wakes the caretaker to die.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn narrow_dir_held(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    name: &'static str,
    rights: u64,
) -> Option<(RendezvousId, u64, RendezvousId, super::holding::Holding)> {
    // The caretaker's two endpoints and the program's stdout, so one reclaim returns them all: the
    // kernel's rendezvous registry is a fixed 512 and the suite runs close to it.
    let ep_region = crate::memory_region::create(GRANT_ENDPOINT_PAGES)
        .expect("no endpoint region for a caretaker");
    let endpoint =
        || crate::sched::create_rendezvous_from(ep_region).expect("no endpoint for a caretaker");
    let narrow_ep = endpoint();
    let caretaker_ready = endpoint();
    let report = endpoint();
    let (file_shared, caretaker) = spawn_caretaker(
        blk_image,
        fs_server_image,
        caretaker_image,
        name,
        rights,
        narrow_ep,
        caretaker_ready,
    )?;
    let mut held = super::holding::Holding::new();
    held.add_thread(caretaker);
    held.add_region(ep_region);
    Some((narrow_ep, file_shared, report, held))
}

/// The caretaker both of the above start, serving `narrow_ep` and reporting once on
/// `caretaker_ready`. Returns the file channel and the caretaker's thread.
#[allow(clippy::too_many_arguments)]
fn spawn_caretaker(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    name: &'static str,
    rights: u64,
    narrow_ep: RendezvousId,
    caretaker_ready: RendezvousId,
) -> Option<(u64, crate::thread::ThreadId)> {
    let (file_ep, file_shared, readiness) = ensure(blk_image, fs_server_image)?;

    let (lo, hi) = filesystem_contract::grant::pack_name(name.as_bytes())
        .expect("a granted name rides in two argument words; this one does not fit");
    let spec = filesystem_contract::grant::spec(name.len(), rights);

    let tid = crate::sched::spawn(move || {
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
    .expect("could not spawn the subtree caretaker");

    // Both handshakes before the client exists, for [`wait_for_caretaker`]'s reason.
    wait_for_service(readiness);
    wait_for_caretaker(caretaker_ready);
    Some((file_shared, tid))
}

/// Wire `grant`: a subtree caretaker and the program confined to it, whose slot 0 is a directory
/// capability for one subtree.
///
/// Returns the confined program's report endpoint and the [`super::holding::Holding`] that hands
/// back the caretaker, the program and every endpoint the grant made. Release it once the last
/// report is read: the caretaker serves until it is killed, so a grant nobody releases is a
/// thread and three endpoints for the rest of the boot.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_granted_dir(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    client_image: &'static [u8],
    grant: DirGrant,
) -> Option<(RendezvousId, super::holding::Holding)> {
    let DirGrant {
        name,
        rights,
        role: client_role,
        arg: client_arg,
        arg2: client_arg2,
        stack_pages,
    } = grant;
    let (narrow_ep, file_shared, report, mut held) =
        narrow_dir_held(blk_image, fs_server_image, caretaker_image, name, rights)?;

    held.add_thread(spawn_fs_client(
        client_image,
        narrow_ep,
        file_shared,
        client_role,
        client_arg,
        client_arg2,
        stack_pages,
        report,
    ));
    Some((report, held))
}
