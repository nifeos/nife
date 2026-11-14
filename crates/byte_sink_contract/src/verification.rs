//! **Machine-checked proofs (Kani) of the byte-sink framing**: the decoder is total, and the five
//! properties a sink implementer relies on hold for every input, not the sampled ones the unit
//! tests pin. Behind `#[cfg(kani)]`, so only `cargo kani` (script/verify) compiles them, and the
//! standard library's copy of this crate, which stops at its test module, never sees them.
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
    let src: [u8; INLINE_MAXIMUM] = kani::any();
    let mut n = 0;
    while n <= INLINE_MAXIMUM {
        let (w0, w1, w2, taken) = pack(&src[..n]);
        assert_eq!(taken, n);
        let mut out = [0u8; INLINE_MAXIMUM];
        assert_eq!(unpack(w0, w1, w2, &mut out), Msg::Bytes(n));
        assert_eq!(out[..n], src[..n]);
        n += 1;
    }
}

/// **`unpack` never answers more bytes than a message holds**, for any three words.
/// Falsification: replayable `crates/byte_sink_contract/falsifications/verification.unpack_never_answers_more_than_a_message_holds.patch`
#[kani::proof]
fn unpack_never_answers_more_than_a_message_holds() {
    let (w0, w1, w2): (u64, u64, u64) = kani::any();
    let mut out = [0u8; INLINE_MAXIMUM];
    if let Msg::Bytes(n) = unpack(w0, w1, w2, &mut out) {
        assert!(n <= INLINE_MAXIMUM);
    }
}

/// **`with_severity` then `severity` reads back the clamped level**, and leaves the count, the
/// opcode and the kind of any first word unchanged.
/// Falsification: replayable `crates/byte_sink_contract/falsifications/verification.severity_reads_back_clamped_and_leaves_the_rest.patch`
#[kani::proof]
fn severity_reads_back_clamped_and_leaves_the_rest() {
    let w0: u64 = kani::any();
    let level: u8 = kani::any();
    let marked = with_severity(w0, level);
    assert_eq!(severity(marked), Some(level.min(7)));
    assert_eq!(len(marked), len(w0));
    assert_eq!(operation(marked), operation(w0));
    assert_eq!(kind(marked), kind(w0));
}

/// **`classify` is `Gone` exactly for [`GONE`] and `Ok` exactly for a non-negative return.**
/// Falsification: replayable `crates/byte_sink_contract/falsifications/verification.classify_separates_the_three_outcomes.patch`
#[kani::proof]
fn classify_separates_the_three_outcomes() {
    let returned: i64 = kani::any();
    assert_eq!(classify(returned) == Sent::Ok, returned >= 0);
    assert_eq!(classify(returned) == Sent::Gone, returned == GONE);
}

/// **An end-of-stream message unpacks as `Eof` whatever its second and third words hold**, and its
/// first word equals no bytes message of any length.
/// Falsification: replayable `crates/byte_sink_contract/falsifications/verification.end_of_stream_ignores_its_other_words.patch`
#[kani::proof]
fn end_of_stream_ignores_its_other_words() {
    let (w1, w2): (u64, u64) = kani::any();
    let mut out = [0u8; INLINE_MAXIMUM];
    assert_eq!(unpack(eof(), w1, w2, &mut out), Msg::Eof);
    let n: u64 = kani::any();
    kani::assume(n <= INLINE_MAXIMUM as u64);
    assert_ne!(eof(), req(OPERATION_BYTES, n));
}
