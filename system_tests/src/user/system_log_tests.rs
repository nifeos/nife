//! **The system log service, end to end** (milestone 613 (a system log service: the in-memory
//! half)).
//!
//! The kernel plays two parts here, the same substitution the sink tests make: the **spawner**,
//! which holds the one unbadged intake capability and registers what each badge means, and the
//! **reader**, which `SEND`s a read through its own badge and waits on its notification. The two
//! writers are real user programs (`sink_transcript_writer`), each holding nothing but a badged
//! copy of the intake endpoint and a report endpoint, so the badge reaches the service through the
//! syscall path every writer will use.

use byte_sink_contract::fixture;
use system_log_contract::{control, read};

use super::*;
use crate::cap::{Rights, notification_cap, rendezvous_cap, rendezvous_cap_badged};
use crate::sched::RendezvousId;

/// The two writers' badges and users. Distinct users, so a reader that confused the two would put
/// the wrong name on a line rather than the right name on both.
const WRITERS: [(u32, &[u8]); 2] = [(1, b"alice"), (2, b"bob")];
/// The system reader's badge, answered in window 0.
const SYSTEM_READER: u32 = 9;
/// The per-user reader's badge, alice's, answered in window 1.
const ALICE_READER: u32 = 8;

/// Play the spawner: one control word, through the unbadged capability (a kernel `SEND` carries
/// badge 0).
fn control(intake: RendezvousId, w: (u64, u64, u64)) {
    crate::sched::ipc_send(intake, [w.0, w.1, w.2]);
}

/// Register `badge` as `program` run by `user` (empty for none).
fn register(intake: RendezvousId, badge: u32, program: &[u8], user: &[u8]) {
    for (field, name) in [(control::PROGRAM, program), (control::USER, user)] {
        for m in control::name_messages(badge, field, name)
            .into_iter()
            .flatten()
        {
            control(intake, m);
        }
    }
}

/// Spawn `sink_transcript_writer` holding only a badged copy of the intake endpoint (slot 0) and
/// its report endpoint (slot 1). It writes the transcript once and reports how its last `SEND`
/// classified.
fn spawn_writer(image: &'static [u8], intake: RendezvousId, badge: u32, report: RendezvousId) {
    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: 1,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap_badged(intake, Rights::WRITE, badge as u64), // slot 0
                    rendezvous_cap(report, Rights::WRITE),                      // slot 1
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn a log writer");
}

/// The most lines a test reads back from one window.
const MAX_LINES: usize = 8;

/// Play a reader: ask for everything from `cursor` through `badge`, wait for the service to say the
/// window is filled, and return the next cursor and the window's lines.
fn read_window(
    intake: RendezvousId,
    badge: u32,
    done: crate::sched::NotificationId,
    window_phys: u64,
    cursor: u64,
) -> (u64, [&'static str; MAX_LINES], usize) {
    crate::sched::ipc_send_badged(intake, [read::request(), cursor, 0], badge as u64);
    crate::sched::notification_wait(done).expect("the reader's notification died");
    // SAFETY: a frame this test allocated and owns until the end of the test, named through the
    // direct map. The service wrote it before signalling, and the signal is the ordering point.
    let window: &'static [u8] = unsafe {
        core::slice::from_raw_parts(
            mmu::phys_to_virt(window_phys) as *const u8,
            read::WINDOW_BYTES,
        )
    };
    let (next, len, status) = read::header(window);
    assert_eq!(
        status,
        read::status::OK,
        "the service refused a registered reader"
    );
    let text = core::str::from_utf8(&window[read::DATA..read::DATA + len])
        .expect("the service wrote a line that is not UTF-8");
    let mut lines = [""; MAX_LINES];
    let mut n = 0;
    for line in text.lines() {
        assert!(n < MAX_LINES, "more lines than this test wrote: {text}");
        lines[n] = line;
        n += 1;
    }
    (next, lines, n)
}

/// **Two differently badged writers, one ring, and each line attributed to the writer that wrote
/// it**, read back by the system reader; then the per-user read, which sees only alice's.
///
/// What would make this pass wrongly, and why it cannot: both writers print the same transcript,
/// so the text cannot tell them apart and only the stamp can. The stamp's `source` is the badge
/// the kernel read off each writer's capability, and `user` is what the spawner registered for that
/// badge. A service that read the badge from anywhere else (or a kernel that dropped it on a plain
/// `SEND`, which it did before this milestone) would stamp both lines with badge 0 and no user.
#[test_case]
fn two_badged_writers_are_attributed_by_the_badge_and_a_per_user_read_filters() {
    let Some(service) = program("system_log") else {
        crate::testing::skip!("no system_log program in this archive");
    };
    let writer = program("sink_transcript_writer")
        .expect("no sink_transcript_writer in the initial ramdisk");

    let region = crate::memory_region::create(4).expect("no region for the log's objects");
    let intake = crate::sched::create_rendezvous_from(region).expect("no intake rendezvous");
    let report = crate::sched::create_rendezvous_from(region).expect("no report rendezvous");
    let system_done = crate::sched::create_notification_from(region).expect("no notification");
    let alice_done = crate::sched::create_notification_from(region).expect("no notification");
    let windows = [
        crate::memory::alloc_zeroed().expect("no frame for window 0"),
        crate::memory::alloc_zeroed().expect("no frame for window 1"),
    ];
    let phys = [windows[0].addr(), windows[1].addr()];

    let tid = crate::sched::spawn(move || {
        run(
            service,
            Spawn {
                arg0: 0,
                arg1: 2, // two readers' notifications, slots 1 and 2
                arg2: 0,
                grants: &[
                    rendezvous_cap(intake, Rights::READ), // slot 0: the intake
                    notification_cap(system_done, Rights::WRITE), // slot 1: window 0's reader
                    notification_cap(alice_done, Rights::WRITE), // slot 2: window 1's reader
                ],
                maps: &[
                    Mapping {
                        va: read::WINDOW_VA,
                        phys: phys[0],
                        flags: Flags::user_data(),
                    },
                    Mapping {
                        va: read::WINDOW_VA + read::WINDOW_BYTES as u64,
                        phys: phys[1],
                        flags: Flags::user_data(),
                    },
                ],
            },
        )
    })
    .expect("could not spawn the system log service");

    register_the_writers_and_readers(intake);

    run_the_writers(writer, intake, report);

    let text = core::str::from_utf8(fixture::TRANSCRIPT)
        .unwrap()
        .trim_end_matches('\n');
    // The comparison below is against the raw text, which holds only while it needs no escaping.
    assert!(
        !text.contains(['"', '\\']),
        "the transcript needs JSON escaping now"
    );
    let (next, all, n) = read_window(intake, SYSTEM_READER, system_done, phys[0], 0);
    let all = &all[..n];
    assert_eq!(n, 2, "the system reader saw {all:?}");
    assert_eq!(next, 2);
    // Both writers printed the same text, so only the stamp tells their lines apart.
    for stamp in [
        r#""source":1,"program":"sink_transcript_writer","user":"alice","severity":"info","msg":""#,
        r#""source":2,"program":"sink_transcript_writer","user":"bob","severity":"info","msg":""#,
    ] {
        let line = all.iter().find(|l| l.contains(stamp));
        let line = line.unwrap_or_else(|| panic!("no line stamped {stamp} in {all:?}"));
        let msg = &line[line.find(stamp).unwrap() + stamp.len()..];
        assert_eq!(msg.strip_suffix("\"}"), Some(text), "{line}");
    }

    let (_, alice, n) = read_window(intake, ALICE_READER, alice_done, phys[1], 0);
    let alice = &alice[..n];
    assert_eq!(n, 1, "alice's read saw {alice:?}");
    assert!(alice[0].contains(r#""user":"alice""#), "{alice:?}");

    // Tear down: reclaiming the region destroys the intake, the service's receive answers `Gone`,
    // and it exits. Only then are its windows free to give back.
    assert!(
        wait_for(|| crate::sched::reclaim_region(region).is_ok()),
        "the log's region would not reclaim"
    );
    assert!(
        wait_for(|| !crate::sched::is_thread_present(tid)),
        "the system log service did not exit when its intake was destroyed"
    );
    for w in windows {
        crate::memory::free(w);
    }
}

/// Register both writers, the system reader and alice's reader with the service on `intake`.
fn register_the_writers_and_readers(intake: RendezvousId) {
    // The spawner's half, before any writer holds a capability: so a writer's first line is
    // already attributed.
    for (badge, user) in WRITERS {
        register(intake, badge, b"sink_transcript_writer", user);
    }
    register(intake, SYSTEM_READER, b"kernel_reader", b"");
    control(
        intake,
        (
            control::word(
                control::OPERATION_READER,
                control::SCOPE_SYSTEM,
                0,
                SYSTEM_READER,
            ),
            0,
            0,
        ),
    );
    register(intake, ALICE_READER, b"kernel_reader", b"alice");
    control(
        intake,
        (
            control::word(
                control::OPERATION_READER,
                control::SCOPE_USER,
                1,
                ALICE_READER,
            ),
            0,
            0,
        ),
    );
}

/// Spawn one `sink_transcript_writer` per badge and wait for each to report a clean run.
fn run_the_writers(writer: &'static [u8], intake: RendezvousId, report: RendezvousId) {
    for (badge, _) in WRITERS {
        spawn_writer(writer, intake, badge, report);
    }
    // Each writer reports after its last `SEND` returned, which is after the service received it;
    // the service handles one message before it receives the next, so by the time both reports
    // are in, every writer byte is in the ring ahead of the read below.
    for _ in WRITERS {
        let [class, total, ..] = crate::sched::ipc_receive(report);
        assert_eq!(
            class,
            fixture::code(byte_sink_contract::Sent::Ok),
            "a writer's log sink refused it"
        );
        assert_eq!(total as usize, fixture::TRANSCRIPT.len());
    }
}

/// Drain one `sink_transcript_writer` run on `ep`, alternating `RECEIVE_CAP` and `RECEIVE` so both of
/// the receives a server might use are asked, and return the badge every message arrived with
/// (asserting it is the same on each). Ends at the writer's end of stream.
fn badge_of_every_message(ep: RendezvousId) -> u64 {
    let mut seen = None;
    let mut bytes = 0;
    for i in 0.. {
        let m = if i % 2 == 0 {
            let m = crate::sched::ipc_receive_cap(ep);
            // Only x0 and x3 are read here. Which word lands in x1 for a plain SEND depends on
            // who reached the rendezvous first (`abi::rendezvous::RECEIVE_CAP`'s BUGS), so a test
            // that asserted it would be asserting the scheduler's order.
            [m[0], 0, 0, m[3]]
        } else {
            let m = crate::sched::ipc_receive(ep);
            [m[0], m[1], m[2], m[3]]
        };
        assert!(
            seen.is_none() || seen == Some(m[3]),
            "the badge changed between messages"
        );
        seen = Some(m[3]);
        if m[0] == byte_sink_contract::eof() {
            break;
        }
        bytes += byte_sink_contract::len(m[0]);
    }
    assert_eq!(bytes, fixture::TRANSCRIPT.len());
    seen.unwrap()
}

/// **A plain `SEND` arrives with the badge of the capability it went through, on `RECEIVE` and on
/// `RECEIVE_CAP`, and an unbadged one still arrives with 0** (calef's ruling on #1494, 2026-10-03
/// UTC, amending §230 (badged endpoint capabilities)).
///
/// The consumer whose behaviour this changes is `redoxfs_server`, which reads `RECEIVE_CAP`'s badge to
/// pick a client's window and scope. Before the change, a client that `SEND`s rather than `CALL`s
/// through its badged capability arrived as badge 0, the unscoped value; now it arrives as itself.
/// The second half pins the value every unbadged sender in the tree still relies on.
#[test_case]
fn a_plain_send_arrives_with_its_capabilitys_badge_on_receive_and_receive_cap() {
    let writer = program("sink_transcript_writer")
        .expect("no sink_transcript_writer in the initial ramdisk");
    let region = crate::memory_region::create(2).expect("no region for the test's endpoints");
    let ep = crate::sched::create_rendezvous_from(region).expect("no rendezvous");
    let report = crate::sched::create_rendezvous_from(region).expect("no rendezvous");

    spawn_writer(writer, ep, 0x5a5a, report);
    assert_eq!(badge_of_every_message(ep), 0x5a5a);
    let [class, ..] = crate::sched::ipc_receive(report);
    assert_eq!(class, fixture::code(byte_sink_contract::Sent::Ok));

    crate::sched::spawn(move || {
        run(
            writer,
            Spawn {
                arg0: 1,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(ep, Rights::WRITE),     // slot 0: unbadged
                    rendezvous_cap(report, Rights::WRITE), // slot 1
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the unbadged writer");
    assert_eq!(badge_of_every_message(ep), 0);
    let [class, ..] = crate::sched::ipc_receive(report);
    assert_eq!(class, fixture::code(byte_sink_contract::Sent::Ok));

    assert!(
        wait_for(|| crate::sched::reclaim_region(region).is_ok()),
        "the test's region would not reclaim"
    );
}

/// Busy-wait `nanos` on the kernel's own clock: long enough for the ring to call its drainer
/// stalled, without a sleep primitive the test harness would have to provide.
fn spin_for(nanos: u64) {
    let hz = crate::arch::timer::frequency();
    let until = crate::arch::timer::now() + nanos * hz / 1_000_000_000;
    while crate::arch::timer::now() < until {
        core::hint::spin_loop();
    }
}

/// The flags record `seq` carries in the kernel's ring.
fn ring_flags(seq: u64) -> u8 {
    let mut text = [0u8; system_log_contract::record::TEXT_MAX];
    crate::kernel_log::test_read(seq, &mut text)
        .1
        .expect("the record left the ring")
        .0
}

/// **A held line waits for the drainer, and a stalled drainer gets the counted fallback**
/// (milestone 342 (the kernel and the `console` server drive one UART from two address spaces),
/// §175 (where the kernel's own output goes)'s ruling B "with a fallback").
///
/// The test plays the drainer by writing the cursor page itself. With a drainer attached and
/// caught up, a `println!` reaches the ring and not the UART (`console::tx_bytes` does not move).
/// Left unread past `STALL_NANOS`, the next line finds the drainer stalled: it prints the held line
/// and itself, flags both `DIRECT`, and counts two fallbacks.
#[test_case]
fn a_held_line_waits_for_its_drainer_and_a_stalled_drainer_falls_back_counted() {
    use system_log_contract::kernel_ring::DETACHED;
    use system_log_contract::record::flags;
    // The interactive boot publishes the ring as it builds the progenitor; the test kernel builds
    // none, so it publishes here. Idempotent, and every later line simply also lands in the ring.
    crate::kernel_log::publish();
    // End the harness's own `test name ... ` line first: a line already begun goes where it
    // began, and that one began on the UART.
    crate::println!();
    let mut text = [0u8; system_log_contract::record::TEXT_MAX];
    let (next, _) = crate::kernel_log::test_read(0, &mut text);
    let fallbacks = crate::kernel_log::fallback_count();
    crate::kernel_log::test_set_cursor(next);
    let tx = crate::console::tx_bytes();
    crate::println!("  kernel log probe: held for the drainer");
    let held_tx = crate::console::tx_bytes();
    let held_flags = ring_flags(next);
    spin_for(crate::kernel_log::STALL_NANOS + 100_000_000);
    crate::println!("  kernel log probe: after the stall");
    let after_tx = crate::console::tx_bytes();
    let (flags_then, flags_now) = (ring_flags(next), ring_flags(next + 1));
    let counted = crate::kernel_log::fallback_count() - fallbacks;
    crate::kernel_log::test_set_cursor(DETACHED);

    assert_eq!(
        held_tx, tx,
        "a line held for a live drainer reached the UART anyway"
    );
    assert_eq!(held_flags & flags::DIRECT, 0);
    assert_ne!(held_flags & flags::KERNEL, 0);
    let both = "  kernel log probe: held for the drainer\n  kernel log probe: after the stall\n";
    assert_eq!(
        after_tx - held_tx,
        both.len() as u64,
        "the fallback did not print both lines"
    );
    assert_ne!(
        flags_then & flags::DIRECT,
        0,
        "the caught-up line was not flagged DIRECT"
    );
    assert_ne!(flags_now & flags::DIRECT, 0);
    assert_eq!(counted, 2);
}

/// **A stalled drainer's last line reaches the wire with nothing printed after it** (milestone
/// 890). The case is a drainer blocked behind a console that died: the line saying why is the
/// last one the kernel prints, so no later line would find the stall. The timer tick's poll
/// (`kernel_log::stall_tick`) must print it within its period on its own.
///
/// Falsification: attested 2026-10-20. On patagonia, riscv64 under QEMU (`script/test --arch riscv64
/// --test` this name), with the `stall_tick` call removed from `sched::preemption`: red, "the held
/// line never reached the UART without a later print". Green with it restored.
#[test_case]
fn a_stalled_drainers_last_line_reaches_the_wire_with_no_line_after_it() {
    use system_log_contract::kernel_ring::DETACHED;
    use system_log_contract::record::flags;
    // As the test above: publish, end the harness's line, and play a drainer that never reads.
    crate::kernel_log::publish();
    crate::println!();
    let mut text = [0u8; system_log_contract::record::TEXT_MAX];
    let (next, _) = crate::kernel_log::test_read(0, &mut text);
    crate::kernel_log::test_set_cursor(next);
    let line = "  kernel log probe: the last line before the drainer stopped\n";
    crate::print!("{line}");
    let held_tx = crate::console::tx_bytes();
    // The stall test needs `STALL_NANOS` and the poll runs once a second; three seconds bounds
    // both with room for a loaded emulator. The poll may run on another core and writes the text
    // and its newline separately, so the loop waits for the whole line, not the first byte.
    let hz = crate::arch::timer::frequency();
    let until = crate::arch::timer::now() + 3 * hz;
    while crate::console::tx_bytes() - held_tx < line.len() as u64
        && crate::arch::timer::now() < until
    {
        core::hint::spin_loop();
    }
    let rescued_tx = crate::console::tx_bytes();
    let rescued_flags = ring_flags(next);
    crate::kernel_log::test_set_cursor(DETACHED);

    assert_eq!(
        rescued_tx - held_tx,
        line.len() as u64,
        "the held line never reached the UART without a later print"
    );
    assert_ne!(rescued_flags & flags::DIRECT, 0);
}

/// **A panic prints what the drainer had not, then everything directly** (§175's panic escape).
/// `console::enter_panic` is what the panic handler calls after breaking the console lock; called
/// here without dying, it must put the held line on the UART, and the line after it must not wait.
#[test_case]
fn a_panic_prints_the_held_lines_first_and_holds_nothing_after() {
    use system_log_contract::kernel_ring::DETACHED;
    // The interactive boot publishes the ring as it builds the progenitor; the test kernel builds
    // none, so it publishes here. Idempotent, and every later line simply also lands in the ring.
    crate::kernel_log::publish();
    // End the harness's own `test name ... ` line first: a line already begun goes where it
    // began, and that one began on the UART.
    crate::println!();
    let mut text = [0u8; system_log_contract::record::TEXT_MAX];
    let (next, _) = crate::kernel_log::test_read(0, &mut text);
    crate::kernel_log::test_set_cursor(next);
    let tx = crate::console::tx_bytes();
    crate::println!("  kernel log probe: held when the panic came");
    let held_tx = crate::console::tx_bytes();
    crate::console::enter_panic();
    let flushed_tx = crate::console::tx_bytes();
    crate::println!("  kernel log probe: after the panic");
    let after_tx = crate::console::tx_bytes();
    crate::kernel_log::test_leave_panic();
    crate::kernel_log::test_set_cursor(DETACHED);

    assert_eq!(held_tx, tx);
    assert_eq!(
        flushed_tx - held_tx,
        "  kernel log probe: held when the panic came\n".len() as u64,
        "the panic path did not print the line the drainer had not"
    );
    assert_eq!(
        after_tx - flushed_tx,
        "  kernel log probe: after the panic\n".len() as u64,
        "a line printed after the panic was held"
    );
}
