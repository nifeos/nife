//! Fuzz the system log's **request handler** with whole sessions of writers, the spawner and
//! readers, and check what every read returns (proposal #1592 part a, rank 3).
//!
//! It runs `system_log::Log::handle` and `Log::fill`, the two calls `components/src/system_log.rs`
//! makes per message, with no syscalls between. A session interleaves byte-sink chunks from several
//! writer badges (hostile bytes included), end-of-stream, the spawner's registrations, and reads at
//! chosen cursors. The registry model is this file's own, kept from the control messages sent. The
//! spawner's words are always well formed (`control::name_messages`, `control::word`), because a
//! raw word from badge 0 is the spawner's own authority; writers' words may be raw.

//! # The rules it checks on every read, beyond "no panic"
//!
//! 1. *Framing.* The data is whole lines, each a record or dropped-count line of exactly
//!    `json::render`'s shape, escaped; a writer's bytes never end a string, add a field or a line.
//! 2. *Stamping.* A record's `program` and `user` are names the spawner registered for its `source`
//!    badge in the session, or `null`. A writer never chooses either.
//! 3. *Scope.* A per-user reader is shown only records stamped with its own registered user.
//! 4. *Order.* Sequence numbers rise strictly within a read, start at or after the cursor, and stay
//!    below the next cursor the read hands back.
//!
//! Name: provisional (`lane/fuzz-service-handlers`, 2026-10-04 UTC); an architect names things.
//! Falsification: replayable `fuzz/falsifications/system_log_session.every_reader_is_a_system_reader.patch`

#![no_main]

use std::collections::{HashMap, HashSet};

use byte_sink_contract::{eof, pack, with_severity};
use libfuzzer_sys::arbitrary::{Result as ArbResult, Unstructured};
use libfuzzer_sys::fuzz_target;
use system_log::{Handled, Log};
use system_log_contract::{control, read, severity};

/// Steps per session. The ring is 64 KiB, so a long session also exercises eviction and the
/// dropped-count line.
const MAX_STEPS: usize = 400;

/// Writer badges: 1 to 7 can be registered; the last cannot (it does not fit a control word).
const BADGES: &[u64] = &[1, 2, 3, 4, 5, 6, 7, 1 << 40];

/// Names a registration may carry, including one of exactly sixteen bytes and one past it, so the
/// second chunk is exercised, and the empty name, which renders as `null`.
const NAMES: &[&[u8]] = &[
    b"alpha",
    b"bob",
    b"ann",
    b"",
    b"sixteen_bytes_ab",
    b"a_name_longer_than_sixteen_bytes",
    b"q\"uote",
];

/// Fragments a writer's chunk is made of: ordinary text, newlines, every escape class, broken
/// UTF-8, and pieces of the record's own syntax.
const FRAGMENTS: &[&[u8]] = &[
    b"hello",
    b" ",
    b"\n",
    b"\"",
    b"\\",
    b"\t",
    b"\r",
    b"\x01",
    b"\x7f",
    b"\xff",
    b"\xe2\x82",
    b"\xe2\x82\xac",
    b"\",\"user\":\"ann",
    b"\"}\n{\"seq\":0,",
    b"\\u0000",
];

#[derive(Default, Clone)]
struct Entry {
    program: Vec<u8>,
    user: Vec<u8>,
    reader: Option<(u8, bool)>,
}

#[derive(Default)]
struct Model {
    entries: HashMap<u64, Entry>,
    /// Every name ever registered per badge and field, which is what a record may be stamped with.
    programs: HashMap<u64, HashSet<Vec<u8>>>,
    users: HashMap<u64, HashSet<Vec<u8>>>,
    /// The last cursor each reader was handed, so a session can read on from it.
    cursors: HashMap<u64, u64>,
}

fn session(u: &mut Unstructured) -> ArbResult<()> {
    let mut log = Log::new();
    let mut model = Model::default();
    let mut window = vec![0u8; read::WINDOW_BYTES];
    let mut now = 0u64;

    for _ in 0..MAX_STEPS {
        if u.is_empty() {
            break;
        }
        now += u.int_in_range(0..=3)?;
        let badge = BADGES[u.choose_index(BADGES.len())?];
        match u.int_in_range(0u8..=15)? {
            // Writers' bytes, most of the time.
            0..=7 => write_chunk(u, &mut log, badge, now)?,
            8 => {
                log.handle(badge, eof(), 0, 0, now);
            }
            // The spawner names a badge's program or user.
            9 | 10 => name_badge(u, &mut log, &mut model, badge, now)?,
            // The spawner makes a badge a reader, of either scope, at a window.
            11 => make_reader(u, &mut log, &mut model, badge, now)?,
            12 => {
                if !control::badge_fits(badge) {
                    continue;
                }
                log.handle(
                    0,
                    control::word(control::OPERATION_FORGET, 0, 0, badge as u32),
                    0,
                    0,
                    now,
                );
                model.entries.remove(&badge);
            }
            13 => {
                if !control::badge_fits(badge) {
                    continue;
                }
                let level = u.int_in_range(0..=severity::DEBUG as u64)?;
                log.handle(
                    0,
                    control::word(control::OPERATION_STREAM, 0, level, badge as u32),
                    0,
                    0,
                    now,
                );
                model.entries.entry(badge).or_default();
            }
            // A read, at the reader's last cursor, from the start, or anywhere.
            _ => read_and_check(u, &mut log, &mut model, &mut window, badge, now)?,
        }
    }
    Ok(())
}

/// A writer's chunk of up to sixteen bytes built from [`FRAGMENTS`], sometimes with a severity and
/// rarely with a wholly arbitrary first word.
fn write_chunk(u: &mut Unstructured, log: &mut Log, badge: u64, now: u64) -> ArbResult<()> {
    let mut chunk = Vec::new();
    while chunk.len() < 16 && !u.ratio(1, 3)? {
        chunk.extend_from_slice(FRAGMENTS[u.choose_index(FRAGMENTS.len())?]);
    }
    chunk.truncate(16);
    let (mut w0, w1, w2, _) = pack(&chunk);
    if u.ratio(1, 4)? {
        w0 = with_severity(w0, u.int_in_range(0..=7)?);
    }
    if u.ratio(1, 32)? {
        w0 = u.arbitrary()?;
    }
    log.handle(badge, w0, w1, w2, now);
    Ok(())
}

/// The spawner names `badge`'s program or user, and the model records the name as one a record
/// from that badge may now be stamped with.
fn name_badge(
    u: &mut Unstructured,
    log: &mut Log,
    model: &mut Model,
    badge: u64,
    now: u64,
) -> ArbResult<()> {
    if !control::badge_fits(badge) {
        return Ok(());
    }
    let field = if u.arbitrary()? {
        control::PROGRAM
    } else {
        control::USER
    };
    let name = NAMES[u.choose_index(NAMES.len())?];
    for m in control::name_messages(badge as u32, field, name)
        .into_iter()
        .flatten()
    {
        log.handle(0, m.0, m.1, m.2, now);
    }
    let e = model.entries.entry(badge).or_default();
    let kept = name[..name.len().min(control::NAME_MAX)].to_vec();
    let ever = if field == control::PROGRAM {
        e.program = kept.clone();
        &mut model.programs
    } else {
        e.user = kept.clone();
        &mut model.users
    };
    ever.entry(badge).or_default().insert(kept);
    Ok(())
}

/// The spawner makes `badge` a reader of either scope at a window.
fn make_reader(
    u: &mut Unstructured,
    log: &mut Log,
    model: &mut Model,
    badge: u64,
    now: u64,
) -> ArbResult<()> {
    if !control::badge_fits(badge) {
        return Ok(());
    }
    let system = u.arbitrary()?;
    let scope = if system {
        control::SCOPE_SYSTEM
    } else {
        control::SCOPE_USER
    };
    let win = u.int_in_range(0u8..=5)?;
    log.handle(
        0,
        control::word(control::OPERATION_READER, scope, win as u64, badge as u32),
        0,
        0,
        now,
    );
    model.entries.entry(badge).or_default().reader = Some((win, system));
    Ok(())
}

/// A read at the reader's last cursor, from the start, or anywhere: the service must answer a
/// registered reader with its own window and refuse anyone else, and what it fills is held to
/// rules 1 to 4 by [`check`].
fn read_and_check(
    u: &mut Unstructured,
    log: &mut Log,
    model: &mut Model,
    window: &mut [u8],
    badge: u64,
    now: u64,
) -> ArbResult<()> {
    let cursor = match u.int_in_range(0u8..=3)? {
        0 => 0,
        1 => u.arbitrary()?,
        _ => model.cursors.get(&badge).copied().unwrap_or(0),
    };
    let reader = model.entries.get(&badge).and_then(|e| e.reader);
    let got = log.handle(badge, read::request(), cursor, 0, now);
    match (reader, got) {
        (
            Some((win, _)),
            Handled::Read {
                window: w,
                cursor: c,
            },
        ) => {
            assert_eq!(
                (w, c),
                (win, cursor),
                "the read named another window or cursor"
            );
        }
        (None, Handled::Refused) => return Ok(()),
        (r, g) => panic!("badge {badge}: model reader {r:?}, service answered {g:?}"),
    }
    window.fill(0xa5);
    log.fill(badge, cursor, window);
    let next = check(model, badge, cursor, window);
    model.cursors.insert(badge, next);
    Ok(())
}

/// Check one filled window against rules 1 to 4, and return the next cursor it carries.
fn check(model: &Model, badge: u64, cursor: u64, window: &[u8]) -> u64 {
    let (next, len, status) = read::header(window);
    assert_eq!(status, read::status::OK, "a registered reader was refused");
    let data = &window[read::DATA..read::DATA + len];
    assert!(
        data.is_empty() || data.ends_with(b"\n"),
        "a read ended mid-line"
    );
    let entry = &model.entries[&badge];
    let system = entry.reader.unwrap().1;
    let mut last: Option<u64> = None;
    for (i, line) in data.split_inclusive(|&b| b == b'\n').enumerate() {
        let text = String::from_utf8_lossy(line);
        let mut p = Parser { s: line, at: 0 };
        let rec = p
            .line()
            .unwrap_or_else(|| panic!("malformed line at byte {}: {text}", p.at));
        match rec {
            Line::Dropped { from, to, count } => {
                assert_eq!(i, 0, "a dropped-count line after a record: {text}");
                assert_eq!(from, cursor, "{text}");
                assert_eq!(count, to + 1 - from, "{text}");
            }
            Line::Record {
                seq,
                source,
                program,
                user,
            } => {
                assert!(seq >= cursor, "seq {seq} before the cursor {cursor}");
                assert!(seq < next, "seq {seq} at or past the next cursor {next}");
                assert!(
                    last.is_none_or(|l| seq > l),
                    "seq {seq} out of order: {text}"
                );
                last = Some(seq);
                for (field, got, ever) in [
                    ("program", &program, &model.programs),
                    ("user", &user, &model.users),
                ] {
                    if let Some(name) = got {
                        assert!(
                            ever.get(&source).is_some_and(|s| s.contains(name)),
                            "{field} {:?} was never registered for source {source}: {text}",
                            String::from_utf8_lossy(name)
                        );
                    }
                }
                if !system {
                    let mine = Some(&entry.user).filter(|u| !u.is_empty());
                    assert_eq!(
                        user.as_ref(),
                        mine,
                        "a per-user reader saw another user's record: {text}"
                    );
                }
            }
        }
    }
    next
}

enum Line {
    Record {
        seq: u64,
        source: u64,
        program: Option<Vec<u8>>,
        user: Option<Vec<u8>>,
    },
    Dropped {
        count: u64,
        from: u64,
        to: u64,
    },
}

/// A parser for exactly the two line shapes `system_log::json` writes, and nothing more lenient.
struct Parser<'a> {
    s: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn lit(&mut self, l: &[u8]) -> Option<()> {
        self.s[self.at..].starts_with(l).then(|| self.at += l.len())
    }

    fn num(&mut self) -> Option<u64> {
        let start = self.at;
        while self.s.get(self.at).is_some_and(u8::is_ascii_digit) {
            self.at += 1;
        }
        std::str::from_utf8(&self.s[start..self.at])
            .ok()?
            .parse()
            .ok()
    }

    /// A JSON string, decoded to bytes. Only the escapes the renderer writes are accepted, and no
    /// raw control byte.
    fn string(&mut self) -> Option<Vec<u8>> {
        self.lit(b"\"")?;
        let mut out = Vec::new();
        loop {
            let b = *self.s.get(self.at)?;
            self.at += 1;
            match b {
                b'"' => return Some(out),
                b'\\' => {
                    let e = *self.s.get(self.at)?;
                    self.at += 1;
                    match e {
                        b'"' | b'\\' => out.push(e),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let hex =
                                std::str::from_utf8(self.s.get(self.at..self.at + 4)?).ok()?;
                            let c = char::from_u32(u32::from_str_radix(hex, 16).ok()?)?;
                            self.at += 4;
                            out.extend_from_slice(c.to_string().as_bytes());
                        }
                        _ => return None,
                    }
                }
                0..0x20 | 0x7f => return None,
                _ => out.push(b),
            }
        }
    }

    fn opt_string(&mut self) -> Option<Option<Vec<u8>>> {
        if self.lit(b"null").is_some() {
            Some(None)
        } else {
            self.string().map(Some)
        }
    }

    fn line(&mut self) -> Option<Line> {
        if self.lit(b"{\"dropped\":").is_some() {
            let count = self.num()?;
            self.lit(b",\"from\":")?;
            let from = self.num()?;
            self.lit(b",\"to\":")?;
            let to = self.num()?;
            self.lit(b"}\n")?;
            return (self.at == self.s.len()).then_some(Line::Dropped { count, from, to });
        }
        self.lit(b"{\"seq\":")?;
        let seq = self.num()?;
        self.lit(b",\"time\":")?;
        self.num()?;
        self.lit(b",\"source\":")?;
        let source = self.num()?;
        self.lit(b",\"program\":")?;
        let program = self.opt_string()?;
        self.lit(b",\"user\":")?;
        let user = self.opt_string()?;
        self.lit(b",\"severity\":")?;
        let level = self.string()?;
        (0..=severity::DEBUG)
            .any(|l| severity::name(l).as_bytes() == level)
            .then_some(())?;
        self.lit(b",\"msg\":")?;
        let msg = self.string()?;
        (msg.len() <= 6 * system_log::record::TEXT_MAX).then_some(())?;
        let _ = self.lit(b",\"cut\":true");
        let _ = self.lit(b",\"dropped_before\":true");
        self.lit(b"}\n")?;
        (self.at == self.s.len()).then_some(Line::Record {
            seq,
            source,
            program,
            user,
        })
    }
}

fuzz_target!(|data: &[u8]| {
    let _ = session(&mut Unstructured::new(data));
});
