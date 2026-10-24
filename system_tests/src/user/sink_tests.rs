use byte_sink_contract::fixture;

use super::*;
use crate::cap::{Rights, rendezvous_cap};
use crate::sched::RendezvousId;

/// Spawn `fixtures/src/sink_transcript_writer.rs` against `sink`, and return the endpoint it
/// reports its classification on.
///
/// `repeat` is the program's `a0`, its only argument: how many times to write the transcript, 0
/// meaning "until the sink stops taking it". `None` leaves slot 0 empty rather than passing a flag:
/// an empty slot is how this kernel spells "you were never given one", the refusal an ungranted
/// display client meets too (§29 (the framebuffer is a bigger grant, not an exemption)). The
/// report goes in at slot 1, placed rather than appended, so the two programs are otherwise wired
/// identically. Its endpoint is carved from `endpoints`, a region the caller owns and reclaims.
fn spawn_writer(
    image: &'static [u8],
    sink: Option<RendezvousId>,
    repeat: u64,
    endpoints: u64,
) -> RendezvousId {
    let report = crate::sched::create_rendezvous_from(endpoints).expect("no writer report");
    crate::sched::spawn(move || match sink {
        Some(ep) => run(
            image,
            Spawn {
                arg0: repeat,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(ep, Rights::WRITE),     // slot 0: the byte sink
                    rendezvous_cap(report, Rights::WRITE), // slot 1: the classification
                ],
                maps: &[],
            },
        ),
        None => {
            crate::sched::grant_at(1, rendezvous_cap(report, Rights::WRITE))
                .expect("the writer's report slot was already occupied");
            run(
                image,
                Spawn {
                    arg0: repeat,
                    arg1: 0,
                    arg2: 0,
                    grants: &[],
                    maps: &[],
                },
            )
        }
    })
    .expect("could not spawn the sink writer");
    report
}

/// Spawn `wc` with `source` in its input slot, and return the endpoint its answer arrives on.
///
/// Two capabilities and nothing else, which is what makes the test below mean something: `wc`
/// holds no file, no directory, no page and nothing that names the FS server. Whatever is
/// behind its input slot, it is handed the bytes.
///
/// The answer endpoint is carved from `endpoints`, a region the caller owns and reclaims.
fn spawn_wc(source: RendezvousId, endpoints: u64) -> RendezvousId {
    let image = program("wc").expect("no wc program in the initial ramdisk");
    let out = crate::sched::create_rendezvous_from(endpoints).expect("no wc answer endpoint");
    crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(out, Rights::WRITE), // slot 0: where its answer goes
                    rendezvous_cap(source, Rights::READ), // slot 1: where its bytes come from
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn wc");
    out
}

/// Drain `wc`'s answer and parse the three numbers out of it.
fn wc_counts(out: RendezvousId, what: &str) -> (u64, u64, u64) {
    let mut buf = [0u8; 64];
    let n = super::std_tests::drain_sink(out, &mut buf, what);
    let text = core::str::from_utf8(&buf[..n]).expect("wc printed non-UTF-8");
    let mut it = text.split_ascii_whitespace().map(|w| {
        w.parse::<u64>()
            .unwrap_or_else(|_| panic!("{what}: wc printed {text:?}"))
    });
    (
        it.next().expect("no line count"),
        it.next().expect("no word count"),
        it.next().expect("no byte count"),
    )
}

/// `<` proven the way `>` is: the same `wc`, two sources, the same answer.
///
/// The indifference claim on the input side, where a source is the sink contract received rather
/// than sent. One `wc` ELF is spawned twice with identical grants except slot 1: a pipe (this test
/// sends the transcript sixteen bytes at a time, then `OPERATION_EOF`) and a file (`file_sink`
/// writes it to the RedoxFS image and `file_source` streams it back, `wc < report.txt` without the
/// shell). `wc` holds nothing that could tell which. The answers must be equal and must equal the
/// transcript, because two arms broken the same way would satisfy equality alone.
#[test_case]
fn one_reader_two_sources_and_the_same_answer() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let writer_image = program("sink_transcript_writer")
        .expect("no sink_transcript_writer in the initial ramdisk");
    let file_sink_image = program("file_sink").expect("no file_sink in the initial ramdisk");
    let file_source_image = program("file_source").expect("no file_source in the initial ramdisk");
    // The block server, whichever binary carries it on this architecture: the binary differs per
    // ISA, and `fs_service::blk_server_image()` is the one place that records which.
    let blk = fs_service::blk_server_image();
    let Some(redoxfs_server) = program("redoxfs_server") else {
        crate::testing::skip!("no redoxfs_server program in this archive");
    };

    // Arm one: a pipe. The kernel is the producer, which is the same position the shell is in
    // when a builtin leads a pipeline (`kernel::user::pipeline_tests`).
    // The pipe, both `wc` answers and the writer's report, then the file sink's and the file
    // source's endpoints.
    let endpoints = crate::memory_region::create(4 + 2 * fs_service::FILE_SINK_PAGES)
        .expect("no endpoint region");
    let pipe = crate::sched::create_rendezvous_from(endpoints).expect("no pipe rendezvous");
    let out = spawn_wc(pipe, endpoints);
    let mut off = 0usize;
    while off < fixture::TRANSCRIPT.len() {
        let (w0, w1, w2, n) = byte_sink_contract::pack(&fixture::TRANSCRIPT[off..]);
        crate::sched::ipc_send(pipe, [w0, w1, w2]);
        off += n;
    }
    crate::sched::ipc_send(pipe, [byte_sink_contract::eof(), 0, 0]);
    let piped = wc_counts(out, "wc reading a pipe");

    // The transcript's own numbers, so the pipe arm is anchored to something rather than only
    // to the file arm.
    let text = core::str::from_utf8(fixture::TRANSCRIPT).expect("the fixture is not UTF-8");
    assert_eq!(
        piped,
        (
            text.lines().count() as u64,
            text.split_ascii_whitespace().count() as u64,
            fixture::TRANSCRIPT.len() as u64,
        ),
        "wc miscounted the transcript it was handed down a pipe",
    );

    // Arm two: the same bytes, through a real filesystem. Write them first.
    let Some(file_sink) =
        fs_service::start_file_sink(blk, redoxfs_server, file_sink_image, endpoints)
    else {
        // The pipe arm ran and asserted; the file arm is missing. A skip, not a pass, because the
        // test's name claims two sources agree and one cannot agree with anything. The reason says
        // which half ran.
        crate::sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
        crate::testing::skip!(
            "no RedoxFS disk attached: the pipe arm ran, the file arm did not, so the two \
             sources were never compared"
        );
    };
    fs_service::wait_for_service(file_sink.readiness);
    assert_eq!(
        crate::sched::ipc_receive(file_sink.report)[0],
        fixture::READY,
        "the file sink could not open its file",
    );
    let wrote = spawn_writer(writer_image, Some(file_sink.sink), 1, endpoints);
    let [code, total, ..] = crate::sched::ipc_receive(wrote);
    assert_eq!(
        code,
        fixture::code(byte_sink_contract::Sent::Ok),
        "the writer did not deliver the transcript to the file sink",
    );
    let [done, wrote_total, ..] = crate::sched::ipc_receive(file_sink.report);
    assert_eq!(done, fixture::DONE, "the file sink did not finish cleanly");
    assert_eq!(
        wrote_total, total,
        "the file sink recorded a different byte count from the one the writer sent",
    );

    // Then read them back into `wc`, which is `<`.
    let Some((source, verify_report)) =
        fs_service::start_file_source(blk, redoxfs_server, file_source_image, endpoints)
    else {
        panic!("the FS service vanished between the file sink and its source");
    };
    let out = spawn_wc(source, endpoints);
    let filed = wc_counts(out, "wc reading a file");
    let [vdone, size, ..] = crate::sched::ipc_receive(verify_report);
    assert_eq!(
        vdone,
        fixture::DONE,
        "the source adapter could not read the file back ({vdone:#x})",
    );
    assert_eq!(
        size, total,
        "the file is not the size that was written into it"
    );

    assert_eq!(
        piped, filed,
        "the same wc answered differently for a pipe and for a file, so its input slot is not \
         opaque after all",
    );
    crate::sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// The same program, two destinations, the same bytes.
///
/// `std_exerciser` is spawned twice with identical grants except slot 1: an endpoint this test
/// receives on (the pipe shape), and one served by `file_sink`, which appends every message to a
/// file on the RedoxFS image; a third process reads the file back over the sink contract. The
/// second arm is compared against what the first actually received, and the first against the
/// pinned transcript, since a constant would pass two arms broken alike. Byte counts are asserted
/// on both sides, since an empty file would satisfy "equal" if the program printed nothing.
#[test_case]
fn a_program_cannot_tell_what_its_output_slot_holds() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    if std_service::std_exerciser_image().is_none() {
        crate::testing::skip!(std_service::NO_STD_EXERCISER);
    }
    let std_exerciser =
        program("std_exerciser").expect("no std_exerciser program in the initial ramdisk");
    let clock = program("clock").expect("no clock program in the initial ramdisk");
    let entropy = program("entropy").expect("no entropy program in the initial ramdisk");
    let file_sink_image = program("file_sink").expect("no file_sink in the initial ramdisk");
    let file_source_image = program("file_source").expect("no file_source in the initial ramdisk");
    let Some(redoxfs_server) = program("redoxfs_server") else {
        crate::testing::skip!("no redoxfs_server program in this archive");
    };

    // Arm one: the kernel receives. Everything here is what the existing std test does, which
    // is the point: nothing was special-cased for the sink.
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let direct = crate::sched::create_rendezvous_from(endpoints).expect("no direct endpoint");
    let direct_run = std_service::start_on_full(std_exerciser, clock, entropy, direct);
    let mut first = [0u8; 512];
    let n1 = super::std_tests::drain_sink(direct, &mut first, "std_exerciser, direct endpoint");
    give_back(&direct_run, "std_exerciser, direct endpoint");
    // Drained to its end of stream, so the program has nothing left to send on it.
    crate::sched::reclaim_region(endpoints).expect("the endpoint region did not come back");

    assert_eq!(
        &first[..n1],
        super::std_tests::EXPECTED,
        "the direct arm did not print the pinned transcript, so it is no reference for what \
         the other arm produces",
    );

    // Arm two: a file sink. It opens its file before the writer exists, for the reason
    // `fs_service::wait_for_caretaker` records: it stages a name in the page it shares with the
    // FS server, and a client that already existed could write over it.
    // The block server, whichever binary carries it on this architecture: the binary differs per
    // ISA, and `fs_service::blk_server_image()` is the one place that records which.
    let blk = fs_service::blk_server_image();
    // The sink's endpoints and the verifier's, reclaimed at the end once both have exited.
    let endpoints =
        crate::memory_region::create(2 * fs_service::FILE_SINK_PAGES).expect("no endpoint region");
    let Some(file_sink) =
        fs_service::start_file_sink(blk, redoxfs_server, file_sink_image, endpoints)
    else {
        crate::sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
        crate::testing::skip!("no RedoxFS disk attached");
    };
    let (sink_ep, sink_report) = (file_sink.sink, file_sink.report);
    fs_service::wait_for_service(file_sink.readiness);
    assert_eq!(
        crate::sched::ipc_receive(sink_report)[0],
        fixture::READY,
        "the file sink could not open its file, so there was nothing to redirect into",
    );

    let sink_run = std_service::start_on_full(std_exerciser, clock, entropy, sink_ep);
    let [done, total, ..] = crate::sched::ipc_receive(sink_report);
    give_back(&sink_run, "std_exerciser, into the file sink");
    assert_eq!(
        done,
        fixture::DONE,
        "the file sink reported {done:#x} rather than finishing: it refused a message, or the \
         FS server refused a write",
    );
    assert_eq!(
        total as usize, n1,
        "the file sink wrote {total} bytes for a program that printed {n1}",
    );

    // The read-back, in a third process with its own FS session, and only now that the sink has
    // closed the file: the two share the FS server's one file page.
    let Some((out, verify_report)) =
        fs_service::start_file_source(blk, redoxfs_server, file_source_image, endpoints)
    else {
        panic!("the FS service vanished between the file sink and its verifier");
    };
    let mut second = [0u8; 512];
    let n2 = super::std_tests::drain_sink(out, &mut second, "std_exerciser, file sink");
    let [vdone, vsize, ..] = crate::sched::ipc_receive(verify_report);
    assert_eq!(
        vdone,
        fixture::DONE,
        "the verifier could not read back the file the sink wrote ({vdone:#x})",
    );
    assert_eq!(
        vsize as usize, n2,
        "the verifier streamed a length it did not find on the platter",
    );

    assert_eq!(
        &second[..n2],
        &first[..n1],
        "the same program printed different bytes into a file than into an endpoint, so its \
         output destination is something it can tell apart",
    );
    crate::println!("    ({n1} bytes, identical through an endpoint and through RedoxFS)");
    crate::sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// A dead reader ends the writer, and a missing one does not: the one behavior change of milestone
/// 50 (pipes and redirection: one sink protocol), asserted by value on all three outcomes.
///
/// A sink that stays classifies `Ok`. A sink whose region this test reclaims after a few messages
/// (§16 (object revocation)) classifies `Gone` from a blocked `SEND`, as in `yes | head`. An empty
/// slot classifies `NoSink` and the program keeps running, which is what makes `Gone` mean
/// anything: a writer that ended on both would kill every program never given a console. A writer
/// in a syscall on a dead capability needs no `SIGPIPE`. See notes/sink-protocol.md.
#[test_case]
fn a_destroyed_sink_ends_the_writer_and_an_absent_one_does_not() {
    let image = program("sink_transcript_writer")
        .expect("no sink_transcript_writer in the initial ramdisk");

    // Four pages: the live sink and the three writers' reports.
    let endpoints = crate::memory_region::create(4).expect("no endpoint region");

    // 1. A sink that stays.
    let live = crate::sched::create_rendezvous_from(endpoints).expect("no live sink");
    let report = spawn_writer(image, Some(live), 1, endpoints);
    let mut got = [0u8; 256];
    let n = super::std_tests::drain_sink(live, &mut got, "the writer with a live sink");
    assert_eq!(
        &got[..n],
        fixture::TRANSCRIPT,
        "the writer did not deliver its transcript to a sink that was there",
    );
    let [class, total, ..] = crate::sched::ipc_receive(report);
    assert_eq!(
        (class, total as usize),
        (
            fixture::code(byte_sink_contract::Sent::Ok),
            fixture::TRANSCRIPT.len()
        ),
        "a writer whose sink stayed put must classify Ok",
    );

    // 2. A sink destroyed under a writer blocked in a send. The endpoint is retyped out of a
    // region this test owns, because destroying the region is how an endpoint dies (§16) and
    // the kernel's own endpoints are not in one.
    let region = crate::memory_region::create(4).expect("no untyped for the doomed sink");
    let doomed = crate::sched::create_rendezvous_from(region).expect("no endpoint in the region");
    let report = spawn_writer(image, Some(doomed), 0, endpoints);
    // Take a few messages first, so the writer is demonstrably running and parked in the next
    // send rather than having failed before it ever reached one.
    for _ in 0..3 {
        let _ = crate::sched::ipc_receive(doomed);
    }
    crate::sched::reclaim_region(region).expect("the doomed sink's region would not reclaim");
    let [class, total, ..] = crate::sched::ipc_receive(report);
    assert_eq!(
        class,
        fixture::code(byte_sink_contract::Sent::Gone),
        "a writer whose sink was destroyed classified {class} rather than Gone. Before \
         milestone 50 this arrived as NoSuchSlot, indistinguishable from never having had a \
         sink, which is exactly why `yes | head` could not end.",
    );
    assert!(
        total >= 3,
        "the writer reported {total} bytes through, so it never reached a live send and the \
         classification above says nothing about a sink dying under one",
    );

    // 3. No sink at all: the slot is empty, and the program keeps running.
    let report = spawn_writer(image, None, 1, endpoints);
    let [class, total, ..] = crate::sched::ipc_receive(report);
    assert_eq!(
        (class, total),
        (fixture::code(byte_sink_contract::Sent::NoSink), 0),
        "a program with an empty output slot must keep running and print into the void, which \
         is what every OS does to a process whose stdout is closed",
    );
    crate::sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// The same indifferent writer, a third destination: the terminal (milestone 50's last remainder).
///
/// The terminal's endpoint also carries `OPERATION_READLINE`, and `WRITE` on it is the right to
/// `CALL`, so a child handed it as output would hold the keyboard. Its sink is a separate endpoint
/// served by `terminal_sink_caretaker`, holding the sink `READ` and a terminal endpoint `WRITE`;
/// the kernel serves the terminal contract on the far side here, so the transcript is asserted byte
/// for byte through a real adapter speaking `line_editor::proto::OPERATION_PRINT`. The writer holds
/// one capability, to the adapter, so it cannot read a line. See notes/sink-protocol.md, "The
/// terminal's sink adapter, and the wall it hit".
#[test_case]
fn the_terminal_is_a_sink_like_any_other_and_the_writer_cannot_tell() {
    let adapter = program("terminal_sink_caretaker")
        .expect("no terminal_sink_caretaker program in the initial ramdisk");
    let writer = program("sink_transcript_writer")
        .expect("no sink_transcript_writer in the initial ramdisk");

    // The adapter serves `sink_ep` in a loop that never ends and ignores a failed receive, so its
    // endpoints go back only once it has been killed: a `Holding` does both, in that order.
    let endpoints = crate::memory_region::create(3).expect("no endpoint region");
    let sink_ep = crate::sched::create_rendezvous_from(endpoints).expect("no sink endpoint");
    let term_ep = crate::sched::create_rendezvous_from(endpoints).expect("no terminal endpoint");

    let adapter_tid = crate::sched::spawn(move || {
        run(
            adapter,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(sink_ep, Rights::READ), // slot 0: the sink it serves
                    rendezvous_cap(term_ep, Rights::WRITE), // slot 1: the terminal it prints to
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the terminal sink adapter");
    let mut held = crate::user::holding::Holding::new();
    held.add_thread(adapter_tid);
    held.add_region(endpoints);

    let report = spawn_writer(writer, Some(sink_ep), 1, endpoints);

    // The terminal, played by this test. `OPERATION_PRINT` carries up to eight bytes in its second word,
    // which is the terminal contract's request shape (a served request arrives with the reply
    // capability and two data words), so a sixteen-byte sink message arrives as two calls.
    let mut got = [0u8; fixture::TRANSCRIPT.len()];
    let mut n = 0usize;
    while n < fixture::TRANSCRIPT.len() {
        let m = crate::sched::ipc_receive_cap(term_ep);
        let (w0, slot, w1) = (m[0], m[1], m[2]);
        let crate::cap::Object::Reply(caller) = crate::sched::current_cap(slot)
            .expect("the adapter's print carried no reply capability")
            .object
        else {
            panic!("the adapter sent the terminal something that was not a CALL");
        };
        assert_eq!(
            line_editor::proto::operation(w0),
            line_editor::proto::OPERATION_PRINT,
            "the adapter sent the terminal something other than a print",
        );
        let len = line_editor::proto::len(w0).min(8);
        let bytes = w1.to_le_bytes();
        for &b in &bytes[..len] {
            assert!(
                n < got.len(),
                "the adapter printed more than the transcript"
            );
            got[n] = b;
            n += 1;
        }
        crate::sched::ipc_reply(caller, [len as u64, 0]);
        crate::sched::delete_current_cap(slot).expect("consume the one-shot reply");
    }

    assert_eq!(
        &got[..],
        fixture::TRANSCRIPT,
        "the bytes that reached the terminal are not the ones the writer wrote",
    );

    let [class, total, ..] = crate::sched::ipc_receive(report);
    assert_eq!(
        (class, total as usize),
        (
            fixture::code(byte_sink_contract::Sent::Ok),
            fixture::TRANSCRIPT.len()
        ),
        "the writer should have classified a terminal exactly as it classifies a pipe and a file",
    );
    held.release_or_fail("terminal_sink_caretaker");
}

/// Wait for a std program started by `start_on_full` to leave, then hand back its heap and its
/// wiring (milestone 801 (packages over the internet): every frame a std spawn holds comes back).
fn give_back(
    (heap, thread, wiring): &(u64, crate::thread::ThreadId, std_service::Wiring),
    what: &str,
) {
    assert!(
        super::wait_for(|| !crate::sched::is_thread_present(*thread)),
        "{what}: the std program never left",
    );
    crate::sched::reclaim_region(*heap).expect("the std heap did not come back");
    wiring.0.release_or_fail(what);
}
