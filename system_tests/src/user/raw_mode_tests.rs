//! **The raw-keystroke primitive** (milestone 169 (`kilo`, the smallest real text editor, as the forcing function for raw terminal input)): `OPERATION_RAWMODE` and `OPERATION_READRAW`, proved against
//! a real `line_editor` process the way this tree proves everything, with a client that asks for
//! raw mode, sends real keystrokes, and gets them back unbuffered.
//!
//! `raw_mode_service::start` plays both the input driver and the application on one `TERM`
//! endpoint (the contract already allows this: DECISIONS §21's endpoint-only naming means nobody
//! on the other end can tell who is calling) and a fake console that never reads its own shared
//! page, so a test can sentinel-fill that page and check it untouched: the same witness-page
//! discipline `c_seam`'s confiner tests use, applied to "nothing was echoed" instead of "nothing
//! outside a grant changed".

use raw_mode_service as svc;

use super::*;
use crate::sched;

const OPERATION_ON: u64 = 1;
const OPERATION_OFF: u64 = 0;

/// Sentinel byte a test fills a shared page with before an exchange it expects to leave that page
/// untouched. Not `0`, so a page that was merely zeroed (e.g. by `line_editor`'s own boot state)
/// cannot be mistaken for a page a test actually observed staying put.
const SENTINEL: u8 = 0xaa;

fn fill(phys: u64, byte: u8, len: usize) {
    let base = mmu::phys_to_virt(phys);
    for i in 0..len {
        // SAFETY: `phys` is a frame this test allocated and is the only writer while the
        // exchange under test runs.
        unsafe { core::ptr::write_volatile((base + i as u64) as *mut u8, byte) };
    }
}

fn read(phys: u64, len: usize) -> [u8; 8] {
    let base = mmu::phys_to_virt(phys);
    let mut out = [0u8; 8];
    for (i, b) in out.iter_mut().enumerate().take(len) {
        // SAFETY: same frame, read after the exchange's reply has already come back, so any
        // write `line_editor` was going to make already happened (the reply is ordered after it:
        // `con.flush()` runs before `reply` in every arm that touches `Con`).
        *b = unsafe { core::ptr::read_volatile((base + i as u64) as *const u8) };
    }
    out
}

fn rawmode(term: sched::RendezvousId, on: bool) {
    let w0 = line_editor::proto::req(
        line_editor::proto::OPERATION_RAWMODE,
        if on { OPERATION_ON } else { OPERATION_OFF },
    );
    let r = sched::ipc_call(term, [w0, 0]);
    assert_eq!(r[0], 0, "OPERATION_RAWMODE did not reply 0");
}

/// Send `bytes` (at most 8) as the input driver would, packed exactly as `OPERATION_BYTES` requires.
fn send_bytes(term: sched::RendezvousId, bytes: &[u8]) -> [u64; 3] {
    assert!(bytes.len() <= 8);
    let mut w1 = 0u64;
    for (i, &b) in bytes.iter().enumerate() {
        w1 |= (b as u64) << (8 * i);
    }
    let w0 = line_editor::proto::req(line_editor::proto::OPERATION_BYTES, bytes.len() as u64);
    sched::ipc_call(term, [w0, w1])
}

fn read_raw(term: sched::RendezvousId) -> (usize, [u8; 8]) {
    let w0 = line_editor::proto::req(line_editor::proto::OPERATION_READRAW, 0);
    let r = sched::ipc_call(term, [w0, 0]);
    (r[0] as usize, r[1].to_le_bytes())
}

/// **Raw mode suppresses echo, and `OPERATION_READRAW` hands back exactly what was sent, unbuffered.**
///
/// This is the milestone's central claim, and it is proven both ways so it cannot be vacuous.
/// The negative half (raw mode: the console page is untouched) would also pass if `line_editor`
/// simply never echoed anything, ever, so the positive control (canonical mode: the same bytes
/// through the same page DO land there) is what proves the sentinel methodology actually detects
/// an echo when there is one.
#[test_case]
fn raw_mode_suppresses_echo_and_delivers_bytes_unbuffered() {
    let (w, held) = svc::start();

    rawmode(w.term, true);
    fill(w.console_phys, SENTINEL, 8);
    let r = send_bytes(w.term, b"hi");
    assert_eq!(
        r[0], 0,
        "a raw OPERATION_BYTES must reply 0, like the canonical path"
    );
    assert_eq!(
        &read(w.console_phys, 2)[..2],
        &[SENTINEL, SENTINEL],
        "raw mode must not echo: the console page moved",
    );

    let (n, bytes) = read_raw(w.term);
    assert_eq!(n, 2, "OPERATION_READRAW returned the wrong byte count");
    assert_eq!(
        &bytes[..2],
        b"hi",
        "OPERATION_READRAW did not return exactly what was sent"
    );

    // The positive control: same bytes, canonical mode, same page. If this did not move, the
    // negative check above would have been meaningless.
    rawmode(w.term, false);
    fill(w.console_phys, SENTINEL, 8);
    send_bytes(w.term, b"hi");
    assert_eq!(
        &read(w.console_phys, 2)[..2],
        b"hi",
        "canonical mode must still echo: the sentinel methodology itself is broken if this fails",
    );
    held.release_or_fail("raw_mode_service");
}

/// **Raw mode delivers control bytes literally; it does not interpret them as editing commands.**
///
/// `^C` is the sharpest witness available: in the line discipline it is intercepted, discards the
/// line, prints `"^C\r\n"`, and bumps `OPERATION_INTRCOUNT`'s counter (DECISIONS §24 (interrupting the foreground process: two-tier, shell-held, no new kernel surface)). None of that may
/// happen in raw mode: the byte `0x03` must reach the application exactly as `kilo` needs it to
/// (real kilo binds its own quit key, not SIGINT, because termios raw mode disables `ISIG`).
#[test_case]
fn raw_mode_delivers_control_bytes_literally() {
    let (w, held) = svc::start();
    rawmode(w.term, true);
    fill(w.console_phys, SENTINEL, 8);

    send_bytes(w.term, &[0x03]);
    assert_eq!(
        read(w.console_phys, 8)[0],
        SENTINEL,
        "^C must not print anything in raw mode",
    );
    let (n, bytes) = read_raw(w.term);
    assert_eq!(n, 1);
    assert_eq!(
        bytes[0], 0x03,
        "^C must arrive as a literal byte in raw mode"
    );

    let intr_w0 = line_editor::proto::req(line_editor::proto::OPERATION_INTRCOUNT, 0);
    let r = sched::ipc_call(w.term, [intr_w0, 0]);
    assert_eq!(
        r[0], 0,
        "raw mode's ^C must not bump the line discipline's interrupt counter"
    );

    // An escape sequence a VT arrow key sends is three more bytes the line discipline would
    // otherwise consume whole and echo nothing for; raw mode must hand back all three raw.
    fill(w.console_phys, SENTINEL, 8);
    send_bytes(w.term, b"\x1b[D");
    assert_eq!(
        read(w.console_phys, 8)[0],
        SENTINEL,
        "no echo for an escape sequence either"
    );
    let (n, bytes) = read_raw(w.term);
    assert_eq!(n, 3);
    assert_eq!(
        &bytes[..3],
        b"\x1b[D",
        "the escape sequence must arrive byte for byte, unparsed"
    );
    held.release_or_fail("raw_mode_service");
}

/// **The two input models refuse each other.** `OPERATION_READLINE` while raw mode is on, and
/// `OPERATION_READRAW` while it is off, are both protocol violations refused with `BAD_REQUEST`, the
/// exact refusal a second concurrent `OPERATION_READLINE` already gets: a client that mixed them up
/// fails fast rather than hanging on a reply that will never come.
#[test_case]
fn the_two_input_models_refuse_each_other() {
    let (w, held) = svc::start();

    let readraw_w0 = line_editor::proto::req(line_editor::proto::OPERATION_READRAW, 0);
    let r = sched::ipc_call(w.term, [readraw_w0, 0]);
    assert_eq!(
        r[0],
        line_editor::proto::BAD_REQUEST,
        "OPERATION_READRAW while raw mode is off must be refused",
    );

    rawmode(w.term, true);
    let readline_w0 = line_editor::proto::req(line_editor::proto::OPERATION_READLINE, 0);
    let r = sched::ipc_call(w.term, [readline_w0, 0]);
    assert_eq!(
        r[0],
        line_editor::proto::BAD_REQUEST,
        "OPERATION_READLINE while raw mode is on must be refused",
    );
    held.release_or_fail("raw_mode_service");
}

/// **A read parked before any byte arrives is still answered correctly once one does.**
///
/// This drives the actual parking path `OPERATION_READLINE` already relies on (`deliver`'s twin,
/// `deliver_raw`): a second thread calls `OPERATION_READRAW` first, with the raw queue empty, and blocks
/// in the kernel exactly as a `CALL` with no waiting server does. The main thread then sends the
/// byte. Both orderings the scheduler could actually choose (the reader's call reaching
/// `line_editor` before or after the byte does) produce the same correct report, so this test is
/// not racy on its assertion; the bounded wait before sending is only to bias real runs toward
/// exercising the parked case rather than the immediate one, which the two tests above already
/// cover.
#[test_case]
fn a_raw_read_parked_before_data_arrives_still_gets_it() {
    let (w, held) = svc::start();
    rawmode(w.term, true);

    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let report = sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let term = w.term;
    sched::spawn(move || {
        let (n, bytes) = read_raw(term);
        sched::ipc_send(report, [n as u64, u64::from_le_bytes(bytes), 0]);
    })
    .expect("could not spawn the raw reader");

    // Bias toward the parked interleaving: give the reader thread real wall-clock time to reach
    // and block in its CALL before this thread sends anything. Not load-bearing for correctness
    // (see the doc comment above), only for which code path a given run actually exercises.
    let deadline = crate::arch::timer::now() + crate::arch::timer::frequency() / 20; // 50ms
    while crate::arch::timer::now() < deadline {
        sched::yield_now();
    }

    send_bytes(w.term, &[0x42]);

    let [n, packed, ..] = sched::ipc_receive(report);
    assert_eq!(n, 1, "the parked reader did not get exactly one byte");
    assert_eq!(
        packed.to_le_bytes()[0],
        0x42,
        "the parked reader got the wrong byte"
    );
    held.release_or_fail("raw_mode_service");
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **Switching mode abandons the line in progress, in both directions**, so a session can never
/// resume half a line typed under the mode it just left.
///
/// Entering raw mode with an `OPERATION_READLINE` parked must fail that read rather than hang it
/// forever (raw mode would never generate the `Event::Line` it is waiting for). Leaving raw mode
/// with an `OPERATION_READRAW` parked must fail that one the same way.
#[test_case]
fn switching_mode_abandons_a_parked_read_of_the_other_kind() {
    let (w, held) = svc::start();

    // A parked OPERATION_READLINE, abandoned by entering raw mode.
    let endpoints = crate::memory_region::create(2).expect("no endpoint region");
    let report = sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let term = w.term;
    sched::spawn(move || {
        let w0 = line_editor::proto::req(line_editor::proto::OPERATION_READLINE, 0);
        let r = sched::ipc_call(term, [w0, 0]);
        sched::ipc_send(report, [r[0], r[1], 0]);
    })
    .expect("could not spawn the readline waiter");
    let deadline = crate::arch::timer::now() + crate::arch::timer::frequency() / 20;
    while crate::arch::timer::now() < deadline {
        sched::yield_now();
    }
    rawmode(w.term, true);
    let [r0, ..] = sched::ipc_receive(report);
    assert_eq!(
        r0,
        line_editor::proto::BAD_REQUEST,
        "entering raw mode must fail a parked OPERATION_READLINE rather than hang it",
    );

    // A parked OPERATION_READRAW, abandoned by leaving raw mode.
    let report2 = sched::create_rendezvous_from(endpoints).expect("no report2 rendezvous");
    let term2 = w.term;
    sched::spawn(move || {
        let (n, bytes) = read_raw(term2);
        sched::ipc_send(report2, [n as u64, u64::from_le_bytes(bytes), 0]);
    })
    .expect("could not spawn the readraw waiter");
    let deadline = crate::arch::timer::now() + crate::arch::timer::frequency() / 20;
    while crate::arch::timer::now() < deadline {
        sched::yield_now();
    }
    rawmode(w.term, false);
    let [n2, ..] = sched::ipc_receive(report2);
    assert_eq!(
        n2,
        line_editor::proto::BAD_REQUEST,
        "leaving raw mode must fail a parked OPERATION_READRAW rather than hang it",
    );
    held.release_or_fail("raw_mode_service");
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **`OPERATION_WRITE` is not raw-mode-gated, and `OPERATION_READLINE` works normally once raw mode is off
/// again.** `kilo` depends on the first (it redraws the whole screen with `OPERATION_WRITE` on every
/// keystroke, in raw mode, the entire time it runs); the second is the check that raw mode is a
/// mode switch and not a one-way door: a terminal that left it broken behind would fail every
/// program written before this milestone.
#[test_case]
fn operation_write_ignores_raw_mode_and_operation_readline_survives_a_round_trip() {
    let (w, held) = svc::start();

    rawmode(w.term, true);
    let text = b"kilo redraws through OPERATION_WRITE while raw mode is on";
    let base = mmu::phys_to_virt(w.app_out_phys);
    for (i, &b) in text.iter().enumerate() {
        // SAFETY: `app_out_phys` is the frame `line_editor` maps read-only at its own APP_OUT_VA;
        // this test is the only writer, and no request naming it is in flight yet.
        unsafe { core::ptr::write_volatile((base + i as u64) as *mut u8, b) };
    }
    let w0 = line_editor::proto::req(line_editor::proto::OPERATION_WRITE, text.len() as u64);
    let r = sched::ipc_call(w.term, [w0, 0]);
    assert_eq!(
        r[0],
        text.len() as u64,
        "OPERATION_WRITE must work while raw mode is on"
    );
    assert_eq!(
        &read(w.console_phys, 8)[..4],
        b"kilo",
        "the written text must reach the console page exactly as line mode would print it",
    );

    // Leaving raw mode restores OPERATION_READLINE. Type a line's worth of raw bytes first (as the input
    // driver would, before anyone asked to read a line -- exactly the type-ahead case) and confirm
    // it comes back through OPERATION_READLINE once a read is posted, which only works if turning raw
    // mode off actually re-enabled the line discipline rather than leaving OPERATION_BYTES stuck.
    rawmode(w.term, false);
    send_bytes(w.term, b"hi\r");
    let w0 = line_editor::proto::req(line_editor::proto::OPERATION_READLINE, 0);
    let r = sched::ipc_call(w.term, [w0, 0]);
    assert_eq!(
        r[0], 2,
        "OPERATION_READLINE must return the type-ahead line's length"
    );
    let base = mmu::phys_to_virt(w.app_in_phys);
    let got: [u8; 2] = core::array::from_fn(|i| {
        // SAFETY: `app_in_phys` is the frame `line_editor` maps read/write at its own APP_IN_VA,
        // and the OPERATION_READLINE reply above is ordered after `line_editor` wrote it there.
        unsafe { core::ptr::read_volatile((base + i as u64) as *const u8) }
    });
    assert_eq!(
        &got, b"hi",
        "OPERATION_READLINE did not deliver the line line_editor assembled"
    );
    held.release_or_fail("raw_mode_service");
}

/// **What one keystroke costs when the application edits its own line** (DECISIONS §227 (how Tab
/// reaches the shell) option D, milestone 47 (navigation and naming)): a measurement, printed as a
/// `measure:` line, beside the one fact it asserts.
///
/// Cooked, a keystroke is one `OPERATION_BYTES` from the input driver, and the terminal echoes it
/// itself. Raw adds the shell's `OPERATION_READRAW` and an `OPERATION_WRITE` of the echo, two more
/// round trips per burst. Both loops run from kernel threads against the real `line_editor`, so the
/// difference excludes an EL0 trap; under TCG the absolute numbers are the emulator's and the ratio
/// is the reading. The assertion: a client that edits its own line must still be seen typing.
#[test_case]
fn a_keystroke_edited_by_the_client_costs_two_more_round_trips() {
    const N: u64 = 200;
    let (w, held) = svc::start();

    // Cooked: type a character and rub it out, so the line never fills.
    let t0 = crate::arch::timer::now();
    for i in 0..N {
        send_bytes(w.term, if i % 2 == 0 { b"a" } else { &[0x7f] });
    }
    let cooked = crate::arch::timer::now() - t0;

    rawmode(w.term, true);
    let app_out = mmu::phys_to_virt(w.app_out_phys);
    fill(w.console_phys, SENTINEL, 8);
    let t0 = crate::arch::timer::now();
    for _ in 0..N {
        send_bytes(w.term, b"a");
        let (n, bytes) = read_raw(w.term);
        assert_eq!(n, 1);
        // SAFETY: the app-output frame this test allocated; `line_editor` maps it read-only and
        // reads it only inside the `OPERATION_WRITE` below.
        unsafe { core::ptr::write_volatile(app_out as *mut u8, bytes[0]) };
        let w0 = line_editor::proto::req(line_editor::proto::OPERATION_WRITE, 1);
        let r = sched::ipc_call(w.term, [w0, 0]);
        assert_eq!(r[0], 1, "OPERATION_WRITE did not consume the echoed byte");
    }
    let raw = crate::arch::timer::now() - t0;
    assert_eq!(
        read(w.console_phys, 1)[0],
        b'a',
        "the client's echo never reached the console"
    );
    rawmode(w.term, false);

    let hz = crate::arch::timer::frequency();
    let ns = |ticks: u64| (ticks as u128 * 1_000_000_000 / hz.max(1) as u128 / N as u128) as u64;
    crate::println!(
        "measure: keystroke cooked {} ns, raw with client echo {} ns (per keystroke, {} each)",
        ns(cooked),
        ns(raw),
        N
    );
    held.release_or_fail("raw_mode_service");
}

/// **A badged copy of the terminal endpoint reads keystrokes and cannot type them** (milestone 709
/// (a graphical terminal session on the no-keyboard arm holds only the raw half of the boot
/// discipline), fatal risk 7's confinement claim).
///
/// A keyboardless `graphical_terminal` session reads the boot discipline through a copy badged
/// [`line_editor::proto::RAW_ONLY_BADGE`], served only `OPERATION_RAWMODE` and `OPERATION_READRAW`.
/// Its cooked `ls\r` is refused, the unbadged reader gets exactly the driver's later line, other
/// requests are refused, and the raw half still works, so a terminal refusing everything fails.
///
/// Falsification: replayable `system_tests/falsifications/user.raw_mode_tests.a_badged_copy_of_the_terminal_reads_keystrokes_and_cannot_type_them.patch`
#[test_case]
fn a_badged_copy_of_the_terminal_reads_keystrokes_and_cannot_type_them() {
    use line_editor::proto::{self, BAD_REQUEST, RAW_ONLY_BADGE};
    let (w, held) = svc::start();
    // The kernel's own delivery of a badged `CALL`: the badge lands in `x3` of the server's
    // `RECEIVE_CAP`, as it would for a user program holding the copy.
    let badged = |operation: u64, len: u64, w1: u64| {
        sched::ipc_call_badged(w.term, [proto::req(operation, len), w1], RAW_ONLY_BADGE)
    };
    let pack = |bytes: &[u8]| {
        bytes
            .iter()
            .enumerate()
            .fold(0u64, |acc, (i, &b)| acc | (b as u64) << (8 * i))
    };

    // The attack: a line typed through the badged copy, in cooked mode, where the shell reads.
    let r = badged(proto::OPERATION_BYTES, 3, pack(b"ls\r"));
    assert_eq!(
        r[0], BAD_REQUEST,
        "a badged holder's OPERATION_BYTES was served (r0 = {}): it can type a line the boot shell reads",
        r[0],
    );

    // The witness: the shell's next line is the input driver's, not the injected one.
    send_bytes(w.term, b"ok\r");
    let r = sched::ipc_call(w.term, [proto::req(proto::OPERATION_READLINE, 0), 0]);
    assert_eq!(
        r[0], 2,
        "the shell's OPERATION_READLINE got a line of the wrong length"
    );
    let base = mmu::phys_to_virt(w.app_in_phys);
    let got: [u8; 2] = core::array::from_fn(|i| {
        // SAFETY: `app_in_phys` is the frame `line_editor` maps read/write at its own APP_IN_VA,
        // and the OPERATION_READLINE reply above is ordered after `line_editor` wrote it there.
        unsafe { core::ptr::read_volatile((base + i as u64) as *const u8) }
    });
    assert_eq!(
        &got, b"ok",
        "the shell read a line the badged holder typed, not the input driver's"
    );

    // Everything else off the raw half. OPERATION_READLINE last among the reads: the queue is empty, so
    // a served one would park this thread rather than fail, and the asserts above would already
    // have gone red first.
    for (operation, what) in [
        (proto::OPERATION_PRINT, "OPERATION_PRINT"),
        (proto::OPERATION_WRITE, "OPERATION_WRITE"),
        (proto::OPERATION_INTRCOUNT, "OPERATION_INTRCOUNT"),
        (proto::OPERATION_QUIESCE, "OPERATION_QUIESCE"),
        (proto::OPERATION_READLINE, "OPERATION_READLINE"),
    ] {
        let r = badged(operation, 1, pack(b"x"));
        assert_eq!(r[0], BAD_REQUEST, "a badged holder's {what} was served");
    }

    // The raw half, through the same badged copy: the session's whole use of it.
    assert_eq!(
        badged(proto::OPERATION_RAWMODE, 1, 0)[0],
        0,
        "a badged OPERATION_RAWMODE was refused"
    );
    send_bytes(w.term, b"q");
    let r = badged(proto::OPERATION_READRAW, 0, 0);
    assert_eq!(
        (r[0], r[1].to_le_bytes()[0]),
        (1, b'q'),
        "a badged OPERATION_READRAW did not get the keystroke the input driver sent"
    );
    assert_eq!(
        badged(proto::OPERATION_RAWMODE, 0, 0)[0],
        0,
        "a badged cooked OPERATION_RAWMODE was refused"
    );
    held.release_or_fail("raw_mode_service");
}

/// **A copy badged as standard input reads the typed line and does nothing else** (milestone 892
/// (a std program at the prompt holds the network and a resolver); calef, 2026-10-19 (UTC), on
/// #2405: "a"). `jig install` reads the owner's yes through such a copy, so it must not be able to
/// type a line the shell runs, print, or switch modes, and the unbadged surface must not serve its
/// read, or the shell's next command could be taken through it.
/// The line is typed by the input driver as the owner types it, longer than eight bytes, so the
/// reads drain it in two replies and the `\n` that ends it. Then the attacks: every other request
/// on the copy is refused, and an unbadged `OPERATION_READ_LINE_BYTES` is refused too.
///
/// Falsification: replayable `system_tests/falsifications/user.raw_mode_tests.a_copy_badged_as_stdin_reads_the_typed_line_and_nothing_else.patch`
#[test_case]
fn a_copy_badged_as_stdin_reads_the_typed_line_and_nothing_else() {
    use line_editor::proto::{self, BAD_REQUEST, STDIN_BADGE};
    let (w, held) = svc::start();
    let stdin = |operation: u64, len: u64, w1: u64| {
        sched::ipc_call_badged(w.term, [proto::req(operation, len), w1], STDIN_BADGE)
    };

    // The owner types "yes, ok!!" and Enter, in two bursts as a driver delivers them.
    send_bytes(w.term, b"yes, ok!");
    send_bytes(w.term, b"!\r");
    let r = stdin(proto::OPERATION_READ_LINE_BYTES, 0, 0);
    assert_eq!(
        (r[0], r[1].to_le_bytes()),
        (8, *b"yes, ok!"),
        "the first read was not the line's first eight bytes"
    );
    let r = stdin(proto::OPERATION_READ_LINE_BYTES, 0, 0);
    assert_eq!(
        (r[0], &r[1].to_le_bytes()[..2]),
        (2, &b"!\n"[..]),
        "the second read was not the rest of the line and its newline"
    );

    for (operation, what) in [
        (proto::OPERATION_BYTES, "OPERATION_BYTES"),
        (proto::OPERATION_PRINT, "OPERATION_PRINT"),
        (proto::OPERATION_WRITE, "OPERATION_WRITE"),
        (proto::OPERATION_RAWMODE, "OPERATION_RAWMODE"),
        (proto::OPERATION_READRAW, "OPERATION_READRAW"),
        (proto::OPERATION_READLINE, "OPERATION_READLINE"),
        (proto::OPERATION_QUIESCE, "OPERATION_QUIESCE"),
    ] {
        let r = stdin(operation, 1, u64::from(b'x'));
        assert_eq!(r[0], BAD_REQUEST, "a stdin holder's {what} was served");
    }
    let r = sched::ipc_call(w.term, [proto::req(proto::OPERATION_READ_LINE_BYTES, 0), 0]);
    assert_eq!(
        r[0], BAD_REQUEST,
        "the unbadged surface served a stdin read, which could take the shell's next line"
    );
    held.release_or_fail("raw_mode_service");
}
