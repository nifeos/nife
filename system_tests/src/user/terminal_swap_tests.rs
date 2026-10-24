//! **The terminal is replaced live, and the person typing at it loses nothing** (milestone 23 (a
//! capability-routed component OS with live replacement); calef's ruling of 2026-09-27 that a
//! terminal supervisor holds the line editor).
//!
//! A real `terminal_supervisor` builds a real `line_editor` over a fake console that acks every
//! flush. The test plays the input driver and a line-mode reader, as `raw_mode_service` does, and
//! holds the swap endpoint a real boot gives the installer (milestone 198 (a package manager, and
//! the trivial install), not built). It proves the whole §209 (state handoff is an opaque blob over
//! a granted frame, and it is optional) swap: a half-typed line is finished after it, history is
//! recalled after it, and the reader parked across it is handed back and asks again.

//! # The hung terminal
//!
//! A second test stops the fake console's acks, so `line_editor` blocks in its sink's reply and
//! never receives the quiesce: a real hang, a forwarding component wedged behind its dependency. It
//! proves that the supervisor's bound fires, and what the supervisor does then.

use core::sync::atomic::{AtomicBool, Ordering};

use line_editor::component::supervisor as s;
use line_editor::proto;

use super::*;
use crate::cap::{
    Rights, memory_region_root_cap as memory_region_root_capability,
    page_frame_cap as page_frame_capability, rendezvous_cap as rendezvous_capability,
};
use crate::sched;
use crate::user::holding::Holding;

const APP_OUT_LEN: u64 = 0;

const MAX_ELF_PAGES: usize = 64;

/// Stack pages below the one `run` maps. `run` gives a program one page, which a debug build of the
/// ELF loader and the supervision builder overflows; `swapper` meets the same need in its own
/// spawn helper with `PROGENITOR_STACK_PAGES`. These are ordinary `Spawn` windows placed below the stack.
const EXTRA_STACK_PAGES: usize = 7;

/// Every window the supervisor is spawned with: the `line_editor` image, its own image (which it
/// builds its stand-ins from) and the extra stack.
const MAX_WINDOWS: usize = 2 * MAX_ELF_PAGES + EXTRA_STACK_PAGES;

/// The supervisor's view of both images, one mapping per page. A static for `spawn`'s 1 KiB
/// capture limit; see its one use.
static mut ELF_MAPS: [Mapping; MAX_WINDOWS] = [const {
    Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_rodata(),
    }
}; MAX_WINDOWS];

/// The fake console stops acknowledging while this is set, which hangs a `line_editor` in its
/// flush. The request it is holding is dropped, never acknowledged, once it clears.
static CONSOLE_WEDGED: AtomicBool = AtomicBool::new(false);
/// Set by the fake console once it is holding a request it will not acknowledge.
static CONSOLE_HOLDING: AtomicBool = AtomicBool::new(false);

/// The swap endpoint's slot in the supervisor, after the seven terminal objects.
const SWAP_SLOT: u64 = 7;

/// What a reader's report carries in its first word when it is the typist's instead.
const TYPIST: u64 = 0x7479_7069;

fn bytes_call(term: sched::RendezvousId, bytes: &[u8]) {
    let mut w1 = 0u64;
    for (i, &b) in bytes.iter().enumerate() {
        w1 |= (b as u64) << (8 * i);
    }
    sched::ipc_call(
        term,
        [proto::req(proto::OPERATION_BYTES, bytes.len() as u64), w1],
    );
}

fn settle() {
    let deadline = crate::arch::timer::now() + crate::arch::timer::frequency() / 20;
    while crate::arch::timer::now() < deadline {
        sched::yield_now();
    }
}

fn line_at(phys: u64, len: usize) -> [u8; 8] {
    let mut out = [0u8; 8];
    for (i, b) in out.iter_mut().take(len.min(8)).enumerate() {
        // SAFETY: the client input page, mapped into `line_editor` read/write; the read is ordered
        // after the reply that says the line is there.
        *b = unsafe { core::ptr::read_volatile((mmu::phys_to_virt(phys) + i as u64) as *const u8) };
    }
    out
}

/// A reader that reads lines until told to stop, reporting every reply, retries included, with
/// whether the call was answered `Gone` in the third word. A `Gone` is asked again, as a
/// `FLAG_RETRY` is.
fn spawn_reader(term: sched::RendezvousId, report: sched::RendezvousId, lines: usize) {
    sched::spawn(move || {
        let mut done = 0;
        while done < lines {
            let r = sched::ipc_call(
                term,
                [proto::req(proto::OPERATION_READLINE, APP_OUT_LEN), 0],
            );
            let gone = sched::take_ipc_aborted();
            sched::ipc_send(report, [r[0], r[1], gone as u64]);
            if !gone && !proto::is_retry(r[0], r[1]) {
                done += 1;
            }
        }
    })
    .expect("could not spawn the reader");
}

/// A running `terminal_supervisor` over a fake console, as a swap requester sees it.
struct Terminal {
    term: sched::RendezvousId,
    swap: sched::RendezvousId,
    report: sched::RendezvousId,
    app_in: u64,
    budget: u64,
    held: Holding,
}

impl Terminal {
    /// Build the supervisor the way a spawner that passes a swap endpoint must
    /// (`supervisor::OWN_IMAGE_ADDRESS`): both images and the swapping budget.
    fn start() -> Self {
        CONSOLE_WEDGED.store(false, Ordering::Release);
        CONSOLE_HOLDING.store(false, Ordering::Release);
        let supervisor_image =
            program("terminal_supervisor").expect("no terminal_supervisor in the initial ramdisk");
        let editor_image = program("line_editor").expect("no line_editor in the initial ramdisk");

        let ep_region = crate::memory_region::create(8).expect("no endpoint region");
        let term = sched::create_rendezvous_from(ep_region).expect("term");
        let conreq = sched::create_rendezvous_from(ep_region).expect("conreq");
        let conrep = sched::create_rendezvous_from(ep_region).expect("conrep");
        let swap = sched::create_rendezvous_from(ep_region).expect("swap");
        let report = sched::create_rendezvous_from(ep_region).expect("report");
        let budget =
            crate::memory_region::create(s::SWAPPING_BUDGET_PAGES).expect("no supervisor budget");

        // Every frame the test hands out comes from one region, so reclaiming it at the end gives
        // them all back: `raw_mode_service`'s BUGS records what allocating them loose costs the
        // suite.
        let frames = crate::memory_region::create(MAX_WINDOWS as u64 + 3).expect("no frame region");
        let alloc = || crate::memory_region::retype_page(frames).expect("frame region exhausted");
        let (console_page, app_out, app_in) = (alloc(), alloc(), alloc());

        let (maps_ro, windows) = copy_the_images_into_frames(editor_image, supervisor_image, alloc);
        let console_tid = sched::spawn(move || {
            loop {
                sched::ipc_receive(conreq);
                if CONSOLE_WEDGED.load(Ordering::Acquire) {
                    CONSOLE_HOLDING.store(true, Ordering::Release);
                    while CONSOLE_WEDGED.load(Ordering::Acquire) {
                        sched::yield_now();
                    }
                    continue;
                }
                sched::ipc_send(conrep, [0, 0, 0]);
            }
        })
        .expect("could not spawn the fake console");

        let lengths = s::image_lengths(editor_image.len() as u64, supervisor_image.len() as u64, 0)
            .expect("an image is too long for its start argument");
        let supervisor_tid = sched::spawn(move || {
            run(
                supervisor_image,
                Spawn {
                    arg0: 0, // the console's sink shape
                    arg1: lengths,
                    arg2: SWAP_SLOT,
                    grants: &[
                        memory_region_root_capability(budget),    // BUDGET
                        rendezvous_capability(term, Rights::ALL), // TERMINAL
                        rendezvous_capability(conreq, Rights::WRITE.union(Rights::GRANT)), // SINK
                        page_frame_capability(console_page, Rights::ALL), // SINK_PAGE
                        page_frame_capability(app_out, Rights::ALL), // CLIENT_OUT
                        page_frame_capability(app_in, Rights::ALL), // CLIENT_IN
                        rendezvous_capability(conrep, Rights::READ.union(Rights::GRANT)), // SINK_REPLY
                        rendezvous_capability(swap, Rights::READ), // the swap endpoint
                    ],
                    maps: &maps_ro[..windows],
                },
            )
        })
        .expect("could not spawn terminal_supervisor");

        let mut held = Holding::new();
        held.add_thread(console_tid);
        held.add_thread(supervisor_tid);
        held.add_region(ep_region);
        held.add_region(frames);
        Terminal {
            term,
            swap,
            report,
            app_in,
            budget,
            held,
        }
    }

    /// Retire the terminal so every instance's region, and every stand-in's, comes home to the
    /// budget.
    fn stop(self) {
        let stopped = sched::ipc_call(self.swap, [s::STOP, 0]);
        assert_eq!(stopped[0], s::STOPPED);
        settle();
        self.held.release_or_fail("terminal_supervisor");
        sched::reclaim_region(self.budget).expect(
            "the supervisor's budget would not reclaim: an instance's or a stand-in's region never \
             came home",
        );
    }
}

#[test_case]
fn the_terminal_is_replaced_under_a_person_typing_and_nothing_they_typed_is_lost() {
    let t = Terminal::start();
    let (term, report) = (t.term, t.report);

    // A line typed and read before any swap, so there is history to carry.
    spawn_reader(term, report, 1);
    settle();
    bytes_call(term, b"first\r");
    let r = sched::ipc_receive(report);
    assert_eq!(
        r[0], 5,
        "the first line never arrived: the supervisor's line_editor is not serving"
    );

    // Half a line, with a reader parked on it, and then the swap.
    spawn_reader(term, report, 2);
    settle();
    bytes_call(term, b"ec");
    let swapped = sched::ipc_call(t.swap, [s::SWAP, 0]);
    assert_eq!(
        swapped[0],
        s::SWAPPED,
        "the swap did not commit (reply {:#x}, detail {}): the replacement refused the blob, or a \
         healthy line_editor was convicted as hung",
        swapped[0],
        swapped[1],
    );
    assert_eq!(
        swapped[1], 2,
        "the supervisor should have started exactly two instances"
    );
    let retried = sched::ipc_receive(report);
    assert!(
        proto::is_retry(retried[0], retried[1]),
        "the reader parked across the swap was not handed back with FLAG_RETRY",
    );

    the_replacement_finishes_the_line_and_recalls_history(term, report, t.app_in);

    // A second swap works too, which is the control endpoints alternating, and a second bounded
    // quiesce that a deadline left over from the first does not end early.
    let again = sched::ipc_call(t.swap, [s::SWAP, 0]);
    assert_eq!(
        (again[0], again[1]),
        (s::SWAPPED, 3),
        "a second swap did not commit"
    );

    t.stop();
}

/// Falsification: replayable `system_tests/falsifications/user.terminal_swap_tests.a_hung_terminal_is_noticed_within_the_bound_and_replaced_without_its_state.patch`
#[test_case]
fn a_hung_terminal_is_noticed_within_the_bound_and_replaced_without_its_state() {
    let t = Terminal::start();
    let (term, report) = (t.term, t.report);

    // A reader parked on the incumbent, and a typist whose keystroke hangs it: `line_editor`
    // echoes before it replies, so the typist's own call is held too.
    spawn_reader(term, report, 1);
    settle();
    CONSOLE_WEDGED.store(true, Ordering::Release);
    sched::spawn(move || {
        bytes_call(term, b"x");
        let gone = sched::take_ipc_aborted();
        sched::ipc_send(report, [TYPIST, 0, gone as u64]);
    })
    .expect("could not spawn the typist");
    let deadline = crate::arch::timer::now() + crate::arch::timer::frequency();
    while !CONSOLE_HOLDING.load(Ordering::Acquire) {
        assert!(
            crate::arch::timer::now() < deadline,
            "the typist's echo never reached the console, so line_editor did not hang"
        );
        sched::yield_now();
    }

    let swapped = sched::ipc_call(t.swap, [s::SWAP, 0]);
    assert_eq!(
        swapped[0],
        s::REPLACED_HUNG,
        "a line_editor blocked in its sink was not noticed (reply {:#x}, detail {})",
        swapped[0],
        swapped[1],
    );
    // At least the bound, so it was the bound that ended the wait. The deadline fires at the first
    // scheduler tick at or after it, and a loaded host stretches that, so the ceiling is loose: it
    // only says the wait ended on the deadline and not on something long after it.
    let waited = swapped[1];
    crate::println!("terminal_swap: a hung line_editor was convicted after {waited} ms");
    assert!(
        (s::QUIESCE_BOUND_MS..4 * s::QUIESCE_BOUND_MS).contains(&waited),
        "the supervisor waited {waited} ms, against a bound of {} ms",
        s::QUIESCE_BOUND_MS,
    );

    // Both callers the hung instance held were answered `Gone` when its region was destroyed.
    let (mut reader_gone, mut typist_gone) = (false, false);
    for _ in 0..2 {
        let r = sched::ipc_receive(report);
        match r[0] {
            TYPIST => typist_gone = r[2] == 1,
            _ => reader_gone = r[2] == 1,
        }
    }
    assert!(
        reader_gone,
        "the reader parked on the hung line_editor was not answered Gone"
    );
    assert!(
        typist_gone,
        "the typist held by the hung line_editor was not answered Gone"
    );

    // The replacement serves, and serves empty: the keystroke typed into the hung instance is lost,
    // which is what replacing without the state costs.
    CONSOLE_WEDGED.store(false, Ordering::Release);
    bytes_call(term, b"ok\r");
    let r = sched::ipc_receive(report);
    assert_eq!(
        r[0], 2,
        "the reader's second attempt was not served by the replacement (reply {:#x}, {:#x})",
        r[0], r[1],
    );
    assert_eq!(&line_at(t.app_in, 2)[..2], b"ok");

    t.stop();
}

/// Copy the `line_editor` image and the supervisor's own into fresh frames from `alloc`, and add
/// the extra stack pages. Returns the mappings the supervisor is spawned with and how many of them
/// are in use.
fn copy_the_images_into_frames(
    editor_image: &[u8],
    own_image: &[u8],
    alloc: impl Fn() -> u64,
) -> (&'static [Mapping; MAX_WINDOWS], usize) {
    // The supervisor's spawn closure borrows this static because a closure that captured the array
    // itself would be over the 1 KiB `spawn` allows.
    let maps_p = &raw mut ELF_MAPS;
    // SAFETY: the two tests here are its only users, the suite runs them one after the other, and
    // each one's supervisor has exited before the next writes it.
    let maps: &'static mut [Mapping; MAX_WINDOWS] = unsafe { &mut *maps_p };
    let mut windows = 0;
    // Each image copied into fresh frames the supervisor maps at its VA: the kernel's stand-in for
    // a spawner's `blobs` copy.
    for (image, va, what) in [
        (editor_image, s::ELF_VA, "line_editor"),
        (own_image, s::OWN_IMAGE_ADDRESS, "terminal_supervisor"),
    ] {
        assert!(
            image.len().div_ceil(FRAME_SIZE as usize) <= MAX_ELF_PAGES,
            "{what}'s stripped image outgrew the test's mapping"
        );
        for (i, chunk) in image.chunks(FRAME_SIZE as usize).enumerate() {
            let phys = alloc();
            // SAFETY: a frame just allocated for this copy and not yet mapped anywhere else.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    chunk.as_ptr(),
                    mmu::phys_to_virt(phys) as *mut u8,
                    chunk.len(),
                );
            }
            maps[windows] = Mapping {
                va: va + i as u64 * FRAME_SIZE,
                phys,
                flags: Flags::user_rodata(),
            };
            windows += 1;
        }
    }

    for k in 0..EXTRA_STACK_PAGES {
        maps[windows] = Mapping {
            va: USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE,
            phys: alloc(),
            flags: Flags::user_data(),
        };
        windows += 1;
    }
    let maps_ro: &'static [Mapping; MAX_WINDOWS] = maps;
    (maps_ro, windows)
}

/// After the first swap: the replacement finishes the line the incumbent started, and recalls
/// the line typed before the swap.
fn the_replacement_finishes_the_line_and_recalls_history(
    term: sched::RendezvousId,
    report: sched::RendezvousId,
    app_in: u64,
) {
    // The replacement finishes the line the incumbent started.
    bytes_call(term, b"ho\r");
    let r = sched::ipc_receive(report);
    assert_eq!(
        r[0], 4,
        "the line typed across the swap came back {} bytes long",
        r[0]
    );
    assert_eq!(
        &line_at(app_in, 4)[..4],
        b"echo",
        "the half-typed line did not survive the swap"
    );

    // And recalls what the incumbent was told before it: Up twice is the pre-swap line.
    settle();
    bytes_call(term, b"\x1b[A\x1b[A\r");
    let r = sched::ipc_receive(report);
    assert_eq!(
        r[0], 5,
        "history typed before the swap is not recallable after it"
    );
    assert_eq!(&line_at(app_in, 5)[..5], b"first");
}
