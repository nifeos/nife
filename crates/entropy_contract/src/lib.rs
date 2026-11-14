//! The entropy contract (entropy is a capability): the request and reply that carry random bytes
//! out of the entropy service, defined once for the service, its clients and their tests.
//!
//! One process holds the entropy device. A client holds an endpoint that names no device, `CALL`s
//! with a byte count and gets the bytes in the reply words, at most [`MAXIMUM_BYTES`] per round
//! trip. There is no shared page, against the usual bulk rule, because random bytes are worth
//! exactly that nobody else has seen them. No byte is transformed, so the claim stays "these are
//! the device's bytes".

//! # BUGS
//!
//! - An all-zero first draw is read as a dead device ([`readiness`]): wrong with probability
//!   2^-64 to 2^-2048 by source, and the safe way round. A first draw of eight or more equal
//!   nonzero bytes is read the same way: wrong with probability at most 255 in 2^64 from a uniform
//!   source.
//! - The check is bring-up only. Continuous health testing is a decision still to be made.
//! - A service answers an unknown operation with [`NO_ENTROPY`], so a client cannot tell that
//!   from a dry source. The reply has no room for a third answer.
//! - No request carries [`VERSION`], and the service answers no query for the versions it serves,
//!   so a client built against another version is not refused at bind.
//!
//! Name: ratified 2026-08-23 (calef): the service's own word, `entropy`, with the contract suffix
//! `_contract` (calef's 2026-10-18 ruling).

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

/// The most bytes one request can carry: the reply's second word.
///
/// A `CALL` replies with two words. The first is the count, so the bytes get exactly one word.
/// Asking for more is not an error. The service answers with this many and the caller loops; see
/// [`requested_bytes`].
///
/// Name: ratified 2026-10-27 (#2797).
pub const MAXIMUM_BYTES: u64 = 8;

/// Where a request packs its opcode: bits 63:56 of the first `CALL` word, the position its sibling
/// message contracts use, so the contracts read alike.
///
/// Name: ratified 2026-11-11 (#3480): kept as written; six sibling contract crates
/// share the name and value.
pub const OPERATION_SHIFT: u32 = 56;

/// The protocol's version, one for the operation set and the reply shape together: one constant
/// per wire format. A break bumps it. A new opcode is an addition and owes no bump, since a
/// service that does not know it answers [`NO_ENTROPY`]. A new field in the request word is not
/// an addition: a service refuses every request with bits 55:8 set (see [`request_is_well_formed`]),
/// so a field there is a new operation or a bump of this constant. A program declares it beside [`INTERFACE_ID`]. No peer checks it at bind yet (see this crate's
/// `BUGS`).
///
/// Name: ratified 2026-11-01 (#3059). The name every contract crate uses for its version.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 12, naming the entropy service
/// request and reply.
///
/// The number is this interface's for good. No other interface takes it, and a format change is a
/// new version, never a new number. The project that links these crates keeps the table of numbers
/// and holds it to that rule. A program declares the pair of this id and [`VERSION`].
///
/// Name: ratified 2026-11-01 (#3059). The name every contract crate uses for its interface identity.
pub const INTERFACE_ID: u64 = 12;

/// Give me this many random bytes. The only operation. A service answers any other opcode with
/// [`NO_ENTROPY`]. [`request`] and [`requested_bytes`] clamp
/// the byte count to [`MAXIMUM_BYTES`], so a client that asks for more is answered rather than
/// refused.
///
/// Name: ratified 2026-11-11 (#3480): kept as written.
pub const GET: u64 = 1;

/// Build a request word: the opcode and the byte count the client wants.
///
/// A count above [`MAXIMUM_BYTES`] encodes as [`MAXIMUM_BYTES`]. The wire never carries more than
/// a reply can hold, and a client that asks for 256 bytes is not mistaken for one that asks for
/// none. The opcode must fit the top byte, which a debug build checks.
///
/// Name: ratified 2026-11-11 (#3480): renamed from `req`, which abbreviates it.
#[must_use]
pub const fn request(operation: u64, byte_count: u64) -> u64 {
    debug_assert!(
        operation <= 0xff,
        "an opcode fits the top byte of a request"
    );
    let clamped = if byte_count > MAXIMUM_BYTES {
        MAXIMUM_BYTES
    } else {
        byte_count
    };
    (operation << OPERATION_SHIFT) | clamped
}

/// Whether a request word is well formed: bits 55:8, which carry no defined meaning, are zero.
///
/// A service answers a request for which this is false with [`NO_ENTROPY`], whatever its opcode.
/// A client sends zero in those bits, which [`request`] does. Refusing what is not defined, rather
/// than ignoring it, leaves those bits free for a later version to give a meaning without an old
/// service misreading it. The opcode and the byte count are not judged here: an unknown opcode is
/// answered [`NO_ENTROPY`] by the service's own dispatch, and a count above [`MAXIMUM_BYTES`] is
/// clamped by [`requested_bytes`].
///
/// # Examples
///
/// ```
/// use entropy_contract::{GET, request, request_is_well_formed};
///
/// assert!(request_is_well_formed(request(GET, 8)));
/// assert!(!request_is_well_formed(request(GET, 8) | (1 << 8)));
/// assert!(!request_is_well_formed(request(GET, 8) | (1 << 55)));
/// ```
///
/// Name: provisional, minted for the reserved bits of the request word; awaiting a naming pass.
#[must_use]
pub const fn request_is_well_formed(request_word: u64) -> bool {
    request_word & RESERVED_REQUEST_BITS == 0
}

/// Bits 55:8 of a request word: reserved, sent as zero.
const RESERVED_REQUEST_BITS: u64 = ((1u64 << OPERATION_SHIFT) - 1) & !0xff;

/// The opcode of a request word.
///
/// Name: ratified 2026-11-11 (#3480): kept as written, its parameter renamed from
/// `w0`.
#[must_use]
pub const fn operation(request_word: u64) -> u64 {
    request_word >> OPERATION_SHIFT
}

/// How many bytes a request asks for, clamped to what one reply can carry.
///
/// Clamping here rather than refusing keeps the server's loop free of an error path that only a
/// buggy client reaches. A request for 500 bytes is a request for eight, answered, and the client
/// asks again.
///
/// Name: ratified 2026-11-11 (#3480): renamed from `want`: a quantity takes its unit.
#[must_use]
pub const fn requested_bytes(request_word: u64) -> u64 {
    let byte_count = request_word & 0xff;
    if byte_count > MAXIMUM_BYTES {
        MAXIMUM_BYTES
    } else {
        byte_count
    }
}

/// The word the entropy service SENDs on its readiness endpoint once the device is up and its
/// first bytes are in hand. ASCII `RNGUP`, so a hex dump of a report reads.
///
/// A bring-up failure reports [`BRING_UP_FAILED`]` | step` instead, a shape any driver can share
/// for its own bring-up report. "Its first bytes are in hand" is a claim about the bytes, not
/// about the handshake. A service holding nothing, or a bufferful of zeros, must not send it, and
/// [`readiness`] makes the claim.
///
/// Name: ratified 2026-11-11 (#3480): kept as written.
pub const READY: u64 = 0x_52_4E_47_55_50;

/// The word a bring-up failure reports instead of [`READY`], with the step that failed in its low
/// byte: `0xDEAD_0000_0000_0000 | step`.
///
/// It lives here rather than in each driver because it is the other half of the readiness
/// contract. A reader decoding a report word should not have to find the literal in each driver.
///
/// Name: ratified 2026-11-11 (#3480), renamed from `DEAD`; `DEAD` refused.
pub const BRING_UP_FAILED: u64 = 0xDEAD_0000_0000_0000;

/// The service asked and got nothing at all: the device never answered inside the driver's own
/// bound. A dry source, an absent one, or a register window that reads as nothing.
///
/// Steps `0x01..=0x0f` are each backend's own, so the shared steps start at `0x10` and cannot
/// collide with them.
///
/// Name: ratified 2026-11-11 (#3480): kept as written.
pub const STEP_NO_FIRST_BYTES: u64 = 0x10;

/// The service asked, was answered, and every byte of the answer was zero. See [`readiness`] for
/// why that is treated as a dead device rather than as randomness.
///
/// Name: ratified 2026-11-11 (#3480): kept as written.
pub const STEP_FIRST_ALL_ZERO: u64 = 0x11;

/// The service asked, was answered, and the answer was one nonzero value repeated: at least
/// [`MAXIMUM_BYTES`] bytes, every one equal to the first.
///
/// A source stuck on a latched value is the other way a generator fails at start, and it passes a
/// check that only looks for a nonzero byte. See [`readiness`].
///
/// Name: provisional, minted for the readiness ruling of 2026-11-11; awaiting a naming pass.
pub const STEP_FIRST_ALL_SAME: u64 = 0x12;

/// Build a bring-up failure's report word from the step that failed. A step fits the low byte.
///
/// Name: ratified 2026-11-11 (#3480): renamed from `bringup_failure`; `bringup` is not
/// a word.
#[must_use]
pub const fn bring_up_failure(step: u64) -> u64 {
    debug_assert!(step <= 0xff, "a step fits the low byte of a report word");
    BRING_UP_FAILED | step
}

/// Decide what a service may say about itself, given the first bytes it drew.
///
/// The answer is [`READY`] only when the draw holds a nonzero byte and is not one value repeated.
/// Otherwise it is a [`bring_up_failure`] naming which of the three shared steps failed. Every
/// backend calls this rather than composing the word itself.
///
/// `first_bytes` is an iterator because backends hold their first draw in different places. An
/// empty iterator is [`STEP_NO_FIRST_BYTES`], a different answer from a bufferful of zeros, which
/// is [`STEP_FIRST_ALL_ZERO`]. A draw of at least [`MAXIMUM_BYTES`] bytes that repeats one nonzero
/// value is [`STEP_FIRST_ALL_SAME`]. A shorter draw is not judged for repeats, since two equal
/// bytes out of two are common for a working source. This is a start-up check, not a continuous
/// health test. Refusing these draws is a deliberate correctness claim about a random variable;
/// see this crate's `BUGS`.
///
/// # Examples
///
/// ```
/// use entropy_contract::{
///     READY, STEP_FIRST_ALL_SAME, STEP_FIRST_ALL_ZERO, STEP_NO_FIRST_BYTES, bring_up_failure,
///     readiness,
/// };
///
/// // A device that answered with real bytes.
/// assert_eq!(readiness([0, 0, 0, 7, 0]), READY);
///
/// // A device that answered with nothing, and one that answered with zeros. Two different
/// // failures, and the report word says which.
/// assert_eq!(readiness([]), bring_up_failure(STEP_NO_FIRST_BYTES));
/// assert_eq!(readiness([0; 32]), bring_up_failure(STEP_FIRST_ALL_ZERO));
/// assert_ne!(readiness([0; 32]), READY);
///
/// // A source stuck on one nonzero value answered, and answered the same thing eight times.
/// assert_eq!(readiness([0xff; 8]), bring_up_failure(STEP_FIRST_ALL_SAME));
/// ```
///
/// Name: ratified 2026-11-11 (#3480): kept as written, its parameter renamed from
/// `first`.
pub fn readiness(first_bytes: impl IntoIterator<Item = u8>) -> u64 {
    let mut first = 0;
    let mut count = 0usize;
    let mut all_same = true;
    let mut any_nonzero = false;
    for byte in first_bytes {
        if count == 0 {
            first = byte;
        } else if byte != first {
            all_same = false;
        }
        any_nonzero |= byte != 0;
        count = count.saturating_add(1);
    }
    if count == 0 {
        bring_up_failure(STEP_NO_FIRST_BYTES)
    } else if !any_nonzero {
        bring_up_failure(STEP_FIRST_ALL_ZERO)
    } else if all_same && count >= MAXIMUM_BYTES as usize {
        bring_up_failure(STEP_FIRST_ALL_SAME)
    } else {
        READY
    }
}

/// The reply's first word when the service has no entropy to give. Zero bytes delivered, and the
/// second word is meaningless.
///
/// The service sends this rather than padding, repeating an earlier answer, or substituting a
/// pseudo-random stand-in. A caller that cannot be given randomness must find out, because the
/// alternative is a silent degradation that no caller should have to guess at.
///
/// Name: ratified 2026-11-11 (#3480): kept as written.
pub const NO_ENTROPY: u64 = 0;

/// Read a reply's first word.
///
/// `Some(count)` when the entropy service answered, with `count` the number of valid low bytes in
/// the second word (0 means [`NO_ENTROPY`]). `None` for every word above [`MAXIMUM_BYTES`]: no
/// reply carries such a count, so the word did not come from the service.
///
/// The rule this rests on is the caller's. The kernel it runs on must report every failure of a
/// `CALL` as a word above [`MAXIMUM_BYTES`]. A kernel that returns a small negative error number
/// meets it, since each reads as an enormous `u64`, and then a caller holding no entropy
/// capability finds out without a probe.
///
/// # Examples
///
/// A count and a kernel error cannot be confused, and "none to give" is a third answer:
///
/// ```
/// use entropy_contract::{MAXIMUM_BYTES, NO_ENTROPY, delivered_bytes};
///
/// // Every count the service can send.
/// for byte_count in 0..=MAXIMUM_BYTES {
///     assert_eq!(delivered_bytes(byte_count), Some(byte_count as usize));
/// }
/// // A small negative error number reads as an enormous u64, so none can be read as a count.
/// assert_eq!(delivered_bytes(-4i64 as u64), None); // an empty slot: no entropy capability
///
/// // And "the service has none to give" is a third answer, distinct from both.
/// assert_eq!(delivered_bytes(NO_ENTROPY), Some(0));
/// ```
///
/// Name: ratified 2026-11-11 (#3480): renamed from `delivered`: it returns a byte
/// count.
#[must_use]
pub const fn delivered_bytes(first_reply_word: u64) -> Option<usize> {
    if first_reply_word <= MAXIMUM_BYTES {
        Some(first_reply_word as usize)
    } else {
        None
    }
}

/// Copy `byte_count` delivered bytes out of a reply's second word into `out`, returning how many
/// landed: the least of `byte_count`, [`MAXIMUM_BYTES`] and the length of `out`.
///
/// Little-endian, and only the low `byte_count` bytes are meaningful. The rest of the word is
/// zero: a service puts nothing but zeros past the delivered bytes (see [`reply`]). Those bytes
/// carry no entropy, so a caller that read them would be mixing zeros into a key and believing
/// otherwise. The returned count is the
/// result: a caller that ignores it has filled less than it thinks.
///
/// # Examples
///
/// A reply is eight bytes, so filling anything larger is a loop: a twenty-byte key is three round
/// trips, the last one short.
///
/// ```
/// use entropy_contract::{GET, MAXIMUM_BYTES, delivered_bytes, operation, request, copy_delivered_bytes, requested_bytes};
///
/// // Stands in for the `CALL`: the real service reads the same two accessors off the request word
/// // and answers with a count and a word of bytes.
/// fn service(request_word: u64) -> (u64, u64) {
///     assert_eq!(operation(request_word), GET);
///     (requested_bytes(request_word), 0x0807_0605_0403_0201)
/// }
///
/// let mut key = [0u8; 20];
/// let mut filled = 0;
/// let mut round_trips = 0;
/// while filled < key.len() {
///     let (first_reply_word, reply_word) = service(request(GET, (key.len() - filled) as u64));
///     round_trips += 1;
///     // `None` would mean the word is a kernel error rather than a count. See below.
///     let byte_count = delivered_bytes(first_reply_word).expect("the service answered");
///     assert_ne!(byte_count, 0, "a zero count is NO_ENTROPY: fail, do not retry");
///     filled += copy_delivered_bytes(byte_count, reply_word, &mut key[filled..]);
/// }
/// assert_eq!(round_trips, 3); // 8 + 8 + 4, because MAXIMUM_BYTES is 8
/// assert_eq!(MAXIMUM_BYTES, 8);
/// assert_eq!(&key[16..], &[0x01, 0x02, 0x03, 0x04]);
/// ```
///
/// Name: ratified 2026-11-11 (#3480): renamed from `take`, since std's `take` moves a
/// value out and this copies bytes.
#[must_use]
pub fn copy_delivered_bytes(byte_count: usize, reply_word: u64, out: &mut [u8]) -> usize {
    let copied = byte_count.min(MAXIMUM_BYTES as usize).min(out.len());
    out[..copied].copy_from_slice(&reply_word.to_le_bytes()[..copied]);
    copied
}

/// Build a reply from the bytes a service delivers: the count and the word that carries them.
///
/// At most [`MAXIMUM_BYTES`] bytes are delivered, and any beyond that are left out. The word holds
/// them little-endian in its low bytes and zeros above, and a service puts nothing else in the
/// word: bytes past the delivered count are zero, so a reply never carries a byte the client did
/// not ask for, and none that another client could be handed. Reading the pair back with
/// [`delivered_bytes`] and [`copy_delivered_bytes`] yields the delivered bytes, so a service and
/// its tests make the reply with the same function the client reads it with.
///
/// # Examples
///
/// ```
/// use entropy_contract::{copy_delivered_bytes, delivered_bytes, reply};
///
/// let (first_reply_word, reply_word) = reply(&[1, 2, 3]);
/// assert_eq!((first_reply_word, reply_word), (3, 0x0003_0201));
///
/// let mut out = [0u8; 8];
/// let byte_count = delivered_bytes(first_reply_word).expect("a count");
/// assert_eq!(copy_delivered_bytes(byte_count, reply_word, &mut out), 3);
/// assert_eq!(out[..3], [1, 2, 3]);
/// ```
///
/// Name: provisional, minted for the service side of the contract; awaiting a naming pass.
#[must_use]
pub fn reply(delivered: &[u8]) -> (u64, u64) {
    let count = delivered.len().min(MAXIMUM_BYTES as usize);
    let mut bytes = [0u8; MAXIMUM_BYTES as usize];
    bytes[..count].copy_from_slice(&delivered[..count]);
    (count as u64, u64::from_le_bytes(bytes))
}

/// Why [`fill_with`] gave up. The two refusals call for different fixes, so they are told apart.
///
/// Name: provisional, minted for the error of `fill_with`; awaiting a naming pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusal {
    /// A reply's first word was not a count (see [`delivered_bytes`]). No service answered: the
    /// caller holds no entropy capability, or the endpoint is gone.
    ///
    /// Name: provisional, minted with [`Refusal`]; awaiting a naming pass.
    NoService,
    /// A reply carried a count other than the one asked for. A count below it, zero included
    /// ([`NO_ENTROPY`]), means the service is dry.
    ///
    /// Name: provisional, minted with [`Refusal`]; awaiting a naming pass.
    ShortReply {
        /// The byte count the request asked for.
        asked: usize,
        /// The byte count the reply delivered.
        delivered: usize,
    },
}

/// Fill `out` with random bytes, one request at a time, refusing a short answer. `call` is the
/// `CALL` on the entropy endpoint: it takes a request word and returns the two reply words.
///
/// Refuses with [`Refusal::NoService`] if any reply is not a count, and with
/// [`Refusal::ShortReply`] if one delivers other than the bytes asked for. After a refusal `out`
/// is unspecified and must not be used. The function asks for [`MAXIMUM_BYTES`] at a time and the
/// remainder last. The `CALL` is a parameter so this crate makes no syscall, and the caller
/// passes a real one. The standard library's platform abstraction layer accepts partial counts
/// instead, deliberately.
///
/// Name: ratified 2026-10-07 (#1795). std's `_with` suffix, for a function that takes a closure.
pub fn fill_with(out: &mut [u8], mut call: impl FnMut(u64) -> (u64, u64)) -> Result<(), Refusal> {
    let mut filled = 0;
    while filled < out.len() {
        let asked = (out.len() - filled).min(MAXIMUM_BYTES as usize);
        let (first_reply_word, reply_word) = call(request(GET, asked as u64));
        let delivered = delivered_bytes(first_reply_word).ok_or(Refusal::NoService)?;
        if delivered != asked {
            return Err(Refusal::ShortReply { asked, delivered });
        }
        filled += copy_delivered_bytes(asked, reply_word, &mut out[filled..]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A full service: sixteen bytes in two round trips, each answered with what it asked for, in
    /// order.
    #[test]
    fn sixteen_bytes_take_two_calls() {
        let mut calls = 0;
        let mut out = [0u8; 16];
        let got = fill_with(&mut out, |request_word| {
            calls += 1;
            assert_eq!(operation(request_word), GET);
            (requested_bytes(request_word), 0x0807_0605_0403_0201 * calls)
        });
        assert_eq!(got, Ok(()));
        assert_eq!(calls, 2);
        assert_eq!(out[..8], [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(out[8..], [2, 4, 6, 8, 10, 12, 14, 16]);
    }

    /// Only bits 55:8 make a request ill formed; the opcode and the count do not.
    #[test]
    fn reserved_request_bits_are_refused() {
        assert!(request_is_well_formed(request(GET, 0)));
        assert!(request_is_well_formed(request(0xff, 8)));
        assert!(request_is_well_formed(0xff));
        for bit in 8..56 {
            assert!(!request_is_well_formed(request(GET, 8) | (1 << bit)), "bit {bit}");
        }
        for bit in (0..8).chain(56..64) {
            assert!(request_is_well_formed(1 << bit), "bit {bit}");
        }
    }

    /// A reply holds zeros past the delivered count, for every length including one too long.
    #[test]
    fn a_reply_is_zero_past_the_delivered_bytes() {
        for length in 0..=12usize {
            let bytes = [0xffu8; 12];
            let (count, word) = reply(&bytes[..length]);
            let delivered = length.min(MAXIMUM_BYTES as usize);
            assert_eq!(count, delivered as u64);
            for (index, byte) in word.to_le_bytes().iter().enumerate() {
                assert_eq!(*byte, if index < delivered { 0xff } else { 0 }, "{length}/{index}");
            }
        }
    }

    /// A short answer on the second trip refuses the whole fill, and the refusal says what was
    /// asked and what came.
    #[test]
    fn a_short_answer_is_a_refusal() {
        let mut trip = 0;
        let got = fill_with(&mut [0u8; 16], |_| {
            trip += 1;
            (if trip == 2 { 7 } else { 8 }, 0)
        });
        assert_eq!(
            got,
            Err(Refusal::ShortReply {
                asked: 8,
                delivered: 7
            })
        );
    }

    /// No capability: an empty slot's error in the count register, and no second call.
    #[test]
    fn a_missing_endpoint_is_a_refusal_after_one_call() {
        let mut calls = 0;
        let got = fill_with(&mut [0u8; 16], |_| {
            calls += 1;
            (-4i64 as u64, 0)
        });
        assert_eq!(got, Err(Refusal::NoService));
        assert_eq!(calls, 1);
    }

    /// A dry service (zero bytes, `NO_ENTROPY`) is a refusal too, not a loop that never ends, and
    /// it is a different refusal from a missing service.
    #[test]
    fn a_dry_service_is_a_refusal() {
        assert_eq!(
            fill_with(&mut [0u8; 8], |_| (NO_ENTROPY, 0)),
            Err(Refusal::ShortReply {
                asked: 8,
                delivered: 0
            })
        );
    }

    /// A length that is not a multiple of eight asks for the remainder last, not eight.
    #[test]
    fn the_last_request_asks_for_the_remainder() {
        let mut asked = [0u64; 3];
        let mut index = 0;
        let got = fill_with(&mut [0u8; 20], |request_word| {
            asked[index] = requested_bytes(request_word);
            index += 1;
            (requested_bytes(request_word), u64::MAX)
        });
        assert_eq!(got, Ok(()));
        assert_eq!(asked, [8, 8, 4]);
    }

    #[test]
    fn a_request_round_trips_its_opcode_and_count() {
        let request_word = request(GET, 5);
        assert_eq!(operation(request_word), GET);
        assert_eq!(requested_bytes(request_word), 5);
    }

    /// Pins the wire layout as an exact word, with an opcode that is not `GET`. `GET` is 1, so an
    /// `operation` that ignored its input and answered 1 would pass every round trip built from it; this
    /// is the one place the shift is checked against a number the protocol does not define.
    #[test]
    fn the_request_word_layout_is_exact() {
        let request_word = request(0x7f, 5);
        assert_eq!(request_word, 0x7f00_0000_0000_0005);
        assert_eq!(operation(request_word), 0x7f);
        assert_eq!(requested_bytes(request_word), 5);
    }

    /// A client asking for more than one reply can carry is answered with a full reply, not an
    /// error. The clamp lives in the contract so the server and the caller agree on it without
    /// either one enforcing it.
    #[test]
    fn an_oversized_request_is_clamped_rather_than_refused() {
        assert_eq!(requested_bytes(request(GET, 200)), MAXIMUM_BYTES);
        assert_eq!(requested_bytes(request(GET, MAXIMUM_BYTES)), MAXIMUM_BYTES);
    }

    /// `request` clamps rather than truncating: 255, 256 and the largest count all read back as
    /// `MAXIMUM_BYTES`, where a mask on the low byte made 256 a request for nothing.
    #[test]
    fn request_clamps_counts_the_low_byte_cannot_hold() {
        for byte_count in [255, 256, 257, 512, u64::MAX] {
            let request_word = request(GET, byte_count);
            assert_eq!(request_word, (GET << OPERATION_SHIFT) | MAXIMUM_BYTES);
            assert_eq!(operation(request_word), GET);
            assert_eq!(requested_bytes(request_word), MAXIMUM_BYTES);
        }
        assert_eq!(requested_bytes(request(GET, 0)), 0);
    }

    /// The property the whole error story rests on: no count the service can send collides with
    /// any error the kernel can return, so "no entropy capability" is distinguishable from "no
    /// entropy" without a probe. A kernel that answers a failed `CALL` with an error number from -1
    /// to -8 puts one of these words in the first register.
    #[test]
    fn a_kernel_refusal_is_never_mistaken_for_a_byte_count() {
        for byte_count in 0..=MAXIMUM_BYTES {
            assert_eq!(delivered_bytes(byte_count), Some(byte_count as usize));
        }
        for error in 1i64..=8 {
            assert_eq!(
                delivered_bytes((-error) as u64),
                None,
                "kernel error -{error}"
            );
        }
        assert_eq!(delivered_bytes(MAXIMUM_BYTES + 1), None);
    }

    /// A reply holds the delivered bytes low and zeros above, and leaves out any past eight.
    #[test]
    fn a_reply_packs_its_bytes_little_endian_and_clamps() {
        assert_eq!(reply(&[]), (0, 0));
        assert_eq!(reply(&[1, 2, 3]), (3, 0x0003_0201));
        assert_eq!(reply(&[0xff; 8]), (8, u64::MAX));
        assert_eq!(
            reply(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
            (8, 0x0807_0605_0403_0201)
        );
    }

    /// The high bytes of the reply word are not entropy and must not reach the caller's buffer.
    #[test]
    fn only_the_delivered_bytes_are_copied_out() {
        let mut out = [0xAAu8; 8];
        let copied = copy_delivered_bytes(3, 0xFFFF_FFFF_FF03_0201, &mut out);
        assert_eq!(copied, 3);
        assert_eq!(out, [0x01, 0x02, 0x03, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA]);
    }

    /// The defect this function exists to prevent: a service holding a
    /// bufferful of zeros must not be able to report [`READY`]. Asserted against every width a
    /// backend draws, because the predicate is "no nonzero byte anywhere" rather than "the first
    /// word is zero". A check written the second way would pass at 8 bytes and let a 256-byte
    /// direct memory access (DMA) page through.
    #[test]
    fn an_all_zero_first_draw_is_never_ready() {
        // The three widths a backend actually draws: the instruction's eight bytes, the JH7110's
        // thirty-two, the virtio pool's bufferful. Spelled out rather than looped because the
        // predicate is "no nonzero byte anywhere", and a check written as "the first word is zero"
        // would pass at eight bytes and let a 256-byte DMA page through.
        assert_ne!(readiness([0u8; 8]), READY, "8 zero bytes reported ready");
        assert_ne!(readiness([0u8; 32]), READY, "32 zero bytes reported ready");
        assert_ne!(
            readiness([0u8; 256]),
            READY,
            "256 zero bytes reported ready"
        );
        assert_eq!(
            readiness([0u8; 32]),
            bring_up_failure(STEP_FIRST_ALL_ZERO),
            "a zero bufferful must name its own step, not just refuse to be ready",
        );
    }

    /// A device that gave nothing is a different failure from one that gave zeros. Both refuse
    /// [`READY`]. A bench session reading the report word has to tell a gated clock from a source
    /// that answered wrongly, which is what the two steps are for.
    #[test]
    fn nothing_at_all_and_all_zero_bytes_are_distinguishable() {
        assert_eq!(readiness([]), bring_up_failure(STEP_NO_FIRST_BYTES));
        assert_ne!(
            readiness([]),
            readiness([0; 32]),
            "a dry device and a zeroing device report the same step",
        );
        assert_eq!(BRING_UP_FAILED & 0xff, 0, "a step has room in the low bits");
    }

    /// One nonzero byte anywhere is enough, including the last one. A predicate that stopped
    /// looking early would condemn a working device whose first bytes happened to be zeros, which
    /// is the false positive this trade is trying to keep rare rather than common.
    #[test]
    fn a_single_nonzero_byte_anywhere_is_ready() {
        let mut first_bytes = [0u8; 32];
        first_bytes[31] = 1;
        assert_eq!(readiness(first_bytes), READY);
        first_bytes = [0u8; 32];
        first_bytes[0] = 0x80;
        assert_eq!(readiness(first_bytes), READY);
    }

    /// A source stuck on one nonzero value is refused, under its own step, from eight bytes up.
    /// A draw that varies anywhere, a short draw and the doc example's sparse draw stay ready.
    #[test]
    fn a_repeated_nonzero_first_draw_is_refused_under_its_own_step() {
        let refused = bring_up_failure(STEP_FIRST_ALL_SAME);
        assert_eq!(readiness([0xffu8; 8]), refused);
        assert_eq!(readiness([0x5au8; 32]), refused);
        assert_eq!(readiness([1u8; 256]), refused);
        assert_eq!(refused, 0xDEAD_0000_0000_0012);
        assert_ne!(refused, bring_up_failure(STEP_FIRST_ALL_ZERO));
        assert_ne!(refused, bring_up_failure(STEP_NO_FIRST_BYTES));
        assert_eq!(
            readiness([0xffu8; 7]),
            READY,
            "a short draw is not judged for repeats"
        );
        assert_eq!(readiness([0, 0, 0, 7, 0]), READY);
        let mut varied = [0xffu8; 8];
        varied[7] = 0xfe;
        assert_eq!(readiness(varied), READY);
        varied = [0xffu8; 8];
        varied[0] = 0;
        assert_eq!(readiness(varied), READY);
    }

    /// No bring-up failure can be mistaken for the ready word or for a byte count, which is the
    /// same non-overlap property [`delivered_bytes`] rests on, one message earlier in the handshake.
    #[test]
    fn a_bring_up_failure_is_never_ready_and_never_a_count() {
        for step in [
            STEP_NO_FIRST_BYTES,
            STEP_FIRST_ALL_ZERO,
            STEP_FIRST_ALL_SAME,
            0x01,
            0xff,
        ] {
            let word = bring_up_failure(step);
            assert_ne!(word, READY);
            assert_eq!(delivered_bytes(word), None);
            assert_eq!(word & 0xff, step);
        }
    }

    /// A short buffer is the caller's business, not a panic: `copy_delivered_bytes` fills what fits.
    #[test]
    fn a_short_buffer_takes_what_fits() {
        let mut out = [0u8; 2];
        assert_eq!(copy_delivered_bytes(8, u64::MAX, &mut out), 2);
        assert_eq!(out, [0xFF, 0xFF]);
    }
}

#[cfg(kani)]
mod verification {
    use super::*;

    /// A request word carries its opcode and its clamped byte count. For any opcode that fits
    /// the top byte and any byte count, [`operation`] reads back the opcode and
    /// [`requested_bytes`] reads back the count, clamped to [`MAXIMUM_BYTES`].
    /// Falsification: replayable `crates/entropy_contract/falsifications/verification.a_request_word_carries_its_opcode_and_clamped_count.patch`
    #[kani::proof]
    fn a_request_word_carries_its_opcode_and_clamped_count() {
        let opcode: u64 = kani::any();
        kani::assume(opcode <= 0xff);
        let byte_count: u64 = kani::any();
        let request_word = request(opcode, byte_count);
        assert_eq!(operation(request_word), opcode);
        assert_eq!(requested_bytes(request_word), byte_count.min(MAXIMUM_BYTES));
    }

    /// Copying writes exactly the delivered bytes, and nothing past them. For any byte count, any
    /// reply word and any slice of up to twelve bytes, [`copy_delivered_bytes`] returns the least
    /// of the count, [`MAXIMUM_BYTES`] and the slice's length. It writes that many low bytes of the
    /// word in little-endian order and leaves every other byte of the buffer as it was.
    /// Falsification: replayable `crates/entropy_contract/falsifications/verification.copying_writes_exactly_the_delivered_bytes.patch`
    #[kani::proof]
    #[kani::unwind(13)]
    fn copying_writes_exactly_the_delivered_bytes() {
        let byte_count: usize = kani::any();
        let reply_word: u64 = kani::any();
        let mut buffer: [u8; 12] = kani::any();
        let before = buffer;
        let length: usize = kani::any();
        kani::assume(length <= buffer.len());
        let copied = copy_delivered_bytes(byte_count, reply_word, &mut buffer[..length]);
        assert_eq!(copied, byte_count.min(MAXIMUM_BYTES as usize).min(length));
        let bytes = reply_word.to_le_bytes();
        for index in 0..buffer.len() {
            if index < copied {
                assert_eq!(buffer[index], bytes[index]);
            } else {
                assert_eq!(buffer[index], before[index]);
            }
        }
    }

    /// A fill succeeds exactly when every reply is full, and a refusal says which kind. For any
    /// slice of up to seventeen bytes and any replies, [`fill_with`] asks for the remainder clamped
    /// to [`MAXIMUM_BYTES`] each time. It answers `Ok` exactly when every reply delivered what it
    /// asked for, and then holds the delivered bytes in order after one request per eight bytes,
    /// rounded up. Otherwise it stops at the first reply that was not full. The refusal is
    /// [`Refusal::NoService`] exactly when that reply's first word was not a count, and
    /// [`Refusal::ShortReply`], naming what was asked and what came, exactly when it was.
    /// Falsification: replayable `crates/entropy_contract/falsifications/verification.a_fill_succeeds_exactly_when_every_reply_is_full.patch`
    #[kani::proof]
    #[kani::unwind(19)]
    fn a_fill_succeeds_exactly_when_every_reply_is_full() {
        let length: usize = kani::any();
        kani::assume(length <= 17);
        let mut out = [0u8; 17];
        let mut expected = [0u8; 17];
        let mut first_refusal: Option<(u64, usize)> = None;
        let mut position = 0;
        let mut calls = 0;
        let result = fill_with(&mut out[..length], |request_word| {
            assert_eq!(operation(request_word), GET);
            let asked = requested_bytes(request_word) as usize;
            assert_eq!(asked, (length - position).min(MAXIMUM_BYTES as usize));
            let first_reply_word: u64 = kani::any();
            let reply_word: u64 = kani::any();
            if first_reply_word == asked as u64 {
                expected[position..position + asked]
                    .copy_from_slice(&reply_word.to_le_bytes()[..asked]);
                position += asked;
            } else {
                first_refusal = Some((first_reply_word, asked));
            }
            calls += 1;
            (first_reply_word, reply_word)
        });
        match (result, first_refusal) {
            (Ok(()), None) => {
                assert_eq!(position, length);
                assert_eq!(out[..length], expected[..length]);
                assert_eq!(calls, length.div_ceil(MAXIMUM_BYTES as usize));
            }
            (Err(Refusal::NoService), Some((first_reply_word, _))) => {
                assert!(first_reply_word > MAXIMUM_BYTES);
            }
            (Err(Refusal::ShortReply { asked, delivered }), Some((first_reply_word, wanted))) => {
                assert!(first_reply_word <= MAXIMUM_BYTES);
                assert_eq!(delivered as u64, first_reply_word);
                assert_eq!(asked, wanted);
                assert_ne!(delivered, asked);
            }
            _ => panic!("a fill succeeds exactly when no reply was refused"),
        }
    }

    /// A reply reads back as built. For any bytes up to twelve, [`reply`] answers a count of the
    /// least of their number and [`MAXIMUM_BYTES`]. [`delivered_bytes`] accepts that count,
    /// [`copy_delivered_bytes`] returns exactly those bytes in order, and every byte of the word
    /// above the count is zero.
    /// Falsification: replayable `crates/entropy_contract/falsifications/verification.a_reply_reads_back_as_built.patch`
    #[kani::proof]
    #[kani::unwind(13)]
    fn a_reply_reads_back_as_built() {
        let bytes: [u8; 12] = kani::any();
        let length: usize = kani::any();
        kani::assume(length <= bytes.len());
        let (first_reply_word, reply_word) = reply(&bytes[..length]);
        let expected = length.min(MAXIMUM_BYTES as usize);
        assert_eq!(first_reply_word, expected as u64);
        let byte_count = delivered_bytes(first_reply_word).unwrap();
        assert_eq!(byte_count, expected);
        let mut out = [0u8; 12];
        assert_eq!(
            copy_delivered_bytes(byte_count, reply_word, &mut out),
            expected
        );
        assert_eq!(out[..expected], bytes[..expected]);
        let word_bytes = reply_word.to_le_bytes();
        for &byte in &word_bytes[expected..] {
            assert_eq!(byte, 0);
        }
    }

    /// Readiness refuses a repeated first draw, and only a repeated one. For any first draw of up to
    /// nine bytes, [`readiness`] names [`STEP_NO_FIRST_BYTES`] for an empty draw and
    /// [`STEP_FIRST_ALL_ZERO`] for a draw of zeros. It names [`STEP_FIRST_ALL_SAME`] for a draw of
    /// at least [`MAXIMUM_BYTES`] bytes that repeats one nonzero value, and is [`READY`] for every
    /// other draw.
    /// Falsification: replayable `crates/entropy_contract/falsifications/verification.readiness_refuses_a_repeated_first_draw_of_at_least_eight_bytes.patch`
    #[kani::proof]
    #[kani::unwind(11)]
    fn readiness_refuses_a_repeated_first_draw_of_at_least_eight_bytes() {
        let first_bytes: [u8; 9] = kani::any();
        let length: usize = kani::any();
        kani::assume(length <= first_bytes.len());
        let drawn = &first_bytes[..length];
        let word = readiness(drawn.iter().copied());
        let all_zero = drawn.iter().all(|&byte| byte == 0);
        let repeated = drawn.iter().all(|&byte| byte == drawn[0]);
        if length == 0 {
            assert_eq!(word, bring_up_failure(STEP_NO_FIRST_BYTES));
        } else if all_zero {
            assert_eq!(word, bring_up_failure(STEP_FIRST_ALL_ZERO));
        } else if repeated && length >= MAXIMUM_BYTES as usize {
            assert_eq!(word, bring_up_failure(STEP_FIRST_ALL_SAME));
        } else {
            assert_eq!(word, READY);
        }
    }

    /// Readiness is reported exactly when a first byte is nonzero. For any first draw of up to four
    /// bytes, [`readiness`] answers [`READY`] exactly when one of them is nonzero. Otherwise it
    /// names [`STEP_NO_FIRST_BYTES`] for an empty draw and [`STEP_FIRST_ALL_ZERO`] for one of
    /// zeros. A draw this short is never judged for repeats.
    /// Falsification: replayable `crates/entropy_contract/falsifications/verification.readiness_is_reported_exactly_when_a_first_byte_is_nonzero.patch`
    #[kani::proof]
    #[kani::unwind(6)]
    fn readiness_is_reported_exactly_when_a_first_byte_is_nonzero() {
        let first_bytes: [u8; 4] = kani::any();
        let length: usize = kani::any();
        kani::assume(length <= first_bytes.len());
        let drawn = &first_bytes[..length];
        let word = readiness(drawn.iter().copied());
        if drawn.iter().any(|&byte| byte != 0) {
            assert_eq!(word, READY);
        } else if length == 0 {
            assert_eq!(word, bring_up_failure(STEP_NO_FIRST_BYTES));
        } else {
            assert_eq!(word, bring_up_failure(STEP_FIRST_ALL_ZERO));
        }
    }
}
