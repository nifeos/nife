//! **`display_terminal` is replaced live, and the screen and its history do not change**
//! (milestone 23 (a capability-routed component OS with live replacement); calef's rulings of
//! 2026-10-19 (UTC): a dedicated swap reserve, "everything" handed off, the spawner supervises).
//!
//! A real `terminal_supervisor` in display mode builds a real `display_terminal` on the kernel's
//! display stand-in, and a `line_editor` printing through it. The test plays the application on the
//! terminal's endpoint, holds the supervisor's swap endpoint, and holds the machine's one swap
//! reserve, which on a real boot would be the installer's (milestone 694 (the installer asks the
//! terminal to swap), not built).
//!
//! It is a kernel test rather than a userspace one because its two witnesses are memory no
//! program holds: the scanout, read through the direct map and compared pixel for pixel with an
//! engine the kernel runs itself, and the handoff blob, read out of the reserve after the swap and
//! compared byte for byte with the blob that engine writes. The pixels show the screen survived;
//! the blob shows the scrollback, the cursor, the rendition and the parser's state did, which no
//! pixel on a live view can.
//!
//! Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23.

use line_editor::component::supervisor as s;
use video_terminal::component as display;
use video_terminal::handoff::HANDOFF_BYTES;
use video_terminal::script::{COLS, ROWS};

use super::*;
use crate::cap::{
    Rights, memory_region_root_cap as memory_region_root_capability,
    page_frame_cap as page_frame_capability, page_frame_run_cap as page_frame_run_capability,
    page_frame_run_len, rendezvous_cap as rendezvous_capability,
};
use crate::sched;
use crate::sched::ipc_send_cap as send_capability;
use crate::user::holding::Holding;

/// `display_terminal`'s image is the largest, about 220 pages: its `Vt` is initialized to
/// blank cells, which are not zero bytes, so the grid is in `.data` rather than `.bss`.
const MAX_ELF_PAGES: usize = 256;
/// Stack pages below the one `run` maps; `terminal_swap_tests`' constant, for its reason.
const EXTRA_STACK_PAGES: usize = 7;
const MAX_WINDOWS: usize = 3 * MAX_ELF_PAGES + EXTRA_STACK_PAGES;
/// The swap endpoint's slot in the supervisor, after the eight terminal objects.
const SWAP_SLOT: u64 = 8;

/// The supervisor's view of its three images, a static for `spawn`'s 1 KiB capture limit.
static mut ELF_MAPS: [Mapping; MAX_WINDOWS] = [const {
    Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_rodata(),
    }
}; MAX_WINDOWS];

/// The engine the kernel runs beside the terminal, fed every byte the terminal is: what the screen
/// and the blob must be. A static because a `Vt` is far past a kernel stack.
static mut EXPECT: video_terminal::Vt = video_terminal::Vt::new(COLS, ROWS);
/// The blob [`EXPECT`] writes, compared with the one in the reserve.
static mut EXPECT_BLOB: [u8; HANDOFF_BYTES] = [0; HANDOFF_BYTES];

/// **The machine's one swap reserve**, as its holder sees it (calef's "D", 2026-10-19 UTC). A lend
/// splits the whole of it, so a second lend while one is out finds nothing to split: that refusal
/// is what serializes swaps, and it lasts exactly until the lent region is destroyed.
struct Reserve(u64);

impl Reserve {
    fn lend(&self) -> Option<u64> {
        crate::memory_region::split(self.0, s::SWAP_RESERVE_PAGES)
    }
}

/// The scanout's witness, the screen half of `display_service::TerminalWiring` with the terminal's
/// endpoint and output page this test made.
fn wiring(term: sched::RendezvousId, out: u64, surface: u64) -> display_service::TerminalWiring {
    display_service::TerminalWiring {
        // Not read by `print` or `assert_screen_is`; the supervisor takes the terminal's reports.
        driver_report: term,
        term_report: term,
        term,
        out,
        surface,
        surface_frames: graphics_contract::SURFACE_PAGE_FRAMES as u64,
        held: Holding::new(),
    }
}

/// Print `bytes` to the terminal and feed them to the expectation.
fn both(w: &display_service::TerminalWiring, expect: &mut video_terminal::Vt, bytes: &[u8]) {
    w.print(bytes);
    expect.feed(bytes);
}

/// `count` numbered lines in eight colors, a page at a time.
fn lines(
    w: &display_service::TerminalWiring,
    expect: &mut video_terminal::Vt,
    from: u32,
    count: u32,
) {
    let mut page = [0u8; 2048];
    let mut n = 0;
    for i in from..from + count {
        let mut line = [0u8; 32];
        let mut cursor = Cursor {
            buf: &mut line,
            at: 0,
        };
        let _ = core::fmt::write(
            &mut cursor,
            format_args!("\x1b[3{}mline {i}\x1b[0m\r\n", i % 8),
        );
        let len = cursor.at;
        if n + len > page.len() {
            both(w, expect, &page[..n]);
            n = 0;
        }
        page[n..n + len].copy_from_slice(&line[..len]);
        n += len;
    }
    both(w, expect, &page[..n]);
}

struct Cursor<'a> {
    buf: &'a mut [u8],
    at: usize,
}

impl core::fmt::Write for Cursor<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.at + s.len();
        self.buf
            .get_mut(self.at..end)
            .ok_or(core::fmt::Error)?
            .copy_from_slice(s.as_bytes());
        self.at = end;
        Ok(())
    }
}

/// **Lend the reserve, swap `display_terminal`, and check what the swap left**: the answer, every
/// pixel, the blob in the reserve, and the reserve coming back whole when it is destroyed.
fn swap_and_check(
    swap: sched::RendezvousId,
    lent: u64,
    w: &display_service::TerminalWiring,
    expect: &video_terminal::Vt,
    started: u64,
    what: &str,
) {
    // With `GRANT`, because the supervisor endows the replacement with a map budget it splits
    // from the reserve. Lending it whole is safe for the reason a lend is: the `DESTROY` below
    // takes back everything made from it, wherever it was delegated.
    send_capability(swap, s::LEND, memory_region_root_capability(lent), 0);
    let swapped = sched::ipc_call(swap, [s::SWAP, s::CHILD_DISPLAY_TERMINAL]);
    assert_eq!(
        (swapped[0], swapped[1]),
        (s::SWAPPED, started),
        "{what}: the display swap did not commit (reply {:#x}, detail {}); {started} instances \
         were expected, one in the reserve and one home again",
        swapped[0],
        swapped[1],
    );
    w.assert_screen_is(expect, what);

    // The run is the first thing the supervisor retypes from the reserve, so it sits at the
    // lent region's base, holding the blob the last instance wrote on its way home.
    let (base, _) = crate::memory_region::region_bounds(lent).expect("the lent region went stale");
    let blob_p = &raw mut EXPECT_BLOB;
    // SAFETY: this test is the only user of `EXPECT_BLOB`, and the suite runs one test at a time.
    let want = unsafe { &mut *blob_p };
    let len = expect
        .write_handoff(want)
        .expect("the expectation's blob does not fit");
    for (i, &b) in want[..len].iter().enumerate() {
        // SAFETY: inside the lent region, which this test made and has not yet destroyed.
        let got =
            unsafe { core::ptr::read_volatile((mmu::phys_to_virt(base) + i as u64) as *const u8) };
        assert_eq!(
            got,
            b,
            "{what}: the blob carried home differs from the engine's at byte {i} of {len}: the \
             swap lost state (the scrollback starts at byte {})",
            4 * (33 + 3 * (COLS * ROWS) as usize),
        );
    }

    // The supervisor gave everything back before answering, so the reserve destroys whole, which
    // revokes the run from the instance that absorbed it.
    sched::reclaim_region(lent).unwrap_or_else(|()| {
        panic!("{what}: the lent reserve would not reclaim: the supervisor kept part of it")
    });
}

/// A running `terminal_supervisor` holding `display_terminal` and `line_editor`, as the swap's
/// requester sees it.
struct Rig {
    sink: sched::RendezvousId,
    sink_page: u64,
    swap: sched::RendezvousId,
    budget: u64,
    held: Holding,
}

impl Rig {
    /// Build the supervisor in display mode with all three images, on the driver's display endpoint
    /// and scanout.
    fn start(display_ep: sched::RendezvousId, surface: u64) -> Self {
        let supervisor_image =
            program("terminal_supervisor").expect("no terminal_supervisor in the initial ramdisk");
        let editor_image = program("line_editor").expect("no line_editor in the initial ramdisk");
        let display_image =
            program("display_terminal").expect("no display_terminal in the initial ramdisk");

        let ep_region = crate::memory_region::create(4).expect("no endpoint region");
        let term = sched::create_rendezvous_from(ep_region).expect("term");
        let sink = sched::create_rendezvous_from(ep_region).expect("sink");
        let swap = sched::create_rendezvous_from(ep_region).expect("swap");
        let budget =
            crate::memory_region::create(s::SWAPPING_BUDGET_PAGES + display::SUPERVISOR_PAGES)
                .expect("no supervisor budget");
        let image_pages: usize = [editor_image, supervisor_image, display_image]
            .iter()
            .map(|i| i.len().div_ceil(FRAME_SIZE as usize))
            .sum();
        let frames = crate::memory_region::create((image_pages + EXTRA_STACK_PAGES + 3) as u64)
            .expect("no frame region");
        let alloc = || crate::memory_region::retype_page(frames).expect("frame region exhausted");
        let (sink_page, app_out, app_in) = (alloc(), alloc(), alloc());
        let windows = copy_the_images_into_frames(
            [
                (editor_image, s::ELF_VA),
                (supervisor_image, s::OWN_IMAGE_ADDRESS),
                (display_image, s::DISPLAY_IMAGE_ADDRESS),
            ],
            alloc,
        );
        let maps_p = &raw const ELF_MAPS;
        // SAFETY: written above, and read only by the supervisor's spawn below.
        let maps: &'static [Mapping; MAX_WINDOWS] = unsafe { &*maps_p };

        let lengths = s::image_lengths(
            editor_image.len() as u64,
            supervisor_image.len() as u64,
            display_image.len() as u64,
        )
        .expect("an image is too long for its start argument");
        let surface_run = page_frame_run_len(graphics_contract::SURFACE_PAGE_FRAMES as u64);
        let supervisor_tid = sched::spawn(move || {
            run(
                supervisor_image,
                Spawn {
                    arg0: 1, // `line_editor`'s display sink
                    arg1: lengths,
                    arg2: SWAP_SLOT,
                    grants: &[
                        memory_region_root_capability(budget),         // BUDGET
                        rendezvous_capability(term, Rights::ALL),      // TERMINAL
                        rendezvous_capability(sink, Rights::ALL), // SINK: display_terminal serves it
                        page_frame_capability(sink_page, Rights::ALL), // SINK_PAGE
                        page_frame_capability(app_out, Rights::ALL), // CLIENT_OUT
                        page_frame_capability(app_in, Rights::ALL), // CLIENT_IN
                        rendezvous_capability(display_ep, Rights::WRITE.union(Rights::GRANT)), // PRESENT
                        page_frame_run_capability(surface, surface_run, Rights::ALL), // SURFACE
                        rendezvous_capability(swap, Rights::READ), // the swap endpoint
                    ],
                    maps: &maps[..windows],
                },
            )
        })
        .expect("could not spawn terminal_supervisor");
        let mut held = Holding::new();
        held.add_thread(supervisor_tid);
        held.add_region(ep_region);
        held.add_region(frames);
        Rig {
            sink,
            sink_page,
            swap,
            budget,
            held,
        }
    }

    /// Retire the supervisor, so every region it built comes home to its budget.
    fn stop(self) {
        let stopped = sched::ipc_call(self.swap, [s::STOP, 0]);
        assert_eq!(stopped[0], s::STOPPED);
        let deadline = crate::arch::timer::now() + crate::arch::timer::frequency() / 20;
        while crate::arch::timer::now() < deadline {
            sched::yield_now();
        }
        self.held.release_or_fail("terminal_supervisor");
        sched::reclaim_region(self.budget).expect(
            "the supervisor's budget would not reclaim: an instance's region never came home",
        );
    }
}

/// Falsification: replayable `system_tests/falsifications/user.terminal_display_swap_tests.the_display_terminal_is_replaced_live_and_its_screen_and_history_survive.patch`
#[test_case]
fn the_display_terminal_is_replaced_live_and_its_screen_and_history_survive() {
    // The kernel's display stand-in rather than the GPU: every `start_driver` takes a virtio slot
    // for the rest of the boot (`kernel::virtio`'s `MAX_DEVICES`), and the pixels this test grades
    // are the surface's, which the stand-in's frames hold as a driver's would.
    let (display_ep, surface, display_region) = super::compositor_tests::kernel_display();
    let rig = Rig::start(display_ep, surface);
    let (swap, sink, sink_page) = (rig.swap, rig.sink, rig.sink_page);

    let expect_p = &raw mut EXPECT;
    // SAFETY: this test is the only user of `EXPECT`, and the suite runs one test at a time.
    let expect: &mut video_terminal::Vt = unsafe { &mut *expect_p };
    expect.reset_to(COLS, ROWS);
    let w = wiring(sink, sink_page, surface);
    // An empty write is answered only once the terminal is serving, which is after its first,
    // blank, paint: the stand-in sends no report a driver would.
    w.print(b"");
    w.assert_screen_is(
        expect,
        "the supervised terminal before anything was printed",
    );

    // Everything a swap must carry: a hundred and twenty colored lines, so seventy-eight rows of
    // history; a bold underlined rendition left on; and a 24-bit color cut after its `2;`.
    lines(&w, expect, 0, 120);
    both(&w, expect, b"\x1b[1;4mstill bold ");
    both(&w, expect, b"\x1b[38;2;");
    w.assert_screen_is(expect, "before the swap");

    // Without a reserve, a display swap is refused before anything is quiesced, and says so.
    let refused = sched::ipc_call(swap, [s::SWAP, s::CHILD_DISPLAY_TERMINAL]);
    assert_eq!(
        refused[0],
        s::NEEDS_RESERVE,
        "a display swap with no reserve lent was not refused (reply {:#x})",
        refused[0],
    );

    let reserve =
        Reserve(crate::memory_region::create(s::SWAP_RESERVE_PAGES).expect("no swap reserve"));
    let first = reserve.lend().expect("the swap reserve could not be lent");
    // A second swap while the first holds the reserve is refused, and retryably: it is asked
    // again once the first is answered, below.
    assert!(
        reserve.lend().is_none(),
        "the reserve was lent twice at once, so two swaps could run together"
    );
    swap_and_check(swap, first, &w, expect, 3, "the first swap");

    // The terminal carries on where the incumbent stopped: the cut sequence finishes as
    // red-orange, the rendition is still on, and more lines push the history ring on.
    both(&w, expect, b"255;64;0mR\x1b[0m\r\n");
    lines(&w, expect, 120, 30);
    w.assert_screen_is(expect, "output after the swap");

    let second = reserve
        .lend()
        .expect("the swap refused while the first held the reserve could not be retried");
    swap_and_check(swap, second, &w, expect, 5, "the retried swap");

    rig.stop();
    sched::reclaim_region(reserve.0).expect("the swap reserve would not reclaim");
    super::compositor_tests::retire_kernel_display(display_region);
}

/// **A supervised `display_terminal` takes a quiesce from its supervisor and from nobody else**
/// (§232 (the `line_editor` swap contract): "only a supervisor sends it"). The test holds the
/// endpoint `display_terminal` serves unbadged, as the `line_editor` printing through it does, and
/// quiesces it. Honored, that would stop the screen on a control endpoint only the supervisor
/// speaks on, and a supervisor that did not ask never says `CTL_RESUME`. So it is refused, the
/// terminal goes on drawing, and the supervisor's own quiesce, sent through its badged copy when
/// the test stops it, still works.
///
/// Falsification: replayable `system_tests/falsifications/user.terminal_display_swap_tests.a_supervised_display_terminal_refuses_a_quiesce_from_anyone_but_its_supervisor.patch`
#[test_case]
fn a_supervised_display_terminal_refuses_a_quiesce_from_anyone_but_its_supervisor() {
    let (display_ep, surface, display_region) = super::compositor_tests::kernel_display();
    let rig = Rig::start(display_ep, surface);
    let (sink, sink_page) = (rig.sink, rig.sink_page);

    let expect_p = &raw mut EXPECT;
    // SAFETY: this test is the only user of `EXPECT` while it runs, and the suite runs one test
    // at a time.
    let expect: &mut video_terminal::Vt = unsafe { &mut *expect_p };
    expect.reset_to(COLS, ROWS);
    let w = wiring(sink, sink_page, surface);
    both(&w, expect, b"before ");

    use line_editor::proto::{self, BAD_REQUEST};
    let quiesce = [proto::req(proto::OPERATION_QUIESCE, 0), 0];
    // Each answer is checked before the next call: a terminal that honored one would never
    // answer the next, and the run would end at the watchdog rather than at the assertion.
    for (badge, what) in [
        (0, "an unbadged holder"),
        (proto::RAW_ONLY_BADGE, "a raw-only holder"),
    ] {
        let q = match badge {
            0 => sched::ipc_call(sink, quiesce),
            b => sched::ipc_call_badged(sink, quiesce, b),
        };
        assert_eq!(
            q[0], BAD_REQUEST,
            "{what} quiesced a supervised display_terminal (reply {:#x}): any application could \
             freeze the screen until a CTL_RESUME nobody sends",
            q[0],
        );
    }

    // The supervisor's copy carries a quiesce and nothing else: it cannot print.
    let printed = sched::ipc_call_badged(
        sink,
        [proto::req(proto::OPERATION_WRITE, 0), 0],
        proto::SUPERVISOR_BADGE,
    );
    assert_eq!(
        printed[0], BAD_REQUEST,
        "the supervisor's badged copy was served OPERATION_WRITE"
    );

    both(&w, expect, b"and after");
    w.assert_screen_is(
        expect,
        "a display_terminal that refused a stranger's quiesce",
    );

    rig.stop();
    super::compositor_tests::retire_kernel_display(display_region);
}

/// Copy each image into fresh frames from `alloc` at its address in the supervisor, and add the
/// extra stack pages, into [`ELF_MAPS`]. Returns how many of its entries are in use.
fn copy_the_images_into_frames(images: [(&[u8], u64); 3], alloc: impl Fn() -> u64) -> usize {
    let maps_p = &raw mut ELF_MAPS;
    // SAFETY: this test is its only user, and the supervisor it is written for has not started.
    let maps: &mut [Mapping; MAX_WINDOWS] = unsafe { &mut *maps_p };
    let mut windows = 0;
    for (image, va) in images {
        assert!(
            image.len().div_ceil(FRAME_SIZE as usize) <= MAX_ELF_PAGES,
            "an image of {} bytes outgrew the test's mapping",
            image.len()
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
    windows
}
