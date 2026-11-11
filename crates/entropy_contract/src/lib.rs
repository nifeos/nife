//! **The entropy contract** (entropy is a capability): the request and
//! reply that carry random bytes out of the entropy service, defined once for the service, its
//! clients, the standard library's platform abstraction layer and the kernel-side tests. One
//! process holds the virtio-rng device; a client holds an endpoint that names no device, `CALL`s with a byte count and gets the bytes in
//! the reply words, at most [`MAXIMUM_BYTES`] per round trip. No shared page, against the usual
//! bulk rule, because random bytes are worth exactly that nobody else has seen them, and no byte is
//! transformed, so the claim stays "these are the device's bytes".

//! # BUGS
//!
//! - **An all-zero first draw is read as a dead device** ([`readiness`]): wrong with probability
//!   2^-64 to 2^-2048 by source, and the safe way round.
//! - The check is bring-up only; continuous health testing is a decision still to be made.
//! - No request carries [`VERSION`], and the service answers no query for the versions it serves,
//!   so a client built against another version is not refused at bind.
//!
//! Name: ratified 2026-08-23 (calef): the service's own word, `entropy`, with the contract suffix
//! `_contract` (calef's 2026-10-18 ruling).

#![cfg_attr(not(test), no_std)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

/// The most bytes one request can carry: the reply's second word.
///
/// A `CALL` replies with two words. The first is the count, so the bytes get exactly one word.
/// Asking for more than this is not an error, it is answered with this many, and the caller loops;
/// see [`requested_bytes`].
///
/// Name: ratified 2026-10-27 (#2797).
pub const MAXIMUM_BYTES: u64 = 8;

/// Where a request packs its opcode: bits 63:56 of the first `CALL` word, the same position its
/// sibling message contracts use, so the contracts read alike.
pub const OPERATION_SHIFT: u32 = 56;

/// The protocol's version, one for the operation set and the reply shape together: one constant
/// per wire format. A break bumps this and owes a dated ledger entry; an addition (a new opcode, an
/// optional field) owes no bump. A program declares it beside [`INTERFACE_ID`]; no peer checks it
/// at bind yet (see this crate's `BUGS`).
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane milestone/956-checkable-interface-versions.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 12, naming the entropy service
/// request and reply. The number is this interface's for good: no other interface takes it, and a
/// format change is a new version, never a new number. The project that links these crates keeps the table of numbers and holds it to that rule. A
/// program declares the pair of this id and [`VERSION`].
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane lane/956-interface-ids-for-the-owed-formats.
pub const INTERFACE_ID: u64 = 12;

/// **Give me this many random bytes.** The only operation. The byte count is clamped to
/// [`MAXIMUM_BYTES`] by [`requested_bytes`], so a client that asks for more is answered rather than
/// refused.
pub const GET: u64 = 1;

/// Build a request word: the opcode and the byte count the client wants.
pub const fn request(operation: u64, byte_count: u64) -> u64 {
    (operation << OPERATION_SHIFT) | (byte_count & 0xff)
}

/// The opcode of a request word.
pub const fn operation(request_word: u64) -> u64 {
    request_word >> OPERATION_SHIFT
}

/// How many bytes a request asks for, clamped to what one reply can carry. Clamping here rather
/// than refusing keeps the server's loop free of an error path that only a buggy client reaches:
/// a request for 500 bytes is a request for eight, answered, and the client asks again.
pub const fn requested_bytes(request_word: u64) -> u64 {
    let byte_count = request_word & 0xff;
    if byte_count > MAXIMUM_BYTES {
        MAXIMUM_BYTES
    } else {
        byte_count
    }
}

/// The word the entropy service SENDs on its readiness endpoint once the device is up and its
/// first bytes are in hand. ASCII `RNGUP`, so a hex dump of a report reads. A bring-up failure
/// reports [`DEAD`]` | step` instead, the same shape every driver here uses.
///
/// **"Its first bytes are in hand" is a claim about the bytes, not about the handshake**: a service
/// holding nothing, or a bufferful of zeros, must not send it, and [`readiness`] is what makes the
/// claim.
pub const READY: u64 = 0x_52_4E_47_55_50;

/// **The word a bring-up failure reports instead of [`READY`]**, with the step that failed in its
/// low bits: `0xDEAD_0000_0000_0000 | step`. Named here rather than spelled in each driver because
/// it is the other half of the readiness contract, and because a reader decoding a report word
/// should not have to find the literal in three programs to know what it means.
pub const DEAD: u64 = 0xDEAD_0000_0000_0000;

/// **The service asked and got nothing at all**: the device never answered inside the driver's own
/// bound. A dry source, an absent one, or a register window that reads as nothing.
///
/// Steps `0x01..=0x0f` are each backend's own, so the two steps every backend shares start at
/// `0x10` and cannot collide with them.
pub const STEP_NO_FIRST_BYTES: u64 = 0x10;

/// **The service asked, was answered, and every byte of the answer was zero.** See [`readiness`]
/// for why that is treated as a dead device rather than as randomness.
pub const STEP_FIRST_ALL_ZERO: u64 = 0x11;

/// Build a bring-up failure's report word from the step that failed.
#[must_use]
pub const fn bring_up_failure(step: u64) -> u64 {
    DEAD | step
}

/// **Decide what a service may say about itself, given the first bytes it drew.** [`READY`] only
/// when at least one of them is nonzero; otherwise a [`bring_up_failure`] naming which of the two
/// shared steps failed. Every backend calls this rather than composing the word itself.
///
/// `first_bytes` is an iterator because the three backends hold their first bufferful in three different
/// places. An empty iterator is [`STEP_NO_FIRST_BYTES`], a different answer from a bufferful of
/// zeros. **Refusing an all-zero draw is a deliberate correctness claim about a random variable**;
/// see this crate's `BUGS`.
///
/// # Examples
///
/// ```
/// use entropy_contract::{READY, STEP_FIRST_ALL_ZERO, STEP_NO_FIRST_BYTES, bring_up_failure, readiness};
///
/// // A device that answered with real bytes.
/// assert_eq!(readiness([0, 0, 0, 7, 0]), READY);
///
/// // A device that answered with nothing, and one that answered with zeros. Two different
/// // failures, and the report word says which.
/// assert_eq!(readiness([]), bring_up_failure(STEP_NO_FIRST_BYTES));
/// assert_eq!(readiness([0; 32]), bring_up_failure(STEP_FIRST_ALL_ZERO));
/// assert_ne!(readiness([0; 32]), READY);
/// ```
pub fn readiness(first_bytes: impl IntoIterator<Item = u8>) -> u64 {
    let mut seen = false;
    for byte in first_bytes {
        if byte != 0 {
            return READY;
        }
        seen = true;
    }
    if seen {
        bring_up_failure(STEP_FIRST_ALL_ZERO)
    } else {
        bring_up_failure(STEP_NO_FIRST_BYTES)
    }
}

/// **The reply's first word when the service has no entropy to give.** Zero bytes delivered, and
/// the second word is meaningless. The service sends this rather than padding, repeating an
/// earlier answer, or substituting a pseudo-random stand-in: a caller that cannot be given
/// randomness must find out, because the alternative is the exact silent-degradation failure
/// that no caller should have to guess at.
pub const NO_ENTROPY: u64 = 0;

/// **Read a reply's first word.** `Some(count)` when the entropy service answered, with `count` the
/// number of valid low bytes in the second word (0 means [`NO_ENTROPY`]). `None` for every word
/// above [`MAXIMUM_BYTES`]: no reply carries such a count, so the word did not come from the service.
///
/// The rule this rests on is the caller's: the kernel it runs on must report every failure of a
/// `CALL` as a word above [`MAXIMUM_BYTES`]. A kernel that returns a small negative error number
/// meets it, since each reads as an enormous `u64`, and then a caller holding no entropy capability
/// finds out without a probe.
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
/// Little-endian, and only the low `byte_count` bytes are meaningful: the rest of the word is whatever the
/// service had left in the register and carries no entropy, so a caller that read them would be
/// mixing zeros into a key and believing otherwise.
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
pub fn copy_delivered_bytes(byte_count: usize, reply_word: u64, out: &mut [u8]) -> usize {
    let copied = byte_count.min(MAXIMUM_BYTES as usize).min(out.len());
    out[..copied].copy_from_slice(&reply_word.to_le_bytes()[..copied]);
    copied
}

/// **Fill `out` with random bytes, one request at a time, refusing a short answer.** `call` is the
/// `CALL` on the entropy endpoint: it takes a request word and returns the two reply words.
///
/// `None` if any reply is not a count (no entropy capability; see [`delivered_bytes`]) or delivers fewer
/// bytes than asked (the service is dry); then `out` is unspecified and must not be used. Asks for
/// [`MAXIMUM_BYTES`] at a time and the remainder last. The `CALL` is a parameter so this crate
/// makes no syscall; the caller passes a real one. The standard library's platform abstraction
/// layer accepts partial counts instead, deliberately.
///
/// Name: ratified 2026-10-07 (#1795). std's `_with` suffix, for a function that takes a closure.
pub fn fill_with(out: &mut [u8], mut call: impl FnMut(u64) -> (u64, u64)) -> Option<()> {
    let mut filled = 0;
    while filled < out.len() {
        let byte_count = (out.len() - filled).min(MAXIMUM_BYTES as usize);
        let (first_reply_word, reply_word) = call(request(GET, byte_count as u64));
        if delivered_bytes(first_reply_word)? != byte_count {
            return None;
        }
        filled += copy_delivered_bytes(byte_count, reply_word, &mut out[filled..]);
    }
    Some(())
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
        assert_eq!(got, Some(()));
        assert_eq!(calls, 2);
        assert_eq!(out[..8], [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(out[8..], [2, 4, 6, 8, 10, 12, 14, 16]);
    }

    /// A short answer on the second trip refuses the whole fill, which is what the four programs'
    /// copies of `random16` did before this existed.
    #[test]
    fn a_short_answer_is_a_refusal() {
        let mut trip = 0;
        let got = fill_with(&mut [0u8; 16], |_| {
            trip += 1;
            (if trip == 2 { 7 } else { 8 }, 0)
        });
        assert_eq!(got, None);
    }

    /// No capability: the kernel's `NoSuchSlot` in the count register, and no second call.
    #[test]
    fn a_missing_endpoint_is_a_refusal_after_one_call() {
        let mut calls = 0;
        let got = fill_with(&mut [0u8; 16], |_| {
            calls += 1;
            (-4i64 as u64, 0)
        });
        assert_eq!(got, None);
        assert_eq!(calls, 1);
    }

    /// A dry service (zero bytes, `NO_ENTROPY`) is a refusal too, not a loop that never ends.
    #[test]
    fn a_dry_service_is_a_refusal() {
        assert_eq!(fill_with(&mut [0u8; 8], |_| (NO_ENTROPY, 0)), None);
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
        assert_eq!(got, Some(()));
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

    /// **The property the whole error story rests on**: no count the service can send collides with
    /// any error the kernel can return, so "no entropy capability" is distinguishable from "no
    /// entropy" without a probe. `abi::Error` is -1..-8; those are the words a `CALL` on an empty
    /// slot puts in the first register.
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

    /// The high bytes of the reply word are not entropy and must not reach the caller's buffer.
    #[test]
    fn only_the_delivered_bytes_are_copied_out() {
        let mut out = [0xAAu8; 8];
        let copied = copy_delivered_bytes(3, 0xFFFF_FFFF_FF03_0201, &mut out);
        assert_eq!(copied, 3);
        assert_eq!(out, [0x01, 0x02, 0x03, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA]);
    }

    /// **The defect this function exists to prevent** (2026-09-04, radon): a service holding a
    /// bufferful of zeros must not be able to report [`READY`]. Asserted against every width a
    /// backend actually draws, because the predicate is "no nonzero byte anywhere" rather than
    /// "the first word is zero", and a check written the second way would pass this test at 8
    /// bytes and let a 256-byte direct memory access (DMA) page through.
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

    /// **And a device that gave nothing is a different failure from one that gave zeros.** Both
    /// refuse [`READY`]; a bench session reading the report word has to be able to tell a gated
    /// clock from a source that answered wrongly, which is what the two steps are for.
    #[test]
    fn nothing_at_all_and_all_zero_bytes_are_distinguishable() {
        assert_eq!(readiness([]), bring_up_failure(STEP_NO_FIRST_BYTES));
        assert_ne!(
            readiness([]),
            readiness([0; 32]),
            "a dry device and a zeroing device report the same step",
        );
        assert_eq!(DEAD & 0xff, 0, "a step has room in the low bits");
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

    /// No bring-up failure can be mistaken for the ready word or for a byte count, which is the
    /// same non-overlap property [`delivered_bytes`] rests on, one message earlier in the handshake.
    #[test]
    fn a_bring_up_failure_is_never_ready_and_never_a_count() {
        for step in [STEP_NO_FIRST_BYTES, STEP_FIRST_ALL_ZERO, 0x01, 0xff] {
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
