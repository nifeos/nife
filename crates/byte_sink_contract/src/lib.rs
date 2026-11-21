//! **The byte-sink contract**: one framing for *write these bytes there*, so a writer cannot tell
//! what is underneath its output. A terminal, a file and a pipe become substitutable in one
//! capability slot.
//!
//! A sink is a `Rendezvous` capability with `WRITE` and nothing else. A writer sends with `SEND`,
//! whose rendezvous is the flow control, and gets no reply. Up to [`MAXIMUM_INLINE_BYTES`] bytes ride in
//! the three message registers with the opcode and the length in the first word, so a write costs
//! one message and touches no memory.
//!
//! | word | holds |
//! |---|---|
//! | first | opcode in bits 63:56 ([`OPERATION_SHIFT`]), body kind in bits 47:40 ([`KIND_SHIFT`]), severity in bits 39:32 ([`SEVERITY_SHIFT`]), byte count in bits 31:0 |
//! | second | bytes 0 to 7, little-endian |
//! | third | bytes 8 to 15, little-endian |
//!
//! # Why this shape
//!
//! - **Registers, not a shared page.** A shared page would give every sink a mapping and a lifetime
//!   rule; sixteen bytes ride free in the registers a `SEND` already has, and a caller chunks longer
//!   writes ([`pack`] says how many it took).
//! - **`SEND`, not `CALL`.** A writer has no reply to wait for, so a slow sink slows the writer by
//!   the rendezvous alone, and a sink needs no reply capability.
//! - **No per-write error.** A write either reaches the sink or it does not, and the one failure a
//!   writer must tell apart is returned by `SEND` itself: [`Sent::NoSink`] means the slot is empty
//!   and writing into the void is correct; [`Sent::Gone`] means the sink was destroyed and **the
//!   writer must end**, `SIGPIPE` as a return code. A standard library maps them to
//!   `ErrorKind::Unsupported` and `ErrorKind::BrokenPipe`.
//!
//! # What it does not carry
//!
//! No types, no seeking, no reply, no flow-control credit beyond the rendezvous, no version
//! operation (a program declares the pair [`INTERFACE_ID`] and [`VERSION`] where it is spawned), and
//! no meaning for the second and third words of an end-of-stream message: they are reserved, a
//! writer sends zero in both, and [`unpack`] refuses a message with either non-zero as
//! [`Message::Malformed`]. A program with a result to report after its stream ends sends it on a
//! channel of its own.
//!
//! Name: ratified 2026-08-23 (calef) as `byte_sink_proto`, renamed from `sink_proto` to match "the
//! byte-sink contract"; `_contract` per calef's 2026-10-18 ruling.

#![no_std]

/// The opcode's position in the first word: bits 63:56, the same place `filesystem_contract` and
/// `terminal_contract` put theirs. One spelling for "the wire contract" across the tree.
pub const OPERATION_SHIFT: u32 = 56;

/// The framing's version, one for the message set and the inline packing together. A break bumps
/// this; an addition owes no bump. A program's note declares the pair ([`INTERFACE_ID`], this
/// number), and whoever spawns and wires it compares them. The contract has no wire operation for
/// versions: a sink is one-way, so there is nothing a writer could `CALL`, and opcode 0 is
/// [`OPERATION_BYTES`]. A sink that serves two versions does so by accepting both framings.
///
/// Name: ratified 2026-11-21 (UTC). The value is frozen at 1.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 6, naming the byte-sink write
/// framing. The number is this interface's for good: no other interface takes it, and a format
/// change is a new version, never a new number. A program declares the pair of this id and
/// [`VERSION`].
///
/// Name: ratified 2026-11-21 (UTC). The value is frozen at 6.
pub const INTERFACE_ID: u64 = 6;

/// **Bytes.** The low 32 bits are the count (0 to [`MAXIMUM_INLINE_BYTES`]); the bytes themselves are in
/// the second and third words, little-endian, low word first. A zero count is legal and a sink
/// treats it as nothing, as `write(2)` treats a zero-byte write.
///
/// It is **zero on purpose.** With the opcode at zero the first word is exactly the byte count,
/// which is bit-for-bit the framing std's stdout already sent before this crate existed. So
/// unifying the protocol changed no instruction on the fastpath and cost no message, and the
/// benchmark that prices `println!` cannot tell that anything happened. An opcode is a claim about
/// what a message means; the cheapest claim to make is the one the wire was already making.
pub const OPERATION_BYTES: u8 = 0;

/// **End of stream.** The writer is finished; no more bytes will come. The message is exactly
/// `(`[`end_of_stream`]`(), 0, 0)`: every bit of the first word below the opcode is zero, and the second and
/// third words are reserved. A writer sends zero in them, and [`unpack`] refuses anything else as
/// [`Message::Malformed`], so no writer can make them carry a meaning a sink would then have to know.
/// A program with a result to report after its stream sends it on a channel of its own.
///
/// A sink acts on this rather than merely noticing it: a file sink closes its handle, a terminal
/// sink may draw a prompt, and the reader of a pipe stops reading instead of blocking forever on a
/// writer that has exited. Without it "the producer is done" would have to be inferred from a death
/// notification the reader may not be the supervisor for, which is a fact about process supervision
/// standing in for a fact about a stream.
///
/// Name: ratified 2026-11-21 (UTC). Spelled out; `eof` is the standard library's and C's spelling,
/// kept as a documentation alias.
#[doc(alias = "eof")]
pub const OPERATION_END_OF_STREAM: u8 = 1;

/// The most bytes one message carries: two 64-bit words. Not a tuning parameter. It is what fits in
/// the registers a `SEND` already has, and enlarging it means giving a sink a shared page, which is
/// the thing this contract exists to avoid.
///
/// Name: ratified 2026-11-21 (UTC). A limit opens with `MAXIMUM_` and ends with its unit.
pub const MAXIMUM_INLINE_BYTES: usize = 16;

/// The kernel's return for "the object no longer exists", restated here so a writer needs no second
/// crate. Anyone who links the kernel's error type as well pins the two numbers together with a
/// compile-time assertion.
pub const GONE: i64 = -11;

/// Pack an opcode and a length into a message's first word: the opcode in bits 63:56 and the length
/// in bits 31:0.
///
/// Name: ratified 2026-11-21 (UTC).
pub const fn first_word(operation: u8, len: u32) -> u64 {
    ((operation as u64) << OPERATION_SHIFT) | len as u64
}

/// The opcode of a message's first word.
pub const fn operation(word_zero: u64) -> u8 {
    (word_zero >> OPERATION_SHIFT) as u8
}

/// The byte count of a message's first word.
pub const fn len(word_zero: u64) -> usize {
    (word_zero & 0xffff_ffff) as usize
}

/// The first word of an end-of-stream message.
///
/// Name: ratified 2026-11-21 (UTC). Spelled out; `eof` is the standard library's and C's spelling,
/// kept as a documentation alias.
#[doc(alias = "eof")]
pub const fn end_of_stream() -> u64 {
    first_word(OPERATION_END_OF_STREAM, 0)
}

/// The three words of one message, in the order a `SEND` carries them: the first word (opcode, body
/// kind, severity and count), then the second and third, which hold the bytes of a bytes message
/// and are reserved in an end of stream. They ride in the three message registers of a `SEND`;
/// this struct names them without tying the contract to a register assignment.
///
/// Name: provisional, minted 2026-11-21 (UTC) by lane byte-sink-contract-names. Replaces three
/// positional `u64` arguments and a four-tuple; the fields take the names of the parameters they
/// replace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageWords {
    /// The first word. See [`first_word`].
    pub word_zero: u64,
    /// The second word: bytes 0 to 7, little-endian, in a bytes message.
    pub word_one: u64,
    /// The third word: bytes 8 to 15, little-endian, in a bytes message.
    pub word_two: u64,
}

impl MessageWords {
    /// The three words of a message, in order.
    ///
    /// Name: provisional, minted 2026-11-21 (UTC) by lane byte-sink-contract-names. The standard
    /// spelling of a constructor.
    pub const fn new(word_zero: u64, word_one: u64, word_two: u64) -> Self {
        Self {
            word_zero,
            word_one,
            word_two,
        }
    }
}

/// Pack up to [`MAXIMUM_INLINE_BYTES`] bytes into the three words of a `SEND`. Bytes past the
/// sixteenth are **not** silently dropped: the caller chunks, and the returned count says how many
/// were taken, so a caller that ignores it writes a short message, not a silently lost tail.
///
/// # Examples
///
/// ```
/// use byte_sink_contract::{MAXIMUM_INLINE_BYTES, Message, MessageWords, end_of_stream, pack, unpack};
///
/// let line = b"the quick brown fox\n"; // 20 bytes, so two messages and an end of stream
/// let mut messages = 0;
/// let mut received = [0u8; 32];
/// let mut got = 0;
/// let mut ended = false;
///
/// let mut rest = &line[..];
/// while !rest.is_empty() {
///     let (words, taken) = pack(rest);
///     messages += 1;
///     rest = &rest[taken..]; // `pack` never drops a tail silently; the count says what it took
///
///     // The other end of the SEND.
///     let mut buf = [0u8; MAXIMUM_INLINE_BYTES];
///     match unpack(words, &mut buf) {
///         Message::Bytes(n) => {
///             received[got..got + n].copy_from_slice(&buf[..n]);
///             got += n;
///         }
///         Message::EndOfStream | Message::Malformed => unreachable!(),
///     }
/// }
/// let end = MessageWords::new(end_of_stream(), 0, 0);
/// if let Message::EndOfStream = unpack(end, &mut [0u8; MAXIMUM_INLINE_BYTES]) {
///     ended = true;
/// }
///
/// assert_eq!(messages, 2); // 16 + 4
/// assert_eq!(&received[..got], line);
/// assert!(ended);
/// ```
pub fn pack(bytes: &[u8]) -> (MessageWords, usize) {
    let n = if bytes.len() < MAXIMUM_INLINE_BYTES {
        bytes.len()
    } else {
        MAXIMUM_INLINE_BYTES
    };
    let mut words = [0u8; MAXIMUM_INLINE_BYTES];
    words[..n].copy_from_slice(&bytes[..n]);
    let mut lo = [0u8; 8];
    let mut hi = [0u8; 8];
    lo.copy_from_slice(&words[..8]);
    hi.copy_from_slice(&words[8..]);
    (
        MessageWords::new(
            first_word(OPERATION_BYTES, n as u32),
            u64::from_le_bytes(lo),
            u64::from_le_bytes(hi),
        ),
        n,
    )
}

/// **Where a bytes message's severity rides**: bits 39:32 of the first word, which the framing left
/// unused (the count is bits 31:0 and the opcode bits 63:56). A log writer shares this framing
/// rather than having its own: a terminal, a pipe and a file read the count through [`len`] and
/// never see these bits. The field holds the syslog level plus one. The levels are
/// those of the syslog protocol (Request for Comments 5424), 0 emergency to 7 debug, so 0 still
/// means not set, and a sink that cares then infers a level from the stream.
/// The width and the plus-one encoding are provisional. A consumer reads these bits only on a bytes
/// message.
pub const SEVERITY_SHIFT: u32 = 32;

/// **Where a bytes message's body kind rides**: bits 47:40 of the first word. A later structured
/// body arrives as a new kind rather than a new framing; [`KIND_TEXT`] is the only kind today, and
/// [`unpack`] refuses a bytes message of any other kind. Bits 55:48 stay zero.
pub const KIND_SHIFT: u32 = 40;

/// The one body kind that exists: UTF-8 text, which is what every writer already sends, and 0 for
/// the reason [`OPERATION_BYTES`] is 0: the wire was already saying it.
pub const KIND_TEXT: u8 = 0;

/// The syslog level a bytes message's first word carries, or `None` when the writer did not set
/// one (see [`SEVERITY_SHIFT`]). A field value past 8 is not a level and also reads as `None`: the
/// sink infers rather than trusting a number it cannot name.
pub const fn severity(word_zero: u64) -> Option<u8> {
    let field = ((word_zero >> SEVERITY_SHIFT) & 0xff) as u8;
    if field == 0 || field > 8 {
        None
    } else {
        Some(field - 1)
    }
}

/// `word_zero` with its severity set to syslog `level` (0 to 7; a larger value is clamped to 7, debug, the
/// least alarming reading of a level nobody defined). It acts on any first word, but only a bytes
/// message's bits are read.
pub const fn with_severity(word_zero: u64, level: u8) -> u64 {
    let level = if level > 7 { 7 } else { level };
    (word_zero & !(0xff << SEVERITY_SHIFT)) | (((level as u64) + 1) << SEVERITY_SHIFT)
}

/// The body kind a bytes message's first word carries (see [`KIND_SHIFT`]).
pub const fn kind(word_zero: u64) -> u8 {
    ((word_zero >> KIND_SHIFT) & 0xff) as u8
}

/// What one received message was.
///
/// Name: ratified 2026-11-21 (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// [`OPERATION_BYTES`]: this many bytes were written into the caller's buffer, 0 to
    /// [`MAXIMUM_INLINE_BYTES`].
    Bytes(usize),
    /// [`OPERATION_END_OF_STREAM`]: the writer is finished. Only the exact message `(end_of_stream(), 0, 0)` decodes
    /// as this.
    ///
    /// Name: ratified 2026-11-21 (UTC). Spelled out; `eof` is kept as a documentation alias.
    #[doc(alias = "eof")]
    EndOfStream,
    /// An opcode this contract does not define, a bytes message whose body kind is not
    /// [`KIND_TEXT`], a byte count past [`MAXIMUM_INLINE_BYTES`], or an end-of-stream message with a
    /// non-zero bit outside its opcode (in the first word's low bits, or in the two reserved words).
    ///
    /// A sink refuses it rather than guessing, and refusing loudly is the point: a length larger
    /// than the message can hold is the shape of a confused or hostile writer, and the read it
    /// would authorize is a read past the end of a buffer. A kind this decoder does not know would
    /// print as text if it were not refused.
    Malformed,
}

/// Unpack a received message into `out`. See [`Message`].
///
/// # Examples
///
/// With [`OPERATION_BYTES`] the first word is the byte count, and a length the message cannot
/// hold is refused rather than obeyed:
///
/// ```
/// use byte_sink_contract::{
///     MAXIMUM_INLINE_BYTES, Message, MessageWords, OPERATION_BYTES, first_word, len, operation, pack, unpack,
/// };
///
/// let (words, _) = pack(b"hi");
/// assert_eq!(words.word_zero, 2); // the whole word, not just its low bits
/// assert_eq!(operation(words.word_zero), OPERATION_BYTES);
/// assert_eq!(len(words.word_zero), 2);
///
/// // A writer claiming more bytes than three words can carry is malformed, not generous.
/// let lying = first_word(OPERATION_BYTES, MAXIMUM_INLINE_BYTES as u32 + 1);
/// assert_eq!(unpack(MessageWords::new(lying, 0, 0), &mut [0u8; MAXIMUM_INLINE_BYTES]), Message::Malformed);
/// ```
///
/// An end of stream is the one exact message `(end_of_stream(), 0, 0)`. Its two reserved words carry
/// nothing, and a writer that puts something in them is refused:
///
/// ```
/// use byte_sink_contract::{MAXIMUM_INLINE_BYTES, Message, MessageWords, end_of_stream, unpack};
///
/// let mut buf = [0u8; MAXIMUM_INLINE_BYTES];
/// assert_eq!(unpack(MessageWords::new(end_of_stream(), 0, 0), &mut buf), Message::EndOfStream);
/// // A status in a reserved word:
/// assert_eq!(unpack(MessageWords::new(end_of_stream(), 0, 2), &mut buf), Message::Malformed);
/// // A length on an end of stream:
/// assert_eq!(unpack(MessageWords::new(end_of_stream() | 1, 0, 0), &mut buf), Message::Malformed);
/// ```
pub fn unpack(words: MessageWords, out: &mut [u8; MAXIMUM_INLINE_BYTES]) -> Message {
    let MessageWords {
        word_zero,
        word_one,
        word_two,
    } = words;
    match operation(word_zero) {
        OPERATION_BYTES => {
            // Compared as `u64`, so a target whose `usize` is narrower cannot truncate the count.
            if word_zero & 0xffff_ffff > MAXIMUM_INLINE_BYTES as u64 || kind(word_zero) != KIND_TEXT
            {
                return Message::Malformed;
            }
            let n = len(word_zero);
            out[..8].copy_from_slice(&word_one.to_le_bytes());
            out[8..].copy_from_slice(&word_two.to_le_bytes());
            Message::Bytes(n)
        }
        OPERATION_END_OF_STREAM
            if word_zero == end_of_stream() && word_one == 0 && word_two == 0 =>
        {
            Message::EndOfStream
        }
        _ => Message::Malformed,
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
pub const fn classify(returned: i64) -> Sent {
    if returned >= 0 {
        Sent::Ok
    } else if returned == GONE {
        Sent::Gone
    } else {
        Sent::NoSink
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A round trip through the wire words, at the boundaries that matter: a message that fills
    /// both words, one that fills neither, and one that straddles them.
    #[test]
    fn bytes_survive_the_wire_words() {
        for n in 0..=MAXIMUM_INLINE_BYTES {
            let src: [u8; MAXIMUM_INLINE_BYTES] =
                core::array::from_fn(|i| (i as u8).wrapping_mul(37) ^ 0x5a);
            let (
                MessageWords {
                    word_zero,
                    word_one,
                    word_two,
                },
                taken,
            ) = pack(&src[..n]);
            assert_eq!(taken, n);
            let mut out = [0u8; MAXIMUM_INLINE_BYTES];
            assert_eq!(
                unpack(MessageWords::new(word_zero, word_one, word_two), &mut out),
                Message::Bytes(n)
            );
            assert_eq!(&out[..n], &src[..n]);
        }
    }

    /// The wire is fixed: the packed words of a known input are the values every sink already
    /// decodes, and the opcode and end of stream occupy the bits they always did.
    #[test]
    fn the_packed_words_of_a_known_input_are_the_wire_values() {
        let (words, taken) = pack(b"0123456789abcdefXYZ");
        assert_eq!(taken, 16);
        assert_eq!(
            words,
            MessageWords::new(16, 0x3736_3534_3332_3130, 0x6665_6463_6261_3938)
        );
        assert_eq!(first_word(OPERATION_BYTES, 4), 4);
        assert_eq!(end_of_stream(), 1 << 56);
        assert_eq!(first_word(0xff, u32::MAX), 0xff00_0000_ffff_ffff);
        assert_eq!(operation(0xab00_0000_0000_0001), 0xab);
    }

    /// A caller handing over more than one message holds gets a full message and an honest count,
    /// not a truncated write it cannot detect.
    #[test]
    fn pack_takes_a_full_message_and_says_so() {
        let src = [7u8; 40];
        let (MessageWords { word_zero, .. }, taken) = pack(&src);
        assert_eq!(taken, MAXIMUM_INLINE_BYTES);
        assert_eq!(len(word_zero), MAXIMUM_INLINE_BYTES);
    }

    /// The framing that makes the unification free: with [`OPERATION_BYTES`] at zero, a bytes message's
    /// first word IS its length, which is the wire std's stdout sent before this contract existed.
    #[test]
    fn a_bytes_messages_first_word_is_its_length() {
        let (MessageWords { word_zero, .. }, _) = pack(b"abcd");
        assert_eq!(word_zero, 4);
    }

    /// The severity bits ride beside the count without disturbing it: every sink that reads only
    /// [`len`] and [`operation`] (a terminal, a pipe, a file) decodes a severity-carrying message exactly
    /// as it decoded one without, and a writer that never set a level reads as "not set", never as
    /// level 0, which is emergency.
    #[test]
    fn severity_rides_beside_the_count_and_unset_is_not_emergency() {
        let (
            MessageWords {
                word_zero,
                word_one,
                word_two,
            },
            _,
        ) = pack(b"disk full");
        assert_eq!(severity(word_zero), None);
        let marked = with_severity(word_zero, 3);
        assert_eq!(severity(marked), Some(3));
        assert_eq!(len(marked), 9);
        assert_eq!(operation(marked), OPERATION_BYTES);
        assert_eq!(kind(marked), KIND_TEXT);
        let mut out = [0u8; MAXIMUM_INLINE_BYTES];
        assert_eq!(
            unpack(MessageWords::new(marked, word_one, word_two), &mut out),
            Message::Bytes(9)
        );
        assert_eq!(&out[..9], b"disk full");
        assert_eq!(severity(with_severity(word_zero, 0)), Some(0));
        assert_eq!(severity(with_severity(word_zero, 200)), Some(7));
        assert_eq!(severity(word_zero | (9 << SEVERITY_SHIFT)), None);
    }

    /// An end of stream is distinguishable from every byte count, including zero, which is the property that
    /// lets one word carry both.
    #[test]
    fn end_of_stream_is_not_a_byte_count() {
        let mut out = [0u8; MAXIMUM_INLINE_BYTES];
        assert_eq!(
            unpack(MessageWords::new(end_of_stream(), 0, 0), &mut out),
            Message::EndOfStream
        );
        let (
            MessageWords {
                word_zero,
                word_one,
                word_two,
            },
            _,
        ) = pack(&[]);
        assert_eq!(
            unpack(MessageWords::new(word_zero, word_one, word_two), &mut out),
            Message::Bytes(0)
        );
        assert_ne!(end_of_stream(), word_zero);
    }

    /// A length past what a message can hold, and an opcode nobody defined, are refused rather than
    /// clamped. Clamping a length would turn a hostile writer's message into a plausible one.
    #[test]
    fn a_malformed_message_is_refused_not_repaired() {
        let mut out = [0u8; MAXIMUM_INLINE_BYTES];
        assert_eq!(
            unpack(
                MessageWords::new(first_word(OPERATION_BYTES, 17), 0, 0),
                &mut out
            ),
            Message::Malformed,
            "a byte count past MAXIMUM_INLINE_BYTES must not be clamped",
        );
        assert_eq!(
            unpack(
                MessageWords::new(first_word(OPERATION_BYTES, u32::MAX), 0, 0),
                &mut out
            ),
            Message::Malformed,
        );
        assert_eq!(
            unpack(MessageWords::new(first_word(9, 4), 0, 0), &mut out),
            Message::Malformed
        );
    }

    /// An end-of-stream message with a bit set in either reserved word, or anywhere in the first
    /// word below the opcode, is refused rather than read as an end: a sink that took it as one
    /// would accept a meaning in words the contract reserves.
    #[test]
    fn an_end_of_stream_with_a_non_zero_reserved_bit_is_malformed() {
        let mut out = [0u8; MAXIMUM_INLINE_BYTES];
        for bit in 0..64 {
            assert_eq!(
                unpack(MessageWords::new(end_of_stream(), 1 << bit, 0), &mut out),
                Message::Malformed,
                "second word, bit {bit}",
            );
            assert_eq!(
                unpack(MessageWords::new(end_of_stream(), 0, 1 << bit), &mut out),
                Message::Malformed,
                "third word, bit {bit}",
            );
        }
        for bit in 0..OPERATION_SHIFT {
            assert_eq!(
                unpack(
                    MessageWords::new(end_of_stream() | (1 << bit), 0, 0),
                    &mut out
                ),
                Message::Malformed,
                "first word, bit {bit}",
            );
        }
        assert_eq!(
            unpack(MessageWords::new(end_of_stream(), 0, 0), &mut out),
            Message::EndOfStream
        );
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
    fn a_severity_set_twice_is_the_second_and_the_neighbors_are_untouched() {
        let word_zero = with_severity((0x2a << KIND_SHIFT) | 9, 5);
        assert_eq!(severity(word_zero), Some(5));
        let again = with_severity(word_zero, 2);
        assert_eq!(severity(again), Some(2), "not 5 | 2");
        assert_eq!(kind(again), 0x2a);
        assert_eq!(len(again), 9);
        assert_eq!(severity(with_severity(0, 200)), Some(7));
        assert_eq!(severity(with_severity(0, 7)), Some(7));
    }

    /// The body kind is its own byte, between the severity above it and nothing below it but the
    /// count: neither neighbor leaks in, and it reads back whole.
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

    /// A bytes message of a body kind this contract does not define is refused, so a future
    /// structured body never prints as text through a decoder that predates it.
    #[test]
    fn a_bytes_message_of_another_kind_is_refused() {
        let mut out = [0u8; MAXIMUM_INLINE_BYTES];
        let (
            MessageWords {
                word_zero,
                word_one,
                word_two,
            },
            _,
        ) = pack(b"text");
        assert_eq!(
            unpack(MessageWords::new(word_zero, word_one, word_two), &mut out),
            Message::Bytes(4)
        );
        let other = word_zero | (1 << KIND_SHIFT);
        assert_eq!(
            unpack(MessageWords::new(other, word_one, word_two), &mut out),
            Message::Malformed
        );
        // The severity beside the kind does not move the line between the two.
        assert_eq!(
            unpack(
                MessageWords::new(with_severity(word_zero, 2), word_one, word_two),
                &mut out
            ),
            Message::Bytes(4)
        );
    }
}

#[cfg(kani)]
mod verification;
