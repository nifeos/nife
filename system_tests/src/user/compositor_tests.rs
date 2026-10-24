use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use compositor::proto::wlist;
use compositor::{Rect, SCENE, status};
use compositor_service::{
    ROLE_CAPTURE, ROLE_FORGE_CTL, ROLE_INPUT, ROLE_LIE_DAMAGE, ROLE_PROBE_INPUT,
    ROLE_PROBE_NEIGHBOUR, ROLE_PROBE_READ, ROLE_PROBE_SCREEN, ROLE_SMALL_DAMAGE, ROLE_VICTIM,
    Wiring,
};

use super::*;
use crate::arch::exceptions::USER_FAULTS;
use crate::sched;

/// Spin until `cond`, bounded by wall clock rather than by a yield count: since DECISIONS §28 the
/// work a test spawns runs on *other* cores, so a yield on an idle core returns at once and a fixed
/// number of them elapses in no real time. Two seconds is far under the 60 s hang watchdog, so a
/// genuine lost wakeup still fails loudly.
fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
    let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if cond() {
            return true;
        }
        sched::yield_now();
    }
    cond()
}

/// What the last flush asked for, and how many there have been. Written by the display stand-in
/// below; reset by each call to [`kernel_display`]. The compositors left behind by earlier tests are
/// parked in `RECEIVE` and flush nothing, so this is not shared state in any live sense.
static LAST_FLUSH: AtomicU64 = AtomicU64::new(u64::MAX);
static FLUSH_COUNT: AtomicUsize = AtomicUsize::new(0);

/// **A display the kernel serves itself**: the rung-one contract (`INFO`, `FLUSH`) over frames the
/// kernel allocated, with no device behind it. Returns `(display endpoint, screen frames, region)`.
///
/// Here the flush *is* the observation, so a compositor that repainted the screen every frame
/// fails a test rather than merely being slow. The screen and the endpoint both come out of
/// `region`, which the test reclaims last ([`retire_kernel_display`]): that revokes every
/// capability and mapping of the screen (§13 (capability revocation and untyped reclamation)), so
/// the compositor left parked loses it, and it ends the stand-in, whose receive comes back aborted.
pub(super) fn kernel_display() -> (sched::RendezvousId, u64, u64) {
    let frames = graphics_contract::SURFACE_PAGE_FRAMES as u64;
    let region = crate::memory_region::create(frames + 1)
        .expect("no contiguous screen frames for the compositor");
    let (screen, _) =
        crate::memory_region::retype_run(region, frames).expect("no screen run in its region");
    LAST_FLUSH.store(u64::MAX, Ordering::SeqCst);
    FLUSH_COUNT.store(0, Ordering::SeqCst);

    let ep = sched::create_rendezvous_from(region).expect("no display rendezvous");
    sched::spawn(move || {
        loop {
            let m = sched::ipc_receive_cap(ep);
            if sched::take_ipc_aborted() {
                return; // `retire_kernel_display` reclaimed the endpoint: this display is gone
            }
            let (w0, slot) = (m[0], m[1]);
            let crate::cap::Object::Reply(caller) = sched::current_cap(slot)
                .expect("the display stand-in got no reply capability")
                .object
            else {
                panic!("the display stand-in was sent something that was not a CALL");
            };
            let (r0, r1) = match graphics_contract::operation(w0) {
                graphics_contract::display::FLUSH => {
                    LAST_FLUSH.store(graphics_contract::operand(w0), Ordering::SeqCst);
                    FLUSH_COUNT.fetch_add(1, Ordering::SeqCst);
                    (0, 0)
                }
                graphics_contract::display::INFO => (
                    0,
                    graphics_contract::WIDTH as u64 | ((graphics_contract::HEIGHT as u64) << 32),
                ),
                _ => (graphics_contract::EINVAL as u64, 0),
            };
            sched::ipc_reply(caller, [r0, r1]);
            sched::delete_current_cap(slot).expect("consume the one-shot reply");
        }
    })
    .expect("could not spawn the display stand-in");
    (ep, screen, region)
}

/// Give back what [`kernel_display`] took: the screen, unmapped from everyone first, and the
/// endpoint, which ends the stand-in. Last in a test, once nothing it asserts reads the screen.
pub(super) fn retire_kernel_display(region: u64) {
    sched::reclaim_region(region).expect("the kernel display's region did not come back");
}

/// Wait for the compositor's one status message, and check it.
fn wait_for_compositor(w: &Wiring) {
    let [tag, windows, focus, ..] = sched::ipc_receive(w.report);
    assert_eq!(
        tag,
        status::COMP_UP,
        "the compositor did not come up (it reported {tag:#x}; a 0xDEAD_.. word's low byte names \
         the step, see components/src/compositor.rs)",
    );
    assert_eq!(windows, w.n as u64, "the compositor wired the wrong scene");
    assert_eq!(focus, 0, "focus should start on the bottom window");
}

/// Take a `CALL` a client parked on its report endpoint: `(the caller, the reply slot, its word)`.
fn take_call(ep: sched::RendezvousId, want: u64) -> (u64, u64, u64) {
    let m = sched::ipc_receive_cap(ep);
    assert_eq!(
        m[0], want,
        "a client reported {:#x} where {want:#x} was expected (a 0xDEAD_.. word's low byte names \
         the step, see fixtures/src/window.rs)",
        m[0],
    );
    let crate::cap::Object::Reply(caller) = sched::current_cap(m[1])
        .expect("a client's report was not a CALL")
        .object
    else {
        panic!("a client's report carried no reply capability");
    };
    (caller, m[1], m[2])
}

fn release(caller: u64, slot: u64) {
    sched::ipc_reply(caller, [0, 0]);
    sched::delete_current_cap(slot).expect("consume the one-shot reply");
}

/// Take client `i`'s "painted and committed" report and check the digest against the pattern the
/// contract says that window holds. Every honest client sends exactly one of these, so a test that
/// spawns a client owes it a receive: a rendezvous SEND nobody takes leaves the client parked.
fn expect_painted(w: &Wiring, i: usize) {
    let [tag, digest, id, ..] = sched::ipc_receive(w.client_report[i]);
    assert_eq!(
        tag,
        status::WIN_PAINTED,
        "window {i} reported {tag:#x} instead of painting (a 0xDEAD_.. word's low byte names the \
         step, see fixtures/src/window.rs)",
    );
    assert_eq!(
        digest,
        compositor::expected_window_checksum(i),
        "window {i} did not paint its own window's pattern into its own surface",
    );
    assert_eq!(
        id, i as u64,
        "window {i} was told it was window {id}: the compositor published the wrong control page",
    );
}

/// The whole screen equals the picture `compositor` says `committed` windows produce. The kernel's own
/// witness, computed from the contract and read through the direct map, so no process is grading its
/// own homework.
fn assert_screen_is(w: &Wiring, committed: usize) {
    for y in 0..compositor::SCREEN_H {
        for x in 0..compositor::SCREEN_W {
            let got = w.screen_pixel(x, y);
            let want = compositor::expected_screen_pixel(committed, x, y);
            assert_eq!(
                got, want,
                "the composed screen is wrong at ({x},{y}): {got:#010x}, expected {want:#010x} \
                 with {committed} windows committed",
            );
        }
    }
}

/// **A client cannot reach its neighbour's pixels, and cannot read the screen it draws into.**
///
/// The attacker is the honest client's binary with its grants, handed the exact virtual address of
/// its neighbour's pixels (asserted to be the neighbour's frame). It finds its ungranted input slot
/// empty (`NoSuchSlot`); its write faults at that address; the victim's pattern digests the same
/// before and after, read by the kernel and by the victim held in a `CALL`; and a third client
/// faults reading the screen's address. See notes/system-test-modules/compositor-tests.md.
///
/// Falsification: replayable `system_tests/falsifications/user.compositor_tests.a_client_holds_no_capability_for_its_neighbours_pixels_or_the_screen.patch`
#[test_case]
fn a_client_holds_no_capability_for_its_neighbours_pixels_or_the_screen() {
    const ATTACKER: usize = 0;
    const VICTIM: usize = 1;
    const PEEPER: usize = 2;

    let (display, screen, display_region) = kernel_display();
    let w = compositor_service::start(3, 0, display, screen);
    wait_for_compositor(&w);

    // The victim paints and then parks in a CALL, so we can hold it there while it is attacked.
    w.spawn_client(VICTIM, ROLE_VICTIM);
    let (victim, victim_slot, reported) = take_call(w.client_report[VICTIM], status::WIN_PAINTED);
    assert_eq!(
        reported,
        compositor::expected_window_checksum(VICTIM),
        "the victim did not paint its own window's pattern",
    );
    let before = w.client_surface_digest(VICTIM);
    assert_eq!(
        before, reported,
        "the kernel and the victim disagree about the victim's surface before any attack",
    );

    // The attacker's address really is the neighbour's pixels. Without this the attack could
    // degenerate into poking an empty hole and still "pass".
    assert_eq!(
        w.neighbour_probe_phys(ATTACKER),
        w.client[VICTIM] + FRAME_SIZE,
        "the probe address is not the victim's first pixel frame: the allocation is not adjacent, \
         so this test would prove nothing about a neighbour",
    );

    let faults = USER_FAULTS.load(Ordering::Relaxed);
    w.spawn_client(ATTACKER, ROLE_PROBE_INPUT | ROLE_PROBE_NEIGHBOUR);

    let [tag, errno, ..] = sched::ipc_receive(w.client_report[ATTACKER]);
    assert_eq!(
        tag,
        status::WIN_REFUSED,
        "the attacker skipped its input probe"
    );
    assert_eq!(
        errno as i64,
        abi::Error::NoSuchSlot as i64,
        "receiving on an ungranted slot must be NoSuchSlot (-1), 'there is nothing there', not \
         {} (a permission error would mean the authority exists and was withheld)",
        errno as i64,
    );

    // It is an honest client up to the moment it is not: it paints its own window and reports, and
    // only then reaches for its neighbour's.
    expect_painted(&w, ATTACKER);

    let [tag, probe_va, ..] = sched::ipc_receive(w.client_report[ATTACKER]);
    assert_eq!(
        tag,
        status::WIN_PROBING,
        "the attacker never reached its probe"
    );
    assert_eq!(probe_va, w.neighbour_probe_va(ATTACKER));

    assert!(
        wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > faults),
        "a client wrote at {probe_va:#x}, its neighbour's pixels, and was NOT stopped",
    );
    // The exact address, on every ISA: the portable last-fault record keeps `stval` as it keeps
    // `FAR_EL1`.
    assert_eq!(
        crate::arch::exceptions::last_user_fault().map(|(_, addr)| addr),
        Some(probe_va),
        "something faulted, but not at the neighbour's address",
    );
    assert_eq!(
        sched::rendezvous_waiting_senders(w.client_report[ATTACKER]),
        0,
        "the attacker reported past its probe: the write did not fault, so it read back what it \
         wrote into a neighbour's surface (WIN_ESCAPED)",
    );

    // The witness pattern, from the kernel and then from the victim itself.
    assert_eq!(
        w.client_surface_digest(VICTIM),
        before,
        "the victim's pixels changed while it was blocked: the attack landed",
    );
    release(victim, victim_slot);
    let [tag, after, was_before, ..] = sched::ipc_receive(w.client_report[VICTIM]);
    assert_eq!(tag, status::WIN_INTACT);
    assert_eq!(was_before, before, "the victim changed its story");
    assert_eq!(
        after, before,
        "the victim's own read-back of its surface changed after the attack",
    );

    // No ambient display: a client that draws into the screen cannot read it.
    let faults = USER_FAULTS.load(Ordering::Relaxed);
    w.spawn_client(PEEPER, ROLE_PROBE_SCREEN);
    expect_painted(&w, PEEPER);
    let [tag, screen_va, ..] = sched::ipc_receive(w.client_report[PEEPER]);
    assert_eq!(tag, status::WIN_PROBING);
    assert!(
        wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > faults),
        "a window client read the composed screen at {screen_va:#x} and was NOT stopped: the \
         display is ambient",
    );
    assert_eq!(
        crate::arch::exceptions::last_user_fault().map(|(_, addr)| addr),
        Some(screen_va),
        "something faulted, but not at the composed screen's address",
    );
    assert_eq!(
        sched::rendezvous_waiting_senders(w.client_report[PEEPER]),
        0,
        "a client read a pixel of the screen it holds no mapping of (WIN_ESCAPED)",
    );
    retire_kernel_display(display_region);
}

// ================================================================================================
// Confinement claim 25, attacked part by part (milestone 719 (compositor confinement claim 25)).
//
// The test above proves four things in one body and a patch recorded against it reaches one of them:
// the others sit behind the fault wait (the digests), behind a probe that cannot see a read-only
// exposure (the neighbour's pixels, as a confidentiality claim), or behind an address the fixture
// never maps (the screen). Each part below is its own test so each can carry its own replayable
// falsification; `notes/confinement-claims.md` row 25 says which patch reaches which.
// ================================================================================================

/// Receive the next report on `ep`, but bounded: a client whose `CALL` the compositor never answers
/// leaves its next report unsent, and an unbounded `ipc_receive` there is a 60 second watchdog and a
/// diagnostic that says nothing about a compositor.
fn receive_within_bound(ep: sched::RendezvousId, what: &str) -> [u64; 5] {
    let arrived = (0..5).any(|_| wait_for(|| sched::rendezvous_waiting_senders(ep) > 0));
    assert!(arrived, "{what}");
    sched::ipc_receive(ep)
}

/// **Part 1 of claim 25: a client that was granted no input endpoint finds nothing in that slot.**
///
/// A non-focusable client holds two capabilities and slot 2 is empty because the spawner left it
/// empty. Asking to receive there is `NoSuchSlot`, "there is nothing there", and not a refusal from a
/// server that checked a list. The compositor holds a `WRITE` capability for every focusable client's
/// input endpoint and uses exactly one at a time; this is what keeps a client that was never given
/// one from being the receiving end of anything.
///
/// Falsification: replayable `system_tests/falsifications/user.compositor_tests.a_client_with_no_input_grant_finds_nothing_in_the_input_slot.patch`
#[test_case]
fn a_client_with_no_input_grant_finds_nothing_in_the_input_slot() {
    let (display, screen, display_region) = kernel_display();
    let w = compositor_service::start(1, 0, display, screen);
    wait_for_compositor(&w);

    w.spawn_client(0, ROLE_PROBE_INPUT);
    let [tag, errno, ..] = sched::ipc_receive(w.client_report[0]);
    assert_eq!(
        tag,
        status::WIN_REFUSED,
        "the client skipped its input probe"
    );
    assert_eq!(
        errno as i64,
        abi::Error::NoSuchSlot as i64,
        "a non-focusable client's input slot must be empty (NoSuchSlot, -1); {} means something \
         was granted there",
        errno as i64,
    );
    expect_painted(&w, 0);
    retire_kernel_display(display_region);
}

/// **Part 2 of claim 25: no page of a neighbour's is readable, whichever page and whichever way.**
///
/// A write probe cannot tell "not mapped" from "mapped read-only" (both fault at the same address),
/// so this attacker **reads**: the neighbour's control page, then each pixel page, one fresh
/// attacker each because the first fault is fatal. The allocation is asserted contiguous, so each
/// address is the victim's own frame, and each fault must be at exactly the address handed in.
///
/// Falsification: replayable `system_tests/falsifications/user.compositor_tests.a_client_cannot_read_any_page_of_its_neighbours_it_can_name.patch`
#[test_case]
fn a_client_cannot_read_any_page_of_its_neighbours_it_can_name() {
    const ATTACKER: usize = 0;
    const VICTIM: usize = 1;

    let (display, screen, display_region) = kernel_display();
    let w = compositor_service::start(2, 0, display, screen);
    wait_for_compositor(&w);

    w.spawn_client(VICTIM, ROLE_VICTIM);
    let (victim, victim_slot, reported) = take_call(w.client_report[VICTIM], status::WIN_PAINTED);
    assert_eq!(reported, compositor::expected_window_checksum(VICTIM));
    let before = w.client_surface_digest(VICTIM);

    // Page k past the end of the attacker's own surface is the victim's frame k: its control page,
    // then each of its pixel frames. Asserted, not assumed.
    let own_frames = compositor::SCENE[ATTACKER].page_frames() as u64;
    assert_eq!(
        w.client[VICTIM],
        w.client[ATTACKER] + (1 + own_frames) * FRAME_SIZE,
        "the clients are not adjacent, so these probes would prove nothing about a neighbour",
    );
    let victim_frames = compositor::SCENE[VICTIM].page_frames() as u64;
    assert!(
        victim_frames >= 2,
        "a one-frame victim has no first and last pixel page to tell apart"
    );
    let past_own_surface = w.neighbour_probe_va(ATTACKER) - FRAME_SIZE;

    // Every page the victim owns, its control page and each pixel frame, is probed in turn.
    for k in 0..=victim_frames {
        let name = match k {
            0 => "control page",
            1 => "first pixel page",
            _ if k == victim_frames => "last pixel page",
            _ => "middle pixel page",
        };
        let va = past_own_surface + k * FRAME_SIZE;
        let faults = USER_FAULTS.load(Ordering::Relaxed);
        w.spawn_client_probing(ATTACKER, ROLE_PROBE_NEIGHBOUR | ROLE_PROBE_READ, va);
        expect_painted(&w, ATTACKER);
        let [tag, probe_va, ..] = sched::ipc_receive(w.client_report[ATTACKER]);
        assert_eq!(
            tag,
            status::WIN_PROBING,
            "the attacker never reached its {name} probe"
        );
        assert_eq!(probe_va, va);
        assert!(
            wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > faults),
            "a client READ its neighbour's {name} at {va:#x} and was NOT stopped",
        );
        assert_eq!(
            crate::arch::exceptions::last_user_fault().map(|(_, addr)| addr),
            Some(va),
            "something faulted, but not at the neighbour's {name}",
        );
    }

    assert_eq!(
        w.client_surface_digest(VICTIM),
        before,
        "the victim's pixels changed"
    );
    release(victim, victim_slot);
    let [tag, after, ..] = sched::ipc_receive(w.client_report[VICTIM]);
    assert_eq!(tag, status::WIN_INTACT);
    assert_eq!(after, before);
    retire_kernel_display(display_region);
}

/// Spawn a victim in window 1 and park it, then let a liar in window 0 commit every rectangle in the
/// fixture's table of lies. Returns the wiring, the victim's `(caller, slot)` and its surface digest
/// and control page as the kernel saw them **before** the liar ran, and the kernel display's region
/// for the test to retire. Panics, naming the compositor, if the liar's commits stop being
/// answered.
fn lie_to_the_compositor() -> (Wiring, (u64, u64), u64, [u32; 12], u64) {
    const LIAR: usize = 0;
    const VICTIM: usize = 1;

    let (display, screen, display_region) = kernel_display();
    let w = compositor_service::start(2, 0, display, screen);
    wait_for_compositor(&w);

    w.spawn_client(VICTIM, ROLE_VICTIM);
    let (victim, victim_slot, reported) = take_call(w.client_report[VICTIM], status::WIN_PAINTED);
    assert_eq!(reported, compositor::expected_window_checksum(VICTIM));
    let digest = w.client_surface_digest(VICTIM);
    let ctl = w.client_control_words(VICTIM);

    w.spawn_client(LIAR, ROLE_LIE_DAMAGE);
    expect_painted(&w, LIAR);
    let [tag, lies, ..] = receive_within_bound(
        w.client_report[LIAR],
        "the compositor stopped answering a client that committed a rectangle with extreme \
         coordinates: one client took the one doorbell every client shares down with it",
    );
    assert_eq!(
        tag,
        status::WIN_LIED,
        "the liar reported {tag:#x} instead of finishing its lies: 0xdead000000000004 is a commit \
         the compositor answered with an error, which is what a compositor that panicked on a \
         client's rectangle leaves behind (any other 0xDEAD_.. word's low byte names the step, see \
         fixtures/src/window.rs)",
    );
    assert_eq!(lies, 9, "the liar did not commit its whole table");
    (w, (victim, victim_slot), digest, ctl, display_region)
}

/// **Part 3 of claim 25: nothing a client writes in its own control page changes a neighbour's.**
///
/// A client's damage rectangle, the one untrusted input the compositor reads, is set to nine lies,
/// with no fault to wait behind, and the **victim's surface and whole control page** are compared
/// before and after, by the kernel and by the victim. The compositor writes every client's control
/// page (`STATUS`, `ACKED`), so the claim is that the *address* it writes never depends on what a
/// client said; the control page is where a confused deputy would aim.
///
/// Falsification: replayable `system_tests/falsifications/user.compositor_tests.a_lying_damage_rectangle_changes_nothing_of_its_neighbours.patch`
#[test_case]
fn a_lying_damage_rectangle_changes_nothing_of_its_neighbours() {
    const VICTIM: usize = 1;
    let (w, (victim, victim_slot), digest, ctl, display_region) = lie_to_the_compositor();

    assert_eq!(
        w.client_surface_digest(VICTIM),
        digest,
        "a neighbour's lying rectangle changed the victim's pixels",
    );
    assert_eq!(
        w.client_control_words(VICTIM),
        ctl,
        "a neighbour's lying rectangle changed the victim's control page (the compositor wrote \
         into a client it was not answering)",
    );
    release(victim, victim_slot);
    let [tag, after, was, ..] = sched::ipc_receive(w.client_report[VICTIM]);
    assert_eq!(tag, status::WIN_INTACT);
    assert_eq!(was, digest);
    assert_eq!(
        after, digest,
        "the victim's own read-back of its surface changed"
    );
    retire_kernel_display(display_region);
}

/// **Part 3, the other half: a lying rectangle neither stops the compositor nor repaints the screen
/// wrong.**
///
/// The same nine lies, now watched from the screen. Every `CALL` is answered (the liar reports only
/// after the ninth), and the composed screen is exactly the picture the contract gives two committed
/// windows, so a rectangle cannot deface a neighbour's region or the background. The first lie,
/// `x = i32::MAX, w = 1`, overflows an unchecked `Rect::right`, and a compositor that panics on it
/// wedges every client behind the shared doorbell.
///
/// Falsification: replayable `system_tests/falsifications/user.compositor_tests.a_lying_damage_rectangle_cannot_stop_the_compositor_or_misdraw_the_screen.patch`
#[test_case]
fn a_lying_damage_rectangle_cannot_stop_the_compositor_or_misdraw_the_screen() {
    let (w, (victim, victim_slot), _, _, display_region) = lie_to_the_compositor();
    assert_screen_is(&w, 2);
    release(victim, victim_slot);
    retire_kernel_display(display_region);
}

/// **Part 4 of claim 25: a client the spawner granted no screen capability cannot map it, so cannot
/// read it.**
///
/// The original test reads `SCREEN_VA` from a client that never tries to map the screen, so a
/// spawner that leaked the screen capability to every client would leave it green: a grant alone maps
/// nothing. This attacker asks for the mapping exactly as a capture client does, and then reads. With
/// nothing granted the map is refused and the read faults at the unmapped address; with the screen
/// leaked read-only the map succeeds, the read returns a pixel, and the fault wait fails.
///
/// Falsification: replayable `system_tests/falsifications/user.compositor_tests.a_client_granted_no_screen_capability_cannot_map_or_read_the_screen.patch`
#[test_case]
fn a_client_granted_no_screen_capability_cannot_map_or_read_the_screen() {
    let (display, screen, display_region) = kernel_display();
    let w = compositor_service::start(1, 0, display, screen);
    wait_for_compositor(&w);

    let faults = USER_FAULTS.load(Ordering::Relaxed);
    w.spawn_client(0, ROLE_PROBE_SCREEN);
    expect_painted(&w, 0);
    let [tag, screen_va, ..] = sched::ipc_receive(w.client_report[0]);
    assert_eq!(tag, status::WIN_PROBING);
    assert!(
        wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > faults),
        "a client with no screen grant mapped and read the composed screen at {screen_va:#x}: the \
         capability was leaked to a window client",
    );
    assert_eq!(
        crate::arch::exceptions::last_user_fault().map(|(_, addr)| addr),
        Some(screen_va),
        "something faulted, but not at the composed screen's address",
    );
    retire_kernel_display(display_region);
}

/// **A one-window redraw costs one rectangle, not a screen.**
///
/// It observes what was flushed and what was left alone, not time, which under TCG means nothing.
/// The kernel plays the display, so the flush rectangle is compared against
/// `compositor::damage_to_screen` and one commit must be one flush; and between frames the kernel
/// **poisons every screen pixel outside** the expected rectangle, which a full repaint would erase.
/// It also pins startup: the first paint is the background over the whole screen, once, because no
/// client has committed yet.
#[test_case]
fn a_one_window_redraw_costs_one_rectangle_and_not_the_screen() {
    const POISON: u32 = 0xDEAD_BEEF;

    let (display, screen, display_region) = kernel_display();
    let w = compositor_service::start(2, 0, display, screen);
    wait_for_compositor(&w);

    assert_eq!(
        FLUSH_COUNT.load(Ordering::SeqCst),
        1,
        "the compositor's startup should be exactly one flush",
    );
    assert_eq!(
        LAST_FLUSH.load(Ordering::SeqCst),
        graphics_contract::rect(0, 0, compositor::SCREEN_W, compositor::SCREEN_H),
        "the startup flush should be the whole screen",
    );
    assert_screen_is(&w, 0);

    // Window 0 paints, commits, reports, and then parks in a CALL waiting for permission to send
    // its second frame; window 1 paints and commits once.
    w.spawn_client(0, ROLE_SMALL_DAMAGE);
    expect_painted(&w, 0);
    let (client0, slot0, d0) = take_call(w.client_report[0], status::WIN_PAINTED);
    assert_eq!(d0, compositor::expected_window_checksum(0));
    w.spawn_client(1, 0);
    expect_painted(&w, 1);
    assert_screen_is(&w, 2);

    // Poison everything the coming commit must not touch.
    let want = compositor::damage_to_screen(&SCENE[0], compositor::SMALL_DAMAGE);
    assert!(
        !want.is_empty() && want.area() * 20 < Rect::screen().area(),
        "the damage rectangle under test is not small: {want:?}",
    );
    for y in 0..compositor::SCREEN_H {
        for x in 0..compositor::SCREEN_W {
            if !want.contains(x as i32, y as i32) {
                w.poison_screen_pixel(x, y, POISON);
            }
        }
    }

    let flushes = FLUSH_COUNT.load(Ordering::SeqCst);
    release(client0, slot0);
    let [tag, _, seq, ..] = sched::ipc_receive(w.client_report[0]);
    assert_eq!(tag, status::WIN_PAINTED, "the second commit never happened");
    assert_eq!(
        seq, 2,
        "the second commit should be the client's second sequence"
    );

    assert_eq!(
        FLUSH_COUNT.load(Ordering::SeqCst),
        flushes + 1,
        "one commit must be one flush",
    );
    let flushed = graphics_contract::unrect(LAST_FLUSH.load(Ordering::SeqCst));
    assert_eq!(
        flushed,
        (want.x as u32, want.y as u32, want.w, want.h),
        "the flush was not the client's damage placed on the screen",
    );

    for y in 0..compositor::SCREEN_H {
        for x in 0..compositor::SCREEN_W {
            let got = w.screen_pixel(x, y);
            if want.contains(x as i32, y as i32) {
                assert_eq!(
                    got,
                    compositor::expected_screen_pixel(2, x, y),
                    "inside the damage, ({x},{y}) was not recomposited",
                );
            } else {
                assert_eq!(
                    got, POISON,
                    "outside the damage, ({x},{y}) was overwritten: the compositor repainted more \
                     than it was asked to",
                );
            }
        }
    }
    retire_kernel_display(display_region);
}

/// **Input reaches the focused client because that client holds a capability, and focus is the
/// compositor's decision.**
///
/// It separates three things Unix conflates. Who may deliver input: only the input source can write
/// the ring page, so no client can inject a keystroke. Who may receive it: a client holding an
/// input endpoint, so one without cannot be sent anything. Who decides: the compositor, in
/// userspace, witnessed through the window-list page it publishes. After focus moves, the unfocused
/// client must receive nothing, checked with `rendezvous_waiting_senders` rather than a receive
/// that would block forever. See notes/system-test-modules/compositor-tests.md.
#[test_case]
fn input_reaches_only_the_focused_client_and_focus_is_the_compositors_call() {
    let (display, screen, display_region) = kernel_display();
    let mut w = compositor_service::start(2, 2, display, screen);
    wait_for_compositor(&w);

    w.spawn_client(0, ROLE_INPUT);
    w.spawn_client(1, ROLE_INPUT);
    for i in 0..2 {
        expect_painted(&w, i);
    }

    assert_eq!(w.focused(), 0, "focus should start on window 0");
    w.type_bytes(b"a");
    let [tag, byte, count, ..] = sched::ipc_receive(w.client_report[0]);
    assert_eq!(
        tag,
        status::WIN_INPUT,
        "the focused client got no keystroke"
    );
    assert_eq!(byte, b'a' as u64);
    assert_eq!(count, 1);

    // Focus moves, and we read the compositor's decision out of the page it publishes.
    w.type_bytes(&[compositor::proto::FOCUS_NEXT]);
    assert_eq!(
        w.focused(),
        1,
        "the compositor did not move focus, or did not publish that it had",
    );
    let record = mmu::phys_to_virt(w.wlist) + wlist::COUNT;
    // SAFETY: the window-list frame this kernel allocated, through the direct map.
    assert_eq!(unsafe { core::ptr::read_volatile(record as *const u32) }, 2);

    w.type_bytes(b"b");
    let [tag, byte, ..] = sched::ipc_receive(w.client_report[1]);
    assert_eq!(
        tag,
        status::WIN_INPUT,
        "the newly focused client got nothing"
    );
    assert_eq!(byte, b'b' as u64);
    assert_eq!(
        sched::rendezvous_waiting_senders(w.client_report[0]),
        0,
        "the unfocused client received a keystroke that was not routed to it",
    );
    retire_kernel_display(display_region);
}

/// **Focus routes a keystroke into one terminal's grid and not its neighbour's** (milestone 29's
/// text increment meeting rung two).
///
/// Two display terminals, each a compositor client with a window client's authority. An `A` typed
/// while terminal 0 has focus appears in its grid, TAB moves focus, and a `B` appears in terminal
/// 1's; the kernel compares every pixel of the composed screen against two engines it ran itself.
/// Terminal 1 receives because it holds an input endpoint (§33 (the compositor's authority is
/// memory, not messages)), and the compositor cannot tell a terminal from a `window` client. It
/// uses the kernel's display stand-in, because a third `gpu_driver` would race the host's scanout
/// check. See notes/system-test-modules/compositor-tests.md.
#[test_case]
fn focus_routes_a_keystroke_to_one_terminals_grid_and_not_its_neighbours() {
    let (display, screen, display_region) = kernel_display();
    let mut w = compositor_service::start(2, 2, display, screen);
    wait_for_compositor(&w);

    // Both terminals up. Each negotiated its geometry out of the control page the compositor
    // published, so a window whose size the client did not choose is the *normal* case here.
    //
    // **`static mut`, not a stack-local array** (milestone 142): a `Vt` is hundreds of KiB now
    // (`Vt`'s own doc comment), so `[Vt; 2]` is over a stack frame's worth on a 24 KiB kernel thread
    // stack. `Vt::new(1, 1)` is `const fn` over a compile-time-constant argument, so this array
    // initializer is evaluated by the compiler and lands in `.bss`; `reset_to` below retargets each
    // entry to its real, runtime-only-known geometry without ever constructing a `Vt` by value at
    // runtime (see `Vt::reset_to`'s own doc for why that matters here specifically).
    static mut TERMS: [video_terminal::Vt; 2] =
        [video_terminal::Vt::new(1, 1), video_terminal::Vt::new(1, 1)];
    let terms_ptr = &raw mut TERMS;
    // SAFETY: this `#[test_case]` runs once, to completion, before any other test that might
    // declare a same-named function-local static begins (the harness runs test cases
    // sequentially); nothing else in this process can reach `TERMS`.
    let terms: &mut [video_terminal::Vt; 2] = unsafe { &mut *terms_ptr };
    let mut clients = [None, None];
    for i in 0..2 {
        let c = w.spawn_terminal(i);
        let [tag, dims, mode, ..] = sched::ipc_receive(w.client_report[i]);
        assert_eq!(
            tag,
            video_terminal::status::TERM_UP,
            "terminal {i} did not come up (it reported {tag:#x}; a 0xDEAD_.. word's low byte \
             names the step, see components/src/display_terminal.rs)",
        );
        assert_eq!(mode, video_terminal::status::MODE_WINDOW);
        let (cols, rows) = ((dims & 0xffff_ffff) as u32, (dims >> 32) as u32);
        assert_eq!(
            (cols, rows),
            (
                SCENE[i].w / video_terminal::CELL_W,
                SCENE[i].h / video_terminal::CELL_H
            ),
            "terminal {i} sized itself to a grid its window cannot hold",
        );
        terms[i].reset_to(cols, rows);
        video_terminal::script::window(&mut terms[i], i);
        clients[i] = Some(c);
    }

    // Each terminal prints its own banner. Different text per window, so an OPERATION_WRITE delivered
    // to the wrong terminal is a wrong picture rather than a duplicate one.
    for (i, c) in clients.iter().enumerate() {
        c.as_ref()
            .unwrap()
            .print(video_terminal::script::WINDOW_BANNER[i]);
    }

    // Type at the focused terminal, move focus with TAB, type at the next one. `type_bytes`
    // writes the input ring and rings the doorbell, which is exactly what a keyboard driver
    // does; the authority it exercises is the ring mapping, and no client has one.
    assert_eq!(w.focused(), 0, "focus should start on the bottom window");
    w.type_bytes(video_terminal::script::WINDOW_TYPED[0]);
    assert_eq!(w.focused(), 0, "typing must not move focus");
    w.type_bytes(&[compositor::proto::FOCUS_NEXT]);
    assert_eq!(
        w.focused(),
        1,
        "TAB did not move focus: the compositor's own policy decision, published in the window \
         list so it can be witnessed rather than asked for",
    );
    w.type_bytes(video_terminal::script::WINDOW_TYPED[1]);

    // **Scroll both windows.** A window-mode scroll publishes whole-window damage and moves the
    // terminal's own pixels, a paint path only a window pushed past its height reaches. One write
    // per line, so each present scrolls once, and the oracle (`window` above) feeds the same lines.
    for (i, c) in clients.iter().enumerate() {
        for n in 0..video_terminal::script::WINDOW_SCROLL_LINES {
            let mut line = [0u8; 8];
            let len = video_terminal::script::window_scroll_line(i, n, &mut line);
            c.as_ref().unwrap().print(&line[..len]);
        }
    }

    // The picture. Every pixel of the composed screen, against the two engines the kernel ran
    // itself: window content from `video_terminal`, placement and stacking from `compositor`.
    for y in 0..compositor::SCREEN_H {
        for x in 0..compositor::SCREEN_W {
            let got = w.screen_pixel(x, y);
            let want =
                compositor::expected_screen_pixel_with(2, x, y, |i, sx, sy| terms[i].pixel(sx, sy));
            assert_eq!(
                got, want,
                "the composed screen is wrong at ({x},{y}): {got:#010x}, expected {want:#010x}",
            );
        }
    }

    // **And the letters really are distinguishable**, so the comparison above has teeth: a
    // compositor that sent both keystrokes to both terminals, or the wrong one to each, would
    // have to produce a different picture. Asserted rather than assumed, because if the two
    // scripts ever became the same text this test would keep passing while proving nothing.
    // Same reasoning as `TERMS` above: a `static`, retargeted in place, never a `Vt` by value.
    static mut SWAPPED: [video_terminal::Vt; 2] =
        [video_terminal::Vt::new(1, 1), video_terminal::Vt::new(1, 1)];
    let swapped_ptr = &raw mut SWAPPED;
    // SAFETY: see `TERMS` above.
    let swapped: &mut [video_terminal::Vt; 2] = unsafe { &mut *swapped_ptr };
    swapped[0].reset_to(terms[0].cols(), terms[0].rows());
    video_terminal::script::window(&mut swapped[0], 1);
    swapped[1].reset_to(terms[1].cols(), terms[1].rows());
    video_terminal::script::window(&mut swapped[1], 0);
    assert!(
        (0..compositor::SCREEN_H).any(|y| (0..compositor::SCREEN_W).any(|x| {
            compositor::expected_screen_pixel_with(2, x, y, |i, sx, sy| swapped[i].pixel(sx, sy))
                != compositor::expected_screen_pixel_with(2, x, y, |i, sx, sy| {
                    terms[i].pixel(sx, sy)
                })
        })),
        "the two terminals show the same thing: this test cannot tell mis-routed input from \
         correct input",
    );
    retire_kernel_display(display_region);
}

/// **Three clients' surfaces become one screen, and the host confirms it.**
///
/// A real virtio-gpu under rung one's unchanged driver, and four witnesses, because one digest of a
/// compositor's output cannot be trusted: the driver (its one report is the startup frame, so an
/// empty screen is a defined picture), the kernel (every scanout pixel against
/// `compositor::expected_screen_pixel`), a capture client holding a read-only mapping of the
/// screen, and the host's `screendump` through QEMU's monitor, which `cargo xtask` runs because
/// under `-display none` only it sees the device's own surface. The capture client enumerates
/// windows from the published page and faults writing the screen. See
/// notes/system-test-modules/compositor-tests.md.
#[test_case]
fn three_clients_compose_into_one_scanout_and_the_host_sees_it() {
    // **A missing GPU is not always the build-order mistake this test exists to catch.** On the
    // `virt` boards (aarch64, riscv64) and on x86_64's PVH runner a real virtio-gpu-pci function
    // is always wired into the test runner, so `start_driver` returning `None` there really is a
    // build-order bug: PCI is enumerated and the device is simply not on the bus. The one boot
    // without a GPU is x86_64 under OVMF (`helpers/qemu-uefi-x86_64.sh` attaches none), so `None`
    // there is an honest, expected gap rather than a bug. Skipping either way keeps the one thing
    // this test cannot tell apart from a hardware absence out of its own hands.
    let gpu_driver = program("gpu_driver").expect("no gpu_driver program in the initial ramdisk");
    let Some((driver_report, display, screen)) = display_service::start_driver(gpu_driver) else {
        crate::testing::skip!(
            "no virtio-gpu-pci function on the bus: either this kernel enumerated no PCI at all, \
             or (x86_64 under OVMF) the firmware runner attaches no GPU; see \
             design/roadmap/0420-the-rest-of-the-x86-64-fixture-set.md"
        );
    };
    assert!(
        crate::iommu::is_active(),
        "a virtio-gpu is present but the IOMMU is not active: the GPU's pixel reads are \
         unconfined (notes/framebuffer-contract.md)",
    );
    let [tag, geometry, ..] = sched::ipc_receive(driver_report);
    assert_eq!(
        tag,
        graphics_contract::status::UP,
        "the GPU driver did not come up (it reported {tag:#x})",
    );
    assert_eq!(
        geometry,
        graphics_contract::WIDTH as u64 | ((graphics_contract::HEIGHT as u64) << 32),
    );

    let w = compositor_service::start(3, 0, display, screen);
    wait_for_compositor(&w);

    // The driver's own account of the compositor's first frame. Taken here and not later because
    // this is a rendezvous SEND: the driver is parked in it, and a test that spawned clients first
    // would deadlock the driver against the compositor's next flush.
    let [tag, driver_digest, pixels, ..] = sched::ipc_receive(driver_report);
    assert_eq!(
        tag,
        graphics_contract::status::FLUSHED,
        "the driver served no flush"
    );
    assert_eq!(pixels, graphics_contract::PIXELS as u64);
    assert_eq!(
        driver_digest,
        compositor::expected_screen_checksum(0),
        "the frames the device read for the compositor's first flush are not the background: an \
         empty screen must be a defined picture",
    );

    // In order, so that the capture client below is looking at a finished screen.
    for i in 0..2 {
        w.spawn_client(i, 0);
        expect_painted(&w, i);
    }

    let faults = USER_FAULTS.load(Ordering::Relaxed);
    w.spawn_client(2, ROLE_CAPTURE);
    expect_painted(&w, 2);

    // The screenshot, taken through a read-only mapping by a process that holds one.
    let [tag, shot, listed, ..] = sched::ipc_receive(w.client_report[2]);
    assert_eq!(
        tag,
        status::WIN_CAPTURED,
        "the capture client reported nothing"
    );
    assert_eq!(
        shot,
        compositor::expected_screen_checksum(3),
        "a client holding the screen's read-only mapping digested a different screen than the \
         kernel computed from the contract",
    );
    assert_eq!(
        listed,
        3u64 << 32,
        "the window list it enumerated does not say three windows with focus on the first",
    );

    // The kernel's own witness, pixel for pixel.
    assert_screen_is(&w, 3);

    // The other half of a read-only grant: it cannot deface what it may read.
    let [tag, va, which, ..] = sched::ipc_receive(w.client_report[2]);
    assert_eq!(tag, status::WIN_PROBING);
    assert_eq!(which, 1, "the capture client skipped its write probe");
    assert!(
        wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > faults),
        "a client wrote to the screen through a read-only mapping at {va:#x} and was NOT stopped",
    );
    assert_eq!(
        sched::rendezvous_waiting_senders(w.client_report[2]),
        0,
        "the capture client survived writing to the screen (WIN_ESCAPED)",
    );
    // And the write did not land, which is the thing that would otherwise corrupt the picture the
    // host is about to check.
    assert_screen_is(&w, 3);

    // **Hold the picture up for the host.** `cargo xtask` polls QEMU's monitor about every 250 ms
    // while this suite runs, and the next test in the file re-registers the device (which destroys
    // the scanout), so a composed screen that vanished immediately could be missed. Three seconds is
    // an order of magnitude more than the poll needs and is nothing against the per-test ceiling.
    // If the host never sees it, the run fails at the scanout check rather than passing quietly.
    let deadline = crate::arch::timer::now() + 3 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        sched::yield_now();
    }
}

/// **A client that forges its whole control page changes nothing but its own window, and the
/// compositor answers every commit** (an eighth outsider pass at the confinement claim, claim 25).
///
/// The forger rewrites `ID`, `WIDTH`, `HEIGHT`, `STRIDE`, `STATUS`, `ACKED` and `SEQ`, committing
/// after each forge; the compositor trusting any of it back would be the escape. It reports
/// `WIN_FORGED` only if all three commits were answered, and the victim and the screen must be
/// unchanged. See notes/system-test-modules/compositor-tests.md.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_forged_control_page_changes_nothing_and_stops_no_one() {
    const VICTIM: usize = 1;
    const FORGER: usize = 0;
    let (display, screen, display_region) = kernel_display();
    let w = compositor_service::start(2, 0, display, screen);
    wait_for_compositor(&w);

    w.spawn_client(VICTIM, ROLE_VICTIM);
    let (victim, victim_slot, reported) = take_call(w.client_report[VICTIM], status::WIN_PAINTED);
    assert_eq!(reported, compositor::expected_window_checksum(VICTIM));
    let digest = w.client_surface_digest(VICTIM);
    let ctl = w.client_control_words(VICTIM);

    w.spawn_client(FORGER, ROLE_FORGE_CTL);
    expect_painted(&w, FORGER);
    let [tag, answered, ..] = receive_within_bound(
        w.client_report[FORGER],
        "the compositor stopped answering a client that forged its whole control page",
    );
    assert_eq!(
        tag,
        status::WIN_FORGED,
        "the forger reported {tag:#x} instead of finishing: a 0xdead word means the compositor          answered one of the forged commits with an error, and anything else means the client          never finished",
    );
    assert_eq!(answered, 3, "the forger did not complete its three commits");

    assert_eq!(
        w.client_surface_digest(VICTIM),
        digest,
        "a neighbor's forged control page changed the victim's pixels",
    );
    assert_eq!(
        w.client_control_words(VICTIM),
        ctl,
        "a neighbor's forged control page changed the victim's control page",
    );
    assert_screen_is(&w, 2);
    release(victim, victim_slot);
    let [tag, after, was, ..] = sched::ipc_receive(w.client_report[VICTIM]);
    assert_eq!(tag, status::WIN_INTACT);
    assert_eq!(was, digest);
    assert_eq!(after, digest);
    retire_kernel_display(display_region);
}
