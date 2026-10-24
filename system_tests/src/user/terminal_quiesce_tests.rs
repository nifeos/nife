//! **A terminal can be quiesced for replacement without stranding its reader** (milestone 23 (a
//! capability-routed component OS with live replacement); calef's ruling of 2026-09-26, "1a and
//! 2b"). `OPERATION_QUIESCE` rides the terminal endpoint, and a parked `OPERATION_READLINE` or `OPERATION_READRAW` is
//! answered `FLAG_RETRY` before the terminal stops receiving, because the reply capability that
//! names the reader cannot leave `line_editor`'s capability table.
//!
//! The test plays the supervisor as well as the input driver and the reader, on the harness
//! `raw_mode_service` already provides. As the supervisor it quiesces through a copy badged
//! `proto::SUPERVISOR_BADGE`; as anyone else, through the unbadged copy every client holds. What it proves is the incumbent's half of a swap: the
//! reader is handed back, the half-typed line survives a resume, and the resumed read repaints
//! nothing. The replacement's half needs the handoff page count first
//! (`design/roadmap/0659-a-region-retypes-a-frame-run.md`).

use line_editor::proto;
use raw_mode_service as svc;

use super::*;
use crate::sched;

const SENTINEL: u8 = 0xaa;

/// `OPERATION_QUIESCE` as the supervisor sends it, through its badged copy.
fn quiesce_as_supervisor(term: sched::RendezvousId) -> [u64; 3] {
    sched::ipc_call_badged(
        term,
        [proto::req(proto::OPERATION_QUIESCE, 0), 0],
        proto::SUPERVISOR_BADGE,
    )
}

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

/// Let the other threads run for a twentieth of a second: long enough for a spawned reader to
/// park, the same settle `raw_mode_tests` uses for the same reason.
fn settle() {
    let deadline = crate::arch::timer::now() + crate::arch::timer::frequency() / 20;
    while crate::arch::timer::now() < deadline {
        sched::yield_now();
    }
}

fn page_byte(phys: u64, i: u64) -> u8 {
    // SAFETY: a frame the harness allocated and mapped into `line_editor`; the read is ordered
    // after the exchange that wrote it by the IPC the test just completed.
    unsafe { core::ptr::read_volatile((mmu::phys_to_virt(phys) + i) as *const u8) }
}

fn fill(phys: u64, bytes: &[u8]) {
    for (i, &b) in bytes.iter().enumerate() {
        // SAFETY: as `page_byte`; no request naming this page is in flight.
        unsafe { core::ptr::write_volatile((mmu::phys_to_virt(phys) + i as u64) as *mut u8, b) };
    }
}

/// **A terminal nobody can resume refuses to quiesce**, even from the supervisor's badge:
/// honoring it would be a dead terminal.
#[test_case]
fn a_terminal_with_no_control_endpoint_refuses_to_quiesce() {
    let (w, held) = svc::start();
    let r = quiesce_as_supervisor(w.term);
    assert_eq!(
        r[0],
        proto::BAD_REQUEST,
        "an unsupervised terminal honored OPERATION_QUIESCE"
    );
    bytes_call(w.term, b"ok\r");
    let r = sched::ipc_call(w.term, [proto::req(proto::OPERATION_READLINE, 0), 0]);
    assert_eq!(
        r[0], 2,
        "the terminal stopped serving after refusing a quiesce"
    );
    held.release_or_fail("raw_mode_service");
}

/// **A terminal that can be quiesced takes the quiesce from its supervisor and from nobody
/// else** (§232 (the `line_editor` swap contract): "only a supervisor sends it"). The input
/// driver, the shell, `login` and the spawner all hold the unbadged copy this test calls through
/// first. Honored, that quiesce would stop the terminal on its control endpoint, where only the
/// supervisor speaks, and a supervisor that did not ask never says `CTL_RESUME`: the boot console
/// would be dead until reboot. So it is refused, the terminal goes on serving a parked reader
/// across it, and the supervisor's badged copy still quiesces it, and carries nothing else.
///
/// Falsification: replayable `system_tests/falsifications/user.terminal_quiesce_tests.a_quiesce_from_anyone_but_the_supervisor_is_refused_and_the_terminal_serves_on.patch`
#[test_case]
fn a_quiesce_from_anyone_but_the_supervisor_is_refused_and_the_terminal_serves_on() {
    let (w, held) = svc::start_replaceable();
    let control = w
        .control
        .expect("start_replaceable grants a control endpoint");

    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let report = sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let term = w.term;
    sched::spawn(move || {
        let r = sched::ipc_call(term, [proto::req(proto::OPERATION_READLINE, 0), 0]);
        sched::ipc_send(report, [r[0], r[1], 0]);
    })
    .expect("could not spawn the reader");
    settle();
    bytes_call(w.term, b"ec");

    let q = sched::ipc_call(w.term, [proto::req(proto::OPERATION_QUIESCE, 0), 0]);
    assert_eq!(
        q[0],
        proto::BAD_REQUEST,
        "an unbadged holder quiesced a supervised terminal (reply {:#x}): any client could stop \
         the boot console until a CTL_RESUME nobody sends",
        q[0],
    );

    // Still serving, and the parked read was neither handed back nor lost: the line finishes.
    bytes_call(w.term, b"ho\r");
    let done = sched::ipc_receive(report);
    assert_eq!(
        (done[0], done[1]),
        (4, 0),
        "the read parked across a refused quiesce did not come back as the line"
    );
    let got: [u8; 4] = core::array::from_fn(|i| page_byte(w.app_in_phys, i as u64));
    assert_eq!(
        &got, b"echo",
        "the line typed across the refused quiesce came back wrong"
    );

    // The supervisor's copy is a quiesce and nothing more: it cannot type into the terminal.
    let typed = sched::ipc_call_badged(
        w.term,
        [proto::req(proto::OPERATION_BYTES, 1), u64::from(b'x')],
        proto::SUPERVISOR_BADGE,
    );
    assert_eq!(
        typed[0],
        proto::BAD_REQUEST,
        "the supervisor's badged copy was served OPERATION_BYTES"
    );

    assert_eq!(
        quiesce_as_supervisor(w.term)[0],
        proto::QUIESCED,
        "the supervisor's own quiesce was refused"
    );
    sched::ipc_send(control, [proto::CTL_QUIT, 0, 0]);
    held.release_or_fail("raw_mode_service");
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **A parked line read is handed back, and the line resumes where it was.** The reader sees
/// `FLAG_RETRY` and asks again; the terminal, resumed, answers the second read with the whole
/// line, half typed before the quiesce and finished after it, and repaints nothing in between.
#[test_case]
fn a_parked_line_read_is_handed_back_and_resumes_where_it_was() {
    let (w, held) = svc::start_replaceable();
    let control = w
        .control
        .expect("start_replaceable grants a control endpoint");
    fill(w.app_out_phys, b"$ ");

    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let report = sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let term = w.term;
    sched::spawn(move || {
        loop {
            let r = sched::ipc_call(term, [proto::req(proto::OPERATION_READLINE, 2), 0]);
            sched::ipc_send(report, [r[0], r[1], 0]);
            if !proto::is_retry(r[0], r[1]) {
                break;
            }
        }
    })
    .expect("could not spawn the reader");
    settle();
    bytes_call(w.term, b"ec");

    let q = quiesce_as_supervisor(w.term);
    assert_eq!(q[0], proto::QUIESCED, "the quiesce was not acknowledged");
    let first = sched::ipc_receive(report);
    assert!(
        proto::is_retry(first[0], first[1]),
        "the parked read was not handed back with FLAG_RETRY (r0 {:#x}, r1 {:#x})",
        first[0],
        first[1],
    );

    // The reader has asked again and is parked on the endpoint's sender queue, with nobody
    // receiving. Resume, and watch the console page: a repaint of "$ ec" would land there.
    settle();
    fill(w.console_phys, &[SENTINEL; 8]);
    sched::ipc_send(control, [proto::CTL_RESUME, 0, 0]);
    settle();
    assert_eq!(
        page_byte(w.console_phys, 0),
        SENTINEL,
        "the resumed read repainted the prompt and the half-typed line, which the screen already shows",
    );

    bytes_call(w.term, b"ho\r");
    let done = sched::ipc_receive(report);
    assert_eq!(done[0], 4, "the resumed read did not return the whole line");
    let got: [u8; 4] = core::array::from_fn(|i| page_byte(w.app_in_phys, i as u64));
    assert_eq!(
        &got, b"echo",
        "the line typed across the quiesce came back wrong"
    );
    held.release_or_fail("raw_mode_service");
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **A parked raw read is handed back in the raw reply's own shape**, r0 = 0 with the flag in r1,
/// because r1 is the data word there. Then the supervisor says quit, and the terminal goes.
#[test_case]
fn a_parked_raw_read_is_handed_back_and_the_terminal_can_be_retired() {
    let (w, held) = svc::start_replaceable();
    let control = w
        .control
        .expect("start_replaceable grants a control endpoint");
    let r = sched::ipc_call(w.term, [proto::req(proto::OPERATION_RAWMODE, 1), 0]);
    assert_eq!(r[0], 0);

    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let report = sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let term = w.term;
    sched::spawn(move || {
        let r = sched::ipc_call(term, [proto::req(proto::OPERATION_READRAW, 0), 0]);
        sched::ipc_send(report, [r[0], r[1], 0]);
    })
    .expect("could not spawn the raw reader");
    settle();

    let q = quiesce_as_supervisor(w.term);
    assert_eq!(q[0], proto::QUIESCED);
    let got = sched::ipc_receive(report);
    assert_eq!(
        (got[0], got[1]),
        (0, proto::FLAG_RETRY),
        "a parked raw read must come back as r0 = 0, r1 = FLAG_RETRY",
    );
    sched::ipc_send(control, [proto::CTL_QUIT, 0, 0]);
    held.release_or_fail("raw_mode_service");
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}
