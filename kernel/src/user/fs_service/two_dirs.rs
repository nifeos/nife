//! Two directory grants to one process. See notes/fs-server/kernel-wiring.md.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// The last pair [`start_granted_two_dirs`] built: both grants as asked for, both narrowed
/// endpoints, and the shared file-channel frame. See the reuse note there.
#[allow(clippy::type_complexity)]
static TWO_DIRS: spin::Mutex<
    Option<(
        (&'static str, u64),
        (&'static str, u64),
        RendezvousId,
        RendezvousId,
        u64,
    )>,
> = spin::Mutex::new(None);

/// Two directory grants for one confined program: slot 0 is always [`TwoDirGrant::a`] and slot 1
/// always [`TwoDirGrant::b`], which is the whole of what tells them apart.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct TwoDirGrant {
    /// The first grant: the directory (one component under the image root) and the
    /// [`filesystem_contract::dir`] rights, delivered at the confined program's capability table slot 0.
    pub a: (&'static str, u64),
    /// The second grant, at slot 1.
    pub b: (&'static str, u64),
    /// The confined program's `arg0` (its role) and `arg1`.
    pub role: u64,
    pub arg: u64,
    /// Stack pages beyond the one `run` maps, for a confined program that needs them.
    pub stack_pages: usize,
}

/// Wire two subtree caretakers on this boot's one FS server and the program that holds both, and
/// return its report endpoint. `None` when no RedoxFS disk is attached.
///
/// Both caretakers map the same file-channel frame, which is safe because the program has at most
/// one `CALL` in flight. See notes/fs-server/kernel-wiring.md.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_granted_two_dirs(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    client_image: &'static [u8],
    grant: TwoDirGrant,
) -> Option<RendezvousId> {
    let TwoDirGrant {
        a,
        b,
        role,
        arg,
        stack_pages,
    } = grant;
    assert!(
        stack_pages <= CLIENT_EXTRA_STACK,
        "a two-directory client asked for more stack than this wiring maps",
    );

    // Both caretakers are up before the program exists, for [`wait_for_caretaker`]'s reason. A pair
    // is built once and reused: a caretaker never exits and the region table is a fixed 256, and a
    // caretaker keeps no state between clients, so a second client gets the same grant.
    let cached = *TWO_DIRS.lock();
    let (narrow_a, narrow_b, file_shared) = match cached {
        Some((ca, cb, na, nb, shared)) if ca == a && cb == b => (na, nb, shared),
        _ => {
            let (narrow_a, file_shared) =
                narrow_dir(blk_image, fs_server_image, caretaker_image, a.0, a.1)?;
            let (narrow_b, file_shared_b) =
                narrow_dir(blk_image, fs_server_image, caretaker_image, b.0, b.1)?;
            debug_assert_eq!(
                file_shared, file_shared_b,
                "one boot has one FS server, so both caretakers must share its one file channel",
            );
            *TWO_DIRS.lock() = Some((a, b, narrow_a, narrow_b, file_shared));
            (narrow_a, narrow_b, file_shared)
        }
    };

    let report = crate::sched::create_rendezvous();
    crate::sched::spawn(move || {
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; FILE_PAGES + CLIENT_EXTRA_STACK];
        let n = map_channel(&mut maps, FILE_VA_CLIENT, file_shared, FILE_PAGES);
        for (k, m) in maps[n..n + stack_pages].iter_mut().enumerate() {
            m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
            m.phys = page_frame();
        }
        run(
            client_image,
            Spawn {
                arg0: role,
                arg1: arg,
                arg2: 0,
                grants: &[
                    rendezvous_cap(narrow_a, Rights::WRITE), // slot 0: CALL grant A's caretaker
                    rendezvous_cap(narrow_b, Rights::WRITE), // slot 1: CALL grant B's caretaker
                    rendezvous_cap(report, Rights::WRITE),   // slot 2: report to the kernel
                ],
                maps: &maps[..n + stack_pages],
            },
        )
    })
    .expect("could not spawn the two-directory client");
    Some(report)
}
