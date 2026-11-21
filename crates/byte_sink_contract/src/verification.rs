//! **Machine-checked proofs (Kani) of the byte-sink framing**: the decoder is total, and the five
//! properties a sink implementer relies on hold for every input, not the sampled ones the unit
//! tests pin. Behind `#[cfg(kani)]`, so only `cargo kani` compiles them.
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane maintainer/outside-review-fixes-1075-1088,
//! after the module every other crate's harnesses live in.

use super::*;

/// **Any prefix of up to sixteen symbolic bytes survives `pack` then `unpack`**, with `taken` equal
/// to its length. A concrete loop over the seventeen lengths, since a symbolic length does not
/// finish.
/// Falsification: replayable `crates/byte_sink_contract/falsifications/verification.any_prefix_survives_pack_then_unpack.patch`
#[kani::proof]
#[kani::unwind(18)]
fn any_prefix_survives_pack_then_unpack() {
    let src: [u8; MAXIMUM_INLINE_BYTES] = kani::any();
    let mut n = 0;
    while n <= MAXIMUM_INLINE_BYTES {
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
        assert_eq!(out[..n], src[..n]);
        n += 1;
    }
}

/// **`unpack` never answers more bytes than a message holds**, for any three words.
/// Falsification: replayable `crates/byte_sink_contract/falsifications/verification.unpack_never_answers_more_than_a_message_holds.patch`
#[kani::proof]
fn unpack_never_answers_more_than_a_message_holds() {
    let (word_zero, word_one, word_two): (u64, u64, u64) = kani::any();
    let mut out = [0u8; MAXIMUM_INLINE_BYTES];
    if let Message::Bytes(n) = unpack(MessageWords::new(word_zero, word_one, word_two), &mut out) {
        assert!(n <= MAXIMUM_INLINE_BYTES);
    }
}

/// **`with_severity` then `severity` reads back the clamped level**, and leaves the count, the
/// opcode and the kind of any first word unchanged.
/// Falsification: replayable `crates/byte_sink_contract/falsifications/verification.severity_reads_back_clamped_and_leaves_the_rest.patch`
#[kani::proof]
fn severity_reads_back_clamped_and_leaves_the_rest() {
    let word_zero: u64 = kani::any();
    let level: u8 = kani::any();
    let marked = with_severity(word_zero, level);
    assert_eq!(severity(marked), Some(level.min(7)));
    assert_eq!(len(marked), len(word_zero));
    assert_eq!(operation(marked), operation(word_zero));
    assert_eq!(kind(marked), kind(word_zero));
}

/// **`classify` is `Gone` exactly for [`GONE`] and `Ok` exactly for a non-negative return.**
/// Falsification: replayable `crates/byte_sink_contract/falsifications/verification.classify_separates_the_three_outcomes.patch`
#[kani::proof]
fn classify_separates_the_three_outcomes() {
    let returned: i64 = kani::any();
    assert_eq!(classify(returned) == Sent::Ok, returned >= 0);
    assert_eq!(classify(returned) == Sent::Gone, returned == GONE);
}

/// **An end of stream decodes from exactly one message**: over every three words, [`unpack`]
/// answers [`Message::EndOfStream`] if and only if they are `(end_of_stream(), 0, 0)`.
/// Falsification: replayable `crates/byte_sink_contract/falsifications/verification.an_end_of_stream_decodes_exactly_from_its_one_message.patch`
#[kani::proof]
fn an_end_of_stream_decodes_exactly_from_its_one_message() {
    let word_zero: u64 = kani::any();
    let word_one: u64 = kani::any();
    let word_two: u64 = kani::any();
    let mut out = [0u8; MAXIMUM_INLINE_BYTES];
    let decoded = unpack(MessageWords::new(word_zero, word_one, word_two), &mut out);
    assert_eq!(
        decoded == Message::EndOfStream,
        (word_zero, word_one, word_two) == (end_of_stream(), 0, 0)
    );
}

/// **An end-of-stream first word equals no bytes message of any length.**
/// Falsification: replayable `crates/byte_sink_contract/falsifications/verification.end_of_stream_is_no_bytes_message.patch`
#[kani::proof]
fn end_of_stream_is_no_bytes_message() {
    let n: u32 = kani::any();
    kani::assume(n <= MAXIMUM_INLINE_BYTES as u32);
    assert_ne!(end_of_stream(), first_word(OPERATION_BYTES, n));
}
