//! Starting a `std` program with a directory capability: whole, narrowed by a caretaker, clocked,
//! or bound to one subtree by the FS server. The spawn they share is `std_spawn`. See notes/std.md.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// The `std::fs` client's heap budget and extra stack. Same magnitudes as the networked std
/// program: it is a full std program (formatting, `Vec`, `String`, `read_to_string`), so it
/// needs the generous heap and the deep stack std's machinery wants.
pub(super) const STD_FS_HEAP_PAGES: u64 = 256;
pub(super) const STD_FS_STACK_PAGES: u64 = std_runtime_contract::STACK_PAGES;

/// Wire the service and endow a std program with a directory capability.
///
/// The std slot convention (heap at 0, stdout at 1) plus the FS-service endpoint at slot 4 and its
/// shared page at the PAL's `FS_PAGE`. Slots 2 and 3 stay empty, so `std::net` can tell it holds no
/// network; without slot 4 every `std::fs` call answers `Unsupported`. Returns `(readiness,
/// stdout)`: the readiness endpoints if this call wired the service, and the program's stdout.
pub fn start_std(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    std_image: &'static [u8],
) -> Option<(Readiness, RendezvousId)> {
    let spawned = start_std_full(blk_image, fs_server_image, std_image)?;
    Some((spawned.readiness, spawned.report))
}

/// What [`start_std_full`] hands back: the service's readiness endpoints if this call wired it, the
/// program's stdout endpoint, the untyped region its heap was drawn from, and the thread it runs as.
pub struct StdSpawn {
    pub readiness: Readiness,
    pub report: RendezvousId,
    pub heap: u64,
    pub thread: crate::thread::ThreadId,
}

/// [`start_std`], also handing back the heap region and the thread, so a caller that knows the
/// program exits can give the memory back.
///
/// A program built outside this tree is present only when somebody built it, so leaving its pages
/// spoken for would fail the frame ledger only for that person; its caller reclaims instead.
pub fn start_std_full(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    std_image: &'static [u8],
) -> Option<StdSpawn> {
    let (file_ep, file_shared, readiness) = ensure(blk_image, fs_server_image)?;
    let report = crate::sched::create_rendezvous();
    let (heap, thread, _stack, _args) =
        spawn_std(file_ep, 0, file_shared, report, std_image, None, None);
    Some(StdSpawn {
        readiness,
        report,
        heap,
        thread,
    })
}

/// What [`start_std_narrowed`] hands back: [`StdSpawn`] without the readiness endpoints, because
/// [`narrow_dir`] has already drained them before the program exists.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct NarrowedStd {
    pub report: RendezvousId,
    pub heap: u64,
    pub thread: crate::thread::ThreadId,
    /// The stack frames [`spawn_std`] allocated. A [`Mapping`] does not give its frame to the
    /// process, so they outlive it unless somebody frees them; see [`NarrowedStd::release`].
    stack: [u64; STD_FS_STACK_PAGES as usize],
    /// The argument page, if the program was given words. Not the process's either.
    args: Option<u64>,
    /// The caretaker in front of it, which [`NarrowedStd::release`] ends.
    caretaker: super::holding::Holding,
}

#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
impl NarrowedStd {
    /// Give back everything the program held that the caller can name: the heap's region and the
    /// stack frames. Panics if the thread is still present, since a frame freed under a running
    /// program is a use-after-free.
    ///
    /// And the caretaker, through its [`super::holding::Holding`]. Returns whether everything
    /// came back, which a test should assert: `false` is a service that outlived its holding.
    pub fn release(self) -> bool {
        assert!(
            !crate::sched::is_thread_present(self.thread),
            "released a std program's memory while it was still running",
        );
        let _ = crate::sched::reclaim_region(self.heap);
        for phys in self.stack.into_iter().chain(self.args) {
            crate::memory::free(page_frames::PageFrame::from_addr(phys));
        }
        self.caretaker.release()
    }
}

/// A std program holding the subtree `name` with `rights`, rather than the image root with all of
/// them: [`start_std_full`]'s spawn with a [`narrow_dir`] caretaker in front.
///
/// `std::fs` inside it sees `name` as `/`, and nothing in the PAL knows the difference. `line`,
/// when given, is a command line as typed at the prompt; it becomes the program's argv through
/// `grant_plan::argv`, as the shell sends it (§170 (how a foreign program is told what to do)).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_std_narrowed(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    std_image: &'static [u8],
    name: &'static str,
    rights: u64,
    line: Option<&[u8]>,
) -> Option<NarrowedStd> {
    start_std_narrowed_clocked(
        blk_image,
        fs_server_image,
        caretaker_image,
        std_image,
        name,
        rights,
        line,
        None,
    )
}

/// [`start_std_narrowed`], and a wall clock: `clock_page` is a clock service's page
/// (`clock_service::Wiring::page_phys`), granted read-only at `std_runtime_contract::CLOCK_SLOT`
/// and mapped at `CLOCK_PAGE`, so `SystemTime::now()` answers instead of refusing.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[allow(clippy::too_many_arguments)]
pub fn start_std_narrowed_clocked(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    std_image: &'static [u8],
    name: &'static str,
    rights: u64,
    line: Option<&[u8]>,
    clock_page: Option<u64>,
) -> Option<NarrowedStd> {
    let (narrow_ep, file_shared, report, caretaker) =
        narrow_dir_held(blk_image, fs_server_image, caretaker_image, name, rights)?;
    let (heap, thread, stack, args) = spawn_std(
        narrow_ep,
        0,
        file_shared,
        report,
        std_image,
        line,
        clock_page,
    );
    Some(NarrowedStd {
        report,
        heap,
        thread,
        stack,
        args,
        caretaker,
    })
}

/// What [`start_std_bound`] hands back: [`NarrowedStd`]'s shape, with the badge and window the
/// grant holds in place of a caretaker.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct BoundStd {
    pub report: RendezvousId,
    pub heap: u64,
    pub thread: crate::thread::ThreadId,
    stack: [u64; STD_FS_STACK_PAGES as usize],
    /// The FS server's endpoint, which the kernel calls to take the grant back.
    file_ep: RendezvousId,
    /// The client window, which is also the badge the grant is bound to, under §230 (badged
    /// endpoint capabilities).
    pub window: u64,
    /// The region the report endpoint was carved from, so one reclaim returns it.
    ep_region: u64,
}

#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
impl BoundStd {
    /// Give back everything, and take the grant back first. `UNBIND` from the kernel's own
    /// unbadged call closes every handle the badge minted and leaves the badge revoked; then the
    /// window returns to the pool, and the heap, stack and endpoint come back as
    /// [`NarrowedStd::release`] returns them. Returns whether the server accepted the `UNBIND`.
    pub fn release(self) -> bool {
        assert!(
            !crate::sched::is_thread_present(self.thread),
            "released a std program's memory while it was still running",
        );
        let unbound = crate::sched::ipc_call(
            self.file_ep,
            [
                filesystem_contract::fs::req(filesystem_contract::fs::UNBIND, 0, 0),
                self.window,
            ],
        )[0] == 0;
        release_window(self.window);
        let _ = crate::sched::reclaim_region(self.heap);
        for phys in self.stack {
            crate::memory::free(page_frames::PageFrame::from_addr(phys));
        }
        let _ = crate::sched::reclaim_region(self.ep_region);
        unbound
    }
}

/// A std program holding one subtree, enforced by the FS server itself (calef's ruling D,
/// 2026-09-27): the same grant as [`start_std_narrowed`], with no caretaker in front.
///
/// The kernel `BIND`s a claimed window's badge to `name` opened with `rights`, so a request with
/// that badge sees the directory as its root (`subtree_scope`). The program gets the badged
/// endpoint and the window. Only an eligible server takes `BIND`. Caretakers stay the default; the
/// progenitor chooses per mount, and this is the test harness's route.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_std_bound(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    std_image: &'static [u8],
    name: &'static str,
    rights: u64,
) -> Option<BoundStd> {
    let grant = bind_subtree(blk_image, fs_server_image, name, rights)?;
    let ep_region = crate::memory_region::create(1).expect("no endpoint region for a bound grant");
    let report = crate::sched::create_rendezvous_from(ep_region).expect("no report endpoint");
    let (heap, thread, stack, _args) = spawn_std(
        grant.file_ep,
        grant.window,
        grant.phys,
        report,
        std_image,
        None,
        None,
    );
    Some(BoundStd {
        report,
        heap,
        thread,
        stack,
        file_ep: grant.file_ep,
        window: grant.window,
        ep_region,
    })
}

/// A subtree bound to a client window's badge, before anyone holds it: the FS server's endpoint,
/// the window (which is the badge), and the window's physical base.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct BoundGrant {
    pub file_ep: RendezvousId,
    pub window: u64,
    pub phys: u64,
    /// Window 0's base, the one an unbadged call stages its name in.
    pub shared: u64,
}

/// Bind `name` under the image root, with `rights`, to a fresh client window's badge:
/// [`start_std_bound`]'s first half, and what a test uses to speak for the badge itself.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn bind_subtree(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    name: &'static str,
    rights: u64,
) -> Option<BoundGrant> {
    use filesystem_contract::fs;
    let (file_ep, shared, readiness) = ensure(blk_image, fs_server_image)?;
    wait_for_service(readiness);
    let (window, phys) = claim_window().expect("no free client window for a bound grant");
    // The binder speaks on its own unbadged channel, as the progenitor would: window 0, badge 0.
    // Never under the grant's badge, which may be a revoked one coming back from the pool (a
    // revoked badge reaches nothing until it is bound again, `subtree_scope::Binding`).
    // SAFETY: window 0's frame, which this module allocated and still owns, via the direct map;
    // the suite runs one test at a time, so no other client is mid-request on it.
    unsafe {
        core::ptr::copy_nonoverlapping(
            name.as_ptr(),
            mmu::phys_to_virt(shared) as *mut u8,
            name.len(),
        );
    }
    let opened = crate::sched::ipc_call(
        file_ep,
        [fs::req(fs::OPENDIR, fs::ROOT, name.len() as u64), rights],
    )[0] as i64;
    assert!(
        opened >= 0,
        "the FS server would not open {name:?} to bind: {opened}"
    );
    let bound =
        crate::sched::ipc_call(file_ep, [fs::req(fs::BIND, opened as u64, 0), window])[0] as i64;
    assert_eq!(bound, 0, "the FS server refused to bind {name:?}");
    Some(BoundGrant {
        file_ep,
        window,
        phys,
        shared,
    })
}
