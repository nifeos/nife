//! A set grant: a caretaker that answers only for the names in a set, the set mapped read-only into
//! it on a frame of its own. See notes/glob-grant.md.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// Where the nameset caretaker expects its read-only name-set page
/// (`components/src/fs_nameset_caretaker.rs`'s `SET_VA`).
const SET_VA_CARETAKER: u64 = 0x0000_0000_0070_0000;

/// Stack pages the caretaker gets beyond the one `run` maps: the progenitor's
/// `CARETAKER_STACK_PAGES` (4), so this wiring runs it on the stack it gets in a booted system.
/// One page is not enough. The caretaker copies the whole set, `nameset::BYTES`, onto its stack
/// before serving, and its first request ran off the page once `program_entry!` put a frame
/// under `start`.
const CARETAKER_EXTRA_STACK: usize = 3;

/// A set grant: [`DirGrant`]'s shape, narrowed to the names in a set within one directory.
///
/// A set does not fit in `START` registers, so it is encoded into a frame of its own, mapped
/// read-only into the caretaker before it is spawned. No client can reach that frame, so it needs
/// none of [`wait_for_caretaker`]'s ordering. See notes/fs-server/kernel-wiring.md.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct SetGrant<'a> {
    /// The directory the caretaker descends into, one component under the image root. The set's
    /// names are the names *in* it that the grant designates.
    pub dir: &'static str,
    /// The set, as `(name, is_dir)`: what the shell's expansion produced. At most
    /// `filesystem_contract::nameset::MAX_NAMES` of them, and an over-long set is a panic here because the
    /// shell refuses it at the prompt (`grant_plan::Refusal::TooManyNames`), so one arriving means
    /// the wiring built a grant no command line could have expressed.
    pub names: &'a [(&'a [u8], bool)],
    /// The [`filesystem_contract::dir`] rights the caretaker asks for on its descent.
    pub rights: u64,
    /// The confined program's three `START` words. `rm` is started with a grant's spec and two
    /// name words rather than a role and a number; see [`DirGrant`].
    pub role: u64,
    pub arg: u64,
    pub arg2: u64,
    /// Stack pages beyond the one `run` maps.
    pub stack_pages: usize,
}

/// Wire `grant` and the program confined to it. Returns the program's report endpoint and what
/// ends the grant, or `None` when no RedoxFS disk is attached.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_granted_set(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    client_image: &'static [u8],
    grant: SetGrant<'_>,
) -> Option<(RendezvousId, super::holding::Holding)> {
    let SetGrant {
        dir,
        names,
        rights,
        role: client_role,
        arg: client_arg,
        arg2: client_arg2,
        stack_pages,
    } = grant;
    let (file_ep, file_shared, readiness) = ensure(blk_image, fs_server_image)?;
    // From one region the returned holding reclaims, as [`start_granted`]'s are. The set's own
    // frame is not in it: the caretaker maps that page, and a mapped page is freed only after its
    // reader is gone (`Holding::add_region_after_death`'s reason), which this wiring never had.
    let ep_region =
        crate::memory_region::create(GRANT_ENDPOINT_PAGES).expect("no endpoint region for a grant");
    let endpoint =
        || crate::sched::create_rendezvous_from(ep_region).expect("no endpoint for a grant");
    let narrow_ep = endpoint();
    let caretaker_ready = endpoint();
    let report = endpoint();

    // The set, encoded into its own frame before anything can see it. `encode` refuses rather
    // than truncating, and a truncated set would be a capability nobody planned.
    let set_phys = page_frame();
    let mut encoded = [0u8; filesystem_contract::nameset::BYTES];
    let n = filesystem_contract::nameset::encode(names, &mut encoded)
        .expect("this set does not fit one grant, so no command line could have named it");
    // SAFETY: a fresh frame of FRAME_SIZE bytes reachable through the direct map, and `n` is at
    // most `nameset::BYTES`, which is far smaller.
    unsafe {
        core::ptr::copy_nonoverlapping(encoded.as_ptr(), mmu::phys_to_virt(set_phys) as *mut u8, n);
    };

    let (lo, hi) = filesystem_contract::grant::pack_name(dir.as_bytes())
        .expect("a granted directory's name rides in two argument words; this one does not fit");
    let spec = filesystem_contract::grant::spec(dir.len(), rights);

    let caretaker = crate::sched::spawn(move || {
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; 2 + CARETAKER_EXTRA_STACK];
        maps[0] = Mapping {
            va: FILE_VA_CLIENT,
            phys: file_shared,
            flags: Flags::user_data(),
        };
        // Read-only: a writable set would let the caretaker widen its own grant.
        maps[1] = Mapping {
            va: SET_VA_CARETAKER,
            phys: set_phys,
            flags: Flags::user_rodata(),
        };
        for (k, m) in maps[2..].iter_mut().enumerate() {
            m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
            m.phys = page_frame();
        }
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
                maps: &maps,
            },
        )
    })
    .expect("could not spawn the nameset caretaker");

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
        client_arg2,
        stack_pages,
        report,
    ));
    Some((report, held))
}
