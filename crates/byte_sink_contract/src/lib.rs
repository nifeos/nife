//! **The byte-sink contract**: one framing for *write these bytes there*, so a writer cannot tell
//! what is underneath its output. A sink is a `Rendezvous` capability with `WRITE` and nothing
//! else: up to [`INLINE_MAX`] bytes ride in registers with the opcode and length in the first word
//! (the three-word fastpath, notes/abi.md), sent with `SEND`, whose rendezvous is the flow control.
//! [`Sent::NoSink`] means the slot is empty and writing into the void is correct; [`Sent::Gone`]
//! means the sink was destroyed (§16 (object revocation)) and **the writer must end**, `SIGPIPE`
//! as a return code. No per-write error, no types, no seeking. See notes/sink-protocol.md.
//!
//! Name: ratified 2026-08-23 (calef) as `byte_sink_proto`, renamed from `sink_proto` to match "the
//! byte-sink contract"; `_contract` per calef's 2026-10-18 ruling.

#![no_std]

/// The opcode's position in the first word: bits 63:56, the same place `filesystem_contract` and
/// `line_editor::proto` put theirs. One spelling for "the wire contract" across the tree.
pub const OPERATION_SHIFT: u32 = 56;

/// **Bytes.** The low 32 bits are the count (1 to [`INLINE_MAX`]); the bytes themselves are in the
/// second and third words, little-endian, low word first.
///
/// It is **zero on purpose.** With the opcode at zero the first word is exactly the byte count,
/// which is bit-for-bit the framing std's stdout already sent before this crate existed. So
/// unifying the protocol changed no instruction on the fastpath and cost no message, and the
/// benchmark that prices `println!` cannot tell that anything happened. An opcode is a claim about
/// what a message means; the cheapest claim to make is the one the wire was already making.
pub const OPERATION_BYTES: u64 = 0;

/// **End of stream.** The writer is finished; no more bytes will come. The low 32 bits are zero.
///
/// A sink acts on this rather than merely noticing it: a file sink closes its handle, a terminal
/// sink may draw a prompt, and the reader of a pipe stops reading instead of blocking forever on a
/// writer that has exited. Without it "the producer is done" would have to be inferred from a death
/// notification the reader may not be the supervisor for, which is a fact about process supervision
/// standing in for a fact about a stream.
pub const OPERATION_EOF: u64 = 1;

/// The most bytes one message carries: two 64-bit words. Not a tuning parameter. It is what fits in
/// the registers a `SEND` already has, and enlarging it means giving a sink a shared page, which is
/// the thing the module note explains this contract exists to avoid.
pub const INLINE_MAX: usize = 16;

/// The kernel's `abi::Error::Gone`, restated so this crate stays dependency-free.
///
/// It has to be restated because `cargo xtask std-src` copies this file verbatim into the patched
/// std sysroot, where there is no `abi` crate to link against. The test module below pins the two
/// numbers together and is truncated out of that copy, so the drift this duplication invites is
/// caught in the workspace where it can be.
pub const GONE: i64 = -11;

/// Pack an opcode and a length into a message's first word.
pub const fn req(operation: u64, len: u64) -> u64 {
    (operation << OPERATION_SHIFT) | (len & 0xffff_ffff)
}

/// The opcode of a message's first word.
pub const fn operation(w0: u64) -> u64 {
    w0 >> OPERATION_SHIFT
}

/// The byte count of a message's first word.
pub const fn len(w0: u64) -> usize {
    (w0 & 0xffff_ffff) as usize
}

/// The first word of an end-of-stream message.
pub const fn eof() -> u64 {
    req(OPERATION_EOF, 0)
}

/// Pack up to [`INLINE_MAX`] bytes into the three words of a `SEND`. Bytes past the sixteenth are
/// **not** silently dropped: the caller chunks, and the returned count says how many were taken, so
/// a caller that ignores it writes a short message rather than losing a tail without knowing.
///
/// # Examples
///
/// ```
/// use byte_sink_contract::{INLINE_MAX, Msg, eof, pack, unpack};
///
/// let line = b"the quick brown fox\n"; // 20 bytes, so two messages and an EOF
/// let mut messages = 0;
/// let mut received = [0u8; 32];
/// let mut got = 0;
/// let mut ended = false;
///
/// let mut rest = &line[..];
/// while !rest.is_empty() {
///     let (w0, w1, w2, taken) = pack(rest);
///     messages += 1;
///     rest = &rest[taken..]; // `pack` never drops a tail silently; the count says what it took
///
///     // The other end of the SEND.
///     let mut buf = [0u8; INLINE_MAX];
///     match unpack(w0, w1, w2, &mut buf) {
///         Msg::Bytes(n) => {
///             received[got..got + n].copy_from_slice(&buf[..n]);
///             got += n;
///         }
///         Msg::Eof | Msg::Malformed => unreachable!(),
///     }
/// }
/// let (w0, w1, w2) = (eof(), 0, 0);
/// if let Msg::Eof = unpack(w0, w1, w2, &mut [0u8; INLINE_MAX]) {
///     ended = true;
/// }
///
/// assert_eq!(messages, 2); // 16 + 4
/// assert_eq!(&received[..got], line);
/// assert!(ended);
/// ```
pub fn pack(bytes: &[u8]) -> (u64, u64, u64, usize) {
    let n = if bytes.len() < INLINE_MAX {
        bytes.len()
    } else {
        INLINE_MAX
    };
    let mut words = [0u8; INLINE_MAX];
    words[..n].copy_from_slice(&bytes[..n]);
    let mut lo = [0u8; 8];
    let mut hi = [0u8; 8];
    lo.copy_from_slice(&words[..8]);
    hi.copy_from_slice(&words[8..]);
    (
        req(OPERATION_BYTES, n as u64),
        u64::from_le_bytes(lo),
        u64::from_le_bytes(hi),
        n,
    )
}

/// **Where a bytes message's severity rides**: bits 39:32 of the first word, which the framing left
/// unused (the count is bits 31:0 and the opcode bits 63:56). §242 (a system log) amended this
/// contract rather than giving a log writer its own framing: a terminal, a pipe and a file read the
/// count through [`len`] and never see these bits. The field holds the syslog level plus one (RFC
/// 5424's 0 emergency to 7 debug), so 0 still means not set, and a sink that cares then infers a
/// level from the stream (diagnostics warns, ordinary output informs, §242). The width and the
/// plus-one encoding are milestone 613 (a system log service)'s, and provisional: §242 ruled the
/// bits, not their layout.
pub const SEVERITY_SHIFT: u32 = 32;

/// **Where a bytes message's body kind rides**: bits 47:40 of the first word. §242's F3 record
/// carries a body-kind byte so a later structured body (F2's key/value arguments) arrives as a new
/// kind rather than a new framing; [`KIND_TEXT`] is the only kind today. Bits 55:48 stay zero.
pub const KIND_SHIFT: u32 = 40;

/// The one body kind that exists: UTF-8 text, which is what every writer already sends, and 0 for
/// the reason [`OPERATION_BYTES`] is 0: the wire was already saying it.
pub const KIND_TEXT: u8 = 0;

/// The syslog level a bytes message's first word carries, or `None` when the writer did not set
/// one (see [`SEVERITY_SHIFT`]). A field value past 8 is not a level and also reads as `None`: the
/// sink infers rather than trusting a number it cannot name.
pub const fn severity(w0: u64) -> Option<u8> {
    let field = ((w0 >> SEVERITY_SHIFT) & 0xff) as u8;
    if field == 0 || field > 8 {
        None
    } else {
        Some(field - 1)
    }
}

/// `w0` with its severity set to syslog `level` (0 to 7; a larger value is clamped to 7, debug, the
/// least alarming reading of a level nobody defined).
pub const fn with_severity(w0: u64, level: u8) -> u64 {
    let level = if level > 7 { 7 } else { level };
    (w0 & !(0xff << SEVERITY_SHIFT)) | (((level as u64) + 1) << SEVERITY_SHIFT)
}

/// The body kind a bytes message's first word carries (see [`KIND_SHIFT`]).
pub const fn kind(w0: u64) -> u8 {
    ((w0 >> KIND_SHIFT) & 0xff) as u8
}

/// What one received message was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Msg {
    /// [`OPERATION_BYTES`]: this many bytes were written into the caller's buffer, 0 to [`INLINE_MAX`].
    Bytes(usize),
    /// [`OPERATION_EOF`]: the writer is finished.
    Eof,
    /// An opcode this contract does not define, or a byte count past [`INLINE_MAX`].
    ///
    /// A sink refuses it rather than guessing, and refusing loudly is the point: a length larger
    /// than the message can hold is the shape of a confused or hostile writer, and the read it
    /// would authorise is a read past the end of a buffer.
    Malformed,
}

/// Unpack a received message into `out`. See [`Msg`].
///
/// # Examples
///
/// With [`OPERATION_BYTES`] the first word is the byte count, and a length the message cannot
/// hold is refused rather than obeyed:
///
/// ```
/// use byte_sink_contract::{INLINE_MAX, Msg, OPERATION_BYTES, len, operation, pack, req, unpack};
///
/// let (w0, ..) = pack(b"hi");
/// assert_eq!(w0, 2); // the whole word, not just its low bits
/// assert_eq!(operation(w0), OPERATION_BYTES);
/// assert_eq!(len(w0), 2);
///
/// // A writer claiming more bytes than three words can carry is malformed, not generous.
/// let lying = req(OPERATION_BYTES, INLINE_MAX as u64 + 1);
/// assert_eq!(unpack(lying, 0, 0, &mut [0u8; INLINE_MAX]), Msg::Malformed);
/// ```
pub fn unpack(w0: u64, w1: u64, w2: u64, out: &mut [u8; INLINE_MAX]) -> Msg {
    match operation(w0) {
        OPERATION_BYTES => {
            let n = len(w0);
            if n > INLINE_MAX {
                return Msg::Malformed;
            }
            out[..8].copy_from_slice(&w1.to_le_bytes());
            out[8..].copy_from_slice(&w2.to_le_bytes());
            Msg::Bytes(n)
        }
        OPERATION_EOF => Msg::Eof,
        _ => Msg::Malformed,
    }
}

/// What a writer learned from a `SEND`'s return value. See the module note: this is the one thing
/// the contract must tell a writer, and the two failures are not the same fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sent {
    /// The bytes are at the sink.
    Ok,
    /// **There was a sink and it is destroyed.** Stop writing; end the program.
    Gone,
    /// **There is no sink and there never was one.** Keep running; the bytes go nowhere.
    NoSink,
}

/// Classify a `SEND`'s return value.
///
/// Every refusal other than [`GONE`] is [`Sent::NoSink`], and that is deliberate rather than lazy.
/// An empty slot, a slot holding a `PageFrame` instead of an `Rendezvous`, and an endpoint held without
/// `WRITE` are all the same fact from the writer's side: *this program was not given a working
/// output destination*, which is a wiring error its parent made and which the program has no
/// channel to report on, its only channel being the broken one. The conservative reading keeps it
/// running; the alternative kills a program because its spawner was careless.
pub const fn classify(ret: i64) -> Sent {
    if ret >= 0 {
        Sent::Ok
    } else if ret == GONE {
        Sent::Gone
    } else {
        Sent::NoSink
    }
}

/// **Shared fixtures for the sink tests**: the bytes an indifferent writer writes, the file a file
/// sink writes them into, and the sentinels its report carries. One definition, so the component
/// and the kernel-side test cannot disagree about what "the same bytes" means. Same split
/// `filesystem_fixture` makes for the filesystem contract.
pub mod fixture {
    use super::Sent;

    /// What `fixtures/src/sink_transcript_writer.rs` writes. Long enough to span several messages and to
    /// end mid-message, so a decoder that only ever sees full 16-byte chunks is not being flattered
    /// by the fixture.
    pub const TRANSCRIPT: &[u8] =
        b"a program does not know what its output slot holds, and that is the point.\n";

    /// The name `fixtures/src/file_sink.rs` writes into. Created on first use rather than shipped in the
    /// image, because the file's existence is part of what the test proves.
    pub const SINK_NAME: &str = "sinkout";

    /// A sink reports this once its destination is open and before it takes a byte, so a hang in
    /// opening the file is distinguishable from a hang in the serve path. Same discipline as
    /// `filesystem_contract::READY`.
    pub const READY: u64 = 0x5142_0000_0000_0001;

    /// A sink reports this after end-of-stream, with the byte total in the second word.
    pub const DONE: u64 = 0x5142_0000_0000_0002;

    /// A sink reports this on a message the contract does not define, with the offending first word
    /// in the second position. It refuses rather than guessing (see [`super::Msg::Malformed`]).
    pub const BAD: u64 = 0x5142_0000_0000_0003;

    /// A writer's report of how its last `SEND` classified. A number, because the assertion it
    /// serves is about telling `Gone` from `NoSink` and a boolean cannot carry that.
    pub const fn code(sent: Sent) -> u64 {
        match sent {
            Sent::Ok => 0,
            Sent::Gone => 1,
            Sent::NoSink => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The number this crate restates and the kernel's enum are one fact in two files, and this is
    /// the only place that can notice them parting. `abi` is a dev-dependency precisely so this
    /// check exists in the workspace and never travels into the std sysroot copy, which cannot
    /// link it.
    #[test]
    fn the_restated_gone_code_is_the_kernels() {
        assert_eq!(GONE, abi::Error::Gone as i64);
    }

    /// A round trip through the wire words, at the boundaries that matter: a message that fills
    /// both words, one that fills neither, and one that straddles them.
    #[test]
    fn bytes_survive_the_wire_words() {
        for n in 0..=INLINE_MAX {
            let src: [u8; INLINE_MAX] = core::array::from_fn(|i| (i as u8).wrapping_mul(37) ^ 0x5a);
            let (w0, w1, w2, taken) = pack(&src[..n]);
            assert_eq!(taken, n);
            let mut out = [0u8; INLINE_MAX];
            assert_eq!(unpack(w0, w1, w2, &mut out), Msg::Bytes(n));
            assert_eq!(&out[..n], &src[..n]);
        }
    }

    /// A caller handing over more than one message holds gets a full message and an honest count,
    /// not a truncated write it cannot detect.
    #[test]
    fn pack_takes_a_full_message_and_says_so() {
        let src = [7u8; 40];
        let (w0, _, _, taken) = pack(&src);
        assert_eq!(taken, INLINE_MAX);
        assert_eq!(len(w0), INLINE_MAX);
    }

    /// The framing that makes the unification free: with [`OPERATION_BYTES`] at zero, a bytes message's
    /// first word IS its length, which is the wire std's stdout sent before this contract existed.
    #[test]
    fn a_bytes_messages_first_word_is_its_length() {
        let (w0, _, _, _) = pack(b"nife");
        assert_eq!(w0, 4);
    }

    /// The severity bits ride beside the count without disturbing it: every sink that reads only
    /// [`len`] and [`operation`] (a terminal, a pipe, a file) decodes a severity-carrying message exactly
    /// as it decoded one without, and a writer that never set a level reads as "not set", never as
    /// level 0, which is emergency.
    #[test]
    fn severity_rides_beside_the_count_and_unset_is_not_emergency() {
        let (w0, w1, w2, _) = pack(b"disk full");
        assert_eq!(severity(w0), None);
        let marked = with_severity(w0, 3);
        assert_eq!(severity(marked), Some(3));
        assert_eq!(len(marked), 9);
        assert_eq!(operation(marked), OPERATION_BYTES);
        assert_eq!(kind(marked), KIND_TEXT);
        let mut out = [0u8; INLINE_MAX];
        assert_eq!(unpack(marked, w1, w2, &mut out), Msg::Bytes(9));
        assert_eq!(&out[..9], b"disk full");
        assert_eq!(severity(with_severity(w0, 0)), Some(0));
        assert_eq!(severity(with_severity(w0, 200)), Some(7));
        assert_eq!(severity(w0 | (9 << SEVERITY_SHIFT)), None);
    }

    /// EOF is distinguishable from every byte count, including zero, which is the property that
    /// lets one word carry both.
    #[test]
    fn eof_is_not_a_byte_count() {
        let mut out = [0u8; INLINE_MAX];
        assert_eq!(unpack(eof(), 0, 0, &mut out), Msg::Eof);
        let (w0, w1, w2, _) = pack(&[]);
        assert_eq!(unpack(w0, w1, w2, &mut out), Msg::Bytes(0));
        assert_ne!(eof(), w0);
    }

    /// A length past what a message can hold, and an opcode nobody defined, are refused rather than
    /// clamped. Clamping a length would turn a hostile writer's message into a plausible one.
    #[test]
    fn a_malformed_message_is_refused_not_repaired() {
        let mut out = [0u8; INLINE_MAX];
        assert_eq!(
            unpack(req(OPERATION_BYTES, 17), 0, 0, &mut out),
            Msg::Malformed,
            "a byte count past INLINE_MAX must not be clamped",
        );
        assert_eq!(
            unpack(req(OPERATION_BYTES, u32::MAX as u64), 0, 0, &mut out),
            Msg::Malformed,
        );
        assert_eq!(unpack(req(9, 4), 0, 0, &mut out), Msg::Malformed);
    }

    /// The three report codes are distinct, which is the only property of them that matters: a test
    /// that could not tell `Gone` from `NoSink` in a report would not be testing anything.
    #[test]
    fn the_report_codes_tell_the_three_outcomes_apart() {
        use fixture::code;
        assert_ne!(code(Sent::Ok), code(Sent::Gone));
        assert_ne!(code(Sent::Ok), code(Sent::NoSink));
        assert_ne!(code(Sent::Gone), code(Sent::NoSink));
    }

    /// The distinction the whole contract exists to carry. `Gone` must not be reachable from any
    /// other refusal, because a writer turns it into "end this program".
    #[test]
    fn gone_is_distinguishable_from_never_had_one() {
        assert_eq!(classify(0), Sent::Ok);
        assert_eq!(classify(16), Sent::Ok);
        assert_eq!(classify(GONE), Sent::Gone);
        for other in [-1i64, -2, -3, -4, -5, -6, -7, -8, -9, -10, -12, -99] {
            assert_eq!(
                classify(other),
                Sent::NoSink,
                "error {other} must not be read as a dead sink; only {GONE} is",
            );
        }
    }

    /// Setting a level replaces the one already there rather than merging with it, leaves the count
    /// and the kind alone, and clamps a level nobody defined to debug.
    #[test]
    fn a_severity_set_twice_is_the_second_and_the_neighbours_are_untouched() {
        let w0 = with_severity((0x2a << KIND_SHIFT) | 9, 5);
        assert_eq!(severity(w0), Some(5));
        let again = with_severity(w0, 2);
        assert_eq!(severity(again), Some(2), "not 5 | 2");
        assert_eq!(kind(again), 0x2a);
        assert_eq!(len(again), 9);
        assert_eq!(severity(with_severity(0, 200)), Some(7));
        assert_eq!(severity(with_severity(0, 7)), Some(7));
    }

    /// The body kind is its own byte, between the severity above it and nothing below it but the
    /// count: neither neighbour leaks in, and it reads back whole.
    #[test]
    fn the_kind_is_read_from_its_own_byte() {
        assert_eq!(kind(0), 0);
        assert_eq!(kind(0xff << KIND_SHIFT), 0xff);
        assert_eq!(kind(3 << KIND_SHIFT), 3);
        assert_eq!(kind((5 << SEVERITY_SHIFT) | 17), 0);
        assert_eq!(
            kind(0xaa << (KIND_SHIFT + 8)),
            0,
            "bits above the kind byte are not it"
        );
    }
}
