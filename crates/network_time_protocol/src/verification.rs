use super::*;

/// The nanosecond bound this harness proves the round trip over: 2^16, not the whole range.
///
/// The property is two multiplications composed, which a bit-blasting model checker handles badly:
/// with kissat, 2^16 takes 9 s, 2^20 takes 212 s, and 2^30 was killed at 10 minutes, twice. The
/// full claim is proved by running the code on all 10^9 values in 0.6 s
/// ([`super::tests::every_nanosecond_survives_the_round_trip`]); this harness guards the low
/// corner, where 1 ns is 4.29 ticks and truncating instead of rounding turns it into 0. See
/// notes/ntp/what-is-proved.md.
const PROVED_NANOS: u64 = 1 << 16;

/// **Nanoseconds survive the fixed-point conversion**, for every value under [`PROVED_NANOS`].
/// See that constant for what is and is not covered and why.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.nanoseconds_survive_the_fixed_point.patch`
#[kani::proof]
fn nanoseconds_survive_the_fixed_point() {
    let nanos: u32 = kani::any();
    kani::assume((nanos as u64) < PROVED_NANOS);
    assert_eq!(ticks_to_nanos(nanos_to_ticks(nanos)), nanos);
}

/// **The era pivot is exact over the whole representable window**: for every Unix second in
/// [`UNIX_MIN`]`..`[`UNIX_LIMIT`], 1970 to 2104, encoding and decoding returns the second it
/// started with. Quantified rather than sampled, and the window spans the 2036 rollover, so this
/// covers the era-0 branch, the era-1 branch, and the boundary between them in one proof. All
/// 4.2 billion of them, in a third of a second, because this half is addition.
///
/// It asserts the seconds; the nanoseconds are the sweep named on [`PROVED_NANOS`]. The halves
/// compose because `from_unix` builds `(seconds << 32) | ticks` with `ticks < 2^32`, disjoint
/// bits that `to_unix` reads back through separate accessors.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.the_era_pivot_is_exact_over_the_window.patch`
#[kani::proof]
fn the_era_pivot_is_exact_over_the_window() {
    let secs: u64 = kani::any();
    kani::assume(secs < UNIX_LIMIT);

    let t = Timestamp::from_unix(secs, 0).expect("in range must encode");
    assert_eq!(t.to_unix().0, secs);
    assert_eq!(t.fraction(), 0);
}

/// **Decoding is total, and it never produces an invalid sub-second.** For every one of the
/// 2^64 bit patterns a packet can carry: `to_unix` does not panic, and the nanosecond it returns
/// is under 1e9. The second half is the clamp in [`ticks_to_nanos`], and it matters because a
/// nanosecond field of exactly 1e9 is the kind of thing that panics inside a `Duration`
/// constructor three layers away from the packet that caused it.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.decoding_any_wire_value_is_total.patch`
#[kani::proof]
fn decoding_any_wire_value_is_total() {
    let bits: u64 = kani::any();
    let (_secs, nanos) = Timestamp::from_bits(bits).to_unix();
    assert!((nanos as u64) < NANOS_PER_SEC);
}

/// **Out of range is refused, never aliased.** For every `secs`, `from_unix` returns `Some` if
/// and only if the value is inside the window. This is the other half of the era decision: the
/// encoder does not quietly wrap a 2105 timestamp onto 1969.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.out_of_range_is_refused.patch`
#[kani::proof]
fn out_of_range_is_refused() {
    let secs: u64 = kani::any();
    let nanos: u32 = kani::any();
    let encoded = Timestamp::from_unix(secs, nanos);
    assert_eq!(
        encoded.is_some(),
        secs < UNIX_LIMIT && (nanos as u64) < NANOS_PER_SEC
    );
}

/// **Parse then serialise is the identity, on every 48-byte input there is.** Not "on a
/// well-formed packet": there is no such qualifier, because parse judges nothing. That is what
/// makes this provable over 2^384 inputs and what keeps every rejection in one function.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.parse_then_serialise_is_the_identity.patch`
#[kani::proof]
#[kani::unwind(49)]
fn parse_then_serialise_is_the_identity() {
    let wire: [u8; PACKET_LEN] = kani::any();
    let p = Packet::parse(&wire).expect("48 bytes always parse");
    assert_eq!(p.to_bytes(), wire);
}

/// **A reply whose origin field is not our nonce is always rejected**, over every packet an
/// attacker could send. The load-bearing security check, proved rather than tested: no
/// combination of the other 40 bytes gets a packet past it.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.an_unmatched_origin_is_always_rejected.patch`
#[kani::proof]
#[kani::unwind(49)]
fn an_unmatched_origin_is_always_rejected() {
    let wire: [u8; PACKET_LEN] = kani::any();
    let nonce: u64 = kani::any();
    let sent: u64 = kani::any();
    let dest: u64 = kani::any();

    let q = Query::with_nonce(Timestamp::from_bits(sent), Timestamp::from_bits(nonce));
    let origin = u64::from_be_bytes([
        wire[24], wire[25], wire[26], wire[27], wire[28], wire[29], wire[30], wire[31],
    ]);
    kani::assume(origin != nonce);
    assert!(q.accept(&wire, Timestamp::from_bits(dest)).is_err());
}

/// **Accepting is total, and an accepted sample has a non-negative delay.** For any 48 bytes and
/// any three timestamps: `accept` does not panic, does not overflow, and if it returns a sample
/// then the delay is non-negative, so the ±δ/2 bound a client reasons with is never derived
/// from a nonsense δ. This is the proof that a hostile packet cannot make the arithmetic
/// misbehave, which is the property the `i128` intervals exist for.
/// Falsification: replayable `crates/network_time_protocol/falsifications/verification.accepting_is_total_and_a_sample_is_coherent.patch`
#[kani::proof]
#[kani::unwind(49)]
fn accepting_is_total_and_a_sample_is_coherent() {
    let wire: [u8; PACKET_LEN] = kani::any();
    let nonce: u64 = kani::any();
    let sent: u64 = kani::any();
    let dest: u64 = kani::any();

    let q = Query::with_nonce(Timestamp::from_bits(sent), Timestamp::from_bits(nonce));
    if let Ok(s) = q.accept(&wire, Timestamp::from_bits(dest)) {
        // **The two predicate-shaped claims are stated over the numbers** (milestone 211).
        // `accept` refuses on `delay.is_negative()` and on `p.transmit.is_zero()`, so
        // asserting through those same two predicates proves only that the guard and the
        // assertion agree: an `is_negative` stuck at false satisfies both sides and a
        // negative delay reaches the caller with this harness still green.
        assert!(
            s.delay.0 >= 0,
            "an accepted sample carries a negative delay"
        );
        assert!(
            s.response.transmit.0 != 0,
            "an accepted sample carries an unset transmit timestamp",
        );
        assert!(s.response.stratum >= 1 && s.response.stratum <= MAX_STRATUM);
        assert!(s.response.mode == mode::SERVER);
    }
}
