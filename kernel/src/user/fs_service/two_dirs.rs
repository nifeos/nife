//! **Two directory grants to one process** (milestone 154).
//!
//! Cut out of `fs_service.rs` unchanged by lane `maintainer/last-three-over-2000`, which left
//! the parent an index (§282 (when a file is split, the parent becomes an index)) and under §266
//! (a Rust source file stays under 2,000 lines).
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// **Two directory grants to one process** (milestone 154,
/// design/roadmap/0154-multi-directory-namespace.md). The endowment question milestone 47's
/// `bind` and milestone 64's `File::open` fork both independently found unbuilt: nothing before
/// this granted a *second* directory capability to one process. `start_granted_dir` starts one
/// caretaker and hands one endpoint; this starts two, for one confined program:
///
/// ```text
///   FS server ──file IPC──► fs_subtree_caretaker A ──narrowed file IPC──►
///            (the image root)      (subtree A, one rights set)          the confined program
///                       └────────► fs_subtree_caretaker B ──narrowed file IPC──►  (slot 0: A
///                                  (subtree B, one rights set)                     slot 1: B
///                                                                                   slot 2: report)
/// ```
///
/// **The spawn-protocol position that says which directory is which is the capability table slot**: the
/// confined program's slot 0 is always [`TwoDirGrant::a`], slot 1 always [`TwoDirGrant::b`].
/// That is the whole of what this milestone decides about the wire, deliberately: a shell-to-progenitor
/// encoding for a *second* `DIRECTORY_GRANT_BIT` grant (extending `grant_plan::spawnproto`'s `GRANT_WORDS`
/// precedent the way a real interactive `bind` eventually will) is a design fork this milestone's
/// own roadmap block leaves to whoever wires this into the shell.
///
/// Both caretakers share the one FS server this boot has: [`narrow_dir`]'s idempotent `ensure`
/// pays for wiring it once and the second call reuses what the first built. Both narrowed
/// endpoints end up mapping the **same** shared file-channel frame at [`FILE_VA_CLIENT`], and
/// that is safe for the reason [`narrow_dir`]'s own doc gives one level narrower: the confined
/// program is one thread of control with at most one `CALL` in flight, so it is never mid-request
/// on both caretakers at once, whichever it happens to be talking to at a given moment owns the
/// page exclusively for the length of that call.
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

    // Both caretakers, fully up (their own readiness drained inside `narrow_dir`) before the
    // confined program exists, for [`wait_for_caretaker`]'s reason: a client that already existed
    // could clobber the shared page while a caretaker was still staging its own startup name in
    // it.
    //
    // **Built once per pair and reused**, since milestone 154 (a process that holds two directory
    // capabilities) added a second test of the same pair. A caretaker never exits, so every pair
    // this builds holds its regions for the rest of the boot, and the suite's region table is a
    // fixed 256 that it already runs close to. The second two-directory test asked for the same
    // two grants and, built fresh, filled the table: a later, unrelated test failed to load as
    // `Unmappable(OutOfPageFrames)`. A caretaker serves one request at a time and keeps no state
    // between clients, so a second client of the same pair is the same grant, not a shared one.
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
