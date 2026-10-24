//! **The system log's wire contracts**: milestone 613 (a system log service: the in-memory half),
//! for §242 (a system log). Three things two programs agree on (AGENTS.md rule 7):
//!
//! - [`record`]: F3, the binary record, the transport and never the stored form (§242, Question 3).
//! - [`control`]: what the spawner that minted a writer's badge tells the service about it, through
//!   the unbadged intake capability, which only the spawner holds.
//! - [`read`]: how a reader asks for records, and the shared page they land in as JSONL.
//!
//! A writer speaks the byte sink (`crates/byte_sink_contract`), not this crate. See
//! notes/system-log.md, "Attribution".

//! # BUGS
//!
//! - Badges are 32 bits here, and the kernel's are 64: [`control::badge_fits`] refuses the rest.
//! - A name is at most [`control::NAME_MAX`] bytes, and a longer one is cut at registration.
//!
//! Name: ratified 2026-10-18 (calef): "Yes". A wire contract for the system log.

#![no_std]

/// Syslog's eight severity levels (RFC 5424, section 6.2.1), numbered as it numbers them: 0 is the
/// most severe. §242 Question 3 ruled these rather than a scheme of the tree's own.
pub mod severity {
    /// System is unusable.
    pub const EMERGENCY: u8 = 0;
    /// Action must be taken immediately.
    pub const ALERT: u8 = 1;
    /// Critical conditions.
    pub const CRITICAL: u8 = 2;
    /// Error conditions.
    pub const ERROR: u8 = 3;
    /// Warning conditions. What a declared diagnostics stream infers (§242, Question 4).
    pub const WARNING: u8 = 4;
    /// Normal but significant condition.
    pub const NOTICE: u8 = 5;
    /// Informational. What ordinary output infers (§242, Question 4), and what a badge nobody
    /// registered a stream for gets.
    pub const INFO: u8 = 6;
    /// Debug-level messages.
    pub const DEBUG: u8 = 7;

    /// The keyword a JSONL record spells a level with: syslog's own (`logger -p user.err`), so a
    /// reader who knows syslog reads these without a table. Past 7 is `debug`, the clamp
    /// `byte_sink_contract::with_severity` makes.
    pub const fn name(level: u8) -> &'static str {
        match level {
            EMERGENCY => "emerg",
            ALERT => "alert",
            CRITICAL => "crit",
            ERROR => "err",
            WARNING => "warning",
            NOTICE => "notice",
            INFO => "info",
            _ => "debug",
        }
    }
}

/// **F3, the binary record** (§242, Question 4): a fixed header, then text. The header plus
/// [`record::TEXT_MAX`] is 256 bytes, Zircon's debuglog record's total.
///
/// | bytes | field | written by |
/// |---|---|---|
/// | 0..8 | `seq`, little-endian | the service: one counter for every record it has accepted |
/// | 8..16 | `time`, boot-relative nanoseconds | the service, at the moment the line completed |
/// | 16..24 | `source`, the writer's badge (0 for the kernel) | the kernel, through the badge |
/// | 24 | `severity`, syslog's level | the writer's framing bits, or inferred |
/// | 25 | `flags` ([`record::flags`]) | the service |
/// | 26..28 | `len`, the text's length | the service |
/// | 28 | `kind`, the body kind (`byte_sink_contract::KIND_TEXT`) | the writer's framing bits |
/// | 29..32 | zero | reserved, and refused when not zero |
pub mod record {
    /// The header's length. F1's 32 bytes with one spare byte spent on `kind`.
    pub const HEADER_LEN: usize = 32;

    /// The most text one record carries. §242 measured it against a whole xenon boot (longest
    /// kernel line 123 bytes) and an eight-hour radon soak (99% of lines fit).
    pub const TEXT_MAX: usize = 224;

    /// Header and text together, at most.
    pub const RECORD_MAX: usize = HEADER_LEN + TEXT_MAX;

    /// The `flags` byte's bits.
    pub mod flags {
        /// The kernel wrote this record (milestone 342's ring). Its `source` is 0 and it belongs
        /// to no user.
        pub const KERNEL: u8 = 1 << 0;
        /// Records were lost immediately before this one (a kernel ring that overflowed before it
        /// was drained).
        pub const DROPPED_BEFORE: u8 = 1 << 1;
        /// The line was longer than [`super::TEXT_MAX`] and this record is its first part; or the
        /// writer ended (or was crowded out) mid-line.
        pub const CUT: u8 = 1 << 2;
        /// The kernel already wrote this line to the UART itself (before a drainer attached, in a
        /// fallback, or in a panic's flush), so the log service stores it and does not forward it.
        pub const DIRECT: u8 = 1 << 3;
    }

    /// One record's header, decoded.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Header {
        /// The service's sequence number for this record.
        pub seq: u64,
        /// Boot-relative nanoseconds.
        pub time: u64,
        /// The writer's badge, or 0 for the kernel.
        pub source: u64,
        /// Syslog's level, 0 to 7 ([`crate::severity`]).
        pub severity: u8,
        /// [`flags`].
        pub flags: u8,
        /// How many text bytes follow, at most [`TEXT_MAX`].
        pub len: u16,
        /// The body kind; 0 is text.
        pub kind: u8,
    }

    impl Header {
        /// The 32 bytes on the wire.
        pub fn encode(&self) -> [u8; HEADER_LEN] {
            let mut b = [0u8; HEADER_LEN];
            b[0..8].copy_from_slice(&self.seq.to_le_bytes());
            b[8..16].copy_from_slice(&self.time.to_le_bytes());
            b[16..24].copy_from_slice(&self.source.to_le_bytes());
            b[24] = self.severity;
            b[25] = self.flags;
            b[26..28].copy_from_slice(&self.len.to_le_bytes());
            b[28] = self.kind;
            b
        }

        /// Decode a header, or `None` when it cannot be one: a length past [`TEXT_MAX`] (obeying it
        /// would be a read past the record), a level past 7, or a reserved byte that is not zero.
        /// Refused rather than repaired, for `byte_sink_contract::Msg::Malformed`'s reason.
        pub fn decode(b: &[u8; HEADER_LEN]) -> Option<Header> {
            let word = |i: usize| {
                let mut w = [0u8; 8];
                w.copy_from_slice(&b[i..i + 8]);
                u64::from_le_bytes(w)
            };
            let len = u16::from_le_bytes([b[26], b[27]]);
            if len as usize > TEXT_MAX || b[24] > 7 || b[29..32] != [0, 0, 0] {
                return None;
            }
            Some(Header {
                seq: word(0),
                time: word(8),
                source: word(16),
                severity: b[24],
                flags: b[25],
                len,
                kind: b[28],
            })
        }
    }
}

/// **What a spawner tells the service about a badge it minted.** Plain `SEND`s on the intake
/// endpoint through the unbadged capability, which only the minter holds: the service reads a
/// badge-0 message as control and nothing else can produce one.
///
/// Every control word is `operation << 56 | field << 40 | arg << 32 | badge`, a badge in the low 32 bits.
/// The opcodes start at 0x10 so none is a byte-sink opcode, which keeps "a writer's bytes" and "the
/// spawner's word about a writer" distinct on one endpoint even before the badge is looked at.
pub mod control {
    /// **Name a badge's program or user.** `arg` is the chunk (0 or 1), `field` is [`PROGRAM`] or
    /// [`USER`], and `w1`/`w2` carry sixteen bytes of the name, NUL-padded. Chunk 0 resets the
    /// field, so a re-registration cannot leave a stale tail. [`name_messages`] builds them.
    pub const OPERATION_NAME: u64 = 0x10;
    /// **Set a writer badge's inferred severity**: `arg` is the syslog level a line gets when its
    /// writer set none. A spawner sends [`crate::severity::WARNING`] for a declared diagnostics
    /// stream and nothing (the default, [`crate::severity::INFO`]) for ordinary output.
    pub const OPERATION_STREAM: u64 = 0x11;
    /// **Make a badge a reader**: `arg` is its window (0 to the service's window count minus 1),
    /// and `field` is [`SCOPE_SYSTEM`] (every record) or [`SCOPE_USER`] (only records stamped with
    /// this badge's own registered user). The spawner decides which, because the per-user read is
    /// a lesser authority than the system read (§242, Question 2).
    pub const OPERATION_READER: u64 = 0x12;
    /// **Forget a badge**: its process is gone. Records already stamped keep its names; a later
    /// message on the badge is from nobody the service knows.
    pub const OPERATION_FORGET: u64 = 0x13;

    /// [`OPERATION_NAME`]'s field: the program.
    pub const PROGRAM: u64 = 0;
    /// [`OPERATION_NAME`]'s field: the user.
    pub const USER: u64 = 1;
    /// [`OPERATION_READER`]'s field: the system read.
    pub const SCOPE_SYSTEM: u64 = 1;
    /// [`OPERATION_READER`]'s field: the per-user read.
    pub const SCOPE_USER: u64 = 0;

    /// The longest name a registration carries: two chunks of sixteen.
    pub const NAME_MAX: usize = 32;

    /// A control word.
    pub const fn word(operation: u64, field: u64, arg: u64, badge: u32) -> u64 {
        (operation << 56) | ((field & 0xff) << 40) | ((arg & 0xff) << 32) | badge as u64
    }

    /// A control word's `(operation, field, arg, badge)`.
    pub const fn fields(w0: u64) -> (u64, u64, u64, u32) {
        (
            w0 >> 56,
            (w0 >> 40) & 0xff,
            (w0 >> 32) & 0xff,
            (w0 & 0xffff_ffff) as u32,
        )
    }

    /// Whether `badge` can be registered at all (see the crate's BUGS): nonzero, and 32 bits.
    pub const fn badge_fits(badge: u64) -> bool {
        badge != 0 && badge <= u32::MAX as u64
    }

    /// The [`OPERATION_NAME`] messages naming `badge`'s `field` as `name` (cut at [`NAME_MAX`]): one when
    /// it fits in sixteen bytes, two otherwise. Unused entries are `None`.
    pub fn name_messages(badge: u32, field: u64, name: &[u8]) -> [Option<(u64, u64, u64)>; 2] {
        let name = &name[..name.len().min(NAME_MAX)];
        let chunk = |i: usize| {
            let mut b = [0u8; 16];
            let part = name.get(i * 16..).unwrap_or(&[]);
            let n = part.len().min(16);
            b[..n].copy_from_slice(&part[..n]);
            let mut lo = [0u8; 8];
            let mut hi = [0u8; 8];
            lo.copy_from_slice(&b[..8]);
            hi.copy_from_slice(&b[8..]);
            (
                word(OPERATION_NAME, field, i as u64, badge),
                u64::from_le_bytes(lo),
                u64::from_le_bytes(hi),
            )
        };
        [
            Some(chunk(0)),
            if name.len() > 16 {
                Some(chunk(1))
            } else {
                None
            },
        ]
    }
}

/// **How a reader reads**: it `SEND`s [`read::OPERATION_READ`] and a cursor through its own badged
/// copy, and the service fills that badge's one-page window, laid out below, with whole JSONL lines
/// and signals the reader's notification. A reader that stops reading cannot hold up a writer
/// (§242's "dropping rather than letting a writer wait").
///
/// | bytes | field |
/// |---|---|
/// | 0..8 | the next cursor: pass it to the next read |
/// | 8..12 | how many bytes of JSONL follow |
/// | 12..16 | [`read::status`] |
/// | [`read::DATA`].. | the lines, each ending in `\n` |
pub mod read {
    /// A read request's opcode, in the first word's top byte; the cursor rides in the second word.
    /// A cursor is a sequence number: the first record wanted. 0 asks for everything still held.
    pub const OPERATION_READ: u64 = 0x20;

    /// The window's size: one page.
    pub const WINDOW_BYTES: usize = 4096;
    /// Where the service maps window 0; window *n* is [`WINDOW_BYTES`] times *n* above it. A
    /// pair-page address (`address_space_map::pair_page`), agreed between the service and the
    /// spawner that maps the same frames into its readers.
    pub const WINDOW_VA: u64 = 0x0060_0000;
    /// How many reader windows the service maps, and so how many readers it can answer.
    pub const WINDOWS: usize = 4;
    /// Where the lines start in a window.
    pub const DATA: usize = 16;
    /// The most JSONL one read returns.
    pub const DATA_MAX: usize = WINDOW_BYTES - DATA;

    /// A read's outcome, at window bytes 12..16.
    pub mod status {
        /// The lines are there (possibly none: the reader is caught up).
        pub const OK: u32 = 0;
        /// The badge is not a registered reader, or its window is not one the service maps.
        /// Nothing was written except this header, and nobody was signalled: the service cannot
        /// know whom to signal. Kept for a window a later registration fixes.
        pub const REFUSED: u32 = 1;
    }

    /// The first word of a read request.
    pub const fn request() -> u64 {
        OPERATION_READ << 56
    }

    /// Write a window's header.
    pub fn put_header(window: &mut [u8], next: u64, len: u32, status: u32) {
        window[0..8].copy_from_slice(&next.to_le_bytes());
        window[8..12].copy_from_slice(&len.to_le_bytes());
        window[12..16].copy_from_slice(&status.to_le_bytes());
    }

    /// Read a window's header: `(next, len, status)`. `len` is clamped to [`DATA_MAX`], because
    /// the page is shared and its bytes are only as honest as whoever wrote them last.
    pub fn header(window: &[u8]) -> (u64, usize, u32) {
        let mut next = [0u8; 8];
        next.copy_from_slice(&window[0..8]);
        let mut len = [0u8; 4];
        len.copy_from_slice(&window[8..12]);
        let mut status = [0u8; 4];
        status.copy_from_slice(&window[12..16]);
        (
            u64::from_le_bytes(next),
            (u32::from_le_bytes(len) as usize).min(DATA_MAX),
            u32::from_le_bytes(status),
        )
    }
}

pub mod kernel_ring {
    //! **The kernel's ring, as the kernel and the log service share it** (milestone 342 (the
    //! kernel and the `console` server drive one UART from two address spaces), calef's ruling F of
    //! 2026-10-03 UTC on pull request #1498): the ring of [`SLOTS`] F3 records the kernel writes, a
    //! cursor page the service writes, and a notification the kernel signals.
    //!
    //! Every word is an `AtomicU64`, and a slot is a seqlock whose sequence word is the record's
    //! `seq`: the writer stores [`BUSY`], then the body, then the `seq`, and a reader that sees the
    //! same `seq` before and after its copy has a record nobody touched. See notes/system-log.md,
    //! "The kernel ring".

    //! # BUGS
    //!
    //! - 63 records, not the 180 lines §242 (a system log) sized 16 KiB for; notes/system-log.md.

    use core::sync::atomic::{AtomicU64, Ordering, fence};

    use super::record::{Header, TEXT_MAX};

    /// The ring's frames: §242 Question 5's 16 KiB.
    pub const PAGES: usize = 4;
    /// The ring's size in bytes.
    pub const BYTES: usize = PAGES * 4096;
    /// The ring's size in words, which is how both sides address it.
    pub const WORDS: usize = BYTES / 8;
    /// One slot: an F3 header and the most text it carries.
    pub const SLOT_WORDS: usize = super::record::RECORD_MAX / 8;
    /// How many records the ring holds. The first slot's worth of words is the header.
    pub const SLOTS: u64 = (WORDS / SLOT_WORDS - 1) as u64;
    /// The header's first word once the kernel has formatted the ring.
    pub const MAGIC: u64 = u64::from_le_bytes(*b"nifeklog");
    /// A slot's sequence word while the kernel is writing it.
    pub const BUSY: u64 = u64::MAX;
    /// A slot's sequence word before anything was written to it.
    pub const EMPTY: u64 = u64::MAX - 1;
    /// The cursor page's word before any drainer has written it.
    pub const DETACHED: u64 = u64::MAX;
    /// The bit the kernel signals on the notification after an append.
    pub const NOTIFY_BIT: u64 = 1;

    const W_MAGIC: usize = 0;
    const W_NEXT: usize = 1;
    const W_FALLBACK: usize = 2;
    const W_SLOTS: usize = 3;
    const TEXT_WORDS: usize = TEXT_MAX / 8;

    /// What [`Ring::read`] found.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Read {
        /// The record, and its text's length in the caller's buffer.
        Record(Header, usize),
        /// The kernel has written past it: the record is gone.
        Overwritten,
        /// Not written yet.
        NotYet,
    }

    /// The ring, over the words either side maps.
    pub struct Ring<'a> {
        w: &'a [AtomicU64],
    }

    impl<'a> Ring<'a> {
        /// A view of `words`, or `None` when there are too few of them.
        pub fn new(words: &'a [AtomicU64]) -> Option<Ring<'a>> {
            (words.len() >= WORDS).then_some(Ring { w: words })
        }

        /// **Writer only.** Lay out an empty ring.
        pub fn format(&self) {
            for s in 0..SLOTS {
                self.w[slot(s)].store(EMPTY, Ordering::Relaxed);
            }
            self.w[W_NEXT].store(0, Ordering::Relaxed);
            self.w[W_FALLBACK].store(0, Ordering::Relaxed);
            self.w[W_SLOTS].store(SLOTS, Ordering::Relaxed);
            self.w[W_MAGIC].store(MAGIC, Ordering::Release);
        }

        /// Whether the kernel has formatted this ring (a reader's first question).
        pub fn is_formatted(&self) -> bool {
            self.w[W_MAGIC].load(Ordering::Acquire) == MAGIC
                && self.w[W_SLOTS].load(Ordering::Relaxed) == SLOTS
        }

        /// The sequence number the next record will take.
        pub fn next_seq(&self) -> u64 {
            self.w[W_NEXT].load(Ordering::Acquire)
        }

        /// The oldest sequence number the ring can still hold.
        pub fn oldest(&self) -> u64 {
            self.next_seq().saturating_sub(SLOTS)
        }

        /// How many lines the kernel wrote to the UART itself after a drainer had attached,
        /// because the drainer was not keeping up.
        pub fn fallback(&self) -> u64 {
            self.w[W_FALLBACK].load(Ordering::Relaxed)
        }

        /// **Writer only.** Count one fallback line.
        pub fn count_fallback(&self) {
            self.w[W_FALLBACK].fetch_add(1, Ordering::Relaxed);
        }

        /// **Writer only.** Append a record and return its sequence number. `text` past
        /// [`TEXT_MAX`] is cut; the caller splits long lines.
        pub fn append(&self, time: u64, severity: u8, flags: u8, text: &[u8]) -> u64 {
            let seq = self.w[W_NEXT].load(Ordering::Relaxed);
            let base = slot(seq);
            let len = text.len().min(TEXT_MAX);
            self.w[base].store(BUSY, Ordering::Relaxed);
            // PAIR: `Ring::read`'s fence(Acquire) before its second look at the slot: the BUSY
            // store above is ordered before the body stores below, so a copy that overlaps them
            // fails its check.
            fence(Ordering::Release);
            self.w[base + 1].store(time, Ordering::Relaxed);
            self.w[base + 2].store(0, Ordering::Relaxed);
            self.w[base + 3].store(meta(severity, flags, len), Ordering::Relaxed);
            for i in 0..TEXT_WORDS {
                let mut b = [0u8; 8];
                let from = (i * 8).min(len);
                let to = (i * 8 + 8).min(len);
                b[..to - from].copy_from_slice(&text[from..to]);
                self.w[base + 4 + i].store(u64::from_le_bytes(b), Ordering::Relaxed);
            }
            self.w[base].store(seq, Ordering::Release);
            self.w[W_NEXT].store(seq + 1, Ordering::Release);
            seq
        }

        /// **Writer only.** OR `flags` into record `seq`'s flags, if it is still in the ring.
        pub fn add_flags(&self, seq: u64, flags: u8) -> bool {
            let base = slot(seq);
            if self.w[base].load(Ordering::Relaxed) != seq {
                return false;
            }
            self.w[base].store(BUSY, Ordering::Relaxed);
            // PAIR: `Ring::read`'s fence(Acquire): a reader that copied across this flag change
            // sees BUSY or nothing changed, never half of it.
            fence(Ordering::Release);
            let m = self.w[base + 3].load(Ordering::Relaxed);
            self.w[base + 3].store(m | ((flags as u64) << 8), Ordering::Relaxed);
            self.w[base].store(seq, Ordering::Release);
            true
        }

        /// Copy record `seq` out: its header, and its text into `out`.
        pub fn read(&self, seq: u64, out: &mut [u8; TEXT_MAX]) -> Read {
            let next = self.next_seq();
            if seq >= next {
                return Read::NotYet;
            }
            if seq + SLOTS < next {
                return Read::Overwritten;
            }
            let base = slot(seq);
            if self.w[base].load(Ordering::Acquire) != seq {
                return Read::Overwritten;
            }
            let time = self.w[base + 1].load(Ordering::Relaxed);
            let source = self.w[base + 2].load(Ordering::Relaxed);
            let m = self.w[base + 3].load(Ordering::Relaxed);
            for i in 0..TEXT_WORDS {
                let v = self.w[base + 4 + i].load(Ordering::Relaxed);
                out[i * 8..i * 8 + 8].copy_from_slice(&v.to_le_bytes());
            }
            // PAIR: `Ring::append`'s fence(Release) after its BUSY store: if the re-read below
            // still sees `seq`, no rewrite of this slot began before the copy ended (the seqlock
            // reader's half).
            fence(Ordering::Acquire);
            if self.w[base].load(Ordering::Relaxed) != seq {
                return Read::Overwritten;
            }
            let len = (((m >> 16) & 0xffff) as usize).min(TEXT_MAX);
            Read::Record(
                Header {
                    seq,
                    time,
                    source,
                    severity: (m & 0xff) as u8 & 7,
                    flags: ((m >> 8) & 0xff) as u8,
                    len: len as u16,
                    kind: ((m >> 32) & 0xff) as u8,
                },
                len,
            )
        }
    }

    /// The F3 header's fourth word, the same bytes [`Header::encode`] puts at 24..32.
    const fn meta(severity: u8, flags: u8, len: usize) -> u64 {
        severity as u64 | (flags as u64) << 8 | (len as u64) << 16
    }

    fn slot(seq: u64) -> usize {
        (1 + (seq % SLOTS) as usize) * SLOT_WORDS
    }

    /// **The drainer's cursor page**, word 0.
    pub struct Cursor<'a>(pub &'a AtomicU64);

    impl Cursor<'_> {
        /// What the drainer has written, or [`DETACHED`].
        pub fn get(&self) -> u64 {
            self.0.load(Ordering::Acquire)
        }
        /// **Drainer only.** Everything below `next` is consumed.
        pub fn set(&self, next: u64) {
            self.0.store(next, Ordering::Release);
        }
    }
}

/// **The log service's side of the console** (milestone 342 (the kernel and the `console` server
/// drive one UART from two address spaces)): how a kernel line reaches the terminal whole.
///
/// The log service sends a kernel line in sixteen-byte chunks, each first word
/// [`OPERATION_KERNEL_LINE`](console::OPERATION_KERNEL_LINE) with the count in the low bits, which
/// the console never acknowledges. [`Inserter`](console::Inserter) writes a kernel line only at
/// the start of a terminal line, and [`OPERATION_FLUSH`](console::OPERATION_FLUSH) forces queued
/// lines out. See notes/system-log.md, "Kernel lines on the console".
pub mod console {
    /// A chunk of a kernel line: `OPERATION_KERNEL_LINE << 56 | count`, bytes in the next two words.
    pub const OPERATION_KERNEL_LINE: u64 = 0x4b;
    /// Write whatever kernel lines are queued now, redrawing the partial line beneath them.
    pub const OPERATION_FLUSH: u64 = 0x46;

    /// The opcode in a request word, 0 for the console's own client's byte counts.
    pub const fn operation(w0: u64) -> u64 {
        w0 >> 56
    }

    /// The first word of a kernel chunk carrying `n` bytes.
    pub const fn chunk(n: usize) -> u64 {
        (OPERATION_KERNEL_LINE << 56) | n as u64
    }

    /// The longest kernel line the console assembles before writing it as cut.
    pub const LINE_MAX: usize = 256;
    /// What ends every line the console writes for the kernel: see [`Inserter::kernel_chunk`].
    pub const LINE_END: &[u8] = b"\r\n";
    /// How much of the terminal's current partial line is kept for a redraw.
    pub const PARTIAL_MAX: usize = 512;
    /// How many bytes of kernel lines wait for a line end.
    pub const QUEUE_MAX: usize = 2048;

    /// The console's kernel-line state. See the module doc.
    pub struct Inserter {
        partial: [u8; PARTIAL_MAX],
        plen: usize,
        partial_lost: bool,
        kline: [u8; LINE_MAX],
        klen: usize,
        queue: [u8; QUEUE_MAX],
        qlen: usize,
    }

    impl Default for Inserter {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Inserter {
        /// At the start of a line, nothing queued.
        pub const fn new() -> Self {
            Inserter {
                partial: [0; PARTIAL_MAX],
                plen: 0,
                partial_lost: false,
                kline: [0; LINE_MAX],
                klen: 0,
                queue: [0; QUEUE_MAX],
                qlen: 0,
            }
        }

        /// Whether the terminal is at the start of a line.
        pub fn at_line_start(&self) -> bool {
            self.plen == 0 && !self.partial_lost
        }

        /// How many bytes of kernel lines are waiting.
        pub fn queued(&self) -> usize {
            self.qlen
        }

        /// The console's own client wrote `bytes` to the terminal: track the partial line, and
        /// write anything queued if they ended one.
        pub fn wrote(&mut self, bytes: &[u8], mut out: impl FnMut(&[u8])) {
            for &b in bytes {
                if b == b'\n' {
                    self.plen = 0;
                    self.partial_lost = false;
                } else if self.plen < PARTIAL_MAX {
                    self.partial[self.plen] = b;
                    self.plen += 1;
                } else {
                    self.partial_lost = true;
                }
            }
            if self.at_line_start() && self.qlen > 0 {
                out(&self.queue[..self.qlen]);
                self.qlen = 0;
            }
        }

        /// A kernel chunk arrived. A finished line is written now if the terminal is at a line
        /// start, and queued otherwise.
        ///
        /// **Every line it writes ends `\r\n`, whatever the chunk ended it with.** The console's
        /// terminals are not all alike about a bare `\n`: the kernel's own screen and most serial
        /// programs return the carriage on it, but the userspace screen terminal
        /// (`video_terminal`) is a VT and only moves down. A bare `\n` there left the next line
        /// starting where the kernel line ended, so the prompt beneath it no longer began a row,
        /// and `cargo xtask uefi-boot` failed on PR #1498 (run 37108087239) with the shell
        /// unreadable on the firmware screen. The line editor writes `\r\n` for the same reason.
        pub fn kernel_chunk(&mut self, bytes: &[u8], mut out: impl FnMut(&[u8])) {
            for &b in bytes {
                if b == b'\r' {
                    continue;
                }
                if b != b'\n' {
                    self.kline[self.klen] = b;
                    self.klen += 1;
                }
                // A cut leaves room for the line end, so it gets a line of its own rather than
                // running into the next.
                if b == b'\n' || self.klen == LINE_MAX - LINE_END.len() {
                    self.kline[self.klen..self.klen + LINE_END.len()].copy_from_slice(LINE_END);
                    self.klen += LINE_END.len();
                    self.finish_line(&mut out);
                }
            }
        }

        fn finish_line(&mut self, out: &mut impl FnMut(&[u8])) {
            if self.at_line_start() && self.qlen == 0 {
                out(&self.kline[..self.klen]);
            } else {
                if self.qlen + self.klen > QUEUE_MAX {
                    self.flush(&mut *out);
                }
                if self.at_line_start() {
                    out(&self.kline[..self.klen]);
                } else {
                    self.queue[self.qlen..self.qlen + self.klen]
                        .copy_from_slice(&self.kline[..self.klen]);
                    self.qlen += self.klen;
                }
            }
            self.klen = 0;
        }

        /// Write the queue now. Mid-line, the queued lines go on a line of their own and the
        /// partial line is drawn again beneath them, as it was.
        pub fn flush(&mut self, mut out: impl FnMut(&[u8])) {
            if self.qlen == 0 {
                return;
            }
            if self.at_line_start() {
                out(&self.queue[..self.qlen]);
            } else {
                out(LINE_END);
                out(&self.queue[..self.qlen]);
                if !self.partial_lost {
                    out(&self.partial[..self.plen]);
                }
            }
            self.qlen = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A header survives the wire, and the three things that cannot be a header are refused: a
    /// length past the text bound, a level syslog does not have, and a reserved byte in use.
    #[test]
    fn a_header_round_trips_and_impossible_ones_are_refused() {
        let h = record::Header {
            seq: 0x0102_0304_0506_0708,
            time: 42,
            source: 7,
            severity: severity::WARNING,
            flags: record::flags::KERNEL | record::flags::CUT,
            len: record::TEXT_MAX as u16,
            kind: 0,
        };
        let b = h.encode();
        assert_eq!(b.len(), 32);
        assert_eq!(record::Header::decode(&b), Some(h));

        let mut long = b;
        long[26..28].copy_from_slice(&(record::TEXT_MAX as u16 + 1).to_le_bytes());
        assert_eq!(record::Header::decode(&long), None);
        let mut loud = b;
        loud[24] = 8;
        assert_eq!(record::Header::decode(&loud), None);
        let mut spare = b;
        spare[31] = 1;
        assert_eq!(record::Header::decode(&spare), None);
    }

    /// The control opcodes cannot be byte-sink opcodes, so a writer's message and the spawner's
    /// word about a writer are distinct on the wire before anyone looks at the badge.
    #[test]
    fn control_opcodes_are_not_byte_sink_opcodes() {
        for operation in [
            control::OPERATION_NAME,
            control::OPERATION_STREAM,
            control::OPERATION_READER,
            control::OPERATION_FORGET,
            read::OPERATION_READ,
        ] {
            assert!(
                operation > 1,
                "opcode {operation:#x} collides with OPERATION_BYTES or OPERATION_EOF"
            );
        }
        let w = control::word(
            control::OPERATION_READER,
            control::SCOPE_SYSTEM,
            3,
            0xdead_beef,
        );
        assert_eq!(
            control::fields(w),
            (
                control::OPERATION_READER,
                control::SCOPE_SYSTEM,
                3,
                0xdead_beef
            )
        );
        assert!(!control::badge_fits(0));
        assert!(!control::badge_fits(1 << 32));
        assert!(control::badge_fits(u32::MAX as u64));
    }

    /// A name longer than one chunk takes two messages, one that fits takes one, and one past the
    /// bound is cut rather than spilling into a third.
    #[test]
    fn a_name_is_one_or_two_chunks() {
        let short = control::name_messages(5, control::PROGRAM, b"wc");
        assert!(short[0].is_some() && short[1].is_none());
        let (w0, w1, _) = short[0].unwrap();
        assert_eq!(
            control::fields(w0),
            (control::OPERATION_NAME, control::PROGRAM, 0, 5)
        );
        assert_eq!(&w1.to_le_bytes()[..3], b"wc\0");

        let long = control::name_messages(5, control::USER, b"sink_transcript_writer");
        let (w0, w1, w2) = long[1].unwrap();
        assert_eq!(
            control::fields(w0),
            (control::OPERATION_NAME, control::USER, 1, 5)
        );
        assert_eq!(&w1.to_le_bytes()[..6], b"writer");
        assert_eq!(w2, 0);
    }

    /// A shared window's length is clamped on the way in, because the page is the reader's too.
    #[test]
    fn a_window_header_round_trips_and_a_lying_length_is_clamped() {
        let mut w = [0u8; read::WINDOW_BYTES];
        read::put_header(&mut w, 9, 100, read::status::OK);
        assert_eq!(read::header(&w), (9, 100, read::status::OK));
        read::put_header(&mut w, 9, u32::MAX, read::status::OK);
        assert_eq!(read::header(&w).1, read::DATA_MAX);
    }

    /// The names are syslog's keywords, one per level, and none repeats.
    #[test]
    fn every_level_has_its_own_syslog_keyword() {
        let names: [&str; 8] = core::array::from_fn(|l| severity::name(l as u8));
        assert_eq!(
            names,
            [
                "emerg", "alert", "crit", "err", "warning", "notice", "info", "debug"
            ]
        );
    }

    /// A record round-trips through a slot, one the kernel wrote past reads as overwritten
    /// rather than as somebody else's line, and an unwritten one reads as not yet.
    #[test]
    fn the_kernel_ring_round_trips_and_says_when_a_record_is_gone() {
        extern crate std;
        use core::sync::atomic::AtomicU64;

        use kernel_ring::{Read, Ring, SLOTS};
        let words: std::vec::Vec<AtomicU64> =
            (0..kernel_ring::WORDS).map(|_| AtomicU64::new(0)).collect();
        let ring = Ring::new(&words).unwrap();
        assert!(!ring.is_formatted());
        ring.format();
        assert!(ring.is_formatted());
        assert_eq!(SLOTS, 63);
        let mut out = [0u8; record::TEXT_MAX];
        assert_eq!(ring.read(0, &mut out), Read::NotYet);
        ring.append(
            7,
            severity::INFO,
            record::flags::KERNEL,
            b"  progenitor stack: 1 of 2",
        );
        let Read::Record(h, n) = ring.read(0, &mut out) else {
            panic!()
        };
        assert_eq!(
            (h.seq, h.time, h.flags, n),
            (0, 7, record::flags::KERNEL, 26)
        );
        assert_eq!(&out[..n], b"  progenitor stack: 1 of 2");
        assert!(ring.add_flags(0, record::flags::DIRECT));
        let Read::Record(h, _) = ring.read(0, &mut out) else {
            panic!()
        };
        assert_eq!(h.flags, record::flags::KERNEL | record::flags::DIRECT);
        for i in 1..=SLOTS {
            ring.append(i, 6, 0, &[b'x'; 300]);
        }
        assert_eq!(ring.read(0, &mut out), Read::Overwritten);
        let Read::Record(h, n) = ring.read(SLOTS, &mut out) else {
            panic!()
        };
        assert_eq!((h.seq, n), (SLOTS, record::TEXT_MAX));
        assert!(!ring.add_flags(0, record::flags::DIRECT));
    }

    /// **A reader that races the writer gets the record or "overwritten", never a mixture.** The
    /// slot's sequence word is set to busy before the body changes, so a copy taken across a
    /// rewrite fails its second check.
    #[test]
    fn a_torn_copy_is_detected() {
        extern crate std;
        use core::sync::atomic::{AtomicU64, Ordering};

        use kernel_ring::{BUSY, Read, Ring, SLOT_WORDS};
        let words: std::vec::Vec<AtomicU64> =
            (0..kernel_ring::WORDS).map(|_| AtomicU64::new(0)).collect();
        let ring = Ring::new(&words).unwrap();
        ring.format();
        ring.append(1, 6, 0, b"whole");
        // The writer is mid-rewrite of slot 0: what a reader sees then.
        words[SLOT_WORDS].store(BUSY, Ordering::Relaxed);
        let mut out = [0u8; record::TEXT_MAX];
        assert_eq!(ring.read(0, &mut out), Read::Overwritten);
    }

    /// **Each record keeps its own words.** Two full records side by side read back whole, with
    /// the severity, source and kind the kernel wrote; the window's edge is exact (a record
    /// [`SLOTS`](kernel_ring::SLOTS) behind the next is still there, one more is gone); a flag
    /// already set stays set; and the counters read what was counted.
    #[test]
    fn the_kernel_ring_keeps_each_record_and_its_counters() {
        extern crate std;
        use core::sync::atomic::{AtomicU64, Ordering};

        use kernel_ring::{Cursor, Read, Ring, SLOTS};
        let words: std::vec::Vec<AtomicU64> =
            (0..kernel_ring::WORDS).map(|_| AtomicU64::new(0)).collect();
        let ring = Ring::new(&words).unwrap();
        // The slot count alone, with no magic, is not a formatted ring, and neither is the magic
        // with another slot count.
        words[3].store(SLOTS, Ordering::Relaxed);
        assert!(!ring.is_formatted());
        ring.format();
        words[3].store(SLOTS + 1, Ordering::Relaxed);
        assert!(!ring.is_formatted());
        ring.format();

        let a = [b'a'; record::TEXT_MAX];
        let b = [b'b'; record::TEXT_MAX];
        ring.append(1, severity::INFO, record::flags::KERNEL, &a);
        ring.append(2, severity::WARNING, 0, &b);
        let mut out = [0u8; record::TEXT_MAX];
        let Read::Record(h, n) = ring.read(0, &mut out) else {
            panic!()
        };
        assert_eq!(&out[..n], &a[..]);
        assert_eq!((h.severity, h.source, h.kind), (severity::INFO, 0, 0));
        let Read::Record(h, n) = ring.read(1, &mut out) else {
            panic!()
        };
        assert_eq!(&out[..n], &b[..]);
        assert_eq!((h.severity, h.source), (severity::WARNING, 0));

        assert!(ring.add_flags(0, record::flags::KERNEL));
        let Read::Record(h, _) = ring.read(0, &mut out) else {
            panic!()
        };
        assert_eq!(
            h.flags,
            record::flags::KERNEL,
            "a flag set twice is still set"
        );

        for i in 2..=SLOTS {
            ring.append(i, 6, 0, b"x");
        }
        assert_eq!(ring.next_seq(), SLOTS + 1);
        assert_eq!(ring.oldest(), 1);
        assert_eq!(ring.read(0, &mut out), Read::Overwritten);
        assert!(matches!(ring.read(1, &mut out), Read::Record(h, _) if h.seq == 1));
        ring.append(0, 6, 0, b"y");
        ring.append(0, 6, 0, b"z");
        assert_eq!(ring.oldest(), 3);

        ring.count_fallback();
        ring.count_fallback();
        assert_eq!(ring.fallback(), 2);

        let page = AtomicU64::new(kernel_ring::DETACHED);
        let cursor = Cursor(&page);
        cursor.set(5);
        assert_eq!(cursor.get(), 5);
    }

    /// A kernel chunk's word is its opcode in the top byte over its byte count.
    #[test]
    fn a_kernel_chunk_word_is_its_opcode_and_its_count() {
        let w = console::chunk(16);
        assert_eq!(console::operation(w), console::OPERATION_KERNEL_LINE);
        assert_eq!(w & 0xff, 16);
    }

    /// **The queue holds exactly [`QUEUE_MAX`](console::QUEUE_MAX) bytes before it flushes**, and
    /// a client's write that does not end the line never releases it. Eight 256-byte lines fill it
    /// to the byte and go nowhere; the ninth flushes them first.
    #[test]
    fn the_queue_flushes_only_past_its_size_and_only_at_a_line_end() {
        extern crate std;
        use std::vec::Vec;
        let mut c = console::Inserter::new();
        c.wrote(b"$ ", |_| unreachable!());
        let line = [b'k'; console::LINE_MAX - console::LINE_END.len()];
        for _ in 0..8 {
            c.kernel_chunk(&line, |_| panic!("flushed before the queue was full"));
        }
        assert_eq!(c.queued(), console::QUEUE_MAX);
        c.wrote(b"more", |_| panic!("released mid-line"));
        let mut t: Vec<u8> = Vec::new();
        c.kernel_chunk(&line, |b| t.extend_from_slice(b));
        assert_eq!(
            t.len(),
            console::LINE_END.len() + console::QUEUE_MAX + b"$ more".len()
        );
        assert_eq!(c.queued(), console::LINE_MAX);
    }

    /// At a line start with nothing queued, the client's own write sends no kernel bytes at all.
    #[test]
    fn nothing_queued_is_nothing_written() {
        let mut c = console::Inserter::new();
        c.wrote(b"x\n", |_| panic!("an empty kernel write"));
    }

    /// A partial line longer than the console keeps is dropped from the redraw rather than
    /// written past the end of its buffer.
    #[test]
    fn a_partial_line_past_its_buffer_is_not_redrawn() {
        extern crate std;
        use std::vec::Vec;
        let mut c = console::Inserter::new();
        c.wrote(&[b'p'; console::PARTIAL_MAX + 1], |_| unreachable!());
        c.kernel_chunk(b"k\n", |_| unreachable!());
        let mut t: Vec<u8> = Vec::new();
        c.flush(|b| t.extend_from_slice(b));
        assert_eq!(t, b"\r\nk\r\n");
    }

    /// The console's half: a kernel line is written at a line start, queued mid-line, released
    /// by the line end that follows, and on a flush drawn on its own line with the partial line
    /// redrawn beneath it.
    #[test]
    fn a_kernel_line_waits_for_a_line_start_and_never_splices() {
        extern crate std;
        use std::vec::Vec;
        let mut c = console::Inserter::new();
        let mut t: Vec<u8> = Vec::new();
        c.kernel_chunk(b"  k one\n", |b| t.extend_from_slice(b));
        assert_eq!(t, b"  k one\r\n");
        c.wrote(b"$ package ins", |b| t.extend_from_slice(b));
        t.extend_from_slice(b"$ package ins");
        c.kernel_chunk(b"  k two\n", |b| t.extend_from_slice(b));
        assert_eq!(c.queued(), 9);
        t.extend_from_slice(b"tall x\n");
        c.wrote(b"tall x\n", |b| t.extend_from_slice(b));
        assert_eq!(t, b"  k one\r\n$ package install x\n  k two\r\n");
        t.clear();
        t.extend_from_slice(b"$ ");
        c.wrote(b"$ ", |_| unreachable!());
        c.kernel_chunk(b"  k three\n", |_| unreachable!());
        c.flush(|b| t.extend_from_slice(b));
        assert_eq!(t, b"$ \r\n  k three\r\n$ ");
        assert_eq!(c.queued(), 0);
    }

    /// **Every line the console writes for the kernel returns the carriage**, because the screen
    /// terminal is a VT and a bare `\n` only moves down (PR #1498's firmware-screen failure, run
    /// 37108087239: the prompt redrawn after a flush began mid-row). A `\r\n` from the kernel
    /// stays one line end, and a line too long for the buffer is cut with no byte lost.
    #[test]
    fn a_kernel_line_ends_with_a_carriage_return_however_it_arrived() {
        extern crate std;
        use std::vec::Vec;
        let mut c = console::Inserter::new();
        let mut t: Vec<u8> = Vec::new();
        c.kernel_chunk(b"bare\nwindows\r\n", |b| t.extend_from_slice(b));
        assert_eq!(t, b"bare\r\nwindows\r\n");
        t.clear();
        let long = [b'x'; console::LINE_MAX + 10];
        c.kernel_chunk(&long, |b| t.extend_from_slice(b));
        c.kernel_chunk(b"\n", |b| t.extend_from_slice(b));
        let text: Vec<u8> = t.iter().copied().filter(|&b| b == b'x').collect();
        assert_eq!(text.len(), long.len());
        assert_eq!(t.iter().filter(|&&b| b == b'\n').count(), 2);
        assert!(t.windows(2).all(|w| w[1] != b'\n' || w[0] == b'\r'));
    }

    /// The sizes the notes quote: a record is at most 256 bytes (Zircon's), a window's data is its
    /// page less the 16-byte header, and the four flag bits are the four low bits.
    #[test]
    fn the_quoted_sizes_and_flag_bits_are_what_the_notes_say() {
        assert_eq!(record::RECORD_MAX, 256);
        assert_eq!(read::DATA_MAX, 4080);
        assert_eq!(record::flags::KERNEL, 1);
        assert_eq!(record::flags::DROPPED_BEFORE, 2);
        assert_eq!(record::flags::CUT, 4);
        assert_eq!(record::flags::DIRECT, 8);
    }

    /// Severity 7 is the last level syslog has: accepted, and 8 is refused.
    #[test]
    fn the_last_syslog_level_is_accepted_and_the_next_is_refused() {
        let h = record::Header {
            seq: 1,
            time: 2,
            source: 3,
            severity: 7,
            flags: 0,
            len: 0,
            kind: 0,
        };
        let mut b = h.encode();
        assert_eq!(record::Header::decode(&b), Some(h));
        b[24] = 8;
        assert_eq!(record::Header::decode(&b), None);
    }

    /// A name of sixteen bytes is one message and seventeen is two: the second chunk carries only
    /// what spills past sixteen.
    #[test]
    fn a_name_of_sixteen_bytes_is_one_message_and_seventeen_is_two() {
        let sixteen = control::name_messages(5, control::USER, &[b'a'; 16]);
        assert!(sixteen[0].is_some() && sixteen[1].is_none());
        let seventeen = control::name_messages(5, control::USER, &[b'a'; 17]);
        let (word, ..) = seventeen[1].expect("a second chunk");
        assert_eq!(
            control::fields(word),
            (control::OPERATION_NAME, control::USER, 1, 5)
        );
        assert_eq!(
            seventeen[1].unwrap().1,
            u64::from_le_bytes(*b"a\0\0\0\0\0\0\0")
        );
    }

    /// A read request carries its opcode in the top byte and nothing else.
    #[test]
    fn a_read_request_is_the_opcode_in_the_top_byte_alone() {
        assert_eq!(read::request() >> 56, read::OPERATION_READ);
        assert_eq!(read::request() & ((1 << 56) - 1), 0);
    }
}
