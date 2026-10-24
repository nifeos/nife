//! **The entropy contract** (§44 (entropy is a capability), notes/entropy.md): the request and
//! reply that carry random bytes out of the entropy service, defined once for the service, its
//! clients, the std PAL and the kernel-side tests. One process holds the virtio-rng device; a
//! client holds an endpoint that names no device, `CALL`s with a byte count and gets the bytes in
//! the reply words, at most [`MAX_BYTES`] per round trip. No shared page, against the usual bulk
//! rule, because random bytes are worth exactly that nobody else has seen them, and no byte is
//! transformed, so the claim stays "these are the device's bytes". notes/entropy.md, "The shape"
//! and "The bytes are the device's bytes", has the argument; the `entropy_rtt` benchmark prices it.

//! # BUGS
//!
//! - **An all-zero first draw is read as a dead device** ([`readiness`]): wrong with probability
//!   2^-64 to 2^-2048 by source, and the safe way round (notes/entropy/contract.md).
//! - The check is bring-up only; continuous health testing is
//!   `design/decisions/0137-trng-health-tests.md`'s to decide.
//!
//! Name: ratified 2026-08-23 (calef): the service's own word, `entropy`, with the contract suffix
//! `_contract` (calef's 2026-10-18 ruling).

#![cfg_attr(not(test), no_std)]

/// The most bytes one request can carry: the reply's second word.
///
/// A `CALL` replies with two words. The first is the count, so the bytes get exactly one word.
/// Asking for more than this is not an error, it is answered with this many, and the caller loops;
/// see [`want`].
pub const MAX_BYTES: u64 = 8;

/// Where a request packs its opcode: bits 63:56 of the first `CALL` word, the same position
/// `filesystem_contract` and `line_editor::proto` use, so the contracts read alike.
pub const OPERATION_SHIFT: u32 = 56;

/// **Give me `n` random bytes.** The only operation. `n` is clamped to [`MAX_BYTES`] by [`want`],
/// so a client that asks for more is answered rather than refused.
pub const GET: u64 = 1;

/// Build a request word: the opcode and the byte count the client wants.
pub const fn req(operation: u64, n: u64) -> u64 {
    (operation << OPERATION_SHIFT) | (n & 0xff)
}

/// The opcode of a request word.
pub const fn operation(w0: u64) -> u64 {
    w0 >> OPERATION_SHIFT
}

/// How many bytes a request asks for, clamped to what one reply can carry. Clamping here rather
/// than refusing keeps the server's loop free of an error path that only a buggy client reaches:
/// a request for 500 bytes is a request for eight, answered, and the client asks again.
pub const fn want(w0: u64) -> u64 {
    let n = w0 & 0xff;
    if n > MAX_BYTES { MAX_BYTES } else { n }
}

/// The word the entropy service SENDs on its readiness endpoint once the device is up and its
/// first bytes are in hand. ASCII `RNGUP`, so a hex dump of a report reads. A bring-up failure
/// reports [`DEAD`]` | step` instead, the same shape every driver here uses.
///
/// **"Its first bytes are in hand" is a claim about the bytes, not about the handshake**: a service
/// holding nothing, or a bufferful of zeros, must not send it, and [`readiness`] is what makes the
/// claim (notes/entropy/contract.md, "Ready means bytes in hand").
pub const READY: u64 = 0x_52_4E_47_55_50;

/// **The word a bring-up failure reports instead of [`READY`]**, with the step that failed in its
/// low bits: `0xDEAD_0000_0000_0000 | step`. Named here rather than spelled in each driver because
/// it is the other half of the readiness contract, and because a reader decoding a report word
/// should not have to find the literal in three programs to know what it means.
pub const DEAD: u64 = 0xDEAD_0000_0000_0000;

/// **The service asked and got nothing at all**: the device never answered inside the driver's own
/// bound. A dry source, an absent one, or a register window that reads as nothing.
///
/// Steps `0x01..=0x0f` are each backend's own (`components/src/entropy.rs` numbers its virtio sequence
/// there), so the two steps every backend shares start at `0x10` and cannot collide with them.
pub const STEP_NO_FIRST_BYTES: u64 = 0x10;

/// **The service asked, was answered, and every byte of the answer was zero.** See [`readiness`]
/// for why that is treated as a dead device rather than as randomness.
pub const STEP_FIRST_ALL_ZERO: u64 = 0x11;

/// Build a bring-up failure's report word from the step that failed.
#[must_use]
pub const fn bringup_failure(step: u64) -> u64 {
    DEAD | step
}

/// **Decide what a service may say about itself, given the first bytes it drew.** [`READY`] only
/// when at least one of them is nonzero; otherwise a [`bringup_failure`] naming which of the two
/// shared steps failed. Every backend calls this rather than composing the word itself.
///
/// `first` is an iterator because the three backends hold their first bufferful in three different
/// places. An empty iterator is [`STEP_NO_FIRST_BYTES`], a different answer from a bufferful of
/// zeros. **Refusing an all-zero draw is a deliberate correctness claim about a random variable**;
/// see this crate's `BUGS`.
///
/// # Examples
///
/// ```
/// use entropy_contract::{READY, STEP_FIRST_ALL_ZERO, STEP_NO_FIRST_BYTES, bringup_failure, readiness};
///
/// // A device that answered with real bytes.
/// assert_eq!(readiness([0, 0, 0, 7, 0]), READY);
///
/// // A device that answered with nothing, and one that answered with zeros. Two different
/// // failures, and the report word says which.
/// assert_eq!(readiness([]), bringup_failure(STEP_NO_FIRST_BYTES));
/// assert_eq!(readiness([0; 32]), bringup_failure(STEP_FIRST_ALL_ZERO));
/// assert_ne!(readiness([0; 32]), READY);
/// ```
pub fn readiness(first: impl IntoIterator<Item = u8>) -> u64 {
    let mut seen = false;
    for byte in first {
        if byte != 0 {
            return READY;
        }
        seen = true;
    }
    if seen {
        bringup_failure(STEP_FIRST_ALL_ZERO)
    } else {
        bringup_failure(STEP_NO_FIRST_BYTES)
    }
}

/// **The reply's first word when the service has no entropy to give.** Zero bytes delivered, and
/// the second word is meaningless. The service sends this rather than padding, repeating an
/// earlier answer, or substituting a pseudo-random stand-in: a caller that cannot be given
/// randomness must find out, because the alternative is the exact silent-degradation failure
/// DECISIONS §42 forbids.
pub const NO_ENTROPY: u64 = 0;

/// **Read a reply's first word.** `Some(n)` when the entropy service answered, with `n` the number
/// of valid low bytes in the second word (0 means [`NO_ENTROPY`]). `None` when the word did not
/// come from the service at all: every kernel failure from a `CALL` is a small negative
/// (`abi::Error`, -1 to -8), an enormous `u64`, so a caller holding no entropy capability finds out
/// without a probe (notes/entropy.md, "One number that could not collide").
///
/// # Examples
///
/// A count and a kernel error cannot be confused, and "none to give" is a third answer:
///
/// ```
/// use entropy_contract::{MAX_BYTES, NO_ENTROPY, delivered};
///
/// // Every count the service can send.
/// for n in 0..=MAX_BYTES {
///     assert_eq!(delivered(n), Some(n as usize));
/// }
/// // `abi::Error` is -1 to -8; as a u64 each is enormous, so none can be read as a count.
/// assert_eq!(delivered(-4i64 as u64), None); // an empty slot: no entropy capability
///
/// // And "the service has none to give" is a third answer, distinct from both.
/// assert_eq!(delivered(NO_ENTROPY), Some(0));
/// ```
pub const fn delivered(r0: u64) -> Option<usize> {
    if r0 <= MAX_BYTES {
        Some(r0 as usize)
    } else {
        None
    }
}

/// Copy the `n` delivered bytes out of a reply's second word into `out`, returning how many landed.
///
/// Little-endian, and only the low `n` bytes are meaningful: the rest of the word is whatever the
/// service had left in the register and carries no entropy, so a caller that read them would be
/// mixing zeros into a key and believing otherwise.
///
/// # Examples
///
/// A reply is eight bytes, so filling anything larger is a loop: a twenty-byte key is three round
/// trips, the last one short.
///
/// ```
/// use entropy_contract::{GET, MAX_BYTES, delivered, operation, req, take, want};
///
/// // Stands in for the `CALL`: the real service reads the same two accessors off the request word
/// // and answers with a count and a word of bytes.
/// fn service(w0: u64) -> (u64, u64) {
///     assert_eq!(operation(w0), GET);
///     (want(w0), 0x0807_0605_0403_0201)
/// }
///
/// let mut key = [0u8; 20];
/// let mut filled = 0;
/// let mut round_trips = 0;
/// while filled < key.len() {
///     let (r0, r1) = service(req(GET, (key.len() - filled) as u64));
///     round_trips += 1;
///     // `None` would mean the word is a kernel error rather than a count. See below.
///     let n = delivered(r0).expect("the service answered");
///     assert_ne!(n, 0, "a zero count is NO_ENTROPY: fail, do not retry");
///     filled += take(n, r1, &mut key[filled..]);
/// }
/// assert_eq!(round_trips, 3); // 8 + 8 + 4, because MAX_BYTES is 8
/// assert_eq!(MAX_BYTES, 8);
/// assert_eq!(&key[16..], &[0x01, 0x02, 0x03, 0x04]);
/// ```
pub fn take(n: usize, word: u64, out: &mut [u8]) -> usize {
    let n = n.min(MAX_BYTES as usize).min(out.len());
    out[..n].copy_from_slice(&word.to_le_bytes()[..n]);
    n
}

/// **Fill `out` with random bytes, one request at a time, refusing a short answer.** `call` is the
/// `CALL` on the entropy endpoint: it takes a request word and returns the two reply words.
///
/// `None` if any reply is not a count (no entropy capability; see [`delivered`]) or delivers fewer
/// bytes than asked (the service is dry); then `out` is unspecified and must not be used. Asks for
/// [`MAX_BYTES`] at a time and the remainder last. The `CALL` is a parameter so this crate makes no
/// syscall; `user_mode_runtime::entropy::fill` passes a real one. The std PAL accepts partial
/// counts instead, deliberately (notes/entropy/contract.md).
///
/// Name: ratified 2026-10-07 (calef, #1795). His word: "Yes". std's `_with` suffix, for a function
/// that takes a closure.
pub fn fill_with(out: &mut [u8], mut call: impl FnMut(u64) -> (u64, u64)) -> Option<()> {
    let mut filled = 0;
    while filled < out.len() {
        let want = (out.len() - filled).min(MAX_BYTES as usize);
        let (r0, r1) = call(req(GET, want as u64));
        if delivered(r0)? != want {
            return None;
        }
        filled += take(want, r1, &mut out[filled..]);
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
        let got = fill_with(&mut out, |w0| {
            calls += 1;
            assert_eq!(operation(w0), GET);
            (want(w0), 0x0807_0605_0403_0201 * calls)
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
        let mut i = 0;
        let got = fill_with(&mut [0u8; 20], |w0| {
            asked[i] = want(w0);
            i += 1;
            (want(w0), u64::MAX)
        });
        assert_eq!(got, Some(()));
        assert_eq!(asked, [8, 8, 4]);
    }

    #[test]
    fn a_request_round_trips_its_opcode_and_count() {
        let w = req(GET, 5);
        assert_eq!(operation(w), GET);
        assert_eq!(want(w), 5);
    }

    /// Pins the wire layout as an exact word, with an opcode that is not `GET`. `GET` is 1, so an
    /// `operation` that ignored its input and answered 1 would pass every round trip built from it; this
    /// is the one place the shift is checked against a number the protocol does not define.
    #[test]
    fn the_request_word_layout_is_exact() {
        let w = req(0x7f, 5);
        assert_eq!(w, 0x7f00_0000_0000_0005);
        assert_eq!(operation(w), 0x7f);
        assert_eq!(want(w), 5);
    }

    /// A client asking for more than one reply can carry is answered with a full reply, not an
    /// error. The clamp lives in the contract so the server and the caller agree on it without
    /// either one enforcing it.
    #[test]
    fn an_oversized_request_is_clamped_rather_than_refused() {
        assert_eq!(want(req(GET, 200)), MAX_BYTES);
        assert_eq!(want(req(GET, MAX_BYTES)), MAX_BYTES);
    }

    /// **The property the whole error story rests on**: no count the service can send collides with
    /// any error the kernel can return, so "no entropy capability" is distinguishable from "no
    /// entropy" without a probe. `abi::Error` is -1..-8; those are the words a `CALL` on an empty
    /// slot puts in the first register.
    #[test]
    fn a_kernel_refusal_is_never_mistaken_for_a_byte_count() {
        for n in 0..=MAX_BYTES {
            assert_eq!(delivered(n), Some(n as usize));
        }
        for err in 1i64..=8 {
            assert_eq!(delivered((-err) as u64), None, "kernel error -{err}");
        }
        assert_eq!(delivered(MAX_BYTES + 1), None);
    }

    /// The high bytes of the reply word are not entropy and must not reach the caller's buffer.
    #[test]
    fn only_the_delivered_bytes_are_copied_out() {
        let mut out = [0xAAu8; 8];
        let n = take(3, 0xFFFF_FFFF_FF03_0201, &mut out);
        assert_eq!(n, 3);
        assert_eq!(out, [0x01, 0x02, 0x03, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA]);
    }

    /// **The defect this function exists to prevent** (2026-09-04, radon): a service holding a
    /// bufferful of zeros must not be able to report [`READY`]. Asserted against every width a
    /// backend actually draws, because the predicate is "no nonzero byte anywhere" rather than
    /// "the first word is zero", and a check written the second way would pass this test at 8
    /// bytes and let a 256-byte DMA page through.
    #[test]
    fn an_all_zero_first_bufferful_is_never_ready() {
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
            bringup_failure(STEP_FIRST_ALL_ZERO),
            "a zero bufferful must name its own step, not just refuse to be ready",
        );
    }

    /// **And a device that gave nothing is a different failure from one that gave zeros.** Both
    /// refuse [`READY`]; a bench session reading the report word has to be able to tell a gated
    /// clock from a source that answered wrongly, which is what the two steps are for.
    #[test]
    fn nothing_at_all_and_a_zero_bufferful_are_distinguishable() {
        assert_eq!(readiness([]), bringup_failure(STEP_NO_FIRST_BYTES));
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
        let mut buf = [0u8; 32];
        buf[31] = 1;
        assert_eq!(readiness(buf), READY);
        buf = [0u8; 32];
        buf[0] = 0x80;
        assert_eq!(readiness(buf), READY);
    }

    /// No bring-up failure can be mistaken for the ready word or for a byte count, which is the
    /// same non-overlap property [`delivered`] rests on, one message earlier in the handshake.
    #[test]
    fn a_bringup_failure_is_never_ready_and_never_a_count() {
        for step in [STEP_NO_FIRST_BYTES, STEP_FIRST_ALL_ZERO, 0x01, 0xff] {
            let word = bringup_failure(step);
            assert_ne!(word, READY);
            assert_eq!(delivered(word), None);
            assert_eq!(word & 0xff, step);
        }
    }

    /// A short buffer is the caller's business, not a panic: `take` fills what fits.
    #[test]
    fn a_short_buffer_takes_what_fits() {
        let mut out = [0u8; 2];
        assert_eq!(take(8, u64::MAX, &mut out), 2);
        assert_eq!(out, [0xFF, 0xFF]);
    }
}
