extern crate std;
use std::string::String;
use std::vec::Vec;

use byte_sink_contract::{eof, pack, with_severity};

use super::*;

fn register(log: &mut Log, badge: u32, program: &[u8], user: &[u8]) {
    for (f, n) in [(control::PROGRAM, program), (control::USER, user)] {
        for m in control::name_messages(badge, f, n).into_iter().flatten() {
            log.handle(0, m.0, m.1, m.2, 0);
        }
    }
}

fn reader(log: &mut Log, badge: u32, user: &[u8], system: bool, window: u8) {
    register(log, badge, b"reader", user);
    let scope = if system {
        control::SCOPE_SYSTEM
    } else {
        control::SCOPE_USER
    };
    log.handle(
        0,
        control::word(control::OPERATION_READER, scope, window as u64, badge),
        0,
        0,
        0,
    );
}

fn write(log: &mut Log, badge: u64, bytes: &[u8], now: u64) {
    let mut rest = bytes;
    while !rest.is_empty() {
        let (w0, w1, w2, n) = pack(rest);
        log.handle(badge, w0, w1, w2, now);
        rest = &rest[n..];
    }
}

fn read_all(log: &mut Log, badge: u64, cursor: u64) -> (u64, Vec<String>) {
    let mut window = [0u8; read::WINDOW_BYTES];
    assert!(matches!(
        log.handle(badge, read::request(), cursor, 0, 0),
        Handled::Read { .. }
    ));
    log.fill(badge, cursor, &mut window);
    let (next, len, status) = read::header(&window);
    assert_eq!(status, read::status::OK);
    let text = core::str::from_utf8(&window[read::DATA..read::DATA + len]).unwrap();
    (next, text.lines().map(String::from).collect())
}

/// **Stamping is the service's, not the writer's.** A writer that puts a forged attribution in
/// its text gets it quoted as text; `program`, `user`, `seq`, `time` and `source` come from the
/// badge and the service's own clock and counter.
#[test]
fn the_service_stamps_and_the_writer_only_supplies_text() {
    let mut log = Log::new();
    register(&mut log, 3, b"honest", b"alice");
    reader(&mut log, 9, b"", true, 0);
    write(
        &mut log,
        3,
        b"{\"program\":\"login\",\"user\":\"root\"}\n",
        777,
    );
    let (_, lines) = read_all(&mut log, 9, 0);
    assert_eq!(
        lines,
        [concat!(
            r#"{"seq":0,"time":777,"source":3,"program":"honest","user":"alice","#,
            r#""severity":"info","msg":"{\"program\":\"login\",\"user\":\"root\"}"}"#
        )]
    );
}

/// Severity: the writer's framing bits when it set them, the registered stream's inference
/// when it did not, and `info` for a badge nobody registered a stream for.
#[test]
fn severity_is_the_writers_bits_or_the_streams_inference() {
    let mut log = Log::new();
    register(&mut log, 1, b"out", b"");
    register(&mut log, 2, b"diag", b"");
    log.handle(
        0,
        control::word(control::OPERATION_STREAM, 0, severity::WARNING as u64, 2),
        0,
        0,
        0,
    );
    reader(&mut log, 9, b"", true, 0);
    write(&mut log, 1, b"plain\n", 1);
    write(&mut log, 2, b"inferred\n", 2);
    let (w0, w1, w2, _) = pack(b"explicit\n");
    log.handle(2, with_severity(w0, severity::ERROR), w1, w2, 3);
    let (_, lines) = read_all(&mut log, 9, 0);
    assert!(lines[0].contains(r#""severity":"info","msg":"plain""#));
    assert!(lines[1].contains(r#""severity":"warning","msg":"inferred""#));
    assert!(lines[2].contains(r#""severity":"err","msg":"explicit""#));
}

/// **Two writers interleaved chunk by chunk still make whole lines each**, which is the
/// property milestone 342 needs this service for: the console sees lines, never a splice.
#[test]
fn interleaved_writers_produce_whole_lines() {
    let mut log = Log::new();
    register(&mut log, 1, b"a", b"");
    register(&mut log, 2, b"b", b"");
    reader(&mut log, 9, b"", true, 0);
    let one = b"the first writer's line is longer than one chunk\n";
    let two = b"and so is the second writer's, by a margin\n";
    let (mut x, mut y) = (&one[..], &two[..]);
    while !x.is_empty() || !y.is_empty() {
        for (badge, rest) in [(1u64, &mut x), (2, &mut y)] {
            if !rest.is_empty() {
                let (w0, w1, w2, n) = pack(rest);
                log.handle(badge, w0, w1, w2, 5);
                *rest = &rest[n..];
            }
        }
    }
    let (_, lines) = read_all(&mut log, 9, 0);
    assert_eq!(lines.len(), 2);
    assert!(
        lines
            .iter()
            .any(|l| l.ends_with(r#""msg":"the first writer's line is longer than one chunk"}"#))
    );
    assert!(
        lines
            .iter()
            .any(|l| l.ends_with(r#""msg":"and so is the second writer's, by a margin"}"#))
    );
}

/// The per-user read is a filter over the one ring: a per-user reader sees its own user's
/// records and nobody else's, and the system reader sees all of them.
#[test]
fn a_per_user_read_filters_the_one_ring() {
    let mut log = Log::new();
    register(&mut log, 1, b"w", b"alice");
    register(&mut log, 2, b"w", b"bob");
    reader(&mut log, 8, b"alice", false, 1);
    reader(&mut log, 9, b"", true, 0);
    for i in 0..6u64 {
        write(&mut log, 1 + i % 2, b"line\n", i);
    }
    let (_, alice) = read_all(&mut log, 8, 0);
    let (next, all) = read_all(&mut log, 9, 0);
    assert_eq!(alice.len(), 3);
    assert!(alice.iter().all(|l| l.contains(r#""user":"alice""#)));
    assert_eq!(all.len(), 6);
    assert_eq!(next, 6);
    assert_eq!(
        read_all(&mut log, 9, next).1.len(),
        0,
        "a caught-up reader gets nothing"
    );
}

/// Reading is a registered authority: a writer's badge asking for records is refused, and so
/// is a badge nobody registered.
#[test]
fn only_a_registered_reader_reads() {
    let mut log = Log::new();
    register(&mut log, 1, b"w", b"");
    assert_eq!(log.handle(1, read::request(), 0, 0, 0), Handled::Refused);
    assert_eq!(log.handle(77, read::request(), 0, 0, 0), Handled::Refused);
    let mut window = [0u8; read::WINDOW_BYTES];
    log.fill(1, 0, &mut window);
    assert_eq!(read::header(&window).2, read::status::REFUSED);
}

/// Control is badge 0's alone: the same words from a writer's badge are refused and change
/// nothing, which is what keeps a writer from renaming itself.
#[test]
fn a_writer_cannot_register_itself() {
    let mut log = Log::new();
    register(&mut log, 1, b"real", b"");
    reader(&mut log, 9, b"", true, 0);
    for m in control::name_messages(1, control::PROGRAM, b"forged")
        .into_iter()
        .flatten()
    {
        log.handle(1, m.0, m.1, m.2, 0);
    }
    write(&mut log, 1, b"x\n", 0);
    let (_, lines) = read_all(&mut log, 9, 0);
    assert!(lines[0].contains(r#""program":"real""#));
    assert!(log.refused > 0);
}

/// End of stream flushes a writer's partial line, flagged as cut, and a line longer than the
/// text bound becomes two records rather than one silently truncated.
#[test]
fn eof_flushes_and_a_long_line_is_split_not_truncated() {
    let mut log = Log::new();
    register(&mut log, 1, b"w", b"");
    reader(&mut log, 9, b"", true, 0);
    write(&mut log, 1, b"no newline", 0);
    log.handle(1, eof(), 0, 0, 1);
    let long = [b'x'; record::TEXT_MAX + 10];
    write(&mut log, 1, &long, 2);
    write(&mut log, 1, b"\n", 3);
    let (_, lines) = read_all(&mut log, 9, 0);
    assert_eq!(lines.len(), 3);
    assert!(lines[0].ends_with(r#""msg":"no newline","cut":true}"#));
    assert!(lines[1].contains(r#""cut":true"#));
    assert!(lines[2].ends_with(r#""msg":"xxxxxxxxxx"}"#));
}

/// **The ring is 64 KiB, oldest out, and a reader who fell behind is told what it lost**: one
/// dropped-count line for the gap, then the records still held, in order.
#[test]
fn overflow_evicts_the_oldest_and_a_late_reader_gets_one_dropped_line() {
    let mut log = Log::new();
    register(&mut log, 1, b"w", b"");
    reader(&mut log, 9, b"", true, 0);
    let line = [b'y'; 150];
    let mut total = 0u64;
    while log.store.ring.first_seq() == 0 {
        write(&mut log, 1, &line, total);
        write(&mut log, 1, b"\n", total);
        total += 1;
    }
    let first = log.store.ring.first_seq();
    let (next, lines) = read_all(&mut log, 9, 0);
    assert_eq!(
        lines[0],
        std::format!(r#"{{"dropped":{first},"from":0,"to":{}}}"#, first - 1)
    );
    assert!(lines[1].starts_with(&std::format!(r#"{{"seq":{first},"#)));
    assert!(next > first);
    const { assert!(ring::RING_BYTES == 64 * 1024) };
}

/// A forgotten badge's later lines are from nobody: no program, no user, still stamped with
/// the badge itself, so a reader can tell an unknown writer from a kernel line.
#[test]
fn a_forgotten_badge_is_nobody() {
    let mut log = Log::new();
    register(&mut log, 1, b"gone", b"alice");
    reader(&mut log, 9, b"", true, 0);
    log.handle(
        0,
        control::word(control::OPERATION_FORGET, 0, 0, 1),
        0,
        0,
        0,
    );
    write(&mut log, 1, b"late\n", 0);
    let (_, lines) = read_all(&mut log, 9, 0);
    assert!(lines[0].starts_with(r#"{"seq":0,"time":0,"source":1,"program":null,"user":null,"#));
}

// ---- survivor triage of the 2026-10-03 census ---------------------------------------------

/// A message that is not text is counted and dropped, each time.
#[test]
fn a_message_of_another_kind_is_counted_every_time() {
    let mut log = Log::new();
    let other_kind = 1u64 << byte_sink_contract::KIND_SHIFT;
    log.handle(7, other_kind, 0, 0, 0);
    assert_eq!(log.refused, 1);
    log.handle(7, other_kind, 0, 0, 0);
    assert_eq!(log.refused, 2);
}

/// Each control word is refused for the one thing wrong with it, and a good one is not counted.
#[test]
fn a_control_word_is_refused_for_each_thing_that_can_be_wrong_with_it() {
    let mut log = Log::new();
    register(&mut log, 5, b"p", b"u");
    assert_eq!(log.refused, 0, "registering is not a refusal");
    let mut refused = 0;
    let mut try_word = |log: &mut Log, w: u64, bad: bool| {
        log.handle(0, w, 0, 0, 0);
        refused += u64::from(bad);
        assert_eq!(log.refused, refused, "{w:#x}");
    };
    // Badge 0 is the spawner itself, and none of the four operations may name it.
    for operation in [
        control::OPERATION_NAME,
        control::OPERATION_STREAM,
        control::OPERATION_READER,
        control::OPERATION_FORGET,
    ] {
        try_word(&mut log, control::word(operation, 0, 0, 0), true);
    }
    // Past the last argument or field each operation takes.
    try_word(
        &mut log,
        control::word(control::OPERATION_NAME, 0, 2, 5),
        true,
    );
    try_word(
        &mut log,
        control::word(control::OPERATION_NAME, control::USER + 1, 0, 5),
        true,
    );
    try_word(
        &mut log,
        control::word(control::OPERATION_STREAM, 0, severity::DEBUG as u64 + 1, 5),
        true,
    );
    try_word(
        &mut log,
        control::word(control::OPERATION_READER, control::SCOPE_SYSTEM + 1, 0, 5),
        true,
    );
    try_word(&mut log, control::word(0x7f, 0, 0, 5), true);
    // And the edges that are allowed.
    try_word(
        &mut log,
        control::word(control::OPERATION_NAME, control::USER, 0, 5),
        false,
    );
    try_word(
        &mut log,
        control::word(control::OPERATION_STREAM, 0, severity::DEBUG as u64, 5),
        false,
    );
    try_word(
        &mut log,
        control::word(control::OPERATION_READER, control::SCOPE_SYSTEM, 0, 5),
        false,
    );
    try_word(
        &mut log,
        control::word(control::OPERATION_FORGET, 0, 0, 5),
        false,
    );
}

/// A record handed straight to the store is held and read back, and counted.
#[test]
fn an_ingested_record_is_held_and_readable() {
    let mut log = Log::new();
    reader(&mut log, 9, b"", true, 0);
    assert_eq!(log.held(), 0);
    let h = Header {
        seq: 99,
        time: 5,
        source: 0,
        severity: severity::ERROR,
        flags: record::flags::KERNEL,
        len: 4,
        kind: 0,
    };
    log.ingest(&h, b"boot", Some(Name::new(b"kernel")), None);
    assert_eq!(log.held(), 1);
    log.ingest(&h, b"more", Some(Name::new(b"kernel")), None);
    assert_eq!(log.held(), 2);
    let (_, lines) = read_all(&mut log, 9, 0);
    assert!(lines[0].starts_with(r#"{"seq":0,"time":5,"source":0,"program":"kernel""#));
    assert!(
        lines[1].starts_with(r#"{"seq":1,"#),
        "the service numbers it, not the header"
    );
}
